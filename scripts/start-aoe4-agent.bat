@echo off
setlocal
rem Start aoe4-agent without Steam launch options.
rem Copy this script, aoe4-agent.exe, and config.yaml into the same folder
rem (for example C:\AoE4Agent) and double-click it, or pass "game" to also
rem ask Steam to launch Age of Empires IV (app 1466860).
rem
rem Do not use this script and the Steam launch option at the same time.
rem That starts two agents. Leave Launch Options empty when you use this file.
rem
rem The agent keeps running after the game exits. Close the "AoE4 Agent"
rem window (or press Ctrl+C in it) before you start another one.

cd /d "%~dp0"
if not exist "%~dp0aoe4-agent.exe" (
  echo aoe4-agent.exe is not next to this script.
  echo Copy the exe and config.yaml into this folder, then try again.
  exit /b 1
)
if not exist "%~dp0config.yaml" (
  echo config.yaml is not next to this script.
  echo Copy config.example.yaml to config.yaml in this folder, then edit it.
  exit /b 1
)

rem /D must not end in a backslash, or cmd treats \" as an escaped quote.
set "AGENT_DIR=%~dp0"
if "%AGENT_DIR:~-1%"=="\" set "AGENT_DIR=%AGENT_DIR:~0,-1%"

start "AoE4 Agent" /D "%AGENT_DIR%" "%AGENT_DIR%\aoe4-agent.exe" --config "%AGENT_DIR%\config.yaml" taunt
if /I "%~1"=="game" (
  start "" steam://rungameid/1466860
)
endlocal
