@echo off
setlocal
title stepler-terminal-app qwen
set "STEPLER_QWEN_STATE=%LOCALAPPDATA%\Stepler\state"
set "STEPLER_QWEN_LOGS=%LOCALAPPDATA%\Stepler\logs"
set "STEPLER_QWEN_MARKER=%STEPLER_QWEN_STATE%\terminal-app-qwen.marker"
set "STEPLER_QWEN_INPUT=%STEPLER_QWEN_STATE%\qwen-input-%RANDOM%%RANDOM%.jsonl"
set "STEPLER_QWEN_EVENTS=%STEPLER_QWEN_LOGS%\qwen-events-%RANDOM%%RANDOM%.jsonl"
if not exist "%STEPLER_QWEN_STATE%" mkdir "%STEPLER_QWEN_STATE%" >nul 2>nul
if not exist "%STEPLER_QWEN_LOGS%" mkdir "%STEPLER_QWEN_LOGS%" >nul 2>nul
type nul > "%STEPLER_QWEN_INPUT%"
type nul > "%STEPLER_QWEN_EVENTS%"
>"%STEPLER_QWEN_MARKER%" echo pid=%PROCESSID%
>>"%STEPLER_QWEN_MARKER%" echo input_file=%STEPLER_QWEN_INPUT%
>>"%STEPLER_QWEN_MARKER%" echo json_file=%STEPLER_QWEN_EVENTS%

where qwen.cmd >nul 2>nul
if %ERRORLEVEL% EQU 0 (
    qwen.cmd --json-file "%STEPLER_QWEN_EVENTS%" --input-file "%STEPLER_QWEN_INPUT%" %*
    set "STEPLER_QWEN_EXIT=%ERRORLEVEL%"
    del "%STEPLER_QWEN_MARKER%" >nul 2>nul
    del "%STEPLER_QWEN_INPUT%" >nul 2>nul
    exit /b %STEPLER_QWEN_EXIT%
)

where qwen.exe >nul 2>nul
if %ERRORLEVEL% EQU 0 (
    qwen.exe --json-file "%STEPLER_QWEN_EVENTS%" --input-file "%STEPLER_QWEN_INPUT%" %*
    set "STEPLER_QWEN_EXIT=%ERRORLEVEL%"
    del "%STEPLER_QWEN_MARKER%" >nul 2>nul
    del "%STEPLER_QWEN_INPUT%" >nul 2>nul
    exit /b %STEPLER_QWEN_EXIT%
)

where qwen.ps1 >nul 2>nul
if %ERRORLEVEL% EQU 0 (
    powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0Stepler.Qwen.ps1" %*
    set "STEPLER_QWEN_EXIT=%ERRORLEVEL%"
    del "%STEPLER_QWEN_MARKER%" >nul 2>nul
    exit /b %STEPLER_QWEN_EXIT%
)

del "%STEPLER_QWEN_MARKER%" >nul 2>nul
del "%STEPLER_QWEN_INPUT%" >nul 2>nul
echo qwen was not found in PATH. Install Qwen CLI first or add it to PATH.
exit /b 1
