@echo off
chcp 65001 >nul
echo Clean-RS 系统清理工具
echo.
echo 正在启动 TUI 界面...
echo.
"target\release\clean-rs.exe" --tui --pause