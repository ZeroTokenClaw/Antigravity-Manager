#!/usr/bin/env python3
"""One-shot SSH probe for deploy target. Do not commit secrets."""
from __future__ import annotations

import os
import sys

import paramiko


def main() -> int:
    host = os.environ.get("DEPLOY_HOST", "121.43.107.68")
    port = int(os.environ.get("DEPLOY_PORT", "2012"))
    user = os.environ.get("DEPLOY_USER", "root")
    password = os.environ.get("DEPLOY_PASSWORD")
    if not password:
        print("DEPLOY_PASSWORD env required", file=sys.stderr)
        return 2

    client = paramiko.SSHClient()
    client.set_missing_host_key_policy(paramiko.AutoAddPolicy())
    client.connect(
        host,
        port=port,
        username=user,
        password=password,
        timeout=20,
        allow_agent=False,
        look_for_keys=False,
    )

    cmds = [
        "uname -a",
        "docker --version",
        "ls -la /web/Antigravity-Manager | head -30",
        "docker ps -a --format 'table {{.Names}}\t{{.Status}}\t{{.Image}}' | head -30",
        "cat /etc/docker/daemon.json 2>/dev/null || echo NO_DAEMON_JSON",
        "ss -lntp | grep -E '8045|7890' || true",
        "df -h / | tail -1",
        "free -h | head -2",
    ]
    for cmd in cmds:
        print("===", cmd)
        _stdin, stdout, stderr = client.exec_command(cmd, timeout=60)
        out = stdout.read().decode("utf-8", "replace")
        err = stderr.read().decode("utf-8", "replace")
        if out:
            print(out, end="" if out.endswith("\n") else "\n")
        if err.strip():
            print("ERR:", err)
    client.close()
    print("SSH_OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
