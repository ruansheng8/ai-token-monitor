# 每日用量走势增加“模型维度”统计实现计划

本文档详述了该功能的实现步骤与验证计划。

## 1. 后端实现 (Rust)

### A. 修改 `src-tauri/src/db.rs`
1. **定义结构体 `ModelTrend`**：
   ```rust
   #[derive(Serialize, Clone, Debug)]
   pub struct ModelTrend {
       pub date: String,
       pub model_name: String,
       pub tokens: i64,
       pub cost: f64,
   }
   ```
2. **修改 `AggregatedMetrics` 结构体**：
   增加 `pub model_trends: Vec<ModelTrend>` 字段。
3. **实现 SQLite 查询 (`get_aggregated_metrics_from_cache`)**：
   * 构建联合查询 `turns` 和 `sessions` 的 SQL。
   * 使用 `where_clause_raw` 过滤会话。
   * 读取查询结果，存入 `model_trends` 向量。
4. **实现 PostgreSQL 查询 (`get_pg_aggregated_metrics`)**：
   * 构建相同的联合查询 SQL。
   * 读取查询结果，存入 `model_trends` 向量。

---

## 2. 前端实现 (React + TypeScript)

### A. 修改 `src/App.tsx`
1. **扩展类型定义**：
   * 增加 `ModelTrendItem` 接口：
     ```typescript
     interface ModelTrendItem {
       date: string;
       model_name: string;
       tokens: number;
       cost: number;
     }
     ```
   * 在 `AggregatedMetrics` 接口中添加 `model_trends: ModelTrendItem[];`。
2. **修改状态 `chartDimension`**：
   ```typescript
   const [chartDimension, setChartDimension] = useState<'type' | 'source' | 'device' | 'model'>('type');
   ```
3. **更新每日趋势图展示组件切换逻辑**：
   * 增加 `chartDimension === 'model'` 的渲染逻辑分支。
   * 传入 `data.model_trends` 到 `DailyTrendChart` 中。
4. **扩展切换按钮组**：
   * 在类型、工具、设备维度按钮后面，加上 `🤖 模型维度` 切换按钮。
   * 相应地根据 `chartDimension` 动态修改图表标题。

### B. 修改 `src/components/charts/DailyTrendChart.tsx`
1. **扩展组件 Props `DailyTrendChartProps`**：
   * 增加 `modelTrends?: ModelTrendItem[]` 可选属性。
   * 将 `dimension` 属性类型扩充为 `'type' | 'device' | 'model'`。
2. **添加模型统计堆叠图逻辑**：
   * 统计该时间范围内各模型的总 tokens，按 tokens 降序排序，提取前 8 个模型名。
   * 其他所有未进入前 8 的模型在映射时，名称转换为 `"其他 (Others)"`。
   * 对每个模型建立 `date -> tokens` 的映射以提高性能。
   * 拼装 `series`，为每个模型创建一个 `'bar'` 堆叠柱状图系列，在 `colors` 数组中依次轮换使用 `PALETTE_COLORS` 的颜色。
   * 适配 tooltip 格式化：若 `dimension === 'model'`，柱状图汇总为多模型消耗。

---

## 3. 验证计划

### 3.1 自动化与编译测试
* 运行前端类型检查：
  ```bash
  npx tsc -b --noEmit
  ```
* 运行后端编译检查：
  ```bash
  cd src-tauri; cargo check
  ```
* 运行后端测试，确保未破坏任何既有测试用例：
  ```bash
  cd src-tauri; cargo test
  ```

### 3.2 手动测试步骤
1. 启动本地开发环境：
   ```bash
   pnpm dev
   ```
2. 访问 Token Insight 界面，点击“每日用量走势”图表的“模型维度”按钮。
3. 检查柱状图堆叠效果，确认各个模型的 Token 消耗正常叠加。
4. 将鼠标悬浮在柱状图上，检查 Tooltip 的多系列数据对齐显示是否工整美观，并检查总消耗 Token 的计算是否无误。
