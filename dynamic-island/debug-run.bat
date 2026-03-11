@echo off
chcp 65001 >nul
echo [1/3] 正在关闭 DynamicIsland 进程...
taskkill /F /IM dynamic-island.exe >nul 2>&1
if %errorlevel%==0 (
    echo       已关闭旧进程
    timeout /t 1 /nobreak >nul
) else (
    echo       没有运行中的进程
)

echo [2/3] 正在构建 Debug 版本...
call npx tauri build --debug
if %errorlevel% neq 0 (
    echo       构建失败！
    pause
    exit /b 1
)

echo [3/3] 正在启动 DynamicIsland...
start "" "src-tauri\target\debug\dynamic-island.exe"
echo       启动完成！
