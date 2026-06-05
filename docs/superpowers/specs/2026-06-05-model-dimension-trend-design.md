# 每日用量走势增加“模型维度”统计设计规格书

本文档定义了在 Token Insight 看板的“每日用量走势”图表中引入“模型维度”统计的设计与实现规范。

## 1. 业务背景
当前每日用量走势支持“类型维度”（输入、输出、缓存、推理等）、“工具维度”（各 AI 客户端）和“设备维度”（各主机设备）的统计，但尚未支持直观的模型用量每日走势（例如 gpt-4o, claude-3-5-sonnet 等每日消耗的 Token 走势）。增加“模型维度”可以帮助用户清晰了解每天在各个模型上的用量分布与变化趋势。

## 2. 详细设计

### 2.1 后端改动 (Rust + Tauri)

#### A. 数据模型定义
在 `src-tauri/src/db.rs` 中：
* 定义 `ModelTrend` 结构体：
  ```rust
  #[derive(Serialize, Clone, Debug)]
  pub struct ModelTrend {
      pub date: String,
      pub model_name: String,
      pub tokens: i64,
      pub cost: f64,
  }
  ```
* 扩展 `AggregatedMetrics` 结构体，添加 `model_trends` 字段：
  ```rust
  #[derive(Serialize)]
  pub struct AggregatedMetrics {
      // ... 现有字段
      pub model_trends: Vec<ModelTrend>,
  }
  ```

#### B. SQLite 数据源查询 (`src-tauri/src/db.rs`)
在 `get_aggregated_metrics_from_cache` 函数中，利用 `turns` 表和 `sessions` 表的 `INNER JOIN` 实时聚合查询：
```rust
let sql_model_trends = format!(
    "SELECT 
        substr(s.created_at, 1, 10) as date,
        t.model as model_name,
        SUM(t.input_tokens + t.output_tokens) as total_tokens,
        SUM(t.cost_usd) as cost
    FROM sessions s
    INNER JOIN turns t ON s.source = t.source AND s.uuid = t.uuid
    {} {}
    GROUP BY date, model_name
    ORDER BY date ASC, model_name ASC",
    where_clause_raw,
    if where_clause_raw.is_empty() { "WHERE t.model IS NOT NULL AND t.model != 'unknown' AND t.model != ''" } else { "AND t.model IS NOT NULL AND t.model != 'unknown' AND t.model != ''" }
);
```
执行查询并将结果填充到 `model_trends` 中。

#### C. PostgreSQL 数据源查询 (`src-tauri/src/db.rs`)
在 `get_pg_aggregated_metrics` 中以相同的逻辑进行实时查询：
```rust
let sql_model_trends = format!(
    "SELECT 
        SUBSTR(s.created_at, 1, 10) as date,
        t.model as model_name,
        CAST(SUM(t.input_tokens + t.output_tokens) AS BIGINT) as total_tokens,
        SUM(t.cost_usd) as cost
    FROM sessions s
    INNER JOIN turns t ON s.source = t.source AND s.uuid = t.uuid
    {} {}
    GROUP BY date, model_name
    ORDER BY date ASC, model_name ASC",
    where_clause_raw,
    if where_clause_raw.is_empty() { "WHERE t.model IS NOT NULL AND t.model != 'unknown' AND t.model != ''" } else { "AND t.model IS NOT NULL AND t.model != 'unknown' AND t.model != ''" }
);
```
执行查询并填充。

---

### 2.2 前端改动 (React + TypeScript)

#### A. 接口与类型定义 (`src/App.tsx`)
* 新增 `ModelTrendItem` 接口：
  ```typescript
  interface ModelTrendItem {
    date: string;
    model_name: string;
    tokens: number;
    cost: number;
  }
  ```
* 在 `AggregatedMetrics` 接口中添加 `model_trends: ModelTrendItem[];`。
* 扩展 `chartDimension` 的状态类型：
  ```typescript
  const [chartDimension, setChartDimension] = useState<'type' | 'source' | 'device' | 'model'>('type');
  ```

#### B. 维度切换控制与面板渲染 (`src/App.tsx`)
* 在“每日趋势图”的切换按钮组中，新增 `🤖 模型维度` 切换按钮。
* 标题动态化：如果 `chartDimension === 'model'`，则标题显示为 `"各模型每日用量对比走势 (Token 堆叠柱状图)"`。
* 在趋势图区域适配渲染逻辑：
  ```tsx
  ) : chartDimension === 'model' ? (
    data?.model_trends && data.model_trends.length > 0 ? (
      <DailyTrendChart data={data.daily_trends} modelTrends={data.model_trends} dimension="model" theme={theme} />
    ) : (
      <div className="h-[300px] flex items-center justify-center text-text-muted italic">暂无趋势图表数据</div>
    )
  )
  ```

#### C. 图表渲染优化 (`src/components/charts/DailyTrendChart.tsx`)
* 扩展组件 Props：
  ```typescript
  interface DailyTrendChartProps {
    data: DailyTrend[];
    deviceTrends?: DeviceTrendItem[];
    modelTrends?: ModelTrendItem[];
    dimension?: 'type' | 'device' | 'model';
    theme: 'light' | 'dark';
  }
  ```
* 聚合“前 8 模型 + 其他”的算法逻辑：
  1. 累加计算当前数据集中各模型的总 Token 消耗。
  2. 对模型进行降序排序，筛选出前 8 个模型。
  3. 对于其他模型，在按天映射时，将其模型名称统一映射为 `"其他 (Others)"`。
  4. 使用全局统一的八色调色板进行图表颜色渲染，保持整体视觉效果的精致感。

---

## 3. 验证方案

### 3.1 编译验证
* 执行前端类型检查：`npx tsc -b --noEmit`
* 执行后端编译检查：`cd src-tauri; cargo check`

### 3.2 运行验证
1. 打开 Token Insight 看板。
2. 观察“每日用量走势”卡片，确认出现“模型维度”按钮。
3. 点击“模型维度”，柱状图应流畅切换，正确展示前 8 个模型加“其他 (Others)”的堆叠用量。
4. 悬浮在图表柱状块上，确认 Tooltip 的多模型明细对齐良好，总消耗 Token 计算正确。
