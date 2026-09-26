@echo off
chcp 65001 >nul
cd /d "%~dp0"
title 圣诞树 · 本地服务
where node >nul 2>nul || (echo 需要先安装 Node.js：https://nodejs.org/ & pause & exit /b 1)
node serve.mjs
pause
