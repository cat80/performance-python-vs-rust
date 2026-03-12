@echo off
echo 🚀 Starting Rust Binance Basis Monitor...

REM Change to project directory
cd /d "%~dp0\..\rust-basis"

REM Check if Rust is installed
where cargo >nul 2>nul
if %errorlevel% neq 0 (
    echo ❌ Rust is not installed. Installing...
    powershell -c "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y"
    call "%USERPROFILE%\.cargo\env.bat"
)

REM Create logs directory
if not exist "logs" mkdir logs

REM Build in release mode for performance
echo 🔨 Building in release mode...
cargo build --release

REM Run the application
echo 🚀 Starting application...
echo 📋 Configuration:
for /f "tokens=2 delims==" %%i in ('findstr "SYMBOLS" ..\.env') do echo    Symbols: %%i
for /f "tokens=2 delims==" %%i in ('findstr "WINDOW_INTERVAL" ..\.env') do echo    Window Interval: %%is
for /f "tokens=2 delims==" %%i in ('findstr "EMA_WINDOW" ..\.env') do echo    EMA Window: %%i

.\target\release\rust-basis.exe