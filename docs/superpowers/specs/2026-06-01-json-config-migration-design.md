# 2026-06-01 客户端 JSON 配置文件迁移设计规范

本规范旨在将 AI Token Monitor 从原先的本地项目 `.env` 环境变量配置方式，重构为符合桌面客户端规范的 `config.json` 文件配置方式，并对本地文件目录结构进行标准化划分，以彻底解决在使用 MSI 安装包发布后写入 `.env` 时遇到的 "拒绝访问 (os error 5)" 错误。

---

## 1. 目标与价值

- **避免权限报错**：将配置存放在用户主目录可写的 App Data 目录下，彻底避开系统 Program Files 目录的只读保护。
- **结构化管理**：将零散的配置参数用 JSON 进行规范化存储，方便未来扩展。
- **目录标准化**：
  - 配置文件存放路径：`~/.ai_token_monitor/config/config.json`
  - 数据库文件存放路径：`~/.ai_token_monitor/db/token_stats.db`
- **最小化侵入性**：在初始化读取 `config.json` 后，将值注入进程环境变量中，下游多处使用 `std::env::var` 的核心代码无需修改。

---

## 2. 详细设计

### 2.1 新增配置结构体与文件读写 (`config.rs`)

在 `src-tauri/src/config.rs` 中定义 `AppConfig` 结构体：

```rust
use std::path::{Path, PathBuf};
use std::fs;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct AppConfig {
    pub database_type: String,      // 默认 "sqlite"
    pub db_sqlite_path: String,     // 自定义 sqlite 路径，默认为空 ""
    pub device_name: String,        // 设备名，默认为空 ""
    pub display_currency: String,   // 显示货币，默认 "USD"
    pub close_behavior: String,     // 退出行为，默认 "prompt"
    
    // PostgreSQL 凭证
    pub db_pg_host: String,
    pub db_pg_port: String,
    pub db_pg_user: String,
    pub db_pg_password: String,
    pub db_pg_database: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            database_type: "sqlite".to_string(),
            db_sqlite_path: "".to_string(),
            device_name: "".to_string(),
            display_currency: "USD".to_string(),
            close_behavior: "prompt".to_string(),
            db_pg_host: "".to_string(),
            db_pg_port: "".to_string(),
            db_pg_user: "".to_string(),
            db_pg_password: "".to_string(),
            db_pg_database: "".to_string(),
        }
    }
}
```

#### 配置读写与环境变量注入 API

```rust
pub fn get_user_profile_dir() -> String {
    std::env::var("USERPROFILE").unwrap_or_else(|_| r"C:\Users\cearn".to_string())
}

pub fn get_config_path() -> PathBuf {
    Path::new(&get_user_profile_dir())
        .join(".ai_token_monitor")
        .join("config")
        .join("config.json")
}

pub fn load_config() -> AppConfig {
    let path = get_config_path();
    if !path.exists() {
        return AppConfig::default();
    }
    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return AppConfig::default(),
    };
    serde_json::from_str(&content).unwrap_or_else(|_| AppConfig::default())
}

pub fn save_config(config: &AppConfig) -> Result<(), String> {
    let path = get_config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建配置文件夹失败: {}", e))?;
    }
    let content = serde_json::to_string_pretty(config)
        .map_err(|e| format!("序列化配置 JSON 失败: {}", e))?;
    fs::write(&path, content).map_err(|e| format!("写入配置文件失败: {}", e))?;
    Ok(())
}

pub fn sync_to_env(config: &AppConfig) {
    std::env::set_var("DATABASE_TYPE", &config.database_type);
    std::env::set_var("DB_SQLITE_PATH", &config.db_sqlite_path);
    std::env::set_var("DEVICE_NAME", &config.device_name);
    std::env::set_var("DISPLAY_CURRENCY", &config.display_currency);
    std::env::set_var("CLOSE_BEHAVIOR", &config.close_behavior);
    std::env::set_var("DB_PG_HOST", &config.db_pg_host);
    std::env::set_var("DB_PG_PORT", &config.db_pg_port);
    std::env::set_var("DB_PG_USER", &config.db_pg_user);
    std::env::set_var("DB_PG_PASSWORD", &config.db_pg_password);
    std::env::set_var("DB_PG_DATABASE", &config.db_pg_database);

    // 适配现有的 DATABASE_URL 环境变量注入逻辑
    if !config.db_pg_host.is_empty() {
        let url = format!(
            "postgresql://{}:{}@{}:{}/{}",
            config.db_pg_user, config.db_pg_password, config.db_pg_host, config.db_pg_port, config.db_pg_database
        );
        std::env::set_var("DATABASE_URL", &url);
    } else {
        std::env::set_var("DATABASE_URL", "");
    }
}

pub fn init_config() -> Result<(), String> {
    let config = load_config();
    // 首次运行时，如果是新环境，生成一份默认配置文件
    let path = get_config_path();
    if !path.exists() {
        save_config(&config)?;
    }
    sync_to_env(&config);
    Ok(())
}
```

### 2.2 数据库存储目录变动

修改以下两处函数：
- `src-tauri/src/db_adapter.rs` 中的 `get_default_sqlite_path()`
- `src-tauri/src/db.rs` 中的 `get_db_cache_path()`

新路径设计：
```rust
pub fn get_db_cache_path() -> PathBuf {
    Path::new(&get_user_profile_dir())
        .join(".ai_token_monitor")
        .join("db")  // 新增子文件夹
        .join("token_stats.db")
}
```

在 `db::init_cache_db` 运行时，会自动调用 `fs::create_dir_all(db_path.parent())`，这能够自动在新路径下创建 `db` 文件夹。

### 2.3 Web 接口调整

修改 `src-tauri/src/server.rs` 中的相应接口：
1. **`handle_config_get`**:
   - 内部不再执行 `dotenvy` 加载，直接调用 `config::load_config()` 读取数据，并转换组装成 `ConfigReq` 结构返回。
2. **`handle_config_save`**:
   - 接收 `ConfigReq` 后，转换构造出 `AppConfig` 结构体。
   - 调用 `config::save_config(&new_config)`。
   - 调用 `config::sync_to_env(&new_config)`，以保证更改实时应用于当前进程的环境。
   - 彻底删除向本地项目 `.env` 文件写入的代码。

---

## 3. 验证方案

- **编译校验**：
  - 在 `src-tauri` 目录下运行 `cargo check` 确保无编译错误。
- **首运行测试**：
  - 启动后检查 `~/.ai_token_monitor/` 路径下是否自动生成 `config/config.json` 与 `db/token_stats.db` 文件。
- **动态保存测试**：
  - 在大盘前端修改系统配置（如设备名、显示货币），点击保存，检查 `config.json` 是否同步更新，且对应更改是否在不重启下实时生效。
