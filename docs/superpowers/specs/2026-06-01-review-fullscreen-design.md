# 智能复盘分析诊断报告全屏查看功能设计规范 (Spec)

## 1. 背景与目标
目前系统提供的 AI 智能复盘分析诊断报告是以卡片容器形式渲染的，高度和宽度受限（尤其是在复杂的大盘背景下），不便于用户阅读长篇的深度分析报告。
本功能的目标是为复盘报告组件增加一个沉浸式全屏查看模式，使用户能够在一个无干扰、更开阔的排版版式中细读分析报告。

## 2. 交互与视觉设计

### 2.1 全屏触发入口
在 `ReviewDrawer.tsx` 中 `view === 'detail'` 的页面内，我们将在以下两处添加全屏入口：
1. **控制按钮组**：在顶栏的“复制全文”、“导出 MD”按钮旁边，添加一个“全屏查看”的按钮，采用 `Maximize2` 图标。
2. **报告标题右侧**：在“📝 智能复盘分析诊断报告”标题右侧，添加一个较小但精致的 `Maximize2` 图标按钮。

### 2.2 全屏蒙层 (Overlay) 布局
全屏状态由 React 的 `isFullscreen` 状态控制。当开启时，将渲染一个固定定位（`fixed`）的容器：
- **背景蒙层**：使用 `fixed inset-0 z-[100] backdrop-blur-xl bg-black/65`，提供深邃的暗黑玻璃拟态效果，并将应用的其他 UI 完全隔离。
- **内容卡片**：
  - 宽度限制：`max-w-[1000px] w-full`
  - 高度限制：`h-[calc(100vh-80px)]`，居中对齐，上下留有 `40px` 间距。
  - 样式：背景使用 `bg-[#0f172a]/90` 加上 `border border-white/10 shadow-2xl rounded-3xl flex flex-col overflow-hidden`。
  
### 2.3 全屏顶部控制条 (Toolbar)
在全屏卡片的顶部，有一个常驻的控制条：
- **标题区**：展示报告标题，形如 `claude 复盘分析报告 (2026-06-01)`。
- **字号缩放区**：
  - 提供 `A-` 和 `A+` 按钮，通过 `fontSize` 状态动态控制 Markdown 文本大小。
  - 字号调节范围限制在 `12px` ~ `18px`，步长为 `1px`，默认值为 `14px`。
- **操作按钮**：同步放置“复制全文”、“导出 MD”、“打印”、“退出全屏(ESC)”按钮。

### 2.4 关闭/退出机制
- 点击右上角的“最小化”或“关闭”图标。
- 点击全屏卡片外部的蒙层区域（即背景遮罩层）。
- 监听系统 `Escape` 按键。

---

## 3. 技术实现方案

### 3.1 状态声明
```tsx
const [isFullscreen, setIsFullscreen] = useState(false);
const [fullscreenFontSize, setFullscreenFontSize] = useState(14);
```

### 3.2 键盘监听与滚动控制
当 `isFullscreen` 变为 `true` 时：
1. 在全局 `window` 上绑定 `keydown` 事件，若按下 `Escape` 则关闭全屏。
2. 将 `document.body.style.overflow` 设置为 `'hidden'`，防止背景穿透滚动。
当 `isFullscreen` 关闭或组件销毁时，移除事件监听并将 `document.body.style.overflow` 恢复原样。

### 3.3 界面结构 (JSX)
在 `ReviewDrawer.tsx` 中，若 `isFullscreen === true`，则以 Portal 或直接在最外层渲染全屏的 `div`。

---

## 4. 验证与测试
- **功能测试**：
  - 验证点击“全屏”按钮可以正常进入和退出全屏。
  - 验证点击背景遮罩区能正常退出。
  - 验证按 `ESC` 键能正常退出。
  - 验证 `A-` 和 `A+` 按钮能改变文本大小。
- **兼容性测试**：
  - 确保 Markdown 渲染出的表格、代码块、列表在全屏模式下表现良好，不发生溢出或变形。
