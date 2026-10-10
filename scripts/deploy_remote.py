#!/usr/bin/env python3
"""
Local Docker build -> transfer image -> remote load & restart.

Usage (from repo root):
  # first time: copy deploy.env.example -> deploy.env and fill secrets
  python scripts/deploy_remote.py              # build + deploy
  python scripts/deploy_remote.py --skip-build # only transfer cached local image
  python scripts/deploy_remote.py --sync-code  # also rsync/git sync source tree

Requires: Docker Desktop running locally, paramiko, deploy.env
"""
from __future__ import annotations

import argparse
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path

import paramiko
from paramiko import SFTPClient

ROOT = Path(__file__).resolve().parents[1]


def load_env(path: Path) -> dict[str, str]:
    data: dict[str, str] = {}
    if not path.exists():
        raise SystemExit(f"Missing {path}. Copy deploy.env.example -> deploy.env first.")
    for raw in path.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        data[key.strip()] = value.strip().strip('"').strip("'")
    return data


def run(cmd: list[str], cwd: Path | None = None) -> None:
    print("+", " ".join(cmd), flush=True)
    subprocess.run(cmd, cwd=str(cwd or ROOT), check=True)


def ssh_connect(cfg: dict[str, str]) -> paramiko.SSHClient:
    client = paramiko.SSHClient()
    client.set_missing_host_key_policy(paramiko.AutoAddPolicy())
    client.connect(
        cfg["DEPLOY_HOST"],
        port=int(cfg.get("DEPLOY_PORT", "22")),
        username=cfg.get("DEPLOY_USER", "root"),
        password=cfg["DEPLOY_PASSWORD"],
        timeout=30,
        allow_agent=False,
        look_for_keys=False,
    )
    return client


def _safe_print(text: str) -> None:
    """Avoid Windows console GBK crashes on emoji / CJK in remote logs."""
    enc = getattr(sys.stdout, "encoding", None) or "utf-8"
    try:
        print(text, end="" if text.endswith("\n") else "\n", flush=True)
    except UnicodeEncodeError:
        print(text.encode(enc, "replace").decode(enc, "replace"), flush=True)


def ssh_exec(client: paramiko.SSHClient, cmd: str, check: bool = True) -> str:
    _safe_print(f"$ {cmd}")
    _stdin, stdout, stderr = client.exec_command(cmd, timeout=3600)
    out = stdout.read().decode("utf-8", "replace")
    err = stderr.read().decode("utf-8", "replace")
    code = stdout.channel.recv_exit_status()
    if out:
        _safe_print(out)
    if err.strip():
        _safe_print(err)
    if check and code != 0:
        raise RuntimeError(f"remote cmd failed ({code}): {cmd}")
    return out


def sftp_put(sftp: SFTPClient, local: Path, remote: str) -> None:
    size = local.stat().st_size
    print(f"Upload {local.name} ({size / 1024 / 1024:.1f} MiB) -> {remote}", flush=True)
    transferred = {"n": 0}
    last = {"t": time.time()}

    def cb(done: int, total: int) -> None:
        now = time.time()
        if now - last["t"] < 2 and done != total:
            return
        last["t"] = now
        pct = (done / total * 100) if total else 0
        print(f"  {pct:5.1f}%  {done / 1024 / 1024:.1f}/{total / 1024 / 1024:.1f} MiB", flush=True)

    sftp.put(str(local), remote, callback=cb)


def ensure_remote_env(client: paramiko.SSHClient, cfg: dict[str, str], remote_dir: str) -> None:
    api_key = cfg.get("API_KEY", "sk-change-me")
    web_password = cfg.get("WEB_PASSWORD", "")
    bind_local = cfg.get("ABV_BIND_LOCAL_ONLY", "false")
    port = cfg.get("PORT", "8045")
    log_level = cfg.get("LOG_LEVEL", "info")
    image = cfg.get("DEPLOY_IMAGE", "antigravity-manager:08gg")

    content = "\n".join(
        [
            f"API_KEY={api_key}",
            f"WEB_PASSWORD={web_password}",
            f"ABV_BIND_LOCAL_ONLY={bind_local}",
            f"PORT={port}",
            f"LOG_LEVEL={log_level}",
            f"RUST_LOG={log_level}",
            f"ABV_MAX_BODY_SIZE=104857600",
            f"ABV_HOST_DATA_DIR=/root/.antigravity_tools",
            f"ABV_DATA_DIR=/root/.antigravity_tools",
            f"# pinned image for compose.prod",
            f"DEPLOY_IMAGE={image}",
            "",
        ]
    )
    # Write via heredoc to avoid quoting hell
    escaped = content.replace("'", "'\"'\"'")
    ssh_exec(
        client,
        f"mkdir -p {remote_dir} /root/.antigravity_tools && "
        f"cat > {remote_dir}/.env <<'EOF'\n{content}EOF",
    )


def sync_code(client: paramiko.SSHClient, cfg: dict[str, str], remote_dir: str) -> None:
    """Best-effort: git pull on server if origin is reachable; else skip."""
    print("[sync-code] attempting git pull on server...", flush=True)
    ssh_exec(
        client,
        f"cd {remote_dir} && git fetch --all --prune && "
        f"(git checkout main || true) && "
        f"(git pull --ff-only origin main || git pull --ff-only || true)",
        check=False,
    )


def main() -> int:
    parser = argparse.ArgumentParser(description="Build locally and deploy Docker image to server")
    parser.add_argument("--skip-build", action="store_true", help="Reuse existing local image")
    parser.add_argument("--sync-code", action="store_true", help="git pull on remote before restart")
    parser.add_argument("--keep-tar", action="store_true", help="Keep local image tar after upload")
    args = parser.parse_args()

    cfg = load_env(ROOT / "deploy.env")
    if not cfg.get("DEPLOY_PASSWORD"):
        raise SystemExit("DEPLOY_PASSWORD is empty in deploy.env")

    image = cfg.get("DEPLOY_IMAGE", "antigravity-manager:08gg")
    remote_dir = cfg.get("DEPLOY_REMOTE_DIR", "/web/Antigravity-Manager")
    remote_tar = "/tmp/antigravity-manager-08gg.tar"

    if not args.skip_build:
        print("=== [1/4] Local docker build ===", flush=True)
        run(
            [
                "docker",
                "build",
                "-f",
                "docker/Dockerfile",
                "-t",
                image,
                "--build-arg",
                "USE_MIRROR=auto",
                ".",
            ]
        )
    else:
        print("=== [1/4] Skip build, use local image ===", flush=True)
        run(["docker", "image", "inspect", image])

    print("=== [2/4] Export image tar ===", flush=True)
    tar_path = Path(tempfile.gettempdir()) / "antigravity-manager-08gg.tar"
    if tar_path.exists():
        tar_path.unlink()
    run(["docker", "save", "-o", str(tar_path), image])
    print(f"Tar size: {tar_path.stat().st_size / 1024 / 1024:.1f} MiB", flush=True)

    print("=== [3/4] Upload + load on server ===", flush=True)
    client = ssh_connect(cfg)
    try:
        if args.sync_code:
            sync_code(client, cfg, remote_dir)

        # Ensure compose prod file exists on server (upload from local)
        sftp = client.open_sftp()
        try:
            remote_compose = f"{remote_dir}/docker/docker-compose.prod.yml"
            ssh_exec(client, f"mkdir -p {remote_dir}/docker")
            sftp_put(sftp, ROOT / "docker" / "docker-compose.prod.yml", remote_compose)
            sftp_put(sftp, tar_path, remote_tar)
        finally:
            sftp.close()

        ensure_remote_env(client, cfg, remote_dir)

        ssh_exec(client, f"docker load -i {remote_tar}")
        ssh_exec(client, f"rm -f {remote_tar}")
        ssh_exec(
            client,
            f"cd {remote_dir} && "
            f"docker compose -f docker/docker-compose.prod.yml --env-file .env "
            f"up -d --force-recreate --remove-orphans",
        )

        print("=== [4/4] Health check ===", flush=True)
        time.sleep(3)
        ssh_exec(client, "docker ps --filter name=antigravity-manager --format 'table {{.Names}}\t{{.Status}}\t{{.Image}}'")
        ssh_exec(
            client,
            "curl -fsS -m 10 http://127.0.0.1:8045/health || "
            "curl -fsS -m 10 http://127.0.0.1:8045/ || true",
            check=False,
        )
        ssh_exec(client, "docker logs --tail 40 antigravity-manager", check=False)
    finally:
        client.close()

    if not args.keep_tar and tar_path.exists():
        tar_path.unlink(missing_ok=True)

    print("\nDEPLOY_OK")
    print(f"Web UI / API: http://{cfg['DEPLOY_HOST']}:8045")
    print("Model: Auto_08gg  |  Base URL: http://HOST:8045/v1  or  /v2")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except subprocess.CalledProcessError as exc:
        print(f"Command failed: {exc}", file=sys.stderr)
        raise SystemExit(exc.returncode or 1)
    except Exception as exc:  # noqa: BLE001
        print(f"Deploy failed: {exc}", file=sys.stderr)
        raise SystemExit(1)
