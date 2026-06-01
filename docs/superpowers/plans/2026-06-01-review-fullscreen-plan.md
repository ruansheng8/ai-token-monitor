# 智能复盘分析诊断报告全屏查看功能实现计划 (Plan)

## 1. 目标
在 `ReviewDrawer.tsx` 中为诊断报告添加全屏查看模式，支持磨砂玻璃蒙层背景、ESC退出、字号调节，并与整体暗黑风格融合。

## 2. 拟修改文件

### [前端组件]
#### [MODIFY] [ReviewDrawer.tsx](file:///d:/VibeCoding/ai-token-monitor/src/components/ReviewDrawer.tsx)
- 引入 `Maximize2` 和 `Minimize2` 图标。
- 在 `ReviewPage` 组件内声明状态：
  - `const [isFullscreen, setIsFullscreen] = useState(false);`
  - `const [fullscreenFontSize, setFullscreenFontSize] = useState(14);`
- 编写 `useEffect` 监听键盘的 `Escape` 按键。当 `isFullscreen` 为 `true` 时，绑定键盘事件并在关闭时解绑；同时设置 `document.body.style.overflow = 'hidden'` 并在关闭时复原。
- 在顶栏按钮区添加“全屏查看”按钮（与“导出 MD”同级）。
- 在报告标题 `📝 智能复盘分析诊断报告` 右侧添加全屏图标入口。
- 在组件底部（或合适的位置）条件渲染全屏 Modal 容器：
  - 遮罩背景：`fixed inset-0 z-[100] backdrop-blur-xl bg-black/65 flex flex-col items-center justify-center p-6 sm:p-10 animate-fade-in`
  - 居中卡片：`max-w-[1000px] w-full h-[calc(100vh-80px)] rounded-3xl bg-[#0f172a]/95 border border-white/10 flex flex-col overflow-hidden shadow-2xl text-left`
  - 全屏控制条：
    - 展示标题、引擎名称。
    - 字体调节按钮：`A-` 与 `A+`（字号范围 `12px` ~ `18px`）。
    - 动作按钮：导出、复制、打印、退出全屏。
  - Markdown 内容容器：大号边距，支持滚动，字号与 `fullscreenFontSize` 绑定。

---

## 3. 验证方案
1. 运行 `tsc -b --noEmit` 进行前端类型检查。
2. 运行 `npm run lint` 检查代码样式。
3. 验证进入/退出全屏逻辑：
   - 点击报告页面的“全屏查看”或标题旁的放大图标，能流畅呼出磨砂玻璃全屏视图。
   - 点击顶部的“退出全屏”、点击蒙层空白背景区、或在键盘上按 `ESC` 键，全屏视图能平滑消失。
4. 验证全屏状态下的功能：
   - 调整字号（点击 `A-` 和 `A+`）可以实时缩放文本大小，且字号在 `12px` 至 `18px` 之间受到限制。
   - 全屏卡片内的“复制全文”、“导出 MD”和“打印”功能工作正常。
