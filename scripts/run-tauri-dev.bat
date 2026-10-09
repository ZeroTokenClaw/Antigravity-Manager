@echo off
setlocal EnableExtensions EnableDelayedExpansion
cd /d "%~dp0.."
if exist "%USERPROFILE%\.cargo\bin\cargo.exe" set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" >nul
set "PATH=%CD%\tools\nasm\nasm-2.16.03;%PATH%"
set "VS_CMAKE=%ProgramFiles(x86)%\Microsoft Visual Studio\2022\BuildTools\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin"
if exist "%VS_CMAKE%\cmake.exe" set "PATH=%VS_CMAKE%;%PATH%"
if exist "C:\Program Files\LLVM\bin\libclang.dll" (
  set "LIBCLANG_PATH=C:\Program Files\LLVM\bin"
  set "PATH=C:\Program Files\LLVM\bin;%PATH%"
)
echo nasm: & where nasm
echo cmake: & where cmake
echo libclang: %LIBCLANG_PATH%
call pnpm tauri dev
