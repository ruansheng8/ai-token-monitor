# 2026-06-01 客户端 JSON 配置文件与数据库目录结构规范化迁移实现计划

本计划详述了将 AI Token Monitor 从原本的 `.env` 配置文件重构为用户目录下的 `config/config.json`，并将 SQLite 数据库迁移至 `db/token_stats.db` 的具体实现细节。

---

## 1. 拟修改的文件与职责划分

### [NEW] [config.rs](file:///d:/VibeCoding/ai-token-monitor/src-tauri/src/config.rs)
- 定义 `AppConfig` 结构体，支持包含数据库类型、SQLite 路径、设备名、货币类型、退出行为以及 PostgreSQL 全套凭证。
- 实现 `load_config`、`save_config`、`sync_to_env` 以及 `init_config`。

### [MODIFY] [db_adapter.rs](file:///d:/VibeCoding/ai-token-monitor/src-tauri/src/db_adapter.rs)
- 修改 `get_default_sqlite_path`，使其默认路径指向 `~/.ai_token_monitor/db/token_stats.db`。
- 移除 `get_user_profile_dir`（统一由新模块 `config.rs` 提供或使用）。

### [MODIFY] [db.rs](file:///d:/VibeCoding/ai-token-monitor/src-tauri/src/db.rs)
- 修改 `get_db_cache_path` 指向 `~/.ai_token_monitor/db/token_stats.db`。
- 移除 `get_user_profile_dir` 并由 `config.rs` 提供。

### [MODIFY] [main.rs](file:///d:/VibeCoding/ai-token-monitor/src-tauri/src/main.rs)
- 声明 `mod config;`。
- 在 `main` 函数首行首先执行 `config::init_config().expect("初始化配置文件失败");` 确保环境变量在整个应用及数据库初始化前已经注入完毕。

### [MODIFY] [server.rs](file:///d:/VibeCoding/ai-token-monitor/src-tauri/src/server.rs)
- 重构 `handle_config_get`，使用 `config::load_config()` 数据拼装 API 响应。
- 重构 `handle_config_save`，将配置映射为 `AppConfig` 后调用 `config::save_config` 及 `config::sync_to_env` 动态同步配置，并移除原本的 `.env` 读写代码。

---

## 2. 验证方案

- **编译检查**：
  - 在 `src-tauri` 目录执行 `cargo check`。
- **文件检查**：
  - 启动后检查 `~/.ai_token_monitor/config/config.json` 和 `~/.ai_token_monitor/db/token_stats.db` 的创建。
- **接口功能测试**：
  - 调用 `handle_config_get` 获取配置，并用 `handle_config_save` 更新配置，确认配置被持久化且实时应用到程序的环境变量。
