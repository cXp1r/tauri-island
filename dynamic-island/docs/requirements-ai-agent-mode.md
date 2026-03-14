# AI Agent 模式 — 需求文档

## 1. 问题陈述

当前灵动岛（Dynamic Island）桌面应用有两个核心模式：

- **时间模式**：显示时钟、日期、天气，是默认常驻视图
- **音乐模式**：显示歌词/歌曲信息、媒体控制，音乐播放时自动激活

用户希望在灵动岛中直接与 AI 进行对话交互，无需切换到其他窗口。灵动岛作为桌面常驻组件，天然适合作为轻量级 AI 对话入口。

**核心问题**：用户需要一个随时可用、非侵入式的 AI 对话通道，嵌入在灵动岛中，支持快速提问和查看回答。

**阶段规划**：
- **V1（当前）**：实现 AI 对话能力（文本问答）
- **V2（未来）**：工具调用能力（function calling）

---

## 2. 目标用户

- 日常使用 AI 助手的开发者和知识工作者
- 已安装灵动岛桌面应用的 Windows 用户

---

## 3. 需求层级

### 3.1 Must Have（V1 必须）

#### R1: AI Agent 视图作为第三个主模式
- 在现有 `ViewMode` 中新增 `"agent"` 类型
- 与 time、lyric 并列为可切换的主视图
- 支持通过双击胶囊切换到 agent 视图
- 在视图切换器（view-dots）中显示对应圆点

#### R2: 七彩流光边框样式
- AI 模式激活时，胶囊边框采用七彩渐变流光动画（rainbow gradient animation）
- 流光效果沿胶囊边框持续旋转流动，体现"灵动"感
- 颜色方案：彩虹色谱（红→橙→黄→绿→青→蓝→紫）循环渐变
- AI 思考/生成时流光速度加快，空闲时减慢或暂停
- 不同状态的流光表现：
  - **空闲**：缓慢流动的柔和七彩光晕
  - **思考中**：加速流动 + 脉冲呼吸效果
  - **生成中**：快速流动 + 边框亮度增强
  - **错误**：流光变为红色闪烁后恢复
- 流光效果使用 CSS `conic-gradient` + `@keyframes` 实现，不使用 Canvas 以保持性能

#### R3: AI 对话交互（收起态）
- 收起态胶囊宽度扩展为 320px（与歌词模式一致）
- 显示最新一条 AI 回复的摘要文本（单行截断）
- 带七彩流光边框动画
- 点击/双击可展开进入完整对话视图

#### R4: AI 对话交互（展开态）
- 展开态胶囊尺寸需要适当增大以容纳对话内容
- 展开态布局：
  - 顶部：AI 状态指示（模型名称 + 状态标签）
  - 中部：最新回复内容区域（支持滚动，显示最近的对话）
  - 底部：输入框 + 发送按钮
- 用户可在输入框中输入问题，按 Enter 或点击发送
- AI 回复以流式（streaming）方式逐字显示
- 支持 Markdown 基础渲染（加粗、代码块、列表）

#### R5: AI API 配置（设置页面）
- 在设置页面新增 "AI Agent" 配置卡片
- 用户可配置：
  - **API 地址**（Base URL）：文本输入框，支持 OpenAI 兼容格式的任意端点
  - **API Key**：密码输入框，输入后以掩码显示
  - **模型名称**（Model）：文本输入框，用户手动填写模型标识（如 `gpt-4o`、`claude-3-opus` 等）
- 保存时系统自动检测该模型是否为思考模型（reasoning model）：
  - 调用 API 发送一个简单测试请求
  - 检查响应中是否包含 `reasoning_content` / `thinking` 等思考字段
  - 自动标记并持久化 `is_reasoning_model: boolean`
  - 在设置页面显示检测结果标签（如 "✓ 思考模型" 或 "普通模型"）
- 配置持久化到本地设置文件

#### R6: AI 请求后端实现
- Rust 后端实现 OpenAI 兼容 API 的 HTTP 请求
- 支持 `/v1/chat/completions` 端点
- 支持 streaming（SSE）响应解析
- 对思考模型：正确处理 `reasoning_content` 字段，在前端区分显示思考过程和最终回答
- 对话历史管理：在内存中维护当前会话的消息列表（最近 N 轮）
- 提供 Tauri commands：
  - `ai_send_message(content: string)` → 发送用户消息并开始流式接收
  - `ai_stop_generation()` → 中断当前生成
  - `ai_clear_history()` → 清空对话历史
  - `ai_get_settings()` / `ai_save_settings()` → AI 配置读写
  - `ai_detect_model_type()` → 检测模型是否为思考模型

#### R7: 流式响应前端展示
- 通过 Tauri 事件接收流式 token：
  - `ai-token`：单个 token 文本
  - `ai-thinking-token`：思考过程 token（仅思考模型）
  - `ai-status`：状态变更（thinking / generating / completed / error）
  - `ai-error`：错误信息
- 思考模型的展示：
  - 思考过程以折叠区域显示（可展开查看）
  - 默认收起，显示 "思考中..." + 思考耗时
  - 最终回答正常显示
- 普通模型：直接逐字显示回复内容

#### R8: Agent 模式自动激活与回退
- 用户可通过双击切换进入 agent 视图
- Agent 模式不自动抢占其他模式（与音乐模式不同，AI 对话需要用户主动发起）
- 当用户在 agent 视图发送消息后，保持在 agent 视图直到用户手动切走
- 如果用户切走后 AI 仍在生成，生成完成后不自动切回

### 3.2 Should Have（V1 建议）

#### R9: 对话历史持久化
- 将对话历史保存到本地文件（JSON 格式）
- 应用重启后可恢复最近一次对话
- 提供清空历史的按钮

#### R10: 快捷键唤起
- 全局快捷键直接切换到 agent 视图并聚焦输入框
- 快捷键可在设置页面配置

#### R11: 停止生成按钮
- AI 生成过程中，显示 "停止" 按钮
- 点击后中断当前流式请求
- 已生成的内容保留显示

### 3.3 Could Have（未来版本）

#### R12: 工具调用能力（Function Calling）
- 支持 AI 调用预定义工具（如搜索、计算、文件操作）
- 工具调用结果回传给 AI 继续对话

#### R13: 多会话管理
- 支持创建多个独立对话会话
- 可在会话间切换

#### R14: 自定义 AI 角色/系统提示词
- 允许用户配置 system prompt
- 预设几个常用角色模板

### 3.4 Won't Have（明确不做）

- 不实现工具调用/function calling（V2 范围）
- 不内置任何 AI 模型（纯 API 调用）
- 不支持图片/文件上传
- 不支持语音输入/输出
- 不提供 AI 模型的下载或管理

---

## 4. 约束清单

### 4.1 真实约束（已确认）

| 约束 | 说明 |
|------|------|
| 窗口尺寸 | 主窗口固定 340×84px，胶囊需要在展开态适当增大以容纳对话 UI |
| 技术栈 | 前端 TypeScript + Vite，后端 Rust + Tauri v2 |
| 平台 | 仅 Windows（使用 Win32 API） |
| 性能 | 灵动岛常驻桌面顶部，CPU/内存占用必须极低 |
| 现有架构 | 必须融入现有 ViewMode 切换体系，不破坏 time/lyric 模式 |
| API 兼容性 | 仅支持 OpenAI 兼容格式的 API（覆盖大多数 LLM 提供商） |

### 4.2 假设（待验证）

| 假设 | 验证方式 |
|------|----------|
| 展开态增大尺寸不影响桌面使用体验 | 实际测试不同尺寸的视觉效果 |
| 流式 SSE 在 Rust reqwest 中稳定可用 | 开发阶段验证 |
| 思考模型检测通过测试请求可靠 | 测试多个模型提供商的响应格式 |

---

## 5. 验收标准

### AC1: Agent 视图基本显示
- **Given** 灵动岛运行中，用户已配置 AI API
- **When** 用户双击切换到 agent 视图
- **Then** 胶囊显示 agent 视图，带七彩流光边框，显示输入区域

### AC2: 七彩流光效果
- **Given** 当前处于 agent 视图
- **When** AI 处于不同状态（空闲/思考/生成/错误）
- **Then** 七彩流光边框的速度和表现随状态变化

### AC3: 发送消息并接收流式回复
- **Given** 用户在 agent 展开态输入框中输入问题
- **When** 按 Enter 发送
- **Then** AI 回复以流式方式逐字显示，状态从 thinking → generating → completed 流转

### AC4: 思考模型检测
- **Given** 用户在设置页面填写了 API 地址、Key 和模型名称
- **When** 点击保存
- **Then** 系统自动发送测试请求，检测是否为思考模型，并显示检测结果

### AC5: 思考模型展示
- **Given** 配置的模型被检测为思考模型
- **When** AI 回复包含思考过程
- **Then** 思考过程以可折叠区域显示，最终回答单独显示

### AC6: 设置持久化
- **Given** 用户在设置页面配置了 AI API 信息
- **When** 重启应用
- **Then** 之前的配置被正确恢复

### AC7: 视图切换集成
- **Given** 灵动岛有 time、lyric、agent 三个可用视图
- **When** 用户双击胶囊
- **Then** 按顺序循环切换视图，agent 视图正常参与轮转

---

## 6. 技术实现要点（供设计阶段参考）

### 6.1 前端变更
- `ViewMode` 类型扩展：`"time" | "notice" | "urls" | "lyric" | "agent"`
- 新增 `#agent-area` DOM 结构（在 `index.html` 中）
- 新增七彩流光 CSS 动画（`conic-gradient` + `@keyframes rotate`）
- 新增 agent 对话 UI 组件（消息列表、输入框、状态指示）
- `getAvailableViews()` 中根据 AI 配置是否完成决定是否包含 "agent"
- 监听 `ai-token`、`ai-thinking-token`、`ai-status`、`ai-error` 事件
- 简易 Markdown 渲染（加粗、代码、列表）

### 6.2 后端变更
- `IslandState` 新增 AI 相关字段：`ai_api_url`、`ai_api_key`、`ai_model`、`is_reasoning_model`、`ai_enabled`
- 新增 Tauri commands：`ai_send_message`、`ai_stop_generation`、`ai_clear_history`、`ai_get_settings`、`ai_save_settings`、`ai_detect_model_type`
- 实现 OpenAI 兼容 API 的 streaming HTTP 请求（使用已有的 `reqwest` 库）
- SSE 解析：逐行读取 `data: {...}` 格式，提取 token 并通过 Tauri 事件发送到前端
- 对话历史管理：`Vec<ChatMessage>` 在内存中维护，限制最大轮数
- 思考模型检测：发送测试请求，检查响应结构

### 6.3 七彩流光实现方案
```css
#island-capsule.agent-active::before {
  content: "";
  position: absolute;
  inset: -2px;
  border-radius: inherit;
  background: conic-gradient(
    from var(--rainbow-angle),
    #ff0000, #ff8800, #ffff00, #00ff00,
    #00ffff, #0088ff, #8800ff, #ff0000
  );
  animation: rainbow-rotate 3s linear infinite;
  z-index: -1;
  mask: /* 仅显示边框区域 */;
}

@keyframes rainbow-rotate {
  to { --rainbow-angle: 360deg; }
}
```

### 6.4 API 请求格式
```json
POST {base_url}/v1/chat/completions
Headers: Authorization: Bearer {api_key}

{
  "model": "{model_name}",
  "messages": [...],
  "stream": true
}
```

---

## 7. 与现有模式的交互优先级

| 场景 | 行为 |
|------|------|
| 时间模式 + 用户双击切换 | 进入 agent 视图 |
| 音乐模式 + 用户双击切换 | 进入 agent 视图 |
| Agent 模式 + 收到通知 | 通知临时覆盖（与现有逻辑一致），通知消失后回到 agent |
| Agent 模式 + 音乐开始播放 | 不自动切换，保持 agent 视图 |
| 用户从 agent 切走 | AI 生成继续在后台进行，不自动切回 |

---

## 8. 范围边界

**V1 范围内**：R1-R8（Must Have）+ R9-R11（Should Have，视开发时间）

**明确延后**：R12-R14（工具调用、多会话、自定义角色）

**触发重新评估延后项的条件**：
- V1 发布后用户反馈需要工具调用能力
- 多会话管理的需求被多次提出
