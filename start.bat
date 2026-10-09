@echo off
chcp 65001 >nul
setlocal EnableExtensions EnableDelayedExpansion

cd /d "%~dp0"

if exist "%USERPROFILE%\.cargo\bin\cargo.exe" set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"

echo ======================================================
echo          Antigravity Manager - One-Click Start
echo ======================================================
echo.
echo This project is a Tauri desktop app.
echo   [1] Frontend only   - browser at http://localhost:1420
echo                        no MSVC / Rust required
echo   [2] Full desktop    - pnpm tauri dev
echo                        needs Rust + MSVC C++ Build Tools
echo   [3] Exit
echo.
set /p "CHOICE=Enter 1/2/3 [default 1]: "
if "!CHOICE!"=="" set "CHOICE=1"
if "!CHOICE!"=="3" exit /b 0

where node >nul 2>&1
if errorlevel 1 (
    echo [ERROR] Node.js not found. Install from https://nodejs.org/
    pause
    exit /b 1
)

where pnpm >nul 2>&1
if errorlevel 1 (
    echo [INFO] Installing pnpm...
    call npm install -g pnpm
    if errorlevel 1 (
        echo [ERROR] Failed to install pnpm.
        pause
        exit /b 1
    )
)

if not exist "node_modules\" (
    echo [INFO] Installing dependencies...
    call pnpm install
    if errorlevel 1 (
        echo [ERROR] pnpm install failed.
        pause
        exit /b 1
    )
)

node scripts\free-port.mjs 1420

if "!CHOICE!"=="2" goto tauri_start
goto web_start

:web_start
echo.
echo [INFO] Starting frontend: pnpm dev
echo Open http://localhost:1420 in your browser
echo.
call pnpm dev
exit /b 0

:tauri_start
where cargo >nul 2>&1
if errorlevel 1 (
    echo [ERROR] Rust/Cargo missing. Install: https://rustup.rs/
    pause
    exit /b 1
)

REM MSVC tools are installed but not on default PATH; load vcvars64
call :load_msvc
where link >nul 2>&1
if errorlevel 1 (
    echo [ERROR] MSVC linker still missing after loading VS env.
    echo Install "Desktop development with C++" from:
    echo   https://aka.ms/vs/17/release/vs_BuildTools.exe
    echo Or choose [1] Frontend only.
    if exist "%~dp0vs_BuildTools.exe" start "" "%~dp0vs_BuildTools.exe"
    pause
    exit /b 1
)

REM boring-sys2 / BoringSSL needs NASM on PATH
call :ensure_nasm
where nasm >nul 2>&1
if errorlevel 1 (
    echo [ERROR] NASM assembler missing. Required to compile boring-sys2.
    echo Run: node scripts\install-nasm.mjs
    pause
    exit /b 1
)

REM Prefer VS-bundled CMake if present
set "VS_CMAKE=%ProgramFiles(x86)%\Microsoft Visual Studio\2022\BuildTools\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin"
if exist "!VS_CMAKE!\cmake.exe" set "PATH=!VS_CMAKE!;!PATH!"

REM bindgen needs libclang
if exist "C:\Program Files\LLVM\bin\libclang.dll" (
    set "LIBCLANG_PATH=C:\Program Files\LLVM\bin"
    set "PATH=C:\Program Files\LLVM\bin;!PATH!"
)

echo.
echo [INFO] Starting Tauri desktop: pnpm tauri dev
echo.
call pnpm tauri dev
if errorlevel 1 (
    echo.
    echo [ERROR] Start failed, exit code: !ERRORLEVEL!
    pause
    exit /b !ERRORLEVEL!
)
exit /b 0

:load_msvc
where link >nul 2>&1
if not errorlevel 1 (
    echo [INFO] MSVC linker already on PATH
    exit /b 0
)

set "VSWHERE=C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe"
if not exist "!VSWHERE!" (
    echo [WARN] vswhere.exe not found
    exit /b 1
)

set "VSINSTALL="
for /f "usebackq delims=" %%i in (`"!VSWHERE!" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`) do set "VSINSTALL=%%i"

if not defined VSINSTALL (
    echo [WARN] Visual Studio Build Tools with C++ not detected by vswhere
    exit /b 1
)

set "VCVARS=!VSINSTALL!\VC\Auxiliary\Build\vcvars64.bat"
if not exist "!VCVARS!" (
    echo [WARN] vcvars64.bat not found: !VCVARS!
    exit /b 1
)

echo [INFO] Loading MSVC env: !VCVARS!
call "!VCVARS!" >nul
exit /b 0

:ensure_nasm
where nasm >nul 2>&1
if not errorlevel 1 exit /b 0
if exist "%~dp0tools\nasm\nasm-2.16.03\nasm.exe" (
    set "PATH=%~dp0tools\nasm\nasm-2.16.03;!PATH!"
    exit /b 0
)
echo [INFO] Installing portable NASM...
call node scripts\install-nasm.mjs
if exist "%~dp0tools\nasm\nasm-2.16.03\nasm.exe" set "PATH=%~dp0tools\nasm\nasm-2.16.03;!PATH!"
exit /b 0
