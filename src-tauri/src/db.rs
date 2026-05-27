use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
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

    conn.execute(
        "CREATE TABLE IF NOT EXISTS sessions (
            uuid TEXT PRIMARY KEY,
            title TEXT,
            created_at TEXT,
            last_parsed_idx INTEGER DEFAULT -1
        )",
        [],
    )?;

    // 检查并升级 sessions 表结构，添加 last_mtime
    let has_mtime: Result<i32, _> = conn.query_row(
        "SELECT 1 FROM pragma_table_info('sessions') WHERE name='last_mtime'",
        [],
        |_| Ok(1),
    );
    if has_mtime.is_err() {
        let _ = conn.execute("ALTER TABLE sessions ADD COLUMN last_mtime REAL DEFAULT 0.0", []);
    }

    conn.execute(
        "CREATE TABLE IF NOT EXISTS turns (
            uuid TEXT,
            idx INTEGER,
            model TEXT,
            uncached_input INTEGER,
            cached_input INTEGER,
            output INTEGER,
            thinking INTEGER,
            PRIMARY KEY (uuid, idx)
        )",
        [],
    )?;

    conn.execute(
        "UPDATE turns SET model = 'gemini-3.5-flash' WHERE model = 'gemini-3-flash-a'",
        [],
    )?;

    Ok(())
}

// 4. 增量扫描逻辑与数据同步

pub fn sync_and_collect_metrics() -> Result<AggregatedMetrics, rusqlite::Error> {
    let db_dir = get_conversations_dir();
    if !db_dir.exists() {
        return get_aggregated_metrics_from_cache();
    }

    let mut active_uuids = std::collections::HashSet::new();
    let mut db_files = Vec::new();

    if let Ok(entries) = std::fs::read_dir(db_dir) {
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

    let cache_path = get_db_cache_path();
    let mut conn_cache = rusqlite::Connection::open(&cache_path)?;

    // A. 自动同步逻辑：如果本地数据库已被物理删除，清理本地缓存
    let cached_uuids: std::collections::HashSet<String> = {
        let mut stmt = conn_cache.prepare("SELECT uuid FROM sessions")?;
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
                tx.execute("DELETE FROM sessions WHERE uuid = ?", [uuid])?;
                tx.execute("DELETE FROM turns WHERE uuid = ?", [uuid])?;
            }
        }
        tx.commit()?;
        println!("Removed deleted sessions from cache: {:?}", deleted_uuids);
    }

    // B. 增量解析，每个会话只拉取新交互数据
    for db_path in db_files {
        let uuid = db_path.file_stem().unwrap().to_str().unwrap().to_string();
        let mtime = match std::fs::metadata(&db_path).and_then(|m| m.modified()) {
            Ok(t) => t
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0),
            Err(_) => 0.0,
        };

        let session_row: Result<(i64, f64, String), _> = conn_cache.query_row(
            "SELECT last_parsed_idx, last_mtime, title FROM sessions WHERE uuid = ?",
            [&uuid],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        );

        let mut last_parsed_idx = -1i64;
        let mut last_mtime = 0.0f64;
        let mut existing_title = String::new();
        let mut is_new_session = true;

        if let Ok((parsed_idx, m, title)) = session_row {
            last_parsed_idx = parsed_idx;
            last_mtime = m;
            existing_title = title;
            is_new_session = false;
        }

        // 超级优化：如果文件修改时间无任何变动，且不是新会话，则直接跳过数据库连接和打开操作
        if !is_new_session && (last_mtime - mtime).abs() < 1e-4 {
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
            for turn in &new_turns {
                tx.execute(
                    "INSERT OR REPLACE INTO turns (uuid, idx, model, uncached_input, cached_input, output, thinking) VALUES (?, ?, ?, ?, ?, ?, ?)",
                    rusqlite::params![turn.0, turn.1, turn.2, turn.3, turn.4, turn.5, turn.6],
                )?;
            }

            if is_new_session || existing_title.starts_with("Unknown Session") {
                let (title, created_at) = extract_convo_info(&uuid, &db_path);
                tx.execute(
                    "INSERT OR REPLACE INTO sessions (uuid, title, created_at, last_parsed_idx, last_mtime) VALUES (?, ?, ?, ?, ?)",
                    rusqlite::params![uuid, title, created_at, max_idx_in_db, mtime],
                )?;
            } else {
                tx.execute(
                    "UPDATE sessions SET last_parsed_idx = ?, last_mtime = ? WHERE uuid = ?",
                    rusqlite::params![max_idx_in_db, mtime, uuid],
                )?;
            }
        }
        tx.commit()?;
    }

    get_aggregated_metrics_from_cache()
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
    pub uuid: String,
    pub title: String,
    pub created_at: String,
    pub input: i64,
    pub output: i64,
    pub cached: i64,
    pub thinking: i64,
    pub models: Vec<String>,
}

#[derive(Serialize)]
pub struct AggregatedMetrics {
    pub totals: Totals,
    pub daily_trends: Vec<DailyTrend>,
    pub monthly_summary: Vec<MonthlySummary>,
    pub model_distribution: Vec<ModelDistribution>,
    pub sessions: Vec<SessionItem>,
}

pub fn get_aggregated_metrics_from_cache() -> Result<AggregatedMetrics, rusqlite::Error> {
    let db_path = get_db_cache_path();
    let conn = rusqlite::Connection::open(&db_path)?;

    // A. Totals 全局指标
    let row: Result<(Option<i64>, Option<i64>, Option<i64>, Option<i64>), _> = conn.query_row(
        "SELECT 
            SUM(uncached_input + cached_input) as total_input,
            SUM(output) as total_output,
            SUM(cached_input) as total_cached,
            SUM(thinking) as total_thinking
        FROM turns",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    );

    let (sum_input, sum_output, sum_cached, sum_thinking) = row.unwrap_or((None, None, None, None));
    let total_input = sum_input.unwrap_or(0);
    let total_output = sum_output.unwrap_or(0);
    let total_cached = sum_cached.unwrap_or(0);
    let total_thinking = sum_thinking.unwrap_or(0);

    let total_sessions: i64 = conn.query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))?;

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
    };

    // B. 每日用量序列 (按会话创建日期进行 GROUP BY 聚合)
    let mut stmt = conn.prepare(
        "SELECT 
            substr(s.created_at, 1, 10) as date,
            SUM(t.uncached_input + t.cached_input) as input,
            SUM(t.output) as output,
            SUM(t.cached_input) as cached,
            SUM(t.thinking) as thinking,
            COUNT(DISTINCT s.uuid) as sessions
        FROM sessions s
        LEFT JOIN turns t ON s.uuid = t.uuid
        GROUP BY date
        ORDER BY date ASC",
    )?;
    let daily_trends: Vec<DailyTrend> = stmt
        .query_map([], |r| {
            let date: Option<String> = r.get(0)?;
            let input: Option<i64> = r.get(1)?;
            let output: Option<i64> = r.get(2)?;
            let cached: Option<i64> = r.get(3)?;
            let thinking: Option<i64> = r.get(4)?;
            let sessions: i64 = r.get(5)?;
            Ok(DailyTrend {
                date: date.unwrap_or_default(),
                input: input.unwrap_or(0),
                output: output.unwrap_or(0),
                cached: cached.unwrap_or(0),
                thinking: thinking.unwrap_or(0),
                sessions,
            })
        })?
        .flatten()
        .filter(|item| !item.date.is_empty())
        .collect();

    // C. 按月聚合汇总
    let mut stmt = conn.prepare(
        "SELECT 
            substr(s.created_at, 1, 7) as month,
            SUM(t.uncached_input + t.cached_input) as input,
            SUM(t.output) as output,
            SUM(t.cached_input) as cached,
            SUM(t.thinking) as thinking,
            COUNT(DISTINCT s.uuid) as sessions
        FROM sessions s
        LEFT JOIN turns t ON s.uuid = t.uuid
        GROUP BY month
        ORDER BY month DESC",
    )?;
    let monthly_summary: Vec<MonthlySummary> = stmt
        .query_map([], |r| {
            let month: Option<String> = r.get(0)?;
            let input: Option<i64> = r.get(1)?;
            let output: Option<i64> = r.get(2)?;
            let cached: Option<i64> = r.get(3)?;
            let thinking: Option<i64> = r.get(4)?;
            let sessions: i64 = r.get(5)?;
            Ok(MonthlySummary {
                month: month.unwrap_or_default(),
                input: input.unwrap_or(0),
                output: output.unwrap_or(0),
                cached: cached.unwrap_or(0),
                thinking: thinking.unwrap_or(0),
                sessions,
            })
        })?
        .flatten()
        .filter(|item| !item.month.is_empty())
        .collect();

    // D. 底层模型分布
    let mut stmt = conn.prepare(
        "SELECT 
            CASE WHEN model = 'gemini-3-flash-a' THEN 'gemini-3.5-flash' ELSE model END as model_mapped,
            SUM(uncached_input + cached_input) as input,
            SUM(output) as output,
            SUM(cached_input) as cached,
            SUM(thinking) as thinking,
            SUM(uncached_input + cached_input + output) as total_tokens
        FROM turns
        WHERE model IS NOT NULL AND model != 'unknown' AND model != ''
        GROUP BY model_mapped
        ORDER BY total_tokens DESC",
    )?;
    let model_distribution: Vec<ModelDistribution> = stmt
        .query_map([], |r| {
            let model: String = r.get(0)?;
            let input: Option<i64> = r.get(1)?;
            let output: Option<i64> = r.get(2)?;
            let cached: Option<i64> = r.get(3)?;
            let thinking: Option<i64> = r.get(4)?;
            let total_tokens: Option<i64> = r.get(5)?;
            Ok(ModelDistribution {
                model,
                input: input.unwrap_or(0),
                output: output.unwrap_or(0),
                cached: cached.unwrap_or(0),
                thinking: thinking.unwrap_or(0),
                total_tokens: total_tokens.unwrap_or(0),
            })
        })?
        .flatten()
        .collect();

    // E. 会话详细明细
    let mut stmt = conn.prepare(
        "SELECT 
            s.uuid,
            s.title,
            s.created_at,
            SUM(t.uncached_input + t.cached_input) as input,
            SUM(t.output) as output,
            SUM(t.cached_input) as cached,
            SUM(t.thinking) as thinking
        FROM sessions s
        LEFT JOIN turns t ON s.uuid = t.uuid
        GROUP BY s.uuid
        ORDER BY s.created_at DESC",
    )?;
    let session_rows: Vec<(String, String, String, i64, i64, i64, i64)> = stmt
        .query_map([], |r| {
            let uuid: String = r.get(0)?;
            let title: Option<String> = r.get(1)?;
            let created_at: Option<String> = r.get(2)?;
            let input: Option<i64> = r.get(3)?;
            let output: Option<i64> = r.get(4)?;
            let cached: Option<i64> = r.get(5)?;
            let thinking: Option<i64> = r.get(6)?;
            Ok((
                uuid,
                title.unwrap_or_default(),
                created_at.unwrap_or_default(),
                input.unwrap_or(0),
                output.unwrap_or(0),
                cached.unwrap_or(0),
                thinking.unwrap_or(0),
            ))
        })?
        .flatten()
        .collect();

    // 额外提取每个会话使用到的引擎去重列表
    let mut stmt = conn.prepare(
        "SELECT uuid, CASE WHEN model = 'gemini-3-flash-a' THEN 'gemini-3.5-flash' ELSE model END as model_mapped
        FROM turns 
        WHERE model IS NOT NULL AND model != 'unknown' AND model != ''
        GROUP BY uuid, model_mapped",
    )?;
    let mut model_map: HashMap<String, Vec<String>> = HashMap::new();
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let uuid: String = row.get(0)?;
        let model: String = row.get(1)?;
        model_map.entry(uuid).or_default().push(model);
    }

    let sessions = session_rows
        .into_iter()
        .map(|(uuid, title, created_at, input, output, cached, thinking)| {
            let models = model_map
                .get(&uuid)
                .cloned()
                .unwrap_or_else(|| vec!["unknown".to_string()]);
            SessionItem {
                uuid,
                title,
                created_at,
                input,
                output,
                cached,
                thinking,
                models,
            }
        })
        .collect();

    Ok(AggregatedMetrics {
        totals,
        daily_trends,
        monthly_summary,
        model_distribution,
        sessions,
    })
}
