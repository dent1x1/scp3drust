@echo off
chcp 65001 >nul
cd /d "%~dp0"
python scp_sl_game.py
if errorlevel 1 pause
