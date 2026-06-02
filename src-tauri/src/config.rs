use std::path::{Path, PathBuf};
use std::fs;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct AppConfig {
    pub database_type: String,
    pub db_sqlite_path: String,
    pub device_name: String,
    pub display_currency: String,
    pub close_behavior: String,
    pub db_pg_host: String,
    pub db_pg_port: String,
    pub db_pg_user: String,
    pub db_pg_password: String,
    pub db_pg_database: String,
    #[serde(default)]
    pub developer_mode: bool,
    /// CLI 引擎自定义环境变量，格式：{ "codex": { "CODEX_BIN": "/path", "OPENAI_API_KEY": "sk-..." } }
    /// 与 open-design 的 agentCliEnv 字段保持一致的数据结构
    #[serde(default)]
    pub agent_cli_env: std::collections::HashMap<String, std::collections::HashMap<String, String>>,
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
            developer_mode: false,
            agent_cli_env: std::collections::HashMap::new(),
        }
    }
}

pub fn get_user_profile_dir() -> String {
    std::env::var("USERPROFILE").unwrap_or_else(|_| r"C:\Users\cearn".to_string())
}

pub fn get_config_path() -> PathBuf {
    Path::new(&get_user_profile_dir())
        .join(".token-insight")
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
    std::env::set_var("DEVELOPER_MODE", if config.developer_mode { "true" } else { "false" });

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
    let path = get_config_path();
    if !path.exists() {
        save_config(&config)?;
    }
    sync_to_env(&config);
    Ok(())
}
