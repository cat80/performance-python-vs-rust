@echo off
echo 🚀 Starting Python Binance Basis Monitor...

REM Change to project directory
cd /d "%~dp0\..\py-basis"

REM Check if uv is installed
where uv >nul 2>nul
if %errorlevel% neq 0 (
    echo ❌ uv is not installed. Installing...
    powershell -c "irm https://astral.sh/uv/install.ps1 | iex"
)

REM Create virtual environment if it doesn't exist
if not exist ".venv" (
    echo 📦 Creating virtual environment...
    uv venv
)

REM Activate virtual environment
if exist ".venv\Scripts\activate.bat" (
    call .venv\Scripts\activate.bat
) else (
    echo ❌ Could not find virtual environment activation script
    exit /b 1
)

REM Install dependencies
echo 📦 Installing dependencies...
uv pip install -e .

REM Create logs directory
if not exist "logs" mkdir logs

REM Run the application
echo 🚀 Starting application...
echo 📋 Configuration:
for /f "tokens=2 delims==" %%i in ('findstr "SYMBOLS" ..\.env') do echo    Symbols: %%i
for /f "tokens=2 delims==" %%i in ('findstr "WINDOW_INTERVAL" ..\.env') do echo    Window Interval: %%is
for /f "tokens=2 delims==" %%i in ('findstr "EMA_WINDOW" ..\.env') do echo    EMA Window: %%i

python main.py %*