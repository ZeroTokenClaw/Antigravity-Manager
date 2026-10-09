@echo off
chcp 65001 >nul
setlocal EnableExtensions EnableDelayedExpansion

cd /d "%~dp0"

if exist "%USERPROFILE%\.cargo\bin\cargo.exe" set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"

echo ======================================================
echo       Antigravity Manager - One-Click Build ^& Package
echo ======================================================
echo.
echo Build options:
echo   [1] Full desktop package ^(Tauri installer + portable exe, recommended^)
echo   [2] Frontend production only ^(Vite build -^> dist/^)
echo   [3] Rust backend release only ^(src-tauri/target/release/^)
echo   [4] Exit
echo.
set /p "CHOICE=Enter choice [1-4, default 1]: "
if "!CHOICE!"=="" set "CHOICE=1"

if "!CHOICE!"=="4" exit /b 0
if "!CHOICE!"=="2" goto build_frontend_only
if "!CHOICE!"=="3" goto build_rust_only
if "!CHOICE!"=="1" goto build_full_package

echo [ERROR] Invalid choice: !CHOICE!
pause
exit /b 1

:build_frontend_only
echo.
echo [INFO] Building frontend production bundle: pnpm run build
call :ensure_node_and_pnpm
if errorlevel 1 goto fail_pause
call pnpm run build
if errorlevel 1 (
    echo.
    echo [ERROR] Frontend build failed.
    pause
    exit /b 1
)
echo.
echo [SUCCESS] Frontend built successfully! Output in: %~dp0dist\
pause
exit /b 0

:build_rust_only
echo.
echo [INFO] Building Rust release binary: cargo build --release
call :prepare_environment
if errorlevel 1 goto fail_pause
pushd src-tauri
cargo build --release
set "BUILD_CODE=!ERRORLEVEL!"
popd
if not "!BUILD_CODE!"=="0" (
    echo.
    echo [ERROR] Rust compilation failed with code: !BUILD_CODE!
    pause
    exit /b !BUILD_CODE!
)
echo.
echo [SUCCESS] Rust binary built successfully! Output in: %~dp0src-tauri\target\release\
pause
exit /b 0

:build_full_package
echo.
echo ======================================================
echo            [1/3] Checking Build Environment
echo ======================================================
call :prepare_environment
if errorlevel 1 goto fail_pause

echo.
echo ======================================================
echo            [2/3] Running Tauri Full Packaging
echo ======================================================
REM Local builds skip updater signature artifacts (need TAURI_SIGNING_PRIVATE_KEY).
REM Installer / MSI still build normally via tauri.local-build.conf.json overlay.
echo [INFO] Command: pnpm tauri build --config src-tauri/tauri.local-build.conf.json
echo [INFO] Updater signing disabled for local package ^(no private key required^)
call pnpm tauri build --config src-tauri/tauri.local-build.conf.json
set "BUILD_CODE=!ERRORLEVEL!"

set "BUNDLE_NSIS=%~dp0src-tauri\target\release\bundle\nsis"
set "BUNDLE_MSI=%~dp0src-tauri\target\release\bundle\msi"
set "HAS_BUNDLE=0"
dir /b "!BUNDLE_NSIS!\*-setup.exe" >nul 2>&1 && set "HAS_BUNDLE=1"
dir /b "!BUNDLE_MSI!\*.msi" >nul 2>&1 && set "HAS_BUNDLE=1"

if not "!BUILD_CODE!"=="0" if "!HAS_BUNDLE!"=="1" (
    echo.
    echo [WARN] Tauri exited with code !BUILD_CODE!, but installer bundles were found.
    echo [WARN] Continuing to collect artifacts. For updater .sig files set TAURI_SIGNING_PRIVATE_KEY.
)

if not "!BUILD_CODE!"=="0" if "!HAS_BUNDLE!"=="0" (
    echo.
    echo [ERROR] Tauri packaging failed with exit code: !BUILD_CODE!
    pause
    exit /b !BUILD_CODE!
)

echo.
echo ======================================================
echo            [3/3] Collecting Artifacts to release-output
echo ======================================================
call :collect_artifacts
echo.
echo ======================================================
echo                Packaging Completed!
echo ======================================================
echo [Output Folder] %~dp0release-output
echo.
echo Generated Artifacts:
dir /b "%~dp0release-output" 2>nul
echo.
echo Opening output directory in Explorer...
start "" explorer.exe "%~dp0release-output"
echo.
pause
exit /b 0

:collect_artifacts
set "OUTPUT_DIR=%~dp0release-output"
if not exist "!OUTPUT_DIR!" mkdir "!OUTPUT_DIR!"

set "BUNDLE_NSIS=%~dp0src-tauri\target\release\bundle\nsis"
set "BUNDLE_MSI=%~dp0src-tauri\target\release\bundle\msi"
set "RELEASE_BIN=%~dp0src-tauri\target\release"

if exist "!BUNDLE_NSIS!" (
    echo [INFO] Copying NSIS setup installer...
    xcopy /Y /Q "!BUNDLE_NSIS!\*.*" "!OUTPUT_DIR!\" >nul 2>&1
)

if exist "!BUNDLE_MSI!" (
    echo [INFO] Copying MSI installer...
    xcopy /Y /Q "!BUNDLE_MSI!\*.*" "!OUTPUT_DIR!\" >nul 2>&1
)

for %%f in ("!RELEASE_BIN!\*.exe") do (
    if /i not "%%~nxf"=="build.exe" copy /Y "%%f" "!OUTPUT_DIR!\%%~nxf" >nul 2>&1
)
exit /b 0

:fail_pause
pause
exit /b 1

REM ======================================================
REM Helper: Environment Setup
REM Never expand !PATH! inside parentheses — PATH often contains (x86).
REM ======================================================
:prepare_environment
call :ensure_node_and_pnpm
if errorlevel 1 exit /b 1

where cargo >nul 2>&1
if errorlevel 1 (
    echo [ERROR] Rust / Cargo toolchain missing. Install: https://rustup.rs/
    exit /b 1
)

call :load_msvc
where link >nul 2>&1
if errorlevel 1 (
    echo [ERROR] MSVC linker ^(link.exe^) missing.
    echo Please install Visual Studio C++ Build Tools:
    echo   https://aka.ms/vs/17/release/vs_BuildTools.exe
    exit /b 1
)

call :ensure_nasm
where nasm >nul 2>&1
if errorlevel 1 (
    echo [ERROR] NASM assembler missing ^(required for boring-sys2^).
    exit /b 1
)

REM VS-bundled CMake — assign PATH outside any paren block
set "PF86=%ProgramFiles(x86)%"
set "VS_CMAKE=!PF86!\Microsoft Visual Studio\2022\BuildTools\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin"
if exist "!VS_CMAKE!\cmake.exe" set "PATH=!VS_CMAKE!;!PATH!"

REM LLVM / LibClang — goto avoids PATH+(x86) inside parentheses
if exist "C:\Program Files\LLVM\bin\libclang.dll" goto add_llvm
goto after_llvm
:add_llvm
set "LIBCLANG_PATH=C:\Program Files\LLVM\bin"
set "PATH=C:\Program Files\LLVM\bin;!PATH!"
:after_llvm

REM Ensure WebView2Loader.dll ^(skip if already present^)
if exist "%~dp0src-tauri\resources\WebView2Loader.dll" goto env_ok
call :copy_webview2_loader
:env_ok
echo [INFO] Build environment ready.
exit /b 0

:copy_webview2_loader
set "WV2_SRC="
for /f "delims=" %%p in ('dir /s /b "%USERPROFILE%\.cargo\registry\src\*webview2*\x64\WebView2Loader.dll" 2^>nul') do (
    if not defined WV2_SRC set "WV2_SRC=%%p"
)
if not defined WV2_SRC exit /b 0
if not exist "!WV2_SRC!" exit /b 0
echo [INFO] Copying WebView2Loader.dll from cargo cache
copy /Y "!WV2_SRC!" "%~dp0src-tauri\resources\WebView2Loader.dll" >nul
exit /b 0

:ensure_node_and_pnpm
where node >nul 2>&1
if errorlevel 1 (
    echo [ERROR] Node.js not found. Install from https://nodejs.org/
    exit /b 1
)
where pnpm >nul 2>&1
if errorlevel 1 goto install_pnpm
goto after_pnpm_check
:install_pnpm
echo [INFO] Installing pnpm...
call npm install -g pnpm
if errorlevel 1 (
    echo [ERROR] Failed to install pnpm.
    exit /b 1
)
:after_pnpm_check
if exist "%~dp0node_modules\" goto node_deps_ok
echo [INFO] Installing node dependencies ^(pnpm install^)...
call pnpm install
if errorlevel 1 (
    echo [ERROR] pnpm install failed.
    exit /b 1
)
:node_deps_ok
exit /b 0

:load_msvc
where link >nul 2>&1
if not errorlevel 1 exit /b 0

set "PF86=%ProgramFiles(x86)%"
set "VSWHERE=!PF86!\Microsoft Visual Studio\Installer\vswhere.exe"
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

if exist "%~dp0tools\nasm\nasm-2.16.03\nasm.exe" goto add_local_nasm
echo [INFO] Installing portable NASM...
call node scripts\install-nasm.mjs
if exist "%~dp0tools\nasm\nasm-2.16.03\nasm.exe" goto add_local_nasm
exit /b 0

:add_local_nasm
set "PATH=%~dp0tools\nasm\nasm-2.16.03;!PATH!"
exit /b 0
