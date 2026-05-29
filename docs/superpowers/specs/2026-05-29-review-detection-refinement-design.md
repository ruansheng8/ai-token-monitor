# 2026-05-29-分析引擎 CLI 缓存、复盘今日维度与 IDE 数据源同步设计文档

该设计文档描述了如何解决 AI Token Monitor 在运行分析引擎 CLI 检测时的性能卡顿问题，并在复盘分析中增加“今日”统计维度，以及统一复盘界面中的 IDE 数据源口径。

## 1. 运行分析引擎 CLI 缓存机制

### 当前问题
当前每次打开复盘抽屉或刷新页面时，系统都会调用后端的 `/api/review/detect` 接口。该接口会启动多个 CLI 子进程（`claude`、`codex`、`gemini`）去执行 `--version` 探测。这在 Windows 环境下耗时较长（每次通常需要 3~10 秒），引起界面明显卡顿。

### 缓存方案
为避免每次重复探测，我们在后端引入 **24 小时本地文件缓存**：
1. **缓存文件路径**：`%USERPROFILE%/.ai_token_monitor/cli_detect_cache.json`。
2. **缓存数据结构**：
   ```rust
   #[derive(Debug, Serialize, Deserialize, Clone)]
   pub struct CliDetectCache {
       pub detected_at: String, // ISO 8601 / RFC 3339 格式的 UTC 时间戳
       pub tools: Vec<CliToolInfo>,
       pub recommended: Option<String>,
   }
   ```
3. **接口逻辑优化** (`handle_review_detect`):
   - 支持通过 Query 接收 `force: Option<bool>` 参数。
   - 若 `force` 不为 `Some(true)`，且缓存文件存在、可解析，并且其记录的 `detected_at` 距当前时间小于 24 小时，则直接读取缓存内容并返回，耗时近乎为 0。
   - 若不满足上述条件，执行真实的 `probe_cli`，并在执行成功后将最新的探测时间与结果写入缓存文件。

4. **前端交互改进**：
   - 打开抽屉时：调用不带 `force` 参数的 `/api/review/detect`。
   - 用户主动点击“刷新检测”按钮时：调用 `/api/review/detect?force=true` 强行触发真实探测并更新缓存。

## 2. 增加“今日”复盘维度

### 方案设计
1. **前端时间选项扩展**：
   在 [ReviewDrawer.tsx](file:///d:/VibeCoding/ai-token-monitor/src/components/ReviewDrawer.tsx) 的 `TIME_RANGE_OPTIONS` 中添加今日选项：
   ```typescript
   { label: '今日', value: '今日' }
   ```
2. **时区与本地时间口径对齐**：
   重构 `getReviewDateBounds` 函数，弃用 UTC 时间格式化，改用本地时间：
   ```typescript
   function getReviewDateBounds(range: string) {
     const end = new Date();
     const start = new Date();
     
     const format = (d: Date) => {
       const y = d.getFullYear();
       const m = String(d.getMonth() + 1).padStart(2, '0');
       const day = String(d.getDate()).padStart(2, '0');
       return `${y}-${m}-${day}`;
     };

     if (range === '今日') {
       // start 保持为今天
     } else if (range === '7天') {
       start.setDate(end.getDate() - 7);
     } else if (range === '30天') {
       start.setDate(end.getDate() - 30);
     } else {
       start.setFullYear(end.getFullYear() - 5);
     }

     return { start: format(start), end: format(end) };
   }
   ```
3. **模板语义化处理**：
   当 `reviewTimeRange` 为 `'今日'` 时，自动将报告模版占位符中的 `'最近7天'` 替换为 `'今日'`，使其生成出的 Prompt 更为自然合理。

## 3. 同步“关联分析 IDE 数据源”

### 方案设计
当前复盘所能勾选的 IDE 选项与首页不一致，需完全同步。
1. **前端数据源同步**：
   更新 `ReviewDrawer.tsx` 中的 `IDE_OPTIONS`：
   ```typescript
   const IDE_OPTIONS = [
     { label: '全部工具 (All)', value: 'all' },
     { label: 'Antigravity', value: 'antigravity' },
     { label: 'Claude Code', value: 'claude_code' },
     { label: 'Codex CLI', value: 'codex' },
     { label: 'Cursor', value: 'cursor' },
     { label: 'Trae', value: 'trae' },
     { label: 'Trae CN', value: 'trae_cn' },
   ];
   ```
2. **前后端工具名称映射**：
   更新前后端中的 `buildPromptFromTemplate`：
   - 前端：
     ```typescript
     function buildPromptFromTemplate(template: string, ides: string[]): string {
       let ide_display = '';
       if (ides.includes('all') || ides.length === 0) {
         ide_display = '全部工具 (Antigravity、Claude Code、Codex CLI、Cursor、Trae、Trae CN)';
       } else {
         const mapped = ides.map((s) => {
           switch (s) {
             case 'antigravity': return 'Antigravity';
             case 'claude_code': return 'Claude Code';
             case 'codex': return 'Codex CLI';
             case 'cursor': return 'Cursor';
             case 'trae': return 'Trae';
             case 'trae_cn': return 'Trae CN';
             default: return s;
           }
         });
         ide_display = mapped.join('、');
       }
       return template.replace('{{IDE}}', ide_display);
     }
     ```
   - 后端 (Rust):
     ```rust
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
     ```
