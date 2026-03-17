# 右击收起功能规划文档

## 需求概述

添加右击事件，实现右击收起功能：
- 右击胶囊区域时收起到顶部，显示一个很小的绿条
- 点击绿条可以重新展开

## 项目结构概览

```
dynamic-island/
├── index.html                    # 主页面结构
├── src/
│   ├── main.ts                   # 主要前端逻辑 (~1200行)
│   ├── styles.css                # 主样式
│   ├── styles-agent.css          # AI Agent 样式
│   └── settings.ts               # 设置页面
└── src-tauri/
    └── src/
        ├── main.rs               # 入口
        └── lib.rs                # 后端核心逻辑 (~2000+行)
```

## 需要修改的文件

### 1. `index.html` (前端结构) ✅ 已完成

**修改内容：**
- 添加一个收起状态的小绿条元素 `<div id="collapsed-indicator">`

**位置：** 在 `</body>` 标签前

---

### 2. `src/styles.css` (前端样式) ✅ 已完成

**新增样式：**

```css
/* 收起状态的小绿条 */
#collapsed-indicator {
    display: none;
    position: fixed;
    top: 5px;
    left: 50%;
    transform: translateX(-50%);
    width: 60px;
    height: 4px;
    background: linear-gradient(90deg, #22c55e, #2edb67, #22c55e);
    border-radius: 2px;
    cursor: pointer;
    pointer-events: auto;
    box-shadow: 0 0 8px rgba(46, 219, 103, 0.5);
    transition: transform 0.2s ease, box-shadow 0.2s ease, width 0.2s ease;
    z-index: 1000;
}

#collapsed-indicator:hover {
    transform: translateX(-50%) scaleX(1.15);
    width: 70px;
    box-shadow: 0 0 14px rgba(46, 219, 103, 0.7);
}

/* 胶囊收起状态 */
#island-capsule.minimized {
    width: 0 !important;
    height: 0 !important;
    opacity: 0;
    pointer-events: none;
}

/* body 收起状态 - 只显示绿条 */
body.minimized #collapsed-indicator {
    display: block;
}
```

---

### 3. `src/main.ts` (前端逻辑) ✅ 已完成

**新增变量：**
```typescript
const collapsedIndicator = document.getElementById("collapsed-indicator") as HTMLDivElement;
let isMinimized = false;
```

**新增函数：**
```typescript
function minimizeIsland() { ... }
function expandFromMinimized() { ... }
```

**新增事件监听：**
- `capsule.addEventListener("contextmenu", ...)` - 右击收起
- `collapsedIndicator.addEventListener("click", ...)` - 绿条点击展开

---

### 4. `src-tauri/src/lib.rs` (后端逻辑) ✅ 已完成

**新增常量：**
```rust
const MINIMIZED_W: f64 = 70.0;
const MINIMIZED_H: f64 = 12.0;
```

**新增命令：**
```rust
#[tauri::command]
fn set_minimized(window: tauri::WebviewWindow, state: tauri::State<'_, IslandState>, minimized: bool) { ... }
```

**新增状态字段：**
```rust
pub is_minimized: Arc<AtomicBool>,
```

---

## 实现步骤

### 阶段 1：前端实现 ✅

- [x] 在 `index.html` 添加绿条元素
- [x] 在 `styles.css` 添加收起状态样式
- [x] 在 `main.ts` 添加右击事件和状态管理

### 阶段 2：后端集成 ✅

- [x] 在 `lib.rs` 添加 `set_minimized` 命令
- [x] 添加窗口尺寸动画过渡
- [x] 添加 `is_minimized` 状态字段

### 阶段 3：编译测试 ✅

- [x] TypeScript 编译通过
- [x] Rust 编译通过
- [x] 打包成功

---

## 交互设计

### 右击行为
- 在胶囊任意位置右击 → 收起到绿条
- Agent 展开态时右击无效
- 隐私弹窗显示时右击无效

### 绿条行为
- 高度：4px
- 宽度：60px（悬停时 70px）
- 位置：屏幕顶部居中
- 悬停效果：轻微放大 + 光晕增强
- 点击：展开胶囊

---

## 测试方法

1. 启动应用后，在胶囊上**右击**，应该看到：
   - 胶囊消失
   - 顶部出现绿色小条

2. **点击绿条**，应该看到：
   - 绿条消失
   - 胶囊重新出现

3. 在 Agent 展开态时右击，应该**无反应**

---

## 完成状态

✅ **功能已实现，编译通过**

测试方法：运行 `npm run tauri dev`，右击灵动岛测试收起功能，点击绿条测试展开功能。