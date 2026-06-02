# 本机 CLI 连通性测试与自定义环境配置设计规格书

本文档定义了在 `token-insight` 项目中集成 `open-design` 的 19 个本地 CLI 引擎、自定义环境变量配置、二进制覆盖路径及引擎连通性测试 (CLI Connection Test) 功能的规格设计。

## 1. 后端修改方案 (Rust / Axum)

### A. 全局配置 AppConfig 扩展
在 `src-tauri/src/config.rs` 中：
- `AppConfig` 结构体新增 `agent_cli_env` 字段，存储为一个嵌套的 HashMap：
  ```rust
  #[serde(default)]
  pub agent_cli_env: std::collections::HashMap<String, std::collections::HashMap<String, String>>,
  ```
- 完善其在 `Default` 实现中的默认值。
- 在 `src-tauri/src/server.rs` 的 `ConfigReq` 中同步追加该字段的可选序列化支持，在 `handle_config_get` 和 `handle_config_save` 中打通该字段的加载与持久化。

### B. 扩展 19 个主流 CLI 引擎探测
在 `src-tauri/src/review.rs` 中：
- 扩展 `candidate_bins` 支持的引擎：
  `["claude", "codex", "gemini", "agy", "cursor-agent", "opencode", "qwen", "copilot", "devin", "kimi", "qoder", "pi", "kiro", "kilo", "vibe", "deepseek", "hermes", "grok-build", "reasonix", "aider"]`。
- 修改 `find_cli_in_path(bin)`：优先检测 `AppConfig` 的 `agent_cli_env` 中是否配置了对应的二进制路径覆盖（键名格式为 `[BIN_UPPERCASE]_BIN`，例如 `CODEX_BIN`）。若存在且文件有效则直接返回该路径；否则退回到系统 PATH 和默认的 well-known 目录查找。

### C. 注入子进程运行环境变量
在拉起分析任务（`run_cli_task_background`）及执行测试（`handle_test_cli`）时：
- 从 `AppConfig` 的 `agent_cli_env` 中取得当前引擎的所有自定义环境变量（排除 `_BIN` 路径键），通过 `cmd.env(key, val)` 注入到子进程的 Command 环境中。

### D. 新增连通性测试 API
- 注册路由：`POST /api/review/test-cli`
- Handler 逻辑：
  1. 读取传入的 `bin` 标识。
  2. 寻找其可执行路径，不存在则返回错误。
  3. 为不同引擎配置其 Smoke 测试参数：
     - `claude` -> `["-p", "--output-format", "text", "--permission-mode", "bypassPermissions"]`
     - `codex` (Windows) -> `["exec", "--skip-git-repo-check", "--sandbox", "danger-full-access"]`
     - `codex` (Unix) -> `["exec", "--skip-git-repo-check", "--sandbox", "workspace-write"]`
     - `gemini` -> `["-p", "--yolo"]`
     - 其它 -> `["-p"]` 或空
  4. 拉起进程，通过 stdin 管道写入 Smoke Prompt：`"Reply with only: ok"`。
  5. 超时时间设定为 **45 秒**。
  6. 收集 stdout/stderr 并判定退出状态码。若有实质输出且退出码为 0，判定为测试成功，提取响应样本；否则收集详细错误日志返回。

---

## 2. 前端修改方案 (React / TypeScript)

### A. 复盘抽屉 (ReviewDrawer.tsx) 轻量化限流展示
- 对 `detectResult.tools` 中 `available: true` 的可用 CLI，默认仅截取前 4 个展示于复盘引擎选择器中。

### B. 新增“配置 CLI”按钮与配置弹窗 (Modal)
- 在复盘抽屉的“重新检测”左侧，新增 `🔧 配置 CLI` 按钮。
- 点击后弹出一个优雅流体玻璃拟态的 Modal 面板。
- **左侧**：展示 19 个 CLI 列表与它们的可用状态及当前版本。
- **右侧**：展示选中 CLI 的专属配置表单。
  - **自定义二进制路径**：输入框允许覆盖系统默认查找路径（如 `CODEX_BIN`）。
  - **特定环境变量配置**：
    - `claude` 关联配置：`CLAUDE_CONFIG_DIR`, `ANTHROPIC_BASE_URL`, `ANTHROPIC_API_KEY`
    - `codex` 关联配置：`CODEX_HOME`, `OPENAI_BASE_URL`, `CODEX_API_KEY`, `OPENAI_API_KEY`
    - `gemini` 关联配置：`GEMINI_API_KEY`
    - 其它引擎提供通用的 API Key 与 Base URL 覆盖字段。
  - **测试与保存**：
    - 面板底部提供 `⚡ 运行连通测试` 按钮，直接在当前页面反馈测试结果，显示绿色成功提示或折叠式红色 stderr/stdout 诊断详情。
    - “保存并应用”按钮：一键持久化到全局 `config.json`。

---

## 3. 验证方案

1. **编译与类型安全验证**：
   - 运行 `cargo check` 验证 Rust 编译通过。
   - 运行 `npx tsc -b --noEmit` 验证 React / TS 前端无类型错误。
2. **测试场景**：
   - 手动配置 `codex` 的自定义路径 `CODEX_BIN`，验证配置可以被正确应用，并能够正常拉起 `codex` 连通性测试。
   - 故意配置错误的路径，验证连通性测试在 45 秒内报告失败，并准确展开 stderr/stdout 诊断信息。
   - 验证主界面的复盘任务运行新版 `codex` 时，不再出现 unexpected argument `-q` 报错，且能够正常分析。
