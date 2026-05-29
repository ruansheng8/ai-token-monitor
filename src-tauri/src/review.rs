/// review.rs — 「使用复盘与建议」功能
///
/// 参考 open-design 项目的 claude-code-integration.md，采用简化的一次性分析模式：
///   - 通过 tokio::process::Command 启动宿主机已安装的 claude/codex CLI
///   - Prompt 经由 stdin 写入（规避 argv 长度限制）
///   - CLI 输出通过 axum SSE 实时推流给前端
///   - 不实现双向 stream-json 通道（复盘场景无需 AskUserQuestion 交互）

use axum::{
    body::Body,
    http::{header, Response, StatusCode},
    response::{
        sse::{Event, Sse},
        IntoResponse,
    },
};
use serde::{Deserialize, Serialize};
use std::{convert::Infallible, path::PathBuf, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
};
use tokio_stream::wrappers::ReceiverStream;

// ============================================================
// 数据结构
// ============================================================

#[derive(Debug, Serialize, Deserialize)]
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

// 从前端传入的指标摘要（简化版，只取复盘需要的关键字段）
#[derive(Debug, Deserialize)]
pub struct ReviewRequest {
    /// 时间范围标签（例如 "30days", "week", "all"）
    pub time_range: Option<String>,
    /// 总 token 数
    pub total_tokens: Option<i64>,
    /// 总费用（USD）
    pub total_cost_usd: Option<f64>,
    /// 总会话数
    pub total_sessions: Option<i64>,
    /// 缓存命中率（0.0~1.0）
    pub cache_hit_rate: Option<f64>,
    /// 推理 token 占比（0.0~1.0）
    pub thinking_ratio: Option<f64>,
    /// 各工具来源占比（JSON 字符串，例如 [{"source":"claude_code","tokens":12345}]）
    pub source_breakdown: Option<String>,
    /// 模型分布（JSON 字符串）
    pub model_distribution: Option<String>,
    /// 日均趋势（JSON 字符串）
    pub daily_trend_summary: Option<String>,
    /// 前端当前选择的 AI 工具（用于决定调用哪个 CLI）
    pub preferred_cli: Option<String>,
    /// 用户自定义提示词（如果非空，优先使用，忽略自动构建的 prompt）
    pub custom_prompt: Option<String>,
    /// 用户选择的 IDE 来源列表（逗号分隔，例如 "claude_code,cursor"）
    pub selected_ides: Option<String>,
}

// ============================================================
// CLI 检测
// ============================================================

/// 在 PATH 中查找可执行文件，Windows 下额外尝试 .cmd 和 .exe 后缀
fn find_cli_in_path(bin: &str) -> Option<PathBuf> {
    // 在 PATH 中直接查找（Unix 风格）
    if let Ok(path) = which::which(bin) {
        return Some(path);
    }

    // Windows 下补全后缀再试
    #[cfg(target_os = "windows")]
    {
        for suffix in &[".cmd", ".exe", ".bat"] {
            let name = format!("{}{}", bin, suffix);
            if let Ok(path) = which::which(&name) {
                return Some(path);
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

    // 尝试获取版本号（claude --version / codex --version）
    let version = tokio::time::timeout(
        Duration::from_secs(5),
        Command::new(&exe_path).arg("--version").output(),
    )
    .await
    .ok()
    .and_then(|r| r.ok())
    .and_then(|out| {
        let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let combined = if !stdout.is_empty() { stdout } else { stderr };
        if combined.is_empty() {
            None
        } else {
            // 取第一行，避免过长
            Some(combined.lines().next().unwrap_or("").to_string())
        }
    });

    CliToolInfo {
        name: bin.to_string(),
        available: true,
        version,
        path: Some(path_str),
    }
}

// ============================================================
// HTTP 路由处理器
// ============================================================

/// GET /api/review/detect
/// 检测宿主机已安装的 AI CLI 工具，返回可用列表
pub async fn handle_review_detect() -> Response<Body> {
    let candidate_bins = ["claude", "codex", "gemini"];

    let mut tools = Vec::new();
    for bin in &candidate_bins {
        tools.push(probe_cli(bin).await);
    }

    // 推荐优先级：claude > codex > gemini
    let recommended = tools
        .iter()
        .find(|t| t.available)
        .map(|t| t.name.clone());

    let resp = DetectResponse { tools, recommended };

    let body_bytes = serde_json::to_vec(&resp).unwrap_or_default();
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(Body::from(body_bytes))
        .unwrap()
}

/// GET /api/review/stream?cli=claude&...（其余 query 参数为指标数据）
/// 启动 CLI 子进程分析，通过 SSE 推流输出
pub async fn handle_review_stream(
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    // 从 query 参数中读取指标数据
    let preferred_cli = params
        .get("cli")
        .cloned()
        .unwrap_or_else(|| "claude".to_string());

    let time_range = params
        .get("time_range")
        .cloned()
        .unwrap_or_else(|| "30天".to_string());

    let req = ReviewRequest {
        time_range: Some(time_range.clone()),
        total_tokens: params
            .get("total_tokens")
            .and_then(|s| s.parse().ok()),
        total_cost_usd: params
            .get("total_cost_usd")
            .and_then(|s| s.parse().ok()),
        total_sessions: params
            .get("total_sessions")
            .and_then(|s| s.parse().ok()),
        cache_hit_rate: params
            .get("cache_hit_rate")
            .and_then(|s| s.parse().ok()),
        thinking_ratio: params
            .get("thinking_ratio")
            .and_then(|s| s.parse().ok()),
        source_breakdown: params.get("source_breakdown").cloned(),
        model_distribution: params.get("model_distribution").cloned(),
        daily_trend_summary: params.get("daily_trend_summary").cloned(),
        preferred_cli: Some(preferred_cli.clone()),
        custom_prompt: params.get("custom_prompt").cloned(),
        selected_ides: params.get("selected_ides").cloned(),
    };

    // 选择可用的 CLI
    let cli_path = find_cli_in_path(&preferred_cli)
        .or_else(|| find_cli_in_path("claude"))
        .or_else(|| find_cli_in_path("codex"));

    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(64);

    // 如果没有找到任何 CLI，直接推送错误事件
    if cli_path.is_none() {
        let tx_clone = tx.clone();
        tokio::spawn(async move {
            let err_msg = "❌ **未检测到 Claude Code / Codex CLI**\n\n请先安装 Claude Code：\n```\nnpm install -g @anthropic-ai/claude-code\n```\n安装完成后刷新页面重试。";
            let _ = tx_clone
                .send(Ok(Event::default().data(err_msg)))
                .await;
            let _ = tx_clone.send(Ok(Event::default().event("done").data("[DONE]"))).await;
        });
        let stream = ReceiverStream::new(rx);
        return Sse::new(stream)
            .keep_alive(axum::response::sse::KeepAlive::new().interval(Duration::from_secs(15)));
    }

    let cli_path = cli_path.unwrap();
    // 如果前端传入了自定义提示词，优先使用；否则自动构建
    let prompt = if let Some(ref cp) = req.custom_prompt {
        if !cp.trim().is_empty() {
            cp.clone()
        } else {
            build_review_prompt(&req)
        }
    } else {
        build_review_prompt(&req)
    };

    // 在后台任务中运行 CLI 并推流
    tokio::spawn(async move {
        if let Err(e) = run_cli_and_stream(cli_path, prompt, tx.clone()).await {
            let err_str = format!("❌ 分析过程中出现错误: {}", e);
            let _ = tx.send(Ok(Event::default().data(err_str))).await;
        }
        let _ = tx.send(Ok(Event::default().event("done").data("[DONE]"))).await;
    });

    let stream = ReceiverStream::new(rx);
    Sse::new(stream)
        .keep_alive(axum::response::sse::KeepAlive::new().interval(Duration::from_secs(15)))
}

// ============================================================
// 核心：spawn CLI 子进程 + SSE 推流
// ============================================================

async fn run_cli_and_stream(
    cli_path: PathBuf,
    prompt: String,
    tx: tokio::sync::mpsc::Sender<Result<Event, Infallible>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // 参考 open-design 的 buildArgs 策略：
    //   claude -p --output-format text --permission-mode bypassPermissions
    // stdin 写入 prompt（规避 argv 32KB 长度限制）
    let cli_name = cli_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("claude");

    let mut cmd = Command::new(&cli_path);

    // 根据 CLI 类型构建参数（参考 open-design claudeAgentDef.buildArgs）
    if cli_name.starts_with("claude") {
        cmd.args([
            "-p",
            "--output-format",
            "text",
            "--permission-mode",
            "bypassPermissions",
        ]);
    } else if cli_name.starts_with("codex") {
        // codex 使用 --full-auto 非交互模式
        cmd.args(["--full-auto", "-q"]);
    } else {
        // 其他 CLI：尝试通用的 -p 参数
        cmd.arg("-p");
    }

    cmd.stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn()?;

    // 向 stdin 写入 prompt（参考 open-design § 3 "promptViaStdin: true"）
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(prompt.as_bytes()).await?;
        // 写完后立即 drop → EOF，告知 CLI 输入结束
        // （复盘场景无需保持 stdin 打开，不同于 open-design 的 AskUserQuestion 模式）
        drop(stdin);
    }

    // 读取 stdout，逐行推送 SSE 事件（流式打字机效果）
    if let Some(stdout) = child.stdout.take() {
        let mut reader = BufReader::new(stdout).lines();
        let mut buffer = String::new();

        loop {
            match tokio::time::timeout(Duration::from_secs(120), reader.next_line()).await {
                Ok(Ok(Some(line))) => {
                    // 把每行内容追加到 buffer 并推 SSE
                    if !line.is_empty() {
                        buffer.push_str(&line);
                        buffer.push('\n');
                        // 每积累一定量或遇到段落分隔就推送（提升实时感）
                        if buffer.len() >= 50 || line.is_empty() {
                            let chunk = buffer.clone();
                            buffer.clear();
                            if tx.send(Ok(Event::default().data(chunk))).await.is_err() {
                                break;
                            }
                        }
                    } else {
                        // 空行：推送换行保留段落结构
                        if !buffer.is_empty() {
                            let chunk = buffer.clone();
                            buffer.clear();
                            let _ = tx.send(Ok(Event::default().data(chunk))).await;
                        }
                        let _ = tx.send(Ok(Event::default().data("\n"))).await;
                    }
                }
                Ok(Ok(None)) => {
                    // stdout EOF
                    if !buffer.is_empty() {
                        let _ = tx.send(Ok(Event::default().data(buffer))).await;
                    }
                    break;
                }
                Ok(Err(e)) => {
                    return Err(Box::new(e));
                }
                Err(_) => {
                    // 超时
                    return Err("CLI 分析超时（120秒），请检查 claude 是否已登录".into());
                }
            }
        }
    }

    // 等待子进程退出，捕获非零退出码
    let status = child.wait().await?;
    if !status.success() {
        let code = status.code().unwrap_or(-1);
        if code != 0 {
            // 尝试读取 stderr 获取错误信息
            return Err(format!("CLI 异常退出 (code={})", code).into());
        }
    }

    Ok(())
}

// ============================================================
// Prompt 工程
// ============================================================

/// 将指标数据序列化为结构化 Prompt，交给 Claude Code 分析
fn build_review_prompt(req: &ReviewRequest) -> String {
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

    format!(
        r#"你是一位专业的 AI 工具使用顾问。我使用 AI Token Monitor 追踪了我在 Claude Code、Codex、Cursor 等工具上的 Token 消耗情况。

请根据下方我的使用数据，为我提供一份**深度使用复盘报告**，用中文回答。

---

## 我的使用数据（{time_label}）

| 指标 | 数值 |
|------|------|
| 总 Token 消耗 | {total_tokens_fmt} tokens |
| 总费用 | ${total_cost:.4} USD |
| 总会话数 | {total_sessions} 次 |
| 缓存命中率 | {cache_pct:.1}% |
| 推理（Thinking）Token 占比 | {thinking_pct:.1}% |
{source_section}{model_section}{trend_section}

---

## 请按以下结构输出分析报告（使用 Markdown 格式）：

### 1. 使用模式诊断
分析我的 AI 工具使用习惯，包括：
- 主要使用哪些工具/模型？
- 使用频率是否均匀，有无明显的高峰/低谷？
- 缓存命中率 {cache_pct:.1}% 是否合理？（业界参考：>30% 较好）
- 推理 Token 占比 {thinking_pct:.1}% 说明什么？

### 2. 成本优化建议
基于以上数据，给出 3~5 条具体、可操作的成本优化建议，例如：
- 哪些场景可以换用更便宜的模型？
- 如何提升缓存命中率？
- 是否存在明显的低效会话模式？

### 3. 效率评估
- 综合评价我的 AI 使用效率（满分100分，给出评分和理由）
- 与一般开发者的平均水平相比，我的数据表现如何？

### 4. 本周行动清单
列出 3 条我这周可以立刻执行的具体优化行动（要具体到操作步骤，不要泛泛而谈）。

---

请直接开始输出报告，不需要前言。保持语言简洁专业，使用 Markdown 格式。
"#,
        time_label = time_label,
        total_tokens_fmt = total_tokens_fmt,
        total_cost = total_cost,
        total_sessions = total_sessions,
        cache_pct = cache_rate * 100.0,
        thinking_pct = thinking_ratio * 100.0,
        source_section = source_section,
        model_section = model_section,
        trend_section = trend_section,
    )

}
