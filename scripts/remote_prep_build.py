#!/usr/bin/env python3
"""Prepare remote host for docker build: mirror, swap, pull base images."""
from __future__ import annotations

import os
import sys

import paramiko


def main() -> int:
    host = os.environ["DEPLOY_HOST"]
    port = int(os.environ.get("DEPLOY_PORT", "2012"))
    user = os.environ.get("DEPLOY_USER", "root")
    password = os.environ["DEPLOY_PASSWORD"]

    client = paramiko.SSHClient()
    client.set_missing_host_key_policy(paramiko.AutoAddPolicy())
    client.connect(host, port=port, username=user, password=password, timeout=30, allow_agent=False, look_for_keys=False)

    script = r"""
set -e
echo '=== docker daemon.json ==='
cat /etc/docker/daemon.json || true

# Ensure useful mirrors
cat >/etc/docker/daemon.json <<'EOF'
{
  "registry-mirrors": [
    "https://docker.m.daocloud.io",
    "https://docker.1panel.live",
    "https://hub.rat.dev"
  ]
}
EOF
systemctl restart docker
sleep 3
docker info | sed -n '/Registry Mirrors/,+6p' || true

# Add 4G swap if missing (server has ~1.6G RAM)
if ! swapon --show | grep -q '/swapfile'; then
  if [ ! -f /swapfile ]; then
    echo '=== creating 4G swapfile ==='
    fallocate -l 4G /swapfile || dd if=/dev/zero of=/swapfile bs=1M count=4096
    chmod 600 /swapfile
    mkswap /swapfile
  fi
  swapon /swapfile || true
fi
swapon --show || true
free -h

# Clear broken proxy env for docker clients if any
unset http_proxy https_proxy HTTP_PROXY HTTPS_PROXY all_proxy ALL_PROXY
mkdir -p /etc/systemd/system/docker.service.d
rm -f /etc/systemd/system/docker.service.d/http-proxy.conf || true
systemctl daemon-reload
systemctl restart docker
sleep 2

echo '=== pull base images via mirror ==='
for img in node:20-slim rust:1-slim-bookworm debian:bookworm-slim; do
  echo "pulling $img ..."
  docker pull "$img" || docker pull "docker.m.daocloud.io/library/${img}" || true
done
docker images | head -30
"""
    print(script[:80], "...")
    _stdin, stdout, stderr = client.exec_command(script, timeout=1200)
    # stream output
    while True:
        line = stdout.readline()
        if not line:
            break
        print(line, end="")
    err = stderr.read().decode("utf-8", "replace")
    code = stdout.channel.recv_exit_status()
    if err.strip():
        print(err)
    client.close()
    return code


if __name__ == "__main__":
    raise SystemExit(main())
