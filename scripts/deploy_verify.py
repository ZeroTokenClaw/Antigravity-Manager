#!/usr/bin/env python3
from __future__ import annotations

import os
import sys

import paramiko


def main() -> int:
    password = os.environ.get("DEPLOY_PASSWORD")
    if not password:
        print("DEPLOY_PASSWORD required", file=sys.stderr)
        return 2
    host = os.environ.get("DEPLOY_HOST", "121.43.107.68")
    port = int(os.environ.get("DEPLOY_PORT", "2012"))
    user = os.environ.get("DEPLOY_USER", "root")

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
        "docker ps --filter name=antigravity-manager --format '{{.Names}} {{.Status}} {{.Image}}'",
        "curl -fsS -m 8 http://127.0.0.1:8045/health",
        "ss -lntp | grep 8045 || true",
    ]
    for cmd in cmds:
        print("===", cmd)
        _i, stdout, _e = client.exec_command(cmd, timeout=30)
        print(stdout.read().decode("utf-8", "replace"), end="")
    client.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
