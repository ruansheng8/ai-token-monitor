# 设计文档：AI Token Monitor 增加日历热力贡献图组件

本设计文档旨在 AI Token Monitor 前端监控面板中，于“每日用量走势图”下方新增一个类似 GitHub 的“日历热力图（Calendar Heatmap）”组件。该图表支持 TOKEN 总数和会话总数两个统计维度的无缝切换。

## 目标与背景

为了让用户能更加直观地评估中长周期（如 365 天滚动一年）的活跃度和使用密度，我们将引入类似 GitHub 贡献图的日历热力图。
主要功能点如下：
1. **摆放位置**：独立卡片形态，放置在“每日用量走势图（DailyTrendChart）”卡片正下方。
2. **展示跨度**：始终滚动展示最近的 365 天（1 年）的网格。
3. **数据补全与置灰**：自动比对最近 365 天的所有日期。对于有数据的日期展示为科技绿渐变色，没有数据的日期（或在筛选范围外的日期）作为空白底色展示为 GitHub 标志性的淡灰色方格（#ebedf0）。
4. **多维度切换**：组件右上角支持“TOKEN 总数”与“会话总数”维度切换。

---

## 方案设计与变更细节

我们选用 **ECharts Calendar Heatmap (方案 A)** 来实现这一需求，保证 100% 的图表性能、主题（明亮/暗黑模式）适配以及高端的白卡片 Tooltip 动画统一。

### 1. 新建 `CalendarHeatmap` 组件
我们将新建一个单独的图表组件 `src/components/charts/CalendarHeatmap.tsx`。

#### 数据格式与属性 (Props)
```typescript
interface DailyTrend {
  date: string;
  input: number;
  output: number;
  cached: number;
  thinking: number;
  sessions: number;
}

interface CalendarHeatmapProps {
  data: DailyTrend[];
  theme: 'light' | 'dark';
}
```

#### 核心实现逻辑
1. **日期区间计算**：
   - 自动生成今天（`today`）与 364 天前（`startDate`）的精确日期字符串（格式：`YYYY-MM-DD`）。
   - 将该范围设定为 ECharts 日历的范围：`calendar: { range: [startDate, today] }`。
2. **数据转换与补零**：
   - 根据选中的维度（`dimension: 'tokens' | 'sessions'`）：
     - `tokens`：值为 `item.input + item.output`。
     - `sessions`：值为 `item.sessions`。
   - 建立 `Map<string, number>` 快速索引已有的 `daily_trends` 数据。
   - 循环最近 365 天中的每一天，如果在 Map 中存在则填充对应的值，否则填充 `0`。
   - ECharts `series.data` 格式转换为 `[ [date, value], ... ]` 的数组。
3. **视觉渐变与动态最大值**：
   - 使用 `visualMap` 来控制颜色深浅。
   - `visualMap.max` 将从当前渲染日期的**实际最大值**动态获取（规避写死固定数值导致不同使用量级别下的颜色无区分）。
   - 配色阶梯（以绿色为例）：`['#ebedf0', '#c6e48b', '#7bc96f', '#239a3b', '#196127']`（在暗黑模式下自动使用适配的深绿底色组合）。
4. **药丸切换器设计**：
   - 在卡片顶部右上角，使用符合 `dashboard-ui-style` 规范的药丸切换按钮。
   - 选中状态使用翠绿到深绿的微光渐变，配合立体投影与 Hover 缩放动画。

---

### 2. 在 `src/App.tsx` 中集成
1. **导入组件**：
   ```typescript
   import { CalendarHeatmap } from './components/charts/CalendarHeatmap';
   ```
2. **插入布局**：
   在 `DailyTrendChart` 所在的 `<section>` 下方，插入新的日历热力图区域：
   ```tsx
   {/* 日历热力图 */}
   <section className="chart-section glass-card p-4 sm:p-5 hover:-translate-y-0.5 hover:shadow-[0_22px_56px_rgba(15,23,42,0.10)] transition-all duration-200">
     <CalendarHeatmap data={data?.daily_trends || []} theme={theme} />
   </section>
   ```

---

## 影响评估与兼容性

1. **零后端负荷**：
   - 完全在前端利用已有的 `daily_trends` 数组进行 365 天的组装、过滤和缺省日期自动填零。
   - **无需修改任何后端 Actix / Axum 路由、数据库结构或 SQL 查询**，安全度与加载性能极高。
2. **主题自适应**：
   - 组件深度融合 `theme` 参数。当切换暗黑/明亮模式时，热力图网格线、图表背景、文字、Tooltip 的背景毛玻璃深度及空白小方格颜色均会动态自适应调整，符合 Premium 数据看板规范。
3. **极佳响应式**：
   - ECharts Calendar 支持 `cellSize: ['auto', 12]`，使网格可以在大屏下自适应拉伸，小屏下展示滚动条，避免出现挤压变形。

---

## 验证计划

### 1. 手动验证
- **维度切换验证**：
  - 点击卡片右上角的“TOKEN 总数”与“会话总数”，验证热力图的格子颜色分布与 Legend 数值是否即时发生了相应变化。
- **空白补零验证**：
  - 检查没有记录的日期是否显示为淡灰色方格，验证日期数是否包含整整一年（53 周）。
- **Tooltip 高端提示验证**：
  - 悬浮在热力图方块上，验证 Tooltip 弹出卡片是否美观，内容包含日期、星期、该维度具体用量。
- **主题切换适配**：
  - 点击大盘右上角的主题切换按钮（月亮/太阳），验证热力图色彩阶梯、卡片描边及空白格颜色是否同步变更为暗色或明亮设计。

### 2. 自动化构建验证
- 运行打包编译指令 `npm run build`，确保 TypeScript 类型校验完全通过，Vite 打包无任何报错。
