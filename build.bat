@echo off
setlocal EnableDelayedExpansion

echo ========================================================
echo               Spotify CLI Build ^& Setup
echo ========================================================
echo.

:: 1. Run Cargo Build
echo [*] Building spotify-cli in release mode...
cargo build --release
if errorlevel 1 (
    echo.
    echo [ERROR] Build failed. Please make sure Rust and Cargo are installed.
    pause
    exit /b 1
)

set "BIN_DIR=%~dp0target\release"
if "%BIN_DIR:~-1%"=="\" set "BIN_DIR=%BIN_DIR:~0,-1%"

echo.
echo [OK] Build successful. Binary located at:
echo      %BIN_DIR%\spotify-cli.exe
echo.

:: 2. Create 'spotify.cmd' wrapper so both 'spotify-cli' and 'spotify' work
(
echo @echo off
echo "%%~dp0spotify-cli.exe" %%*
) > "%BIN_DIR%\spotify.cmd"

:: 3. Check if BIN_DIR is in User PATH
powershell -NoProfile -Command "if (([Environment]::GetEnvironmentVariable('Path', 'User') -split ';') -contains '%BIN_DIR%') { exit 0 } else { exit 1 }"
set "IN_USER_PATH=%errorlevel%"

if "%IN_USER_PATH%"=="0" (
    echo [OK] '%BIN_DIR%' is already in your User PATH.
    echo      Commands available: 'spotify-cli' and 'spotify'.
) else (
    echo [-] '%BIN_DIR%' is NOT in your User PATH.
    echo.
    set /p "ADD_CHOICE=Do you want to add this directory to your User PATH so 'spotify-cli' works anywhere? (Y/N): "
    if /i "!ADD_CHOICE!"=="Y" (
        powershell -NoProfile -Command "$dir = '%BIN_DIR%'; $curr = [Environment]::GetEnvironmentVariable('Path', 'User'); if (-not ($curr -split ';' -contains $dir)) { $newPath = if ($curr) { $curr.TrimEnd(';') + ';' + $dir } else { $dir }; [Environment]::SetEnvironmentVariable('Path', $newPath, 'User'); Write-Host '[OK] Added to User PATH successfully!' -ForegroundColor Green }"
        echo.
        echo [NOTE] Please restart your terminal for the updated PATH to take effect.
        echo        Then you can run 'spotify-cli' or 'spotify' anywhere.
    ) else (
        echo.
        echo [INFO] Skipped adding to PATH. You can run the binary directly:
        echo        %BIN_DIR%\spotify-cli.exe
    )
)

echo.
echo ========================================================
echo Setup finished.
echo ========================================================
