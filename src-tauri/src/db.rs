use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::Serialize;

use crate::proto::{parse_protobuf_orig, try_parse_sub_messages, extract_metrics_from_proto};

// 1. 动态路径获取逻辑（适配不同 Windows 用户目录）

pub fn get_user_profile_dir() -> String {
    std::env::var("USERPROFILE").unwrap_or_else(|_| r"C:\Users\cearn".to_string())
}

pub fn get_db_cache_path() -> PathBuf {
    Path::new(&get_user_profile_dir())
        .join(".ai_token_monitor")
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

    // 检查 turns 表是否已升级包含 latency 列，如果没有则添加
    let has_latency: Result<i32, _> = conn.query_row(
        "SELECT 1 FROM pragma_table_info('turns') WHERE name='latency'",
        [],
        |_| Ok(1),
    );
    if has_latency.is_err() {
        println!("正在平滑迁移本地 cache 数据库表结构：在 turns 表中新增 latency 和 tps 列...");
        let _ = conn.execute("ALTER TABLE turns ADD COLUMN latency REAL DEFAULT 0.0;", []);
        let _ = conn.execute("ALTER TABLE turns ADD COLUMN tps REAL DEFAULT 0.0;", []);
    }

    conn.execute(
        "UPDATE turns SET model = 'gemini-3.5-flash' WHERE model = 'gemini-3-flash-a'",
        [],
    )?;

    // 直接创建基于联合主键的最新 daily_stats 缓存表结构
    conn.execute(
        "CREATE TABLE IF NOT EXISTS daily_stats (
            date TEXT NOT NULL,
            source TEXT NOT NULL,
            input_tokens INTEGER DEFAULT 0,
            cached_input_tokens INTEGER DEFAULT 0,
            output_tokens INTEGER DEFAULT 0,
            thinking_tokens INTEGER DEFAULT 0,
            sessions_count INTEGER DEFAULT 0,
            cost_usd REAL DEFAULT 0.0,
            PRIMARY KEY (date, source)
        )",
        [],
    )?;

    // 创建高性能索引以优化大盘统计查询性能
    conn.execute("CREATE INDEX IF NOT EXISTS idx_sessions_created_at ON sessions(created_at);", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_sessions_source_created ON sessions(source, created_at);", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_turns_model ON turns(model);", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_turns_latency ON turns(latency);", [])?;

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
            "INSERT INTO daily_stats (date, source, input_tokens, cached_input_tokens, output_tokens, thinking_tokens, sessions_count, cost_usd)
             SELECT 
                 substr(s.created_at, 1, 10) as date,
                 s.source,
                 COALESCE(SUM(t.input_tokens), 0) as input_tokens,
                 COALESCE(SUM(t.cached_input_tokens), 0) as cached_input_tokens,
                 COALESCE(SUM(t.output_tokens), 0) as output_tokens,
                 COALESCE(SUM(t.thinking_tokens), 0) as thinking_tokens,
                 COUNT(DISTINCT s.uuid) as sessions_count,
                 COALESCE(SUM(t.cost_usd), 0.0) as cost_usd
             FROM sessions s
             LEFT JOIN turns t ON s.source = t.source AND s.uuid = t.uuid
             GROUP BY date, s.source"
        )?;
        let _ = stmt_insert.execute([])?;
    } else {
        // 日常增量同步重建：只删除并重新聚合最近 365 天的数据
        let one_year_ago = Utc::now() - chrono::Duration::days(365);
        let one_year_ago_str = one_year_ago.format("%Y-%m-%d").to_string();

        let mut stmt_del = conn.prepare("DELETE FROM daily_stats WHERE date >= ?")?;
        let _ = stmt_del.execute(rusqlite::params![one_year_ago_str])?;

        let mut stmt_insert = conn.prepare(
            "INSERT INTO daily_stats (date, source, input_tokens, cached_input_tokens, output_tokens, thinking_tokens, sessions_count, cost_usd)
             SELECT 
                 substr(s.created_at, 1, 10) as date,
                 s.source,
                 COALESCE(SUM(t.input_tokens), 0) as input_tokens,
                 COALESCE(SUM(t.cached_input_tokens), 0) as cached_input_tokens,
                 COALESCE(SUM(t.output_tokens), 0) as output_tokens,
                 COALESCE(SUM(t.thinking_tokens), 0) as thinking_tokens,
                 COUNT(DISTINCT s.uuid) as sessions_count,
                 COALESCE(SUM(t.cost_usd), 0.0) as cost_usd
             FROM sessions s
             LEFT JOIN turns t ON s.source = t.source AND s.uuid = t.uuid
             WHERE substr(s.created_at, 1, 10) >= ?
             GROUP BY date, s.source"
        )?;
        let _ = stmt_insert.execute(rusqlite::params![one_year_ago_str])?;
    }
    
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
            "INSERT INTO daily_stats (date, source, input_tokens, cached_input_tokens, output_tokens, thinking_tokens, sessions_count, cost_usd)
             SELECT 
                 SUBSTR(s.created_at, 1, 10) as date,
                 s.source,
                 COALESCE(SUM(t.input_tokens), 0) as input_tokens,
                 COALESCE(SUM(t.cached_input_tokens), 0) as cached_input_tokens,
                 COALESCE(SUM(t.output_tokens), 0) as output_tokens,
                 COALESCE(SUM(t.thinking_tokens), 0) as thinking_tokens,
                 COUNT(DISTINCT s.uuid) as sessions_count,
                 COALESCE(SUM(t.cost_usd), 0.0) as cost_usd
             FROM sessions s
             LEFT JOIN turns t ON s.source = t.source AND s.uuid = t.uuid
             GROUP BY SUBSTR(s.created_at, 1, 10), s.source",
            &[],
        ).map_err(|e| e.to_string())?;
    } else {
        // 增量同步重建最近 365 天的数据
        let one_year_ago = Utc::now() - chrono::Duration::days(365);
        let one_year_ago_str = one_year_ago.format("%Y-%m-%d").to_string();

        tx.execute("DELETE FROM daily_stats WHERE date >= $1", &[&one_year_ago_str])
            .map_err(|e| e.to_string())?;

        tx.execute(
            "INSERT INTO daily_stats (date, source, input_tokens, cached_input_tokens, output_tokens, thinking_tokens, sessions_count, cost_usd)
             SELECT 
                 SUBSTR(s.created_at, 1, 10) as date,
                 s.source,
                 COALESCE(SUM(t.input_tokens), 0) as input_tokens,
                 COALESCE(SUM(t.cached_input_tokens), 0) as cached_input_tokens,
                 COALESCE(SUM(t.output_tokens), 0) as output_tokens,
                 COALESCE(SUM(t.thinking_tokens), 0) as thinking_tokens,
                 COUNT(DISTINCT s.uuid) as sessions_count,
                 COALESCE(SUM(t.cost_usd), 0.0) as cost_usd
             FROM sessions s
             LEFT JOIN turns t ON s.source = t.source AND s.uuid = t.uuid
             WHERE SUBSTR(s.created_at, 1, 10) >= $1
             GROUP BY SUBSTR(s.created_at, 1, 10), s.source",
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
    println!("{}", msg);
    if let Ok(mut status) = get_scan_status().lock() {
        status.status_msg = msg.to_string();
        status.logs.push(msg.to_string());
        if status.logs.len() > 1000 {
            status.logs.remove(0);
        }
    }
}

pub fn start_background_scan() {
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
        let result = sync_cache_db_with_progress(|scanned, total| {
            let status_lock = get_scan_status();
            if let Ok(mut status) = status_lock.lock() {
                status.scanned_files = scanned;
                status.total_files = total;
            }
        });

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
    let (input, _cache_creation, cache_read, output) = extract_claude_tokens(val);
    if input > 0 || output > 0 {
        let model = extract_claude_model(val);
        let model_name = if model == "unknown" { default_model.to_string() } else { model };
        let thinking = val.get("thinking_tokens")
            .or_else(|| val.get("thinking"))
            .and_then(|v| v.as_i64()).unwrap_or(0);
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

pub fn estimate_cost(model: &str, input: i64, cached: i64, output: i64) -> f64 {
    let model_lower = model.to_lowercase();
    if model_lower.contains("opus") {
        let uncached = (input - cached).max(0);
        ((uncached as f64 * 15.0) + (cached as f64 * 1.5) + (output as f64 * 75.0)) / 1_000_000.0
    } else if model_lower.contains("sonnet") || model_lower.contains("claude-3-5") {
        let uncached = (input - cached).max(0);
        ((uncached as f64 * 3.0) + (cached as f64 * 0.3) + (output as f64 * 15.0)) / 1_000_000.0
    } else if model_lower.contains("haiku") {
        let uncached = (input - cached).max(0);
        ((uncached as f64 * 0.25) + (cached as f64 * 0.03) + (output as f64 * 1.25)) / 1_000_000.0
    } else if model_lower.contains("gemini") {
        if model_lower.contains("pro") {
            let uncached = (input - cached).max(0);
            ((uncached as f64 * 1.25) + (cached as f64 * 0.3125) + (output as f64 * 5.0)) / 1_000_000.0
        } else {
            let uncached = (input - cached).max(0);
            ((uncached as f64 * 0.075) + (cached as f64 * 0.01875) + (output as f64 * 0.3)) / 1_000_000.0
        }
    } else {
        let uncached = (input - cached).max(0);
        ((uncached as f64 * 2.5) + (cached as f64 * 0.25) + (output as f64 * 10.0)) / 1_000_000.0
    }
}

fn extract_claude_tokens(val: &serde_json::Value) -> (i64, i64, i64, i64) {
    let mut input = 0;
    let mut cache_creation = 0;
    let mut cache_read = 0;
    let mut output = 0;

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

        if in_t > 0 || out_t > 0 {
            input = in_t;
            output = out_t;
            cache_creation = c_create;
            cache_read = c_read;
            break;
        }
    }

    (input, cache_creation, cache_read, output)
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

    for (idx, file_path) in jsonl_files.into_iter().enumerate() {
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
                        let (input, _cache_creation, cache_read, output) = extract_claude_tokens(&val);
                        if input > 0 || output > 0 {
                            let model = extract_claude_model(&val);
                            let timestamp = extract_claude_timestamp(&val);
                            let (message_id, request_id) = extract_claude_ids(&val);
                            let total_input = get_total_input_tokens(&model, input, cache_read);
                            let cost = estimate_cost(&model, total_input, cache_read, output);

                            new_turns.push((
                                line_idx - 1,
                                model,
                                total_input,
                                cache_read,
                                output,
                                0,
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
                log_progress(&format!("发现 Claude Code 会话 [{}] 有 {} 条新轮次，正在同步...", uuid, new_turns.len()));
            }

            let tx = conn_cache.transaction()?;
            {
                let title = file_path.file_name().unwrap_or_default().to_string_lossy().to_string();
                let created_at = if !new_turns.is_empty() {
                    new_turns[0].9.clone()
                } else {
                    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
                };

                tx.execute(
                    "INSERT INTO sessions (source, uuid, title, created_at, last_parsed_idx, last_mtime, project_path)
                     VALUES ('claude_code', ?, ?, ?, ?, ?, ?)
                     ON CONFLICT(source, uuid) DO UPDATE SET
                        last_parsed_idx = excluded.last_parsed_idx,
                        last_mtime = excluded.last_mtime,
                        title = excluded.title",
                    rusqlite::params![uuid, title, created_at, line_idx, mtime, file_path.to_string_lossy().to_string()],
                )?;

                for turn in &new_turns {
                    tx.execute(
                        "INSERT OR REPLACE INTO turns (source, uuid, idx, model, input_tokens, cached_input_tokens, output_tokens, thinking_tokens, cost_usd, message_id, request_id, timestamp)
                         VALUES ('claude_code', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                        rusqlite::params![uuid, turn.0, turn.1, turn.2, turn.3, turn.4, turn.5, turn.6, turn.7, turn.8, turn.9],
                    )?;
                }
            }
            tx.commit()?;
        }
    }

    Ok(())
}

pub fn sync_codex(
    conn_cache: &mut rusqlite::Connection,
    progress_offset: usize,
    total_files: usize,
    progress_cb: &impl Fn(usize, usize),
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

    for (idx, file_path) in jsonl_files.into_iter().enumerate() {
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
                            let cost = estimate_cost(&model, total_input, cache_read, output);

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
            }

            let tx = conn_cache.transaction()?;
            {
                let title = file_path.file_stem().unwrap_or_default().to_string_lossy().to_string();
                let created_at = if !new_turns.is_empty() {
                    new_turns[0].9.clone()
                } else {
                    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
                };

                tx.execute(
                    "INSERT INTO sessions (source, uuid, title, created_at, last_parsed_idx, last_mtime, project_path)
                     VALUES ('codex', ?, ?, ?, ?, ?, ?)
                     ON CONFLICT(source, uuid) DO UPDATE SET
                        last_parsed_idx = excluded.last_parsed_idx,
                        last_mtime = excluded.last_mtime,
                        title = excluded.title",
                    rusqlite::params![uuid, title, created_at, line_idx, mtime, file_path.to_string_lossy().to_string()],
                )?;

                for turn in &new_turns {
                    tx.execute(
                        "INSERT OR REPLACE INTO turns (source, uuid, idx, model, input_tokens, cached_input_tokens, output_tokens, thinking_tokens, cost_usd, message_id, request_id, timestamp)
                         VALUES ('codex', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                        rusqlite::params![uuid, turn.0, turn.1, turn.2, turn.3, turn.4, turn.5, turn.6, turn.7, turn.8, turn.9],
                    )?;
                }
            }
            tx.commit()?;
        }
    }

    Ok(())
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
) -> Result<(), rusqlite::Error> {
    let cursor_db = get_cursor_db_path();
    if !cursor_db.exists() {
        return Ok(());
    }

    log_progress("正在扫描并增量同步 Cursor 编辑器历史会话数据...");

    // 使用只读标志打开 Cursor 的 SQLite 数据库，避免占用文件锁
    let conn_cursor = match rusqlite::Connection::open_with_flags(
        &cursor_db,
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

    for (session_idx, (key, val)) in composer_sessions.into_iter().enumerate() {
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

        let mut last_parsed_idx = -1i64;
        let mut last_mtime = 0.0f64;
        let mut is_new_session = true;

        if let Some((parsed_idx, m)) = session_cache.get(&composer_id) {
            last_parsed_idx = *parsed_idx;
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

                            let cost = estimate_cost(&model, input_tokens, 0, output_tokens);
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

        // 执行增量写入事务
        let tx = conn_cache.transaction()?;
        {
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

            tx.execute(
                "INSERT INTO sessions (source, uuid, title, created_at, last_parsed_idx, last_mtime, project_path)
                 VALUES ('cursor', ?, ?, ?, ?, ?, ?)
                 ON CONFLICT(source, uuid) DO UPDATE SET
                    last_parsed_idx = excluded.last_parsed_idx,
                    last_mtime = excluded.last_mtime,
                    title = excluded.title",
                rusqlite::params![
                    composer_id,
                    title,
                    created_at,
                    idx as i64,
                    last_updated,
                    cursor_db.to_string_lossy().to_string(),
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
        }
        tx.commit()?;
    }

    Ok(())
}

pub fn sync_cache_db_with_progress<F>(progress_cb: F) -> Result<(), rusqlite::Error>
where
    F: Fn(usize, usize) + Send + 'static,
{
    // 获取全局数据库锁，避免多线程写入冲突
    let _lock = DB_LOCK.lock().unwrap();

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

    // 计算总文件数
    let total_files = db_files.len() + claude_files.len() + codex_files.len() + if has_cursor { 1 } else { 0 };
    progress_cb(0, total_files);

    let msg = format!("发现待同步物理数据源共 {} 个（Antigravity: {}, Claude Code: {}, Codex: {}, Cursor: {}）", 
        total_files, db_files.len(), claude_files.len(), codex_files.len(), if has_cursor { 1 } else { 0 });
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
    for (i, db_path) in db_files.into_iter().enumerate() {
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
                            if let Ok(raw_parsed) = parse_protobuf_orig(&blob, &mut pos, len) {
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
            }

            // 写入增量数据并更新修改时间
            let tx = conn_cache.transaction()?;
            {
                if is_new_session || existing_title.starts_with("Unknown Session") {
                    let (title, created_at) = extract_convo_info(&uuid, &db_path);
                    tx.execute(
                        "INSERT INTO sessions (source, uuid, title, created_at, last_parsed_idx, last_mtime) VALUES ('antigravity', ?, ?, ?, ?, ?)
                         ON CONFLICT(source, uuid) DO UPDATE SET
                            title = excluded.title,
                            created_at = excluded.created_at,
                            last_parsed_idx = excluded.last_parsed_idx,
                            last_mtime = excluded.last_mtime",
                        rusqlite::params![uuid, title, created_at, max_idx_in_db, mtime],
                    )?;
                } else {
                    tx.execute(
                        "UPDATE sessions SET last_parsed_idx = ?, last_mtime = ? WHERE source = 'antigravity' AND uuid = ?",
                        rusqlite::params![max_idx_in_db, mtime, uuid],
                    )?;
                }

                for turn in &new_turns {
                    let cost = estimate_cost(&turn.2, turn.3 + turn.4, turn.4, turn.5);
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
            }
            tx.commit()?;
        } else {
            log_progress(&format!("跳过会话 [{}] 的更新：物理数据库目前无法访问或格式不兼容，下次将重新尝试", uuid));
        }
        progress_cb(i + 1, total_files);
    }

    // D. 增量同步 Claude Code 数据
    let _ = sync_claude_code(&mut conn_cache, db_files_len, total_files, &progress_cb);

    // E. 增量同步 Codex 数据
    let _ = sync_codex(&mut conn_cache, db_files_len + claude_files.len(), total_files, &progress_cb);

    // F. 增量同步 Cursor 数据
    let _ = sync_cursor(&mut conn_cache, db_files_len + claude_files.len() + codex_files.len(), total_files, &progress_cb);

    // H. 在同步结束前，一键重建本地 daily_stats 预聚合缓存表，保证大盘毫秒级查询
    log_progress("正在重建本地大盘预计算聚合缓存...");
    if let Err(e) = rebuild_daily_stats_cache(&conn_cache) {
        log_progress(&format!("重建本地大盘缓存失败: {}", e));
    }

    // G. 如果配置了 PostgreSQL 模式，自动将本地 SQLite 增量好的最新数据一键同步至 PostgreSQL
    if let Err(e) = sync_local_to_postgres() {
        log_progress(&format!("同步到 PostgreSQL 失败: {}", e));
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("同步至 PostgreSQL 失败: {}", e),
        ))));
    }

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
            conditions.push("(s.title LIKE ? OR s.uuid LIKE ?)");
            let like_str = format!("%{}%", kw_trimmed);
            params.push(rusqlite::types::Value::Text(like_str.clone()));
            params.push(rusqlite::types::Value::Text(like_str));
        }
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
pub struct AggregatedMetrics {
    pub totals: Totals,
    pub daily_trends: Vec<DailyTrend>,
    pub monthly_summary: Vec<MonthlySummary>,
    pub model_distribution: Vec<ModelDistribution>,
    pub sessions: Vec<SessionItem>,
    pub source_trends: Vec<SourceTrend>,
    pub model_performance: Vec<ModelPerformance>,
    pub performance_trends: Vec<PerformanceTrend>,
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

    Ok(AggregatedMetrics {
        totals,
        daily_trends,
        monthly_summary,
        model_distribution,
        sessions,
        source_trends,
        model_performance,
        performance_trends,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;

    #[test]
    fn test_estimate_cost() {
        let cost_opus = estimate_cost("claude-3-opus", 1000, 200, 500);
        assert!((cost_opus - 0.0498).abs() < 1e-6);

        let cost_sonnet = estimate_cost("claude-3-5-sonnet", 1000, 300, 500);
        assert!((cost_sonnet - 0.00969).abs() < 1e-6);

        let cost_flash = estimate_cost("gemini-2.5-flash", 10000, 2000, 5000);
        assert!((cost_flash - 0.0021375).abs() < 1e-8);
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

        let (in_t, c_create, c_read, out_t) = extract_claude_tokens(&sample_json);
        assert_eq!(in_t, 120);
        assert_eq!(c_create, 0);
        assert_eq!(c_read, 20);
        assert_eq!(out_t, 80);

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
        let temp_path = std::env::temp_dir().join(format!("ai_token_monitor_test_{}", test_id));
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
        sync_claude_code(&mut conn, 0, 1, &|_, _| {}).unwrap();
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

        sync_claude_code(&mut conn, 0, 1, &|_, _| {}).unwrap();
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

        sync_codex(&mut conn, 0, 1, &|_, _| {}).unwrap();
        rebuild_daily_stats_cache(&conn).unwrap();

        let metrics_all_3 = get_aggregated_metrics_from_cache(None, None, None).unwrap();
        assert_eq!(metrics_all_3.totals.total_sessions, 2);
        assert_eq!(metrics_all_3.totals.total_input, 1530);
        assert_eq!(metrics_all_3.totals.total_output, 850);
        assert_eq!(metrics_all_3.totals.total_cached, 160);

        drop(conn);
        let _ = fs::remove_dir_all(&temp_path);
    }
}

// ==================== PostgreSQL 同步与查询路由代理模块 ====================

pub fn sync_local_to_postgres() -> Result<(), String> {
    let _ = dotenvy::dotenv();
    let db_type = std::env::var("DATABASE_TYPE").unwrap_or_else(|_| "sqlite".to_string());
    if db_type.to_lowercase() != "postgres" {
        return Ok(());
    }

    println!("检测到远程 PostgreSQL 模式，正在触发增量同步...");
    
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

    // 2. 使用 Config 配置超时时间并进行连接，快速失败防止无限卡死
    let mut config: postgres::config::Config = db_url.parse().map_err(|e: postgres::Error| format!("解析 PostgreSQL 连接 URL 失败: {}", e))?;
    config.connect_timeout(std::time::Duration::from_secs(5));
    let mut pg_client = config.connect(postgres::NoTls).map_err(|e| format!("无法连接到远程 PostgreSQL 数据库: {}", e))?;

    // 运行 Postgres 数据库迁移以保证表结构最新 (包含新列如 latency 和 tps)
    {
        let db_conn = crate::db_adapter::DbConn::Postgres(std::sync::Mutex::new(pg_client));
        crate::db_adapter::init_tables(&db_conn).map_err(|e| format!("执行 PostgreSQL 数据库迁移失败: {}", e))?;
        pg_client = match db_conn {
            crate::db_adapter::DbConn::Postgres(mutex) => mutex.into_inner().map_err(|_| "无法重新获取 PostgreSQL 客户端".to_string())?,
            _ => unreachable!(),
        };
    }

    // ===== PostgreSQL 数据清洗迁移，防 claude_code / codex 历史会话 input_tokens 偏小 (v1) =====
    let pg_meta_exists = pg_client.query_one(
        "SELECT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'db_meta')",
        &[],
    );
    if let Ok(row) = pg_meta_exists {
        let exists: bool = row.get(0);
        if !exists {
            // 只有当 turns 表存在，且其中包含 claude_code 或 codex 的老旧数据时，才进行清洗和日志打印
            let has_old_data = pg_client.query_one(
                "SELECT EXISTS (
                    SELECT 1 FROM information_schema.tables WHERE table_name = 'turns'
                )",
                &[],
            ).map(|r| r.get::<_, bool>(0)).unwrap_or(false) && 
            pg_client.query_one(
                "SELECT EXISTS (
                    SELECT 1 FROM turns WHERE source IN ('claude_code', 'codex')
                )",
                &[],
            ).map(|r| r.get::<_, bool>(0)).unwrap_or(false);

            if has_old_data {
                println!("检测到远程 PostgreSQL 老数据库版本，执行一键数据清洗并记录 db_meta 表...");
                let _ = pg_client.execute("DELETE FROM turns WHERE source IN ('claude_code', 'codex')", &[]);
                let _ = pg_client.execute("DELETE FROM sessions WHERE source IN ('claude_code', 'codex')", &[]);
            }
            let _ = pg_client.execute(
                "CREATE TABLE db_meta (
                    key TEXT PRIMARY KEY,
                    value TEXT
                )",
                &[],
            );
            let _ = pg_client.execute("INSERT INTO db_meta (key, value) VALUES ('version', '1')", &[]);
        }
    }

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
        .prepare("SELECT source, uuid, title, created_at, last_parsed_idx, last_mtime, project_path FROM sessions")
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
                pg_last_parsed_idx,
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

    // 5. 分批（每 50 个会话）镜像同步变动部分
    for session_chunk in sessions_to_sync.chunks(50) {
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
                project_path TEXT
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

        for (source, uuid, title, created_at, last_parsed_idx, last_mtime, project_path, pg_last_parsed_idx) in session_chunk {
            scanned_count += 1;
            if let Ok(mut status) = get_scan_status().lock() {
                status.scanned_files = scanned_count;
            }

            // 构造 sessions COPY 行
            session_copy_data.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                pg_copy_string_raw(source),
                pg_copy_string_raw(uuid),
                pg_copy_string(title),
                pg_copy_string(created_at),
                last_parsed_idx,
                last_mtime,
                pg_copy_string(project_path)
            ));

            // 查询该会话的增量 turns
            let mut sqlite_turns_stmt = sqlite_conn
                .prepare(
                    "SELECT source, uuid, idx, model, input_tokens, cached_input_tokens, output_tokens, thinking_tokens, cost_usd, message_id, request_id, timestamp, latency, tps 
                     FROM turns 
                     WHERE source = ? AND uuid = ? AND idx > ?"
                )
                .map_err(|e| e.to_string())?;
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
            "INSERT INTO sessions (source, uuid, title, created_at, last_parsed_idx, last_mtime, project_path)
             SELECT source, uuid, title, created_at, last_parsed_idx, last_mtime, project_path FROM temp_sessions
             ON CONFLICT (source, uuid) DO UPDATE SET
                title = EXCLUDED.title,
                created_at = EXCLUDED.created_at,
                last_parsed_idx = EXCLUDED.last_parsed_idx,
                last_mtime = EXCLUDED.last_mtime,
                project_path = EXCLUDED.project_path",
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

    // 2. 使用 Config 配置超时时间并进行连接，快速失败防止大盘加载挂起
    let mut config: postgres::config::Config = db_url.parse().map_err(|e: postgres::Error| format!("解析 PostgreSQL 连接 URL 失败: {}", e))?;
    config.connect_timeout(std::time::Duration::from_secs(5));
    let mut pg_client = config.connect(postgres::NoTls).map_err(|e| format!("无法连接到远程 PostgreSQL 数据库: {}", e))?;

    // 运行 Postgres 数据库迁移以保证表结构最新 (包含新列如 latency 和 tps)
    {
        let db_conn = crate::db_adapter::DbConn::Postgres(std::sync::Mutex::new(pg_client));
        crate::db_adapter::init_tables(&db_conn).map_err(|e| format!("执行 PostgreSQL 数据库迁移失败: {}", e))?;
        pg_client = match db_conn {
            crate::db_adapter::DbConn::Postgres(mutex) => mutex.into_inner().map_err(|_| "无法重新获取 PostgreSQL 客户端".to_string())?,
            _ => unreachable!(),
        };
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
            param_idx_cache += 1;
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
            param_idx_raw += 1;
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
        model_performance,
        performance_trends,
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
            "远程 PostgreSQL 数据库优化完成！\n共清理无效交互: {} 轮\n共删除僵尸空会话: {} 个",
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
        "本地 SQLite 缓存数据库优化瘦身成功！\n共清理无效交互: {} 轮\n共删除僵尸空会话: {} 个\n物理磁盘碎片整理已生效 (VACUUM)",
        deleted_turns, deleted_sessions
    ))
}



