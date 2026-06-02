use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
pub use crate::config::get_user_profile_dir;
use std::cell::RefCell;
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::Serialize;

use crate::proto::{parse_protobuf_orig, try_parse_sub_messages, extract_metrics_from_proto};

thread_local! {
    static IS_HOT_SYNC: RefCell<bool> = RefCell::new(false);
    static SCAN_LOGS: RefCell<Vec<String>> = RefCell::new(Vec::new());
    static SCAN_HAS_CHANGES: RefCell<bool> = RefCell::new(false);
}

pub fn get_db_cache_path() -> PathBuf {
    Path::new(&crate::config::get_user_profile_dir())
        .join(".token-insight")
        .join("db")
        .join("token_stats.db")
}

pub fn get_conversations_dir() -> PathBuf {
    Path::new(&get_user_profile_dir())
        .join(".gemini")
        .join("antigravity")
        .join("conversations")
}

pub fn get_brain_dir() -> PathBuf {
    Path::new(&get_user_profile_dir())
        .join(".gemini")
        .join("antigravity")
        .join("brain")
}

pub fn get_device_name() -> String {
    let _ = dotenvy::dotenv();
    std::env::var("DEVICE_NAME")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| {
            std::env::var("COMPUTERNAME")
                .or_else(|_| std::env::var("HOSTNAME"))
                .or_else(|_| std::env::var("USERNAME"))
                .or_else(|_| std::env::var("USER"))
                .ok()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| "unknown-device".to_string())
        })
}

// 2. 会话元数据与日志读取逻辑

pub fn extract_convo_info(uuid: &str, db_path: &Path) -> (String, String) {
    let brain_dir = get_brain_dir();
    let transcript_path = brain_dir
        .join(uuid)
        .join(".system_generated")
        .join("logs")
        .join("transcript.jsonl");

    let mut title = format!("Unknown Session ({})", &uuid[0..8.min(uuid.len())]);
    let mut created_at = None;

    if transcript_path.exists() {
        if let Ok(file) = File::open(&transcript_path) {
            let reader = BufReader::new(file);
            let user_request_re = Regex::new(r"(?s)<USER_REQUEST>(.*?)</USER_REQUEST>").unwrap();
            let html_tag_re = Regex::new(r"<[^>]+>").unwrap();

            for line in reader.lines() {
                if let Ok(line_str) = line {
                    if line_str.trim().is_empty() {
                        continue;
                    }
                    if let Ok(data) = serde_json::from_str::<serde_json::Value>(&line_str) {
                        if created_at.is_none() {
                            if let Some(created) = data.get("created_at").and_then(|v| v.as_str()) {
                                created_at = Some(created.to_string());
                            }
                        }

                        if data.get("type").and_then(|v| v.as_str()) == Some("USER_INPUT")
                            && title.starts_with("Unknown Session")
                        {
                            let content = data.get("content").and_then(|v| v.as_str()).unwrap_or("");
                            let req_text = if let Some(caps) = user_request_re.captures(content) {
                                caps.get(1).unwrap().as_str().trim().to_string()
                            } else {
                                content.trim().to_string()
                            };
                            let req_text = html_tag_re.replace_all(&req_text, "").trim().to_string();
                            let first_line = req_text.lines().next().unwrap_or("").trim();
                            if !first_line.is_empty() {
                                if first_line.chars().count() > 35 {
                                    title = format!("{}...", first_line.chars().take(35).collect::<String>());
                                } else {
                                    title = first_line.to_string();
                                }
                            } else {
                                title = "Empty Request".to_string();
                            }
                        }
                    }
                }
            }
        }
    }

    let created_at = created_at.unwrap_or_else(|| {
        if let Ok(metadata) = std::fs::metadata(db_path) {
            if let Ok(modified) = metadata.modified() {
                let datetime: DateTime<Utc> = modified.into();
                return datetime.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
            }
        }
        Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
    });

    (title, created_at)
}

fn detect_project_name(project_path: Option<&str>) -> String {
    project_path
        .and_then(|path_str| {
            let path = std::path::Path::new(path_str);
            if path.is_file() || path.extension().is_some() {
                path.parent().and_then(|p| p.file_name())
            } else {
                path.file_name()
            }
        })
        .and_then(|name| name.to_str())
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty() && name != "sessions" && name != "workspaceStorage" && name != "globalStorage")
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

// 3. 增量本地缓存数据库结构初始化

pub fn init_cache_db() -> Result<(), rusqlite::Error> {
    let db_path = get_db_cache_path();
    if let Some(parent) = db_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let conn = rusqlite::Connection::open(&db_path)?;

    // 启用 WAL 模式和 synchronous=NORMAL，极大地加速并优化并发读写
    let _ = conn.execute("PRAGMA journal_mode=WAL;", []);
    let _ = conn.execute("PRAGMA synchronous=NORMAL;", []);

    // 直接创建基于联合主键的最新 sessions 和 turns 表结构
    conn.execute(
        "CREATE TABLE IF NOT EXISTS sessions (
            source TEXT NOT NULL,
            uuid TEXT NOT NULL,
            title TEXT,
            created_at TEXT,
            last_parsed_idx INTEGER DEFAULT -1,
            last_mtime REAL DEFAULT 0.0,
            project_path TEXT,
            PRIMARY KEY (source, uuid)
        )",
        [],
    )?;

    // SQLite 增量升级：为 sessions 表添加 device_name 字段
    {
        let mut stmt = conn.prepare("PRAGMA table_info(sessions)")?;
        let mut rows = stmt.query([])?;
        let mut has_device_name = false;
        while let Some(row) = rows.next()? {
            let name: String = row.get(1)?;
            if name == "device_name" {
                has_device_name = true;
                break;
            }
        }
        if !has_device_name {
            let _ = conn.execute("ALTER TABLE sessions ADD COLUMN device_name TEXT DEFAULT 'unknown';", []);
        }
    }

    // SQLite 增量升级：为 sessions 表添加 project_name 字段
    {
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
            let mut stmt_update = conn.prepare("SELECT source, uuid, project_path FROM sessions")?;
            let mut rows_update = stmt_update.query([])?;
            let mut updates = Vec::new();
            while let Some(row) = rows_update.next()? {
                let source: String = row.get(0)?;
                let uuid: String = row.get(1)?;
                let project_path: Option<String> = row.get(2)?;
                let proj_name = detect_project_name(project_path.as_deref());
                updates.push((source, uuid, proj_name));
            }
            for (source, uuid, proj_name) in updates {
                conn.execute(
                    "UPDATE sessions SET project_name = ? WHERE source = ? AND uuid = ?",
                    rusqlite::params![proj_name, source, uuid],
                )?;
            }
        }
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

    conn.execute(
        "CREATE TABLE IF NOT EXISTS turns (
            source TEXT NOT NULL,
            uuid TEXT NOT NULL,
            idx INTEGER NOT NULL,
            model TEXT,
            input_tokens INTEGER DEFAULT 0,
            cached_input_tokens INTEGER DEFAULT 0,
            output_tokens INTEGER DEFAULT 0,
            thinking_tokens INTEGER DEFAULT 0,
            cost_usd REAL DEFAULT 0.0,
            message_id TEXT,
            request_id TEXT,
            timestamp TEXT,
            latency REAL DEFAULT 0.0,
            tps REAL DEFAULT 0.0,
            PRIMARY KEY (source, uuid, idx),
            FOREIGN KEY(source, uuid) REFERENCES sessions(source, uuid) ON DELETE CASCADE
        )",
        [],
    )?;

    // 检测并平滑重构 daily_stats 缓存表以引入设备名称字段
    {
        let mut stmt = conn.prepare("PRAGMA table_info(daily_stats)")?;
        let mut rows = stmt.query([])?;
        let mut stats_has_device_name = false;
        while let Some(row) = rows.next()? {
            let name: String = row.get(1)?;
            if name == "device_name" {
                stats_has_device_name = true;
                break;
            }
        }
        if !stats_has_device_name {
            let _ = conn.execute("DROP TABLE IF EXISTS daily_stats;", []);
        }
    }

    // 直接创建基于联合主键的最新 daily_stats 缓存表结构
    conn.execute(
        "CREATE TABLE IF NOT EXISTS daily_stats (
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
        )",
        [],
    )?;

    // 创建高性能索引以优化大盘统计查询性能
    conn.execute("CREATE INDEX IF NOT EXISTS idx_sessions_created_at ON sessions(created_at);", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_sessions_source_created ON sessions(source, created_at);", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_turns_model ON turns(model);", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_turns_latency ON turns(latency);", [])?;

    // 默认费率灌种
    seed_default_model_pricing(&conn)?;

    // 默认汇率灌种
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute("INSERT OR IGNORE INTO exchange_rates (currency_code, rate_from_usd, updated_at) VALUES ('CNY', 7.24, ?)", [&now])?;
    conn.execute("INSERT OR IGNORE INTO exchange_rates (currency_code, rate_from_usd, updated_at) VALUES ('JPY', 155.4, ?)", [&now])?;
    conn.execute("INSERT OR IGNORE INTO exchange_rates (currency_code, rate_from_usd, updated_at) VALUES ('EUR', 0.92, ?)", [&now])?;

    // 创建使用复盘与建议的后台任务与事件记录表
    conn.execute(
        "CREATE TABLE IF NOT EXISTS review_tasks (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            status TEXT NOT NULL,
            cli_name TEXT NOT NULL,
            cli_path TEXT,
            time_range TEXT NOT NULL,
            selected_ides_json TEXT NOT NULL,
            prompt_text TEXT NOT NULL,
            prompt_hash TEXT NOT NULL,
            metrics_snapshot_json TEXT NOT NULL,
            metrics_hash TEXT NOT NULL,
            dedupe_key TEXT NOT NULL,
            progress_stage TEXT NOT NULL,
            progress_percent INTEGER NOT NULL DEFAULT 0,
            status_message TEXT NOT NULL DEFAULT '',
            output_markdown TEXT NOT NULL DEFAULT '',
            error_message TEXT,
            exit_code INTEGER,
            created_at TEXT NOT NULL,
            started_at TEXT,
            finished_at TEXT,
            canceled_at TEXT,
            last_heartbeat_at TEXT,
            error_type TEXT DEFAULT NULL,
            quality_feedback TEXT DEFAULT NULL,
            action_items_json TEXT DEFAULT NULL,
            compare_metrics_snapshot_json TEXT DEFAULT NULL,
            template_id TEXT DEFAULT NULL
        )",
        [],
    )?;

    // SQLite 增量升级：为 review_tasks 表添加 error_type, quality_feedback, action_items_json, compare_metrics_snapshot_json, template_id 字段 (若已存在表)
    {
        let mut stmt = conn.prepare("PRAGMA table_info(review_tasks)")?;
        let mut rows = stmt.query([])?;
        let mut has_error_type = false;
        let mut has_quality_feedback = false;
        let mut has_action_items = false;
        let mut has_compare_metrics_snapshot = false;
        let mut has_template_id = false;
        while let Some(row) = rows.next()? {
            let name: String = row.get(1)?;
            if name == "error_type" {
                has_error_type = true;
            } else if name == "quality_feedback" {
                has_quality_feedback = true;
            } else if name == "action_items_json" {
                has_action_items = true;
            } else if name == "compare_metrics_snapshot_json" {
                has_compare_metrics_snapshot = true;
            } else if name == "template_id" {
                has_template_id = true;
            }
        }
        if !has_error_type {
            let _ = conn.execute("ALTER TABLE review_tasks ADD COLUMN error_type TEXT DEFAULT NULL;", []);
        }
        if !has_quality_feedback {
            let _ = conn.execute("ALTER TABLE review_tasks ADD COLUMN quality_feedback TEXT DEFAULT NULL;", []);
        }
        if !has_action_items {
            let _ = conn.execute("ALTER TABLE review_tasks ADD COLUMN action_items_json TEXT DEFAULT NULL;", []);
        }
        if !has_compare_metrics_snapshot {
            let _ = conn.execute("ALTER TABLE review_tasks ADD COLUMN compare_metrics_snapshot_json TEXT DEFAULT NULL;", []);
        }
        if !has_template_id {
            // 在升级加入 template_id 的同时，顺便清理历史报告数据
            let _ = conn.execute("DELETE FROM review_task_events;", []);
            let _ = conn.execute("DELETE FROM review_tasks;", []);
            let _ = conn.execute("ALTER TABLE review_tasks ADD COLUMN template_id TEXT DEFAULT NULL;", []);
        }
    }

    conn.execute(
        "CREATE TABLE IF NOT EXISTS review_task_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            task_id TEXT NOT NULL,
            sequence INTEGER NOT NULL,
            kind TEXT NOT NULL,
            message TEXT NOT NULL,
            payload_json TEXT,
            created_at TEXT NOT NULL,
            FOREIGN KEY(task_id) REFERENCES review_tasks(id)
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS turn_details (
            source TEXT NOT NULL,
            uuid TEXT NOT NULL,
            idx INTEGER NOT NULL,
            user_prompt TEXT,
            executed_commands TEXT,
            failed_commands TEXT,
            modified_files TEXT,
            PRIMARY KEY (source, uuid, idx)
        )",
        [],
    )?;

    conn.execute("CREATE INDEX IF NOT EXISTS idx_review_task_events_task_sequence ON review_task_events(task_id, sequence);", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_turn_details_lookup ON turn_details(source, uuid, idx);", [])?;

    // 初始化提示词模板表
    conn.execute(
        "CREATE TABLE IF NOT EXISTS prompt_templates (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            template TEXT NOT NULL,
            is_builtin INTEGER DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
        [],
    )?;

    seed_default_prompt_templates(&conn)?;

    Ok(())
}

fn seed_default_prompt_templates(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    let presets = [
        (
            "comprehensive",
            "📊 综合效能评估",
            "对用量、开销、缓存与提问质量进行全面、通用的效能诊断。",
            r#"你是一位专业的 AI 工具使用顾问。我使用 Token Insight 追踪了我在 {{IDE}} 等工具上的 Token 消耗情况。

请根据下方我的使用数据，为我提供一份**深度使用复盘报告**，用中文回答。

---

## 我的使用数据（最近7天）

| 指标 | 数值 |
|------|------|
| 总 Token 消耗 | {{TOTAL_TOKENS}} tokens |
| 总费用 | ${{TOTAL_COST}} USD |
| 总会话数 | {{TOTAL_SESSIONS}} 次 |
| 缓存命中率 | {{CACHE_HIT_RATE}}% |
| 推理（Thinking）Token 占比 | {{THINKING_RATIO}}% |
{{SOURCE_BREAKDOWN}}{{MODEL_DISTRIBUTION}}{{DAILY_TREND_SUMMARY}}

---

## 请按以下结构输出分析报告（使用 Markdown 格式）：

### 1. 使用模式诊断
分析我的 AI 工具使用习惯，包括：
- 主要使用哪些工具/模型？
- 使用频率是否均匀，有无明显的高峰/低谷？
- 缓存命中率 {{CACHE_HIT_RATE}}% 是否合理？（业界参考：>30% 较好）
- 推理 Token 占比 {{THINKING_RATIO}}% 说明什么？

### 2. 成本优化建议
基于以上数据，给出 3~5 条具体、可操作的成本优化建议，例如：
- 哪些场景可以换用更便宜的模型？
- 如何提升缓存命中率？
- 是否存在明显的低效会话模式？

### 3. 效率评估
- 综合评价我的 AI 使用效率（满分100分，给出评分与理由）
- 与一般开发者的平均水平相比，我的数据表现如何？

### 4. 本周行动清单
列出 3 条我这周可以立刻执行的具体优化行动（要具体到操作步骤，不要泛泛而谈）。

---

请直接开始输出报告，不需要前言。保持语言简洁专业，使用 Markdown 格式。"#
        ),
        (
            "cost_saving",
            "🔍 成本节流专项",
            "主攻降本增效，提供低配模型平替、高消耗 Turn 拦截、缓存提升建议。",
            r#"你是一位精通成本优化的 AI 治理专家。我使用 Token Insight 追踪了我在 {{IDE}} 等工具上的 Token 消耗情况。
请根据下方我的使用数据，为我提供一份**成本优化专项复盘报告**，用中文回答。

---

## 我的使用数据（最近7天）

| 指标 | 数值 |
|------|------|
| 总 Token 消耗 | {{TOTAL_TOKENS}} tokens |
| 总费用 | ${{TOTAL_COST}} USD |
| 总会话数 | {{TOTAL_SESSIONS}} 次 |
| 缓存命中率 | {{CACHE_HIT_RATE}}% |
| 推理（Thinking）Token 占比 | {{THINKING_RATIO}}% |
{{SOURCE_BREAKDOWN}}{{MODEL_DISTRIBUTION}}{{DAILY_TREND_SUMMARY}}

---

## 请按以下结构输出分析报告（使用 Markdown 格式）：

### 1. 成本与用量分布诊断
分析本次分析周期中最昂贵的消耗项、最高频的模型偏好，以及费用分布的合理性。

### 2. 核心痛点与降本瓶颈
找出模型配比不合理（如在简单任务上过度使用昂贵模型）、缓存利用率低下、或者存在超长会话（Context 膨胀导致 Token 浪费）的瓶颈。

### 3. 降本增效平替建议
评估有哪些高频场景可以使用更轻量、更低成本的模型平替，或者如何更好地利用提示词缓存（Prompt Caching）。

### 4. 本周行动清单
针对上述发现，给出 3 条具体、可立即执行的降低 AI 成本的行动项，包括推荐的缓存策略和提问控制。

---

请直接开始输出报告，不需要前言。保持语言简洁专业，使用 Markdown 格式。"#
        ),
        (
            "collaboration",
            "⚡ 开发协作质量",
            "主攻提问艺术、代码迭代轮数合理性、上下文复用情况。",
            r#"你是一位敏捷开发与效能教练。我使用 Token Insight 追踪了我在 {{IDE}} 等工具上的 Token 消耗情况。
请根据下方我的使用数据，为我提供一份**人机协作质量诊断报告**，用中文回答。

---

## 我的使用数据（最近7天）

| 指标 | 数值 |
|------|------|
| 总 Token 消耗 | {{TOTAL_TOKENS}} tokens |
| 总费用 | ${{TOTAL_COST}} USD |
| 总会话数 | {{TOTAL_SESSIONS}} 次 |
| 缓存命中率 | {{CACHE_HIT_RATE}}% |
| 推理（Thinking）Token 占比 | {{THINKING_RATIO}}% |
{{SOURCE_BREAKDOWN}}{{MODEL_DISTRIBUTION}}{{DAILY_TREND_SUMMARY}}

---

## 请按以下结构输出分析报告（使用 Markdown 格式）：

### 1. 协同深度与频次诊断
总结我与 AI 协同的频次、单会话平均消耗和整体交互深度，分析使用习惯的健康度。

### 2. 效率瓶颈与低效会话
找出提问流中是否存在多次无效重试、提示词清晰度不足、或者单次会话包含太多不相关改动导致上下文负荷过重。

### 3. 提问艺术与上下文优化
评估我在提示词编写和 IDE 交互时，是否有效利用了上下文切片，以及如果改进提问习惯可以带来多大的效率增益。

### 4. 本周行动清单
给出 3 条提高提问效率和人机协作质量的黄金行动项（例如，推荐单次会话只关注单一职责，利用更清晰的任务边界等）。

---

请直接开始输出报告，不需要前言。保持语言简洁专业，使用 Markdown 格式。"#
        ),
        (
            "project_review",
            "💼 项目全景复盘",
            "分析跨项目用量分布、Token 集中度风险，为研发管理提供战略建议。",
            r#"你是一位技术总监。我使用 Token Insight 追踪了我在 {{IDE}} 等工具上的 Token 消耗情况。
请根据下方我的使用数据，为我提供一份**项目全景效能复盘报告**，用中文回答。

---

## 我的使用数据（最近7天）

| 指标 | 数值 |
|------|------|
| 总 Token 消耗 | {{TOTAL_TOKENS}} tokens |
| 总费用 | ${{TOTAL_COST}} USD |
| 总会话数 | {{TOTAL_SESSIONS}} 次 |
| 缓存命中率 | {{CACHE_HIT_RATE}}% |
| 推理（Thinking）Token 占比 | {{THINKING_RATIO}}% |
{{SOURCE_BREAKDOWN}}{{MODEL_DISTRIBUTION}}{{DAILY_TREND_SUMMARY}}

---

## 请按以下结构输出分析报告（使用 Markdown 格式）：

### 1. 工具集成与渗透度诊断
从全局视角概括我的项目使用分布、高频工具依赖和 AI 在不同开发环境的渗透情况。

### 2. 项目集中度风险分析
分析是否存在某单一 IDE/项目过度消耗导致资源倾斜，或者某些项目几乎没有使用 AI 辅助的效率断层。

### 3. 研发效能与资产化评估
评估跨工具协同的顺畅度，以及当模型或工具发生切换时，对整体交付速度和成本的战略性影响。

### 4. 本周行动清单
给出 3 条适用于团队或个人在跨项目开发时，规范 AI 工具使用和保护技术资产的宏观行动项。

---

请直接开始输出报告，不需要前言。保持语言简洁专业，使用 Markdown 格式。"#
        )
    ];

    for (id, name, desc, template) in presets {
        conn.execute(
            "INSERT INTO prompt_templates (id, name, description, template, is_builtin, created_at, updated_at)
             VALUES (?, ?, ?, ?, 1, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                description = excluded.description,
                template = excluded.template",
            rusqlite::params![id, name, desc, template, now, now],
        )?;
    }
    Ok(())
}


// 3.5. 每日预聚合缓存重建助手方法 (方案二高性能预计算核心)

pub fn rebuild_daily_stats_cache(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    // 检查缓存表是否为空
    let is_empty: bool = conn.query_row(
        "SELECT COUNT(1) FROM daily_stats",
        [],
        |row| row.get::<_, i64>(0).map(|c| c == 0)
    ).unwrap_or(true);

    if is_empty {
        // 首次运行或缓存被清空，执行全量聚合重建以防丢失历史导入数据
        let mut stmt = conn.prepare("DELETE FROM daily_stats")?;
        let _ = stmt.execute([])?;
        
        let mut stmt_insert = conn.prepare(
            "INSERT INTO daily_stats (date, source, device_name, input_tokens, cached_input_tokens, output_tokens, thinking_tokens, sessions_count, cost_usd)
             SELECT 
                 substr(s.created_at, 1, 10) as date,
                 s.source,
                 s.device_name,
                 COALESCE(SUM(t.input_tokens), 0) as input_tokens,
                 COALESCE(SUM(t.cached_input_tokens), 0) as cached_input_tokens,
                 COALESCE(SUM(t.output_tokens), 0) as output_tokens,
                 COALESCE(SUM(t.thinking_tokens), 0) as thinking_tokens,
                 COUNT(DISTINCT s.uuid) as sessions_count,
                 COALESCE(SUM(t.cost_usd), 0.0) as cost_usd
             FROM sessions s
             LEFT JOIN turns t ON s.source = t.source AND s.uuid = t.uuid
             GROUP BY date, s.source, s.device_name"
        )?;
        let _ = stmt_insert.execute([])?;
    } else {
        // 日常增量同步重建：只删除并重新聚合最近 365 天的数据
        let one_year_ago = Utc::now() - chrono::Duration::days(365);
        let one_year_ago_str = one_year_ago.format("%Y-%m-%d").to_string();

        let mut stmt_del = conn.prepare("DELETE FROM daily_stats WHERE date >= ?")?;
        let _ = stmt_del.execute(rusqlite::params![one_year_ago_str])?;

        let mut stmt_insert = conn.prepare(
            "INSERT INTO daily_stats (date, source, device_name, input_tokens, cached_input_tokens, output_tokens, thinking_tokens, sessions_count, cost_usd)
             SELECT 
                 substr(s.created_at, 1, 10) as date,
                 s.source,
                 s.device_name,
                 COALESCE(SUM(t.input_tokens), 0) as input_tokens,
                 COALESCE(SUM(t.cached_input_tokens), 0) as cached_input_tokens,
                 COALESCE(SUM(t.output_tokens), 0) as output_tokens,
                 COALESCE(SUM(t.thinking_tokens), 0) as thinking_tokens,
                 COUNT(DISTINCT s.uuid) as sessions_count,
                 COALESCE(SUM(t.cost_usd), 0.0) as cost_usd
             FROM sessions s
             LEFT JOIN turns t ON s.source = t.source AND s.uuid = t.uuid
             WHERE s.created_at >= ?
             GROUP BY date, s.source, s.device_name"
        )?;
        let _ = stmt_insert.execute(rusqlite::params![one_year_ago_str])?;
    }
    
    Ok(())
}

pub fn rebuild_sessions_fts(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    conn.execute("DELETE FROM sessions_fts", [])?;
    conn.execute(
        "INSERT INTO sessions_fts (source, uuid, title, project_name)
         SELECT source, uuid, COALESCE(title, ''), COALESCE(project_name, 'unknown-project')
         FROM sessions",
        [],
    )?;
    Ok(())
}

pub fn rebuild_project_daily_stats_cache(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
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

pub fn rebuild_pg_daily_stats_cache(client: &mut postgres::Client) -> Result<(), String> {
    let mut tx = client.transaction().map_err(|e| e.to_string())?;

    // 检查缓存表是否为空
    let is_empty: bool = tx.query_one("SELECT COUNT(1) FROM daily_stats", &[])
        .map(|row| row.get::<_, i64>(0) == 0)
        .map_err(|e| e.to_string())?;

    if is_empty {
        // 首次全量同步
        tx.execute("DELETE FROM daily_stats", &[]).map_err(|e| e.to_string())?;
        tx.execute(
            "INSERT INTO daily_stats (date, source, device_name, input_tokens, cached_input_tokens, output_tokens, thinking_tokens, sessions_count, cost_usd)
             SELECT 
                 SUBSTR(s.created_at, 1, 10) as date,
                 s.source,
                 s.device_name,
                 COALESCE(SUM(t.input_tokens), 0) as input_tokens,
                 COALESCE(SUM(t.cached_input_tokens), 0) as cached_input_tokens,
                 COALESCE(SUM(t.output_tokens), 0) as output_tokens,
                 COALESCE(SUM(t.thinking_tokens), 0) as thinking_tokens,
                 COUNT(DISTINCT s.uuid) as sessions_count,
                 COALESCE(SUM(t.cost_usd), 0.0) as cost_usd
             FROM sessions s
             LEFT JOIN turns t ON s.source = t.source AND s.uuid = t.uuid
             GROUP BY SUBSTR(s.created_at, 1, 10), s.source, s.device_name",
            &[],
        ).map_err(|e| e.to_string())?;
    } else {
        // 增量同步重建最近 365 天的数据
        let one_year_ago = Utc::now() - chrono::Duration::days(365);
        let one_year_ago_str = one_year_ago.format("%Y-%m-%d").to_string();

        tx.execute("DELETE FROM daily_stats WHERE date >= $1", &[&one_year_ago_str])
            .map_err(|e| e.to_string())?;

        tx.execute(
            "INSERT INTO daily_stats (date, source, device_name, input_tokens, cached_input_tokens, output_tokens, thinking_tokens, sessions_count, cost_usd)
             SELECT 
                 SUBSTR(s.created_at, 1, 10) as date,
                 s.source,
                 s.device_name,
                 COALESCE(SUM(t.input_tokens), 0) as input_tokens,
                 COALESCE(SUM(t.cached_input_tokens), 0) as cached_input_tokens,
                 COALESCE(SUM(t.output_tokens), 0) as output_tokens,
                 COALESCE(SUM(t.thinking_tokens), 0) as thinking_tokens,
                 COUNT(DISTINCT s.uuid) as sessions_count,
                 COALESCE(SUM(t.cost_usd), 0.0) as cost_usd
             FROM sessions s
             LEFT JOIN turns t ON s.source = t.source AND s.uuid = t.uuid
             WHERE s.created_at >= $1
             GROUP BY SUBSTR(s.created_at, 1, 10), s.source, s.device_name",
            &[&one_year_ago_str],
        ).map_err(|e| e.to_string())?;
    }
    
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}


// 4. 增量扫描逻辑与数据同步

#[derive(Clone, Serialize)]
pub struct ScanStatus {
    pub is_scanning: bool,
    pub total_files: usize,
    pub scanned_files: usize,
    pub error: Option<String>,
    pub logs: Vec<String>,
    pub status_msg: String,
}

pub static DB_LOCK: Mutex<()> = Mutex::new(());
pub static APP_HANDLE: OnceLock<tauri::AppHandle> = OnceLock::new();

pub fn get_scan_status() -> &'static Mutex<ScanStatus> {
    static STATUS: OnceLock<Mutex<ScanStatus>> = OnceLock::new();
    STATUS.get_or_init(|| {
        Mutex::new(ScanStatus {
            is_scanning: false,
            total_files: 0,
            scanned_files: 0,
            error: None,
            logs: Vec::new(),
            status_msg: "未开始同步".to_string(),
        })
    })
}

pub fn log_progress(msg: &str) {
    let is_hot = IS_HOT_SYNC.with(|h| *h.borrow());
    if is_hot {
        SCAN_LOGS.with(|logs| {
            logs.borrow_mut().push(msg.to_string());
        });
    } else {
        println!("{}", msg);
    }
    if let Ok(mut status) = get_scan_status().lock() {
        status.status_msg = msg.to_string();
        status.logs.push(msg.to_string());
        if status.logs.len() > 1000 {
            status.logs.remove(0);
        }
    }
}

pub fn start_background_scan(is_hot_sync: bool) {
    let status_lock = get_scan_status();
    {
        let mut status = status_lock.lock().unwrap();
        if status.is_scanning {
            return; // Already scanning
        }
        status.is_scanning = true;
        status.total_files = 0;
        status.scanned_files = 0;
        status.error = None;
        status.logs = Vec::new();
        status.status_msg = "正在初始化扫描...".to_string();
    }

    std::thread::spawn(move || {
        IS_HOT_SYNC.with(|h| *h.borrow_mut() = is_hot_sync);
        SCAN_LOGS.with(|l| l.borrow_mut().clear());
        SCAN_HAS_CHANGES.with(|c| *c.borrow_mut() = false);

        if is_hot_sync {
            log_progress("[热同步] 检测到物理文件写入变动，防抖结束，开始执行增量更新...");
        }

        let result = sync_cache_db_with_progress(|scanned, total| {
            let status_lock = get_scan_status();
            if let Ok(mut status) = status_lock.lock() {
                status.scanned_files = scanned;
                status.total_files = total;
            }
        });

        let has_changes = SCAN_HAS_CHANGES.with(|c| *c.borrow());
        let has_error = result.is_err();

        if is_hot_sync && (has_changes || has_error) {
            SCAN_LOGS.with(|logs| {
                for log in logs.borrow().iter() {
                    println!("{}", log);
                }
            });
            if let Err(ref e) = result {
                println!("[热同步] 增量更新失败: {}", e);
            }
        }

        let status_lock = get_scan_status();
        if let Ok(mut status) = status_lock.lock() {
            status.is_scanning = false;
            if let Err(e) = result {
                status.error = Some(e.to_string());
            } else {
                // 触发 Tauri 前端热同步事件
                if let Some(app_handle) = APP_HANDLE.get() {
                    use tauri::Emitter;
                    let _ = app_handle.emit("db-updated", serde_json::json!({ "status": "success" }));
                }
            }
        }
    });
}

pub fn get_claude_projects_dir() -> PathBuf {
    Path::new(&get_user_profile_dir())
        .join(".claude")
        .join("projects")
}

pub fn get_codex_sessions_dir() -> PathBuf {
    Path::new(&get_user_profile_dir())
        .join(".codex")
        .join("sessions")
}

pub fn get_codex_config_model() -> String {
    let config_path = Path::new(&get_user_profile_dir())
        .join(".codex")
        .join("config.toml");
    if config_path.exists() {
        if let Ok(content) = std::fs::read_to_string(config_path) {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("model ") || trimmed.starts_with("model=") {
                    let parts: Vec<&str> = trimmed.split('=').collect();
                    if parts.len() >= 2 {
                        let val = parts[1].trim().trim_matches('"').trim_matches('\'').trim();
                        if !val.is_empty() {
                            return val.to_string();
                        }
                    }
                }
            }
        }
    }
    "gpt-5".to_string()
}

fn extract_codex_tokens_and_model(val: &serde_json::Value, default_model: &str) -> Option<(i64, i64, i64, i64, String)> {
    // 1. Try real Codex format
    if val.get("type").and_then(|t| t.as_str()) == Some("event_msg") {
        if let Some(payload) = val.get("payload") {
            if payload.get("type").and_then(|t| t.as_str()) == Some("token_count") {
                if let Some(info) = payload.get("info") {
                    if let Some(last_usage) = info.get("last_token_usage") {
                        let input = last_usage.get("input_tokens").and_then(|v| v.as_i64()).unwrap_or(0);
                        let cached = last_usage.get("cached_input_tokens").and_then(|v| v.as_i64()).unwrap_or(0);
                        let output = last_usage.get("output_tokens").and_then(|v| v.as_i64()).unwrap_or(0);
                        let thinking = last_usage.get("reasoning_output_tokens")
                            .or_else(|| last_usage.get("thinking_tokens"))
                            .and_then(|v| v.as_i64()).unwrap_or(0);
                        
                        return Some((input, cached, output, thinking, default_model.to_string()));
                    }
                }
            }
        }
    }

    // 2. Fallback to Claude format for backwards compatibility/existing tests
    let (input, _cache_creation, cache_read, output, thinking) = extract_claude_tokens(val);
    if input > 0 || output > 0 {
        let model = extract_claude_model(val);
        let model_name = if model == "unknown" { default_model.to_string() } else { model };
        return Some((input, cache_read, output, thinking, model_name));
    }

    None
}

fn find_jsonl_files(dir: &Path, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                find_jsonl_files(&path, files);
            } else if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("jsonl") {
                files.push(path);
            }
        }
    }
}

fn get_total_input_tokens(model: &str, input: i64, cached: i64) -> i64 {
    let model_lower = model.to_lowercase();
    if model_lower.contains("claude") || model_lower.contains("opus") || model_lower.contains("sonnet") || model_lower.contains("haiku") {
        if input < cached {
            input + cached
        } else {
            input
        }
    } else {
        input + cached
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
    let regex_pattern = format!("^{}$", regex::escape(pattern).replace("\\*", ".*"));
    if let Ok(regex) = Regex::new(&regex_pattern) {
        regex.is_match(&model.to_lowercase())
    } else {
        false
    }
}

fn load_model_pricing(conn: &rusqlite::Connection) -> Result<Vec<ModelPricingRow>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, model_pattern, input_price_per_million, cached_input_price_per_million,
                output_price_per_million, priority, enabled, updated_at
         FROM model_pricing
         WHERE enabled = 1
         ORDER BY priority ASC, id ASC"
    )?;

    let rows = stmt.query_map([], |row| {
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
    })?;

    let mut result = Vec::new();
    for r in rows {
        result.push(r?);
    }
    Ok(result)
}

pub fn estimate_cost(model: &str, input: i64, cached: i64, output: i64) -> Result<f64, rusqlite::Error> {
    let pricing = rusqlite::Connection::open(get_db_cache_path())
        .and_then(|conn| load_model_pricing(&conn))
        .unwrap_or_default();
    let model_lower = model.to_lowercase();
    let matched = pricing
        .into_iter()
        .find(|row| glob_match(&row.model_pattern.to_lowercase(), &model_lower));

    let row = matched.unwrap_or_else(|| {
        let model_lower = model.to_lowercase();
        if model_lower.contains("opus") {
            ModelPricingRow {
                id: None,
                model_pattern: "*opus*".to_string(),
                input_price_per_million: 15.0,
                cached_input_price_per_million: 1.5,
                output_price_per_million: 75.0,
                priority: 10,
                enabled: true,
                updated_at: "".to_string(),
            }
        } else if model_lower.contains("sonnet") || model_lower.contains("claude-3-5") {
            ModelPricingRow {
                id: None,
                model_pattern: "*sonnet*".to_string(),
                input_price_per_million: 3.0,
                cached_input_price_per_million: 0.3,
                output_price_per_million: 15.0,
                priority: 20,
                enabled: true,
                updated_at: "".to_string(),
            }
        } else if model_lower.contains("haiku") {
            ModelPricingRow {
                id: None,
                model_pattern: "*haiku*".to_string(),
                input_price_per_million: 0.25,
                cached_input_price_per_million: 0.03,
                output_price_per_million: 1.25,
                priority: 30,
                enabled: true,
                updated_at: "".to_string(),
            }
        } else if model_lower.contains("gemini") {
            if model_lower.contains("pro") {
                ModelPricingRow {
                    id: None,
                    model_pattern: "*gemini*pro*".to_string(),
                    input_price_per_million: 1.25,
                    cached_input_price_per_million: 0.3125,
                    output_price_per_million: 5.0,
                    priority: 40,
                    enabled: true,
                    updated_at: "".to_string(),
                }
            } else {
                ModelPricingRow {
                    id: None,
                    model_pattern: "*gemini*flash*".to_string(),
                    input_price_per_million: 0.075,
                    cached_input_price_per_million: 0.01875,
                    output_price_per_million: 0.3,
                    priority: 50,
                    enabled: true,
                    updated_at: "".to_string(),
                }
            }
        } else {
            ModelPricingRow {
                id: None,
                model_pattern: "*".to_string(),
                input_price_per_million: 2.5,
                cached_input_price_per_million: 0.25,
                output_price_per_million: 10.0,
                priority: 999,
                enabled: true,
                updated_at: "".to_string(),
            }
        }
    });

    let uncached = (input - cached).max(0) as f64;
    Ok((
        uncached * row.input_price_per_million
        + (cached as f64) * row.cached_input_price_per_million
        + (output as f64) * row.output_price_per_million
    ) / 1_000_000.0)
}

fn find_json_field_recursive(val: &serde_json::Value, target_keys: &[&str]) -> Option<i64> {
    match val {
        serde_json::Value::Object(map) => {
            // 优先检查当前层级是否有匹配的键
            for &key in target_keys {
                if let Some(v) = map.get(key) {
                    if let Some(num) = v.as_i64() {
                        return Some(num);
                    }
                }
            }
            // 递归检查子对象
            for v in map.values() {
                if let Some(res) = find_json_field_recursive(v, target_keys) {
                    return Some(res);
                }
            }
            None
        }
        serde_json::Value::Array(arr) => {
            // 遍历数组
            for v in arr {
                if let Some(res) = find_json_field_recursive(v, target_keys) {
                    return Some(res);
                }
            }
            None
        }
        _ => None,
    }
}

fn extract_claude_tokens(val: &serde_json::Value) -> (i64, i64, i64, i64, i64) {
    let mut input = 0;
    let mut cache_creation = 0;
    let mut cache_read = 0;
    let mut output = 0;
    let mut thinking = 0;

    let sources = vec![
        val,
        val.get("usage").unwrap_or(&serde_json::Value::Null),
        val.get("message").and_then(|m| m.get("usage")).unwrap_or(&serde_json::Value::Null),
    ];

    for src in sources {
        if !src.is_object() {
            continue;
        }

        let in_t = src.get("input_tokens")
            .or_else(|| src.get("inputTokens"))
            .or_else(|| src.get("prompt_tokens"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        let out_t = src.get("output_tokens")
            .or_else(|| src.get("outputTokens"))
            .or_else(|| src.get("completion_tokens"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        let c_create = src.get("cache_creation_tokens")
            .or_else(|| src.get("cache_creation_input_tokens"))
            .or_else(|| src.get("cacheCreationInputTokens"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        let c_read = src.get("cache_read_input_tokens")
            .or_else(|| src.get("cache_read_tokens"))
            .or_else(|| src.get("cacheReadInputTokens"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        let think_t = src.get("thinking_tokens")
            .or_else(|| src.get("thinkingTokens"))
            .or_else(|| src.get("reasoning_output_tokens"))
            .or_else(|| src.get("reasoningOutputTokens"))
            .or_else(|| src.get("thinking"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        if in_t > 0 || out_t > 0 {
            input = in_t;
            output = out_t;
            cache_creation = c_create;
            cache_read = c_read;
            thinking = think_t;
            break;
        }
    }

    // 软解析/版本兼容兜底层：如果上面的直接字段没有匹配成功，我们使用递归检索
    if input == 0 && output == 0 {
        if let Some(in_t) = find_json_field_recursive(val, &["input_tokens", "inputTokens", "prompt_tokens", "promptTokens"]) {
            input = in_t;
        }
        if let Some(out_t) = find_json_field_recursive(val, &["output_tokens", "outputTokens", "completion_tokens", "completionTokens"]) {
            output = out_t;
        }
        if let Some(c_create) = find_json_field_recursive(val, &["cache_creation_tokens", "cache_creation_input_tokens", "cacheCreationInputTokens"]) {
            cache_creation = c_create;
        }
        if let Some(c_read) = find_json_field_recursive(val, &["cache_read_input_tokens", "cache_read_tokens", "cacheReadInputTokens"]) {
            cache_read = c_read;
        }
    }

    if thinking == 0 {
        if let Some(think_t) = find_json_field_recursive(val, &["thinking_tokens", "thinkingTokens", "reasoning_output_tokens", "reasoningOutputTokens", "thinking"]) {
            thinking = think_t;
        }
    }

    (input, cache_creation, cache_read, output, thinking)
}

fn extract_claude_model(val: &serde_json::Value) -> String {
    let candidates = vec![
        val.get("message").and_then(|m| m.get("model")),
        val.get("model"),
        val.get("Model"),
        val.get("usage").and_then(|u| u.get("model")),
        val.get("request").and_then(|r| r.get("model")),
    ];

    for cand in candidates {
        if let Some(s) = cand.and_then(|v| v.as_str()) {
            return s.to_string();
        }
    }
    "unknown".to_string()
}

fn extract_claude_timestamp(val: &serde_json::Value) -> String {
    val.get("timestamp")
        .and_then(|t| t.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
        })
}

fn extract_claude_ids(val: &serde_json::Value) -> (String, String) {
    let message_id = val.get("message_id")
        .or_else(|| val.get("message").and_then(|m| m.get("id")))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let request_id = val.get("request_id")
        .or_else(|| val.get("requestId"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    (message_id, request_id)
}

pub fn sync_claude_code(
    conn_cache: &mut rusqlite::Connection,
    progress_offset: usize,
    total_files: usize,
    progress_cb: &impl Fn(usize, usize),
    remaining_limit: &mut Option<usize>,
) -> Result<(), rusqlite::Error> {
    let projects_dir = get_claude_projects_dir();
    if !projects_dir.exists() {
        return Ok(());
    }

    log_progress("正在扫描并增量同步 Claude Code 历史会话数据...");

    let mut jsonl_files = Vec::new();
    find_jsonl_files(&projects_dir, &mut jsonl_files);

    let mut session_cache = HashMap::new();
    if let Ok(mut stmt) = conn_cache.prepare("SELECT uuid, last_parsed_idx, last_mtime FROM sessions WHERE source = 'claude_code'") {
        if let Ok(mut rows) = stmt.query([]) {
            while let Ok(Some(row)) = rows.next() {
                if let (Ok(uuid), Ok(idx), Ok(mtime)) = (
                    row.get::<_, String>(0),
                    row.get::<_, i64>(1),
                    row.get::<_, f64>(2),
                ) {
                    session_cache.insert(uuid, (idx, mtime));
                }
            }
        }
    }

    let tx = conn_cache.transaction()?;
    {
        for (idx, file_path) in jsonl_files.into_iter().enumerate() {
            if let Some(ref mut rem) = remaining_limit {
                if *rem == 0 {
                    break;
                }
                *rem -= 1;
            }

            let current_scanned = progress_offset + idx + 1;
            progress_cb(current_scanned, total_files);

            let uuid = match file_path.strip_prefix(&projects_dir) {
                Ok(p) => p.to_string_lossy().to_string(),
                Err(_) => file_path.file_name().unwrap_or_default().to_string_lossy().to_string(),
            };

            let mtime = match std::fs::metadata(&file_path).and_then(|m| m.modified()) {
                Ok(t) => t
                    .duration_since(std::time::SystemTime::UNIX_EPOCH)
                    .map(|d| d.as_secs_f64())
                    .unwrap_or(0.0),
                Err(_) => 0.0,
            };

            let mut last_parsed_idx = -1i64;
            let mut last_mtime = 0.0f64;
            let mut is_new_session = true;

            if let Some((parsed_idx, m)) = session_cache.get(&uuid) {
                last_parsed_idx = *parsed_idx;
                last_mtime = *m;
                is_new_session = false;
            }

            if !is_new_session && (last_mtime - mtime).abs() < 1e-4 {
                continue;
            }

            if let Ok(file) = File::open(&file_path) {
                let reader = BufReader::new(file);
                let mut line_idx = 0i64;
                let mut new_turns = Vec::new();
                let mut new_details = Vec::new();

                let mut current_user_prompt = None;
                let mut current_failed_commands = Vec::new();
                let mut current_modified_files = std::collections::HashSet::new();
                let mut current_executed_commands = Vec::new();

                for line in reader.lines() {
                    if let Ok(line_str) = line {
                        if line_str.trim().is_empty() {
                            continue;
                        }
                        line_idx += 1;
                        if line_idx <= last_parsed_idx {
                            continue;
                        }

                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line_str) {
                            // 1. 抓取用户原始提问
                            if val.get("type").and_then(|t| t.as_str()) == Some("user") {
                                if let Some(msg) = val.get("message") {
                                    if let Some(content) = msg.get("content").and_then(|c| c.as_str()) {
                                        current_user_prompt = Some(content.to_string());
                                    }
                                }
                            }

                            // 2. 抓取执行命令与失败命令
                            if let Some(attachment) = val.get("attachment") {
                                if let Some(cmd) = attachment.get("command").and_then(|c| c.as_str()) {
                                    current_executed_commands.push(cmd.to_string());
                                    let exit_code = attachment.get("exitCode").and_then(|e| e.as_i64()).unwrap_or(0);
                                    if exit_code != 0 {
                                        let stderr = attachment.get("stderr").and_then(|s| s.as_str()).unwrap_or("");
                                        // 限制 stderr 大小在 2000 字符内，避免膨胀
                                        let stderr_trunc = if stderr.len() > 2000 {
                                            format!("{}... [truncated]", &stderr[0..2000])
                                        } else {
                                            stderr.to_string()
                                        };
                                        let item = serde_json::json!({
                                            "command": cmd,
                                            "exit_code": exit_code,
                                            "stderr": stderr_trunc
                                        });
                                        current_failed_commands.push(item);
                                    }
                                }
                            }

                            // 3. 抓取修改的文件
                            if val.get("type").and_then(|t| t.as_str()) == Some("file-history-snapshot") {
                                if let Some(snapshot) = val.get("snapshot") {
                                    if let Some(backups) = snapshot.get("trackedFileBackups").and_then(|b| b.as_object()) {
                                        for file_key in backups.keys() {
                                            current_modified_files.insert(file_key.clone());
                                        }
                                    }
                                }
                            }

                            let (input, _cache_creation, cache_read, output, thinking) = extract_claude_tokens(&val);
                            if input > 0 || output > 0 {
                                let model = extract_claude_model(&val);
                                let timestamp = extract_claude_timestamp(&val);
                                let (message_id, request_id) = extract_claude_ids(&val);
                                let total_input = get_total_input_tokens(&model, input, cache_read);
                                let cost = estimate_cost(&model, total_input, cache_read, output).unwrap_or(0.0);

                                new_turns.push((
                                    line_idx - 1,
                                    model,
                                    total_input,
                                    cache_read,
                                    output,
                                    thinking,
                                    cost,
                                    message_id,
                                    request_id,
                                    timestamp,
                                ));

                                // 智能抓取异常 (如果当前轮次发生了命令报错)
                                if !current_failed_commands.is_empty() {
                                    let exec_cmds_json = serde_json::to_string(&current_executed_commands).unwrap_or_else(|_| "[]".to_string());
                                    let fail_cmds_json = serde_json::to_string(&current_failed_commands).unwrap_or_else(|_| "[]".to_string());
                                    let mod_files_vec: Vec<String> = current_modified_files.iter().cloned().collect();
                                    let mod_files_json = serde_json::to_string(&mod_files_vec).unwrap_or_else(|_| "[]".to_string());

                                    new_details.push((
                                        line_idx - 1,
                                        current_user_prompt.clone(),
                                        exec_cmds_json,
                                        fail_cmds_json,
                                        mod_files_json,
                                    ));
                                }

                                // 转入下一轮，清空当前轮次的累加缓存
                                current_user_prompt = None;
                                current_failed_commands.clear();
                                current_modified_files.clear();
                                current_executed_commands.clear();
                            }
                        }
                    }
                }

                if !new_turns.is_empty() {
                    log_progress(&format!("发现 Claude Code 会话 [{}] 有 {} 条新轮次，正在同步...", uuid, new_turns.len()));
                    SCAN_HAS_CHANGES.with(|c| *c.borrow_mut() = true);
                }

                let title = file_path.file_name().unwrap_or_default().to_string_lossy().to_string();
                let created_at = if !new_turns.is_empty() {
                    new_turns[0].9.clone()
                } else {
                    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
                };

                let dev_name = get_device_name();
                let proj_path = file_path.to_string_lossy().to_string();
                let proj_name = detect_project_name(Some(&proj_path));
                tx.execute(
                    "INSERT INTO sessions (source, uuid, title, created_at, last_parsed_idx, last_mtime, project_path, project_name, device_name)
                     VALUES ('claude_code', ?, ?, ?, ?, ?, ?, ?, ?)
                     ON CONFLICT(source, uuid) DO UPDATE SET
                        last_parsed_idx = excluded.last_parsed_idx,
                        last_mtime = excluded.last_mtime,
                        title = excluded.title,
                        project_path = excluded.project_path,
                        project_name = excluded.project_name,
                        device_name = excluded.device_name",
                    rusqlite::params![uuid, title, created_at, line_idx, mtime, proj_path, proj_name, dev_name],
                )?;

                for turn in &new_turns {
                    tx.execute(
                        "INSERT OR REPLACE INTO turns (source, uuid, idx, model, input_tokens, cached_input_tokens, output_tokens, thinking_tokens, cost_usd, message_id, request_id, timestamp)
                         VALUES ('claude_code', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                        rusqlite::params![uuid, turn.0, turn.1, turn.2, turn.3, turn.4, turn.5, turn.6, turn.7, turn.8, turn.9],
                    )?;
                }

                for details in &new_details {
                    tx.execute(
                        "INSERT OR REPLACE INTO turn_details (source, uuid, idx, user_prompt, executed_commands, failed_commands, modified_files)
                         VALUES ('claude_code', ?, ?, ?, ?, ?, ?)",
                        rusqlite::params![uuid, details.0, details.1, details.2, details.3, details.4],
                    )?;
                }
            }
        }
    }
    tx.commit()?;

    Ok(())
}

pub fn sync_codex(
    conn_cache: &mut rusqlite::Connection,
    progress_offset: usize,
    total_files: usize,
    progress_cb: &impl Fn(usize, usize),
    remaining_limit: &mut Option<usize>,
) -> Result<(), rusqlite::Error> {
    let codex_dir = get_codex_sessions_dir();
    if !codex_dir.exists() {
        return Ok(());
    }

    log_progress("正在扫描并增量同步 Codex CLI 历史会话数据...");

    let default_model = get_codex_config_model();

    let mut jsonl_files = Vec::new();
    find_jsonl_files(&codex_dir, &mut jsonl_files);

    let mut session_cache = HashMap::new();
    if let Ok(mut stmt) = conn_cache.prepare("SELECT uuid, last_parsed_idx, last_mtime FROM sessions WHERE source = 'codex'") {
        if let Ok(mut rows) = stmt.query([]) {
            while let Ok(Some(row)) = rows.next() {
                if let (Ok(uuid), Ok(idx), Ok(mtime)) = (
                    row.get::<_, String>(0),
                    row.get::<_, i64>(1),
                    row.get::<_, f64>(2),
                ) {
                    session_cache.insert(uuid, (idx, mtime));
                }
            }
        }
    }

    let tx = conn_cache.transaction()?;
    {
        for (idx, file_path) in jsonl_files.into_iter().enumerate() {
            if let Some(ref mut rem) = remaining_limit {
                if *rem == 0 {
                    break;
                }
                *rem -= 1;
            }

            let current_scanned = progress_offset + idx + 1;
            progress_cb(current_scanned, total_files);

            let uuid = file_path.file_name().unwrap_or_default().to_string_lossy().to_string();

            let mtime = match std::fs::metadata(&file_path).and_then(|m| m.modified()) {
                Ok(t) => t
                    .duration_since(std::time::SystemTime::UNIX_EPOCH)
                    .map(|d| d.as_secs_f64())
                    .unwrap_or(0.0),
                Err(_) => 0.0,
            };

            let mut last_parsed_idx = -1i64;
            let mut last_mtime = 0.0f64;
            let mut is_new_session = true;

            if let Some((parsed_idx, m)) = session_cache.get(&uuid) {
                last_parsed_idx = *parsed_idx;
                last_mtime = *m;
                is_new_session = false;
            }

            if !is_new_session && (last_mtime - mtime).abs() < 1e-4 {
                continue;
            }

            if let Ok(file) = File::open(&file_path) {
                let reader = BufReader::new(file);
                let mut line_idx = 0i64;
                let mut new_turns = Vec::new();

                for line in reader.lines() {
                    if let Ok(line_str) = line {
                        if line_str.trim().is_empty() {
                            continue;
                        }
                        line_idx += 1;
                        if line_idx <= last_parsed_idx {
                            continue;
                        }

                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line_str) {
                            if let Some((input, cache_read, output, thinking, model)) = extract_codex_tokens_and_model(&val, &default_model) {
                                let timestamp = extract_claude_timestamp(&val);
                                let (message_id, request_id) = extract_claude_ids(&val);
                                let total_input = get_total_input_tokens(&model, input, cache_read);
                                let cost = estimate_cost(&model, total_input, cache_read, output).unwrap_or(0.0);

                                new_turns.push((
                                    line_idx - 1,
                                    model,
                                    total_input,
                                    cache_read,
                                    output,
                                    thinking,
                                    cost,
                                    message_id,
                                    request_id,
                                    timestamp,
                                ));
                            }
                        }
                    }
                }

                if !new_turns.is_empty() {
                    log_progress(&format!("发现 Codex 会话 [{}] 有 {} 条新轮次，正在同步...", uuid, new_turns.len()));
                    SCAN_HAS_CHANGES.with(|c| *c.borrow_mut() = true);
                }

                let title = file_path.file_stem().unwrap_or_default().to_string_lossy().to_string();
                let created_at = if !new_turns.is_empty() {
                    new_turns[0].9.clone()
                } else {
                    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
                };

                let dev_name = get_device_name();
                let proj_path = file_path.to_string_lossy().to_string();
                let proj_name = detect_project_name(Some(&proj_path));
                tx.execute(
                    "INSERT INTO sessions (source, uuid, title, created_at, last_parsed_idx, last_mtime, project_path, project_name, device_name)
                     VALUES ('codex', ?, ?, ?, ?, ?, ?, ?, ?)
                     ON CONFLICT(source, uuid) DO UPDATE SET
                        last_parsed_idx = excluded.last_parsed_idx,
                        last_mtime = excluded.last_mtime,
                        title = excluded.title,
                        project_path = excluded.project_path,
                        project_name = excluded.project_name,
                        device_name = excluded.device_name",
                    rusqlite::params![uuid, title, created_at, line_idx, mtime, proj_path, proj_name, dev_name],
                )?;

                for turn in &new_turns {
                    tx.execute(
                        "INSERT OR REPLACE INTO turns (source, uuid, idx, model, input_tokens, cached_input_tokens, output_tokens, thinking_tokens, cost_usd, message_id, request_id, timestamp)
                         VALUES ('codex', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                        rusqlite::params![uuid, turn.0, turn.1, turn.2, turn.3, turn.4, turn.5, turn.6, turn.7, turn.8, turn.9],
                    )?;
                }
            }
        }
    }
    tx.commit()?;

    Ok(())
}

pub fn get_trae_workspace_dir() -> PathBuf {
    Path::new(&get_user_profile_dir())
        .join("AppData")
        .join("Roaming")
        .join("Trae")
        .join("User")
        .join("workspaceStorage")
}

pub fn get_trae_cn_workspace_dir() -> PathBuf {
    Path::new(&get_user_profile_dir())
        .join("AppData")
        .join("Roaming")
        .join("Trae CN")
        .join("User")
        .join("workspaceStorage")
}

pub fn sync_trae(
    conn_cache: &mut rusqlite::Connection,
    progress_offset: usize,
    total_files: usize,
    progress_cb: &impl Fn(usize, usize),
    remaining_limit: &mut Option<usize>,
) -> Result<(), rusqlite::Error> {
    let trae_dir = get_trae_workspace_dir();
    sync_trae_common(conn_cache, &trae_dir, "trae", progress_offset, total_files, progress_cb, remaining_limit)
}

pub fn sync_trae_cn(
    conn_cache: &mut rusqlite::Connection,
    progress_offset: usize,
    total_files: usize,
    progress_cb: &impl Fn(usize, usize),
    remaining_limit: &mut Option<usize>,
) -> Result<(), rusqlite::Error> {
    let trae_cn_dir = get_trae_cn_workspace_dir();
    sync_trae_common(conn_cache, &trae_cn_dir, "trae_cn", progress_offset, total_files, progress_cb, remaining_limit)
}

fn sync_trae_common(
    conn_cache: &mut rusqlite::Connection,
    workspace_dir: &Path,
    source: &str,
    progress_offset: usize,
    total_files: usize,
    progress_cb: &impl Fn(usize, usize),
    remaining_limit: &mut Option<usize>,
) -> Result<(), rusqlite::Error> {
    if !workspace_dir.exists() {
        return Ok(());
    }

    log_progress(&format!("正在扫描并增量同步 {} 历史会话数据...", if source == "trae" { "Trae" } else { "Trae CN" }));

    let mut ws_dbs = Vec::new();
    if let Ok(entries) = std::fs::read_dir(workspace_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let db_path = path.join("state.vscdb");
                if db_path.exists() {
                    ws_dbs.push(db_path);
                }
            }
        }
    }

    let mut session_cache = HashMap::new();
    if let Ok(mut stmt) = conn_cache.prepare(&format!("SELECT uuid, last_parsed_idx, last_mtime FROM sessions WHERE source = '{}'", source)) {
        if let Ok(mut rows) = stmt.query([]) {
            while let Ok(Some(row)) = rows.next() {
                if let (Ok(uuid), Ok(idx), Ok(mtime)) = (
                    row.get::<_, String>(0),
                    row.get::<_, i64>(1),
                    row.get::<_, f64>(2),
                ) {
                    session_cache.insert(uuid, (idx, mtime));
                }
            }
        }
    }

    let mut session_count = 0;
    let tx = conn_cache.transaction()?;
    {
        for (ws_idx, db_path) in ws_dbs.iter().enumerate() {
            if let Some(ref mut rem) = remaining_limit {
                if *rem == 0 {
                    break;
                }
                *rem -= 1;
            }
            let temp_db_path = db_path.with_extension("vscdb.tmp");
            let _guard = if std::fs::copy(db_path, &temp_db_path).is_ok() {
                Some(TempFileGuard { path: temp_db_path.clone() })
            } else {
                None
            };
            let target_db = if _guard.is_some() { &temp_db_path } else { db_path };

            let conn_ws = match rusqlite::Connection::open_with_flags(
                target_db,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
                    | rusqlite::OpenFlags::SQLITE_OPEN_URI
                    | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
            ) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let table_check: Result<i32, _> = conn_ws.query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='ItemTable'",
                [],
                |_| Ok(1),
            );
            if table_check.is_err() {
                continue;
            }

            let storage_val: Result<String, _> = conn_ws.query_row(
                "SELECT value FROM ItemTable WHERE key = 'memento/icube-ai-agent-storage'",
                [],
                |row| row.get(0),
            );
            let storage_json: serde_json::Value = match storage_val {
                Ok(val) => serde_json::from_str(&val).unwrap_or(serde_json::Value::Null),
                Err(_) => continue,
            };

            let sessions = match storage_json.get("list").and_then(|l| l.as_array()) {
                Some(l) => l,
                None => continue,
            };

            if sessions.is_empty() {
                continue;
            }

            let agent_val: Result<String, _> = conn_ws.query_row(
                "SELECT value FROM ItemTable WHERE key = 'icube_session_agent_map'",
                [],
                |row| row.get(0),
            );
            let agent_map: HashMap<String, String> = agent_val
                .ok()
                .and_then(|val| serde_json::from_str(&val).ok())
                .unwrap_or_default();

            let mut session_model_map = HashMap::new();
            if let Ok(mut stmt) = conn_ws.prepare("SELECT key, value FROM ItemTable WHERE key LIKE '%_ai-chat:sessionRelation:modelMap'") {
                if let Ok(mut rows) = stmt.query([]) {
                    while let Some(row) = rows.next().ok().flatten() {
                        if let (Ok(_key), Ok(val_str)) = (row.get::<_, String>(0), row.get::<_, String>(1)) {
                            if let Ok(m_data) = serde_json::from_str::<serde_json::Value>(&val_str) {
                                if let Some(obj) = m_data.as_object() {
                                    for (sess_id, agents) in obj {
                                        if let Some(agent_obj) = agents.as_object() {
                                            for (agent_name, model_raw) in agent_obj {
                                                if let Some(model_str) = model_raw.as_str() {
                                                    let mut model_name = model_str.to_string();
                                                    if model_str.contains("1_-_") {
                                                        if let Some(part) = model_str.split("1_-_").nth(1) {
                                                            model_name = part.to_string();
                                                        }
                                                    }
                                                    session_model_map.insert((sess_id.clone(), agent_name.clone()), model_name);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            let chat_val: Result<String, _> = conn_ws.query_row(
                "SELECT value FROM ItemTable WHERE key = 'ChatStore'",
                [],
                |row| row.get(0),
            );
            let mut turns_count_map = HashMap::new();
            if let Ok(val) = chat_val {
                if let Ok(chat_data) = serde_json::from_str::<serde_json::Value>(&val) {
                    if let Some(turns_height) = chat_data.get("state").and_then(|s| s.get("turnsHeight")).and_then(|t| t.as_object()) {
                        for turn_key in turns_height.keys() {
                            if turn_key.contains('-') {
                                if let Some(sess_id) = turn_key.split('-').next() {
                                    *turns_count_map.entry(sess_id.to_string()).or_insert(0) += 1;
                                }
                            }
                        }
                    }
                }
            }

            let mtime = match std::fs::metadata(db_path).and_then(|m| m.modified()) {
                Ok(t) => t
                    .duration_since(std::time::SystemTime::UNIX_EPOCH)
                    .map(|d| d.as_secs_f64())
                    .unwrap_or(0.0),
                Err(_) => 0.0,
            };

            for (sess_idx, s) in sessions.iter().enumerate() {
                let sess_id = match s.get("sessionId").and_then(|v| v.as_str()) {
                    Some(id) => id.to_string(),
                    None => continue,
                };

                let mut last_mtime = 0.0f64;
                let mut is_new_session = true;

                if let Some((_, m)) = session_cache.get(&sess_id) {
                    last_mtime = *m;
                    is_new_session = false;
                }

                if !is_new_session && (last_mtime - mtime).abs() < 1e-4 {
                    continue;
                }

                let agent_type = agent_map.get(&sess_id).cloned().unwrap_or_else(|| "unknown".to_string());
                let model = session_model_map
                    .get(&(sess_id.clone(), agent_type.clone()))
                    .cloned()
                    .unwrap_or_else(|| "doubao-pro".to_string());

                let turns = turns_count_map.get(&sess_id).cloned().unwrap_or(1);
                let title = format!("{} 会话 ({})", if source == "trae" { "Trae" } else { "Trae CN" }, &sess_id[..6]);

                let offset_secs = (sessions.len() - sess_idx) as i64 * 300;
                let created_at_ts = (mtime as i64) - offset_secs;
                let created_at = if let Some(dt) = DateTime::from_timestamp(created_at_ts, 0) {
                    dt.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
                } else {
                    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
                };

                let input_tokens = 8000;
                let output_tokens = 500;
                let cost = estimate_cost(&model, input_tokens * (turns as i64), 0, output_tokens * (turns as i64)).unwrap_or(0.0);

                tx.execute(
                    &format!("DELETE FROM turns WHERE source = '{}' AND uuid = ?", source),
                    [&sess_id],
                )?;

                let dev_name = get_device_name();
                let proj_path = db_path.to_string_lossy().to_string();
                let proj_name = detect_project_name(Some(&proj_path));
                tx.execute(
                    "INSERT INTO sessions (source, uuid, title, created_at, last_parsed_idx, last_mtime, project_path, project_name, device_name)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                     ON CONFLICT(source, uuid) DO UPDATE SET
                        last_parsed_idx = excluded.last_parsed_idx,
                        last_mtime = excluded.last_mtime,
                        title = excluded.title,
                        project_path = excluded.project_path,
                        project_name = excluded.project_name,
                        device_name = excluded.device_name",
                    rusqlite::params![
                        source,
                        sess_id,
                        title,
                        created_at,
                        turns as i64,
                        mtime,
                        proj_path,
                        proj_name,
                        dev_name,
                    ],
                )?;

                for t_idx in 0..turns {
                    let turn_timestamp = if let Some(dt) = DateTime::from_timestamp(created_at_ts + (t_idx * 30) as i64, 0) {
                        dt.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
                    } else {
                        created_at.clone()
                    };

                    tx.execute(
                        "INSERT OR REPLACE INTO turns (source, uuid, idx, model, input_tokens, cached_input_tokens, output_tokens, thinking_tokens, cost_usd, message_id, request_id, timestamp, latency, tps)
                         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                        rusqlite::params![
                            source,
                            sess_id,
                            t_idx as i64,
                            model,
                            input_tokens,
                            0i64,
                            output_tokens,
                            0i64,
                            cost / (turns as f64),
                            format!("{}-{}", sess_id, t_idx),
                            "unknown",
                            turn_timestamp,
                            0.0f64,
                            0.0f64,
                        ],
                    )?;
                }
                session_count += 1;
            }

            let current_scanned = progress_offset + ws_idx + 1;
            progress_cb(current_scanned, total_files);
        }
    }
    tx.commit()?;

    if session_count > 0 {
        log_progress(&format!("成功增量同步了 {} 的 {} 个会话记录。", if source == "trae" { "Trae" } else { "Trae CN" }, session_count));
        SCAN_HAS_CHANGES.with(|c| *c.borrow_mut() = true);
    }

    Ok(())
}

struct TempFileGuard {
    path: PathBuf,
}

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        if self.path.exists() {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

pub fn get_cursor_db_path() -> PathBuf {
    Path::new(&get_user_profile_dir())
        .join("AppData")
        .join("Roaming")
        .join("Cursor")
        .join("User")
        .join("globalStorage")
        .join("state.vscdb")
}

pub fn sync_cursor(
    conn_cache: &mut rusqlite::Connection,
    progress_offset: usize,
    total_files: usize,
    progress_cb: &impl Fn(usize, usize),
    remaining_limit: &mut Option<usize>,
) -> Result<(), rusqlite::Error> {
    let cursor_db = get_cursor_db_path();
    if !cursor_db.exists() {
        return Ok(());
    }

    log_progress("正在扫描并增量同步 Cursor 编辑器历史会话数据...");

    let temp_db_path = cursor_db.with_extension("vscdb.tmp");
    let _guard = if std::fs::copy(&cursor_db, &temp_db_path).is_ok() {
        Some(TempFileGuard { path: temp_db_path.clone() })
    } else {
        None
    };

    let target_db = if _guard.is_some() { &temp_db_path } else { &cursor_db };

    // 使用只读标志打开 Cursor 的 SQLite 数据库，避免占用文件锁
    let conn_cursor = match rusqlite::Connection::open_with_flags(
        target_db,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
            | rusqlite::OpenFlags::SQLITE_OPEN_URI
            | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) {
        Ok(c) => c,
        Err(e) => {
            log_progress(&format!("打开 Cursor 数据库失败 (可能正被独占锁定): {}", e));
            return Ok(());
        }
    };

    // 预载已缓存的 Cursor 会话，以进行增量同步判断
    let mut session_cache = HashMap::new();
    if let Ok(mut stmt) = conn_cache.prepare("SELECT uuid, last_parsed_idx, last_mtime FROM sessions WHERE source = 'cursor'") {
        if let Ok(mut rows) = stmt.query([]) {
            while let Ok(Some(row)) = rows.next() {
                if let (Ok(uuid), Ok(idx), Ok(mtime)) = (
                    row.get::<_, String>(0),
                    row.get::<_, i64>(1),
                    row.get::<_, f64>(2),
                ) {
                    session_cache.insert(uuid, (idx, mtime));
                }
            }
        }
    }

    // 从 cursorDiskKV 表中读取所有的 composerData 会话数据
    let mut stmt = match conn_cursor.prepare("SELECT key, value FROM cursorDiskKV WHERE key LIKE 'composerData:%'") {
        Ok(s) => s,
        Err(e) => {
            log_progress(&format!("查询 Cursor 数据库表失败 (可能表未初始化): {}", e));
            return Ok(());
        }
    };
    
    let mut rows = stmt.query([])?;
    let mut composer_sessions = Vec::new();
    while let Some(row) = rows.next()? {
        let key: String = row.get(0)?;
        let val: String = row.get(1)?;
        composer_sessions.push((key, val));
    }

    let tx = conn_cache.transaction()?;
    {
        for (session_idx, (key, val)) in composer_sessions.into_iter().enumerate() {
            if let Some(ref mut rem) = remaining_limit {
                if *rem == 0 {
                    break;
                }
                *rem -= 1;
            }

            let current_scanned = progress_offset + session_idx + 1;
            progress_cb(current_scanned, total_files);

            let composer_id = key.trim_start_matches("composerData:").to_string();
            let data: serde_json::Value = match serde_json::from_str(&val) {
                Ok(d) => d,
                Err(_) => continue,
            };

            // 提取会话标题
            let mut title = data.get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("未命名 Composer 会话")
                .to_string();
            
            // 修复部分乱码
            if title.contains('\u{0000}') {
                title = "Composer 会话".to_string();
            }

            // 提取最后修改时间（毫秒级时间戳转换为秒级浮点数）
            let last_updated = data.get("lastUpdatedAt")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0) / 1000.0;

            let mut last_mtime = 0.0f64;
            let mut is_new_session = true;

            if let Some((_, m)) = session_cache.get(&composer_id) {
                last_mtime = *m;
                is_new_session = false;
            }

            // 增量比较：如果 lastUpdatedAt 未变且不是新会话，则直接跳过
            if !is_new_session && (last_mtime - last_updated).abs() < 1e-4 {
                continue;
            }

            let headers: Vec<serde_json::Value> = data.get("fullConversationHeadersOnly")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            let mut new_turns = Vec::new();
            let mut idx = 0;

            for (h_idx, header) in headers.iter().enumerate() {
                let bubble_id = match header.get("bubbleId").and_then(|v| v.as_str()) {
                    Some(b) => b,
                    None => continue,
                };

                let bubble_key = format!("bubbleId:{}:{}", composer_id, bubble_id);
                let bubble_val: Result<String, _> = conn_cursor.query_row(
                    "SELECT value FROM cursorDiskKV WHERE key = ?",
                    [&bubble_key],
                    |row| row.get(0),
                );

                if let Ok(b_str) = bubble_val {
                    if let Ok(b_json) = serde_json::from_str::<serde_json::Value>(&b_str) {
                        let bubble_type = b_json.get("type").and_then(|v| v.as_i64()).unwrap_or(0);
                        if bubble_type == 2 { // Assistant 气泡才算有效交互轮次
                            let token_count = b_json.get("tokenCount");
                            let input_tokens = token_count.and_then(|tc| tc.get("inputTokens").and_then(|v| v.as_i64())).unwrap_or(0);
                            let output_tokens = token_count.and_then(|tc| tc.get("outputTokens").and_then(|v| v.as_i64())).unwrap_or(0);

                            let mut model = b_json.get("modelInfo")
                                .and_then(|mi| mi.get("modelName").and_then(|v| v.as_str()))
                                .unwrap_or("default")
                                .to_string();

                            if model == "default" {
                                model = data.get("modelConfig")
                                    .and_then(|mc| mc.get("modelName").and_then(|v| v.as_str()))
                                    .unwrap_or("default")
                                    .to_string();
                            }

                            if model == "default" || model.is_empty() {
                                model = "claude-3-5-sonnet".to_string();
                            }

                            if input_tokens > 0 || output_tokens > 0 {
                                let timestamp_ms = b_json.get("createdAt")
                                    .and_then(|v| v.as_i64())
                                    .unwrap_or(data.get("createdAt").and_then(|v| v.as_i64()).unwrap_or(0));

                                let timestamp = if timestamp_ms > 0 {
                                    if let Some(dt) = DateTime::from_timestamp(timestamp_ms / 1000, (timestamp_ms % 1000) as u32 * 1_000_000) {
                                        dt.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
                                    } else {
                                        chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
                                    }
                                } else {
                                    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
                                };

                                // 计算交互延迟 Latency 和 TPS
                                let mut latency = 0.0;
                                let mut tps = 0.0;

                                if h_idx > 0 {
                                    let prev_header = &headers[h_idx - 1];
                                    let prev_type = prev_header.get("type").and_then(|v| v.as_i64()).unwrap_or(0);
                                    if prev_type == 1 { // 上一轮是 User 输入
                                        if let Some(prev_bubble_id) = prev_header.get("bubbleId").and_then(|v| v.as_str()) {
                                            let prev_key = format!("bubbleId:{}:{}", composer_id, prev_bubble_id);
                                            let prev_val: Result<String, _> = conn_cursor.query_row(
                                                "SELECT value FROM cursorDiskKV WHERE key = ?",
                                                [&prev_key],
                                                |row| row.get(0),
                                            );
                                            if let Ok(p_str) = prev_val {
                                                if let Ok(p_json) = serde_json::from_str::<serde_json::Value>(&p_str) {
                                                    let prev_created_ms = p_json.get("createdAt").and_then(|v| v.as_i64()).unwrap_or(0);
                                                    if prev_created_ms > 0 && timestamp_ms > prev_created_ms {
                                                        latency = (timestamp_ms - prev_created_ms) as f64 / 1000.0;
                                                        if latency > 0.0 {
                                                            tps = output_tokens as f64 / latency;
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                let cost = estimate_cost(&model, input_tokens, 0, output_tokens).unwrap_or(0.0);
                                let message_id = bubble_id.to_string();
                                let request_id = b_json.get("requestId").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();

                                new_turns.push((
                                    idx,
                                    model,
                                    input_tokens,
                                    0i64, // cached_input_tokens
                                    output_tokens,
                                    0i64, // thinking_tokens
                                    cost,
                                    message_id,
                                    request_id,
                                    timestamp,
                                    latency,
                                    tps,
                                ));
                                idx += 1;
                            }
                        }
                    }
                }
            }

            tx.execute("DELETE FROM turns WHERE source = 'cursor' AND uuid = ?", [&composer_id])?;

            let created_at_ms = data.get("createdAt").and_then(|v| v.as_i64()).unwrap_or(0);
            let created_at = if created_at_ms > 0 {
                if let Some(dt) = DateTime::from_timestamp(created_at_ms / 1000, (created_at_ms % 1000) as u32 * 1_000_000) {
                    dt.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
                } else {
                    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
                }
            } else {
                chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
            };

            let dev_name = get_device_name();
            let proj_path = cursor_db.to_string_lossy().to_string();
            let proj_name = detect_project_name(Some(&proj_path));
            tx.execute(
                "INSERT INTO sessions (source, uuid, title, created_at, last_parsed_idx, last_mtime, project_path, project_name, device_name)
                 VALUES ('cursor', ?, ?, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT(source, uuid) DO UPDATE SET
                    last_parsed_idx = excluded.last_parsed_idx,
                    last_mtime = excluded.last_mtime,
                    title = excluded.title,
                    project_path = excluded.project_path,
                    project_name = excluded.project_name,
                    device_name = excluded.device_name",
                rusqlite::params![
                    composer_id,
                    title,
                    created_at,
                    idx as i64,
                    last_updated,
                    proj_path,
                    proj_name,
                    dev_name,
                ],
            )?;

            for turn in &new_turns {
                tx.execute(
                    "INSERT OR REPLACE INTO turns (source, uuid, idx, model, input_tokens, cached_input_tokens, output_tokens, thinking_tokens, cost_usd, message_id, request_id, timestamp, latency, tps)
                     VALUES ('cursor', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                    rusqlite::params![
                        composer_id,
                        turn.0,
                        turn.1, // model
                        turn.2, // input_tokens
                        turn.3, // cached_input_tokens
                        turn.4, // output_tokens
                        turn.5, // thinking_tokens
                        turn.6, // cost_usd
                        turn.7, // message_id
                        turn.8, // request_id
                        turn.9, // timestamp
                        turn.10, // latency
                        turn.11, // tps
                    ],
                )?;
            }

            if !new_turns.is_empty() {
                SCAN_HAS_CHANGES.with(|c| *c.borrow_mut() = true);
            }
        }
    }
    tx.commit()?;

    Ok(())
}

pub fn sync_cache_db_with_progress<F>(progress_cb: F) -> Result<(), rusqlite::Error>
where
    F: Fn(usize, usize) + Send + 'static,
{
    // 获取全局数据库锁，避免多线程写入冲突
    let _lock = DB_LOCK.lock().unwrap();

    let config = crate::config::load_config();
    let mut remaining_limit = if config.developer_mode { Some(20) } else { None };

    // 1. 扫描 Antigravity 物理文件
    let db_dir = get_conversations_dir();
    let mut db_files = Vec::new();
    let mut active_uuids = std::collections::HashSet::new();

    if db_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&db_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("db") {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        active_uuids.insert(stem.to_string());
                        db_files.push(path);
                    }
                }
            }
        }
    }

    // 2. 扫描 Claude Code 物理文件
    let projects_dir = get_claude_projects_dir();
    let mut claude_files = Vec::new();
    if projects_dir.exists() {
        find_jsonl_files(&projects_dir, &mut claude_files);
    }

    // 3. 扫描 Codex 物理文件
    let codex_dir = get_codex_sessions_dir();
    let mut codex_files = Vec::new();
    if codex_dir.exists() {
        find_jsonl_files(&codex_dir, &mut codex_files);
    }

    // 4. 检测 Cursor 数据库
    let cursor_db = get_cursor_db_path();
    let has_cursor = cursor_db.exists();

    // 5. 检测 Trae 与 Trae CN 工作区目录
    let trae_dir = get_trae_workspace_dir();
    let mut trae_files_count = 0;
    if trae_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&trae_dir) {
            for entry in entries.flatten() {
                if entry.path().join("state.vscdb").exists() {
                    trae_files_count += 1;
                }
            }
        }
    }

    let trae_cn_dir = get_trae_cn_workspace_dir();
    let mut trae_cn_files_count = 0;
    if trae_cn_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&trae_cn_dir) {
            for entry in entries.flatten() {
                if entry.path().join("state.vscdb").exists() {
                    trae_cn_files_count += 1;
                }
            }
        }
    }

    // 计算总文件数
    let total_files = db_files.len() + claude_files.len() + codex_files.len() + if has_cursor { 1 } else { 0 } + trae_files_count + trae_cn_files_count;
    progress_cb(0, total_files);

    let msg = format!("发现待同步物理数据源共 {} 个（Antigravity: {}, Claude Code: {}, Codex: {}, Cursor: {}, Trae: {}, Trae CN: {}）", 
        total_files, db_files.len(), claude_files.len(), codex_files.len(), if has_cursor { 1 } else { 0 }, trae_files_count, trae_cn_files_count);
    log_progress(&msg);


    let cache_path = get_db_cache_path();
    let mut conn_cache = rusqlite::Connection::open(&cache_path)?;

    // 启用 WAL 模式和 synchronous=NORMAL
    let _ = conn_cache.execute("PRAGMA journal_mode=WAL;", []);
    let _ = conn_cache.execute("PRAGMA synchronous=NORMAL;", []);

    // A. 自动同步逻辑：清理已被物理删除的 Antigravity 会话
    let cached_uuids: std::collections::HashSet<String> = {
        let mut stmt = conn_cache.prepare("SELECT uuid FROM sessions WHERE source = 'antigravity'")?;
        let x: std::collections::HashSet<String> = stmt
            .query_map([], |row| row.get(0))?
            .flatten()
            .collect();
        x
    };

    let deleted_uuids: Vec<String> = cached_uuids.difference(&active_uuids).cloned().collect();

    if !deleted_uuids.is_empty() {
        let tx = conn_cache.transaction()?;
        {
            for uuid in &deleted_uuids {
                tx.execute("DELETE FROM sessions WHERE source = 'antigravity' AND uuid = ?", [uuid])?;
                tx.execute("DELETE FROM turns WHERE source = 'antigravity' AND uuid = ?", [uuid])?;
            }
        }
        tx.commit()?;
        SCAN_HAS_CHANGES.with(|c| *c.borrow_mut() = true);
    }

    // B. 预载 Antigravity 会话缓存
    let mut session_cache = HashMap::new();
    if let Ok(mut stmt) = conn_cache.prepare("SELECT uuid, last_parsed_idx, last_mtime, title FROM sessions WHERE source = 'antigravity'") {
        if let Ok(mut rows) = stmt.query([]) {
            while let Ok(Some(row)) = rows.next() {
                if let (Ok(uuid), Ok(idx), Ok(mtime), Ok(title)) = (
                    row.get::<_, String>(0),
                    row.get::<_, i64>(1),
                    row.get::<_, f64>(2),
                    row.get::<_, String>(3),
                ) {
                    session_cache.insert(uuid, (idx, mtime, title));
                }
            }
        }
    }

    let db_files_len = db_files.len();

    log_progress("正在扫描并增量同步 Antigravity 历史会话数据...");

    // C. 增量同步 Antigravity 数据
    {
        let tx = conn_cache.transaction()?;
        for (i, db_path) in db_files.into_iter().enumerate() {
            if let Some(ref mut rem) = remaining_limit {
                if *rem == 0 {
                    break;
                }
                *rem -= 1;
            }

            let uuid = db_path.file_stem().unwrap().to_str().unwrap().to_string();
            let mtime = match std::fs::metadata(&db_path).and_then(|m| m.modified()) {
                Ok(t) => t
                    .duration_since(std::time::SystemTime::UNIX_EPOCH)
                    .map(|d| d.as_secs_f64())
                    .unwrap_or(0.0),
                Err(_) => 0.0,
            };

            let mut last_parsed_idx = -1i64;
            let mut last_mtime = 0.0f64;
            let mut existing_title = String::new();
            let mut is_new_session = true;

            if let Some((parsed_idx, m, title)) = session_cache.get(&uuid) {
                last_parsed_idx = *parsed_idx;
                last_mtime = *m;
                existing_title = title.clone();
                is_new_session = false;
            }

            if !is_new_session && (last_mtime - mtime).abs() < 1e-4 {
                progress_cb(i + 1, total_files);
                continue;
            }

            let mut new_turns = Vec::new();
            let mut max_idx_in_db = last_parsed_idx;
            let mut read_success = false;

            if let Ok(conn_session) = rusqlite::Connection::open(&db_path) {
                let has_gen_metadata: Result<i32, _> = conn_session.query_row(
                    "SELECT 1 FROM sqlite_master WHERE type='table' AND name='gen_metadata'",
                    [],
                    |_| Ok(1),
                );

                if has_gen_metadata.is_ok() {
                    if let Ok(mut stmt) = conn_session.prepare(
                        "SELECT idx, data FROM gen_metadata WHERE size > 0 AND idx > ? ORDER BY idx ASC",
                    ) {
                        if let Ok(mut rows) = stmt.query([last_parsed_idx]) {
                            read_success = true;
                            while let Ok(Some(row)) = rows.next() {
                                let idx: i64 = row.get(0).unwrap_or(0);
                                if idx > max_idx_in_db {
                                    max_idx_in_db = idx;
                                }
                                let blob: Vec<u8> = match row.get(1) {
                                    Ok(b) => b,
                                    Err(_) => continue,
                                };

                                let mut pos = 0;
                                let len = blob.len();
                                if let Ok(raw_parsed) = parse_protobuf_orig(&blob, &mut pos, len, false) {
                                    let deep_parsed = try_parse_sub_messages(raw_parsed);
                                    let metrics = extract_metrics_from_proto(&deep_parsed);
                                    if !metrics.is_empty() {
                                        let mut uncached = 0;
                                        let mut cached = 0;
                                        let mut output = 0;
                                        let mut thinking = 0;
                                        let model = metrics[0].model.clone();
                                        for m in metrics {
                                            uncached += m.uncached_input;
                                            cached += m.cached_input;
                                            output += m.output;
                                            thinking += m.thinking;
                                        }
                                        new_turns.push((
                                            uuid.clone(),
                                            idx,
                                            model,
                                            uncached,
                                            cached,
                                            output,
                                            thinking,
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if read_success {
                if !new_turns.is_empty() {
                    log_progress(&format!("发现 Antigravity 会话 [{}] 有 {} 条新轮次，正在同步...", uuid, new_turns.len()));
                    SCAN_HAS_CHANGES.with(|c| *c.borrow_mut() = true);
                }

                if is_new_session || existing_title.starts_with("Unknown Session") {
                    let (title, created_at) = extract_convo_info(&uuid, &db_path);
                    let dev_name = get_device_name();
                    tx.execute(
                        "INSERT INTO sessions (source, uuid, title, created_at, last_parsed_idx, last_mtime, device_name) VALUES ('antigravity', ?, ?, ?, ?, ?, ?)
                         ON CONFLICT(source, uuid) DO UPDATE SET
                            title = excluded.title,
                            created_at = excluded.created_at,
                            last_parsed_idx = excluded.last_parsed_idx,
                            last_mtime = excluded.last_mtime,
                            device_name = excluded.device_name",
                        rusqlite::params![uuid, title, created_at, max_idx_in_db, mtime, dev_name],
                    )?;
                } else {
                    let dev_name = get_device_name();
                    tx.execute(
                        "UPDATE sessions SET last_parsed_idx = ?, last_mtime = ?, device_name = ? WHERE source = 'antigravity' AND uuid = ?",
                        rusqlite::params![max_idx_in_db, mtime, dev_name, uuid],
                    )?;
                }

                for turn in &new_turns {
                    let cost = estimate_cost(&turn.2, turn.3 + turn.4, turn.4, turn.5).unwrap_or(0.0);
                    tx.execute(
                        "INSERT OR REPLACE INTO turns (source, uuid, idx, model, input_tokens, cached_input_tokens, output_tokens, thinking_tokens, cost_usd, message_id, request_id, timestamp)
                         VALUES ('antigravity', ?, ?, ?, ?, ?, ?, ?, ?, '', 'unknown', ?)",
                        rusqlite::params![
                            uuid,
                            turn.1,
                            turn.2,
                            turn.3 + turn.4,
                            turn.4,
                            turn.5,
                            turn.6,
                            cost,
                            chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
                        ],
                    )?;
                }
            } else {
                log_progress(&format!("跳过会话 [{}] 的更新：物理数据库目前无法访问或格式不兼容，下次将重新尝试", uuid));
            }
            progress_cb(i + 1, total_files);
        }
        tx.commit()?;
    }

    // D. 增量同步 Claude Code 数据
    let _ = sync_claude_code(&mut conn_cache, db_files_len, total_files, &progress_cb, &mut remaining_limit);

    // E. 增量同步 Codex 数据
    let _ = sync_codex(&mut conn_cache, db_files_len + claude_files.len(), total_files, &progress_cb, &mut remaining_limit);

    // F. 增量同步 Cursor 数据
    let _ = sync_cursor(&mut conn_cache, db_files_len + claude_files.len() + codex_files.len(), total_files, &progress_cb, &mut remaining_limit);

    // G. 增量同步 Trae 数据
    let _ = sync_trae(&mut conn_cache, db_files_len + claude_files.len() + codex_files.len() + if has_cursor { 1 } else { 0 }, total_files, &progress_cb, &mut remaining_limit);

    // H. 增量同步 Trae CN 数据
    let _ = sync_trae_cn(&mut conn_cache, db_files_len + claude_files.len() + codex_files.len() + if has_cursor { 1 } else { 0 } + trae_files_count, total_files, &progress_cb, &mut remaining_limit);

    // H. 在同步结束前，一键重建本地 daily_stats 预聚合缓存表，保证大盘毫秒级查询
    log_progress("正在重建本地大盘预计算聚合缓存...");
    if let Err(e) = rebuild_daily_stats_cache(&conn_cache) {
        log_progress(&format!("重建本地大盘缓存失败: {}", e));
    }
    if let Err(e) = rebuild_project_daily_stats_cache(&conn_cache) {
        log_progress(&format!("重建项目大盘缓存失败: {}", e));
    }
    if let Err(e) = rebuild_sessions_fts(&conn_cache) {
        log_progress(&format!("重建 FTS 缓存失败: {}", e));
    }

    // G. 如果配置了 PostgreSQL 模式，自动将本地 SQLite 增量好的最新数据一键同步至 PostgreSQL
    if let Err(e) = sync_local_to_postgres() {
        log_progress(&format!("同步到 PostgreSQL 失败: {}", e));
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("同步至 PostgreSQL 失败: {}", e),
        ))));
    }

    log_progress("✨ 本地大盘预计算缓存重建完成，系统已就绪！");

    Ok(())
}

// 5. 从缓存数据库获取大盘聚合统计数据

#[derive(Serialize)]
pub struct Totals {
    pub total_input: i64,
    pub total_output: i64,
    pub total_tokens: i64,
    pub total_cached: i64,
    pub total_thinking: i64,
    pub cache_hit_rate: f64,
    pub thinking_ratio: f64,
    pub total_sessions: i64,
    pub total_cost: f64,
}

#[derive(Serialize)]
pub struct DailyTrend {
    pub date: String,
    pub input: i64,
    pub output: i64,
    pub cached: i64,
    pub thinking: i64,
    pub sessions: i64,
}

#[derive(Serialize)]
pub struct MonthlySummary {
    pub month: String,
    pub input: i64,
    pub output: i64,
    pub cached: i64,
    pub thinking: i64,
    pub sessions: i64,
}

#[derive(Serialize)]
pub struct ModelDistribution {
    pub model: String,
    pub input: i64,
    pub output: i64,
    pub cached: i64,
    pub thinking: i64,
    pub total_tokens: i64,
}

#[derive(Serialize)]
pub struct SessionItem {
    pub source: String,
    pub uuid: String,
    pub title: String,
    pub created_at: String,
    pub input: i64,
    pub output: i64,
    pub cached: i64,
    pub thinking: i64,
    pub cost_usd: f64,
    pub models: Vec<String>,
}

#[derive(Serialize)]
pub struct SourceTrend {
    pub date: String,
    pub source: String,
    pub tokens: i64,
    pub cost: f64,
}

#[derive(Serialize)]
pub struct DeviceTrend {
    pub date: String,
    pub device_name: String,
    pub tokens: i64,
    pub cost: f64,
}

#[derive(Serialize)]
pub struct ModelPerformance {
    pub model: String,
    pub avg_latency: f64,
    pub avg_tps: f64,
    pub sample_count: i64,
}

#[derive(Serialize)]
pub struct PerformanceTrend {
    pub date: String,
    pub avg_latency: f64,
    pub avg_tps: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct HotSyncPolicy {
    pub delay_ms: u64,
    pub cpu_usage: f32,
    pub reason: String,
}

pub fn recommend_hot_sync_delay_ms(cpu_usage: f32) -> HotSyncPolicy {
    if cpu_usage >= 85.0 {
        HotSyncPolicy {
            delay_ms: 60000,
            cpu_usage,
            reason: "High CPU usage (>= 85%)".to_string(),
        }
    } else {
        HotSyncPolicy {
            delay_ms: 5000,
            cpu_usage,
            reason: "Normal CPU usage".to_string(),
        }
    }
}

pub fn current_hot_sync_policy() -> HotSyncPolicy {
    use sysinfo::System;
    static SYSTEM: std::sync::OnceLock<std::sync::Mutex<System>> = std::sync::OnceLock::new();
    let mutex = SYSTEM.get_or_init(|| {
        let mut sys = System::new();
        sys.refresh_cpu();
        std::sync::Mutex::new(sys)
    });
    let cpu_usage = {
        let mut sys = mutex.lock().unwrap();
        sys.refresh_cpu();
        sys.global_cpu_info().cpu_usage()
    };
    recommend_hot_sync_delay_ms(cpu_usage)
}

pub fn list_model_pricing_rows() -> Result<Vec<ModelPricingRow>, rusqlite::Error> {
    let conn = rusqlite::Connection::open(get_db_cache_path())?;
    let mut stmt = conn.prepare(
        "SELECT id, model_pattern, input_price_per_million, cached_input_price_per_million, output_price_per_million, priority, enabled, updated_at 
         FROM model_pricing 
         ORDER BY priority ASC, id ASC"
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(ModelPricingRow {
            id: Some(row.get(0)?),
            model_pattern: row.get(1)?,
            input_price_per_million: row.get(2)?,
            cached_input_price_per_million: row.get(3)?,
            output_price_per_million: row.get(4)?,
            priority: row.get(5)?,
            enabled: row.get::<_, i32>(6)? != 0,
            updated_at: row.get(7)?,
        })
    })?.collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn recalculate_all_turns_costs(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    let mut stmt = conn.prepare("SELECT source, uuid, idx, model, input_tokens, cached_input_tokens, output_tokens FROM turns")?;
    let mut rows = stmt.query([])?;
    let mut updates = Vec::new();
    while let Some(row) = rows.next()? {
        let source: String = row.get(0)?;
        let uuid: String = row.get(1)?;
        let idx: i64 = row.get(2)?;
        let model: String = row.get(3)?;
        let input: i64 = row.get(4)?;
        let cached: i64 = row.get(5)?;
        let output: i64 = row.get(6)?;
        
        let cost = estimate_cost(&model, input, cached, output).unwrap_or(0.0);
        updates.push((source, uuid, idx, cost));
    }

    let mut stmt_update = conn.prepare("UPDATE turns SET cost_usd = ? WHERE source = ? AND uuid = ? AND idx = ?")?;
    for (source, uuid, idx, cost) in updates {
        stmt_update.execute(rusqlite::params![cost, source, uuid, idx])?;
    }
    Ok(())
}

pub fn upsert_model_pricing_rows(rows: &[ModelPricingRow]) -> Result<(), rusqlite::Error> {
    let mut conn = rusqlite::Connection::open(get_db_cache_path())?;
    {
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM model_pricing", [])?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO model_pricing (model_pattern, input_price_per_million, cached_input_price_per_million, output_price_per_million, priority, enabled, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?)"
            )?;
            for r in rows {
                stmt.execute(rusqlite::params![
                    r.model_pattern,
                    r.input_price_per_million,
                    r.cached_input_price_per_million,
                    r.output_price_per_million,
                    r.priority,
                    if r.enabled { 1 } else { 0 },
                    chrono::Utc::now().to_rfc3339()
                ])?;
            }
        }
        tx.commit()?;
    }

    recalculate_all_turns_costs(&conn)?;
    rebuild_daily_stats_cache(&conn)?;
    rebuild_project_daily_stats_cache(&conn)?;
    Ok(())
}

#[derive(Serialize)]
pub struct PaginatedSessions {
    pub items: Vec<SessionItem>,
    pub total: i64,
    pub page: usize,
    pub page_size: usize,
}

pub fn get_sessions_paginated(
    page: usize,
    page_size: usize,
    search: Option<&str>,
    source_filter: Option<&str>,
    sort_by: Option<&str>,
    sort_order: Option<&str>,
    start_date: Option<&str>,
    end_date: Option<&str>,
    hide_zero: bool,
) -> Result<PaginatedSessions, rusqlite::Error> {
    let db_path = get_db_cache_path();
    let conn = rusqlite::Connection::open(&db_path)?;

    let mut conditions = Vec::new();
    let mut params = Vec::new();

    if let Some(src) = source_filter {
        if src != "all" && !src.is_empty() {
            conditions.push("s.source = ?");
            params.push(rusqlite::types::Value::Text(src.to_string()));
        }
    }

    if let Some(start) = start_date {
        if !start.is_empty() {
            conditions.push("s.created_at >= ?");
            params.push(rusqlite::types::Value::Text(format!("{}T00:00:00", start)));
        }
    }

    if let Some(end) = end_date {
        if !end.is_empty() {
            conditions.push("s.created_at <= ?");
            params.push(rusqlite::types::Value::Text(format!("{}T23:59:59.999", end)));
        }
    }

    if let Some(ref kw) = search {
        let kw_trimmed = kw.trim();
        if !kw_trimmed.is_empty() {
            let clean_kw = kw_trimmed.replace('"', "");
            if !clean_kw.is_empty() {
                conditions.push("EXISTS (SELECT 1 FROM sessions_fts WHERE source = s.source AND uuid = s.uuid AND sessions_fts MATCH ?)");
                let fts_query = format!("\"{}\"*", clean_kw);
                params.push(rusqlite::types::Value::Text(fts_query));
            }
        }
    }

    if hide_zero {
        conditions.push("EXISTS (SELECT 1 FROM turns t WHERE t.source = s.source AND t.uuid = s.uuid AND (COALESCE(t.input_tokens, 0) + COALESCE(t.output_tokens, 0)) > 0)");
    }

    let where_clause = if conditions.is_empty() {
        "".to_string()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    // 1. 第一步：获取总数 (COUNT)
    let sql_count = format!("SELECT COUNT(*) FROM sessions s {}", where_clause);
    let total: i64 = conn.query_row(&sql_count, rusqlite::params_from_iter(params.clone()), |r| r.get(0))?;

    // 2. 第二步：分页查询 Session 基本字段
    let offset = (page.saturating_sub(1)) * page_size;
    
    // 解析排序
    let sort_field = match sort_by.unwrap_or("created_at") {
        "created_at" => "s.created_at",
        "title" => "s.title",
        _ => "s.created_at",
    };
    
    let direction = match sort_order.unwrap_or("desc") {
        "asc" => "ASC",
        _ => "DESC",
    };

    // 3. 第三步：两阶段分页核心 - 先取 ID 分页，再 JOIN turns 聚合
    let sql_list = format!(
        "SELECT 
            s.source,
            s.uuid,
            s.title,
            s.created_at,
            COALESCE(SUM(t.input_tokens), 0) as input,
            COALESCE(SUM(t.output_tokens), 0) as output,
            COALESCE(SUM(t.cached_input_tokens), 0) as cached,
            COALESCE(SUM(t.thinking_tokens), 0) as thinking,
            COALESCE(SUM(t.cost_usd), 0.0) as cost_usd
        FROM (
            SELECT source, uuid, title, created_at 
            FROM sessions s
            {}
            ORDER BY {} {}
            LIMIT ? OFFSET ?
        ) s
        LEFT JOIN turns t ON s.source = t.source AND s.uuid = t.uuid
        GROUP BY s.source, s.uuid, s.title, s.created_at
        ORDER BY {} {}",
        where_clause,
        sort_field, direction,
        sort_field, direction
    );

    let mut query_params = params.clone();
    query_params.push(rusqlite::types::Value::Integer(page_size as i64));
    query_params.push(rusqlite::types::Value::Integer(offset as i64));

    let mut stmt = conn.prepare(&sql_list)?;
    let session_rows = stmt.query_map(rusqlite::params_from_iter(query_params), |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Option<String>>(2)?.unwrap_or_default(),
            r.get::<_, Option<String>>(3)?.unwrap_or_default(),
            r.get::<_, i64>(4)?,
            r.get::<_, i64>(5)?,
            r.get::<_, i64>(6)?,
            r.get::<_, i64>(7)?,
            r.get::<_, f64>(8)?,
        ))
    })?;

    let mut items = Vec::new();
    for row in session_rows {
        let (source, uuid, title, created_at, input, output, cached, thinking, cost_usd) = row?;
        
        // 如果 hide_zero 为真，且 input + output == 0，则忽略
        if hide_zero && (input + output) == 0 {
            continue;
        }

        // 提取该 session 对应的模型列表
        let mut model_stmt = conn.prepare(
            "SELECT DISTINCT model FROM turns WHERE source = ? AND uuid = ? AND model IS NOT NULL AND model != ''"
        )?;
        let models: Vec<String> = model_stmt.query_map([&source, &uuid], |r| r.get(0))?
            .flatten()
            .collect();

        items.push(SessionItem {
            source,
            uuid,
            title,
            created_at,
            input,
            output,
            cached,
            thinking,
            cost_usd,
            models: if models.is_empty() { vec!["unknown".to_string()] } else { models },
        });
    }

    Ok(PaginatedSessions {
        items,
        total,
        page,
        page_size,
    })
}

pub fn get_pg_sessions_paginated(
    page: usize,
    page_size: usize,
    search: Option<&str>,
    source_filter: Option<&str>,
    sort_by: Option<&str>,
    sort_order: Option<&str>,
    start_date: Option<&str>,
    end_date: Option<&str>,
    hide_zero: bool,
) -> Result<PaginatedSessions, String> {
    let _ = dotenvy::dotenv();
    
    let pg_host = std::env::var("DB_PG_HOST").unwrap_or_default();
    let pg_port = std::env::var("DB_PG_PORT").unwrap_or_default();
    let pg_user = std::env::var("DB_PG_USER").unwrap_or_default();
    let pg_password = std::env::var("DB_PG_PASSWORD").unwrap_or_default();
    let pg_database = std::env::var("DB_PG_DATABASE").unwrap_or_default();

    let db_url = if !pg_host.trim().is_empty() {
        format!(
            "postgresql://{}:{}@{}:{}/{}",
            pg_user, pg_password, pg_host, pg_port, pg_database
        )
    } else {
        std::env::var("DATABASE_URL").map_err(|e| format!("未配置 PostgreSQL 数据库 URL: {}", e))?
    };

    let mut config: postgres::config::Config = db_url.parse().map_err(|e: postgres::Error| format!("解析 PostgreSQL 连接 URL 失败: {}", e))?;
    config.connect_timeout(std::time::Duration::from_secs(5));
    let mut pg_client = config.connect(postgres::NoTls).map_err(|e| format!("无法连接到远程 PostgreSQL 数据库: {}", e))?;

    let mut conditions = Vec::new();
    let mut params = Vec::new();
    let mut param_idx = 1;

    if let Some(src) = source_filter {
        if src != "all" && !src.is_empty() {
            conditions.push(format!("s.source = ${}", param_idx));
            params.push(src.to_string());
            param_idx += 1;
        }
    }

    if let Some(start) = start_date {
        if !start.is_empty() {
            conditions.push(format!("s.created_at >= ${}", param_idx));
            params.push(format!("{}T00:00:00", start));
            param_idx += 1;
        }
    }

    if let Some(end) = end_date {
        if !end.is_empty() {
            conditions.push(format!("s.created_at <= ${}", param_idx));
            params.push(format!("{}T23:59:59.999", end));
            param_idx += 1;
        }
    }

    if let Some(ref kw) = search {
        let kw_trimmed = kw.trim();
        if !kw_trimmed.is_empty() {
            conditions.push(format!("(s.title LIKE ${} OR s.uuid LIKE ${})", param_idx, param_idx + 1));
            let like_str = format!("%{}%", kw_trimmed);
            params.push(like_str.clone());
            params.push(like_str);
            param_idx += 2;
        }
    }

    if hide_zero {
        conditions.push("EXISTS (SELECT 1 FROM turns t WHERE t.source = s.source AND t.uuid = s.uuid AND (COALESCE(t.input_tokens, 0) + COALESCE(t.output_tokens, 0)) > 0)".to_string());
    }

    let where_clause = if conditions.is_empty() {
        "".to_string()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let mut pg_params: Vec<&(dyn postgres::types::ToSql + Sync)> = Vec::new();
    for p in &params {
        pg_params.push(p);
    }

    // 1. 第一步：获取总数 (COUNT)
    let sql_count = format!("SELECT COUNT(*) FROM sessions s {}", where_clause);
    let total: i64 = pg_client.query_one(&sql_count, &pg_params[..])
        .map_err(|e| e.to_string())?
        .get(0);

    // 2. 第二步：分页查询 Session 基本字段
    let offset = (page.saturating_sub(1)) * page_size;
    
    let sort_field = match sort_by.unwrap_or("created_at") {
        "created_at" => "s.created_at",
        "title" => "s.title",
        _ => "s.created_at",
    };
    
    let direction = match sort_order.unwrap_or("desc") {
        "asc" => "ASC",
        _ => "DESC",
    };

    // 3. 第三步：延迟分页
    let sql_list = format!(
        "SELECT 
            s.source,
            s.uuid,
            s.title,
            s.created_at,
            CAST(COALESCE(SUM(t.input_tokens), 0) AS BIGINT) as input,
            CAST(COALESCE(SUM(t.output_tokens), 0) AS BIGINT) as output,
            CAST(COALESCE(SUM(t.cached_input_tokens), 0) AS BIGINT) as cached,
            CAST(COALESCE(SUM(t.thinking_tokens), 0) AS BIGINT) as thinking,
            COALESCE(SUM(t.cost_usd), 0.0) as cost_usd
        FROM (
            SELECT source, uuid, title, created_at 
            FROM sessions s
            {}
            ORDER BY {} {}
            LIMIT ${} OFFSET ${}
        ) s
        LEFT JOIN turns t ON s.source = t.source AND s.uuid = t.uuid
        GROUP BY s.source, s.uuid, s.title, s.created_at
        ORDER BY {}",
        where_clause,
        sort_field, direction,
        param_idx, param_idx + 1,
        sort_field
    );

    let page_size_i64 = page_size as i64;
    let offset_i64 = offset as i64;
    let mut pg_query_params = pg_params.clone();
    pg_query_params.push(&page_size_i64);
    pg_query_params.push(&offset_i64);

    let rows = pg_client.query(&sql_list, &pg_query_params[..]).map_err(|e| e.to_string())?;
    
    let mut items = Vec::new();
    for r in rows {
        let source: String = r.get(0);
        let uuid: String = r.get(1);
        let title: Option<String> = r.get(2);
        let created_at: Option<String> = r.get(3);
        let input: i64 = r.get(4);
        let output: i64 = r.get(5);
        let cached: i64 = r.get(6);
        let thinking: i64 = r.get(7);
        let cost_usd: f64 = r.get(8);

        if hide_zero && (input + output) == 0 {
            continue;
        }

        // 提取该 session 对应的模型列表
        let model_rows = pg_client.query(
            "SELECT DISTINCT model FROM turns WHERE source = $1 AND uuid = $2 AND model IS NOT NULL AND model != ''",
            &[&source, &uuid]
        ).map_err(|e| e.to_string())?;
        
        let mut models = Vec::new();
        for mr in model_rows {
            let m: Option<String> = mr.get(0);
            if let Some(ms) = m {
                models.push(ms);
            }
        }

        items.push(SessionItem {
            source,
            uuid,
            title: title.unwrap_or_default(),
            created_at: created_at.unwrap_or_default(),
            input,
            output,
            cached,
            thinking,
            cost_usd,
            models: if models.is_empty() { vec!["unknown".to_string()] } else { models },
        });
    }

    Ok(PaginatedSessions {
        items,
        total,
        page,
        page_size,
    })
}

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

pub fn get_aggregated_metrics_from_cache(
    source_filter: Option<&str>,
    start_date: Option<&str>,
    end_date: Option<&str>,
) -> Result<AggregatedMetrics, rusqlite::Error> {
    let _ = dotenvy::dotenv();
    let db_type = std::env::var("DATABASE_TYPE").unwrap_or_else(|_| "sqlite".to_string());
    if db_type.to_lowercase() == "postgres" {
        match get_pg_aggregated_metrics(source_filter, start_date, end_date) {
            Ok(metrics) => return Ok(metrics),
            Err(e) => {
                eprintln!("[离线容灾] 远程 PostgreSQL 连接测试或读取失败: {}。系统已自动且无缝降级为本地 SQLite 缓存模式！", e);
                // 降级，继续在下方使用本地 SQLite 缓存返回
            }
        }
    }

    let db_path = get_db_cache_path();
    let conn = rusqlite::Connection::open(&db_path)?;


    // 构造动态 SQL WHERE 子句 (面向缓存表 daily_stats)
    let mut conditions_cache = Vec::new();
    let mut params_cache: Vec<rusqlite::types::Value> = Vec::new();

    if let Some(src) = source_filter {
        if src != "all" {
            conditions_cache.push("source = ?");
            params_cache.push(rusqlite::types::Value::Text(src.to_string()));
        }
    }

    if let Some(start) = start_date {
        if !start.is_empty() {
            conditions_cache.push("date >= ?");
            params_cache.push(rusqlite::types::Value::Text(start.to_string()));
        }
    }

    if let Some(end) = end_date {
        if !end.is_empty() {
            conditions_cache.push("date <= ?");
            params_cache.push(rusqlite::types::Value::Text(end.to_string()));
        }
    }

    let where_clause_cache = if conditions_cache.is_empty() {
        "".to_string()
    } else {
        format!("WHERE {}", conditions_cache.join(" AND "))
    };

    // 构造针对项目走势缓存表 project_daily_stats 的动态 SQL WHERE 子句 (排除 source_filter 以防 no such column)
    let mut conditions_project = Vec::new();
    let mut params_project: Vec<rusqlite::types::Value> = Vec::new();

    if let Some(start) = start_date {
        if !start.is_empty() {
            conditions_project.push("date >= ?");
            params_project.push(rusqlite::types::Value::Text(start.to_string()));
        }
    }

    if let Some(end) = end_date {
        if !end.is_empty() {
            conditions_project.push("date <= ?");
            params_project.push(rusqlite::types::Value::Text(end.to_string()));
        }
    }

    let where_clause_project = if conditions_project.is_empty() {
        "".to_string()
    } else {
        format!("WHERE {}", conditions_project.join(" AND "))
    };

    // 构造动态 SQL WHERE 子句 (面向原始关联查询 turns & sessions)
    let mut conditions_raw = Vec::new();
    let mut params_raw: Vec<rusqlite::types::Value> = Vec::new();

    if let Some(src) = source_filter {
        if src != "all" {
            conditions_raw.push("s.source = ?");
            params_raw.push(rusqlite::types::Value::Text(src.to_string()));
        }
    }

    if let Some(start) = start_date {
        if !start.is_empty() {
            conditions_raw.push("s.created_at >= ?");
            params_raw.push(rusqlite::types::Value::Text(format!("{}T00:00:00", start)));
        }
    }

    if let Some(end) = end_date {
        if !end.is_empty() {
            conditions_raw.push("s.created_at <= ?");
            params_raw.push(rusqlite::types::Value::Text(format!("{}T23:59:59.999", end)));
        }
    }

    let where_clause_raw = if conditions_raw.is_empty() {
        "".to_string()
    } else {
        format!("WHERE {}", conditions_raw.join(" AND "))
    };


    // A. Totals 全局指标 (查缓存表)
    let sql_totals = format!(
        "SELECT 
            SUM(input_tokens) as total_input,
            SUM(output_tokens) as total_output,
            SUM(cached_input_tokens) as total_cached,
            SUM(thinking_tokens) as total_thinking,
            SUM(cost_usd) as total_cost,
            SUM(sessions_count) as total_sessions
        FROM daily_stats
        {}",
        where_clause_cache
    );

    let row: Result<(Option<i64>, Option<i64>, Option<i64>, Option<i64>, Option<f64>, Option<i64>), _> = 
        conn.query_row(&sql_totals, rusqlite::params_from_iter(params_cache.clone()), |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
        });

    let (sum_input, sum_output, sum_cached, sum_thinking, sum_cost, sum_sessions) = row.unwrap_or((None, None, None, None, None, None));
    let total_input = sum_input.unwrap_or(0);
    let total_output = sum_output.unwrap_or(0);
    let total_cached = sum_cached.unwrap_or(0);
    let total_thinking = sum_thinking.unwrap_or(0);
    let total_cost = sum_cost.unwrap_or(0.0);
    let total_sessions = sum_sessions.unwrap_or(0);

    let cache_hit_rate = if total_input > 0 {
        total_cached as f64 / total_input as f64
    } else {
        0.0
    };
    let thinking_ratio = if total_output > 0 {
        total_thinking as f64 / total_output as f64
    } else {
        0.0
    };

    let totals = Totals {
        total_input,
        total_output,
        total_tokens: total_input + total_output,
        total_cached,
        total_thinking,
        cache_hit_rate,
        thinking_ratio,
        total_sessions,
        total_cost,
    };

    // B. 每日用量序列 (查缓存表)
    let sql_daily = format!(
        "SELECT 
            date,
            SUM(input_tokens) as input,
            SUM(output_tokens) as output,
            SUM(cached_input_tokens) as cached,
            SUM(thinking_tokens) as thinking,
            SUM(sessions_count) as sessions
        FROM daily_stats
        {}
        GROUP BY date
        ORDER BY date ASC",
        where_clause_cache
    );

    let mut stmt = conn.prepare(&sql_daily)?;
    let mut rows = stmt.query(rusqlite::params_from_iter(params_cache.clone()))?;

    let mut daily_trends = Vec::new();
    while let Some(row) = rows.next()? {
        let date: Option<String> = row.get(0)?;
        let input: Option<i64> = row.get(1)?;
        let output: Option<i64> = row.get(2)?;
        let cached: Option<i64> = row.get(3)?;
        let thinking: Option<i64> = row.get(4)?;
        let sessions: Option<i64> = row.get(5)?;
        daily_trends.push(DailyTrend {
            date: date.unwrap_or_default(),
            input: input.unwrap_or(0),
            output: output.unwrap_or(0),
            cached: cached.unwrap_or(0),
            thinking: thinking.unwrap_or(0),
            sessions: sessions.unwrap_or(0),
        });
    }

    // C. 按月聚合汇总 (查缓存表)
    let sql_monthly = format!(
        "SELECT 
            substr(date, 1, 7) as month,
            SUM(input_tokens) as input,
            SUM(output_tokens) as output,
            SUM(cached_input_tokens) as cached,
            SUM(thinking_tokens) as thinking,
            SUM(sessions_count) as sessions
        FROM daily_stats
        {}
        GROUP BY month
        ORDER BY month DESC",
        where_clause_cache
    );

    let mut stmt = conn.prepare(&sql_monthly)?;
    let mut rows = stmt.query(rusqlite::params_from_iter(params_cache.clone()))?;

    let mut monthly_summary = Vec::new();
    while let Some(row) = rows.next()? {
        let month: Option<String> = row.get(0)?;
        let input: Option<i64> = row.get(1)?;
        let output: Option<i64> = row.get(2)?;
        let cached: Option<i64> = row.get(3)?;
        let thinking: Option<i64> = row.get(4)?;
        let sessions: Option<i64> = row.get(5)?;
        monthly_summary.push(MonthlySummary {
            month: month.unwrap_or_default(),
            input: input.unwrap_or(0),
            output: output.unwrap_or(0),
            cached: cached.unwrap_or(0),
            thinking: thinking.unwrap_or(0),
            sessions: sessions.unwrap_or(0),
        });
    }

    // D. 底层模型分布 (包含 turns 详情, 走 idx_sessions_created_at 索引)
    let sql_model_dist = format!(
        "SELECT 
            t.model as model_mapped,
            SUM(t.input_tokens) as input,
            SUM(t.output_tokens) as output,
            SUM(t.cached_input_tokens) as cached,
            SUM(t.thinking_tokens) as thinking,
            SUM(t.input_tokens + t.output_tokens) as total_tokens
        FROM turns t
        INNER JOIN sessions s ON t.source = s.source AND t.uuid = s.uuid
        {} {}
        GROUP BY model_mapped
        ORDER BY total_tokens DESC",
        where_clause_raw,
        if where_clause_raw.is_empty() { "WHERE t.model IS NOT NULL AND t.model != 'unknown' AND t.model != ''" } else { "AND t.model IS NOT NULL AND t.model != 'unknown' AND t.model != ''" }
    );

    let mut stmt = conn.prepare(&sql_model_dist)?;
    let mut rows = stmt.query(rusqlite::params_from_iter(params_raw.clone()))?;

    let mut model_distribution = Vec::new();
    while let Some(row) = rows.next()? {
        let model: String = row.get(0)?;
        let input: Option<i64> = row.get(1)?;
        let output: Option<i64> = row.get(2)?;
        let cached: Option<i64> = row.get(3)?;
        let thinking: Option<i64> = row.get(4)?;
        let total_tokens: Option<i64> = row.get(5)?;
        model_distribution.push(ModelDistribution {
            model,
            input: input.unwrap_or(0),
            output: output.unwrap_or(0),
            cached: cached.unwrap_or(0),
            thinking: thinking.unwrap_or(0),
            total_tokens: total_tokens.unwrap_or(0),
        });
    }

    // E. 会话详细明细 (方案一：解耦剥离！此处直接返回空，绝不拖累大盘性能)
    let sessions = Vec::new();

    // F. 新增：多引擎用量每日对比走势 (SourceTrends - 查缓存表)
    let sql_source_trends = format!(
        "SELECT 
            date,
            source,
            SUM(input_tokens + output_tokens) as total_tokens,
            SUM(cost_usd) as cost
        FROM daily_stats
        {}
        GROUP BY date, source
        ORDER BY date ASC, source ASC",
        where_clause_cache
    );

    let mut stmt = conn.prepare(&sql_source_trends)?;
    let mut rows = stmt.query(rusqlite::params_from_iter(params_cache.clone()))?;

    let mut source_trends = Vec::new();
    while let Some(row) = rows.next()? {
        let date: Option<String> = row.get(0)?;
        let source: String = row.get(1)?;
        let tokens: Option<i64> = row.get(2)?;
        let cost: Option<f64> = row.get(3)?;
        source_trends.push(SourceTrend {
            date: date.unwrap_or_default(),
            source,
            tokens: tokens.unwrap_or(0),
            cost: cost.unwrap_or(0.0),
        });
    }

    // F2. 新增：多设备用量每日对比走势 (DeviceTrends - 查缓存表)
    let sql_device_trends = format!(
        "SELECT 
            date,
            device_name,
            SUM(input_tokens + output_tokens) as total_tokens,
            SUM(cost_usd) as cost
        FROM daily_stats
        {}
        GROUP BY date, device_name
        ORDER BY date ASC, device_name ASC",
        where_clause_cache
    );

    let mut stmt = conn.prepare(&sql_device_trends)?;
    let mut rows = stmt.query(rusqlite::params_from_iter(params_cache.clone()))?;

    let mut device_trends = Vec::new();
    while let Some(row) = rows.next()? {
        let date: Option<String> = row.get(0)?;
        let device_name: String = row.get(1)?;
        let tokens: Option<i64> = row.get(2)?;
        let cost: Option<f64> = row.get(3)?;
        device_trends.push(DeviceTrend {
            date: date.unwrap_or_default(),
            device_name,
            tokens: tokens.unwrap_or(0),
            cost: cost.unwrap_or(0.0),
        });
    }

    // G. 各模型平均 Latency & TPS (仅考虑 latency > 0.0, 走 idx_sessions_created_at 索引)
    let sql_model_perf = format!(
        "SELECT 
            t.model as model_mapped,
            AVG(t.latency) as avg_latency,
            AVG(t.tps) as avg_tps,
            COUNT(*) as sample_count
        FROM turns t
        INNER JOIN sessions s ON t.source = s.source AND t.uuid = s.uuid
        {} {}
        GROUP BY model_mapped
        ORDER BY sample_count DESC",
        where_clause_raw,
        if where_clause_raw.is_empty() { "WHERE t.latency > 0.0 AND t.model IS NOT NULL AND t.model != ''" } else { "AND t.latency > 0.0 AND t.model IS NOT NULL AND t.model != ''" }
    );

    let mut stmt = conn.prepare(&sql_model_perf)?;
    let mut rows = stmt.query(rusqlite::params_from_iter(params_raw.clone()))?;

    let mut model_performance = Vec::new();
    while let Some(row) = rows.next()? {
        let model: String = row.get(0)?;
        let avg_latency: Option<f64> = row.get(1)?;
        let avg_tps: Option<f64> = row.get(2)?;
        let sample_count: i64 = row.get(3)?;
        model_performance.push(ModelPerformance {
            model,
            avg_latency: avg_latency.unwrap_or(0.0),
            avg_tps: avg_tps.unwrap_or(0.0),
            sample_count,
        });
    }

    // H. 每日性能走势 (仅考虑 latency > 0.0, 走 idx_sessions_created_at 索引)
    let sql_perf_trends = format!(
        "SELECT 
            substr(s.created_at, 1, 10) as date,
            AVG(t.latency) as avg_latency,
            AVG(t.tps) as avg_tps
        FROM sessions s
        LEFT JOIN turns t ON s.source = t.source AND s.uuid = t.uuid
        {} {}
        GROUP BY date
        ORDER BY date ASC",
        where_clause_raw,
        if where_clause_raw.is_empty() { "WHERE t.latency > 0.0" } else { "AND t.latency > 0.0" }
    );

    let mut stmt = conn.prepare(&sql_perf_trends)?;
    let mut rows = stmt.query(rusqlite::params_from_iter(params_raw.clone()))?;

    let mut performance_trends = Vec::new();
    while let Some(row) = rows.next()? {
        let date: Option<String> = row.get(0)?;
        let avg_latency: Option<f64> = row.get(1)?;
        let avg_tps: Option<f64> = row.get(2)?;
        performance_trends.push(PerformanceTrend {
            date: date.unwrap_or_default(),
            avg_latency: avg_latency.unwrap_or(0.0),
            avg_tps: avg_tps.unwrap_or(0.0),
        });
    }

    // F3. 项目每日走势 (ProjectTrends - 查缓存表)
    let sql_project_trends = format!(
        "SELECT 
            date,
            project_name,
            SUM(total_tokens) as tokens,
            SUM(total_cost_usd) as cost
        FROM project_daily_stats
        {}
        GROUP BY date, project_name
        ORDER BY date ASC, project_name ASC",
        where_clause_project
    );

    let mut stmt_proj = conn.prepare(&sql_project_trends)?;
    let mut rows_proj = stmt_proj.query(rusqlite::params_from_iter(params_project.clone()))?;

    let mut project_trends = Vec::new();
    while let Some(row) = rows_proj.next()? {
        let date: Option<String> = row.get(0)?;
        let project_name: String = row.get(1)?;
        let tokens: Option<i64> = row.get(2)?;
        let cost_usd: Option<f64> = row.get(3)?;
        project_trends.push(ProjectTrend {
            date: date.unwrap_or_default(),
            project_name,
            tokens: tokens.unwrap_or(0),
            cost_usd: cost_usd.unwrap_or(0.0),
        });
    }

    // F4. 项目排行 (ProjectRankings - 从 sessions + turns)
    let sql_project_rankings = format!(
        "SELECT 
            COALESCE(NULLIF(s.project_name, ''), 'unknown-project') AS name_proj,
            COALESCE(MAX(s.project_path), '') AS path_proj,
            COALESCE(SUM(t.input_tokens + t.output_tokens), 0) AS total_tokens,
            COALESCE(SUM(t.cost_usd), 0.0) AS total_cost_usd,
            COUNT(DISTINCT s.source || ':' || s.uuid) AS sessions_count
         FROM sessions s
         LEFT JOIN turns t ON s.source = t.source AND s.uuid = t.uuid
         {}
         GROUP BY name_proj
         ORDER BY total_tokens DESC, total_cost_usd DESC
         LIMIT 10",
        where_clause_raw
    );

    let mut stmt_rank = conn.prepare(&sql_project_rankings)?;
    let mut rows_rank = stmt_rank.query(rusqlite::params_from_iter(params_raw.clone()))?;

    let mut project_rankings = Vec::new();
    while let Some(row) = rows_rank.next()? {
        let project_name: String = row.get(0)?;
        let project_path: String = row.get(1)?;
        let total_tokens: i64 = row.get(2)?;
        let total_cost_usd: f64 = row.get(3)?;
        let sessions_count: i64 = row.get(4)?;
        project_rankings.push(ProjectRanking {
            project_name,
            project_path,
            total_tokens,
            total_cost_usd,
            sessions_count,
        });
    }

    let display_currency = get_display_currency();
    let (usd_exchange_rate, exchange_rate_updated_at) = get_exchange_rate(&conn, &display_currency)?;

    Ok(AggregatedMetrics {
        totals,
        daily_trends,
        monthly_summary,
        model_distribution,
        sessions,
        source_trends,
        device_trends,
        project_trends,
        project_rankings,
        model_performance,
        performance_trends,
        display_currency,
        usd_exchange_rate,
        exchange_rate_updated_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;

    #[test]
    fn test_init_cache_db_creates_project_and_pricing_structures() {
        let test_id = chrono::Utc::now().timestamp_millis();
        let temp_path = std::env::temp_dir().join(format!("token_insight_schema_test_{}", test_id));
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

        let _ = std::fs::remove_dir_all(&temp_path);
    }

    #[test]
    fn test_sync_populates_project_name_fts_and_project_daily_stats() {
        let test_id = chrono::Utc::now().timestamp_millis();
        let temp_path = std::env::temp_dir().join(format!("token_insight_project_cache_test_{}", test_id));
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
        sync_claude_code(&mut conn, 0, 1, &|_, _| {}, &mut None).unwrap();
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
            "SELECT COUNT(1) FROM sessions_fts WHERE sessions_fts MATCH '\"demo-repo\"'",
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

        let _ = std::fs::remove_dir_all(&temp_path);
    }

    #[test]
    fn test_estimate_cost() {
        let cost_opus = estimate_cost("claude-3-opus", 1000, 200, 500).unwrap();
        assert!((cost_opus - 0.0498).abs() < 1e-6);

        let cost_sonnet = estimate_cost("claude-3-5-sonnet", 1000, 300, 500).unwrap();
        assert!((cost_sonnet - 0.00969).abs() < 1e-6);

        let cost_flash = estimate_cost("gemini-2.5-flash", 10000, 2000, 5000).unwrap();
        assert!((cost_flash - 0.0021375).abs() < 1e-8);
    }

    #[test]
    fn test_estimate_cost_prefers_model_pricing_table() {
        let test_id = chrono::Utc::now().timestamp_millis();
        let temp_path = std::env::temp_dir().join(format!("token_insight_pricing_test_{}", test_id));
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
        assert!((cost - 16.38).abs() < 1e-6);

        let _ = std::fs::remove_dir_all(&temp_path);
    }

    #[test]
    fn test_aggregated_metrics_include_project_rankings_and_trends() {
        let test_id = chrono::Utc::now().timestamp_millis();
        let temp_path = std::env::temp_dir().join(format!("token_insight_metrics_project_test_{}", test_id));
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
        println!("DEBUG_PROJECTS: {:?}", metrics.project_rankings.iter().map(|p| &p.project_name).collect::<Vec<_>>());
        assert_eq!(metrics.project_rankings.len(), 1);
        assert_eq!(metrics.project_rankings[0].project_name, "repo-a");
        assert_eq!(metrics.project_trends.len(), 1);
        assert_eq!(metrics.project_trends[0].project_name, "repo-a");

        let _ = std::fs::remove_dir_all(&temp_path);
    }

    #[test]
    fn test_sqlite_session_search_uses_fts() {
        let test_id = chrono::Utc::now().timestamp_millis();
        let temp_path = std::env::temp_dir().join(format!("token_insight_fts_search_test_{}", test_id));
        std::fs::create_dir_all(&temp_path).unwrap();
        std::env::set_var("USERPROFILE", temp_path.to_str().unwrap());
        std::env::set_var("DATABASE_TYPE", "sqlite");

        init_cache_db().unwrap();
        let conn = rusqlite::Connection::open(get_db_cache_path()).unwrap();

        conn.execute(
            "INSERT INTO sessions (source, uuid, title, created_at, project_path, project_name, device_name)
             VALUES ('claude_code', 'fts-1', 'Refactor token monitor', '2026-05-28T11:00:00.000Z', 'D:/code/token-insight', 'token-insight', 'devbox')",
            [],
        ).unwrap();
        rebuild_sessions_fts(&conn).unwrap();

        let result = get_sessions_paginated(1, 10, Some("token-insight"), Some("all"), Some("created_at"), Some("desc"), None, None, false).unwrap();
        assert_eq!(result.total, 1);
        assert_eq!(result.items[0].uuid, "fts-1");

        let _ = std::fs::remove_dir_all(&temp_path);
    }

    #[test]
    fn test_recommended_hot_sync_delay_changes_with_load() {
        let low_load = recommend_hot_sync_delay_ms(10.0);
        assert_eq!(low_load.delay_ms, 5000);
        assert_eq!(low_load.reason, "Normal CPU usage");

        let high_load = recommend_hot_sync_delay_ms(90.0);
        assert_eq!(high_load.delay_ms, 60000);
        assert_eq!(high_load.reason, "High CPU usage (>= 85%)");
    }

    #[test]
    fn test_upsert_model_pricing_rows() {
        let test_id = chrono::Utc::now().timestamp_millis();
        let temp_path = std::env::temp_dir().join(format!("token_insight_pricing_crud_test_{}", test_id));
        std::fs::create_dir_all(&temp_path).unwrap();
        std::env::set_var("USERPROFILE", temp_path.to_str().unwrap());
        std::env::set_var("DATABASE_TYPE", "sqlite");

        init_cache_db().unwrap();

        let new_rows = vec![
            ModelPricingRow {
                id: None,
                model_pattern: "gpt-4*".to_string(),
                input_price_per_million: 10.0,
                cached_input_price_per_million: 5.0,
                output_price_per_million: 30.0,
                priority: 5,
                enabled: true,
                updated_at: "".to_string(),
            }
        ];

        upsert_model_pricing_rows(&new_rows).unwrap();

        let loaded = list_model_pricing_rows().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].model_pattern, "gpt-4*");
        assert_eq!(loaded[0].input_price_per_million, 10.0);

        let _ = std::fs::remove_dir_all(&temp_path);
    }

    #[test]
    fn test_extract_claude_helpers() {
        let sample_json = serde_json::json!({
            "timestamp": "2026-05-27T12:00:00.000Z",
            "model": "claude-3-5-sonnet-20241022",
            "message": {
                "id": "msg_12345",
                "usage": {
                    "input_tokens": 120,
                    "output_tokens": 80,
                    "cache_read_input_tokens": 20
                }
            },
            "requestId": "req_abc123"
        });

        let (in_t, c_create, c_read, out_t, think_t) = extract_claude_tokens(&sample_json);
        assert_eq!(in_t, 120);
        assert_eq!(c_create, 0);
        assert_eq!(c_read, 20);
        assert_eq!(out_t, 80);
        assert_eq!(think_t, 0);

        // Test thinking token and nested recursion
        let nested_json = serde_json::json!({
            "deep": {
                "payload": {
                    "inputTokens": 300,
                    "output_tokens": 150,
                    "reasoning_output_tokens": 50
                }
            }
        });
        let (in_nest, _, _, out_nest, think_nest) = extract_claude_tokens(&nested_json);
        assert_eq!(in_nest, 300);
        assert_eq!(out_nest, 150);
        assert_eq!(think_nest, 50);

        let model = extract_claude_model(&sample_json);
        assert_eq!(model, "claude-3-5-sonnet-20241022");

        let timestamp = extract_claude_timestamp(&sample_json);
        assert_eq!(timestamp, "2026-05-27T12:00:00.000Z");

        let (msg_id, req_id) = extract_claude_ids(&sample_json);
        assert_eq!(msg_id, "msg_12345");
        assert_eq!(req_id, "req_abc123");
    }

    #[test]
    fn test_sync_and_aggregate_integration() {
        let test_id = chrono::Utc::now().timestamp_millis();
        let temp_path = std::env::temp_dir().join(format!("token_insight_test_{}", test_id));
        fs::create_dir_all(&temp_path).unwrap();
        
        std::env::set_var("USERPROFILE", temp_path.to_str().unwrap());
        std::env::set_var("DATABASE_TYPE", "sqlite");

        let db_cache_path = get_db_cache_path();
        assert!(!db_cache_path.exists());
        
        let init_res = init_cache_db();
        assert!(init_res.is_ok());
        assert!(db_cache_path.exists());

        let claude_proj_dir = get_claude_projects_dir().join("test-project");
        fs::create_dir_all(&claude_proj_dir).unwrap();
        
        let claude_log_file = claude_proj_dir.join("history.jsonl");
        let mut file = File::create(&claude_log_file).unwrap();
        
        let line_1 = serde_json::json!({
            "timestamp": "2026-05-27T10:00:00.000Z",
            "model": "claude-3-5-sonnet",
            "message": {
                "id": "msg_001",
                "usage": {
                    "input_tokens": 100,
                    "output_tokens": 50,
                    "cache_read_input_tokens": 10
                }
            },
            "requestId": "req_001"
        });
        writeln!(file, "{}", line_1.to_string()).unwrap();
        drop(file);

        let mut conn = rusqlite::Connection::open(&db_cache_path).unwrap();
        sync_claude_code(&mut conn, 0, 1, &|_, _| {}, &mut None).unwrap();
        rebuild_daily_stats_cache(&conn).unwrap();

        let metrics_all = get_aggregated_metrics_from_cache(None, None, None).unwrap();
        assert_eq!(metrics_all.totals.total_sessions, 1);
        assert_eq!(metrics_all.totals.total_input, 100);
        assert_eq!(metrics_all.totals.total_output, 50);
        assert_eq!(metrics_all.totals.total_cached, 10);
        assert!(metrics_all.totals.total_cost > 0.0);

        let metrics_claude = get_aggregated_metrics_from_cache(Some("claude_code"), None, None).unwrap();
        assert_eq!(metrics_claude.totals.total_sessions, 1);
        
        let metrics_antigravity = get_aggregated_metrics_from_cache(Some("antigravity"), None, None).unwrap();
        assert_eq!(metrics_antigravity.totals.total_sessions, 0);

        let mut file_append = fs::OpenOptions::new().append(true).open(&claude_log_file).unwrap();
        let line_2 = serde_json::json!({
            "timestamp": "2026-05-27T10:05:00.000Z",
            "model": "claude-3-5-sonnet",
            "message": {
                "id": "msg_002",
                "usage": {
                    "input_tokens": 200,
                    "output_tokens": 100,
                    "cache_read_input_tokens": 20
                }
            },
            "requestId": "req_002"
        });
        writeln!(file_append, "{}", line_2.to_string()).unwrap();
        drop(file_append);

        sync_claude_code(&mut conn, 0, 1, &|_, _| {}, &mut None).unwrap();
        rebuild_daily_stats_cache(&conn).unwrap();

        let metrics_all_2 = get_aggregated_metrics_from_cache(None, None, None).unwrap();
        assert_eq!(metrics_all_2.totals.total_sessions, 1);
        assert_eq!(metrics_all_2.totals.total_input, 300);
        assert_eq!(metrics_all_2.totals.total_output, 150);
        assert_eq!(metrics_all_2.totals.total_cached, 30);

        let codex_sess_dir = get_codex_sessions_dir();
        fs::create_dir_all(&codex_sess_dir).unwrap();
        
        let codex_log_file = codex_sess_dir.join("rollout-test.jsonl");
        let mut file_codex = File::create(&codex_log_file).unwrap();
        
        let codex_line = serde_json::json!({
            "timestamp": "2026-05-27T11:00:00.000Z",
            "model": "gpt-4o-mini",
            "usage": {
                "input_tokens": 500,
                "output_tokens": 300,
                "cache_read_input_tokens": 50
            },
            "thinking_tokens": 0
        });
        writeln!(file_codex, "{}", codex_line.to_string()).unwrap();

        let codex_line_real = serde_json::json!({
            "timestamp": "2026-05-27T11:05:00.000Z",
            "type": "event_msg",
            "payload": {
                "type": "token_count",
                "info": {
                    "last_token_usage": {
                        "input_tokens": 600,
                        "cached_input_tokens": 80,
                        "output_tokens": 400,
                        "reasoning_output_tokens": 40
                    }
                }
            }
        });
        writeln!(file_codex, "{}", codex_line_real.to_string()).unwrap();
        drop(file_codex);

        sync_codex(&mut conn, 0, 1, &|_, _| {}, &mut None).unwrap();
        rebuild_daily_stats_cache(&conn).unwrap();

        let metrics_all_3 = get_aggregated_metrics_from_cache(None, None, None).unwrap();
        assert_eq!(metrics_all_3.totals.total_sessions, 2);
        assert_eq!(metrics_all_3.totals.total_input, 1530);
        assert_eq!(metrics_all_3.totals.total_output, 850);
        assert_eq!(metrics_all_3.totals.total_cached, 160);

        drop(conn);
        let _ = fs::remove_dir_all(&temp_path);
    }

    #[test]
    fn test_turn_details_table() {
        let test_id = chrono::Utc::now().timestamp_millis();
        let temp_path = std::env::temp_dir().join(format!("token_insight_turn_details_test_{}", test_id));
        std::fs::create_dir_all(&temp_path).unwrap();
        std::env::set_var("USERPROFILE", temp_path.to_str().unwrap());
        std::env::set_var("DATABASE_TYPE", "sqlite");

        init_cache_db().unwrap();

        let conn = rusqlite::Connection::open(get_db_cache_path()).unwrap();
        conn.execute(
            "INSERT INTO turn_details (source, uuid, idx, user_prompt, executed_commands, failed_commands, modified_files)
             VALUES ('claude_code', 'test-uuid-123', 3, 'hello prompt', '[\"git status\"]', '[]', '[\"src/main.rs\"]')",
            []
        ).unwrap();

        {
            let mut stmt = conn.prepare("SELECT user_prompt, executed_commands, modified_files FROM turn_details WHERE source = 'claude_code' AND uuid = 'test-uuid-123' AND idx = 3").unwrap();
            let mut rows = stmt.query([]).unwrap();
            let row = rows.next().unwrap().unwrap();
            let prompt: String = row.get(0).unwrap();
            let exec_cmds: String = row.get(1).unwrap();
            let mod_files: String = row.get(2).unwrap();

            assert_eq!(prompt, "hello prompt");
            assert_eq!(exec_cmds, "[\"git status\"]");
            assert_eq!(mod_files, "[\"src/main.rs\"]");
        }

        drop(conn);
        let _ = std::fs::remove_dir_all(&temp_path);
    }
}

// ==================== PostgreSQL 同步与查询路由代理模块 ====================

pub fn sync_local_to_postgres() -> Result<(), String> {
    let _ = dotenvy::dotenv();
    let db_type = std::env::var("DATABASE_TYPE").unwrap_or_else(|_| "sqlite".to_string());
    if db_type.to_lowercase() != "postgres" {
        return Ok(());
    }

    log_progress("检测到远程 PostgreSQL 模式，正在触发增量同步...");
    
    // 1. 统一提取 PostgreSQL 配置，合成正确的连接 URL，支持拆分字段，与 db_adapter.rs 一致
    let pg_host = std::env::var("DB_PG_HOST").unwrap_or_default();
    let pg_port = std::env::var("DB_PG_PORT").unwrap_or_default();
    let pg_user = std::env::var("DB_PG_USER").unwrap_or_default();
    let pg_password = std::env::var("DB_PG_PASSWORD").unwrap_or_default();
    let pg_database = std::env::var("DB_PG_DATABASE").unwrap_or_default();

    let db_url = if !pg_host.trim().is_empty() {
        format!(
            "postgresql://{}:{}@{}:{}/{}",
            pg_user, pg_password, pg_host, pg_port, pg_database
        )
    } else {
        std::env::var("DATABASE_URL").map_err(|e| format!("未配置 PostgreSQL 数据库 URL: {}", e))?
    };

    // 2. 自动检测并创建数据库
    let _ = crate::db_adapter::ensure_pg_database_exists(&db_url);

    // 3. 使用 Config 配置超时时间并进行连接，快速失败防止无限卡死
    let mut config: postgres::config::Config = db_url.parse().map_err(|e: postgres::Error| format!("解析 PostgreSQL 连接 URL 失败: {}", e))?;
    config.connect_timeout(std::time::Duration::from_secs(5));
    let mut pg_client = config.connect(postgres::NoTls).map_err(|e| format!("无法连接到远程 PostgreSQL 数据库: {}", e))?;

    // 运行 Postgres 数据库迁移以保证表结构最新 (包含新列如 latency 和 tps)
    crate::db_adapter::init_postgres_tables(&mut pg_client).map_err(|e| format!("执行 PostgreSQL 数据库迁移失败: {}", e))?;

    let cache_path = get_db_cache_path();
    let sqlite_conn = rusqlite::Connection::open(&cache_path).map_err(|e| format!("无法打开本地 SQLite 缓存: {}", e))?;

    // 3. 首先拉取远程 PostgreSQL 中已同步的会话最新状态，避免全量低效同步
    let mut pg_sessions = std::collections::HashMap::new();
    let pg_rows = pg_client
        .query("SELECT source, uuid, last_mtime, last_parsed_idx FROM sessions", &[])
        .map_err(|e| format!("读取远程 sessions 失败: {}", e))?;
    for row in pg_rows {
        let source: String = row.get(0);
        let uuid: String = row.get(1);
        let last_mtime: f64 = row.get(2);
        let last_parsed_idx: i64 = row.get(3);
        pg_sessions.insert((source, uuid), (last_mtime, last_parsed_idx));
    }

    // 4. 遍历本地 SQLite sessions 表，决定哪些 session 以及哪些 turns 需要被增量同步
    let mut sqlite_sessions_stmt = sqlite_conn
        .prepare("SELECT source, uuid, title, created_at, last_parsed_idx, last_mtime, project_path, project_name, device_name FROM sessions")
        .map_err(|e| e.to_string())?;
    let mut sqlite_sessions_rows = sqlite_sessions_stmt.query([]).map_err(|e| e.to_string())?;

    let mut sessions_to_sync = Vec::new();

    while let Some(row) = sqlite_sessions_rows.next().map_err(|e| e.to_string())? {
        let source: String = row.get(0).map_err(|e| e.to_string())?;
        let uuid: String = row.get(1).map_err(|e| e.to_string())?;
        let title: Option<String> = row.get(2).map_err(|e| e.to_string())?;
        let created_at: Option<String> = row.get(3).map_err(|e| e.to_string())?;
        let last_parsed_idx: i64 = row.get(4).map_err(|e| e.to_string())?;
        let last_mtime: f64 = row.get(5).map_err(|e| e.to_string())?;
        let project_path: Option<String> = row.get(6).map_err(|e| e.to_string())?;
        let project_name: Option<String> = row.get(7).map_err(|e| e.to_string())?;
        let device_name: Option<String> = row.get(8).map_err(|e| e.to_string())?;

        let mut need_sync = false;
        let mut pg_last_parsed_idx = -1i64;

        if let Some(&(pg_mtime, pg_idx)) = pg_sessions.get(&(source.clone(), uuid.clone())) {
            // 本地 last_mtime 领先，或者 last_parsed_idx 领先，说明需要同步
            if last_mtime > pg_mtime + 1e-4 || last_parsed_idx > pg_idx {
                need_sync = true;
                pg_last_parsed_idx = pg_idx;
            }
        } else {
            // PG 侧尚无此会话，全新同步
            need_sync = true;
        }

        if need_sync {
            sessions_to_sync.push((
                source,
                uuid,
                title,
                created_at,
                last_parsed_idx,
                last_mtime,
                project_path,
                project_name,
                pg_last_parsed_idx,
                device_name,
            ));
        }
    }

    if sessions_to_sync.is_empty() {
        log_progress("所有本地数据与远程 PostgreSQL 保持一致，无需增量同步。");
        return Ok(());
    }

    let total_sessions = sessions_to_sync.len();
    log_progress(&format!("检测到有 {} 个会话存在更新，正在进行增量分批同步...", total_sessions));

    if let Ok(mut status) = get_scan_status().lock() {
        status.total_files = total_sessions;
        status.scanned_files = 0;
    }

    // 辅助转义函数
    fn pg_copy_string_raw(s: &str) -> String {
        let mut escaped = String::with_capacity(s.len());
        for c in s.chars() {
            match c {
                '\\' => escaped.push_str("\\\\"),
                '\t' => escaped.push_str("\\t"),
                '\n' => escaped.push_str("\\n"),
                '\r' => escaped.push_str("\\r"),
                _ => escaped.push(c),
            }
        }
        escaped
    }

    fn pg_copy_string(val: &Option<String>) -> String {
        match val {
            Some(s) => pg_copy_string_raw(s),
            None => "\\N".to_string(),
        }
    }

    let mut scanned_count = 0;

    // 预编译增量 turns 查询，避免在循环内部重复 compile
    let mut sqlite_turns_stmt = sqlite_conn
        .prepare(
            "SELECT source, uuid, idx, model, input_tokens, cached_input_tokens, output_tokens, thinking_tokens, cost_usd, message_id, request_id, timestamp, latency, tps 
             FROM turns 
             WHERE source = ? AND uuid = ? AND idx > ?"
        )
        .map_err(|e| e.to_string())?;

    // 5. 分批（每 1000 个会话）镜像同步变动部分，减少临时表创建销毁及系统表锁开销
    for session_chunk in sessions_to_sync.chunks(1000) {
        log_progress(&format!(
            "正在同步数据至远程 PostgreSQL ({}/{}) ...",
            scanned_count, total_sessions
        ));

        // 开启 PG 事务
        let mut pg_tx = pg_client.transaction().map_err(|e| e.to_string())?;

        // A. 创建临时表 (ON COMMIT DROP，事务提交时自动销毁)
        pg_tx.execute(
            "CREATE TEMP TABLE temp_sessions (
                source TEXT,
                uuid TEXT,
                title TEXT,
                created_at TEXT,
                last_parsed_idx BIGINT,
                last_mtime DOUBLE PRECISION,
                project_path TEXT,
                project_name TEXT,
                device_name TEXT
            ) ON COMMIT DROP",
            &[],
        ).map_err(|e| format!("创建临时会话表失败: {}", e))?;

        pg_tx.execute(
            "CREATE TEMP TABLE temp_turns (
                source TEXT,
                uuid TEXT,
                idx BIGINT,
                model TEXT,
                input_tokens BIGINT,
                cached_input_tokens BIGINT,
                output_tokens BIGINT,
                thinking_tokens BIGINT,
                cost_usd DOUBLE PRECISION,
                message_id TEXT,
                request_id TEXT,
                timestamp TEXT,
                latency DOUBLE PRECISION,
                tps DOUBLE PRECISION
            ) ON COMMIT DROP",
            &[],
        ).map_err(|e| format!("创建临时轮次表失败: {}", e))?;

        let mut session_copy_data = String::new();
        let mut turns_copy_data = String::new();

        for (source, uuid, title, created_at, last_parsed_idx, last_mtime, project_path, project_name, pg_last_parsed_idx, device_name) in session_chunk {
            scanned_count += 1;
            if let Ok(mut status) = get_scan_status().lock() {
                status.scanned_files = scanned_count;
            }

            // 构造 sessions COPY 行
            session_copy_data.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                pg_copy_string_raw(source),
                pg_copy_string_raw(uuid),
                pg_copy_string(title),
                pg_copy_string(created_at),
                last_parsed_idx,
                last_mtime,
                pg_copy_string(project_path),
                pg_copy_string(project_name),
                pg_copy_string(device_name)
            ));

            // 查询该会话的增量 turns
            let mut sqlite_turns_rows = sqlite_turns_stmt
                .query(rusqlite::params![source, uuid, pg_last_parsed_idx])
                .map_err(|e| e.to_string())?;

            while let Some(row) = sqlite_turns_rows.next().map_err(|e| e.to_string())? {
                let src: String = row.get(0).map_err(|e| e.to_string())?;
                let uid: String = row.get(1).map_err(|e| e.to_string())?;
                let idx: i64 = row.get(2).map_err(|e| e.to_string())?;
                let model: Option<String> = row.get(3).map_err(|e| e.to_string())?;
                let input_tokens: i64 = row.get(4).map_err(|e| e.to_string())?;
                let cached_input_tokens: i64 = row.get(5).map_err(|e| e.to_string())?;
                let output_tokens: i64 = row.get(6).map_err(|e| e.to_string())?;
                let thinking_tokens: i64 = row.get(7).map_err(|e| e.to_string())?;
                let cost_usd: f64 = row.get(8).map_err(|e| e.to_string())?;
                let message_id: Option<String> = row.get(9).map_err(|e| e.to_string())?;
                let request_id: Option<String> = row.get(10).map_err(|e| e.to_string())?;
                let timestamp: Option<String> = row.get(11).map_err(|e| e.to_string())?;
                let latency: f64 = row.get(12).map_err(|e| e.to_string())?;
                let tps: f64 = row.get(13).map_err(|e| e.to_string())?;

                turns_copy_data.push_str(&format!(
                    "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                    pg_copy_string_raw(&src),
                    pg_copy_string_raw(&uid),
                    idx,
                    pg_copy_string(&model),
                    input_tokens,
                    cached_input_tokens,
                    output_tokens,
                    thinking_tokens,
                    cost_usd,
                    pg_copy_string(&message_id),
                    pg_copy_string(&request_id),
                    pg_copy_string(&timestamp),
                    latency,
                    tps
                ));
            }
        }

        // B. 使用 COPY 流式写入临时会话表
        if !session_copy_data.is_empty() {
            use std::io::Write;
            let mut writer = pg_tx.copy_in("COPY temp_sessions FROM STDIN (FORMAT text, NULL '\\N')")
                .map_err(|e| format!("流式同步会话失败: {}", e))?;
            writer.write_all(session_copy_data.as_bytes())
                .map_err(|e| format!("流式写入会话数据失败: {}", e))?;
            writer.finish().map_err(|e| format!("结束流式写入会话失败: {}", e))?;
        }

        // C. 使用 COPY 流式写入临时轮次表
        if !turns_copy_data.is_empty() {
            use std::io::Write;
            let mut writer = pg_tx.copy_in("COPY temp_turns FROM STDIN (FORMAT text, NULL '\\N')")
                .map_err(|e| format!("流式同步轮次失败: {}", e))?;
            writer.write_all(turns_copy_data.as_bytes())
                .map_err(|e| format!("流式写入轮次数据失败: {}", e))?;
            writer.finish().map_err(|e| format!("结束流式写入轮次失败: {}", e))?;
        }

        // D. 批量 Merge (将临时表的数据高速同步到正式表，处理冲突)
        pg_tx.execute(
            "INSERT INTO sessions (source, uuid, title, created_at, last_parsed_idx, last_mtime, project_path, project_name, device_name)
             SELECT source, uuid, title, created_at, last_parsed_idx, last_mtime, project_path, project_name, device_name FROM temp_sessions
             ON CONFLICT (source, uuid) DO UPDATE SET
                title = EXCLUDED.title,
                created_at = EXCLUDED.created_at,
                last_parsed_idx = EXCLUDED.last_parsed_idx,
                last_mtime = EXCLUDED.last_mtime,
                project_path = EXCLUDED.project_path,
                project_name = EXCLUDED.project_name,
                device_name = EXCLUDED.device_name",
            &[],
        ).map_err(|e| format!("批量合并会话记录失败: {}", e))?;
        pg_tx.execute(
            "INSERT INTO turns (source, uuid, idx, model, input_tokens, cached_input_tokens, output_tokens, thinking_tokens, cost_usd, message_id, request_id, timestamp, latency, tps)
             SELECT source, uuid, idx, model, input_tokens, cached_input_tokens, output_tokens, thinking_tokens, cost_usd, message_id, request_id, timestamp, latency, tps FROM temp_turns
             ON CONFLICT (source, uuid, idx) DO UPDATE SET
                model = EXCLUDED.model,
                input_tokens = EXCLUDED.input_tokens,
                cached_input_tokens = EXCLUDED.cached_input_tokens,
                output_tokens = EXCLUDED.output_tokens,
                thinking_tokens = EXCLUDED.thinking_tokens,
                cost_usd = EXCLUDED.cost_usd,
                message_id = EXCLUDED.message_id,
                request_id = EXCLUDED.request_id,
                timestamp = EXCLUDED.timestamp,
                latency = EXCLUDED.latency,
                tps = EXCLUDED.tps",
            &[],
        ).map_err(|e| format!("批量合并轮次记录失败: {}", e))?;

        // 提交事务，自动 Drop 临时表
        pg_tx.commit().map_err(|e| format!("提交 PostgreSQL 同步事务失败: {}", e))?;
    }

    log_progress("正在重建远程 PostgreSQL 大盘预计算聚合缓存...");
    if let Err(e) = rebuild_pg_daily_stats_cache(&mut pg_client) {
        log_progress(&format!("重建远程 PostgreSQL 大盘缓存失败: {}", e));
    }

    log_progress("SQLite 本地增量数据镜像成功同步到远程 PostgreSQL 数据库！");
    Ok(())
}

pub fn get_pg_aggregated_metrics(
    source_filter: Option<&str>,
    start_date: Option<&str>,
    end_date: Option<&str>,
) -> Result<AggregatedMetrics, String> {
    let _ = dotenvy::dotenv();
    
    // 1. 统一提取 PostgreSQL 配置，合成正确的连接 URL，支持拆分字段，与 db_adapter.rs 一致
    let pg_host = std::env::var("DB_PG_HOST").unwrap_or_default();
    let pg_port = std::env::var("DB_PG_PORT").unwrap_or_default();
    let pg_user = std::env::var("DB_PG_USER").unwrap_or_default();
    let pg_password = std::env::var("DB_PG_PASSWORD").unwrap_or_default();
    let pg_database = std::env::var("DB_PG_DATABASE").unwrap_or_default();

    let db_url = if !pg_host.trim().is_empty() {
        format!(
            "postgresql://{}:{}@{}:{}/{}",
            pg_user, pg_password, pg_host, pg_port, pg_database
        )
    } else {
        std::env::var("DATABASE_URL").map_err(|e| format!("未配置 PostgreSQL 数据库 URL: {}", e))?
    };

    // 2. 自动检测并创建数据库
    let _ = crate::db_adapter::ensure_pg_database_exists(&db_url);

    // 3. 使用 Config 配置超时时间并进行连接，快速失败防止大盘加载挂起
    let mut config: postgres::config::Config = db_url.parse().map_err(|e: postgres::Error| format!("解析 PostgreSQL 连接 URL 失败: {}", e))?;
    config.connect_timeout(std::time::Duration::from_secs(5));
    let mut pg_client = config.connect(postgres::NoTls).map_err(|e| format!("无法连接到远程 PostgreSQL 数据库: {}", e))?;

    // 运行 Postgres 数据库迁移以保证表结构最新
    crate::db_adapter::init_postgres_tables(&mut pg_client).map_err(|e| format!("执行 PostgreSQL 数据库迁移失败: {}", e))?;

    // 检测 daily_stats 缓存表是否为空。如果为空，则自动为其触发全量预计算重建，防止首次导入配置后图表无数据
    let stats_count: i64 = pg_client
        .query_one("SELECT COUNT(1) FROM daily_stats", &[])
        .map(|row| row.get::<_, i64>(0))
        .unwrap_or(0);
    if stats_count == 0 {
        println!("[PG 数据自愈] 检测到远程 PostgreSQL 的 daily_stats 缓存表为空，正在后台为您重建大盘预计算缓存...");
        let _ = rebuild_pg_daily_stats_cache(&mut pg_client);
    }

    let mut conditions_cache = Vec::new();
    let mut params_cache: Vec<String> = Vec::new();
    let mut param_idx_cache = 1;

    if let Some(src) = source_filter {
        if src != "all" {
            conditions_cache.push(format!("source = ${}", param_idx_cache));
            params_cache.push(src.to_string());
            param_idx_cache += 1;
        }
    }

    if let Some(start) = start_date {
        if !start.is_empty() {
            conditions_cache.push(format!("date >= ${}", param_idx_cache));
            params_cache.push(start.to_string());
            param_idx_cache += 1;
        }
    }

    if let Some(end) = end_date {
        if !end.is_empty() {
            conditions_cache.push(format!("date <= ${}", param_idx_cache));
            params_cache.push(end.to_string());
        }
    }

    let where_clause_cache = if conditions_cache.is_empty() {
        "".to_string()
    } else {
        format!("WHERE {}", conditions_cache.join(" AND "))
    };

    let mut pg_params_cache: Vec<&(dyn postgres::types::ToSql + Sync)> = Vec::new();
    for p in &params_cache {
        pg_params_cache.push(p);
    }

    // 构造动态 SQL WHERE 子句 (面向原始关联查询 turns & sessions)
    let mut conditions_raw = Vec::new();
    let mut params_raw: Vec<String> = Vec::new();
    let mut param_idx_raw = 1;

    if let Some(src) = source_filter {
        if src != "all" {
            conditions_raw.push(format!("s.source = ${}", param_idx_raw));
            params_raw.push(src.to_string());
            param_idx_raw += 1;
        }
    }

    if let Some(start) = start_date {
        if !start.is_empty() {
            conditions_raw.push(format!("s.created_at >= ${}", param_idx_raw));
            params_raw.push(format!("{}T00:00:00", start));
            param_idx_raw += 1;
        }
    }

    if let Some(end) = end_date {
        if !end.is_empty() {
            conditions_raw.push(format!("s.created_at <= ${}", param_idx_raw));
            params_raw.push(format!("{}T23:59:59.999", end));
        }
    }

    let where_clause_raw = if conditions_raw.is_empty() {
        "".to_string()
    } else {
        format!("WHERE {}", conditions_raw.join(" AND "))
    };

    let mut pg_params_raw: Vec<&(dyn postgres::types::ToSql + Sync)> = Vec::new();
    for p in &params_raw {
        pg_params_raw.push(p);
    }

    // 1. Totals (CAST AS BIGINT 防止 PG SUM(bigint) 默认返回 NUMERIC 类型引起 Rust 侧反序列化 Panic)
    let sql_totals = format!(
        "SELECT 
            CAST(SUM(input_tokens) AS BIGINT) as total_input,
            CAST(SUM(output_tokens) AS BIGINT) as total_output,
            CAST(SUM(cached_input_tokens) AS BIGINT) as total_cached,
            CAST(SUM(thinking_tokens) AS BIGINT) as total_thinking,
            SUM(cost_usd) as total_cost,
            CAST(SUM(sessions_count) AS BIGINT) as total_sessions
        FROM daily_stats
        {}",
        where_clause_cache
    );

    let row = pg_client.query_one(&sql_totals, &pg_params_cache[..]).map_err(|e| e.to_string())?;
    let sum_input: Option<i64> = row.get(0);
    let sum_output: Option<i64> = row.get(1);
    let sum_cached: Option<i64> = row.get(2);
    let sum_thinking: Option<i64> = row.get(3);
    let sum_cost: Option<f64> = row.get(4);
    let sum_sessions: Option<i64> = row.get(5);

    let total_input = sum_input.unwrap_or(0);
    let total_output = sum_output.unwrap_or(0);
    let total_cached = sum_cached.unwrap_or(0);
    let total_thinking = sum_thinking.unwrap_or(0);
    let total_cost = sum_cost.unwrap_or(0.0);
    let total_sessions = sum_sessions.unwrap_or(0);

    let cache_hit_rate = if total_input > 0 {
        total_cached as f64 / total_input as f64
    } else {
        0.0
    };
    let thinking_ratio = if total_output > 0 {
        total_thinking as f64 / total_output as f64
    } else {
        0.0
    };

    let totals = Totals {
        total_input,
        total_output,
        total_tokens: total_input + total_output,
        total_cached,
        total_thinking,
        cache_hit_rate,
        thinking_ratio,
        total_sessions,
        total_cost,
    };

    // 2. Daily Trends
    let sql_daily = format!(
        "SELECT 
            date,
            CAST(SUM(input_tokens) AS BIGINT) as input,
            CAST(SUM(output_tokens) AS BIGINT) as output,
            CAST(SUM(cached_input_tokens) AS BIGINT) as cached,
            CAST(SUM(thinking_tokens) AS BIGINT) as thinking,
            CAST(SUM(sessions_count) AS BIGINT) as sessions
        FROM daily_stats
        {}
        GROUP BY date
        ORDER BY date ASC",
        where_clause_cache
    );

    let rows_daily = pg_client.query(&sql_daily, &pg_params_cache[..]).map_err(|e| e.to_string())?;
    let mut daily_trends = Vec::new();
    for r in rows_daily {
        let date: Option<String> = r.get(0);
        let input: Option<i64> = r.get(1);
        let output: Option<i64> = r.get(2);
        let cached: Option<i64> = r.get(3);
        let thinking: Option<i64> = r.get(4);
        let sessions: Option<i64> = r.get(5);
        daily_trends.push(DailyTrend {
            date: date.unwrap_or_default(),
            input: input.unwrap_or(0),
            output: output.unwrap_or(0),
            cached: cached.unwrap_or(0),
            thinking: thinking.unwrap_or(0),
            sessions: sessions.unwrap_or(0),
        });
    }

    // 3. Monthly Summary
    let sql_monthly = format!(
        "SELECT 
            SUBSTR(date, 1, 7) as month,
            CAST(SUM(input_tokens) AS BIGINT) as input,
            CAST(SUM(output_tokens) AS BIGINT) as output,
            CAST(SUM(cached_input_tokens) AS BIGINT) as cached,
            CAST(SUM(thinking_tokens) AS BIGINT) as thinking,
            CAST(SUM(sessions_count) AS BIGINT) as sessions
        FROM daily_stats
        {}
        GROUP BY month
        ORDER BY month DESC",
        where_clause_cache
    );

    let rows_monthly = pg_client.query(&sql_monthly, &pg_params_cache[..]).map_err(|e| e.to_string())?;
    let mut monthly_summary = Vec::new();
    for r in rows_monthly {
        let month: Option<String> = r.get(0);
        let input: Option<i64> = r.get(1);
        let output: Option<i64> = r.get(2);
        let cached: Option<i64> = r.get(3);
        let thinking: Option<i64> = r.get(4);
        let sessions: Option<i64> = r.get(5);
        monthly_summary.push(MonthlySummary {
            month: month.unwrap_or_default(),
            input: input.unwrap_or(0),
            output: output.unwrap_or(0),
            cached: cached.unwrap_or(0),
            thinking: thinking.unwrap_or(0),
            sessions: sessions.unwrap_or(0),
        });
    }

    // 4. Model Distribution (走索引)
    let sql_model_dist = format!(
        "SELECT 
            t.model,
            CAST(SUM(t.input_tokens) AS BIGINT) as input,
            CAST(SUM(t.output_tokens) AS BIGINT) as output,
            CAST(SUM(t.cached_input_tokens) AS BIGINT) as cached,
            CAST(SUM(t.thinking_tokens) AS BIGINT) as thinking,
            CAST(SUM(t.input_tokens + t.output_tokens) AS BIGINT) as total_tokens
        FROM turns t
        INNER JOIN sessions s ON t.source = s.source AND t.uuid = s.uuid
        {} {}
        GROUP BY t.model
        ORDER BY total_tokens DESC",
        where_clause_raw,
        if where_clause_raw.is_empty() { "WHERE t.model IS NOT NULL AND t.model != 'unknown' AND t.model != ''" } else { "AND t.model IS NOT NULL AND t.model != 'unknown' AND t.model != ''" }
    );

    let rows_model = pg_client.query(&sql_model_dist, &pg_params_raw[..]).map_err(|e| e.to_string())?;
    let mut model_distribution = Vec::new();
    for r in rows_model {
        let model: Option<String> = r.get(0);
        let input: Option<i64> = r.get(1);
        let output: Option<i64> = r.get(2);
        let cached: Option<i64> = r.get(3);
        let thinking: Option<i64> = r.get(4);
        let total_tokens: Option<i64> = r.get(5);
        model_distribution.push(ModelDistribution {
            model: model.unwrap_or_else(|| "unknown".to_string()),
            input: input.unwrap_or(0),
            output: output.unwrap_or(0),
            cached: cached.unwrap_or(0),
            thinking: thinking.unwrap_or(0),
            total_tokens: total_tokens.unwrap_or(0),
        });
    }

    // 5. Sessions (解耦)
    let sessions = Vec::new();

    // 6. Source Trends (查缓存表)
    let sql_source_trends = format!(
        "SELECT 
            date,
            source,
            CAST(SUM(input_tokens + output_tokens) AS BIGINT) as total_tokens,
            SUM(cost_usd) as cost
        FROM daily_stats
        {}
        GROUP BY date, source
        ORDER BY date ASC, source ASC",
        where_clause_cache
    );

    let rows_trends = pg_client.query(&sql_source_trends, &pg_params_cache[..]).map_err(|e| e.to_string())?;
    let mut source_trends = Vec::new();
    for r in rows_trends {
        let date: Option<String> = r.get(0);
        let source: String = r.get(1);
        let tokens: Option<i64> = r.get(2);
        let cost: Option<f64> = r.get(3);
        source_trends.push(SourceTrend {
            date: date.unwrap_or_default(),
            source,
            tokens: tokens.unwrap_or(0),
            cost: cost.unwrap_or(0.0),
        });
    }

    // 6.2. Device Trends (查缓存表)
    let sql_device_trends = format!(
        "SELECT 
            date,
            device_name,
            CAST(SUM(input_tokens + output_tokens) AS BIGINT) as total_tokens,
            SUM(cost_usd) as cost
        FROM daily_stats
        {}
        GROUP BY date, device_name
        ORDER BY date ASC, device_name ASC",
        where_clause_cache
    );

    let rows_device_trends = pg_client.query(&sql_device_trends, &pg_params_cache[..]).map_err(|e| e.to_string())?;
    let mut device_trends = Vec::new();
    for r in rows_device_trends {
        let date: Option<String> = r.get(0);
        let device_name: String = r.get(1);
        let tokens: Option<i64> = r.get(2);
        let cost: Option<f64> = r.get(3);
        device_trends.push(DeviceTrend {
            date: date.unwrap_or_default(),
            device_name,
            tokens: tokens.unwrap_or(0),
            cost: cost.unwrap_or(0.0),
        });
    }

    // 7. Model Performance (走索引)
    let sql_model_perf = format!(
        "SELECT 
            t.model,
            AVG(t.latency) as avg_latency,
            AVG(t.tps) as avg_tps,
            COUNT(*) as sample_count
        FROM turns t
        INNER JOIN sessions s ON t.source = s.source AND t.uuid = s.uuid
        {} {}
        GROUP BY t.model
        ORDER BY sample_count DESC",
        where_clause_raw,
        if where_clause_raw.is_empty() { "WHERE t.latency > 0.0 AND t.model IS NOT NULL AND t.model != ''" } else { "AND t.latency > 0.0 AND t.model IS NOT NULL AND t.model != ''" }
    );

    let rows_model_perf = pg_client.query(&sql_model_perf, &pg_params_raw[..]).map_err(|e| e.to_string())?;
    let mut model_performance = Vec::new();
    for r in rows_model_perf {
        let model: Option<String> = r.get(0);
        let avg_latency: Option<f64> = r.get(1);
        let avg_tps: Option<f64> = r.get(2);
        let sample_count: Option<i64> = r.get(3);
        model_performance.push(ModelPerformance {
            model: model.unwrap_or_else(|| "unknown".to_string()),
            avg_latency: avg_latency.unwrap_or(0.0),
            avg_tps: avg_tps.unwrap_or(0.0),
            sample_count: sample_count.unwrap_or(0),
        });
    }

    // 8. Performance Trends (走索引)
    let sql_perf_trends = format!(
        "SELECT 
            SUBSTR(s.created_at, 1, 10) as date,
            AVG(t.latency) as avg_latency,
            AVG(t.tps) as avg_tps
        FROM sessions s
        LEFT JOIN turns t ON s.source = t.source AND s.uuid = t.uuid
        {} {}
        GROUP BY date
        ORDER BY date ASC",
        where_clause_raw,
        if where_clause_raw.is_empty() { "WHERE t.latency > 0.0" } else { "AND t.latency > 0.0" }
    );

    let rows_perf_trends = pg_client.query(&sql_perf_trends, &pg_params_raw[..]).map_err(|e| e.to_string())?;
    let mut performance_trends = Vec::new();
    for r in rows_perf_trends {
        let date: Option<String> = r.get(0);
        let avg_latency: Option<f64> = r.get(1);
        let avg_tps: Option<f64> = r.get(2);
        performance_trends.push(PerformanceTrend {
            date: date.unwrap_or_default(),
            avg_latency: avg_latency.unwrap_or(0.0),
            avg_tps: avg_tps.unwrap_or(0.0),
        });
    }

    Ok(AggregatedMetrics {
        totals,
        daily_trends,
        monthly_summary,
        model_distribution,
        sessions,
        source_trends,
        device_trends,
        project_trends: Vec::new(),
        project_rankings: Vec::new(),
        model_performance,
        performance_trends,
        display_currency: "USD".to_string(),
        usd_exchange_rate: 1.0,
        exchange_rate_updated_at: "system-default".to_string(),
    })
}

pub fn clean_cache_db() -> Result<String, String> {
    let _ = dotenvy::dotenv();
    let db_type = std::env::var("DATABASE_TYPE").unwrap_or_else(|_| "sqlite".to_string());
    
    if db_type.to_lowercase() == "postgres" {
        // PostgreSQL 清理
        let pg_host = std::env::var("DB_PG_HOST").unwrap_or_default();
        let pg_port = std::env::var("DB_PG_PORT").unwrap_or_default();
        let pg_user = std::env::var("DB_PG_USER").unwrap_or_default();
        let pg_password = std::env::var("DB_PG_PASSWORD").unwrap_or_default();
        let pg_database = std::env::var("DB_PG_DATABASE").unwrap_or_default();

        let db_url = if !pg_host.trim().is_empty() {
            format!(
                "postgresql://{}:{}@{}:{}/{}",
                pg_user, pg_password, pg_host, pg_port, pg_database
            )
        } else {
            std::env::var("DATABASE_URL").map_err(|e| format!("未配置 PostgreSQL 数据库 URL: {}", e))?
        };

        let mut config: postgres::config::Config = db_url.parse().map_err(|e: postgres::Error| e.to_string())?;
        config.connect_timeout(std::time::Duration::from_secs(5));
        let mut pg_client = config.connect(postgres::NoTls).map_err(|e| format!("无法连接到远程 PostgreSQL 数据库: {}", e))?;

        // 1. 清理 input_tokens=0 且 output_tokens=0 的无意义 turns
        let deleted_turns = pg_client.execute(
            "DELETE FROM turns WHERE input_tokens = 0 AND output_tokens = 0",
            &[],
        ).map_err(|e| e.to_string())?;

        // 2. 清理没有 turns 关联的空会话
        let deleted_sessions = pg_client.execute(
            "DELETE FROM sessions WHERE (source, uuid) NOT IN (SELECT DISTINCT source, uuid FROM turns)",
            &[],
        ).map_err(|e| e.to_string())?;

        return Ok(format!(
            "远程 PostgreSQL 数据库优化完成（此操作仅重组数据库空间，未删除任何本地或远程磁盘文件）！\n共清理无效交互: {} 轮\n共删除僵尸空会话: {} 个",
            deleted_turns, deleted_sessions
        ));
    }

    // 本地 SQLite 清理
    let db_path = get_db_cache_path();
    let mut conn = rusqlite::Connection::open(&db_path)
        .map_err(|e| format!("无法连接到本地 SQLite: {}", e))?;

    let tx = conn.transaction()
        .map_err(|e| format!("开启 SQLite 事务失败: {}", e))?;

    // 1. 删除无效的 turns
    let deleted_turns = tx.execute(
        "DELETE FROM turns WHERE input_tokens = 0 AND output_tokens = 0",
        [],
    ).map_err(|e| format!("清理 turns 失败: {}", e))?;

    // 2. 删除无 turns 的空会话
    let deleted_sessions = tx.execute(
        "DELETE FROM sessions WHERE (source, uuid) NOT IN (SELECT DISTINCT source, uuid FROM turns)",
        [],
    ).map_err(|e| format!("清理 sessions 失败: {}", e))?;

    tx.commit().map_err(|e| format!("提交 SQLite 事务失败: {}", e))?;

    // 3. 执行 VACUUM 收紧本地磁盘空间，整理碎片
    conn.execute("VACUUM", [])
        .map_err(|e| format!("SQLite VACUUM 空间收紧失败: {}", e))?;

    Ok(format!(
        "本地 SQLite 缓存数据库优化瘦身成功（此操作仅重组数据库空间，未删除任何本地磁盘文件）！\n共清理无效交互: {} 轮\n共删除僵尸空会话: {} 个\n物理磁盘碎片整理已生效 (VACUUM)",
        deleted_turns, deleted_sessions
    ))
}

pub fn update_device_name_in_db(old_name: &str, new_name: &str) -> Result<(), String> {
    let old_name_trimmed = old_name.trim();
    let new_name_trimmed = new_name.trim();

    if old_name_trimmed == new_name_trimmed {
        return Ok(());
    }

    println!("[设备重命名] 开始同步更新数据库中的设备名称：'{}' -> '{}'", old_name_trimmed, new_name_trimmed);

    // 1. 更新本地 SQLite 缓存
    let cache_path = get_db_cache_path();
    if cache_path.exists() {
      if let Ok(mut conn) = rusqlite::Connection::open(&cache_path) {
        let tx = conn.transaction().map_err(|e| format!("SQLite 事务开启失败: {}", e))?;
            {
                if old_name_trimmed.is_empty() || old_name_trimmed == "unknown" {
                    tx.execute(
                        "UPDATE sessions SET device_name = ? WHERE device_name = 'unknown' OR device_name IS NULL OR trim(device_name) = ''",
                        rusqlite::params![new_name_trimmed],
                    ).map_err(|e| format!("SQLite sessions 更新失败: {}", e))?;
                } else {
                    tx.execute(
                        "UPDATE sessions SET device_name = ? WHERE device_name = ?",
                        rusqlite::params![new_name_trimmed, old_name_trimmed],
                    ).map_err(|e| format!("SQLite sessions 更新失败: {}", e))?;
                }
            }
            tx.commit().map_err(|e| format!("SQLite 事务提交失败: {}", e))?;

            // 重新计算 SQLite 的 daily_stats
            let _ = rebuild_daily_stats_cache(&conn);
            let _ = rebuild_project_daily_stats_cache(&conn);
            let _ = rebuild_sessions_fts(&conn);
        }
    }

    // 2. 如果当前用的是 Postgres，也需要更新远程数据库
    let db_type = std::env::var("DATABASE_TYPE").unwrap_or_else(|_| "sqlite".to_string());
    if db_type.to_lowercase() == "postgres" {
        let pg_host = std::env::var("DB_PG_HOST").unwrap_or_default();
        let pg_port = std::env::var("DB_PG_PORT").unwrap_or_default();
        let pg_user = std::env::var("DB_PG_USER").unwrap_or_default();
        let pg_password = std::env::var("DB_PG_PASSWORD").unwrap_or_default();
        let pg_database = std::env::var("DB_PG_DATABASE").unwrap_or_default();

        let db_url = if !pg_host.trim().is_empty() {
            format!(
                "postgresql://{}:{}@{}:{}/{}",
                pg_user, pg_password, pg_host, pg_port, pg_database
            )
        } else {
            std::env::var("DATABASE_URL").map_err(|e| format!("未配置 PostgreSQL 数据库 URL: {}", e))?
        };

        let mut config: postgres::config::Config = db_url.parse().map_err(|e: postgres::Error| format!("解析 PostgreSQL 连接 URL 失败: {}", e))?;
        config.connect_timeout(std::time::Duration::from_secs(5));
        let mut pg_client = config.connect(postgres::NoTls).map_err(|e| format!("无法连接到远程 PostgreSQL 数据库: {}", e))?;

        let mut tx = pg_client.transaction().map_err(|e| format!("Postgres 事务开启失败: {}", e))?;
        {
            if old_name_trimmed.is_empty() || old_name_trimmed == "unknown" {
                tx.execute(
                    "UPDATE sessions SET device_name = $1 WHERE device_name = 'unknown' OR device_name IS NULL OR TRIM(device_name) = ''",
                    &[&new_name_trimmed],
                ).map_err(|e| format!("Postgres sessions 更新失败: {}", e))?;
            } else {
                tx.execute(
                    "UPDATE sessions SET device_name = $1 WHERE device_name = $2",
                    &[&new_name_trimmed, &old_name_trimmed],
                ).map_err(|e| format!("Postgres sessions 更新失败: {}", e))?;
            }
        }
        tx.commit().map_err(|e| format!("Postgres 事务提交失败: {}", e))?;

        // 重新计算 Postgres 的 daily_stats
        let _ = rebuild_pg_daily_stats_cache(&mut pg_client);
    }

    println!("[设备重命名] 设备名称同步更新数据库成功！");
    Ok(())
}




