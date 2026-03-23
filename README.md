# Dynamic Island for Windows

仿 macOS 灵动岛的 Windows 桌面小组件，基于 Tauri 2 + Rust + TypeScript 构建。

![Windows](https://img.shields.io/badge/platform-Windows-blue)
![Tauri](https://img.shields.io/badge/Tauri-2.0-orange)
![Rust](https://img.shields.io/badge/Rust-1.70+-red)

## 功能

- 🕐 **时间与天气** — 屏幕顶部居中的胶囊状悬浮窗，实时显示时间与天气信息
- 🎵 **音乐歌词** — 自动识别正在播放的音乐（SMTC），同步显示滚动歌词
- 🎛️ **媒体控制** — 播放/暂停、上一曲/下一曲、进度拖拽、音量控制
- 🤖 **AI Agent** — 内置 AI 对话助手，支持 OpenAI 兼容 API 流式对话
- 🌐 **系统监控** — 网络/蓝牙状态变化通知，麦克风/摄像头隐私指示器
- 🔗 **剪贴板监控** — 复制链接时快捷跳转，支持自定义链接处理器
- 🖱️ **鼠标悬浮展开** — 靠近顶部自动展开，移开自动收缩
- ⌨️ **全局快捷键** — Alt+O 快速打开剪贴板链接（可自定义）
- 📌 **系统托盘** — 右键托盘图标可打开设置或退出

## 歌词来源

- [LRCLIB.net](https://lrclib.net) — 主要歌词源，4 种搜索策略
- [网易云音乐](https://music.163.com) — 备用歌词源

## 技术栈

| 层级 | 技术 |
|------|------|
| 框架 | Tauri 2 |
| 后端 | Rust + Windows API |
| 前端 | Vanilla TypeScript + Vite |
| 媒体 | Windows SMTC (System Media Transport Controls) |

## 构建

```bash
# 安装依赖
cd dynamic-island
npm install

# 开发模式
npx tauri dev

# 打包安装包
npx tauri build
```

构建产物：
- MSI: `src-tauri/target/release/bundle/msi/DynamicIsland_0.1.3_x64_en-US.msi`
- NSIS: `src-tauri/target/release/bundle/nsis/DynamicIsland_0.1.3_x64-setup.exe`

## 项目结构

```
dynamic-island/
├── src/                          # 前端代码
│   ├── main.ts                   # 主界面交互逻辑
│   ├── settings.ts               # 设置页面逻辑
│   ├── styles.css                # 主界面样式
│   └── styles-agent.css          # AI Agent 模式样式
├── src-tauri/
│   └── src/
│       ├── lib.rs                # 核心逻辑与状态管理
│       ├── ai.rs                 # AI Agent 流式对话
│       ├── media.rs              # SMTC 媒体控制与音量
│       ├── lyrics.rs             # 歌词获取
│       ├── settings.rs           # 设置持久化
│       ├── window.rs             # 窗口管理与动画
│       ├── clipboard.rs          # 剪贴板监控
│       ├── privacy.rs            # 隐私指示器
│       ├── link_handler.rs       # 自定义链接处理器
│       └── betterncm.rs          # BetterNCM 支持
├── index.html                    # 主界面
└── settings.html                 # 设置页面
```

## 设置

通过系统托盘右键 → 设置，可配置：
- 剪贴板链接监控开关
- 快捷键自定义
- 歌词显示模式（歌词 / 仅歌曲信息 / 关闭）
- 收起态指示器颜色
- AI Agent（API 地址、API Key、模型名称、窗口大小）
- 天气城市设置
- 自定义链接处理器

## 分支说明

本分支 (`tauri-island`) 是使用 Tauri 2 完全重写的版本，与其他 Python 分支无关。
