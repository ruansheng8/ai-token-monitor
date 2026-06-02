/// review.rs — 「使用复盘与建议」功能系统
///
/// 升级为支持后台任务、进度管理和历史记录的系统：
///   - 本地任务管理器 ReviewTaskManager 支持任务控制与取消
///   - 状态与事件实时落库（SQLite）
///   - 支持 SSE 重放历史与实时推送（完美解决断线恢复问题）

use axum::{
    body::Body,
    http::{header, Response, StatusCode},
    response::{
        sse::{Event, Sse},
        IntoResponse,
    },
};
use serde::{Deserialize, Serialize};
use std::{
    convert::Infallible,
    path::PathBuf,
    time::Duration,
    sync::{Arc, Mutex, OnceLock},
    collections::HashMap,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
};
use tokio_stream::wrappers::ReceiverStream;
use tokio::sync::broadcast;
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

// ============================================================
// 数据结构与序列化
// ============================================================

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CliToolInfo {
    pub name: String,
    pub available: bool,
    pub version: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DetectResponse {
    pub tools: Vec<CliToolInfo>,
    pub recommended: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub totalTokens: i64,
    pub totalCostUsd: f64,
    pub totalSessions: i64,
    pub cacheHitRate: f64,
    pub thinkingRatio: f64,
    pub sourceBreakdown: Option<String>,
    pub modelDistribution: Option<String>,
    pub dailyTrendSummary: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateTaskRequest {
    pub cli: String,
    pub time_range: String,
    pub selected_ides: Vec<String>,
    pub custom_prompt: Option<String>,
    pub force: Option<bool>,
    pub metrics_snapshot: MetricsSnapshot,
    pub compare_metrics_snapshot: Option<MetricsSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewTask {
    pub id: String,
    pub title: String,
    pub status: String, // pending, running, succeeded, failed, canceled, interrupted
    pub cli_name: String,
    pub cli_path: Option<String>,
    pub time_range: String,
    pub selected_ides_json: String,
    pub prompt_text: String,
    pub prompt_hash: String,
    pub metrics_snapshot_json: String,
    pub metrics_hash: String,
    pub dedupe_key: String,
    pub progress_stage: String,
    pub progress_percent: i32,
    pub status_message: String,
    pub output_markdown: String,
    pub error_message: Option<String>,
    pub exit_code: Option<i32>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub canceled_at: Option<String>,
    pub last_heartbeat_at: Option<String>,
    pub error_type: Option<String>,
    pub quality_feedback: Option<String>,
    pub action_items_json: Option<String>,
    pub compare_metrics_snapshot_json: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskEvent {
    pub id: Option<i64>,
    pub task_id: String,
    pub sequence: i64,
    pub kind: String, // stage, progress, stdout, stderr, heartbeat, error, done
    pub message: String,
    pub payload_json: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TurnDetails {
    pub source: String,
    pub uuid: String,
    pub idx: i64,
    pub user_prompt: Option<String>,
    pub executed_commands: Option<String>,
    pub failed_commands: Option<String>,
    pub modified_files: Option<String>,
}


// 从前端传入的指标摘要（用于旧兼容性，保留定义）
#[derive(Debug, Deserialize)]
pub struct ReviewRequest {
    pub time_range: Option<String>,
    pub total_tokens: Option<i64>,
    pub total_cost_usd: Option<f64>,
    pub total_sessions: Option<i64>,
    pub cache_hit_rate: Option<f64>,
    pub thinking_ratio: Option<f64>,
    pub source_breakdown: Option<String>,
    pub model_distribution: Option<String>,
    pub daily_trend_summary: Option<String>,
    pub preferred_cli: Option<String>,
    pub custom_prompt: Option<String>,
    pub selected_ides: Option<String>,
}

// ============================================================
// 全局任务管理器 ReviewTaskManager
// ============================================================

pub struct ActiveTask {
    pub task_id: String,
    pub tx: broadcast::Sender<TaskEvent>,
    pub child: Arc<tokio::sync::Mutex<Option<tokio::process::Child>>>,
}

pub struct ReviewTaskManager {
    pub active_tasks: HashMap<String, Arc<ActiveTask>>,
}

static TASK_MANAGER: OnceLock<Mutex<ReviewTaskManager>> = OnceLock::new();

pub fn get_task_manager() -> &'static Mutex<ReviewTaskManager> {
    TASK_MANAGER.get_or_init(|| {
        Mutex::new(ReviewTaskManager {
            active_tasks: HashMap::new(),
        })
    })
}

// ============================================================
// 辅助工具方法
// ============================================================

fn calculate_hash<T: Hash>(t: &T) -> String {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    format!("{:x}", s.finish())
}

/// 辅助方法：遍历 Node 版本管理器中的 bin 文件夹
fn find_node_version_bin_dirs(root: &std::path::Path, child_segments: &[&str]) -> Vec<PathBuf> {
    let mut result = Vec::new();
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_dir() || file_type.is_symlink() {
                    let mut bin_path = entry.path();
                    for seg in child_segments {
                        bin_path = bin_path.join(seg);
                    }
                    if bin_path.exists() {
                        result.push(bin_path);
                    }
                }
            }
        }
    }
    result
}

/// 扫描系统中常见但可能不在 GUI 进程默认 PATH 中的工具链目录
fn get_well_known_toolchain_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    
    // 获取 Home 目录
    let home = std::env::var("USERPROFILE")
        .ok()
        .or_else(|| std::env::var("HOME").ok())
        .map(PathBuf::from);

    if let Some(home_path) = home {
        dirs.push(home_path.join(".cargo").join("bin"));
        dirs.push(home_path.join(".bun").join("bin"));
        dirs.push(home_path.join(".volta").join("bin"));
        dirs.push(home_path.join(".npm-global").join("bin"));
        dirs.push(home_path.join(".npm-packages").join("bin"));
        dirs.push(home_path.join(".local").join("bin"));
        
        #[cfg(target_os = "windows")]
        {
            if let Ok(app_data) = std::env::var("LOCALAPPDATA") {
                let local_app_data = PathBuf::from(app_data);
                let fnm_versions = local_app_data.join("fnm").join("node-versions");
                dirs.extend(find_node_version_bin_dirs(&fnm_versions, &["installation", "bin"]));
            }
            if let Ok(app_data) = std::env::var("APPDATA") {
                let roaming_app_data = PathBuf::from(app_data);
                dirs.push(roaming_app_data.join("npm"));
            }
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            let nvm_versions = home_path.join(".nvm").join("versions").join("node");
            dirs.extend(find_node_version_bin_dirs(&nvm_versions, &["bin"]));
            
            let fnm_versions_1 = home_path.join(".local").join("share").join("fnm").join("node-versions");
            dirs.extend(find_node_version_bin_dirs(&fnm_versions_1, &["installation", "bin"]));
            
            let fnm_versions_2 = home_path.join(".fnm").join("node-versions");
            dirs.extend(find_node_version_bin_dirs(&fnm_versions_2, &["installation", "bin"]));
        }
    }
    
    if let Ok(prefix) = std::env::var("NPM_CONFIG_PREFIX").or_else(|_| std::env::var("npm_config_prefix")) {
        let prefix_path = PathBuf::from(prefix.trim());
        if !prefix_path.to_string_lossy().is_empty() {
            dirs.push(prefix_path.join("bin"));
            #[cfg(target_os = "windows")]
            dirs.push(prefix_path.clone());
        }
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        dirs.push(PathBuf::from("/opt/homebrew/bin"));
        dirs.push(PathBuf::from("/usr/local/bin"));
    }

    dirs.into_iter()
        .filter(|p| p.exists() && p.is_dir())
        .collect()
}

/// 在 PATH 中查找可执行文件，Windows 下额外尝试 .cmd 和 .exe 后缀
/// 优先检查 AppConfig 的 agent_cli_env 中是否有 [BIN_UPPER]_BIN 的自定义路径覆盖
fn find_cli_in_path(bin: &str) -> Option<PathBuf> {
    // 0. 优先检查配置文件中是否有自定义 BIN 路径覆盖（如 CODEX_BIN, CLAUDE_BIN）
    let config = crate::config::load_config();
    let bin_key = format!("{}_BIN", bin.replace('-', "_").to_uppercase());
    if let Some(agent_env) = config.agent_cli_env.get(bin) {
        if let Some(custom_bin) = agent_env.get(&bin_key) {
            let custom_path = PathBuf::from(custom_bin.trim());
            if custom_path.exists() && custom_path.is_file() {
                return Some(custom_path);
            }
        }
    }

    // 1. 尝试直接通过系统的 PATH 寻找
    if let Ok(path) = which::which(bin) {
        return Some(path);
    }

    #[cfg(target_os = "windows")]
    {
        for suffix in &[".cmd", ".exe", ".bat"] {
            let name = format!("{}{}", bin, suffix);
            if let Ok(path) = which::which(&name) {
                return Some(path);
            }
        }
    }

    // 2. 遍历 well-known 目录进行补充搜索
    let well_known_dirs = get_well_known_toolchain_dirs();
    
    #[cfg(target_os = "windows")]
    let suffixes = &[".exe", ".cmd", ".bat", ""];
    #[cfg(not(target_os = "windows"))]
    let suffixes = &[""];

    for dir in well_known_dirs {
        for suffix in suffixes {
            let file_name = if suffix.is_empty() {
                bin.to_string()
            } else {
                format!("{}{}", bin, suffix)
            };
            let full_path = dir.join(&file_name);
            if full_path.exists() && full_path.is_file() {
                return Some(full_path);
            }
        }
    }

    None
}

/// 探测单个 CLI 工具：是否存在 + 版本号
async fn probe_cli(bin: &str) -> CliToolInfo {
    let path = find_cli_in_path(bin);

    if path.is_none() {
        return CliToolInfo {
            name: bin.to_string(),
            available: false,
            version: None,
            path: None,
        };
    }

    let exe_path = path.unwrap();
    let path_str = exe_path.to_string_lossy().to_string();

    let mut cmd = Command::new(&exe_path);
    cmd.arg("--version");
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

    let run_res = tokio::time::timeout(
        Duration::from_secs(5),
        cmd.output(),
    )
    .await;

    match run_res {
        // 超时，视作可调用，但由于未能提供版本因此设为 None
        Err(_) => CliToolInfo {
            name: bin.to_string(),
            available: true,
            version: None,
            path: Some(path_str),
        },
        Ok(Err(io_err)) => {
            // IO 级别错误：若文件无法启动（NotFound、PermissionDenied 等），标记为不可调用
            let is_not_invocable = matches!(
                io_err.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied
            );
            CliToolInfo {
                name: bin.to_string(),
                available: !is_not_invocable,
                version: None,
                path: Some(path_str),
            }
        }
        Ok(Ok(output)) => {
            let exit_code = output.status.code();
            // 退出状态码 126/127/9009 代表解释器/底层文件损坏丢失，不可调用
            let is_not_invocable = matches!(exit_code, Some(126) | Some(127) | Some(9009));

            if is_not_invocable {
                CliToolInfo {
                    name: bin.to_string(),
                    available: false,
                    version: None,
                    path: Some(path_str),
                }
            } else {
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                let combined = if !stdout.is_empty() { stdout } else { stderr };
                
                // 特殊：Windows CMD/NPM shim 目标损坏时的中英文错误标志，防止 Windows 环境 GBK 乱码导致匹配失败
                let is_broken_shim = combined.contains("系统找不到指定的路径") 
                    || combined.contains("找不到文件")
                    || combined.contains("The system cannot find the path specified")
                    || combined.contains("is not recognized as an internal or external command")
                    || combined.contains("不是内部或外部命令");
                
                let version = if combined.is_empty() || is_broken_shim {
                    None
                } else {
                    Some(combined.lines().next().unwrap_or("").to_string())
                };

                CliToolInfo {
                    name: bin.to_string(),
                    available: !is_broken_shim,
                    version,
                    path: Some(path_str),
                }
            }
        }
    }
}

async fn record_and_broadcast_event(
    task_id: &str,
    kind: &str,
    message: &str,
    payload_json: Option<&str>,
    tx: &broadcast::Sender<TaskEvent>,
) -> Result<i64, String> {
    let now = chrono::Utc::now().to_rfc3339();
    let task_id_str = task_id.to_string();
    let kind_str = kind.to_string();
    let message_str = message.to_string();
    let payload_str = payload_json.map(|s| s.to_string());

    let res = tokio::task::spawn_blocking(move || {
        let conn = rusqlite::Connection::open(crate::db::get_db_cache_path())
            .map_err(|e| format!("打开数据库失败: {}", e))?;
        
        let next_seq: i64 = conn.query_row(
            "SELECT COALESCE(MAX(sequence), 0) + 1 FROM review_task_events WHERE task_id = ?",
            [&task_id_str],
            |row| row.get(0),
        ).unwrap_or(1);

        conn.execute(
            "INSERT INTO review_task_events (task_id, sequence, kind, message, payload_json, created_at)
             VALUES (?, ?, ?, ?, ?, ?)",
            rusqlite::params![task_id_str, next_seq, kind_str, message_str, payload_str, &now],
        ).map_err(|e| format!("插入事件失败: {}", e))?;

        let id = conn.last_insert_rowid();
        Ok::<TaskEvent, String>(TaskEvent {
            id: Some(id),
            task_id: task_id_str,
            sequence: next_seq,
            kind: kind_str,
            message: message_str,
            payload_json: payload_str,
            created_at: now,
        })
    }).await;

    match res {
        Ok(Ok(ev)) => {
            let seq = ev.sequence;
            let _ = tx.send(ev);
            Ok(seq)
        }
        Ok(Err(e)) => Err(e),
        Err(e) => Err(format!("线程池执行异常: {}", e)),
    }
}

// ============================================================
// API 路由处理器
// ============================================================

/// GET /api/review/detect
/// 检测宿主机已安装的 AI CLI 工具，返回可用列表
pub async fn handle_review_detect(
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> Response<Body> {
    let force = params.get("force").map(|s| s == "true").unwrap_or(false);

    let cache_path = std::path::Path::new(&crate::db::get_user_profile_dir())
        .join(".token-insight")
        .join("cli_detect_cache.json");

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct CliDetectCache {
        pub detected_at: String,
        pub tools: Vec<CliToolInfo>,
        pub recommended: Option<String>,
    }

    // 1. 尝试从缓存中读取 (非 force 模式)
    if !force {
        if let Ok(content) = std::fs::read_to_string(&cache_path) {
            if let Ok(cache) = serde_json::from_str::<CliDetectCache>(&content) {
                if let Ok(detected_dt) = chrono::DateTime::parse_from_rfc3339(&cache.detected_at) {
                    let now = chrono::Utc::now();
                    let duration = now.signed_duration_since(detected_dt.with_timezone(&chrono::Utc));
                    if duration.num_hours() < 24 {
                        let resp = DetectResponse {
                            tools: cache.tools,
                            recommended: cache.recommended,
                        };
                        if let Ok(body_bytes) = serde_json::to_vec(&resp) {
                            return Response::builder()
                                .status(StatusCode::OK)
                                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                                .body(Body::from(body_bytes))
                                .unwrap();
                        }
                    }
                }
            }
        }
    }

    // 2. 缓存失效或 force 模式：执行真实检测
    let candidate_bins = [
        // 核心 tier-1 支持
        "claude", "codex", "gemini", "agy",
        // open-design 完整 CLI 列表
        "cursor-agent", "opencode", "qwen", "copilot",
        "devin", "kimi", "qoder", "pi", "kiro", "kilo",
        "vibe", "deepseek", "hermes", "grok-build", "reasonix", "aider",
    ];

    let mut tools = Vec::new();
    for bin in &candidate_bins {
        tools.push(probe_cli(bin).await);
    }

    let recommended = tools
        .iter()
        .find(|t| t.available)
        .map(|t| t.name.clone());

    let resp = DetectResponse {
        tools: tools.clone(),
        recommended: recommended.clone(),
    };

    // 3. 写入最新探测数据到缓存文件
    let cache_data = CliDetectCache {
        detected_at: chrono::Utc::now().to_rfc3339(),
        tools,
        recommended,
    };
    if let Ok(cache_json) = serde_json::to_string(&cache_data) {
        if let Some(parent) = cache_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&cache_path, cache_json);
    }

    let body_bytes = serde_json::to_vec(&resp).unwrap_or_default();
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(Body::from(body_bytes))
        .unwrap()
}

/// POST /api/review/tasks
/// 创建并拉起后台复盘分析任务
fn build_compare_metrics_section(current: &MetricsSnapshot, previous: &MetricsSnapshot) -> String {
    let pct_diff = |curr: f64, prev: f64| -> String {
        if prev > 0.0 {
            let diff = ((curr - prev) / prev) * 100.0;
            if diff > 0.0 {
                format!("+{:.1}% 📈", diff)
            } else if diff < 0.0 {
                format!("{:.1}% 📉", diff)
            } else {
                "无变化".to_string()
            }
        } else {
            "新增数据".to_string()
        }
    };

    let abs_diff = |curr: f64, prev: f64| -> String {
        let diff = (curr - prev) * 100.0;
        if diff > 0.0 {
            format!("+{:.1}% 📈", diff)
        } else if diff < 0.0 {
            format!("{:.1}% 📉", diff)
        } else {
            "无变化".to_string()
        }
    };

    let curr_tokens_fmt = if current.totalTokens >= 1_000_000 {
        format!("{:.1}M", current.totalTokens as f64 / 1_000_000.0)
    } else if current.totalTokens >= 1_000 {
        format!("{:.1}K", current.totalTokens as f64 / 1_000.0)
    } else {
        current.totalTokens.to_string()
    };

    let prev_tokens_fmt = if previous.totalTokens >= 1_000_000 {
        format!("{:.1}M", previous.totalTokens as f64 / 1_000_000.0)
    } else if previous.totalTokens >= 1_000 {
        format!("{:.1}K", previous.totalTokens as f64 / 1_000.0)
    } else {
        previous.totalTokens.to_string()
    };

    format!(
        "我追踪了我的 AI 工具使用情况。以下是我的周度环比效能对比数据（本周 vs 上周）：\n\n\
        | 指标维度 | 本周数值 | 上周数值 | 环比变化趋势 |\n\
        |----------|----------|----------|--------------|\n\
        | 总 Token 消耗 | {} tokens | {} tokens | {} |\n\
        | 总预估费用 | ${:.4} USD | ${:.4} USD | {} |\n\
        | 会话交互总数 | {} 次 | {} 次 | {} |\n\
        | 缓存命中率 | {:.1}% | {:.1}% | {} |\n\
        | 推理 Token 占比 | {:.1}% | {:.1}% | {} |\n\n\
        请帮我着重从以下维度进行深度诊断：\n\
        1. **本周与上周的成本和用量变动原因**：分析为什么成本或 Token 变多了或变少了，是否与特定的开发习惯或模型偏好变化有关。\n\
        2. **缓存效率及提问质量的环比变动**：分析缓存命中率是否有提升，推理（Thinking）模型的使用占比变化，以及这些变动反映了怎样的人机协作模式变迁。\n\
        3. **提供精细的环比优化行动项**：在报告第 4 部分中，提供 3-5 条针对这周用量瓶颈的、具体的下周落地行动项。",
        curr_tokens_fmt, prev_tokens_fmt, pct_diff(current.totalTokens as f64, previous.totalTokens as f64),
        current.totalCostUsd, previous.totalCostUsd, pct_diff(current.totalCostUsd, previous.totalCostUsd),
        current.totalSessions, previous.totalSessions, pct_diff(current.totalSessions as f64, previous.totalSessions as f64),
        current.cacheHitRate * 100.0, previous.cacheHitRate * 100.0, abs_diff(current.cacheHitRate, previous.cacheHitRate),
        current.thinkingRatio * 100.0, previous.thinkingRatio * 100.0, abs_diff(current.thinkingRatio, previous.thinkingRatio)
    )
}

/// POST /api/review/tasks
/// 创建并拉起后台复盘分析任务
pub async fn handle_create_task(
    axum::Json(req): axum::Json<CreateTaskRequest>,
) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let conn = rusqlite::Connection::open(crate::db::get_db_cache_path())
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("数据库打开错误: {}", e)))?;

        // 1. 检查是否存在正在运行的任务
        let running_task: Option<(String, String)> = conn.query_row(
            "SELECT id, title FROM review_tasks WHERE status IN ('pending', 'running') LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).ok();

        if let Some((r_id, r_title)) = running_task {
            return Err((
                StatusCode::CONFLICT,
                serde_json::json!({
                    "code": "REVIEW_TASK_RUNNING",
                    "message": format!("已有复盘任务正在运行: {}", r_title),
                    "task_id": r_id
                }).to_string()
            ));
        }

        // 2. 生成各种哈希和 dedupe_key
        let req_mock = ReviewRequest {
            time_range: Some(req.time_range.clone()),
            total_tokens: Some(req.metrics_snapshot.totalTokens),
            total_cost_usd: Some(req.metrics_snapshot.totalCostUsd),
            total_sessions: Some(req.metrics_snapshot.totalSessions),
            cache_hit_rate: Some(req.metrics_snapshot.cacheHitRate),
            thinking_ratio: Some(req.metrics_snapshot.thinkingRatio),
            source_breakdown: req.metrics_snapshot.sourceBreakdown.clone(),
            model_distribution: req.metrics_snapshot.modelDistribution.clone(),
            daily_trend_summary: req.metrics_snapshot.dailyTrendSummary.clone(),
            preferred_cli: Some(req.cli.clone()),
            custom_prompt: None,
            selected_ides: Some(req.selected_ides.join(",")),
        };

        let prompt_text = match req.custom_prompt.clone() {
            None => {
                if let Some(ref prev) = req.compare_metrics_snapshot {
                    build_compare_metrics_section(&req.metrics_snapshot, prev)
                } else {
                    build_review_prompt(&req_mock)
                }
            }
            Some(custom) => {
                let custom_trimmed = custom.trim();
                if let Some(ref prev) = req.compare_metrics_snapshot {
                    let compare_table = build_compare_metrics_section(&req.metrics_snapshot, prev);
                    let merged = format!("{}\n\n---\n\n## 我的自定义分析要求与指令：\n{}", compare_table, custom_trimmed);
                    replace_placeholders(&merged, &req_mock)
                } else {
                    // 检查是否包含核心度量标识
                    if !custom_trimmed.contains("{{TOTAL_TOKENS}}") && !custom_trimmed.contains("总 Token 消耗") {
                        // 无度量数据，自动合成为包含基本度量卡片与用户定制指令的组合 Prompt
                        let time_label = req_mock.time_range.as_deref().unwrap_or("最近30天");
                        let total_tokens = req_mock.total_tokens.unwrap_or(0);
                        let total_cost = req_mock.total_cost_usd.unwrap_or(0.0);
                        let total_sessions = req_mock.total_sessions.unwrap_or(0);
                        let cache_rate = req_mock.cache_hit_rate.unwrap_or(0.0);
                        let thinking_ratio = req_mock.thinking_ratio.unwrap_or(0.0);

                        let total_tokens_fmt = if total_tokens >= 1_000_000 {
                            format!("{:.1}M", total_tokens as f64 / 1_000_000.0)
                        } else if total_tokens >= 1_000 {
                            format!("{:.1}K", total_tokens as f64 / 1_000.0)
                        } else {
                            total_tokens.to_string()
                        };

                        let source_section = req_mock
                            .source_breakdown
                            .as_deref()
                            .map(|s| format!("\n**各工具来源分布：**\n```json\n{}\n```", s))
                            .unwrap_or_default();

                        let model_section = req_mock
                            .model_distribution
                            .as_deref()
                            .map(|s| format!("\n**模型使用分布：**\n```json\n{}\n```", s))
                            .unwrap_or_default();

                        let trend_section = req_mock
                            .daily_trend_summary
                            .as_deref()
                            .map(|s| format!("\n**日均消耗趋势（近期）：**\n```json\n{}\n```", s))
                            .unwrap_or_default();

                        let base_metrics = format!(
                            "我追踪了我的 AI 工具使用情况。以下是我的核心使用数据（{}）：\n\n\
                            | 指标 | 数值 |\n\
                            |------|------|\n\
                            | 总 Token 消耗 | {} tokens |\n\
                            | 总费用 | ${:.4} USD |\n\
                            | 总会话数 | {} 次 |\n\
                            | 缓存命中率 | {:.1}% |\n\
                            | 推理（Thinking）Token 占比 | {:.1}% |\n\
                            {}{}{}",
                            time_label,
                            total_tokens_fmt,
                            total_cost,
                            total_sessions,
                            cache_rate * 100.0,
                            thinking_ratio * 100.0,
                            source_section,
                            model_section,
                            trend_section
                        );

                        let merged_prompt = format!("{}\n\n---\n\n## 我的自定义分析要求与指令：\n{}", base_metrics, custom_trimmed);
                        replace_placeholders(&merged_prompt, &req_mock)
                    } else {
                        replace_placeholders(custom_trimmed, &req_mock)
                    }
                }
            }
        };

        let mut prompt_text = prompt_text;
        // 抓取并追加最近的调试与终端异常记录
        let recent_errors: Vec<(String, i64, String, String)> = {
            let mut stmt = conn.prepare(
                "SELECT uuid, idx, user_prompt, failed_commands FROM turn_details 
                 ORDER BY idx DESC LIMIT 15"
            ).unwrap();
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0).unwrap_or_default(),
                    row.get::<_, i64>(1).unwrap_or_default(),
                    row.get::<_, Option<String>>(2).unwrap_or_default().unwrap_or_default(),
                    row.get::<_, Option<String>>(3).unwrap_or_default().unwrap_or_default()
                ))
            }).unwrap();
            let mut errs = Vec::new();
            for r in rows {
                if let Ok(item) = r {
                    if !item.3.is_empty() && item.3 != "[]" {
                        errs.push(item);
                    }
                }
            }
            errs
        };

        if !recent_errors.is_empty() {
            prompt_text.push_str("\n\n---\n\n## 诊断调试轨迹与终端异常事件（请结合具体报错进行精准诊断。如果报告中分析到了对应的事件，必须在结论后加上超链接 `[点击查看详情及错误日志](turn-details://claude_code/会话ID/轮次ID)`，其中会话ID和轮次ID必须与提供的数据严格匹配）：\n");
            for (idx, (uuid, turn_idx, prompt, fails_json)) in recent_errors.iter().enumerate() {
                if let Ok(fails) = serde_json::from_str::<serde_json::Value>(&fails_json) {
                    if let Some(arr) = fails.as_array() {
                        for f in arr {
                            let cmd = f.get("command").and_then(|c| c.as_str()).unwrap_or("");
                            let stderr = f.get("stderr").and_then(|s| s.as_str()).unwrap_or("");
                            let err_line = stderr.lines().next().unwrap_or("");
                            let prompt_summary = if prompt.len() > 35 {
                                format!("{}...", &prompt[0..35])
                            } else {
                                prompt.clone()
                            };
                            prompt_text.push_str(&format!(
                                "{}. **会话ID**: `{}` | **轮次ID**: `{}` | **提问**: \"{}\" | **执行失败指令**: `{}` | **报错**: `{}`\n",
                                idx + 1, uuid, turn_idx, prompt_summary.replace('\n', " "), cmd, err_line
                            ));
                        }
                    }
                }
            }
        }

        let prompt_hash = calculate_hash(&prompt_text);
        let metrics_snapshot_json = serde_json::to_string(&req.metrics_snapshot).unwrap_or_default();
        let metrics_hash = calculate_hash(&metrics_snapshot_json);
        let selected_ides_json = serde_json::to_string(&req.selected_ides).unwrap_or_default();

        let dedupe_str = format!("{}_{}_{}_{}", req.time_range, selected_ides_json, prompt_hash, metrics_hash);
        let dedupe_key = calculate_hash(&dedupe_str);

        // 3. 去重模糊校验
        if req.force != Some(true) {
            let six_hours_ago = (chrono::Utc::now() - chrono::Duration::hours(6)).to_rfc3339();
            
            // 查询 6 小时内，相同参数 (time_range, CLI, IDEs) 的成功任务
            let recent_tasks: Vec<(String, String, String)> = {
                let mut stmt = conn.prepare(
                    "SELECT id, metrics_snapshot_json, created_at FROM review_tasks \
                     WHERE time_range = ? AND cli_name = ? AND selected_ides_json = ? \
                     AND status = 'succeeded' \
                     AND created_at >= ? \
                     ORDER BY created_at DESC"
                ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("查询历史任务失败: {}", e)))?;
                
                let rows = stmt.query_map(rusqlite::params![req.time_range, req.cli, selected_ides_json, six_hours_ago], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
                }).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("读取历史任务失败: {}", e)))?;
                
                let mut list = Vec::new();
                for r in rows {
                    if let Ok(item) = r {
                        list.push(item);
                    }
                }
                list
            };

            for (dup_id, old_snapshot_json, created_at) in recent_tasks {
                if let Ok(old_snapshot) = serde_json::from_str::<MetricsSnapshot>(&old_snapshot_json) {
                    let old_tokens = old_snapshot.totalTokens;
                    let old_cost = old_snapshot.totalCostUsd;
                    
                    let new_tokens = req.metrics_snapshot.totalTokens;
                    let new_cost = req.metrics_snapshot.totalCostUsd;
                    
                    // 模糊比较：如果 Tokens 和 费用相差在 2% 以内，我们认为是重复生成！
                    let tokens_diff_pct = if old_tokens > 0 {
                        ((new_tokens - old_tokens).abs() as f64) / (old_tokens as f64)
                    } else {
                        0.0
                    };
                    let cost_diff_pct = if old_cost > 0.0 {
                        (new_cost - old_cost).abs() / old_cost
                    } else {
                        0.0
                    };
                    
                    if tokens_diff_pct <= 0.02 && cost_diff_pct <= 0.02 {
                        // 判定为重复任务
                        let dt = chrono::DateTime::parse_from_rfc3339(&created_at).ok();
                        let local_time_str = dt.map(|d| d.with_timezone(&chrono::Local).format("%H:%M:%S").to_string())
                            .unwrap_or_else(|| "近期".to_string());
                            
                        return Ok((
                            StatusCode::OK,
                            serde_json::json!({
                                "duplicate_of": dup_id,
                                "message": format!("您在今天 {} 刚刚成功生成过一份内容几乎完全一致的报告（指标偏差 < 2%），是否直接打开查看？", local_time_str)
                            }).to_string()
                        ));
                    }
                }
            }
        }

        // 4. 创建新任务
        let task_id = format!("task_{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0));
        let ides_display = if req.selected_ides.contains(&"all".to_string()) || req.selected_ides.is_empty() {
            "全部 IDE"
        } else {
            "部分 IDE"
        };
        let title = format!("{} · {} · {}", req.time_range, ides_display, get_cli_display_name(&req.cli));
        let created_at = chrono::Utc::now().to_rfc3339();

        let cli_path = find_cli_in_path(&req.cli).map(|p| p.to_string_lossy().to_string());
        let compare_metrics_snapshot_json = req.compare_metrics_snapshot.as_ref().map(|s| serde_json::to_string(s).unwrap_or_default());

        conn.execute(
            "INSERT INTO review_tasks (
                id, title, status, cli_name, cli_path, time_range, selected_ides_json,
                prompt_text, prompt_hash, metrics_snapshot_json, metrics_hash, dedupe_key,
                progress_stage, progress_percent, status_message, created_at, compare_metrics_snapshot_json
            ) VALUES (?, ?, 'pending', ?, ?, ?, ?, ?, ?, ?, ?, ?, 'created', 5, '任务已创建', ?, ?)",
            rusqlite::params![
                task_id, title, req.cli, cli_path, req.time_range, selected_ides_json,
                prompt_text, prompt_hash, metrics_snapshot_json, metrics_hash, dedupe_key,
                created_at, compare_metrics_snapshot_json
            ],
        ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("插入任务失败: {}", e)))?;

        // 读取完整的 task 结构回显给前端
        let task = query_task_by_id(&conn, &task_id)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("查询任务详情失败: {}", e)))?;

        Ok::<_, (StatusCode, String)>((StatusCode::CREATED, serde_json::to_string(&task).unwrap()))
    }).await;

    match result {
        Ok(Ok((status, body))) => {
            // 如果任务成功创建，则需要拉起后台子协程
            if status == StatusCode::CREATED {
                if let Ok(task) = serde_json::from_str::<ReviewTask>(&body) {
                    let task_id = task.id.clone();
                    let cli_name = task.cli_name.clone();
                    let prompt = task.prompt_text.clone();

                    let (tx, _) = broadcast::channel::<TaskEvent>(100);
                    let child = Arc::new(tokio::sync::Mutex::new(None));

                    let active_task = Arc::new(ActiveTask {
                        task_id: task_id.clone(),
                        tx: tx.clone(),
                        child: child.clone(),
                    });

                    // 注册到全局活跃任务
                    if let Ok(mut mgr) = get_task_manager().lock() {
                        mgr.active_tasks.insert(task_id.clone(), active_task);
                    }

                    // 启动后台分析协程
                    tokio::spawn(async move {
                        if let Err(e) = run_cli_task_background(&task_id, &cli_name, prompt, child, tx.clone()).await {
                            eprintln!("[后台复盘任务] 任务 {} 异常结束: {}", task_id, e);
                        }
                    });
                }
            }

            Response::builder()
                .status(status)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from(body))
                .unwrap()
        }
        Ok(Err((status, body))) => {
            Response::builder()
                .status(status)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from(body))
                .unwrap()
        }
        Err(e) => {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from(format!("内部并发错误: {}", e)))
                .unwrap()
        }
    }
}

/// GET /api/review/tasks
/// 获取任务历史列表，支持 status、limit、offset、q 过滤
pub async fn handle_list_tasks(
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let status_filter = params.get("status").cloned();
    let q_filter = params.get("q").cloned();
    let limit: usize = params.get("limit").and_then(|s| s.parse().ok()).unwrap_or(50);
    let offset: usize = params.get("offset").and_then(|s| s.parse().ok()).unwrap_or(0);

    let result = tokio::task::spawn_blocking(move || {
        let conn = rusqlite::Connection::open(crate::db::get_db_cache_path())
            .map_err(|e| e.to_string())?;

        let mut query = "SELECT id, title, status, cli_name, cli_path, time_range, selected_ides_json,
                                prompt_text, prompt_hash, metrics_snapshot_json, metrics_hash, dedupe_key,
                                progress_stage, progress_percent, status_message, output_markdown,
                                error_message, exit_code, created_at, started_at, finished_at,
                                canceled_at, last_heartbeat_at, error_type, quality_feedback, action_items_json, compare_metrics_snapshot_json FROM review_tasks WHERE 1=1".to_string();
        
        let mut query_params = Vec::new();

        if let Some(ref st) = status_filter {
            if !st.trim().is_empty() {
                query.push_str(" AND status = ?");
                query_params.push(rusqlite::types::Value::Text(st.trim().to_string()));
            }
        }

        if let Some(ref q) = q_filter {
            if !q.trim().is_empty() {
                query.push_str(" AND (title LIKE ? OR output_markdown LIKE ?)");
                let q_str = format!("%{}%", q.trim());
                query_params.push(rusqlite::types::Value::Text(q_str.clone()));
                query_params.push(rusqlite::types::Value::Text(q_str));
            }
        }

        query.push_str(" ORDER BY created_at DESC LIMIT ? OFFSET ?");
        query_params.push(rusqlite::types::Value::Integer(limit as i64));
        query_params.push(rusqlite::types::Value::Integer(offset as i64));

        let mut stmt = conn.prepare(&query).map_err(|e| e.to_string())?;
        
        let rows = stmt.query_map(rusqlite::params_from_iter(query_params), |row| {
            Ok(ReviewTask {
                id: row.get(0)?,
                title: row.get(1)?,
                status: row.get(2)?,
                cli_name: row.get(3)?,
                cli_path: row.get(4)?,
                time_range: row.get(5)?,
                selected_ides_json: row.get(6)?,
                prompt_text: row.get(7)?,
                prompt_hash: row.get(8)?,
                metrics_snapshot_json: row.get(9)?,
                metrics_hash: row.get(10)?,
                dedupe_key: row.get(11)?,
                progress_stage: row.get(12)?,
                progress_percent: row.get(13)?,
                status_message: row.get(14)?,
                output_markdown: row.get(15)?,
                error_message: row.get(16)?,
                exit_code: row.get(17)?,
                created_at: row.get(18)?,
                started_at: row.get(19)?,
                finished_at: row.get(20)?,
                canceled_at: row.get(21)?,
                last_heartbeat_at: row.get(22)?,
                error_type: row.get(23)?,
                quality_feedback: row.get(24)?,
                action_items_json: row.get(25)?,
                compare_metrics_snapshot_json: row.get(26)?,
            })
        }).map_err(|e| e.to_string())?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r.map_err(|e| e.to_string())?);
        }

        Ok::<_, String>(list)
    }).await;

    match result {
        Ok(Ok(list)) => {
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from(serde_json::to_string(&list).unwrap()))
                .unwrap()
        }
        _ => {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from("查询任务历史失败"))
                .unwrap()
        }
    }
}

/// GET /api/review/tasks/active
/// 获取当前处于 pending/running 的任务，若无则返回 null
pub async fn handle_get_active_task() -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let conn = rusqlite::Connection::open(crate::db::get_db_cache_path())
            .map_err(|e| e.to_string())?;

        let task: Option<ReviewTask> = conn.query_row(
            "SELECT id, title, status, cli_name, cli_path, time_range, selected_ides_json,
                    prompt_text, prompt_hash, metrics_snapshot_json, metrics_hash, dedupe_key,
                    progress_stage, progress_percent, status_message, output_markdown,
                    error_message, exit_code, created_at, started_at, finished_at,
                    canceled_at, last_heartbeat_at, error_type, quality_feedback, action_items_json, compare_metrics_snapshot_json 
             FROM review_tasks 
             WHERE status IN ('pending', 'running') 
             ORDER BY created_at DESC LIMIT 1",
            [],
            |row| {
                Ok(ReviewTask {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    status: row.get(2)?,
                    cli_name: row.get(3)?,
                    cli_path: row.get(4)?,
                    time_range: row.get(5)?,
                    selected_ides_json: row.get(6)?,
                    prompt_text: row.get(7)?,
                    prompt_hash: row.get(8)?,
                    metrics_snapshot_json: row.get(9)?,
                    metrics_hash: row.get(10)?,
                    dedupe_key: row.get(11)?,
                    progress_stage: row.get(12)?,
                    progress_percent: row.get(13)?,
                    status_message: row.get(14)?,
                    output_markdown: row.get(15)?,
                    error_message: row.get(16)?,
                    exit_code: row.get(17)?,
                    created_at: row.get(18)?,
                    started_at: row.get(19)?,
                    finished_at: row.get(20)?,
                    canceled_at: row.get(21)?,
                    last_heartbeat_at: row.get(22)?,
                    error_type: row.get(23)?,
                    quality_feedback: row.get(24)?,
                    action_items_json: row.get(25)?,
                    compare_metrics_snapshot_json: row.get(26)?,
                })
            },
        ).ok();

        Ok::<_, String>(task)
    }).await;

    match result {
        Ok(Ok(Some(task))) => {
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from(serde_json::to_string(&task).unwrap()))
                .unwrap()
        }
        _ => {
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from("null"))
                .unwrap()
        }
    }
}

/// GET /api/review/tasks/{id}
/// 获取特定任务详情
pub async fn handle_get_task(
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let conn = rusqlite::Connection::open(crate::db::get_db_cache_path())
            .map_err(|e| e.to_string())?;

        query_task_by_id(&conn, &id).map_err(|e| e.to_string())
    }).await;

    match result {
        Ok(Ok(task)) => {
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from(serde_json::to_string(&task).unwrap()))
                .unwrap()
        }
        _ => {
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from("任务不存在"))
                .unwrap()
        }
    }
}

/// GET /api/review/tasks/{id}/events?after={sequence}
/// 智能 SSE 推流，实现事件重放 + 实时订阅
pub async fn handle_task_events(
    axum::extract::Path(id): axum::extract::Path<String>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let after_seq: i64 = params.get("after").and_then(|s| s.parse().ok()).unwrap_or(0);

    // 1. 尝试获取实时广播通道的订阅器 (在查询 DB 之前，防止漏掉高并发下的实时包)
    let rx_opt = {
        if let Ok(mgr) = get_task_manager().lock() {
            mgr.active_tasks.get(&id).map(|t| t.tx.subscribe())
        } else {
            None
        }
    };

    // 2. 查询并读取已经持久化在数据库中的事件历史
    let id_str = id.clone();
    let history_res = tokio::task::spawn_blocking(move || {
        let conn = rusqlite::Connection::open(crate::db::get_db_cache_path())?;
        let mut stmt = conn.prepare(
            "SELECT id, task_id, sequence, kind, message, payload_json, created_at 
             FROM review_task_events 
             WHERE task_id = ? AND sequence > ? 
             ORDER BY sequence ASC"
        )?;
        
        let rows = stmt.query_map(rusqlite::params![id_str, after_seq], |row| {
            Ok(TaskEvent {
                id: row.get(0)?,
                task_id: row.get(1)?,
                sequence: row.get(2)?,
                kind: row.get(3)?,
                message: row.get(4)?,
                payload_json: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?;

        let mut events = Vec::new();
        for r in rows {
            events.push(r?);
        }
        Ok::<_, rusqlite::Error>(events)
    }).await;

    let history_events = history_res.unwrap_or_else(|_| Ok(Vec::new())).unwrap_or_default();
    
    // 计算历史最大序列号，用于后期去重
    let max_history_seq = history_events.iter().map(|e| e.sequence).max().unwrap_or(after_seq);

    // 3. 构建异步 Stream 通道
    let (tx_sse, rx_sse) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(128);

    tokio::spawn(async move {
        // 先逐个推送历史重放事件
        for ev in history_events {
            let data_str = serde_json::to_string(&ev).unwrap_or_default();
            let event = Event::default().event(ev.kind).data(data_str);
            if tx_sse.send(Ok(event)).await.is_err() {
                return; // SSE client disconnected
            }
        }

        // 若有活跃任务订阅，无缝切换到实时流式推送
        if let Some(mut rx) = rx_opt {
            loop {
                tokio::select! {
                    msg_res = rx.recv() => {
                        match msg_res {
                            Ok(ev) => {
                                // 去重校验：只推送高于最大历史序号的事件
                                if ev.sequence > max_history_seq {
                                    let is_done_event = ev.kind == "done" || ev.kind == "error";
                                    let data_str = serde_json::to_string(&ev).unwrap_or_default();
                                    let event = Event::default().event(ev.kind).data(data_str);
                                    if tx_sse.send(Ok(event)).await.is_err() {
                                        break; // client disconnected
                                    }
                                    if is_done_event {
                                        break; // 任务正常结束，退出流
                                    }
                                }
                            }
                            Err(_) => {
                                // 广播通道关闭，退出
                                break;
                            }
                        }
                    }
                    else => {
                        break;
                    }
                }
            }
        } else {
            // 没有活跃任务，表示任务早已结束，无需实时推送。若刚才历史数据中没有 done/error，最后追加补发 done 告知前端关闭
            let conn_res = rusqlite::Connection::open(crate::db::get_db_cache_path());
            let is_finished = if let Ok(conn) = conn_res {
                let status: String = conn.query_row(
                    "SELECT status FROM review_tasks WHERE id = ?",
                    [&id],
                    |row| row.get(0),
                ).unwrap_or_else(|_| "succeeded".to_string());
                status != "pending" && status != "running"
            } else {
                true
            };

            if is_finished {
                let ev = TaskEvent {
                    id: None,
                    task_id: id,
                    sequence: max_history_seq + 1,
                    kind: "done".to_string(),
                    message: "[DONE]".to_string(),
                    payload_json: None,
                    created_at: chrono::Utc::now().to_rfc3339(),
                };
                let event = Event::default().event("done").data(serde_json::to_string(&ev).unwrap());
                let _ = tx_sse.send(Ok(event)).await;
            }
        }
    });

    let stream = ReceiverStream::new(rx_sse);
    Sse::new(stream)
        .keep_alive(axum::response::sse::KeepAlive::new().interval(Duration::from_secs(15)))
}

/// POST /api/review/tasks/{id}/cancel
/// 取消复盘任务，终止 CLI 子进程
pub async fn handle_cancel_task(
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    let id_clone = id.clone();
    
    // 1. 在全局活跃任务管理器中寻找该任务
    let active_task = {
        if let Ok(mut mgr) = get_task_manager().lock() {
            mgr.active_tasks.remove(&id)
        } else {
            None
        }
    };

    if let Some(t) = active_task {
        // 2. Kill 活跃子进程
        if let Some(mut child) = t.child.lock().await.take() {
            #[cfg(target_os = "windows")]
            {
                if let Some(pid) = child.id() {
                    let mut cmd = tokio::process::Command::new("taskkill");
                    cmd.args(["/F", "/T", "/PID", &pid.to_string()]);
                    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
                    let _ = cmd.output().await;
                } else {
                    let _ = child.kill().await;
                }
            }
            #[cfg(not(target_os = "windows"))]
            {
                let _ = child.kill().await;
            }
        }

        // 3. 记录 cancel 状态并向订阅流推送事件
        let _ = record_and_broadcast_event(&id_clone, "stage", "收到用户主动发起的取消请求，正在终止进程...", None, &t.tx).await;
        
        let now = chrono::Utc::now().to_rfc3339();
        let _ = tokio::task::spawn_blocking(move || {
            if let Ok(conn) = rusqlite::Connection::open(crate::db::get_db_cache_path()) {
                let _ = conn.execute(
                    "UPDATE review_tasks 
                     SET status = 'canceled', 
                         progress_stage = 'canceled', 
                         status_message = '任务已被取消', 
                         canceled_at = ?, 
                         finished_at = ? 
                     WHERE id = ?",
                    [now.clone(), now, id_clone],
                );
            }
        }).await;

        let ev_done = TaskEvent {
            id: None,
            task_id: id,
            sequence: 9999,
            kind: "done".to_string(),
            message: "任务已被取消".to_string(),
            payload_json: None,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        let _ = t.tx.send(ev_done);

        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .body(Body::from(serde_json::json!({ "success": true, "message": "任务取消成功，AI 子进程已强行终止。" }).to_string()))
            .unwrap()
    } else {
        // 若活跃任务中没有，但数据库中仍为运行中状态，执行静默修复
        let _ = tokio::task::spawn_blocking(move || {
            if let Ok(conn) = rusqlite::Connection::open(crate::db::get_db_cache_path()) {
                let now = chrono::Utc::now().to_rfc3339();
                let _ = conn.execute(
                    "UPDATE review_tasks 
                     SET status = 'canceled', 
                         progress_stage = 'canceled', 
                         status_message = '任务已被取消', 
                         canceled_at = ?, 
                         finished_at = ? 
                     WHERE id = ? AND status IN ('pending', 'running')",
                    [now.clone(), now, id],
                );
            }
        }).await;

        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .body(Body::from(serde_json::json!({ "success": true, "message": "任务未在内存活跃列表，但已在数据库中标记为取消。" }).to_string()))
            .unwrap()
    }
}

/// DELETE /api/review/tasks/{id}
/// 删除指定历史复盘任务记录
pub async fn handle_delete_task(
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    // 1. 判断是否为运行中任务
    let is_active = {
        if let Ok(mgr) = get_task_manager().lock() {
            mgr.active_tasks.contains_key(&id)
        } else {
            false
        }
    };

    if is_active {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .body(Body::from(serde_json::json!({ "success": false, "message": "不能删除正在运行的分析任务！" }).to_string()))
            .unwrap();
    }

    let id_clone = id.clone();
    let result = tokio::task::spawn_blocking(move || {
        let conn = rusqlite::Connection::open(crate::db::get_db_cache_path())
            .map_err(|e| e.to_string())?;

        // 校验数据库状态，确保不是 running 状态
        let status: String = conn.query_row(
            "SELECT status FROM review_tasks WHERE id = ?",
            [&id_clone],
            |row| row.get(0),
        ).unwrap_or_else(|_| "not_found".to_string());

        if status == "pending" || status == "running" {
            return Err("不能删除数据库中标记为进行中的任务！".to_string());
        }

        // 删除任务关联的事件表
        conn.execute(
            "DELETE FROM review_task_events WHERE task_id = ?",
            [&id_clone],
        ).map_err(|e| e.to_string())?;

        // 删除任务主表
        let count = conn.execute(
            "DELETE FROM review_tasks WHERE id = ?",
            [&id_clone],
        ).map_err(|e| e.to_string())?;

        if count == 0 {
            return Err("未找到待删除的任务记录".to_string());
        }

        Ok::<_, String>(())
    }).await;

    match result {
        Ok(Ok(())) => {
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from(serde_json::json!({ "success": true, "message": "删除复盘历史成功" }).to_string()))
                .unwrap()
        }
        Ok(Err(e)) => {
            Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from(serde_json::json!({ "success": false, "message": e }).to_string()))
                .unwrap()
        }
        _ => {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from(serde_json::json!({ "success": false, "message": "删除任务发生服务器内部异常" }).to_string()))
                .unwrap()
        }
    }
}

/// POST /api/review/tasks/{id}/retry
/// 重新运行失败/中断的任务
pub async fn handle_retry_task(
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking({
        let id_clone = id.clone();
        move || {
            let conn = rusqlite::Connection::open(crate::db::get_db_cache_path())
                .map_err(|e| e.to_string())?;

            // 1. 获取原任务详情
            let task = query_task_by_id(&conn, &id_clone).map_err(|e| e.to_string())?;

            // 仅允许重试非活跃（已失败、已取消或已中断）的任务
            if task.status == "pending" || task.status == "running" {
                return Err("任务已处于活跃状态，无需重试。".to_string());
            }

            // 2. 将原任务状态重置为 pending，清空错误和旧输出
            conn.execute(
                "UPDATE review_tasks 
                 SET status = 'pending', 
                     progress_stage = 'created', 
                     progress_percent = 5, 
                     status_message = '任务正在重新启动...', 
                     output_markdown = '', 
                     error_message = NULL, 
                     error_type = NULL,
                     exit_code = NULL, 
                     started_at = NULL, 
                     finished_at = NULL, 
                     canceled_at = NULL 
                 WHERE id = ?",
                [&id_clone],
            ).map_err(|e| format!("重置任务状态失败: {}", e))?;

            // 清空该任务旧的历史事件
            conn.execute(
                "DELETE FROM review_task_events WHERE task_id = ?",
                [&id_clone],
            ).map_err(|e| format!("清理历史事件失败: {}", e))?;

            Ok::<ReviewTask, String>(task)
        }
    }).await;

    match result {
        Ok(Ok(task)) => {
            let task_id = task.id.clone();
            let cli_name = task.cli_name.clone();
            let prompt = task.prompt_text.clone();

            let (tx, _) = broadcast::channel::<TaskEvent>(100);
            let child = Arc::new(tokio::sync::Mutex::new(None));

            let active_task = Arc::new(ActiveTask {
                task_id: task_id.clone(),
                tx: tx.clone(),
                child: child.clone(),
            });

            // 注册到全局活跃任务
            if let Ok(mut mgr) = get_task_manager().lock() {
                mgr.active_tasks.insert(task_id.clone(), active_task);
            }

            // 启动后台分析协程
            tokio::spawn(async move {
                if let Err(e) = run_cli_task_background(&task_id, &cli_name, prompt, child, tx.clone()).await {
                    eprintln!("[后台复盘任务] 重试任务 {} 异常结束: {}", task_id, e);
                }
            });

            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from(serde_json::json!({ "success": true, "message": "任务已成功重试并拉起分析。" }).to_string()))
                .unwrap()
        }
        Ok(Err(e)) => {
            Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from(serde_json::json!({ "success": false, "message": e }).to_string()))
                .unwrap()
        }
        _ => {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from(serde_json::json!({ "success": false, "message": "内部服务器并发错误" }).to_string()))
                .unwrap()
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SaveActionItemsRequest {
    pub action_items_json: String,
}

/// POST /api/review/tasks/{id}/action-items
/// 保存或更新行动项
pub async fn handle_save_action_items(
    axum::extract::Path(id): axum::extract::Path<String>,
    axum::Json(req): axum::Json<SaveActionItemsRequest>,
) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let conn = rusqlite::Connection::open(crate::db::get_db_cache_path())
            .map_err(|e| e.to_string())?;

        conn.execute(
            "UPDATE review_tasks SET action_items_json = ? WHERE id = ?",
            rusqlite::params![req.action_items_json, id],
        ).map_err(|e| e.to_string())?;

        Ok::<(), String>(())
    }).await;

    match result {
        Ok(Ok(())) => {
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from(serde_json::json!({ "success": true }).to_string()))
                .unwrap()
        }
        _ => {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from("更新行动项失败"))
                .unwrap()
        }
    }
}

// ============================================================
// 崩溃检查与异常恢复
// ============================================================

pub fn recover_interrupted_tasks() -> Result<(), rusqlite::Error> {
    let conn = rusqlite::Connection::open(crate::db::get_db_cache_path())?;
    
    // 查询是否有残留的 pending/running 任务
    let count: i64 = conn.query_row(
        "SELECT COUNT(1) FROM review_tasks WHERE status IN ('pending', 'running')",
        [],
        |row| row.get(0),
    )?;

    if count > 0 {
        let now = chrono::Utc::now().to_rfc3339();
        println!("[异常恢复] 检测到数据库中有 {} 条残留任务未正常终止，将其全部标记为 'interrupted'。", count);
        
        conn.execute(
            "UPDATE review_tasks 
             SET status = 'interrupted', 
                 progress_stage = 'interrupted',
                 status_message = '任务随软件重启中断',
                 finished_at = ? 
             WHERE status IN ('pending', 'running')",
            [now],
        )?;
    }

    Ok(())
}

// ============================================================
// 核心：后台 CLI 执行协程
// ============================================================

enum ReaderMsg {
    Stdout(String),
    Stderr(String),
}

async fn run_cli_task_background(
    task_id: &str,
    cli_name: &str,
    prompt: String,
    child_mutex: Arc<tokio::sync::Mutex<Option<tokio::process::Child>>>,
    tx: broadcast::Sender<TaskEvent>,
) -> Result<(), String> {
    let task_id_str = task_id.to_string();
    let now = chrono::Utc::now().to_rfc3339();

    // 1. 初始化更新任务主表 status='running' 和 started_at
    tokio::task::spawn_blocking({
        let t_id = task_id_str.clone();
        let now_clone = now.clone();
        move || {
            if let Ok(conn) = rusqlite::Connection::open(crate::db::get_db_cache_path()) {
                let _ = conn.execute(
                    "UPDATE review_tasks 
                     SET status = 'running', 
                         progress_stage = 'snapshot_ready', 
                         progress_percent = 15, 
                         status_message = '已冻结分析数据快照', 
                         started_at = ? 
                     WHERE id = ?",
                    [now_clone, t_id],
                );
            }
        }
    }).await.map_err(|e| format!("启动状态同步失败: {}", e))?;

    let _ = record_and_broadcast_event(&task_id_str, "stage", "已冻结分析数据快照", Some("{\"percent\": 15, \"stage\": \"snapshot_ready\"}"), &tx).await;
    let _ = record_and_broadcast_event(&task_id_str, "stage", "已生成分析提示词", Some("{\"percent\": 25, \"stage\": \"prompt_ready\"}"), &tx).await;

    // 2. 检测并定位可执行 CLI 引擎路径
    let _ = record_and_broadcast_event(&task_id_str, "stage", "正在定位 AI CLI 引擎...", Some("{\"percent\": 30, \"stage\": \"prompt_ready\"}"), &tx).await;
    let cli_path = find_cli_in_path(cli_name);

    if cli_path.is_none() {
        let err_msg = format!("❌ 未在系统的 PATH 环境变量中检测到 AI CLI 引擎「{}」", get_cli_display_name(cli_name));
        let _ = record_and_broadcast_event(&task_id_str, "error", &err_msg, None, &tx).await;
        
        update_task_finish(&task_id_str, "failed", Some("cli_resolved"), 35, &err_msg, None, None, Some("CLI_NOT_FOUND")).await;
        
        let _ = tx.send(TaskEvent {
            id: None,
            task_id: task_id_str,
            sequence: 9999,
            kind: "done".to_string(),
            message: "引擎未找到".to_string(),
            payload_json: None,
            created_at: chrono::Utc::now().to_rfc3339(),
        });
        return Err(err_msg);
    }

    let exe_path = cli_path.unwrap();
    let _ = record_and_broadcast_event(&task_id_str, "stage", &format!("已成功定位 AI CLI 引擎：{}", exe_path.to_string_lossy()), Some("{\"percent\": 35, \"stage\": \"cli_resolved\"}"), &tx).await;

    // 3. 构建子进程参数并启动（注入 CLI 自定义环境变量）
    let mut cmd = Command::new(&exe_path);

    // 注入该 CLI 的自定义环境变量（排除 _BIN 路径键）
    {
        let app_cfg = crate::config::load_config();
        if let Some(agent_env) = app_cfg.agent_cli_env.get(cli_name) {
            let bin_key = format!("{}_BIN", cli_name.replace('-', "_").to_uppercase());
            for (k, v) in agent_env {
                if k != &bin_key {
                    cmd.env(k, v);
                }
            }
        }
    }

    if cli_name.starts_with("claude") {
        cmd.args([
            "-p",
            "--output-format",
            "text",
            "--permission-mode",
            "bypassPermissions",
        ]);
    } else if cli_name.starts_with("codex") {
        // 新版 codex 使用 exec 子命令，不再支持 --full-auto -q
        // Windows/WSL 使用 danger-full-access，Unix 使用 workspace-write
        #[cfg(target_os = "windows")]
        cmd.args(["exec", "--skip-git-repo-check", "--sandbox", "danger-full-access"]);
        #[cfg(not(target_os = "windows"))]
        cmd.args(["exec", "--skip-git-repo-check", "--sandbox", "workspace-write"]);
    } else if cli_name.starts_with("gemini") {
        // gemini 需要 --yolo 跳过 TTY 交互确认，通过 stdin 管道接收输入
        cmd.args(["--yolo"]);
    } else if cli_name.starts_with("agy") {
        // agy.exe 新版 CLI 不带 -p 参数，通过 stdin 接收输入
    } else {
        cmd.arg("-p");
    }

    cmd.stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            let err_msg = format!("❌ 启动子进程失败: {}", e);
            let _ = record_and_broadcast_event(&task_id_str, "error", &err_msg, None, &tx).await;
            update_task_finish(&task_id_str, "failed", Some("cli_started"), 45, &err_msg, None, None, Some("CLI_EXECUTION_FAILED")).await;
            return Err(err_msg);
        }
    };

    // 4. 将 stdin 塞入并 EOF
    if let Some(mut stdin) = child.stdin.take() {
        if let Err(e) = stdin.write_all(prompt.as_bytes()).await {
            let err_msg = format!("❌ 往子进程 Stdin 写入 Prompt 发生异常: {}", e);
            let _ = record_and_broadcast_event(&task_id_str, "error", &err_msg, None, &tx).await;
            let _ = child.kill().await;
            update_task_finish(&task_id_str, "failed", Some("cli_started"), 45, &err_msg, None, None, Some("CLI_EXECUTION_FAILED")).await;
            return Err(err_msg);
        }
        drop(stdin); // 关闭输入
    }

    // 5. 登记活跃进程到 Mutex
    {
        *child_mutex.lock().await = Some(child);
    }

    let _ = record_and_broadcast_event(&task_id_str, "stage", "已成功拉起分析进程，正在等待 AI 引擎响应并接收数据...", Some("{\"percent\": 45, \"stage\": \"cli_started\"}"), &tx).await;

    // 6. 开始流式并发读取并输出
    let child = {
        let mut active_child_lock = child_mutex.lock().await;
        active_child_lock.take()
    };
    if child.is_none() {
        // 如果已经被主动取消
        return Ok(());
    }
    let mut child = child.unwrap();

    let stdout_pipe = child.stdout.take().ok_or("Failed to open stdout")?;
    let stderr_pipe = child.stderr.take().ok_or("Failed to open stderr")?;

    let (r_tx, mut r_rx) = tokio::sync::mpsc::channel::<ReaderMsg>(100);

    // 独立Stdout读取器
    let stdout_tx = r_tx.clone();
    tokio::spawn(async move {
        let mut reader = BufReader::new(stdout_pipe).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            let _ = stdout_tx.send(ReaderMsg::Stdout(line)).await;
        }
    });

    // 独立Stderr读取器
    let stderr_tx = r_tx;
    tokio::spawn(async move {
        let mut reader = BufReader::new(stderr_pipe).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            let _ = stderr_tx.send(ReaderMsg::Stderr(line)).await;
        }
    });

    // 将 child 放回 Mutex 挂靠，以便用户能在中途取消它
    {
        *child_mutex.lock().await = Some(child);
    }

    let mut total_chars = 0;
    let mut last_stdout_time = tokio::time::Instant::now();
    let mut last_heartbeat_sec = 0;
    let mut interval = tokio::time::interval(Duration::from_secs(1));

    let mut is_streaming_started = false;
    let mut stderr_buffer = String::new();

    // 协调轮询循环
    loop {
        tokio::select! {
            msg_opt = r_rx.recv() => {
                match msg_opt {
                    Some(ReaderMsg::Stdout(line)) => {
                        if !is_streaming_started {
                            is_streaming_started = true;
                            let _ = record_and_broadcast_event(&task_id_str, "stage", "分析引擎开始响应，流式输出进行中...", Some("{\"percent\": 55, \"stage\": \"streaming\"}"), &tx).await;
                        }

                        last_stdout_time = tokio::time::Instant::now();
                        last_heartbeat_sec = 0;

                        total_chars += line.len();
                        
                        // 动态推进百分比 (55% ~ 90%，每 400 字符推进 1%，直到 90% 封顶)
                        let raw_pct = 55 + (total_chars / 400) as i32;
                        let pct = raw_pct.min(90);

                        // 广播并持久化事件与累加 Markdown 输出
                        let _ = record_and_broadcast_event(&task_id_str, "stdout", &line, None, &tx).await;
                        let _ = record_and_broadcast_event(&task_id_str, "stdout", "\n", None, &tx).await;

                        append_markdown_to_db(&task_id_str, &format!("{}\n", line), pct).await;
                    }
                    Some(ReaderMsg::Stderr(line)) => {
                        let _ = record_and_broadcast_event(&task_id_str, "stderr", &line, None, &tx).await;
                        if stderr_buffer.len() < 5000 {
                            stderr_buffer.push_str(&line);
                            stderr_buffer.push('\n');
                        }
                    }
                    None => {
                        // 读取流全部结束
                        break;
                    }
                }
            }
            _ = interval.tick() => {
                // 心跳机制：超过 15 秒无响应，且每隔 15 秒发起等待通知
                let elapsed = last_stdout_time.elapsed().as_secs();
                if elapsed >= 15 && elapsed % 15 == 0 && elapsed != last_heartbeat_sec {
                    last_heartbeat_sec = elapsed;
                    let hb_msg = format!("正在等待 {} 响应，已等待 {} 秒", get_cli_display_name(cli_name), elapsed);
                    let _ = record_and_broadcast_event(&task_id_str, "heartbeat", &hb_msg, None, &tx).await;
                    
                    update_task_heartbeat(&task_id_str, &hb_msg).await;
                }
            }
        }
    }

    // 从 Mutex 移除 child
    let mut final_child_lock = child_mutex.lock().await;
    let final_child = final_child_lock.take();

    let exit_status = if let Some(mut c) = final_child {
        tokio::time::timeout(Duration::from_secs(10), c.wait()).await.ok().and_then(|r| r.ok())
    } else {
        None
    };

    // 7. 处理任务结束状态
    let exit_code = exit_status.and_then(|s| s.code());
    let success = exit_status.map(|s| s.success()).unwrap_or(true); // 即使没等到，若有大量输出也可以视作成功

    let _ = record_and_broadcast_event(&task_id_str, "stage", "正在保存报告并完成收尾工作...", Some("{\"percent\": 95, \"stage\": \"persisting\"}"), &tx).await;

    let (status_str, progress_stage, progress_percent, status_msg, err_type) = if success {
        ("succeeded", "done", 100, "分析完成", None)
    } else {
        let diag = diagnose_error(&stderr_buffer, exit_code);
        ("failed", "done", 100, "CLI 分析进程异常退出", Some(diag))
    };

    update_task_finish(&task_id_str, status_str, Some(progress_stage), progress_percent, status_msg, None, exit_code, err_type).await;

    let _ = record_and_broadcast_event(&task_id_str, "stage", status_msg, Some(&format!("{{\"percent\": {}, \"stage\": \"{}\"}}", progress_percent, progress_stage)), &tx).await;

    // 清除全局活跃任务
    if let Ok(mut mgr) = get_task_manager().lock() {
        mgr.active_tasks.remove(&task_id_str);
    }

    let ev_done = TaskEvent {
        id: None,
        task_id: task_id_str.clone(),
        sequence: 9999,
        kind: "done".to_string(),
        message: "[DONE]".to_string(),
        payload_json: None,
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    let _ = tx.send(ev_done);

    Ok(())
}

// ============================================================
// 数据库更新助手方法
// ============================================================

fn query_task_by_id(conn: &rusqlite::Connection, id: &str) -> Result<ReviewTask, rusqlite::Error> {
    conn.query_row(
        "SELECT id, title, status, cli_name, cli_path, time_range, selected_ides_json,
                prompt_text, prompt_hash, metrics_snapshot_json, metrics_hash, dedupe_key,
                progress_stage, progress_percent, status_message, output_markdown,
                error_message, exit_code, created_at, started_at, finished_at,
                canceled_at, last_heartbeat_at, error_type, quality_feedback, action_items_json, compare_metrics_snapshot_json FROM review_tasks WHERE id = ?",
        [id],
        |row| {
            Ok(ReviewTask {
                id: row.get(0)?,
                title: row.get(1)?,
                status: row.get(2)?,
                cli_name: row.get(3)?,
                cli_path: row.get(4)?,
                time_range: row.get(5)?,
                selected_ides_json: row.get(6)?,
                prompt_text: row.get(7)?,
                prompt_hash: row.get(8)?,
                metrics_snapshot_json: row.get(9)?,
                metrics_hash: row.get(10)?,
                dedupe_key: row.get(11)?,
                progress_stage: row.get(12)?,
                progress_percent: row.get(13)?,
                status_message: row.get(14)?,
                output_markdown: row.get(15)?,
                error_message: row.get(16)?,
                exit_code: row.get(17)?,
                created_at: row.get(18)?,
                started_at: row.get(19)?,
                finished_at: row.get(20)?,
                canceled_at: row.get(21)?,
                last_heartbeat_at: row.get(22)?,
                error_type: row.get(23)?,
                quality_feedback: row.get(24)?,
                action_items_json: row.get(25)?,
                compare_metrics_snapshot_json: row.get(26)?,
            })
        },
    )
}

async fn append_markdown_to_db(task_id: &str, chunk: &str, pct: i32) {
    let task_id_str = task_id.to_string();
    let chunk_str = chunk.to_string();
    let _ = tokio::task::spawn_blocking(move || {
        if let Ok(conn) = rusqlite::Connection::open(crate::db::get_db_cache_path()) {
            let _ = conn.execute(
                "UPDATE review_tasks 
                 SET output_markdown = output_markdown || ?, 
                     progress_percent = ? 
                 WHERE id = ?",
                rusqlite::params![chunk_str, pct, task_id_str],
            );
        }
    }).await;
}

async fn update_task_heartbeat(task_id: &str, msg: &str) {
    let task_id_str = task_id.to_string();
    let msg_str = msg.to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let _ = tokio::task::spawn_blocking(move || {
        if let Ok(conn) = rusqlite::Connection::open(crate::db::get_db_cache_path()) {
            let _ = conn.execute(
                "UPDATE review_tasks 
                 SET status_message = ?, 
                     last_heartbeat_at = ? 
                 WHERE id = ?",
                rusqlite::params![msg_str, now, task_id_str],
            );
        }
    }).await;
}

fn diagnose_error(stderr: &str, _exit_code: Option<i32>) -> &'static str {
    let s_lower = stderr.to_lowercase();
    if s_lower.contains("not found") || s_lower.contains("cannot find") || s_lower.contains("is not recognized") {
        "CLI_NOT_FOUND"
    } else if s_lower.contains("login") || s_lower.contains("auth") || s_lower.contains("authenticate") || s_lower.contains("unauthorized") || s_lower.contains("not logged in") {
        "CLI_NOT_LOGGED_IN"
    } else if s_lower.contains("permission") || s_lower.contains("access denied") || s_lower.contains("eacces") || s_lower.contains("privilege") {
        "CLI_PERMISSION_DENIED"
    } else if s_lower.contains("timeout") || s_lower.contains("timed out") {
        "CLI_TIMEOUT"
    } else {
        "CLI_EXECUTION_FAILED"
    }
}

async fn update_task_finish(
    task_id: &str,
    status: &str,
    stage: Option<&str>,
    percent: i32,
    msg: &str,
    err: Option<&str>,
    exit_code: Option<i32>,
    error_type: Option<&str>,
) {
    let task_id_str = task_id.to_string();
    let status_str = status.to_string();
    let stage_str = stage.map(|s| s.to_string());
    let msg_str = msg.to_string();
    let err_str = err.map(|s| s.to_string());
    let error_type_str = error_type.map(|s| s.to_string());
    let now = chrono::Utc::now().to_rfc3339();

    let _ = tokio::task::spawn_blocking(move || {
        if let Ok(conn) = rusqlite::Connection::open(crate::db::get_db_cache_path()) {
            let _ = conn.execute(
                "UPDATE review_tasks 
                 SET status = ?, 
                     progress_stage = COALESCE(?, progress_stage), 
                     progress_percent = ?, 
                     status_message = ?, 
                     error_message = ?, 
                     exit_code = ?,
                     finished_at = ?,
                     error_type = ?
                 WHERE id = ?",
                rusqlite::params![
                    status_str, stage_str, percent, msg_str, err_str, exit_code, now, error_type_str, task_id_str
                ],
            );
        }
    }).await;
}

// ============================================================
// 兼容性/辅助 CLI 处理器及模板
// ============================================================

fn get_cli_display_name(bin: &str) -> &'static str {
    match bin {
        "claude" => "Claude Code",
        "codex" => "Codex CLI",
        "gemini" => "Gemini CLI",
        "agy" => "Antigravity CLI",
        "cursor-agent" => "Cursor Agent",
        "opencode" => "OpenCode CLI",
        "qwen" => "Qwen CLI",
        "copilot" => "GitHub Copilot",
        "devin" => "Devin CLI",
        "kimi" => "Kimi CLI",
        "qoder" => "Qoder CLI",
        "pi" => "Pi CLI",
        "kiro" => "Kiro Agent",
        "kilo" => "Kilo Code",
        "vibe" => "Vibe CLI",
        "deepseek" => "DeepSeek CLI",
        "hermes" => "Hermes CLI",
        "grok-build" => "Grok Build",
        "reasonix" => "Reasonix CLI",
        "aider" => "Aider",
        _ => "AI CLI",
    }
}

/// 保持原 review.rs 核心 Prompt 模板与生成工程
const DEFAULT_PROMPT_TEMPLATE: &str = r#"你是一位专业的 AI 工具使用顾问。我使用 AI Token Monitor 追踪了我在 {{IDE}} 等工具上的 Token 消耗情况。

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
- 综合评价我的 AI 使用效率（满分100分，给出评分 and 理由）
- 与一般开发者的平均水平相比，我的数据表现如何？

### 4. 本周行动清单
列出 3 条我这周可以立刻执行的具体优化行动（要具体到操作步骤，不要泛泛而谈）。

---

请直接开始输出报告，不需要前言。保持语言简洁专业，使用 Markdown 格式。
"#;

fn buildPromptFromTemplate(template: &str, ides: &[String]) -> String {
    let ide_display = if ides.contains(&"all".to_string()) || ides.is_empty() {
        "全部工具 (Antigravity、Claude Code、Codex CLI、Cursor、Trae、Trae CN)".to_string()
    } else {
        let mapped: Vec<String> = ides.iter().map(|s| {
            match s.as_str() {
                "antigravity" => "Antigravity".to_string(),
                "claude_code" => "Claude Code".to_string(),
                "codex" => "Codex CLI".to_string(),
                "cursor" => "Cursor".to_string(),
                "trae" => "Trae".to_string(),
                "trae_cn" => "Trae CN".to_string(),
                _ => s.to_string(),
            }
        }).collect();
        mapped.join("、")
    };
    template.replace("{{IDE}}", &ide_display)
}

fn replace_placeholders(text: &str, req: &ReviewRequest) -> String {
    let time_label = req.time_range.as_deref().unwrap_or("最近30天");
    let total_tokens = req.total_tokens.unwrap_or(0);
    let total_cost = req.total_cost_usd.unwrap_or(0.0);
    let total_sessions = req.total_sessions.unwrap_or(0);
    let cache_rate = req.cache_hit_rate.unwrap_or(0.0);
    let thinking_ratio = req.thinking_ratio.unwrap_or(0.0);

    let source_section = req
        .source_breakdown
        .as_deref()
        .map(|s| format!("\n**各工具来源分布：**\n```json\n{}\n```", s))
        .unwrap_or_default();

    let model_section = req
        .model_distribution
        .as_deref()
        .map(|s| format!("\n**模型使用分布：**\n```json\n{}\n```", s))
        .unwrap_or_default();

    let trend_section = req
        .daily_trend_summary
        .as_deref()
        .map(|s| format!("\n**日均消耗趋势（近期）：**\n```json\n{}\n```", s))
        .unwrap_or_default();

    let total_tokens_fmt = if total_tokens >= 1_000_000 {
        format!("{:.1}M", total_tokens as f64 / 1_000_000.0)
    } else if total_tokens >= 1_000 {
        format!("{:.1}K", total_tokens as f64 / 1_000.0)
    } else {
        total_tokens.to_string()
    };

    let selected_ides_vec: Vec<String> = req.selected_ides.as_ref()
        .map(|s| s.split(',').map(|i| i.to_string()).collect())
        .unwrap_or_else(|| vec!["all".to_string()]);

    let mut replaced = buildPromptFromTemplate(text, &selected_ides_vec);

    replaced = replaced
        .replace("最近7天", time_label)
        .replace("{{TOTAL_TOKENS}}", &total_tokens_fmt)
        .replace("{{TOTAL_COST}}", &format!("{:.4}", total_cost))
        .replace("{{TOTAL_SESSIONS}}", &total_sessions.to_string())
        .replace("{{CACHE_HIT_RATE}}", &format!("{:.1}", cache_rate * 100.0))
        .replace("{{THINKING_RATIO}}", &format!("{:.1}", thinking_ratio * 100.0))
        .replace("{{SOURCE_BREAKDOWN}}", &source_section)
        .replace("{{MODEL_DISTRIBUTION}}", &model_section)
        .replace("{{DAILY_TREND_SUMMARY}}", &trend_section);

    replaced
}

fn build_review_prompt(req: &ReviewRequest) -> String {
    replace_placeholders(DEFAULT_PROMPT_TEMPLATE, req)
}

#[derive(Debug, Clone, Deserialize)]
pub struct SaveFeedbackRequest {
    pub feedback: String,
}

/// POST /api/review/tasks/{id}/feedback
/// 保存或更新报告有用性反馈
pub async fn handle_save_quality_feedback(
    axum::extract::Path(id): axum::extract::Path<String>,
    axum::Json(req): axum::Json<SaveFeedbackRequest>,
) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let conn = rusqlite::Connection::open(crate::db::get_db_cache_path())
            .map_err(|e| e.to_string())?;

        conn.execute(
            "UPDATE review_tasks SET quality_feedback = ? WHERE id = ?",
            rusqlite::params![req.feedback, id],
        ).map_err(|e| e.to_string())?;

        Ok::<(), String>(())
    }).await;

    match result {
        Ok(Ok(())) => {
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from(serde_json::json!({ "success": true }).to_string()))
                .unwrap()
        }
        _ => {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from("更新反馈失败"))
                .unwrap()
        }
    }
}

/// GET /api/review/turns/details
pub async fn handle_get_turn_details(
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let source = params.get("source").cloned().unwrap_or_default();
    let uuid = params.get("uuid").cloned().unwrap_or_default();
    let idx: i64 = params.get("idx").and_then(|s| s.parse().ok()).unwrap_or(0);

    let result = tokio::task::spawn_blocking(move || {
        let conn = rusqlite::Connection::open(crate::db::get_db_cache_path())
            .map_err(|e| e.to_string())?;

        let details = conn.query_row(
            "SELECT source, uuid, idx, user_prompt, executed_commands, failed_commands, modified_files 
             FROM turn_details 
             WHERE source = ? AND uuid = ? AND idx = ?",
            rusqlite::params![source, uuid, idx],
            |row| {
                Ok(TurnDetails {
                    source: row.get(0)?,
                    uuid: row.get(1)?,
                    idx: row.get(2)?,
                    user_prompt: row.get(3)?,
                    executed_commands: row.get(4)?,
                    failed_commands: row.get(5)?,
                    modified_files: row.get(6)?,
                })
            },
        ).map_err(|e| e.to_string())?;

        Ok::<_, String>(details)
    }).await;

    match result {
        Ok(Ok(details)) => {
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from(serde_json::to_string(&details).unwrap()))
                .unwrap()
        }
        _ => {
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from("未找到该轮次的执行细节"))
                .unwrap()
        }
    }
}

// ============================================================
// 专家分析提示词模板 CRUD
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTemplate {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub template: String,
    pub is_builtin: i32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreatePromptTemplateReq {
    pub name: String,
    pub description: Option<String>,
    pub template: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdatePromptTemplateReq {
    pub name: String,
    pub description: Option<String>,
    pub template: String,
}

/// GET /api/review/prompt_templates
pub async fn handle_list_prompt_templates() -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let conn = rusqlite::Connection::open(crate::db::get_db_cache_path())
            .map_err(|e| e.to_string())?;
        
        let mut stmt = conn.prepare(
            "SELECT id, name, description, template, is_builtin, created_at, updated_at 
             FROM prompt_templates 
             ORDER BY is_builtin DESC, created_at ASC"
        ).map_err(|e| e.to_string())?;

        let rows = stmt.query_map([], |row| {
            Ok(PromptTemplate {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                template: row.get(3)?,
                is_builtin: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        }).map_err(|e| e.to_string())?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r.map_err(|e| e.to_string())?);
        }
        Ok::<_, String>(list)
    }).await;

    match result {
        Ok(Ok(list)) => {
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from(serde_json::to_string(&list).unwrap()))
                .unwrap()
        }
        Ok(Err(e)) => {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from(serde_json::json!({ "success": false, "message": e }).to_string()))
                .unwrap()
        }
        Err(e) => {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from(serde_json::json!({ "success": false, "message": format!("Server thread error: {}", e) }).to_string()))
                .unwrap()
        }
    }
}

/// POST /api/review/prompt_templates
pub async fn handle_create_prompt_template(
    axum::Json(req): axum::Json<CreatePromptTemplateReq>,
) -> impl IntoResponse {
    let name_trimmed = req.name.trim().to_string();
    let template_trimmed = req.template.trim().to_string();

    if name_trimmed.is_empty() || template_trimmed.is_empty() {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .body(Body::from(serde_json::json!({ "success": false, "message": "模板名称和内容不能为空" }).to_string()))
            .unwrap();
    }

    let result = tokio::task::spawn_blocking(move || {
        let conn = rusqlite::Connection::open(crate::db::get_db_cache_path())
            .map_err(|e| e.to_string())?;

        let template_id = format!("template_{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0));
        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO prompt_templates (id, name, description, template, is_builtin, created_at, updated_at)
             VALUES (?, ?, ?, ?, 0, ?, ?)",
            rusqlite::params![template_id, name_trimmed, req.description, template_trimmed, now, now],
        ).map_err(|e| e.to_string())?;

        Ok::<String, String>(template_id)
    }).await;

    match result {
        Ok(Ok(id)) => {
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from(serde_json::json!({ "success": true, "id": id }).to_string()))
                .unwrap()
        }
        _ => {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from(serde_json::json!({ "success": false, "message": "创建模板失败" }).to_string()))
                .unwrap()
        }
    }
}

/// PUT /api/review/prompt_templates/:id
pub async fn handle_update_prompt_template(
    axum::extract::Path(id): axum::extract::Path<String>,
    axum::Json(req): axum::Json<UpdatePromptTemplateReq>,
) -> impl IntoResponse {
    let name_trimmed = req.name.trim().to_string();
    let template_trimmed = req.template.trim().to_string();

    if name_trimmed.is_empty() || template_trimmed.is_empty() {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .body(Body::from(serde_json::json!({ "success": false, "message": "模板名称和内容不能为空" }).to_string()))
            .unwrap();
    }

    let result = tokio::task::spawn_blocking(move || {
        let conn = rusqlite::Connection::open(crate::db::get_db_cache_path())
            .map_err(|e| e.to_string())?;

        // 校验是否是内置模板
        let is_builtin: i32 = conn.query_row(
            "SELECT is_builtin FROM prompt_templates WHERE id = ?",
            [&id],
            |row| row.get(0),
        ).map_err(|e| format!("模板不存在: {}", e))?;

        if is_builtin == 1 {
            return Err("系统内置模板不支持直接修改".to_string());
        }

        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE prompt_templates 
             SET name = ?, description = ?, template = ?, updated_at = ?
             WHERE id = ?",
            rusqlite::params![name_trimmed, req.description, template_trimmed, now, id],
        ).map_err(|e| e.to_string())?;

        Ok::<(), String>(())
    }).await;

    match result {
        Ok(Ok(())) => {
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from(serde_json::json!({ "success": true }).to_string()))
                .unwrap()
        }
        Ok(Err(e)) => {
            Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from(serde_json::json!({ "success": false, "message": e }).to_string()))
                .unwrap()
        }
        _ => {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from(serde_json::json!({ "success": false, "message": "修改模板失败" }).to_string()))
                .unwrap()
        }
    }
}

/// DELETE /api/review/prompt_templates/:id
pub async fn handle_delete_prompt_template(
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let conn = rusqlite::Connection::open(crate::db::get_db_cache_path())
            .map_err(|e| e.to_string())?;

        // 校验是否是内置模板
        let is_builtin: i32 = conn.query_row(
            "SELECT is_builtin FROM prompt_templates WHERE id = ?",
            [&id],
            |row| row.get(0),
        ).map_err(|e| format!("模板不存在: {}", e))?;

        if is_builtin == 1 {
            return Err("系统内置模板不支持直接删除".to_string());
        }

        conn.execute(
            "DELETE FROM prompt_templates WHERE id = ?",
            [&id],
        ).map_err(|e| e.to_string())?;

        Ok::<(), String>(())
    }).await;

    match result {
        Ok(Ok(())) => {
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from(serde_json::json!({ "success": true }).to_string()))
                .unwrap()
        }
        Ok(Err(e)) => {
            Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from(serde_json::json!({ "success": false, "message": e }).to_string()))
                .unwrap()
        }
        _ => {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from(serde_json::json!({ "success": false, "message": "删除模板失败" }).to_string()))
                .unwrap()
        }
    }
}

// ============================================================
// CLI 连通性测试接口
// ============================================================

#[derive(Debug, Deserialize)]
pub struct TestCliRequest {
    pub bin: String,
}

#[derive(Debug, Serialize)]
pub struct TestCliResponse {
    pub ok: bool,
    pub latency_ms: u64,
    pub sample: Option<String>,
    pub stdout: String,
    pub stderr: String,
    pub detail: String,
}

/// POST /api/review/test-cli
/// 对指定的 CLI 引擎发送一个 Smoke Prompt，验证其连通性与可调用性
/// 超时时间 45 秒，与 open-design 保持一致
pub async fn handle_test_cli(
    axum::Json(req): axum::Json<TestCliRequest>,
) -> Response<Body> {
    let bin = req.bin.trim().to_string();
    if bin.is_empty() {
        let body = serde_json::to_vec(&TestCliResponse {
            ok: false,
            latency_ms: 0,
            sample: None,
            stdout: String::new(),
            stderr: String::new(),
            detail: "bin 参数不能为空".to_string(),
        }).unwrap_or_default();
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .body(Body::from(body))
            .unwrap();
    }

    let start = std::time::Instant::now();

    // 查找可执行文件（会优先使用自定义 BIN 路径）
    let exe_path = match find_cli_in_path(&bin) {
        Some(p) => p,
        None => {
            let body = serde_json::to_vec(&TestCliResponse {
                ok: false,
                latency_ms: start.elapsed().as_millis() as u64,
                sample: None,
                stdout: String::new(),
                stderr: String::new(),
                detail: format!("未找到 '{}' 可执行文件，请检查安装或配置自定义路径", bin),
            }).unwrap_or_default();
            return Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from(body))
                .unwrap();
        }
    };

    // Smoke Prompt — 与 open-design 保持一致
    let smoke_prompt = "Reply with only: ok\n";

    // 构建子进程
    let mut cmd = Command::new(&exe_path);

    // 注入该 CLI 的自定义环境变量（排除 _BIN 路径键）
    {
        let app_cfg = crate::config::load_config();
        if let Some(agent_env) = app_cfg.agent_cli_env.get(&bin) {
            let bin_key = format!("{}_BIN", bin.replace('-', "_").to_uppercase());
            for (k, v) in agent_env {
                if k != &bin_key {
                    cmd.env(k, v);
                }
            }
        }
    }

    // 根据 CLI 类型配置启动参数（仅保留 stdin 接收 prompt 的非交互参数）
    if bin.starts_with("claude") {
        cmd.args(["-p", "--output-format", "text", "--permission-mode", "bypassPermissions"]);
    } else if bin.starts_with("codex") {
        #[cfg(target_os = "windows")]
        cmd.args(["exec", "--skip-git-repo-check", "--sandbox", "danger-full-access"]);
        #[cfg(not(target_os = "windows"))]
        cmd.args(["exec", "--skip-git-repo-check", "--sandbox", "workspace-write"]);
    } else if bin.starts_with("gemini") {
        cmd.args(["--yolo"]);
    } else if bin.starts_with("agy") {
        // agy 无需额外参数，通过 stdin 接收
    } else {
        cmd.arg("-p");
    }

    cmd.stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            let body = serde_json::to_vec(&TestCliResponse {
                ok: false,
                latency_ms: start.elapsed().as_millis() as u64,
                sample: None,
                stdout: String::new(),
                stderr: String::new(),
                detail: format!("启动 '{}' 进程失败: {}", bin, e),
            }).unwrap_or_default();
            return Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from(body))
                .unwrap();
        }
    };

    // 写入 Smoke Prompt 并关闭 stdin
    if let Some(mut stdin_pipe) = child.stdin.take() {
        let _ = stdin_pipe.write_all(smoke_prompt.as_bytes()).await;
        drop(stdin_pipe);
    }

    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();

    // 45 秒超时（与 open-design 一致）
    let timeout_result = tokio::time::timeout(Duration::from_secs(45), async {
        let mut stdout_buf = String::new();
        let mut stderr_buf = String::new();

        // 并发读取 stdout 和 stderr
        let (r_tx, mut r_rx) = tokio::sync::mpsc::channel::<ReaderMsg>(200);

        if let Some(pipe) = stdout_pipe {
            let tx2 = r_tx.clone();
            tokio::spawn(async move {
                let mut reader = BufReader::new(pipe).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let _ = tx2.send(ReaderMsg::Stdout(line)).await;
                }
            });
        }
        if let Some(pipe) = stderr_pipe {
            let tx2 = r_tx;
            tokio::spawn(async move {
                let mut reader = BufReader::new(pipe).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let _ = tx2.send(ReaderMsg::Stderr(line)).await;
                }
            });
        }

        while let Some(msg) = r_rx.recv().await {
            match msg {
                ReaderMsg::Stdout(line) => {
                    stdout_buf.push_str(&line);
                    stdout_buf.push('\n');
                }
                ReaderMsg::Stderr(line) => {
                    if stderr_buf.len() < 4000 {
                        stderr_buf.push_str(&line);
                        stderr_buf.push('\n');
                    }
                }
            }
        }

        let exit_status = tokio::time::timeout(Duration::from_secs(5), child.wait()).await
            .ok()
            .and_then(|r| r.ok());
        let exit_code = exit_status.and_then(|s| s.code());
        let exit_ok = exit_status.map(|s| s.success()).unwrap_or(false);

        (stdout_buf, stderr_buf, exit_code, exit_ok)
    }).await;

    let latency_ms = start.elapsed().as_millis() as u64;

    let response = match timeout_result {
        Err(_) => {
            // 45 秒超时
            TestCliResponse {
                ok: false,
                latency_ms,
                sample: None,
                stdout: String::new(),
                stderr: String::new(),
                detail: format!("连通性测试超时 (>45s)：'{}' 引擎在规定时间内没有响应", bin),
            }
        }
        Ok((stdout, stderr, _exit_code, exit_ok)) => {
            let trimmed = stdout.trim().to_string();
            let sample: Option<String> = if !trimmed.is_empty() {
                // 截取最多 120 字符作为样本
                Some(trimmed.chars().take(120).collect())
            } else {
                None
            };

            if exit_ok || !trimmed.is_empty() {
                TestCliResponse {
                    ok: true,
                    latency_ms,
                    sample,
                    stdout: trimmed,
                    stderr: stderr.trim().to_string(),
                    detail: format!("连通性测试成功 ({:.1}s)", latency_ms as f64 / 1000.0),
                }
            } else {
                let stderr_trim = stderr.trim().to_string();
                TestCliResponse {
                    ok: false,
                    latency_ms,
                    sample: None,
                    stdout: trimmed,
                    stderr: stderr_trim.clone(),
                    detail: if !stderr_trim.is_empty() {
                        format!("'{}' 引擎异常退出: {}", bin, &stderr_trim.chars().take(200).collect::<String>())
                    } else {
                        format!("'{}' 引擎异常退出，无输出", bin)
                    },
                }
            }
        }
    };

    let body = serde_json::to_vec(&response).unwrap_or_default();
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(Body::from(body))
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    #[tokio::test]
    async fn test_well_known_dirs() {
        let dirs = get_well_known_toolchain_dirs();
        // 验证过滤存在性后，返回的全部是真实存在的目录
        for d in &dirs {
            assert!(d.exists());
            assert!(d.is_dir());
        }
    }

    #[tokio::test]
    async fn test_probe_non_existent_cli() {
        let info = probe_cli("non_existent_crazy_cli_tool_xxx").await;
        assert_eq!(info.available, false);
        assert_eq!(info.version, None);
        assert_eq!(info.path, None);
    }

    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn test_probe_broken_cmd_shim() {
        let temp_dir = std::env::temp_dir();
        let shim_path = temp_dir.join("test_broken_ghost_cli.cmd");
        
        // 模拟一个损坏的 npm 批处理代理
        // 写入中英文混合的找不到指定路径字眼，以防在非中文 Windows 系统下匹配失败
        let mut file = File::create(&shim_path).unwrap();
        writeln!(file, "@echo off\necho 系统找不到指定的路径。 >&2\necho The system cannot find the path specified. >&2\nexit /b 1").unwrap();
        drop(file);

        let path_str = shim_path.to_string_lossy().to_string();
        let info = probe_cli(&path_str).await;
        
        // 清理
        let _ = std::fs::remove_file(&shim_path);

        // 验证它必须被判定为 available: false 
        assert_eq!(info.available, false);
        assert_eq!(info.version, None);
    }

    #[cfg(not(target_os = "windows"))]
    #[tokio::test]
    async fn test_probe_broken_unix_shim() {
        let temp_dir = std::env::temp_dir();
        let shim_path = temp_dir.join("test_broken_ghost_cli");
        
        // 模拟 Unix 下返回 127/126 的不可用 shim
        let mut file = File::create(&shim_path).unwrap();
        writeln!(file, "#!/bin/sh\nexit 127").unwrap();
        drop(file);

        // 赋予执行权限
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&shim_path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        let path_str = shim_path.to_string_lossy().to_string();
        let info = probe_cli(&path_str).await;

        // 清理
        let _ = std::fs::remove_file(&shim_path);

        // 验证必须被判定为 available: false
        assert_eq!(info.available, false);
        assert_eq!(info.version, None);
    }
}



