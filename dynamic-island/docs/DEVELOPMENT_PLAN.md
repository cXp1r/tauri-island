# Development Plan for Dynamic Island — AI Agent Mode

## Project Purpose and Goals

为灵动岛桌面应用新增第三个核心模式——AI Agent 模式，使用户可以直接在灵动岛中与 AI 进行对话交互。V1 聚焦文本对话能力，支持 OpenAI 兼容 API，具备七彩流光边框视觉效果，并自动检测思考模型。

## Context and Background

### 现有架构
- 前端：TypeScript + Vite，单文件 `main.ts` 管理所有视图逻辑
- 后端：Rust + Tauri v2，`lib.rs` 包含所有状态管理和后台线程
- 视图系统：`ViewMode = "time" | "notice" | "urls" | "lyric"`，通过 `viewElements` 映射 DOM 元素，`setView()` 控制切换动画
- 设置系统：独立 `settings.html` + `settings.ts`，通过 Tauri commands 与后端交互
- 胶囊尺寸：收起 140-320px × 50px，展开 330px × 74px
- 已有 `reqwest` HTTP client（用于歌词获取），可复用用于 AI API 请求
- 已有 SSE 类似的流式处理模式（歌词监控线程中逐行解析）

### 关键设计决策
- AI API 请求在 Rust 后端执行（非前端 fetch），利用已有 `reqwest` 和线程模型
- 流式 token 通过 Tauri 事件推送到前端，与现有 `lyric-update` 模式一致
- 七彩流光使用 CSS `@property` + `conic-gradient` 实现，避免 Canvas 开销
- 展开态需要增大窗口尺寸以容纳对话 UI，需要动态调整 Tauri 窗口大小
- 思考模型检测在保存设置时自动触发，结果持久化

## Development Tasks

### Phase 1: 后端 AI 基础设施

- [x] 在 `IslandState` 中新增 AI 相关字段
  - [ ] `ai_api_url: Arc<Mutex<String>>`（API 地址）
  - [ ] `ai_api_key: Arc<Mutex<String>>`（API Key）
  - [ ] `ai_model: Arc<Mutex<String>>`（模型名称）
  - [ ] `is_reasoning_model: Arc<AtomicBool>`（是否为思考模型）
  - [ ] `ai_enabled: Arc<AtomicBool>`（AI 功能是否已配置可用）
  - [ ] `ai_generating: Arc<AtomicBool>`（是否正在生成中）
  - [ ] `ai_history: Arc<Mutex<Vec<ChatMessage>>>`（对话历史）
- [x] 定义 `ChatMessage` 结构体（role: system/user/assistant, content: String, reasoning_content: Option<String>）
- [x] 实现 AI 设置读写 Tauri commands
  - [ ] `ai_get_settings()` → 返回 AI 配置（不含完整 key，仅掩码）
  - [ ] `ai_save_settings(api_url, api_key, model)` → 保存配置到设置文件
- [x] 实现设置持久化（扩展已有的设置 JSON 文件，新增 `ai_api_url`、`ai_api_key`、`ai_model`、`is_reasoning_model` 字段）
- [x] 实现 `ai_detect_model_type()` command
  - [ ] 向配置的 API 发送简单测试请求（如 "Hi"）
  - [ ] 解析响应，检查是否包含 `reasoning_content` 或 `thinking` 字段
  - [ ] 更新 `is_reasoning_model` 并持久化
  - [ ] 返回检测结果给前端
- [x] Perform a self-review of your code, once you're certain it's 100% complete to the requirements in this phase mark the task as done.
- [x] STOP and wait for human review

### Phase 2: 后端流式对话实现

- [x] 实现 `ai_send_message(content: String)` Tauri command
  - [ ] 将用户消息追加到 `ai_history`
  - [ ] 构建 OpenAI 兼容的请求体（messages 数组、model、stream: true）
  - [ ] 在新线程中发起 streaming HTTP 请求
  - [ ] 逐行解析 SSE 响应（`data: {...}` 格式）
  - [ ] 提取 `choices[0].delta.content` 作为普通 token
  - [ ] 提取 `choices[0].delta.reasoning_content`（如有）作为思考 token
  - [ ] 通过 Tauri 事件发送到前端
    - [ ] `ai-token` → `{ text: String }`
    - [ ] `ai-thinking-token` → `{ text: String }`（仅思考模型）
    - [ ] `ai-status` → `{ status: "thinking" | "generating" | "completed" | "error" }`
  - [ ] 生成完成后，将完整 assistant 回复追加到 `ai_history`
  - [ ] 限制历史长度（最近 20 轮对话）
- [x] 实现 `ai_stop_generation()` command
  - [ ] 设置 `ai_generating` 为 false，中断流式读取循环
  - [ ] 已生成的部分内容仍追加到历史
- [x] 实现 `ai_clear_history()` command
  - [ ] 清空 `ai_history`
- [x] 处理错误场景
  - [ ] 网络错误 → 发送 `ai-error` 事件
  - [ ] API 返回错误（401/429/500）→ 解析错误信息并发送
  - [ ] 请求超时处理
- [x] Perform a self-review of your code, once you're certain it's 100% complete to the requirements in this phase mark the task as done.
- [x] STOP and wait for human review

### Phase 3: 前端 Agent 视图 — HTML 结构与 CSS 样式

- [x] 在 `index.html` 中新增 `#agent-area` DOM 结构
  - [ ] 状态指示区（模型名称 + 状态标签）
  - [ ] 消息显示区（可滚动容器）
  - [ ] 输入区（输入框 + 发送按钮 + 停止按钮）
- [x] 实现七彩流光边框 CSS
  - [ ] 使用 `@property --rainbow-angle` 注册自定义属性
  - [ ] `#island-capsule.agent-active::before` 使用 `conic-gradient` 绘制彩虹渐变
  - [ ] `@keyframes rainbow-rotate` 控制旋转动画
  - [ ] 使用 `mask` 或内层背景遮罩实现仅边框可见的效果
  - [ ] 不同状态的动画速度变体
    - [ ] `.agent-idle` → `animation-duration: 4s`（缓慢）
    - [ ] `.agent-thinking` → `animation-duration: 1.5s` + 脉冲效果
    - [ ] `.agent-generating` → `animation-duration: 0.8s` + 亮度增强
    - [ ] `.agent-error` → 红色闪烁动画
- [x] Agent 视图收起态样式
  - [ ] 宽度 320px，显示单行 AI 回复摘要
  - [ ] AI 图标/标识
- [x] Agent 视图展开态样式
  - [ ] 增大胶囊尺寸（约 330px × 200px 或更大）
  - [ ] 消息区域滚动样式
  - [ ] 输入框样式（底部固定）
  - [ ] 发送/停止按钮样式
  - [ ] 思考过程折叠区域样式
- [x] 简易 Markdown 渲染样式
  - [ ] 代码块（`code` 和 `pre`）背景色和字体
  - [ ] 加粗文本
  - [ ] 列表样式
- [x] Perform a self-review of your code, once you're certain it's 100% complete to the requirements in this phase mark the task as done.
- [x] STOP and wait for human review

### Phase 4: 前端 Agent 视图 — 交互逻辑

- [ ] 在 `main.ts` 中扩展 ViewMode 类型，新增 `"agent"`
- [ ] 注册 `#agent-area` 到 `viewElements`
- [ ] 修改 `getAvailableViews()` — 当 AI 已配置（`ai_enabled`）时包含 `"agent"`
- [ ] 实现 agent 视图的展开/收起逻辑
  - [x] 进入 agent 视图时动态调整窗口大小（调用 Tauri window resize）
  - [x] 离开 agent 视图时恢复原始窗口大小
- [ ] 实现消息发送逻辑
  - [x] 输入框 Enter 键监听
  - [x] 调用 `ai_send_message` Tauri command
  - [x] 发送后清空输入框，显示用户消息气泡
- [ ] 实现流式响应接收
  - [x] 监听 `ai-token` 事件，逐字追加到当前 assistant 消息
  - [x] 监听 `ai-thinking-token` 事件，追加到思考区域
  - [x] 监听 `ai-status` 事件，更新七彩流光状态 class
  - [x] 监听 `ai-error` 事件，显示错误提示
- [ ] 实现思考模型展示
  - [x] 思考过程默认折叠，显示 "思考中..." + 计时
  - [x] 点击可展开查看完整思考内容
  - [x] 思考结束后显示思考耗时
- [ ] 实现停止生成按钮
  - [x] 生成中显示停止按钮，点击调用 `ai_stop_generation`
  - [x] 生成完成后隐藏停止按钮
- [ ] 实现简易 Markdown 渲染函数
  - [x] 处理 `**bold**`、`` `code` ``、代码块、列表
  - [x] 使用正则替换，不引入外部库
- [ ] 消息区域自动滚动到底部
- [ ] 实现清空历史按钮（展开态可见）
- [ ] Perform a self-review of your code, once you're certain it's 100% complete to the requirements in this phase mark the task as done.
- [ ] STOP and wait for human review

### Phase 5: 设置页面 — AI 配置 UI

- [x] 在 `settings.html` 中新增 "AI Agent" 配置卡片
  - [x] API 地址输入框（placeholder: `https://api.openai.com`）
  - [x] API Key 密码输入框（type="password"）
  - [x] 模型名称输入框（placeholder: `gpt-4o`）
  - [x] 模型类型检测结果标签（"✅ 思考模型" / "普通模型" / "未检测"）
  - [x] "检测模型类型" 按钮（手动触发检测）
- [x] 在 `settings.ts` 中实现 AI 设置逻辑
  - [x] `loadSettings()` 扩展：加载 AI 配置字段
  - [x] 保存时调用 `ai_save_settings` command
  - [x] 保存成功后自动触发 `ai_detect_model_type`
  - [x] 显示检测进度和结果
  - [x] 错误处理（API 不可达、Key 无效等）
- [x] 设置变更后通知主窗口更新 AI 可用状态
  - [x] 通过 Tauri 事件 `ai-settings-changed` 通知
  - [x] 主窗口收到后更新 `ai_enabled` 状态和视图切换器
- [x] Perform a self-review of your code, once you're certain it's 100% complete to the requirements in this phase mark the task as done.
- [x] STOP and wait for human review

### Phase 6: 窗口尺寸动态调整与集成测试

- [x] 实现 agent 展开态的窗口尺寸动态调整
  - [x] 新增 Tauri command `resize_for_agent(expanded: bool)`
  - [x] 展开时增大窗口高度（从 84px → 约 260px）
  - [x] 收起时恢复原始高度
  - [x] 确保窗口位置不偏移（从顶部向下扩展）
- [x] 在 Rust 后端常量中新增 agent 相关尺寸
  - [x] `CAPSULE_AGENT_EXPANDED_H`
  - [x] `CAPSULE_AGENT_EXPANDED_W`（如需）
- [x] 集成验证
  - [x] 验证 time → agent → lyric → time 视图循环切换正常
  - [x] 验证 agent 展开/收起时窗口尺寸正确变化
  - [x] 验证通知覆盖 agent 视图后正确恢复
  - [x] 验证拖拽功能在 agent 视图下正常工作
  - [x] 验证七彩流光在不同状态下的视觉效果
  - [x] 验证流式对话的完整流程（发送→思考→生成→完成）
  - [x] 验证思考模型检测和展示
  - [x] 验证设置持久化和应用重启恢复
- [x] Perform a self-review of your code, once you're certain it's 100% complete to the requirements in this phase mark the task as done.
- [x] STOP and wait for human review

## Important Considerations & Requirements

- [ ] 不要过度工程化，保持与现有代码风格一致
- [ ] 不要添加 placeholder 或 TODO 代码
- [ ] 七彩流光必须使用纯 CSS 实现（`@property` + `conic-gradient`），不使用 Canvas
- [ ] AI API 请求必须在 Rust 后端执行，不在前端直接 fetch（安全性：API Key 不暴露给 webview）
- [ ] 流式 token 通过 Tauri 事件推送，与现有事件模式（如 `lyric-update`）保持一致
- [ ] 展开态窗口尺寸变化必须平滑，不能出现闪烁
- [ ] 对话历史限制在内存中最近 20 轮，防止内存膨胀
- [ ] API Key 在设置页面以掩码显示，在前端永远不暴露明文
- [ ] 思考模型检测是自动的，用户无需手动标记
- [ ] 所有新增 UI 文本使用中文

## Technical Decisions

| 决策 | 选择 | 理由 |
|------|------|------|
| AI 请求执行位置 | Rust 后端 | API Key 安全性，复用现有 reqwest client |
| 流式通信方式 | Tauri 事件 | 与现有 lyric-update 模式一致，前端无需 WebSocket |
| 七彩流光实现 | CSS @property + conic-gradient | 性能优于 Canvas，GPU 加速 |
| Markdown 渲染 | 自实现正则替换 | 避免引入外部依赖，仅需基础格式 |
| 思考模型检测 | 测试请求 + 响应结构分析 | 最可靠的运行时检测方式 |
| 对话历史存储 | 内存 + 可选文件持久化 | V1 优先内存，Should Have 中加入持久化 |
| 展开态尺寸 | 动态 Tauri window resize | 灵动岛窗口本身需要变大才能容纳对话 UI |

## Debugging Protocol

If issues arise during implementation:

- **SSE 解析失败**：检查不同 API 提供商的响应格式差异，确认兼容 `data: [DONE]` 终止标记
- **七彩流光不显示**：检查 `@property` 浏览器兼容性（Tauri webview 基于 WebView2，支持 Chromium 特性）
- **窗口尺寸闪烁**：使用 `set_size` 前先计算目标位置，避免先 resize 再 reposition
- **API Key 泄露风险**：确认前端代码中无法访问到明文 key
- **流式中断后状态不一致**：确保 `ai_stop_generation` 正确清理状态并发送 `ai-status: completed`
- **思考模型误判**：增加多种检测策略（字段名、模型名称模式匹配作为辅助）

## QA Checklist

- [ ] 所有用户需求已实现
- [ ] 无关键代码异味警告
- [ ] 代码遵循项目现有约定和风格
- [ ] 七彩流光在所有状态下视觉效果正确
- [ ] AI 对话流式响应稳定，无丢 token
- [ ] 思考模型检测准确（测试 OpenAI、DeepSeek 等）
- [ ] API Key 在前端不可见
- [ ] 展开/收起窗口尺寸变化平滑
- [ ] 视图切换不影响现有 time/lyric 模式
- [ ] 设置持久化正确，重启后恢复
- [ ] 错误场景（网络断开、API 错误、超时）有合理提示
- [ ] 内存使用合理，对话历史有上限
- [ ] 所有 UI 文本为中文
