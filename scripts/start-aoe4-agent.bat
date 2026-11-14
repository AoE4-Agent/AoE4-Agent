@echo off
setlocal
rem Start aoe4-agent without Steam launch options.
rem Put this script, aoe4-agent.exe, and config.yaml in the same folder:
rem the AoE4 install directory's .\AoE4Agent
rem (steamapps\common\Age of Empires IV\AoE4Agent).
rem
rem This file does not use the relative path .\AoE4Agent. A double-click sets
rem the working directory to this folder (or, for some shortcuts, to
rem System32). .\AoE4Agent from inside the folder would look for a nested
rem AoE4Agent\AoE4Agent. %~dp0 is the folder that contains this script, which
rem does not depend on Steam or on the caller's current directory.
rem
rem Do not use this script and the Steam launch option at the same time.
rem That starts two agents. Leave Launch Options empty when you use this file.
rem
rem The agent keeps running after the game exits. Close the "AoE4 Agent"
rem window (or press Ctrl+C in it) before you start another one.
rem
rem Pass "game" to also ask Steam to launch Age of Empires IV (app 1466860).

cd /d "%~dp0"
if not exist "%~dp0aoe4-agent.exe" (
  echo aoe4-agent.exe is not next to this script.
  echo Copy the exe and config.yaml into .\AoE4Agent under the AoE4 install, then try again.
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

start "AoE4 Agent" /D "%AGENT_DIR%" "%AGENT_DIR%\aoe4-agent.exe" taunt
if /I "%~1"=="game" (
  start "" steam://rungameid/1466860
)
endlocal
