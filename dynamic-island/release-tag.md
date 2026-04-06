---
description: Tauri 版本 Tag 发布规范流程
---

# Tauri 版本 Tag 发布规范

## Tag 命名规范

格式: `tauri-v{主版本}.{次版本}.{补丁版本}`

示例:
- `tauri-v0.1.0` - 首个可用版本
- `tauri-v0.2.0` - 新增功能
- `tauri-v0.2.1` - Bug 修复

版本号规则:
- **主版本**: 重大架构变更或不兼容更新
- **次版本**: 新增功能（向后兼容）
- **补丁版本**: Bug 修复或小改动

## Commit Message 规范（Conventional Commits）

格式: `<type>(<scope>): <description>`

### Type 类型
- `feat`: 新功能
- `fix`: Bug 修复
- `refactor`: 代码重构（不改变功能）
- `style`: 样式/UI 调整
- `perf`: 性能优化
- `docs`: 文档更新
- `chore`: 构建/工具/依赖变更
- `test`: 测试相关

### Scope 范围（可选）
- `ui` - 前端界面
- `media` - 媒体控制
- `weather` - 天气功能
- `agent` - AI Agent
- `settings` - 设置
- `core` - 核心/后端

### 示例
```
feat(media): 添加音乐进度条拖拽功能
fix(weather): 修复天气 API 超时问题
refactor(core): 重构窗口状态管理逻辑
style(ui): 优化灵动岛展开动画效果
chore: 升级 Tauri 到 v2.x
```

## 版本号文件

发布新版本前需同步更新以下 3 个文件中的版本号：
- `dynamic-island/src-tauri/tauri.conf.json` → `"version": "X.Y.Z"`
- `dynamic-island/src-tauri/Cargo.toml` → `version = "X.Y.Z"`
- `dynamic-island/package.json` → `"version": "X.Y.Z"`

## 发布流程（GitHub Actions 自动化）

已配置 `.github/workflows/release.yml`，推送 `tauri-v*` tag 后 **自动完成**：
- ✅ 在 Windows 环境构建 Tauri 应用
- ✅ 创建 GitHub Release（标题：`[Tauri] vX.Y.Z`）
- ✅ 上传 `.exe` 和 `.msi` 安装包
- ✅ 标记为 Latest Release

构建状态查看：https://github.com/Python-island/Python-island/actions

### 1. 确认分支
```bash
git branch --show-current
# 确保在 tauri-island 分支
```

### 2. 更新版本号
修改上述 3 个文件中的版本号为新版本。

### 3. 提交代码
```bash
git add .
git commit -m "chore: bump version to vX.Y.Z"
```

### 4. 创建 Tag
```bash
git tag -a tauri-vX.Y.Z -m "Tauri vX.Y.Z: 简要更新说明"
```

### 5. 推送（之后全自动）
```bash
# 推送代码
git push origin tauri-island

# 推送 tag（触发 GitHub Actions 自动构建+发布）
git push origin tauri-vX.Y.Z
```

### 6. 验证
- 前往 https://github.com/Python-island/Python-island/actions 查看构建进度
- 构建完成后 Releases 页面会自动出现新版本和安装包

## 手动发布（备用）

如 GitHub Actions 构建失败，可在本地手动构建后上传：
```bash
# 本地构建
cd dynamic-island
npm run tauri build
```
安装包路径：
- `src-tauri/target/release/bundle/nsis/DynamicIsland_X.Y.Z_x64-setup.exe`
- `src-tauri/target/release/bundle/msi/DynamicIsland_X.Y.Z_x64_en-US.msi`

然后前往 https://github.com/Python-island/Python-island/releases/new 手动创建 Release 并上传。

## AI 助手提示词模板

每次发版时，可以使用以下提示词：

```
请帮我按 /release-tag 规范发布新版本
```

AI 助手会自动完成：
1. 确认分支
2. 查看修改内容，生成规范 commit message
3. 更新 3 个文件中的版本号
4. 创建 tag 并推送（触发自动构建+发布）
