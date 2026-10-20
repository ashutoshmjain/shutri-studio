@echo off
cd /d "%~dp0"
set "PATH=%LOCALAPPDATA%\ShutriStudio\bin;%PATH%"
set "SHERPA_ONNX_BIN=%LOCALAPPDATA%\ShutriStudio\bin\sherpa-onnx-offline.exe"
echo Starting Shutri Studio (Research to Reels)...
npm run tauri
pause
