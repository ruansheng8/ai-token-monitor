# 首页布局紧凑度优化设计规范 (Option A)

本设计文档旨在通过微调页面间距、缩减部分元素的大小以及优化图表高度，使得「每日用量走势」图表在普通桌面或笔记本屏幕（如 1080p）下无需滚动即可完整展示。

## 用户评审要点
- **图表高度调整**：将 ECharts 走势图的高度从原来的 350px 调整为 300px。
- **页面间距缩减**：将全局垂直 Gap 从 24px (gap-6) 调整为 16px (gap-4)。
- **KPI 卡片尺寸优化**：微调 KPI 卡片的内边距和字体大小，保持其精致感的同时减少垂直占用空间。

## 方案设计

### 1. 全局布局容器微调
修改 [App.tsx](file:///d:/VibeCoding/ai-token-monitor/src/App.tsx) 中的主容器样式：
- 从 `p-6 flex flex-col gap-6` 调整为 `p-4 sm:p-5 flex flex-col gap-4`，减少不必要的垂直留白。

### 2. Header 与时间筛选栏优化
- 头部导航栏：将内边距从 `px-7 py-4 gap-4` 微调为 `px-5 py-3 gap-3`，将标题大小微调，使其更紧凑。
- 时间筛选栏：内边距从 `p-4` 微调为 `p-3`，减少高度。

### 3. KPI 指标卡片紧凑化
- 卡片内边距从 `p-5` 调整为 `p-3.5`。
- 图标容器从 `w-12 h-12` 调整为 `w-10 h-10`，内部图标从 `w-6 h-6` 缩放到 `w-5 h-5`。
- 卡片数值字体从 `text-2xl` 调整为 `text-xl`，外边距 `mb-1` 调整为 `mb-0.5`。

### 4. 走势图表部分优化
- 走势图外层 section 的 padding 从 `p-6` 调整为 `p-4`，头部底边距从 `mb-5` 调整为 `mb-3`。
- 将 [DailyTrendChart.tsx](file:///d:/VibeCoding/ai-token-monitor/src/components/charts/DailyTrendChart.tsx) 和 [SourceTrendChart.tsx](file:///d:/VibeCoding/ai-token-monitor/src/components/charts/SourceTrendChart.tsx) 中 ECharts 容器的高度从 `350px` 调整为 `300px`。

## 验证计划
- 启动本地开发服务，在主流分辨率（如 1920x1080、1440x900）下预览首页。
- 确认「每日用量走势」图表能够在不滚动屏幕的情况下完全呈现在视口（Viewport）内。
