# UI 配色修改实施计划 - 极简冷灰与皇家蓝

此计划的目标是将 Token Insight 首页在 Light 模式（浅色模式）下的配色修改为冷灰背景、纯白卡片、皇家蓝与薄荷绿高亮的极简扁平化现代 SaaS 风格（对应 Shopeers 仪表盘配色风格）。布局保持不变，Dark 模式保持不变。

---

## 1. 拟修改文件

### 1.1. 全局样式文件

#### [MODIFY] [index.css](file:///d:/VibeCoding/ai-token-monitor/src/index.css)
* 修改 `:root` 中的 CSS 变量：
  * `--bg-app` 改为偏冷灰蓝色 `#f4f6f8`。
  * `--card-bg` 改为纯白 `#ffffff`，移除毛玻璃的半透明效果。
  * `--card-border` 改为更显精致的浅灰 `rgba(226, 232, 240, 0.8)`。
  * `--card-border-hover` 改为淡皇家蓝 `rgba(37, 99, 235, 0.25)`。
  * 微调 `--neon-green`、`--neon-cyan`、`--neon-orange` 为薄荷绿、天蓝、琥珀黄。
* 重定义 `.dashboard-header-bg` 顶部渐变，移除彩色发光斑点，改为极其素雅的纯净线性灰白渐变加下边框。
* 重定义 `.kpi-blue`, `.kpi-cyan`, `.kpi-orange`, `.kpi-green`, `.kpi-purple`, `.kpi-slate` 等 KPI 卡片底色，使其在浅色模式下统一为纯白底 `#ffffff` 及浅灰细边框，仅在 `.dark` 下保留原彩底渐变。

### 1.2. 图表组件文件

#### [MODIFY] [DailyTrendChart.tsx](file:///d:/VibeCoding/ai-token-monitor/src/components/charts/DailyTrendChart.tsx)
* 在 `chartOption` 中，当 `!isDark` 时，将首选色板替换为更平缓高级的冷色板 `[薄荷绿, 浅青色, 皇家蓝, 科技紫]`。
* 将推理 Token 折线图及折线面积渐变的基色根据主题动态映射。

#### [MODIFY] [PerformanceChart.tsx](file:///d:/VibeCoding/ai-token-monitor/src/components/charts/PerformanceChart.tsx)
* 将折线图（TPS 和 Latency）在浅色模式下的配色替换为皇家蓝与薄荷绿，并动态适配面积渐变基色。

#### [MODIFY] [ProjectTrendChart.tsx](file:///d:/VibeCoding/ai-token-monitor/src/components/charts/ProjectTrendChart.tsx)
* 支持浅色模式自适应色板，使 Top 排名的主要项目默认显示皇家蓝和薄荷绿。

#### [MODIFY] [SourceTrendChart.tsx](file:///d:/VibeCoding/ai-token-monitor/src/components/charts/SourceTrendChart.tsx)
* 针对浅色模式创建一套平滑、冷调的 `lightEngineColors` 配色映射，使主要开发工具的柱状图颜色具有皇家蓝与薄荷绿的质感。

---

## 2. 验证方案

### 2.1. 自动构建与类型检查
在 `src-tauri/` 和根目录下分别执行以下命令进行检查：
* `npx tsc -b --noEmit` （前端类型检查）
* `npm run lint` （Linter 检查）
* `npm run build` （构建验证前端静态资源包）

### 2.2. 手动/视觉核对
1. 观察浅色模式首页的背景色是否偏向于冷灰色。
2. 核对卡片是否为边界分明的纯白卡片，悬浮是否有轻微淡蓝色边框。
3. 检查看板头部背景是否极其素雅清爽，不再包含明显的红/绿/青径向渐变大斑点。
4. KPI 指标卡片是否均为纯白，里面图标与趋势数字保持彩色。
5. 图表主色调是否以皇家蓝（折线、柱子）和薄荷绿为第一、第二顺位。
6. 切换至 Dark Mode，确认深色模式依然保留原有的磨砂玻璃与渐变霓虹效果，功能正常且不受影响。
