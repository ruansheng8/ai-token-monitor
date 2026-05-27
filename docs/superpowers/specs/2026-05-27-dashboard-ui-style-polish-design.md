# AI Token 监控看板 UI 样式与颜色美化设计方案（静谧极光质感）

本设计文档旨在将 AI Token 监控工具的 UI 升级为 **现代高端 SaaS / 磨砂玻璃拟态与静谧极光质感 (Quiet Aurora Glassmorphism)**。整个设计去除了刺眼的动效和高对比度边框，代之以极其淡雅的静态漫反射渐变与高透明度毛玻璃卡片，以确保用户在进行数据分析时视觉的舒适度与沉浸感。

## 1. 全局设计规范 (CSS 样式升级)

修改 [src/index.css](file:///d:/VibeCoding/ai-token-monitor/src/index.css) 中的全局变量与样式：

### CSS 变量微调
* **毛玻璃背景 (Light)**：卡片底色 `--card-bg` 从 `rgba(255, 255, 255, 0.7)` 调整为 `rgba(255, 255, 255, 0.75)`。
* **卡片边框 (Light)**：边框颜色 `--card-border` 改为更淡雅的 `rgba(148, 163, 184, 0.15)`。
* **漫反射阴影 (Light)**：阴影改为 `0 12px 40px -12px rgba(15, 23, 42, 0.04)`。
* **毛玻璃背景 (Dark)**：卡片底色 `--card-bg` 改为半透明极夜蓝 `rgba(11, 19, 36, 0.7)`。
* **卡片边框 (Dark)**：边框颜色 `--card-border` 降为微弱的白色半透明 `rgba(255, 255, 255, 0.05)`。
* **漫反射阴影 (Dark)**：阴影升级为更具质感的 `0 20px 50px -16px rgba(0, 0, 0, 0.4)`。

### 静态背景设计
* **极光斑点背景**：去掉 `@keyframes pulse-glow` 带来的漂移动效和频繁缩放，将背景色块（Spots）设为静态漫反射光晕，避免视觉干扰。
* **头部流光渐变**：保留大气的流光背景配色，但去除呼吸式跳动，呈现宁静雅致的极光底色。

---

## 2. ECharts 图表视觉重构

美化 [DailyTrendChart.tsx](file:///d:/VibeCoding/ai-token-monitor/src/components/charts/DailyTrendChart.tsx) 和 [SourceTrendChart.tsx](file:///d:/VibeCoding/ai-token-monitor/src/components/charts/SourceTrendChart.tsx)：

### 调色板与分割线
* 全局应用统一的八色调色板 `PALETTE_COLORS`，微调前四色（薄荷绿、明亮青、柔和粉、科技紫）在暗色和亮色下的饱和度表现。
* X轴/Y轴网格分割线（splitLine）：亮色模式设为淡雅的 `#f1f5f9`，暗色模式设为极浅的 `rgba(255, 255, 255, 0.04)`。

### 卡片式提示框 (Tooltip)
* **背景色与毛玻璃**：亮色模式为 `rgba(255, 255, 255, 0.96)`，暗色模式为 `rgba(11, 21, 40, 0.94)`。添加 `backdrop-filter: blur(8px)`。
* **排版排布**：内容使用双栏对齐（Flex layout），数值部分全部强制使用 `JetBrains Mono`（monospace）等宽字体，提升可读性。

### 折线与柱状图样式
* 折线图强制使用 `smooth: true` 平滑曲线，数据标记点采用白色描边（`borderWidth: 2, borderColor: '#fff'`），面积渐变填充起始透明度设为 `0.16`，向下方淡化至 `0.01`。
* 柱状图使用 4px 圆角（堆叠柱状图），并使用白色/极夜蓝描边作为自然切缝线。

---

## 3. 页面关键组件重塑 (App.tsx)

修改 [src/App.tsx](file:///d:/VibeCoding/ai-token-monitor/src/App.tsx) 中的组件样式：

### 药丸形时间范围切换器
* 整体容器采用 `.pill-container` 类，圆角设为 `rounded-full`，带微弱阴影和精细边框。
* 激活选项应用渐变底色：`linear-gradient(to right, #3b82f6, #06b6d4)`（活力蓝到明亮青），搭配优雅白色文字与微弱投影；未激活选项采用极简的文字过渡和淡灰悬浮态。

### 增量同步进度条
* 高度缩窄为 `h-1.5`，去除之前强烈的发光动效，采用平滑的 `bg-gradient-to-r from-neon-cyan to-neon-purple` 极光渐变，进度更新时过渡更为柔和。

### 历史会话数据表格
* 悬浮背景色高亮使用极细微的毛玻璃半透明：亮色为 `rgba(15, 23, 42, 0.015)`，暗色为 `rgba(255, 255, 255, 0.015)`。
* 保留原有的中文单位数字格式化，增强表头与列数据的对齐。

---

## 4. 验证计划

### 自动化与人工验证
1. 启动本地开发服务验证是否编译成功。
2. 在浏览器中打开应用程序，人工检查亮色和暗色模式，确保在两种主题下，所有毛玻璃卡片、图表 tooltip、药丸形切换器、数据表格及进度条都渲染正常且符合“静谧极光”的调性。
