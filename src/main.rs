use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use axum::{
    body::Body,
    http::{header, Response, StatusCode, Uri},
    response::IntoResponse,
    routing::get,
    Router,
};
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::Serialize;

// 全局数据库操作锁，防止多线程 SQLite 写入冲突
static DB_LOCK: Mutex<()> = Mutex::new(());

// 1. 动态路径获取逻辑（适配不同 Windows 用户目录）

fn get_user_profile_dir() -> String {
    std::env::var("USERPROFILE").unwrap_or_else(|_| r"C:\Users\cearn".to_string())
}

fn get_db_cache_path() -> PathBuf {
    Path::new(&get_user_profile_dir())
        .join(".gemini")
        .join("antigravity")
        .join("token_stats.db")
}

fn get_conversations_dir() -> PathBuf {
    Path::new(&get_user_profile_dir())
        .join(".gemini")
        .join("antigravity")
        .join("conversations")
}

fn get_brain_dir() -> PathBuf {
    Path::new(&get_user_profile_dir())
        .join(".gemini")
        .join("antigravity")
        .join("brain")
}

// 2. Protobuf 动态解码与 Token 字段提取逻辑

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
enum ProtoValue {
    Varint(u64),
    Fixed64(Vec<u8>), // 8 bytes
    Bytes(Vec<u8>),
    SubMessage(HashMap<u32, Vec<ProtoValue>>),
    String(String),
    Fixed32(Vec<u8>), // 4 bytes
}

fn read_varint(data: &[u8], pos: &mut usize) -> Result<u64, String> {
    let mut result = 0u64;
    let mut shift = 0;
    loop {
        if *pos >= data.len() {
            return Err("Unexpected EOF while reading varint".to_string());
        }
        let b = data[*pos];
        *pos += 1;
        result |= ((b & 0x7f) as u64) << shift;
        if (b & 0x80) == 0 {
            break;
        }
        shift += 7;
        if shift >= 64 {
            return Err("Varint too long".to_string());
        }
    }
    Ok(result)
}

fn parse_protobuf_orig(data: &[u8], pos: &mut usize, end: usize) -> Result<HashMap<u32, Vec<ProtoValue>>, String> {
    let mut result: HashMap<u32, Vec<ProtoValue>> = HashMap::new();
    while *pos < end {
        let key = match read_varint(data, pos) {
            Ok(k) => k,
            Err(_) => break,
        };
        let wire_type = (key & 0x07) as u32;
        let field_num = (key >> 3) as u32;

        if field_num == 0 || field_num > (1 << 29) - 1 {
            return Err("Invalid field number".to_string());
        }

        match wire_type {
            0 => {
                match read_varint(data, pos) {
                    Ok(val) => {
                        result.entry(field_num).or_default().push(ProtoValue::Varint(val));
                    }
                    Err(_) => break,
                }
            }
            1 => {
                if *pos + 8 > end {
                    break;
                }
                let val = data[*pos..*pos + 8].to_vec();
                *pos += 8;
                result.entry(field_num).or_default().push(ProtoValue::Fixed64(val));
            }
            2 => {
                let length = match read_varint(data, pos) {
                    Ok(l) => l as usize,
                    Err(_) => break,
                };
                if *pos + length > end {
                    break;
                }
                let val = data[*pos..*pos + length].to_vec();
                *pos += length;
                result.entry(field_num).or_default().push(ProtoValue::Bytes(val));
            }
            5 => {
                if *pos + 4 > end {
                    break;
                }
                let val = data[*pos..*pos + 4].to_vec();
                *pos += 4;
                result.entry(field_num).or_default().push(ProtoValue::Fixed32(val));
            }
            _ => {
                return Err(format!("Unsupported wire type {}", wire_type));
            }
        }
    }
    Ok(result)
}

fn is_printable_string(s: &str) -> bool {
    s.chars().all(|c| {
        (!c.is_control() && c != '\u{2028}' && c != '\u{2029}') || c == '\n' || c == '\r' || c == '\t'
    })
}

fn try_parse_sub_messages(mut parsed_dict: HashMap<u32, Vec<ProtoValue>>) -> HashMap<u32, Vec<ProtoValue>> {
    for (_field, values) in parsed_dict.iter_mut() {
        let mut new_values = Vec::new();
        for v in values.drain(..) {
            match v {
                ProtoValue::Bytes(bytes) => {
                    let len = bytes.len();
                    let mut pos = 0;
                    if len > 0 {
                        if let Ok(sub_msg) = parse_protobuf_orig(&bytes, &mut pos, len) {
                            if pos == len && !sub_msg.is_empty() {
                                let sub_msg = try_parse_sub_messages(sub_msg);
                                new_values.push(ProtoValue::SubMessage(sub_msg));
                                continue;
                            }
                        }
                    }

                    if let Ok(s) = String::from_utf8(bytes.clone()) {
                        if is_printable_string(&s) {
                            new_values.push(ProtoValue::String(s));
                            continue;
                        }
                    }

                    new_values.push(ProtoValue::Bytes(bytes));
                }
                ProtoValue::SubMessage(sub_msg) => {
                    new_values.push(ProtoValue::SubMessage(try_parse_sub_messages(sub_msg)));
                }
                other => {
                    new_values.push(other);
                }
            }
        }
        *values = new_values;
    }
    parsed_dict
}

struct Metric {
    model: String,
    uncached_input: i64,
    cached_input: i64,
    output: i64,
    thinking: i64,
}

fn get_varint_val(val: &ProtoValue) -> i64 {
    match val {
        ProtoValue::Varint(v) => *v as i64,
        _ => 0,
    }
}

fn extract_metrics_from_proto(proto_dict: &HashMap<u32, Vec<ProtoValue>>) -> Vec<Metric> {
    let mut metrics = Vec::new();
    if let Some(items) = proto_dict.get(&1) {
        for item in items {
            if let ProtoValue::SubMessage(item_dict) = item {
                let mut model_name = "unknown".to_string();
                if let Some(field_19) = item_dict.get(&19) {
                    if let Some(val) = field_19.first() {
                        let raw_model = match val {
                            ProtoValue::String(s) => Some(s.clone()),
                            ProtoValue::Bytes(b) => String::from_utf8(b.clone()).ok(),
                            _ => None,
                        };
                        if let Some(rm) = raw_model {
                            model_name = if rm == "gemini-3-flash-a" {
                                "gemini-3.5-flash".to_string()
                            } else {
                                rm
                            };
                        }
                    }
                }
                if let Some(token_blocks) = item_dict.get(&4) {
                    for token_block in token_blocks {
                        if let ProtoValue::SubMessage(block_dict) = token_block {
                            let uncached = block_dict
                                .get(&2)
                                .and_then(|v| v.first())
                                .map(get_varint_val)
                                .unwrap_or(0);
                            let candidates = block_dict
                                .get(&3)
                                .and_then(|v| v.first())
                                .map(get_varint_val)
                                .unwrap_or(0);
                            let cached = block_dict
                                .get(&5)
                                .and_then(|v| v.first())
                                .map(get_varint_val)
                                .unwrap_or(0);
                            let thinking = block_dict
                                .get(&10)
                                .and_then(|v| v.first())
                                .map(get_varint_val)
                                .unwrap_or(0);

                            metrics.push(Metric {
                                model: model_name.clone(),
                                uncached_input: uncached,
                                cached_input: cached,
                                output: candidates,
                                thinking,
                            });
                        }
                    }
                }
            }
        }
    }
    metrics
}

// 3. 会话元数据与日志读取逻辑

fn extract_convo_info(uuid: &str, db_path: &Path) -> (String, String) {
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

// 4. 增量本地缓存数据库结构初始化

fn init_cache_db() -> Result<(), rusqlite::Error> {
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

// 5. 增量扫描逻辑与数据同步

fn sync_and_collect_metrics() -> Result<AggregatedMetrics, rusqlite::Error> {
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

// 6. 从缓存数据库获取大盘聚合统计数据

#[derive(Serialize)]
struct Totals {
    total_input: i64,
    total_output: i64,
    total_tokens: i64,
    total_cached: i64,
    total_thinking: i64,
    cache_hit_rate: f64,
    thinking_ratio: f64,
    total_sessions: i64,
}

#[derive(Serialize)]
struct DailyTrend {
    date: String,
    input: i64,
    output: i64,
    cached: i64,
    thinking: i64,
    sessions: i64,
}

#[derive(Serialize)]
struct MonthlySummary {
    month: String,
    input: i64,
    output: i64,
    cached: i64,
    thinking: i64,
    sessions: i64,
}

#[derive(Serialize)]
struct ModelDistribution {
    model: String,
    input: i64,
    output: i64,
    cached: i64,
    thinking: i64,
    total_tokens: i64,
}

#[derive(Serialize)]
struct SessionItem {
    uuid: String,
    title: String,
    created_at: String,
    input: i64,
    output: i64,
    cached: i64,
    thinking: i64,
    models: Vec<String>,
}

#[derive(Serialize)]
struct AggregatedMetrics {
    totals: Totals,
    daily_trends: Vec<DailyTrend>,
    monthly_summary: Vec<MonthlySummary>,
    model_distribution: Vec<ModelDistribution>,
    sessions: Vec<SessionItem>,
}

fn get_aggregated_metrics_from_cache() -> Result<AggregatedMetrics, rusqlite::Error> {
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

// 7. 后端路由与服务搭建

async fn handle_metrics() -> Response<Body> {
    match tokio::task::spawn_blocking(|| {
        let _lock = DB_LOCK.lock().unwrap();
        let _ = init_cache_db();
        sync_and_collect_metrics()
    })
    .await
    {
        Ok(Ok(data)) => {
            let body = match serde_json::to_vec(&data) {
                Ok(bytes) => Body::from(bytes),
                Err(e) => return Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
                    .body(Body::from(format!("JSON Serialization Error: {}", e)))
                    .unwrap(),
            };
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CACHE_CONTROL, "no-cache, no-store, must-revalidate")
                .header(header::PRAGMA, "no-cache")
                .header(header::EXPIRES, "0")
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .body(body)
                .unwrap()
        }
        Ok(Err(e)) => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(Body::from(format!("Database Error: {}", e)))
            .unwrap(),
        Err(e) => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(Body::from(format!("Server Thread Error: {}", e)))
            .unwrap(),
    }
}

async fn serve_static_file_fallback(uri: Uri) -> impl IntoResponse {
    let path_str = uri.path();
    let clean_path = percent_encoding::percent_decode_str(path_str)
        .decode_utf8_lossy()
        .into_owned();
    let clean_path = clean_path.trim_start_matches('/');

    let file_name = if clean_path.is_empty() {
        "index.html"
    } else {
        clean_path
    };

    // 优先返回内置的前端静态资源
    let (embedded_content, content_type) = match file_name {
        "index.html" => (Some(include_str!("../index.html")), "text/html; charset=utf-8"),
        "style.css" => (Some(include_str!("../style.css")), "text/css; charset=utf-8"),
        "app.js" => (Some(include_str!("../app.js")), "application/javascript; charset=utf-8"),
        _ => (None, ""),
    };

    if let Some(content) = embedded_content {
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, content_type)
            .body(Body::from(content))
            .unwrap();
    }

    // 回退逻辑：如果内置文件未匹配到，则尝试从磁盘读取（为了支持本地其他图片/文件等静态资源）
    let mut file_path = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|parent| parent.join(file_name)))
        .unwrap_or_else(|| PathBuf::from(file_name));

    if !file_path.exists() {
        file_path = PathBuf::from(file_name);
    }

    if !file_path.exists() || !file_path.is_file() {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(Body::from("File Not Found"))
            .unwrap();
    }

    let content_type = if file_name.ends_with(".html") {
        "text/html; charset=utf-8"
    } else if file_name.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if file_name.ends_with(".js") {
        "application/javascript; charset=utf-8"
    } else if file_name.ends_with(".png") {
        "image/png"
    } else if file_name.ends_with(".jpg") || file_name.ends_with(".jpeg") {
        "image/jpeg"
    } else {
        "text/plain; charset=utf-8"
    };

    match std::fs::read(&file_path) {
        Ok(content) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, content_type)
            .body(Body::from(content))
            .unwrap(),
        Err(_) => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(Body::from("Error reading static file"))
            .unwrap(),
    }
}

#[tokio::main]
async fn main() {
    let mut port = 19362;
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        if let Ok(p) = args[1].parse::<u16>() {
            port = p;
        }
    }

    let app = Router::new()
        .route("/api/metrics", get(handle_metrics))
        .fallback(serve_static_file_fallback);

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    println!("\n==================================================");
    println!(" Antigravity 极速增量缓存用量统计服务已成功启动！");
    println!(" 服务地址: http://localhost:{}", port);
    println!(" 正在自动为您打开浏览器，如果没有打开，请手动访问上述地址。");
    println!(" 请保持此命令行窗口开启。");
    println!(" 按 Ctrl+C 可以退出本服务。");
    println!("==================================================\n");

    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Error binding to port {}: {}", port, e);
            return;
        }
    };

    // 自动打开浏览器
    let url = format!("http://localhost:{}", port);
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd")
        .args(["/C", "start", &url])
        .spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open")
        .arg(&url)
        .spawn();
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open")
        .arg(&url)
        .spawn();

    if let Err(e) = axum::serve(listener, app).await {
        eprintln!("Server error: {}", e);
    }
}
