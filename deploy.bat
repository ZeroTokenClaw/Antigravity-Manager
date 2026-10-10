@echo off
chcp 65001 >nul
setlocal EnableExtensions
cd /d "%~dp0"

if not exist "deploy.env" (
  echo [ERROR] Missing deploy.env. Copy deploy.env.example to deploy.env and fill secrets.
  pause
  exit /b 1
)

echo ======================================================
echo   Antigravity Manager - Fast Remote Docker Deploy
echo ======================================================
echo.
echo   [1] Full: local build + upload + restart  ^(first time / dependency changes^)
echo   [2] Fast: skip build, upload existing image + restart
echo   [3] Verify remote health only
echo   [4] Exit
echo.
set /p "CHOICE=Enter choice [1-4, default 2]: "
if "%CHOICE%"=="" set "CHOICE=2"
if "%CHOICE%"=="4" exit /b 0

where docker >nul 2>&1
if errorlevel 1 (
  echo [ERROR] Docker not found. Start Docker Desktop first.
  pause
  exit /b 1
)

if "%CHOICE%"=="3" (
  python scripts\deploy_verify.py
  pause
  exit /b %ERRORLEVEL%
)

if "%CHOICE%"=="2" (
  python scripts\deploy_remote.py --skip-build --sync-code
  pause
  exit /b %ERRORLEVEL%
)

if "%CHOICE%"=="1" (
  echo [INFO] Prefetch base images via DaoCloud mirrors...
  docker pull docker.m.daocloud.io/library/node:20-slim
  docker tag docker.m.daocloud.io/library/node:20-slim node:20-slim
  docker pull docker.m.daocloud.io/library/rust:1-slim-bookworm
  docker tag docker.m.daocloud.io/library/rust:1-slim-bookworm rust:1-slim-bookworm
  docker pull docker.m.daocloud.io/library/debian:bookworm-slim
  docker tag docker.m.daocloud.io/library/debian:bookworm-slim debian:bookworm-slim
  python scripts\deploy_remote.py --sync-code
  pause
  exit /b %ERRORLEVEL%
)

echo [ERROR] Invalid choice
pause
exit /b 1
