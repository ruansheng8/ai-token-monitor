# 仅保留“每日用量走势”上方 UI 配色调整的实现计划

在提交 `507569393eb3c0f1ed3d6e5639b0abbe462f665c` 中，原版 UI 风格（流光渐变玻璃拟态暗黑/明亮风格）被大幅修改为极简冷灰与皇家蓝配色，导致整站大部分样式都发生了变化。

用户要求**只调整`每日用量走势` 上面的UI**，其余都不要动。

## 变更范围与策略

根据 Dashboard 页面结构，“每日用量走势”上面的 UI 包含：
1. **顶栏 Header** (`.dashboard-header-bg`)
2. **KPI 核心指标看板** (`.kpi-*` 样式卡片)
3. **时间筛选控制栏 / 药丸切换器** (`.pill-container`)

其余区域（`每日用量走势` 图表本身，以及下方的所有图表与卡片等）都需要**恢复为修改前的样式**。

为此，我们将执行以下变更：
1. **完全回滚 4 个图表文件**，将其恢复至 5075693 之前的状态，以还原它们本来的色彩调色板（ECharts 的 `PALETTE_COLORS` 等）：
   - [DailyTrendChart.tsx](file:///d:/VibeCoding/ai-token-monitor/src/components/charts/DailyTrendChart.tsx)
   - [PerformanceChart.tsx](file:///d:/VibeCoding/ai-token-monitor/src/components/charts/PerformanceChart.tsx)
   - [ProjectTrendChart.tsx](file:///d:/VibeCoding/ai-token-monitor/src/components/charts/ProjectTrendChart.tsx)
   - [SourceTrendChart.tsx](file:///d:/VibeCoding/ai-token-monitor/src/components/charts/SourceTrendChart.tsx)
2. **修改全局样式文件**：
   - [index.css](file:///d:/VibeCoding/ai-token-monitor/src/index.css)：回滚 `:root`（浅色模式）下的全局基础 CSS 变量，恢复原版的透明玻璃背景及 neon 霓虹颜色变量。保留 `.dashboard-header-bg`、`.kpi-*` 系列和 `.pill-container` 的新样式。

---

## 详细变更方案

### 1. 图表组件

我们将使用 Git 工具把以下 4 个图表回滚到 `507569393eb3c0f1ed3d6e5639b0abbe462f665c` 的父提交状态：
- [DailyTrendChart.tsx](file:///d:/VibeCoding/ai-token-monitor/src/components/charts/DailyTrendChart.tsx)
- [PerformanceChart.tsx](file:///d:/VibeCoding/ai-token-monitor/src/components/charts/PerformanceChart.tsx)
- [ProjectTrendChart.tsx](file:///d:/VibeCoding/ai-token-monitor/src/components/charts/ProjectTrendChart.tsx)
- [SourceTrendChart.tsx](file:///d:/VibeCoding/ai-token-monitor/src/components/charts/SourceTrendChart.tsx)

### 2. 全局样式 [index.css](file:///d:/VibeCoding/ai-token-monitor/src/index.css)

我们将修改 `:root` 样式块，把全局浅色模式的 CSS 变量改回原本的值，以恢复图表下方的卡片以及全局页面的流光渐变毛玻璃样式。

```css
:root {
  /* Light Mode (Default) Colors */
  --bg-app: #f8fafc;
  --bg-secondary: #ffffff;
  --card-bg: rgba(255, 255, 255, 0.75);
  --card-border: rgba(148, 163, 184, 0.15);
  --card-border-hover: rgba(6, 182, 212, 0.4);
  --card-shadow: 0 12px 40px -12px rgba(15, 23, 42, 0.04);
  --card-shadow-hover: 0 16px 48px -12px rgba(15, 23, 42, 0.08);

  --text-primary: #0f172a;
  --text-secondary: #475569;
  --text-muted: #94a3b8;

  --table-border: rgba(15, 23, 42, 0.05);
  --table-row-hover: rgba(15, 23, 42, 0.015);

  --scrollbar-track: rgba(15, 23, 42, 0.05);
  --scrollbar-thumb: rgba(15, 23, 42, 0.12);
  --scrollbar-thumb-hover: rgba(6, 182, 212, 0.5);

  --decor-opacity: 0.06;

  /* Neon Colors for Light Mode */
  --neon-cyan: #0891b2;
  --neon-purple: #9333ea;
  --neon-pink: #db2777;
  --neon-orange: #ea580c;
  --neon-green: #16a34a;
  --neon-blue: #2563eb;
  --neon-gold: #ca8a04;
  --neon-teal: #0d9488;
}
```

同时保留已修改的 Header 顶栏、KPI 指标卡及切换药丸的冷灰极简背景。

---

## 验证计划

1. **类型与格式检查**：
   - 运行 `npx tsc -b --noEmit` 确保没有 TypeScript 编译错误。
   - 运行 `npm run lint` 验证 ESLint 代码规范。
2. **构建验证**：
   - 运行 `npm run build` 确保前端构建无误。
   - 可在开发环境下运行查看 UI 实际展现是否符合预期。
