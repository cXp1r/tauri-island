# Dynamic Island for Windows

仿 macOS 灵动岛的 Windows 桌面小组件，基于 Tauri 2 + Rust + TypeScript 构建。

![Windows](https://img.shields.io/badge/platform-Windows_10%2F11-blue)
![Tauri](https://img.shields.io/badge/Tauri-2.0-orange)
![Rust](https://img.shields.io/badge/Rust-1.70+-red)
![Version](https://img.shields.io/badge/version-0.8.6-green)

## 功能

- 🕐 **时间与天气** — 屏幕顶部居中的胶囊状悬浮窗，实时显示时间与天气信息（Open-Meteo API）
- 🎵 **音乐歌词** — 通过 SMTC 自动识别正在播放的音乐，同步显示滚动歌词
- 🎛️ **媒体控制** — 播放/暂停、上一曲/下一曲、进度拖拽、系统音量控制
- 🤖 **AI Agent** — 内置 AI 对话助手，支持 OpenAI 兼容 API 流式对话、Markdown/KaTeX/代码高亮渲染、思考模型
- 📱 **SADB 手机投屏** — Android 屏幕镜像与触控控制，基于 ADB + scrcpy 协议，支持音频转发
- 🔍 **全局搜索** — 集成 Everything SDK 文件搜索与快捷应用搜索
- 📧 **邮件监控** — IMAP 邮件轮询，新邮件通知提醒
- 🌐 **系统监控** — 麦克风/摄像头隐私指示器
- 🔗 **剪贴板监控** — 复制链接时快捷跳转，支持 TCP 局域网剪贴板同步（CC）
- 🖱️ **鼠标悬浮展开** — 靠近顶部自动展开，移开自动收缩
- ⌨️ **全局快捷键** — Alt+O 快速打开剪贴板链接（可自定义）
- 📌 **系统托盘** — 右键托盘图标可打开设置或退出

## 歌词来源

- [Lyrix](https://github.com/cXp1r/Lyrix) — 集成多平台歌词（网易云、QQ 音乐、酷狗、汽水音乐）

## 技术栈

| 层级 | 技术 |
|------|------|
| 框架 | Tauri 2（tray-icon, image-png, global-shortcut） |
| 后端 | Rust + Windows API (Win32, WinRT, COM) |
| 前端 | Vanilla TypeScript + Vite |
| 媒体 | Windows SMTC + IAudioEndpointVolume (COM) |
| 歌词 | lyrix crate（集成多平台歌词） |
| 天气 | Open-Meteo API |
| AI | OpenAI 兼容 Chat Completions API (SSE) |
| 搜索 | Everything SDK |
| 投屏 | ADB + scrcpy 协议 |
| 邮件 | IMAP + native-tls |
| 渲染 | marked + highlight.js + KaTeX |

## 构建

### 前置要求

- [Node.js](https://nodejs.org/) (LTS)
- [Rust](https://www.rust-lang.org/tools/install) (1.70+)
- [Tauri 2 CLI](https://v2.tauri.app/start/prerequisites/)
- Windows 10/11

### 开发与打包

```bash
npm install

# 开发模式
npx tauri dev

# 打包安装包
npx tauri build
```

构建产物位于 `src-tauri/target/release/bundle/`：
- NSIS: `nsis/DynamicIsland_<version>_x64-setup.exe`
- MSI: `msi/DynamicIsland_<version>_x64_en-US.msi`

## 项目结构

```
dynamic-island/
├── src/                          # 前端 TypeScript 代码
│   ├── main.ts                   # 主入口
│   ├── dom.ts                    # DOM 元素引用
│   ├── state.ts                  # 前端状态管理
│   ├── types.ts                  # 类型定义
│   ├── utils.ts                  # 工具函数
│   ├── highlight-setup.ts        # highlight.js 配置
│   ├── settings.ts               # 设置页面逻辑
│   ├── settings-lyric-offset.ts  # 歌词偏移设置
│   ├── settings.css              # 设置页面样式
│   ├── assets/                   # 静态资源
│   │   ├── icons/                # 应用图标
│   │   └── logo-bw-morph.svg     # Logo
│   └── modules/                  # 功能模块
│       ├── agent.ts              # AI Agent
│       ├── music-controls.ts     # 媒体控制
│       ├── lyric-renderer.ts     # 歌词渲染
│       ├── weather.ts            # 天气
│       ├── privacy.ts            # 隐私指示器
│       ├── notice-url.ts         # 链接通知
│       ├── notice-queue.ts       # 通知队列
│       ├── view-switcher.ts      # 视图切换
│       ├── capsule-interaction.ts # 胶囊交互
│       ├── minimize-drag.ts      # 最小化与拖拽
│       ├── resize-observer.ts    # 尺寸监听
│       ├── search.ts             # 全局搜索
│       └── sadb.ts               # 手机投屏控制
├── public/themes/                # 主题样式
│   ├── base.css                  # 基础样式
│   ├── classic.css               # 经典主题
│   ├── liquid-glass.css          # 液态玻璃主题
│   ├── agent-base.css            # Agent 模式样式
│   └── glow-border.css           # 音乐光效边框
├── src-tauri/src/                # Rust 后端
│   ├── lib.rs                    # Tauri 命令注册
│   ├── main.rs                   # 程序入口
│   ├── ai.rs                     # AI Agent 流式对话
│   ├── media.rs                  # SMTC 媒体控制与音量
│   ├── lyrics.rs                 # 歌词获取
│   ├── settings.rs               # 设置持久化
│   ├── window.rs                 # 窗口管理与动画
│   ├── clipboard.rs              # 剪贴板监控
│   ├── cc.rs                     # TCP 局域网剪贴板同步
│   ├── ceeverything.rs           # Everything 文件搜索
│   ├── tools.rs                  # ADB 工具安装
│   ├── sadb.rs                   # Android 投屏（scrcpy）
│   ├── privacy.rs                # 隐私指示器
│   ├── email.rs                  # IMAP 邮件轮询
│   ├── link_handler.rs           # 自定义链接处理器
│   ├── logger.rs                 # 日志系统
│   ├── updater.rs                # 自动更新
│   └── betterncm.rs              # BetterNCM 支持
├── sadb-core/                    # ADB 核心库（Rust crate）
├── scripts/                      # 构建辅助脚本
│   └── gen-icons.mjs             # 图标生成
├── index.html                    # 主界面
├── settings.html                 # 设置页面
├── email.html                    # 邮件浏览页面
├── package.json
├── tsconfig.json
└── vite.config.ts
```

## 设置

通过系统托盘右键 → 设置，可配置：
- 剪贴板链接监控开关与 TCP 局域网同步（CC）
- 快捷键自定义
- 歌词显示模式（歌词 / 仅歌曲信息 / 关闭）与歌词偏移
- 收起态指示器颜色
- AI Agent（API 地址、API Key、模型名称、窗口大小）
- 天气城市设置
- 自定义链接处理器
- 邮件 IMAP 轮询（服务器、账号、密码、间隔）
- SADB 手机投屏与控制

设置持久化至 `%APPDATA%/dynamic-island/settings.json`

## 发布

推送 `tauri-v*` Tag 后 GitHub Actions 自动构建并创建 Release。
