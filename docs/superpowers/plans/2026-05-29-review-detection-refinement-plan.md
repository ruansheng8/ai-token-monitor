# 2026-05-29-分析引擎 CLI 缓存、复盘今日维度与 IDE 数据源同步实现计划

该实现计划列出了为 AI Token Monitor 完成 CLI 检测缓存、增加今日复盘维度以及同步 IDE 数据源的具体代码修改步骤。

## 1. 待修改的文件与步骤

### 后端部分 (Rust)

#### [MODIFY] [review.rs](file:///d:/VibeCoding/ai-token-monitor/src-tauri/src/review.rs)
1. **添加数据结构 `CliDetectCache`**，用于缓存 CLI 的检测结果：
   ```rust
   #[derive(Debug, Serialize, Deserialize, Clone)]
   pub struct CliDetectCache {
       pub detected_at: String,
       pub tools: Vec<CliToolInfo>,
       pub recommended: Option<String>,
   }
   ```
2. **修改 `handle_review_detect` 接口**：
   - 导入必要的依赖，如 `std::collections::HashMap` 以及用于提取 Query 参数的 `axum::extract::Query`。
   - 解析 `force` 查询参数，默认值为 `false`。
   - 定义缓存文件位置在 `%USERPROFILE%/.ai_token_monitor/cli_detect_cache.json`。
   - 在非 `force` 情况下，尝试读取缓存文件并解析。如果缓存内的 `detected_at` 距当前时间小于 24 小时，直接序列化返回缓存结果。
   - 若缓存不可用或为 `force` 请求，运行真实的探测逻辑 `probe_cli`，并将成功生成的结果以及当前时间写入缓存文件中。
3. **更新 `buildPromptFromTemplate` 函数**，将 `all` 和各种工具映射为和首页完全一致的中文名：
   - `antigravity` -> `Antigravity`
   - `claude_code` -> `Claude Code`
   - `codex` -> `Codex CLI`
   - `cursor` -> `Cursor`
   - `trae` -> `Trae`
   - `trae_cn` -> `Trae CN`

---

### 前端部分 (React & TypeScript)

#### [MODIFY] [ReviewDrawer.tsx](file:///d:/VibeCoding/ai-token-monitor/src/components/ReviewDrawer.tsx)
1. **更新 `TIME_RANGE_OPTIONS`**，加入“今日”选项：
   ```typescript
   { label: '今日', value: '今日' }
   ```
2. **更新 `IDE_OPTIONS`**，保持与首页数据源下拉框的项完全一致。
3. **重构 `getReviewDateBounds`**，支持 `range === '今日'`，且弃用 UTC 转换（`toISOString`），改用本地时间获取 `YYYY-MM-DD`（避免白天因为时差查不出今日数据）。
4. **修改 `buildPromptFromTemplate`** 前端映射，使其与后端 Rust 的映射命名保持对齐（包括 `all` 展示）。
5. **在模版提示词替换 `useEffect` 中优化 `timeLabel`**：
   ```typescript
   const timeLabel = reviewTimeRange === '今日' ? '今日' : `最近${reviewTimeRange}`;
   const templateWithTime = selectedPreset.template.replace('最近7天', timeLabel);
   ```
6. **修改 `detectCliTools`**，让其接收 `force` 可选参数，请求 API 时若为 force 则请求 `/api/review/detect?force=true`。
7. **修改刷新按钮事件**，改为 `detectCliTools(true)` 强刷。页面初次加载时打开抽屉则调用 `detectCliTools()` (使用缓存)。

---

## 2. 验证方案

### 自动化编译与类型校验
在 Windows PowerShell 中执行以下命令：
```powershell
# 后端编译与语法检查
cd src-tauri; cargo check; cd ..

# 前端 TypeScript 与 Lint 校验
npx tsc -b --noEmit
npm run lint
```

### 手动验证步骤
1. **CLI 检测缓存与强刷验证**：
   - 首次打开复盘抽屉时，观察后端响应和控制台，第一次加载需要探测物理 CLI。
   - 关闭复盘抽屉重新打开，由于有了本地缓存，抽屉瞬间加载完毕。
   - 点击复盘界面中 CLI 后方的“重新检测”刷新按钮，按钮正常 Spin 旋转，触发了 `force=true` 强刷，并成功更新缓存文件。
2. **今日维度与指标快照验证**：
   - 切换时间区间到“今日”，观察快照度量卡片以及模版提示词预览区域。
   - 模版提示词中应正确替换为“今日”，而不是“最近今日”。
   - 查看抓取到的数据指标是否准确，并且与首页选择“今日”时的总 Token 消耗等数据完全吻合。
3. **IDE 数据源同步验证**：
   - 查看“关联分析 IDE 数据源”中的工具是否包含 “Antigravity、Claude Code、Codex CLI、Cursor、Trae、Trae CN”。
   - 勾选特定工具（如 `trae_cn`）或点击全部，生成的复盘 Prompt 模版中的 `{{IDE}}` 占位符被正确解析为对应的中文名称。
