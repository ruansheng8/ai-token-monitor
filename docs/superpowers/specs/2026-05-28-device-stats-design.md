# 2026-05-28 设备用量统计与配置设计方案

## 1. 目标 (Goal)
支持手动和自动配置的物理设备用量统计，并在每日用量走势图表中集成“按设备堆叠统计”的维度。

## 2. 用户反馈与设计修正 (User Feedback & Adjustments)
根据用户反馈：
* **主键不作变更**：全球唯一的会话 UUID（Cursor, Antigravity 等生成的 UUID）和 source 已经足够保证唯一性，实际运行中不会发生会话冲突。为了避免影响数据库性能和更新效率，**`sessions` 表的主键保持 `(source, uuid)` 不变**。仅在 `sessions` 表中新增 `device_name` 属性字段。
* **`turns` 表不做变更**：所有的交互轮次只与 `(source, uuid)` 会话关联，不需要存储设备名称。这保证了高频写入的 `turns` 表完全不受影响，维持原有的高性能。
* **`daily_stats` 缓存表主键调整**：大盘的预聚合缓存表 `daily_stats` 需要支持按日期、来源、设备进行汇总，因此其主键调整为 `(date, source, device_name)`。

## 3. 详细设计 (Detailed Design)

### 3.1. 数据库结构变更 (Database Migrations)
我们将编写新迁移脚本 `V2__add_device_name.sql`。

#### SQLite 迁移 (`src-tauri/migrations/sqlite/V2__add_device_name.sql`)
```sql
-- 1. 为 sessions 表添加 device_name 字段
ALTER TABLE sessions ADD COLUMN device_name TEXT DEFAULT 'unknown';

-- 2. 重构 daily_stats 缓存表以支持设备维度
DROP TABLE IF EXISTS daily_stats;

CREATE TABLE daily_stats (
    date TEXT NOT NULL,
    source TEXT NOT NULL,
    device_name TEXT NOT NULL DEFAULT 'unknown',
    input_tokens INTEGER DEFAULT 0,
    cached_input_tokens INTEGER DEFAULT 0,
    output_tokens INTEGER DEFAULT 0,
    thinking_tokens INTEGER DEFAULT 0,
    sessions_count INTEGER DEFAULT 0,
    cost_usd REAL DEFAULT 0.0,
    PRIMARY KEY (date, source, device_name)
);
```

#### PostgreSQL 迁移 (`src-tauri/migrations/postgres/V2__add_device_name.sql`)
```sql
-- 1. 为 sessions 表添加 device_name 字段
ALTER TABLE sessions ADD COLUMN device_name VARCHAR(100) DEFAULT 'unknown';

-- 2. 重构 daily_stats 缓存表以支持设备维度
DROP TABLE IF EXISTS daily_stats;

CREATE TABLE daily_stats (
    date VARCHAR(50) NOT NULL,
    source VARCHAR(50) NOT NULL,
    device_name VARCHAR(100) NOT NULL DEFAULT 'unknown',
    input_tokens BIGINT DEFAULT 0,
    cached_input_tokens BIGINT DEFAULT 0,
    output_tokens BIGINT DEFAULT 0,
    thinking_tokens BIGINT DEFAULT 0,
    sessions_count BIGINT DEFAULT 0,
    cost_usd DOUBLE PRECISION DEFAULT 0.0,
    PRIMARY KEY (date, source, device_name)
);
```

### 3.2. 后端逻辑 (Backend Logic)
1. **自动识别设备名**：
   - 尝试读取环境变量：Windows 为 `COMPUTERNAME`，Linux/macOS 为 `HOSTNAME`。
   - 若不存在，尝试读取系统用户名：`USERNAME` 或 `USER`。
   - 若均不存在，则回退为 `unknown-device`。
2. **配置文件扩展**：
   - 在 `.env` 中加入 `DEVICE_NAME` 变量。
   - 后端在启动扫描时，将同步的会话 `device_name` 统一记为当前 `.env` 中配置的值。
3. **数据预聚合缓存更新 (`rebuild_daily_stats_cache`)**：
   - 在统计 daily_stats 时，将 `s.device_name` 纳入 `GROUP BY` 条件：
     ```sql
     GROUP BY date, s.source, s.device_name
     ```
4. **大盘 API (/api/metrics)**：
   - `AggregatedMetrics` 结构体中扩展 `device_trends` 或让 `daily_trends` 支持按设备分维度输出。
   - 考虑简单高效，我们可以扩展 API 返回 `daily_device_trends` 列表，包含 `(date, device_name, tokens)`，或者直接在 `daily_trends` 中携带各设备消耗。

### 3.3. 前端交互 (Frontend Interaction)
1. **未配置弹窗 (No-Config Popup)**：
   - 在进入大盘主页后，前端通过接口 `/api/config` 检查 `device_name`。
   - 若 `device_name` 为空，弹出一个磨砂玻璃质感的对话框。
   - 对话框呈现**系统自动识别出的推荐设备名称**（例如 `LAPTOP-CEARN`）。
   - 用户可以直接点击“确认使用”，也可以修改后再保存。保存后，通过 `/api/config/save` 写入 `.env` 并重启后台生效。
2. **走势图表集成**：
   - 在大盘“每日用量走势”图表的头部，增加**维度切换**药丸按钮：【Token类型】 / 【物理设备】。
   - 【Token类型】模式：展示当前的堆叠柱状图（未缓存输入、已缓存输入、输出）及推理 Token 折线。
   - 【物理设备】模式：堆叠柱状图的每一个堆叠系列代表一台设备（如 `LAPTOP-CEARN` 消耗多少，`Home-PC` 消耗多少），直观展示不同设备之间的用量对比。

---

## 4. 验证计划 (Verification Plan)
- **数据库迁移测试**：启动程序验证 SQLite 和 Postgres 数据库中的 `device_name` 字段已成功创建，并且 `daily_stats` 表主键成功更新。
- **自动识别测试**：检查未配置时弹出的对话框是否正确回显了当前电脑的主机名（如 `LAPTOP-XXXX`）。
- **同步测试**：手动配置不同设备名称，生成会话数据，在 PostgreSQL 中验证数据按设备名区分存储和聚合。
- **图表切换测试**：在前端点击“维度切换”，验证 ECharts 图表能在“按 Token 类型”与“按设备用量”之间进行无缝、平滑切换。
