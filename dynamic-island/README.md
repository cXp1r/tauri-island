# Dynamic Island for Windows

仿 macOS 灵动岛的 Windows 桌面小组件，基于 Tauri 2 + Rust + TypeScript 构建。

![Windows](https://img.shields.io/badge/platform-Windows-blue)
![Tauri](https://img.shields.io/badge/Tauri-2.0-orange)
![Rust](https://img.shields.io/badge/Rust-1.70+-red)
![Version](https://img.shields.io/badge/version-0.1.3-green)

## ✨ 功能一览

### 🕐 时间与天气
- 屏幕顶部居中的胶囊状悬浮窗，实时显示时间
- 天气信息展示（Open-Meteo API），支持自动定位与手动设置城市
- 位置获取优先使用 Windows 系统定位，备选 IP 定位

### 🎵 音乐控制与歌词
- 通过 Windows SMTC（System Media Transport Controls）自动识别正在播放的音乐
- 实时同步滚动歌词显示，支持多行歌词上下文预览
- 支持播放/暂停、上一曲/下一曲、进度拖拽（Seek）
- 系统音量控制（音量增减、精确设置）
- 专辑封面展示（Base64 编码）
- 网易云音乐窗口标题兜底方案（SMTC 缺少元数据时自动回退）
- 智能会话选择：多个媒体播放器时优先选择正在播放的首选音乐应用

### 🎶 歌词来源
- [LRCLIB.net](https://lrclib.net) — 主要歌词源，多种搜索策略
- [网易云音乐](https://music.163.com) — 备用歌词源
- 异步获取歌词，代数计数器防止竞态条件

### 🤖 AI Agent 模式
- 内置 AI 对话助手，支持 OpenAI 兼容 API
- SSE 流式对话，逐字实时显示回复
- Markdown 渲染（代码块、行内代码、加粗、列表等）
- KaTeX 数学公式渲染
- 代码高亮（highlight.js）
- 思考模型支持（DeepSeek R1、OpenAI o 系列、QwQ 等）
  - 自动检测是否为思考模型（名称启发式 + API 响应检测）
  - 可折叠的思考过程展示，显示思考耗时
- 七彩流光边框状态指示（idle / thinking / generating / error）
- 对话历史管理（最近 20 轮 / 40 条消息）
- 三种窗口大小档位可选
- API Key 安全处理（前端仅显示掩码，所有请求在 Rust 后端执行）

### 🌐 系统监控与通知
- 网络状态监控 — WiFi 连接/断开自动提示（抗抖动，连续 3 次判定）
- 蓝牙设备监控 — 设备连接/断开自动提示
- 隐私指示器 — 麦克风/摄像头使用状态实时监控

### 🔗 剪贴板与链接
- 剪贴板 URL 自动检测，支持多链接列表
- 全局快捷键快速打开链接（默认 Alt+O，可自定义）
- 自定义链接处理器（按域名匹配不同应用打开）
- URL 域名白名单（可选）

### 🖱️ 交互体验
- 鼠标悬浮自动展开，移开自动收缩（带动画）
- 窗口拖拽支持
- 收起态绿色小横条指示器（颜色可自定义）
- 最小化模式
- 穿透点击（非胶囊区域不拦截鼠标事件）
- 右键上下文菜单

### 📌 系统托盘
- 托盘图标常驻
- 右键菜单：设置 / 退出

### ⚙️ 设置页面
- 独立设置窗口（1000×600，可调整大小）
- 可配置项：
  - 剪贴板链接监控开/关
  - 全局快捷键自定义
  - 歌词显示模式（歌词 / 仅歌曲信息 / 关闭）
  - 收起态指示器颜色
  - AI Agent 配置（API 地址、API Key、模型名称、模型类型检测）
  - AI 窗口大小档位
  - 天气城市设置（支持城市搜索，Open-Meteo Geocoding API）
  - 自定义链接处理器管理
- 设置持久化至 `%APPDATA%/dynamic-island/settings.json`

## 🛠️ 技术栈

| 层级 | 技术 |
|------|------|
| 框架 | Tauri 2（tray-icon, image-png, global-shortcut） |
| 后端 | Rust + Windows API (Win32, WinRT, COM) |
| 前端 | Vanilla TypeScript + Vite |
| 媒体 | Windows SMTC + IAudioEndpointVolume (COM) |
| 天气 | Open-Meteo API（免费，无需 API Key） |
| 歌词 | LRCLIB.net + 网易云音乐 API |
| AI | OpenAI 兼容 Chat Completions API (SSE) |
| 渲染 | marked + highlight.js + KaTeX |
| HTTP | reqwest（blocking，连接池复用） |

## 📁 项目结构

```
dynamic-island/
├── src/                          # 前端代码
│   ├── main.ts                   # 主界面交互逻辑（~57KB）
│   ├── settings.ts               # 设置页面逻辑
│   ├── styles.css                # 主界面样式
│   ├── styles-agent.css          # AI Agent 模式样式
│   └── assets/                   # 静态资源
├── src-tauri/
│   └── src/
│       ├── lib.rs                # 核心逻辑：状态管理、线程监控、通知、天气
│       ├── main.rs               # 程序入口
│       ├── ai.rs                 # AI Agent：流式对话、模型检测、历史管理
│       ├── media.rs              # 媒体控制：SMTC 集成、音量、封面
│       ├── lyrics.rs             # 歌词获取：LRCLIB + 网易云
│       ├── settings.rs           # 设置持久化与城市搜索
│       ├── window.rs             # 窗口管理：拖拽、动画、穿透点击
│       ├── clipboard.rs          # 剪贴板监控与 URL 提取
│       ├── privacy.rs            # 麦克风/摄像头隐私指示器
│       ├── link_handler.rs       # 自定义链接处理器
│       └── betterncm.rs          # BetterNCM 支持（网易云插件集成）
├── index.html                    # 主界面
├── settings.html                 # 设置页面
├── vite.config.ts                # Vite 配置（多页面入口）
├── package.json                  # 前端依赖
├── scripts/
│   └── gen-icons.mjs             # 图标生成脚本
└── docs/                         # 开发文档
    ├── DEVELOPMENT_PLAN.md       # 开发计划
    ├── AI_AGENT_IMPLEMENTATION_SUMMARY.md  # AI Agent 实现总结
    └── requirements-ai-agent-mode.md       # AI Agent 需求文档
```

## 🚀 构建与运行

### 前置要求
- [Node.js](https://nodejs.org/) (LTS)
- [Rust](https://www.rust-lang.org/tools/install) (1.70+)
- [Tauri 2 CLI](https://v2.tauri.app/start/prerequisites/)
- Windows 10/11

### 开发模式

```bash
cd dynamic-island
npm install
npx tauri dev
```

### 打包发布

```bash
npx tauri build
```

构建产物：
- MSI: `src-tauri/target/release/bundle/msi/DynamicIsland_0.1.3_x64_en-US.msi`
- NSIS: `src-tauri/target/release/bundle/nsis/DynamicIsland_0.1.3_x64-setup.exe`

## ⚙️ 配置说明

### AI Agent 配置
1. 打开设置页面（系统托盘右键 → 设置）
2. 在 AI Agent 配置区填写：
   - **API 地址** — OpenAI 兼容 API 的 Base URL
   - **API Key** — API 密钥
   - **模型名称** — 如 `gpt-4o`、`deepseek-chat`、`qwq-32b` 等
3. 点击「检测模型类型」自动识别是否为思考模型
4. 保存后即可在主界面切换到 Agent 视图使用

### 天气配置
- 默认使用自动定位（Windows 系统定位 → IP 定位）
- 可在设置页面手动搜索并选择城市

## 🔧 后台线程

应用启动后会创建以下后台监控线程：

| 线程 | 间隔 | 说明 |
|------|------|------|
| 鼠标监控 | 16ms | 检测鼠标位置，控制展开/收缩与穿透点击 |
| 媒体/歌词 | 200ms | 轮询 SMTC 播放状态，同步歌词 |
| 硬件监控 | 8s | 检测网络和蓝牙状态变化 |
| 隐私监控 | 1.2s | 检测麦克风/摄像头使用状态 |
| 剪贴板监控 | 1.2s | 检测剪贴板是否包含 URL |

## 📄 License

MIT
