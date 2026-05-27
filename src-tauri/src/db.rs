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

    // 首先检查是否存在 sessions 表
    let sessions_exists: Result<i32, _> = conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type='table' AND name='sessions'",
        [],
        |r| r.get(0),
    );

    let mut should_migrate = false;
    if sessions_exists.is_ok() {
        // 如果 sessions 表存在，检查表结构是否已升级为包含 source 列的新版表结构
        let has_source: Result<i32, _> = conn.query_row(
            "SELECT 1 FROM pragma_table_info('sessions') WHERE name='source'",
            [],
            |_| Ok(1),
        );
        if has_source.is_err() {
            should_migrate = true;
        }
    }

    if should_migrate {
        println!("检测到旧版数据库表结构，正在执行平滑迁移...");
        // 开启数据库迁移事务
        let _ = conn.execute("BEGIN TRANSACTION;", []);

        // 1. 将 old 表重命名
        let rename_sessions = conn.execute("ALTER TABLE sessions RENAME TO sessions_old;", []);
        let rename_turns = conn.execute("ALTER TABLE turns RENAME TO turns_old;", []);

        if rename_sessions.is_ok() && rename_turns.is_ok() {
            // 2. 创建基于联合主键的新表结构
            conn.execute(
                "CREATE TABLE sessions (
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
                "CREATE TABLE turns (
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
                    PRIMARY KEY (source, uuid, idx),
                    FOREIGN KEY(source, uuid) REFERENCES sessions(source, uuid) ON DELETE CASCADE
                )",
                [],
            )?;

            // 3. 将旧 sessions 表数据迁移至新表（source = 'antigravity'）
            conn.execute(
                "INSERT INTO sessions (source, uuid, title, created_at, last_parsed_idx, last_mtime)
                 SELECT 'antigravity', uuid, title, created_at, last_parsed_idx, last_mtime
                 FROM sessions_old",
                [],
            )?;

            // 4. 将旧 turns 表数据迁移至新表（source = 'antigravity'，合并 input_tokens，估算 cost_usd）
            conn.execute(
                "INSERT INTO turns (source, uuid, idx, model, input_tokens, cached_input_tokens, output_tokens, thinking_tokens, cost_usd, message_id, request_id)
                 SELECT 'antigravity', uuid, idx, model, (uncached_input + cached_input), cached_input, output, thinking, 0.0, '', 'unknown'
                 FROM turns_old",
                [],
            )?;

            // 5. 对历史的 antigravity 轮次数据根据模型进行费用估算
            conn.execute(
                "UPDATE turns SET cost_usd = (
                    CASE 
                        WHEN model LIKE '%pro%' THEN (input_tokens * 1.25 / 1000000.0 + output_tokens * 5.0 / 1000000.0)
                        ELSE (input_tokens * 0.075 / 1000000.0 + output_tokens * 0.3 / 1000000.0)
                    END
                ) WHERE source = 'antigravity'",
                [],
            )?;

            // 6. 清理旧临时表
            let _ = conn.execute("DROP TABLE sessions_old;", []);
            let _ = conn.execute("DROP TABLE turns_old;", []);
            let _ = conn.execute("COMMIT;", []);
            println!("旧版数据平滑迁移完成！已升级为多源统计表结构。");
        } else {
            let _ = conn.execute("ROLLBACK;", []);
            eprintln!("重命名旧表失败，取消迁移。");
        }
    } else {
        // 如果已是新结构，则直接执行 CREATE TABLE IF NOT EXISTS 以做安全保障
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
                PRIMARY KEY (source, uuid, idx),
                FOREIGN KEY(source, uuid) REFERENCES sessions(source, uuid) ON DELETE CASCADE
            )",
            [],
        )?;
    }

    conn.execute(
        "UPDATE turns SET model = 'gemini-3.5-flash' WHERE model = 'gemini-3-flash-a'",
        [],
    )?;

    Ok(())
}

// 4. 增量扫描逻辑与数据同步

#[derive(Clone, Serialize)]
pub struct ScanStatus {
    pub is_scanning: bool,
    pub total_files: usize,
    pub scanned_files: usize,
    pub error: Option<String>,
}

pub static DB_LOCK: Mutex<()> = Mutex::new(());

pub fn get_scan_status() -> &'static Mutex<ScanStatus> {
    static STATUS: OnceLock<Mutex<ScanStatus>> = OnceLock::new();
    STATUS.get_or_init(|| {
        Mutex::new(ScanStatus {
            is_scanning: false,
            total_files: 0,
            scanned_files: 0,
            error: None,
        })
    })
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
                            let cost = estimate_cost(&model, input, cache_read, output);

                            new_turns.push((
                                line_idx - 1,
                                model,
                                input,
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
                            let cost = estimate_cost(&model, input, cache_read, output);

                            new_turns.push((
                                line_idx - 1,
                                model,
                                input,
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

    // 计算总文件数
    let total_files = db_files.len() + claude_files.len() + codex_files.len();
    progress_cb(0, total_files);

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
        progress_cb(i + 1, total_files);
    }

    // D. 增量同步 Claude Code 数据
    let _ = sync_claude_code(&mut conn_cache, db_files_len, total_files, &progress_cb);

    // E. 增量同步 Codex 数据
    let _ = sync_codex(&mut conn_cache, db_files_len + claude_files.len(), total_files, &progress_cb);

    // F. 如果配置了 PostgreSQL 模式，自动将本地 SQLite 增量好的最新数据一键同步至 PostgreSQL
    let _ = sync_local_to_postgres();

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
pub struct AggregatedMetrics {
    pub totals: Totals,
    pub daily_trends: Vec<DailyTrend>,
    pub monthly_summary: Vec<MonthlySummary>,
    pub model_distribution: Vec<ModelDistribution>,
    pub sessions: Vec<SessionItem>,
    pub source_trends: Vec<SourceTrend>,
}

pub fn get_aggregated_metrics_from_cache(
    source_filter: Option<&str>,
    start_date: Option<&str>,
    end_date: Option<&str>,
) -> Result<AggregatedMetrics, rusqlite::Error> {
    let _ = dotenvy::dotenv();
    let db_type = std::env::var("DATABASE_TYPE").unwrap_or_else(|_| "sqlite".to_string());
    if db_type.to_lowercase() == "postgres" {
        return get_pg_aggregated_metrics(source_filter, start_date, end_date)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(std::io::ErrorKind::Other, e))));
    }

    let db_path = get_db_cache_path();
    let conn = rusqlite::Connection::open(&db_path)?;


    // 构造动态 SQL WHERE 子句与参数绑定
    let mut conditions = Vec::new();
    let mut params: Vec<rusqlite::types::Value> = Vec::new();

    if let Some(src) = source_filter {
        if src != "all" {
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

    let where_clause = if conditions.is_empty() {
        "".to_string()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    // A. Totals 全局指标 (INNER JOIN sessions 确保过滤生效)
    let sql_totals = format!(
        "SELECT 
            SUM(t.input_tokens) as total_input,
            SUM(t.output_tokens) as total_output,
            SUM(t.cached_input_tokens) as total_cached,
            SUM(t.thinking_tokens) as total_thinking,
            SUM(t.cost_usd) as total_cost
        FROM turns t
        INNER JOIN sessions s ON t.source = s.source AND t.uuid = s.uuid
        {}",
        where_clause
    );

    let row: Result<(Option<i64>, Option<i64>, Option<i64>, Option<i64>, Option<f64>), _> = 
        conn.query_row(&sql_totals, rusqlite::params_from_iter(params.clone()), |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        });

    let (sum_input, sum_output, sum_cached, sum_thinking, sum_cost) = row.unwrap_or((None, None, None, None, None));
    let total_input = sum_input.unwrap_or(0);
    let total_output = sum_output.unwrap_or(0);
    let total_cached = sum_cached.unwrap_or(0);
    let total_thinking = sum_thinking.unwrap_or(0);
    let total_cost = sum_cost.unwrap_or(0.0);

    let sql_sessions_count = format!("SELECT COUNT(*) FROM sessions s {}", where_clause);
    let total_sessions: i64 = conn.query_row(&sql_sessions_count, rusqlite::params_from_iter(params.clone()), |r| r.get(0))?;

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

    // B. 每日用量序列 (按会话创建日期进行 GROUP BY 聚合)
    let sql_daily = format!(
        "SELECT 
            substr(s.created_at, 1, 10) as date,
            SUM(t.input_tokens) as input,
            SUM(t.output_tokens) as output,
            SUM(t.cached_input_tokens) as cached,
            SUM(t.thinking_tokens) as thinking,
            COUNT(DISTINCT s.uuid) as sessions
        FROM sessions s
        LEFT JOIN turns t ON s.source = t.source AND s.uuid = t.uuid
        {}
        GROUP BY date
        ORDER BY date ASC",
        where_clause
    );

    let mut stmt = conn.prepare(&sql_daily)?;
    let mut rows = stmt.query(rusqlite::params_from_iter(params.clone()))?;

    let mut daily_trends = Vec::new();
    while let Some(row) = rows.next()? {
        let date: Option<String> = row.get(0)?;
        let input: Option<i64> = row.get(1)?;
        let output: Option<i64> = row.get(2)?;
        let cached: Option<i64> = row.get(3)?;
        let thinking: Option<i64> = row.get(4)?;
        let sessions: i64 = row.get(5)?;
        daily_trends.push(DailyTrend {
            date: date.unwrap_or_default(),
            input: input.unwrap_or(0),
            output: output.unwrap_or(0),
            cached: cached.unwrap_or(0),
            thinking: thinking.unwrap_or(0),
            sessions,
        });
    }

    // C. 按月聚合汇总
    let sql_monthly = format!(
        "SELECT 
            substr(s.created_at, 1, 7) as month,
            SUM(t.input_tokens) as input,
            SUM(t.output_tokens) as output,
            SUM(t.cached_input_tokens) as cached,
            SUM(t.thinking_tokens) as thinking,
            COUNT(DISTINCT s.uuid) as sessions
        FROM sessions s
        LEFT JOIN turns t ON s.source = t.source AND s.uuid = t.uuid
        {}
        GROUP BY month
        ORDER BY month DESC",
        where_clause
    );

    let mut stmt = conn.prepare(&sql_monthly)?;
    let mut rows = stmt.query(rusqlite::params_from_iter(params.clone()))?;

    let mut monthly_summary = Vec::new();
    while let Some(row) = rows.next()? {
        let month: Option<String> = row.get(0)?;
        let input: Option<i64> = row.get(1)?;
        let output: Option<i64> = row.get(2)?;
        let cached: Option<i64> = row.get(3)?;
        let thinking: Option<i64> = row.get(4)?;
        let sessions: i64 = row.get(5)?;
        monthly_summary.push(MonthlySummary {
            month: month.unwrap_or_default(),
            input: input.unwrap_or(0),
            output: output.unwrap_or(0),
            cached: cached.unwrap_or(0),
            thinking: thinking.unwrap_or(0),
            sessions,
        });
    }

    // D. 底层模型分布
    let sql_model_dist = format!(
        "SELECT 
            CASE WHEN t.model = 'gemini-3-flash-a' THEN 'gemini-3.5-flash' ELSE t.model END as model_mapped,
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
        where_clause,
        if where_clause.is_empty() { "WHERE t.model IS NOT NULL AND t.model != 'unknown' AND t.model != ''" } else { "AND t.model IS NOT NULL AND t.model != 'unknown' AND t.model != ''" }
    );

    let mut stmt = conn.prepare(&sql_model_dist)?;
    let mut rows = stmt.query(rusqlite::params_from_iter(params.clone()))?;

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

    // E. 会话详细明细
    let sql_sessions = format!(
        "SELECT 
            s.source,
            s.uuid,
            s.title,
            s.created_at,
            SUM(t.input_tokens) as input,
            SUM(t.output_tokens) as output,
            SUM(t.cached_input_tokens) as cached,
            SUM(t.thinking_tokens) as thinking,
            SUM(t.cost_usd) as cost_usd
        FROM sessions s
        LEFT JOIN turns t ON s.source = t.source AND s.uuid = t.uuid
        {}
        GROUP BY s.source, s.uuid
        ORDER BY s.created_at DESC",
        where_clause
    );

    let mut stmt = conn.prepare(&sql_sessions)?;
    let session_rows: Vec<(String, String, String, String, i64, i64, i64, i64, f64)> = stmt
        .query_map(rusqlite::params_from_iter(params.clone()), |r| {
            let source: String = r.get(0)?;
            let uuid: String = r.get(1)?;
            let title: Option<String> = r.get(2)?;
            let created_at: Option<String> = r.get(3)?;
            let input: Option<i64> = r.get(4)?;
            let output: Option<i64> = r.get(5)?;
            let cached: Option<i64> = r.get(6)?;
            let thinking: Option<i64> = r.get(7)?;
            let cost_usd: Option<f64> = r.get(8)?;
            Ok((
                source,
                uuid,
                title.unwrap_or_default(),
                created_at.unwrap_or_default(),
                input.unwrap_or(0),
                output.unwrap_or(0),
                cached.unwrap_or(0),
                thinking.unwrap_or(0),
                cost_usd.unwrap_or(0.0),
            ))
        })?
        .flatten()
        .collect();

    // 额外提取每个会话使用到的引擎去重列表
    let sql_models = format!(
        "SELECT t.source, t.uuid, CASE WHEN t.model = 'gemini-3-flash-a' THEN 'gemini-3.5-flash' ELSE t.model END as model_mapped
        FROM turns t
        INNER JOIN sessions s ON t.source = s.source AND t.uuid = s.uuid
        {} {}
        GROUP BY t.source, t.uuid, model_mapped",
        where_clause,
        if where_clause.is_empty() { "WHERE t.model IS NOT NULL AND t.model != 'unknown' AND t.model != ''" } else { "AND t.model IS NOT NULL AND t.model != 'unknown' AND t.model != ''" }
    );

    let mut stmt = conn.prepare(&sql_models)?;
    let mut model_map: HashMap<String, Vec<String>> = HashMap::new();
    let mut rows = stmt.query(rusqlite::params_from_iter(params.clone()))?;

    while let Some(row) = rows.next()? {
        let src: String = row.get(0)?;
        let uuid: String = row.get(1)?;
        let model: String = row.get(2)?;
        let key = format!("{}:{}", src, uuid);
        model_map.entry(key).or_default().push(model);
    }

    let sessions = session_rows
        .into_iter()
        .map(|(source, uuid, title, created_at, input, output, cached, thinking, cost_usd)| {
            let key = format!("{}:{}", source, uuid);
            let models = model_map
                .get(&key)
                .cloned()
                .unwrap_or_else(|| vec!["unknown".to_string()]);
            SessionItem {
                source,
                uuid,
                title,
                created_at,
                input,
                output,
                cached,
                thinking,
                cost_usd,
                models,
            }
        })
        .collect();

    // F. 新增：多引擎用量每日对比走势 (SourceTrends)
    let sql_source_trends = format!(
        "SELECT 
            substr(s.created_at, 1, 10) as date,
            s.source,
            SUM(t.input_tokens + t.output_tokens) as total_tokens,
            SUM(t.cost_usd) as cost
        FROM sessions s
        LEFT JOIN turns t ON s.source = t.source AND s.uuid = t.uuid
        {}
        GROUP BY date, s.source
        ORDER BY date ASC, s.source ASC",
        where_clause
    );

    let mut stmt = conn.prepare(&sql_source_trends)?;
    let mut rows = stmt.query(rusqlite::params_from_iter(params.clone()))?;

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

    Ok(AggregatedMetrics {
        totals,
        daily_trends,
        monthly_summary,
        model_distribution,
        sessions,
        source_trends,
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

        let metrics_all_3 = get_aggregated_metrics_from_cache(None, None, None).unwrap();
        assert_eq!(metrics_all_3.totals.total_sessions, 2);
        assert_eq!(metrics_all_3.totals.total_input, 1400);
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
        println!("所有本地数据与远程 PostgreSQL 保持一致，无需增量同步。");
        return Ok(());
    }

    println!("检测到有 {} 个会话存在更新，正在进行增量差分同步...", sessions_to_sync.len());

    // 5. 开启 PG 事务，批量/增量镜像同步变动部分
    let mut pg_tx = pg_client.transaction().map_err(|e| e.to_string())?;

    for (source, uuid, title, created_at, last_parsed_idx, last_mtime, project_path, pg_last_parsed_idx) in sessions_to_sync {
        // A. 同步会话表状态
        pg_tx.execute(
            "INSERT INTO sessions (source, uuid, title, created_at, last_parsed_idx, last_mtime, project_path)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (source, uuid) DO UPDATE SET
                title = EXCLUDED.title,
                created_at = EXCLUDED.created_at,
                last_parsed_idx = EXCLUDED.last_parsed_idx,
                last_mtime = EXCLUDED.last_mtime,
                project_path = EXCLUDED.project_path",
            &[&source, &uuid, &title, &created_at, &last_parsed_idx, &last_mtime, &project_path],
        ).map_err(|e| format!("同步会话记录失败: {}", e))?;

        // B. 只查询本地 SQLite 中 idx 大于 PostgreSQL 侧已记录最大 index 的增量 turns 并同步
        let mut sqlite_turns_stmt = sqlite_conn
            .prepare(
                "SELECT source, uuid, idx, model, input_tokens, cached_input_tokens, output_tokens, thinking_tokens, cost_usd, message_id, request_id, timestamp 
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

            pg_tx.execute(
                "INSERT INTO turns (source, uuid, idx, model, input_tokens, cached_input_tokens, output_tokens, thinking_tokens, cost_usd, message_id, request_id, timestamp)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                 ON CONFLICT (source, uuid, idx) DO UPDATE SET
                    model = EXCLUDED.model,
                    input_tokens = EXCLUDED.input_tokens,
                    cached_input_tokens = EXCLUDED.cached_input_tokens,
                    output_tokens = EXCLUDED.output_tokens,
                    thinking_tokens = EXCLUDED.thinking_tokens,
                    cost_usd = EXCLUDED.cost_usd,
                    message_id = EXCLUDED.message_id,
                    request_id = EXCLUDED.request_id,
                    timestamp = EXCLUDED.timestamp",
                &[&src, &uid, &idx, &model, &input_tokens, &cached_input_tokens, &output_tokens, &thinking_tokens, &cost_usd, &message_id, &request_id, &timestamp],
            ).map_err(|e| format!("同步轮次记录失败: {}", e))?;
        }
    }

    pg_tx.commit().map_err(|e| format!("提交 PostgreSQL 同步事务失败: {}", e))?;
    println!("SQLite 本地增量数据镜像成功同步到远程 PostgreSQL 数据库！");
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

    let mut conditions = Vec::new();
    let mut params: Vec<String> = Vec::new();
    let mut param_idx = 1;

    if let Some(src) = source_filter {
        if src != "all" {
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

    let where_clause = if conditions.is_empty() {
        "".to_string()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let mut pg_params: Vec<&(dyn postgres::types::ToSql + Sync)> = Vec::new();
    for p in &params {
        pg_params.push(p);
    }

    // 1. Totals (CAST AS BIGINT 防止 PG SUM(bigint) 默认返回 NUMERIC 类型引起 Rust 侧反序列化 Panic)
    let sql_totals = format!(
        "SELECT 
            CAST(SUM(t.input_tokens) AS BIGINT) as total_input,
            CAST(SUM(t.output_tokens) AS BIGINT) as total_output,
            CAST(SUM(t.cached_input_tokens) AS BIGINT) as total_cached,
            CAST(SUM(t.thinking_tokens) AS BIGINT) as total_thinking,
            SUM(t.cost_usd) as total_cost
        FROM turns t
        INNER JOIN sessions s ON t.source = s.source AND t.uuid = s.uuid
        {}",
        where_clause
    );

    let row = pg_client.query_one(&sql_totals, &pg_params[..]).map_err(|e| e.to_string())?;
    let sum_input: Option<i64> = row.get(0);
    let sum_output: Option<i64> = row.get(1);
    let sum_cached: Option<i64> = row.get(2);
    let sum_thinking: Option<i64> = row.get(3);
    let sum_cost: Option<f64> = row.get(4);

    let total_input = sum_input.unwrap_or(0);
    let total_output = sum_output.unwrap_or(0);
    let total_cached = sum_cached.unwrap_or(0);
    let total_thinking = sum_thinking.unwrap_or(0);
    let total_cost = sum_cost.unwrap_or(0.0);

    let sql_sessions_count = format!("SELECT COUNT(*) FROM sessions s {}", where_clause);
    let total_sessions: i64 = pg_client.query_one(&sql_sessions_count, &pg_params[..])
        .map_err(|e| e.to_string())?
        .get(0);

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
            SUBSTR(s.created_at, 1, 10) as date,
            CAST(SUM(t.input_tokens) AS BIGINT) as input,
            CAST(SUM(t.output_tokens) AS BIGINT) as output,
            CAST(SUM(t.cached_input_tokens) AS BIGINT) as cached,
            CAST(SUM(t.thinking_tokens) AS BIGINT) as thinking,
            COUNT(DISTINCT s.uuid) as sessions
        FROM sessions s
        LEFT JOIN turns t ON s.source = t.source AND s.uuid = t.uuid
        {}
        GROUP BY date
        ORDER BY date ASC",
        where_clause
    );

    let rows_daily = pg_client.query(&sql_daily, &pg_params[..]).map_err(|e| e.to_string())?;
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
            SUBSTR(s.created_at, 1, 7) as month,
            CAST(SUM(t.input_tokens) AS BIGINT) as input,
            CAST(SUM(t.output_tokens) AS BIGINT) as output,
            CAST(SUM(t.cached_input_tokens) AS BIGINT) as cached,
            CAST(SUM(t.thinking_tokens) AS BIGINT) as thinking,
            COUNT(DISTINCT s.uuid) as sessions
        FROM sessions s
        LEFT JOIN turns t ON s.source = t.source AND s.uuid = t.uuid
        {}
        GROUP BY month
        ORDER BY month DESC",
        where_clause
    );

    let rows_monthly = pg_client.query(&sql_monthly, &pg_params[..]).map_err(|e| e.to_string())?;
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

    // 4. Model Distribution
    let sql_model = format!(
        "SELECT 
            t.model,
            CAST(SUM(t.input_tokens) AS BIGINT) as input,
            CAST(SUM(t.output_tokens) AS BIGINT) as output,
            CAST(SUM(t.cached_input_tokens) AS BIGINT) as cached,
            CAST(SUM(t.thinking_tokens) AS BIGINT) as thinking
        FROM turns t
        INNER JOIN sessions s ON t.source = s.source AND t.uuid = s.uuid
        {}
        GROUP BY t.model
        ORDER BY SUM(t.input_tokens + t.output_tokens) DESC",
        where_clause
    );

    let rows_model = pg_client.query(&sql_model, &pg_params[..]).map_err(|e| e.to_string())?;
    let mut model_distribution = Vec::new();
    for r in rows_model {
        let model: Option<String> = r.get(0);
        let input: Option<i64> = r.get(1);
        let output: Option<i64> = r.get(2);
        let cached: Option<i64> = r.get(3);
        let thinking: Option<i64> = r.get(4);
        let total_tokens = input.unwrap_or(0) + output.unwrap_or(0);
        model_distribution.push(ModelDistribution {
            model: model.unwrap_or_else(|| "unknown".to_string()),
            input: input.unwrap_or(0),
            output: output.unwrap_or(0),
            cached: cached.unwrap_or(0),
            thinking: thinking.unwrap_or(0),
            total_tokens,
        });
    }

    // 5. Sessions明细
    let sql_sessions = format!(
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
        FROM sessions s
        LEFT JOIN turns t ON s.source = t.source AND s.uuid = t.uuid
        {}
        GROUP BY s.source, s.uuid, s.title, s.created_at
        ORDER BY s.created_at DESC",
        where_clause
    );

    let rows_sessions = pg_client.query(&sql_sessions, &pg_params[..]).map_err(|e| e.to_string())?;
    let mut sessions = Vec::new();
    for r in rows_sessions {
        let source: String = r.get(0);
        let uuid: String = r.get(1);
        let title: Option<String> = r.get(2);
        let created_at: Option<String> = r.get(3);
        let input: i64 = r.get(4);
        let output: i64 = r.get(5);
        let cached: i64 = r.get(6);
        let thinking: i64 = r.get(7);
        let cost_usd: f64 = r.get(8);

        let sql_session_models = "SELECT DISTINCT model FROM turns WHERE source = $1 AND uuid = $2 AND model IS NOT NULL";
        let rows_models = pg_client.query(sql_session_models, &[&source, &uuid]).map_err(|e| e.to_string())?;
        let mut models = Vec::new();
        for mr in rows_models {
            if let Some(m) = mr.get::<_, Option<String>>(0) {
                models.push(m);
            }
        }

        sessions.push(SessionItem {
            source,
            uuid,
            title: title.unwrap_or_else(|| "Untitled Session".to_string()),
            created_at: created_at.unwrap_or_default(),
            input,
            output,
            cached,
            thinking,
            cost_usd,
            models,
        });
    }

    // 6. Source Trends
    let sql_source = format!(
        "SELECT 
            SUBSTR(s.created_at, 1, 10) as date,
            s.source,
            CAST(SUM(t.input_tokens + t.output_tokens) AS BIGINT) as tokens,
            SUM(t.cost_usd) as cost
        FROM sessions s
        LEFT JOIN turns t ON s.source = t.source AND s.uuid = t.uuid
        {}
        GROUP BY date, s.source
        ORDER BY date ASC",
        where_clause
    );

    let rows_source = pg_client.query(&sql_source, &pg_params[..]).map_err(|e| e.to_string())?;
    let mut source_trends = Vec::new();
    for r in rows_source {
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

    Ok(AggregatedMetrics {
        totals,
        daily_trends,
        monthly_summary,
        model_distribution,
        sessions,
        source_trends,
    })
}



