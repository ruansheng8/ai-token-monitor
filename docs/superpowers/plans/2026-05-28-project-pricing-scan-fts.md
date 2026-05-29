# 项目归属统计、模型费率、自适应扫描与 FTS 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 AI Token Monitor 增加项目维度消耗大盘、可编辑模型费率与多币种换算、自适应后台扫描频率，以及基于 SQLite FTS5 的会话检索提速。

**Architecture:** 沿用当前“Rust 后端聚合 + Axum JSON API + React 单页大盘”的结构，把新增数据能力尽量收敛到 `src-tauri/src/db.rs`，把 HTTP 暴露收敛到 `src-tauri/src/server.rs`。为避免每次查询都对 `project_path` 做字符串切割，本计划在 `sessions` 中增加一次性派生列 `project_name`，并新增 `project_daily_stats`、`model_pricing`、`exchange_rates` 等缓存/配置表；前端只消费新的聚合字段，不直接拼装复杂统计逻辑。

**Tech Stack:** Rust (`rusqlite`, `postgres`, `axum`, `tokio`, `sysinfo`, `reqwest`), SQLite FTS5, PostgreSQL migrations (`refinery`), React 19, TypeScript, ECharts, Tauri。

---

## 范围说明

这份需求实际覆盖 4 个相对独立的子系统：

1. 项目维度统计与排行榜
2. 模型费率与汇率管理
3. 后台自适应热同步
4. SQLite FTS5 搜索提速

推荐最终执行时按任务批次推进，而不是一次性改完全部逻辑；但为了便于一次评审，这里先给出统一总计划，并且把任务边界切成可单独提交的增量。

## 文件结构与职责

### 后端

- Modify: `src-tauri/src/db.rs`
  - 继续作为 SQLite / PostgreSQL 双栈数据访问层。
  - 新增表结构初始化：`project_name`、`sessions_fts`、`project_daily_stats`、`model_pricing`、`exchange_rates`。
  - 新增项目聚合查询、项目缓存重建、FTS 搜索、模型费率查找、汇率读取、自适应扫描策略。
  - 增加对应 Rust 单元测试。

- Modify: `src-tauri/src/server.rs`
  - 暴露新的配置与数据接口：模型费率列表/保存、汇率刷新、扩展 metrics/config 响应。
  - 保持现有 `/api/metrics`、`/api/sessions`、`/api/config` 风格一致。

- Modify: `src-tauri/src/main.rs`
  - 把固定 `HOTSYNC_DEBOUNCE_MS` 改成动态读取推荐扫描延迟。
  - 在 watcher 触发时根据当前系统负载决定延长或恢复热同步延迟。

- Modify: `src-tauri/src/db_adapter.rs`
  - 保持 PostgreSQL 初始化入口，确保新 migration 会在配置测试/连接生效时被执行。

- Create: `src-tauri/migrations/postgres/V3__project_pricing_tables.sql`
  - 给 PostgreSQL 加 `project_name`、`project_daily_stats`、`model_pricing`、`exchange_rates` 结构与索引。
  - PostgreSQL 不做 FTS5；保留当前分页搜索逻辑。

### 前端

- Modify: `src/App.tsx`
  - 扩展 `AggregatedMetrics` 类型。
  - 新增项目维度图表和排行榜区块。
  - 在现有设置弹窗中增加“模型费率管理”和“显示币种/汇率刷新”交互。

- Create: `src/components/charts/ProjectTrendChart.tsx`
  - 专门渲染“按项目每日 Token / 成本走势”折线图。
  - 与当前 `SourceTrendChart`、`DailyTrendChart` 的主题切换方式保持一致。

## 数据结构约定

### 1. `sessions` 新增派生列

给 `sessions` 新增 `project_name`，避免运行时重复解析路径：

```rust
#[derive(Serialize)]
pub struct ProjectRanking {
    pub project_name: String,
    pub project_path: String,
    pub total_tokens: i64,
    pub total_cost_usd: f64,
    pub sessions_count: i64,
}

#[derive(Serialize)]
pub struct ProjectTrend {
    pub date: String,
    pub project_name: String,
    pub tokens: i64,
    pub cost_usd: f64,
}
```

### 2. 模型费率与汇率表

```sql
CREATE TABLE IF NOT EXISTS model_pricing (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    model_pattern TEXT NOT NULL UNIQUE,
    input_price_per_million REAL NOT NULL,
    cached_input_price_per_million REAL NOT NULL,
    output_price_per_million REAL NOT NULL,
    priority INTEGER NOT NULL DEFAULT 100,
    enabled INTEGER NOT NULL DEFAULT 1,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS exchange_rates (
    currency_code TEXT PRIMARY KEY,
    rate_from_usd REAL NOT NULL,
    updated_at TEXT NOT NULL
);
```

### 3. metrics 返回体扩展

```ts
interface ProjectTrendItem {
  date: string;
  project_name: string;
  tokens: number;
  cost_usd: number;
}

interface ProjectRankingItem {
  project_name: string;
  project_path: string;
  total_tokens: number;
  total_cost_usd: number;
  sessions_count: number;
}

interface PricingConfigResponse {
  display_currency: string;
  usd_exchange_rate: number;
  exchange_rate_updated_at: string;
}
```

---

### Task 1: 扩展数据库结构并为项目维度做准备

**Files:**
- Modify: `src-tauri/src/db.rs`
- Create: `src-tauri/migrations/postgres/V3__project_pricing_tables.sql`
- Test: `src-tauri/src/db.rs`

- [ ] **Step 1: 先写失败测试，约束 SQLite 初始化后的新表和新列**

```rust
#[test]
fn test_init_cache_db_creates_project_and_pricing_structures() {
    let test_id = chrono::Utc::now().timestamp_millis();
    let temp_path = std::env::temp_dir().join(format!("ai_token_monitor_schema_test_{}", test_id));
    std::fs::create_dir_all(&temp_path).unwrap();
    std::env::set_var("USERPROFILE", temp_path.to_str().unwrap());
    std::env::set_var("DATABASE_TYPE", "sqlite");

    init_cache_db().unwrap();

    let conn = rusqlite::Connection::open(get_db_cache_path()).unwrap();

    let session_columns: Vec<String> = conn
        .prepare("PRAGMA table_info(sessions)")
        .unwrap()
        .query_map([], |row| row.get(1))
        .unwrap()
        .map(|row| row.unwrap())
        .collect();

    assert!(session_columns.contains(&"project_name".to_string()));

    let fts_exists: i64 = conn.query_row(
        "SELECT COUNT(1) FROM sqlite_master WHERE type='table' AND name='sessions_fts'",
        [],
        |row| row.get(0),
    ).unwrap();
    assert_eq!(fts_exists, 1);

    let pricing_exists: i64 = conn.query_row(
        "SELECT COUNT(1) FROM sqlite_master WHERE type='table' AND name='model_pricing'",
        [],
        |row| row.get(0),
    ).unwrap();
    assert_eq!(pricing_exists, 1);

    let project_stats_exists: i64 = conn.query_row(
        "SELECT COUNT(1) FROM sqlite_master WHERE type='table' AND name='project_daily_stats'",
        [],
        |row| row.get(0),
    ).unwrap();
    assert_eq!(project_stats_exists, 1);
}
```

- [ ] **Step 2: 运行测试，确认当前实现会失败**

Run: `cargo test test_init_cache_db_creates_project_and_pricing_structures -- --exact`

Expected: FAIL，提示 `project_name`/`sessions_fts`/`model_pricing`/`project_daily_stats` 尚不存在。

- [ ] **Step 3: 在 SQLite 初始化中加入列升级、表创建与默认费率灌种**

把下面代码补到 `src-tauri/src/db.rs` 的 `init_cache_db()` 和同文件辅助函数中：

```rust
fn detect_project_name(project_path: Option<&str>) -> String {
    project_path
        .and_then(|path| std::path::Path::new(path).file_name())
        .and_then(|name| name.to_str())
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "unknown-project".to_string())
}

fn seed_default_model_pricing(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    let defaults = [
        ("*opus*", 15.0_f64, 1.5_f64, 75.0_f64, 10_i64),
        ("*sonnet*", 3.0_f64, 0.3_f64, 15.0_f64, 20_i64),
        ("*haiku*", 0.25_f64, 0.03_f64, 1.25_f64, 30_i64),
        ("*gemini*pro*", 1.25_f64, 0.3125_f64, 5.0_f64, 40_i64),
        ("*gemini*flash*", 0.075_f64, 0.01875_f64, 0.3_f64, 50_i64),
        ("*", 2.5_f64, 0.25_f64, 10.0_f64, 999_i64),
    ];

    for (pattern, input, cached, output, priority) in defaults {
        conn.execute(
            "INSERT INTO model_pricing (
                model_pattern,
                input_price_per_million,
                cached_input_price_per_million,
                output_price_per_million,
                priority,
                enabled,
                updated_at
            ) VALUES (?, ?, ?, ?, ?, 1, ?)
            ON CONFLICT(model_pattern) DO NOTHING",
            rusqlite::params![pattern, input, cached, output, priority, now],
        )?;
    }

    Ok(())
}
```

```rust
let mut stmt = conn.prepare("PRAGMA table_info(sessions)")?;
let mut rows = stmt.query([])?;
let mut has_project_name = false;
while let Some(row) = rows.next()? {
    let name: String = row.get(1)?;
    if name == "project_name" {
        has_project_name = true;
        break;
    }
}
if !has_project_name {
    let _ = conn.execute("ALTER TABLE sessions ADD COLUMN project_name TEXT DEFAULT 'unknown-project';", []);
    let _ = conn.execute(
        "UPDATE sessions
         SET project_name = COALESCE(NULLIF(
            CASE
              WHEN project_path IS NULL OR trim(project_path) = '' THEN 'unknown-project'
              ELSE replace(project_path, '\\', '/')
            END,
            ''
         ), 'unknown-project')",
        [],
    );
}

conn.execute(
    "CREATE VIRTUAL TABLE IF NOT EXISTS sessions_fts USING fts5(
        source,
        uuid,
        title,
        project_name,
        tokenize='unicode61 remove_diacritics 2'
    )",
    [],
)?;

conn.execute(
    "CREATE TABLE IF NOT EXISTS project_daily_stats (
        date TEXT NOT NULL,
        project_name TEXT NOT NULL,
        total_tokens INTEGER DEFAULT 0,
        total_cost_usd REAL DEFAULT 0.0,
        sessions_count INTEGER DEFAULT 0,
        PRIMARY KEY (date, project_name)
    )",
    [],
)?;

conn.execute(
    "CREATE TABLE IF NOT EXISTS model_pricing (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        model_pattern TEXT NOT NULL UNIQUE,
        input_price_per_million REAL NOT NULL,
        cached_input_price_per_million REAL NOT NULL,
        output_price_per_million REAL NOT NULL,
        priority INTEGER NOT NULL DEFAULT 100,
        enabled INTEGER NOT NULL DEFAULT 1,
        updated_at TEXT NOT NULL
    )",
    [],
)?;

conn.execute(
    "CREATE TABLE IF NOT EXISTS exchange_rates (
        currency_code TEXT PRIMARY KEY,
        rate_from_usd REAL NOT NULL,
        updated_at TEXT NOT NULL
    )",
    [],
)?;

seed_default_model_pricing(&conn)?;
```

- [ ] **Step 4: 为 PostgreSQL 加迁移脚本，保持双库结构一致**

创建 `src-tauri/migrations/postgres/V3__project_pricing_tables.sql`：

```sql
ALTER TABLE sessions ADD COLUMN IF NOT EXISTS project_name VARCHAR(255) DEFAULT 'unknown-project';

UPDATE sessions
SET project_name = COALESCE(NULLIF(project_name, ''), 'unknown-project')
WHERE project_name IS NULL OR btrim(project_name) = '';

CREATE TABLE IF NOT EXISTS project_daily_stats (
    date VARCHAR(50) NOT NULL,
    project_name VARCHAR(255) NOT NULL,
    total_tokens BIGINT DEFAULT 0,
    total_cost_usd DOUBLE PRECISION DEFAULT 0.0,
    sessions_count BIGINT DEFAULT 0,
    PRIMARY KEY (date, project_name)
);

CREATE TABLE IF NOT EXISTS model_pricing (
    id BIGSERIAL PRIMARY KEY,
    model_pattern VARCHAR(255) NOT NULL UNIQUE,
    input_price_per_million DOUBLE PRECISION NOT NULL,
    cached_input_price_per_million DOUBLE PRECISION NOT NULL,
    output_price_per_million DOUBLE PRECISION NOT NULL,
    priority BIGINT NOT NULL DEFAULT 100,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    updated_at VARCHAR(50) NOT NULL
);

CREATE TABLE IF NOT EXISTS exchange_rates (
    currency_code VARCHAR(16) PRIMARY KEY,
    rate_from_usd DOUBLE PRECISION NOT NULL,
    updated_at VARCHAR(50) NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_project_daily_stats_date ON project_daily_stats(date);
CREATE INDEX IF NOT EXISTS idx_sessions_project_name ON sessions(project_name);
```

- [ ] **Step 5: 回跑测试并提交**

Run: `cargo test test_init_cache_db_creates_project_and_pricing_structures -- --exact`

Expected: PASS

```bash
git add src-tauri/src/db.rs src-tauri/migrations/postgres/V3__project_pricing_tables.sql
git commit -m "feat(db): add project and pricing schema"
```

### Task 2: 让写入链路维护 `project_name`、FTS 和项目日聚合缓存

**Files:**
- Modify: `src-tauri/src/db.rs`
- Test: `src-tauri/src/db.rs`

- [ ] **Step 1: 先写失败测试，约束同步后 `project_name`、FTS、项目日缓存都被填充**

```rust
#[test]
fn test_sync_populates_project_name_fts_and_project_daily_stats() {
    let test_id = chrono::Utc::now().timestamp_millis();
    let temp_path = std::env::temp_dir().join(format!("ai_token_monitor_project_cache_test_{}", test_id));
    std::fs::create_dir_all(&temp_path).unwrap();

    std::env::set_var("USERPROFILE", temp_path.to_str().unwrap());
    std::env::set_var("DATABASE_TYPE", "sqlite");

    init_cache_db().unwrap();

    let claude_proj_dir = get_claude_projects_dir().join("demo-repo");
    std::fs::create_dir_all(&claude_proj_dir).unwrap();
    let log_file = claude_proj_dir.join("history.jsonl");
    let mut file = std::fs::File::create(&log_file).unwrap();

    let line = serde_json::json!({
        "timestamp": "2026-05-28T09:00:00.000Z",
        "model": "claude-3-5-sonnet",
        "message": {
            "id": "msg_project_1",
            "usage": {
                "input_tokens": 120,
                "output_tokens": 60,
                "cache_read_input_tokens": 20
            }
        },
        "requestId": "req_project_1"
    });
    writeln!(file, "{}", line).unwrap();
    drop(file);

    let mut conn = rusqlite::Connection::open(get_db_cache_path()).unwrap();
    sync_claude_code(&mut conn, 0, 1, &|_, _| {}).unwrap();
    rebuild_daily_stats_cache(&conn).unwrap();
    rebuild_project_daily_stats_cache(&conn).unwrap();
    rebuild_sessions_fts(&conn).unwrap();

    let project_name: String = conn.query_row(
        "SELECT project_name FROM sessions WHERE source = 'claude_code' LIMIT 1",
        [],
        |row| row.get(0),
    ).unwrap();
    assert_eq!(project_name, "demo-repo");

    let fts_hits: i64 = conn.query_row(
        "SELECT COUNT(1) FROM sessions_fts WHERE sessions_fts MATCH 'demo-repo'",
        [],
        |row| row.get(0),
    ).unwrap();
    assert_eq!(fts_hits, 1);

    let project_tokens: i64 = conn.query_row(
        "SELECT total_tokens FROM project_daily_stats WHERE project_name = 'demo-repo' LIMIT 1",
        [],
        |row| row.get(0),
    ).unwrap();
    assert_eq!(project_tokens, 180);
}
```

- [ ] **Step 2: 运行测试，确认当前链路还不会维护这些缓存**

Run: `cargo test test_sync_populates_project_name_fts_and_project_daily_stats -- --exact`

Expected: FAIL，提示 `project_name` 仍为默认值或项目缓存表为空。

- [ ] **Step 3: 给所有 `sessions` upsert 路径补 `project_name`，并新增缓存重建函数**

在 `src-tauri/src/db.rs` 增加：

```rust
fn rebuild_sessions_fts(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    conn.execute("DELETE FROM sessions_fts", [])?;
    conn.execute(
        "INSERT INTO sessions_fts (source, uuid, title, project_name)
         SELECT source, uuid, COALESCE(title, ''), COALESCE(project_name, 'unknown-project')
         FROM sessions",
        [],
    )?;
    Ok(())
}

fn rebuild_project_daily_stats_cache(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    conn.execute("DELETE FROM project_daily_stats", [])?;
    conn.execute(
        "INSERT INTO project_daily_stats (date, project_name, total_tokens, total_cost_usd, sessions_count)
         SELECT
            SUBSTR(s.created_at, 1, 10) AS date,
            COALESCE(NULLIF(s.project_name, ''), 'unknown-project') AS project_name,
            COALESCE(SUM(t.input_tokens + t.output_tokens), 0) AS total_tokens,
            COALESCE(SUM(t.cost_usd), 0.0) AS total_cost_usd,
            COUNT(DISTINCT s.source || ':' || s.uuid) AS sessions_count
         FROM sessions s
         LEFT JOIN turns t ON s.source = t.source AND s.uuid = t.uuid
         GROUP BY SUBSTR(s.created_at, 1, 10), COALESCE(NULLIF(s.project_name, ''), 'unknown-project')",
        [],
    )?;
    Ok(())
}
```

把现有所有 session upsert 语句统一改成：

```rust
let project_name = detect_project_name(Some(&project_path));
conn_cache.execute(
    "INSERT INTO sessions (
        source, uuid, title, created_at, last_parsed_idx, last_mtime, project_path, project_name, device_name
     ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
     ON CONFLICT(source, uuid) DO UPDATE SET
        title = excluded.title,
        created_at = excluded.created_at,
        last_parsed_idx = excluded.last_parsed_idx,
        last_mtime = excluded.last_mtime,
        project_path = excluded.project_path,
        project_name = excluded.project_name,
        device_name = excluded.device_name",
    rusqlite::params![
        source,
        uuid,
        title,
        created_at,
        last_parsed_idx,
        last_mtime,
        project_path,
        project_name,
        dev_name,
    ],
)?;
```

- [ ] **Step 4: 在扫描收尾和设备名回刷后同步重建两个新缓存**

把 `sync_cache_db_with_progress()` 和 `update_device_name_in_db()` 末尾补齐：

```rust
if let Err(e) = rebuild_project_daily_stats_cache(&conn_cache) {
    eprintln!("[项目缓存] 重建 project_daily_stats 失败: {}", e);
}
if let Err(e) = rebuild_sessions_fts(&conn_cache) {
    eprintln!("[FTS] 重建 sessions_fts 失败: {}", e);
}
```

以及 SQLite 设备名更新后：

```rust
let _ = rebuild_daily_stats_cache(&conn);
let _ = rebuild_project_daily_stats_cache(&conn);
let _ = rebuild_sessions_fts(&conn);
```

- [ ] **Step 5: 回跑测试并提交**

Run: `cargo test test_sync_populates_project_name_fts_and_project_daily_stats -- --exact`

Expected: PASS

```bash
git add src-tauri/src/db.rs
git commit -m "feat(sync): maintain project and fts caches"
```

### Task 3: 用表驱动费率替代硬编码 `estimate_cost`

**Files:**
- Modify: `src-tauri/src/db.rs`
- Test: `src-tauri/src/db.rs`

- [ ] **Step 1: 先写失败测试，锁定“优先匹配费率表”的行为**

```rust
#[test]
fn test_estimate_cost_prefers_model_pricing_table() {
    let test_id = chrono::Utc::now().timestamp_millis();
    let temp_path = std::env::temp_dir().join(format!("ai_token_monitor_pricing_test_{}", test_id));
    std::fs::create_dir_all(&temp_path).unwrap();
    std::env::set_var("USERPROFILE", temp_path.to_str().unwrap());
    std::env::set_var("DATABASE_TYPE", "sqlite");

    init_cache_db().unwrap();
    let conn = rusqlite::Connection::open(get_db_cache_path()).unwrap();

    conn.execute("DELETE FROM model_pricing", []).unwrap();
    conn.execute(
        "INSERT INTO model_pricing (
            model_pattern,
            input_price_per_million,
            cached_input_price_per_million,
            output_price_per_million,
            priority,
            enabled,
            updated_at
        ) VALUES ('*custom-sonnet*', 9.0, 0.9, 18.0, 1, 1, ?1)",
        [chrono::Utc::now().to_rfc3339()],
    ).unwrap();

    let cost = estimate_cost("custom-sonnet-2026", 1_000_000, 200_000, 500_000).unwrap();
    assert!((cost - 16.2).abs() < 1e-6);
}
```

- [ ] **Step 2: 运行测试，确认现有 `estimate_cost` 还是写死分支**

Run: `cargo test test_estimate_cost_prefers_model_pricing_table -- --exact`

Expected: FAIL，因为当前 `estimate_cost` 返回固定 Sonnet 价格。

- [ ] **Step 3: 把费率模型抽成数据库查询 + 通配符匹配**

在 `src-tauri/src/db.rs` 中把 `estimate_cost` 改为返回 `Result<f64, rusqlite::Error>`，并新增：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPricingRow {
    pub id: Option<i64>,
    pub model_pattern: String,
    pub input_price_per_million: f64,
    pub cached_input_price_per_million: f64,
    pub output_price_per_million: f64,
    pub priority: i64,
    pub enabled: bool,
    pub updated_at: String,
}

fn glob_match(pattern: &str, model: &str) -> bool {
    let regex = regex::Regex::new(
        &format!("^{}$", regex::escape(pattern).replace("\\*", ".*"))
    ).unwrap();
    regex.is_match(&model.to_lowercase())
}

fn load_model_pricing(conn: &rusqlite::Connection) -> Result<Vec<ModelPricingRow>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, model_pattern, input_price_per_million, cached_input_price_per_million,
                output_price_per_million, priority, enabled, updated_at
         FROM model_pricing
         WHERE enabled = 1
         ORDER BY priority ASC, id ASC"
    )?;

    stmt.query_map([], |row| {
        Ok(ModelPricingRow {
            id: row.get(0)?,
            model_pattern: row.get(1)?,
            input_price_per_million: row.get(2)?,
            cached_input_price_per_million: row.get(3)?,
            output_price_per_million: row.get(4)?,
            priority: row.get(5)?,
            enabled: row.get::<_, i64>(6)? == 1,
            updated_at: row.get(7)?,
        })
    })?
    .collect()
}

pub fn estimate_cost(model: &str, input: i64, cached: i64, output: i64) -> Result<f64, rusqlite::Error> {
    let conn = rusqlite::Connection::open(get_db_cache_path())?;
    let pricing = load_model_pricing(&conn)?;
    let model_lower = model.to_lowercase();
    let matched = pricing
        .into_iter()
        .find(|row| glob_match(&row.model_pattern.to_lowercase(), &model_lower));

    let row = matched.unwrap_or(ModelPricingRow {
        id: None,
        model_pattern: "*".to_string(),
        input_price_per_million: 2.5,
        cached_input_price_per_million: 0.25,
        output_price_per_million: 10.0,
        priority: 999,
        enabled: true,
        updated_at: chrono::Utc::now().to_rfc3339(),
    });

    let uncached = (input - cached).max(0) as f64;
    Ok((
        uncached * row.input_price_per_million
        + (cached as f64) * row.cached_input_price_per_million
        + (output as f64) * row.output_price_per_million
    ) / 1_000_000.0)
}
```

- [ ] **Step 4: 顺手修正所有调用点，让编译器帮忙兜底**

把所有：

```rust
let cost = estimate_cost(&model, total_input, cache_read, output);
```

改成：

```rust
let cost = estimate_cost(&model, total_input, cache_read, output).unwrap_or(0.0);
```

测试代码同步改成：

```rust
let cost_opus = estimate_cost("claude-3-opus", 1000, 200, 500).unwrap();
```

- [ ] **Step 5: 回跑测试并提交**

Run: `cargo test test_estimate_cost test_estimate_cost_prefers_model_pricing_table -- --nocapture`

Expected: PASS

```bash
git add src-tauri/src/db.rs
git commit -m "feat(pricing): make cost estimation table-driven"
```

### Task 4: 增加项目聚合查询与多币种展示字段

**Files:**
- Modify: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/server.rs`
- Test: `src-tauri/src/db.rs`

- [ ] **Step 1: 先写失败测试，约束 metrics 必须返回项目排行和趋势**

```rust
#[test]
fn test_aggregated_metrics_include_project_rankings_and_trends() {
    let test_id = chrono::Utc::now().timestamp_millis();
    let temp_path = std::env::temp_dir().join(format!("ai_token_monitor_metrics_project_test_{}", test_id));
    std::fs::create_dir_all(&temp_path).unwrap();
    std::env::set_var("USERPROFILE", temp_path.to_str().unwrap());
    std::env::set_var("DATABASE_TYPE", "sqlite");

    init_cache_db().unwrap();
    let conn = rusqlite::Connection::open(get_db_cache_path()).unwrap();

    conn.execute(
        "INSERT INTO sessions (source, uuid, title, created_at, project_path, project_name, device_name)
         VALUES ('claude_code', 's1', 'A', '2026-05-28T10:00:00.000Z', 'D:/code/repo-a', 'repo-a', 'devbox')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO turns (source, uuid, idx, model, input_tokens, cached_input_tokens, output_tokens, cost_usd)
         VALUES ('claude_code', 's1', 0, 'claude-3-5-sonnet', 100, 20, 50, 0.01)",
        [],
    ).unwrap();

    rebuild_daily_stats_cache(&conn).unwrap();
    rebuild_project_daily_stats_cache(&conn).unwrap();

    let metrics = get_aggregated_metrics_from_cache(None, None, None).unwrap();
    assert_eq!(metrics.project_rankings.len(), 1);
    assert_eq!(metrics.project_rankings[0].project_name, "repo-a");
    assert_eq!(metrics.project_trends.len(), 1);
    assert_eq!(metrics.project_trends[0].project_name, "repo-a");
}
```

- [ ] **Step 2: 运行测试，确认返回结构尚未包含项目数据**

Run: `cargo test test_aggregated_metrics_include_project_rankings_and_trends -- --exact`

Expected: FAIL，提示 `AggregatedMetrics` 不含 `project_rankings` / `project_trends` 字段。

- [ ] **Step 3: 扩展 Rust 返回体并从 `project_daily_stats` 聚合项目数据**

在 `src-tauri/src/db.rs` 中扩展结构：

```rust
#[derive(Serialize)]
pub struct ProjectTrend {
    pub date: String,
    pub project_name: String,
    pub tokens: i64,
    pub cost_usd: f64,
}

#[derive(Serialize)]
pub struct ProjectRanking {
    pub project_name: String,
    pub project_path: String,
    pub total_tokens: i64,
    pub total_cost_usd: f64,
    pub sessions_count: i64,
}

#[derive(Serialize)]
pub struct AggregatedMetrics {
    pub totals: Totals,
    pub daily_trends: Vec<DailyTrend>,
    pub monthly_summary: Vec<MonthlySummary>,
    pub model_distribution: Vec<ModelDistribution>,
    pub sessions: Vec<SessionItem>,
    pub source_trends: Vec<SourceTrend>,
    pub device_trends: Vec<DeviceTrend>,
    pub project_trends: Vec<ProjectTrend>,
    pub project_rankings: Vec<ProjectRanking>,
    pub model_performance: Vec<ModelPerformance>,
    pub performance_trends: Vec<PerformanceTrend>,
    pub display_currency: String,
    pub usd_exchange_rate: f64,
    pub exchange_rate_updated_at: String,
}
```

追加 SQLite 聚合查询：

```rust
let mut project_trends = Vec::new();
let sql_project_trends = format!(
    "SELECT date, project_name, total_tokens, total_cost_usd
     FROM project_daily_stats
     {}
     ORDER BY date ASC, project_name ASC",
    where_clause_cache
);
let mut stmt_project_trends = conn.prepare(&sql_project_trends)?;
for row in stmt_project_trends.query_map(rusqlite::params_from_iter(params_cache.clone()), |row| {
    Ok(ProjectTrend {
        date: row.get(0)?,
        project_name: row.get(1)?,
        tokens: row.get(2)?,
        cost_usd: row.get(3)?,
    })
})? {
    project_trends.push(row?);
}

let sql_project_rankings = format!(
    "SELECT
        COALESCE(NULLIF(s.project_name, ''), 'unknown-project') AS project_name,
        COALESCE(MAX(s.project_path), '') AS project_path,
        COALESCE(SUM(t.input_tokens + t.output_tokens), 0) AS total_tokens,
        COALESCE(SUM(t.cost_usd), 0.0) AS total_cost_usd,
        COUNT(DISTINCT s.source || ':' || s.uuid) AS sessions_count
     FROM sessions s
     LEFT JOIN turns t ON s.source = t.source AND s.uuid = t.uuid
     {}
     GROUP BY COALESCE(NULLIF(s.project_name, ''), 'unknown-project')
     ORDER BY total_tokens DESC, total_cost_usd DESC
     LIMIT 10",
    where_clause_raw
);
```

- [ ] **Step 4: 把汇率展示字段也一起塞进 metrics，方便前端统一换算**

在 `src-tauri/src/db.rs` 增加：

```rust
fn get_display_currency() -> String {
    std::env::var("DISPLAY_CURRENCY")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "USD".to_string())
        .to_uppercase()
}

fn get_exchange_rate(conn: &rusqlite::Connection, currency: &str) -> Result<(f64, String), rusqlite::Error> {
    if currency.eq_ignore_ascii_case("USD") {
        return Ok((1.0, "system-default".to_string()));
    }

    let mut stmt = conn.prepare(
        "SELECT rate_from_usd, updated_at FROM exchange_rates WHERE currency_code = ?"
    )?;
    let result = stmt.query_row([currency], |row| Ok((row.get(0)?, row.get(1)?)));
    Ok(result.unwrap_or((1.0, "missing-rate".to_string())))
}
```

返回值拼装时追加：

```rust
let display_currency = get_display_currency();
let (usd_exchange_rate, exchange_rate_updated_at) = get_exchange_rate(&conn, &display_currency)?;
```

- [ ] **Step 5: 回跑测试并提交**

Run: `cargo test test_aggregated_metrics_include_project_rankings_and_trends -- --exact`

Expected: PASS

```bash
git add src-tauri/src/db.rs src-tauri/src/server.rs
git commit -m "feat(metrics): expose project and currency data"
```

### Task 5: 用 FTS5 替换 SQLite `LIKE` 搜索

**Files:**
- Modify: `src-tauri/src/db.rs`
- Test: `src-tauri/src/db.rs`

- [ ] **Step 1: 先写失败测试，验证 SQLite 搜索命中标题与项目名**

```rust
#[test]
fn test_sqlite_session_search_uses_fts() {
    let test_id = chrono::Utc::now().timestamp_millis();
    let temp_path = std::env::temp_dir().join(format!("ai_token_monitor_fts_search_test_{}", test_id));
    std::fs::create_dir_all(&temp_path).unwrap();
    std::env::set_var("USERPROFILE", temp_path.to_str().unwrap());
    std::env::set_var("DATABASE_TYPE", "sqlite");

    init_cache_db().unwrap();
    let conn = rusqlite::Connection::open(get_db_cache_path()).unwrap();

    conn.execute(
        "INSERT INTO sessions (source, uuid, title, created_at, project_path, project_name, device_name)
         VALUES ('claude_code', 'fts-1', 'Refactor token monitor', '2026-05-28T11:00:00.000Z', 'D:/code/ai-token-monitor', 'ai-token-monitor', 'devbox')",
        [],
    ).unwrap();
    rebuild_sessions_fts(&conn).unwrap();

    let result = get_sessions_paginated(1, 10, Some("monitor"), Some("all"), Some("created_at"), Some("desc"), None, None, false).unwrap();
    assert_eq!(result.total, 1);
    assert_eq!(result.items[0].uuid, "fts-1");
}
```

- [ ] **Step 2: 运行测试，确认当前 `LIKE` 实现还没有走 FTS**

Run: `cargo test test_sqlite_session_search_uses_fts -- --exact`

Expected: FAIL，或只能命中标题不能命中项目名。

- [ ] **Step 3: 只在 SQLite 分支切到 `MATCH`，PostgreSQL 维持现状**

把 `get_sessions_paginated()` 中的搜索条件替换为：

```rust
if let Some(ref kw) = search {
    let kw_trimmed = kw.trim();
    if !kw_trimmed.is_empty() {
        conditions.push(
            "EXISTS (
                SELECT 1
                FROM sessions_fts f
                WHERE f.source = s.source
                  AND f.uuid = s.uuid
                  AND sessions_fts MATCH ?
            )"
        );
        let escaped = kw_trimmed.replace('"', " ");
        params.push(rusqlite::types::Value::Text(format!("\"{}\"*", escaped)));
    }
}
```

保留 PostgreSQL 的 `LIKE` 逻辑，不在这次需求里引入 PG 全文检索，避免扩大范围。

- [ ] **Step 4: 补一个重建入口，保证旧库升级后也能拿到 FTS 内容**

在 `init_cache_db()` 末尾调用：

```rust
let _ = rebuild_sessions_fts(&conn);
```

- [ ] **Step 5: 回跑测试并提交**

Run: `cargo test test_sqlite_session_search_uses_fts -- --exact`

Expected: PASS

```bash
git add src-tauri/src/db.rs
git commit -m "feat(search): switch sqlite session search to fts5"
```

### Task 6: 实现自适应热同步延迟策略

**Files:**
- Modify: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/Cargo.toml`
- Test: `src-tauri/src/db.rs`

- [ ] **Step 1: 先写失败测试，锁定负载到延迟的决策规则**

```rust
#[test]
fn test_recommended_hot_sync_delay_changes_with_load() {
    let idle = recommend_hot_sync_delay_ms(12.0, 8_000_000.0);
    let busy_cpu = recommend_hot_sync_delay_ms(92.0, 8_000_000.0);
    let busy_disk = recommend_hot_sync_delay_ms(40.0, 180_000_000.0);

    assert_eq!(idle.delay_ms, 5_000);
    assert_eq!(busy_cpu.delay_ms, 60_000);
    assert_eq!(busy_disk.delay_ms, 60_000);
}
```

- [ ] **Step 2: 运行测试，确认目前没有负载决策函数**

Run: `cargo test test_recommended_hot_sync_delay_changes_with_load -- --exact`

Expected: FAIL，提示 `recommend_hot_sync_delay_ms` 不存在。

- [ ] **Step 3: 加依赖与纯函数，把复杂性锁在可测试逻辑里**

修改 `src-tauri/Cargo.toml`：

```toml
sysinfo = "0.33"
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
```

在 `src-tauri/src/db.rs` 增加：

```rust
#[derive(Clone, Serialize)]
pub struct HotSyncPolicy {
    pub delay_ms: u64,
    pub reason: String,
}

pub fn recommend_hot_sync_delay_ms(cpu_usage: f32, disk_write_bytes_per_sec: f64) -> HotSyncPolicy {
    if cpu_usage >= 85.0 {
        return HotSyncPolicy {
            delay_ms: 60_000,
            reason: format!("CPU {:.1}% 偏高，延长热同步防抖", cpu_usage),
        };
    }

    if disk_write_bytes_per_sec >= 120_000_000.0 {
        return HotSyncPolicy {
            delay_ms: 60_000,
            reason: format!("磁盘写入 {:.0} B/s 偏高，延长热同步防抖", disk_write_bytes_per_sec),
        };
    }

    HotSyncPolicy {
        delay_ms: std::env::var("HOTSYNC_DEBOUNCE_MS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(5_000),
        reason: "系统负载正常，使用默认热同步防抖".to_string(),
    }
}
```

- [ ] **Step 4: 在 watcher 中改成动态取值，而不是固定环境变量**

在 `src-tauri/src/db.rs` 增加运行时采样函数：

```rust
pub fn current_hot_sync_policy() -> HotSyncPolicy {
    let mut system = sysinfo::System::new_all();
    system.refresh_cpu_usage();
    std::thread::sleep(std::time::Duration::from_millis(200));
    system.refresh_cpu_usage();

    let cpu_usage = system.global_cpu_usage();
    let disk_write_bytes_per_sec = 0.0_f64;

    recommend_hot_sync_delay_ms(cpu_usage, disk_write_bytes_per_sec)
}
```

把 `src-tauri/src/main.rs` 的固定读取替换成：

```rust
let policy = db::current_hot_sync_policy();
let debounce_ms = policy.delay_ms;
debounce_timer = Some(
    tokio::time::Instant::now() + tokio::time::Duration::from_millis(debounce_ms)
);
```

并在真正触发热同步前再判一次：

```rust
let policy = db::current_hot_sync_policy();
if policy.delay_ms >= 60_000 {
    debounce_timer = Some(
        tokio::time::Instant::now() + tokio::time::Duration::from_millis(policy.delay_ms)
    );
} else {
    db::start_background_scan(true);
}
```

- [ ] **Step 5: 回跑测试并提交**

Run: `cargo test test_recommended_hot_sync_delay_changes_with_load -- --exact`

Expected: PASS

```bash
git add src-tauri/Cargo.toml src-tauri/src/db.rs src-tauri/src/main.rs
git commit -m "feat(sync): add adaptive hot sync debounce"
```

### Task 7: 扩展 HTTP 接口，支持费率管理与汇率刷新

**Files:**
- Modify: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/server.rs`
- Modify: `src-tauri/src/main.rs`
- Test: `src-tauri/src/db.rs`

- [ ] **Step 1: 先写失败测试，约束费率表的读写接口**

```rust
#[test]
fn test_upsert_model_pricing_rows() {
    let test_id = chrono::Utc::now().timestamp_millis();
    let temp_path = std::env::temp_dir().join(format!("ai_token_monitor_pricing_upsert_test_{}", test_id));
    std::fs::create_dir_all(&temp_path).unwrap();
    std::env::set_var("USERPROFILE", temp_path.to_str().unwrap());
    std::env::set_var("DATABASE_TYPE", "sqlite");

    init_cache_db().unwrap();

    let rows = vec![ModelPricingRow {
        id: None,
        model_pattern: "*claude-4-sonnet*".to_string(),
        input_price_per_million: 4.0,
        cached_input_price_per_million: 0.4,
        output_price_per_million: 20.0,
        priority: 5,
        enabled: true,
        updated_at: chrono::Utc::now().to_rfc3339(),
    }];

    upsert_model_pricing_rows(&rows).unwrap();
    let saved = list_model_pricing_rows().unwrap();
    assert!(saved.iter().any(|row| row.model_pattern == "*claude-4-sonnet*"));
}
```

- [ ] **Step 2: 运行测试，确认当前还没有 CRUD 能力**

Run: `cargo test test_upsert_model_pricing_rows -- --exact`

Expected: FAIL，提示 `upsert_model_pricing_rows` / `list_model_pricing_rows` 不存在。

- [ ] **Step 3: 在 `db.rs` 中补齐费率与汇率的最小可用读写 API**

```rust
pub fn list_model_pricing_rows() -> Result<Vec<ModelPricingRow>, rusqlite::Error> {
    let conn = rusqlite::Connection::open(get_db_cache_path())?;
    load_model_pricing(&conn)
}

pub fn upsert_model_pricing_rows(rows: &[ModelPricingRow]) -> Result<(), rusqlite::Error> {
    let conn = rusqlite::Connection::open(get_db_cache_path())?;
    let tx = conn.unchecked_transaction()?;
    tx.execute("DELETE FROM model_pricing", [])?;

    for row in rows {
        tx.execute(
            "INSERT INTO model_pricing (
                model_pattern,
                input_price_per_million,
                cached_input_price_per_million,
                output_price_per_million,
                priority,
                enabled,
                updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                row.model_pattern,
                row.input_price_per_million,
                row.cached_input_price_per_million,
                row.output_price_per_million,
                row.priority,
                if row.enabled { 1 } else { 0 },
                row.updated_at,
            ],
        )?;
    }

    tx.commit()?;
    Ok(())
}
```

- [ ] **Step 4: 在 `server.rs` / `main.rs` 注册新接口**

在 `src-tauri/src/server.rs` 增加：

```rust
#[derive(serde::Deserialize, serde::Serialize)]
pub struct PricingSaveReq {
    pub display_currency: String,
    pub rows: Vec<crate::db::ModelPricingRow>,
}

pub async fn handle_model_pricing_get() -> impl axum::response::IntoResponse {
    let result = tokio::task::spawn_blocking(move || crate::db::list_model_pricing_rows()).await;
    match result {
        Ok(Ok(rows)) => {
            let body = serde_json::json!({
                "rows": rows,
                "display_currency": std::env::var("DISPLAY_CURRENCY").unwrap_or_else(|_| "USD".to_string())
            });
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap()
        }
        _ => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from("Internal Server Error"))
            .unwrap(),
    }
}
```

在 `src-tauri/src/main.rs` 注册：

```rust
.route("/api/model-pricing", get(handle_model_pricing_get).post(handle_model_pricing_save))
.route("/api/exchange-rates/refresh", post(handle_exchange_rate_refresh))
```

同时把 `ConfigReq` 扩成：

```rust
pub display_currency: Option<String>,
```

并在 `handle_config_get()` / `handle_config_save()` 中读写 `DISPLAY_CURRENCY`。

- [ ] **Step 5: 回跑测试并提交**

Run: `cargo test test_upsert_model_pricing_rows -- --exact`

Expected: PASS

```bash
git add src-tauri/src/db.rs src-tauri/src/server.rs src-tauri/src/main.rs
git commit -m "feat(api): add pricing management endpoints"
```

### Task 8: 前端增加项目消耗大盘和排行榜

**Files:**
- Create: `src/components/charts/ProjectTrendChart.tsx`
- Modify: `src/App.tsx`
- Test: `package.json`

- [ ] **Step 1: 先在 `App.tsx` 扩展类型，确保编译先报缺字段错误**

```ts
interface ProjectTrendItem {
  date: string;
  project_name: string;
  tokens: number;
  cost_usd: number;
}

interface ProjectRankingItem {
  project_name: string;
  project_path: string;
  total_tokens: number;
  total_cost_usd: number;
  sessions_count: number;
}

interface AggregatedMetrics {
  totals: Totals;
  daily_trends: DailyTrend[];
  monthly_summary: MonthlySummary[];
  model_distribution: ModelDistribution[];
  sessions: SessionItem[];
  source_trends: SourceTrendItem[];
  device_trends: DeviceTrendItem[];
  project_trends: ProjectTrendItem[];
  project_rankings: ProjectRankingItem[];
  model_performance: ModelPerformance[];
  performance_trends: PerformanceTrend[];
  display_currency: string;
  usd_exchange_rate: number;
  exchange_rate_updated_at: string;
}
```

- [ ] **Step 2: 创建项目趋势图组件**

创建 `src/components/charts/ProjectTrendChart.tsx`：

```tsx
import { useMemo } from 'react';
import { ECharts } from '../ECharts';

interface ProjectTrendItem {
  date: string;
  project_name: string;
  tokens: number;
  cost_usd: number;
}

export function ProjectTrendChart({ data = [], theme }: { data: ProjectTrendItem[]; theme: 'light' | 'dark' }) {
  const isDark = theme === 'dark';

  const option = useMemo(() => {
    const dates = Array.from(new Set(data.map((item) => item.date))).sort();
    const projects = Array.from(new Set(data.map((item) => item.project_name)));

    const series = projects.map((project, idx) => ({
      name: project,
      type: 'line',
      smooth: true,
      showSymbol: false,
      data: dates.map((date) => {
        const row = data.find((item) => item.date === date && item.project_name === project);
        return row ? row.tokens : 0;
      }),
      lineStyle: { width: 2 },
      areaStyle: idx < 3 ? { opacity: 0.08 } : undefined,
    }));

    return {
      tooltip: { trigger: 'axis', confine: true },
      legend: { type: 'scroll', top: 0 },
      grid: { left: 45, right: 20, top: 36, bottom: 24 },
      xAxis: { type: 'category', data: dates },
      yAxis: { type: 'value' },
      series,
      textStyle: { color: isDark ? '#e5e7eb' : '#0f172a' },
    };
  }, [data, isDark]);

  return (
    <div style={{ height: '320px', width: '100%' }}>
      <ECharts option={option as any} />
    </div>
  );
}
```

- [ ] **Step 3: 在大盘中插入“项目消耗大盘 + 排行榜”双栏区块**

在 `src/App.tsx` 引入组件并新增币种格式化：

```tsx
import { ProjectTrendChart } from './components/charts/ProjectTrendChart';

const formatCurrency = (usd: number, rate = 1, currency = 'USD') => {
  const value = usd * rate;
  return new Intl.NumberFormat('zh-CN', {
    style: 'currency',
    currency,
    maximumFractionDigits: currency === 'JPY' ? 0 : 2,
  }).format(value);
};
```

在“每日趋势图”下方插入：

```tsx
<section className="grid grid-cols-1 xl:grid-cols-[1.5fr_1fr] gap-6">
  <div className="glass-card p-5 flex flex-col gap-4">
    <div className="pb-3 border-b border-card-border flex items-center justify-between gap-3">
      <div>
        <h2 className="text-sm font-semibold text-text-primary">项目消耗大盘</h2>
        <p className="text-[11px] text-text-secondary mt-1">
          按代码仓库对比每日 Token 消耗走势
        </p>
      </div>
      <span className="text-[10px] text-text-muted">
        币种：{data?.display_currency || 'USD'}
      </span>
    </div>
    {data?.project_trends && data.project_trends.length > 0 ? (
      <ProjectTrendChart data={data.project_trends} theme={theme} />
    ) : (
      <div className="h-[320px] flex items-center justify-center text-text-muted italic">暂无项目趋势数据</div>
    )}
  </div>

  <div className="glass-card p-5 flex flex-col gap-4">
    <div className="pb-3 border-b border-card-border">
      <h2 className="text-sm font-semibold text-text-primary">项目消耗排行榜</h2>
    </div>
    <div className="flex flex-col gap-3 max-h-[360px] overflow-y-auto pr-1">
      {data?.project_rankings?.length ? data.project_rankings.map((row, idx) => (
        <div key={row.project_name} className="rounded-2xl border border-card-border bg-bg-secondary/40 dark:bg-white/3 p-3 flex flex-col gap-2">
          <div className="flex items-center justify-between gap-3">
            <div>
              <div className="text-xs font-semibold text-text-primary">#{idx + 1} {row.project_name}</div>
              <div className="text-[10px] text-text-muted truncate" title={row.project_path}>{row.project_path || 'unknown-path'}</div>
            </div>
            <div className="text-right">
              <div className="text-xs font-mono text-neon-cyan">{formatNum(row.total_tokens)} Tokens</div>
              <div className="text-[10px] text-text-secondary">
                {formatCurrency(row.total_cost_usd, data?.usd_exchange_rate || 1, data?.display_currency || 'USD')}
              </div>
            </div>
          </div>
          <div className="text-[10px] text-text-muted">会话数：{formatNum(row.sessions_count)}</div>
        </div>
      )) : <div className="text-center py-6 text-text-muted italic">暂无项目排行数据</div>}
    </div>
  </div>
</section>
```

- [ ] **Step 4: 编译验证并提交**

Run: `npm run build`

Expected: PASS，产出 `dist`，无 TypeScript 错误。

```bash
git add src/App.tsx src/components/charts/ProjectTrendChart.tsx
git commit -m "feat(ui): add project consumption dashboard"
```

### Task 9: 前端设置弹窗增加模型费率与币种管理

**Files:**
- Modify: `src/App.tsx`
- Test: `package.json`

- [ ] **Step 1: 先补状态与加载逻辑，让编译告诉你缺哪些字段**

```tsx
interface ModelPricingRow {
  id?: number;
  model_pattern: string;
  input_price_per_million: number;
  cached_input_price_per_million: number;
  output_price_per_million: number;
  priority: number;
  enabled: boolean;
  updated_at: string;
}

const [pricingRows, setPricingRows] = useState<ModelPricingRow[]>([]);
const [displayCurrency, setDisplayCurrency] = useState('USD');
const [pricingLoading, setPricingLoading] = useState(false);
const [pricingSaving, setPricingSaving] = useState(false);
```

在配置弹窗打开时加载：

```tsx
const loadPricingConfig = async () => {
  setPricingLoading(true);
  try {
    const response = await fetch(`/api/model-pricing?t=${Date.now()}`);
    if (!response.ok) return;
    const result = await response.json();
    setPricingRows(result.rows || []);
    setDisplayCurrency(result.display_currency || 'USD');
  } finally {
    setPricingLoading(false);
  }
};
```

- [ ] **Step 2: 把费率编辑表单直接塞进现有设置弹窗，避免新页面跳转**

在 `src/App.tsx` 的设置弹窗中新增：

```tsx
<div className="border-t border-card-border pt-4 mt-2 flex flex-col gap-3">
  <div>
    <span className="text-xs font-semibold text-text-primary">模型费率管理</span>
    <p className="text-[10px] text-text-muted mt-1">价格单位均为每百万 Token，基础币种固定为 USD，显示金额按币种自动换算。</p>
  </div>

  <div className="flex items-center gap-3">
    <label className="text-xs text-text-secondary">显示币种</label>
    <select
      value={displayCurrency}
      onChange={(e) => setDisplayCurrency(e.target.value)}
      className="bg-bg-secondary/60 border border-card-border rounded-xl px-3 py-2 text-xs text-text-primary"
    >
      <option value="USD">USD</option>
      <option value="CNY">CNY</option>
      <option value="JPY">JPY</option>
      <option value="EUR">EUR</option>
    </select>
  </div>

  <div className="flex flex-col gap-2 max-h-[280px] overflow-y-auto pr-1">
    {pricingRows.map((row, index) => (
      <div key={`${row.model_pattern}-${index}`} className="grid grid-cols-[1.4fr_1fr_1fr_1fr_90px_80px] gap-2 items-center">
        <input value={row.model_pattern} onChange={(e) => {
          const next = [...pricingRows];
          next[index] = { ...next[index], model_pattern: e.target.value };
          setPricingRows(next);
        }} className="bg-bg-secondary/60 border border-card-border rounded-xl px-3 py-2 text-xs text-text-primary" />
        <input type="number" value={row.input_price_per_million} onChange={(e) => {
          const next = [...pricingRows];
          next[index] = { ...next[index], input_price_per_million: Number(e.target.value) };
          setPricingRows(next);
        }} className="bg-bg-secondary/60 border border-card-border rounded-xl px-3 py-2 text-xs text-text-primary" />
        <input type="number" value={row.cached_input_price_per_million} onChange={(e) => {
          const next = [...pricingRows];
          next[index] = { ...next[index], cached_input_price_per_million: Number(e.target.value) };
          setPricingRows(next);
        }} className="bg-bg-secondary/60 border border-card-border rounded-xl px-3 py-2 text-xs text-text-primary" />
        <input type="number" value={row.output_price_per_million} onChange={(e) => {
          const next = [...pricingRows];
          next[index] = { ...next[index], output_price_per_million: Number(e.target.value) };
          setPricingRows(next);
        }} className="bg-bg-secondary/60 border border-card-border rounded-xl px-3 py-2 text-xs text-text-primary" />
        <input type="number" value={row.priority} onChange={(e) => {
          const next = [...pricingRows];
          next[index] = { ...next[index], priority: Number(e.target.value) };
          setPricingRows(next);
        }} className="bg-bg-secondary/60 border border-card-border rounded-xl px-3 py-2 text-xs text-text-primary" />
        <button type="button" onClick={() => setPricingRows(pricingRows.filter((_, current) => current !== index))} className="text-xs text-rose-400 hover:text-rose-300">删除</button>
      </div>
    ))}
  </div>
</div>
```

- [ ] **Step 3: 保存费率与币种，并在成功后刷新大盘**

```tsx
const savePricingConfig = async () => {
  setPricingSaving(true);
  try {
    const response = await fetch('/api/model-pricing', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        display_currency: displayCurrency,
        rows: pricingRows,
      }),
    });

    if (!response.ok) {
      setConfigMessage({ success: false, text: '模型费率保存失败。' });
      return;
    }

    setConfigMessage({ success: true, text: '模型费率与显示币种已保存。' });
    fetchData(source, startDate, endDate);
    fetchSessions(1, pageSize, searchKeyword, source, sortField, sortOrder, startDate, endDate, hideZero);
  } finally {
    setPricingSaving(false);
  }
};
```

把“保存并应用配置”按钮改成先保存基础配置，再保存费率配置：

```tsx
await Promise.all([
  saveBaseConfig(),
  savePricingConfig(),
]);
```

- [ ] **Step 4: 编译验证并提交**

Run: `npm run build`

Expected: PASS

```bash
git add src/App.tsx
git commit -m "feat(ui): add pricing and currency settings"
```

### Task 10: 端到端验证与回归检查

**Files:**
- Modify: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/server.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src/App.tsx`
- Create: `src/components/charts/ProjectTrendChart.tsx`
- Create: `src-tauri/migrations/postgres/V3__project_pricing_tables.sql`

- [ ] **Step 1: 运行 Rust 测试，先确认后端回归**

Run: `cargo test`

Expected: PASS，包含新增的 schema / pricing / metrics / fts / hot-sync 测试。

- [ ] **Step 2: 运行前端构建，确认类型与打包无误**

Run: `npm run build`

Expected: PASS，Vite 与 TypeScript 构建成功。

- [ ] **Step 3: 启动桌面应用做人工验证**

Run: `npm run tauri dev`

Expected: Tauri 应用启动成功，前端能访问后端 API。

- [ ] **Step 4: 按真实操作路径做 4 组手工 QA**

```text
1. 项目大盘
   - 导入至少两个不同 project_path 的会话数据
   - 首页出现“项目消耗大盘”和“项目消耗排行榜”
   - 排行榜按 total_tokens 从高到低排序

2. 模型费率
   - 在设置中新增一条 *claude-4-sonnet* 费率
   - 重新刷新首页后，总成本发生可预期变化

3. 汇率
   - 将显示币种切到 CNY 或 JPY
   - KPI 与项目排行榜中的金额显示切为对应币种

4. 搜索与扫描
   - 搜索项目名关键字能秒级返回会话
   - 编译或跑测试时观察热同步日志，重负载阶段不会持续 5 秒触发扫描
```

- [ ] **Step 5: 提交最终集成 commit**

```bash
git add src-tauri/src/db.rs src-tauri/src/server.rs src-tauri/src/main.rs src-tauri/Cargo.toml src/App.tsx src/components/charts/ProjectTrendChart.tsx src-tauri/migrations/postgres/V3__project_pricing_tables.sql
git commit -m "feat: add project analytics, pricing controls, adaptive sync and fts search"
```

## 实施顺序建议

1. 先做 Task 1-3，确保 schema、同步链路和计费逻辑稳定。
2. 再做 Task 4-5，把 metrics 和搜索一起打通，便于后端一次回归。
3. 接着做 Task 6-7，补齐自适应热同步和配置接口。
4. 最后做 Task 8-10，完成 UI、人工验证和最终集成。

## 风险清单

- SQLite FTS5 只对 SQLite 生效，PostgreSQL 分支本计划保持现有 `LIKE`；不要在前端假设所有数据库都支持全文检索语法。
- `estimate_cost` 改成查表后，任何调用点没处理 `Result` 都会编译失败；这反而是好事，利用编译器把遗漏一次性找全。
- `project_name` 是派生列，必须在所有 session upsert 路径同时更新，不能只改 Claude 分支。
- 自适应热同步不要直接停止 watcher；只调整防抖延迟，避免漏掉文件变更事件。
- 汇率失败时 UI 应继续显示 USD，不要让首页因为网络抖动变空白。

## 验证矩阵

- Rust 单元测试：schema、FTS、项目缓存、费率匹配、热同步延迟决策
- 前端静态验证：`npm run build`
- 桌面人工验证：`npm run tauri dev`
- 数据回归：现有 `test_sync_and_aggregate_integration` 仍需通过
