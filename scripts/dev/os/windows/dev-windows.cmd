@echo off
setlocal EnableExtensions EnableDelayedExpansion

REM PromptLab / AISec — Windows Tauri dev launcher
REM Handles: MSVC env, libclang, short cargo target dir, npm.cmd (avoids PS execution policy)

cd /d "%~dp0..\..\..\.."
set "REPO_ROOT=%CD%"

echo [dev-windows] repo: %REPO_ROOT%

REM --- npm (use npm.cmd so PowerShell execution policy does not block npm.ps1) ---
where npm.cmd >nul 2>&1
if errorlevel 1 (
  echo [dev-windows] ERROR: npm.cmd not found. Install Node.js and reopen the terminal.
  exit /b 1
)

REM --- LLVM / libclang (required by llama-cpp-sys bindgen) ---
if not defined LIBCLANG_PATH (
  if exist "%ProgramFiles%\LLVM\bin\libclang.dll" (
    set "LIBCLANG_PATH=%ProgramFiles%\LLVM\bin"
  ) else if exist "%ProgramFiles(x86)%\LLVM\bin\libclang.dll" (
    set "LIBCLANG_PATH=%ProgramFiles(x86)%\LLVM\bin"
  )
)
if not defined LIBCLANG_PATH (
  echo [dev-windows] ERROR: libclang.dll not found.
  echo   Install LLVM: winget install LLVM.LLVM
  echo   Or set LIBCLANG_PATH to the folder containing libclang.dll
  exit /b 1
)
if not exist "%LIBCLANG_PATH%\libclang.dll" (
  echo [dev-windows] ERROR: LIBCLANG_PATH does not contain libclang.dll: %LIBCLANG_PATH%
  exit /b 1
)
echo [dev-windows] LIBCLANG_PATH=%LIBCLANG_PATH%

REM --- Prefer a short local cargo target dir (MSVC path length limits) ---
if not defined CARGO_TARGET_DIR (
  set "CARGO_TARGET_DIR=%REPO_ROOT%\target"
)
echo [dev-windows] CARGO_TARGET_DIR=%CARGO_TARGET_DIR%

REM --- Free Vite port if still held by a previous session ---
for /f "tokens=5" %%P in ('netstat -ano ^| findstr /R /C:":5173 .*LISTENING"') do (
  echo [dev-windows] port 5173 in use by PID %%P — stopping it
  taskkill /F /PID %%P >nul 2>&1
)

REM --- Load MSVC / Windows SDK via vcvars64.bat ---
set "VSWHERE=%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe"
if not exist "%VSWHERE%" (
  echo [dev-windows] ERROR: vswhere.exe not found. Install Visual Studio Build Tools with C++ workload.
  exit /b 1
)

for /f "usebackq delims=" %%I in (`"%VSWHERE%" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`) do set "VSINSTALL=%%I"
if not defined VSINSTALL (
  echo [dev-windows] ERROR: No VS install with VC Tools x64 found.
  echo   Install "Desktop development with C++" or Build Tools + MSVC.
  exit /b 1
)

set "VCVARS=!VSINSTALL!\VC\Auxiliary\Build\vcvars64.bat"
if not exist "!VCVARS!" (
  echo [dev-windows] ERROR: vcvars64.bat missing: !VCVARS!
  exit /b 1
)
echo [dev-windows] vcvars: !VCVARS!

call "!VCVARS!"
if errorlevel 1 (
  echo [dev-windows] ERROR: vcvars64.bat failed
  exit /b 1
)

echo [dev-windows] starting: npm.cmd run tauri dev
echo.

npm.cmd run tauri dev
set "EXIT_CODE=%ERRORLEVEL%"
exit /b %EXIT_CODE%
