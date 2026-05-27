use std::sync::{Arc, Mutex, OnceLock};
use std::path::{Path, PathBuf};
use std::fs;

// 1. 统一的 SQL 绑参变体
#[derive(Clone, Debug)]
pub enum SqlParam {
    Text(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Null,
}

// 2. 统一的行字段读取 Trait
pub trait RowWrapper {
    fn get_str(&self, idx: usize) -> Result<String, String>;
    fn get_opt_str(&self, idx: usize) -> Result<Option<String>, String>;
    fn get_i64(&self, idx: usize) -> Result<i64, String>;
    fn get_opt_i64(&self, idx: usize) -> Result<Option<i64>, String>;
    fn get_f64(&self, idx: usize) -> Result<f64, String>;
    fn get_opt_f64(&self, idx: usize) -> Result<Option<f64>, String>;
    fn get_bool(&self, idx: usize) -> Result<bool, String>;
}

// 3. 统一的多库物理连接池枚举 (使用 Mutex 包装以支持多线程下的可变借用操作)
pub enum DbConn {
    Sqlite(Mutex<rusqlite::Connection>),
    Postgres(Mutex<postgres::Client>),
}

// SQLite 行映射实现
struct SqliteRowWrapper<'a>(&'a rusqlite::Row<'a>);
impl<'a> RowWrapper for SqliteRowWrapper<'a> {
    fn get_str(&self, idx: usize) -> Result<String, String> {
        self.0.get(idx).map_err(|e| e.to_string())
    }
    fn get_opt_str(&self, idx: usize) -> Result<Option<String>, String> {
        self.0.get(idx).map_err(|e| e.to_string())
    }
    fn get_i64(&self, idx: usize) -> Result<i64, String> {
        self.0.get(idx).map_err(|e| e.to_string())
    }
    fn get_opt_i64(&self, idx: usize) -> Result<Option<i64>, String> {
        self.0.get(idx).map_err(|e| e.to_string())
    }
    fn get_f64(&self, idx: usize) -> Result<f64, String> {
        self.0.get(idx).map_err(|e| e.to_string())
    }
    fn get_opt_f64(&self, idx: usize) -> Result<Option<f64>, String> {
        self.0.get(idx).map_err(|e| e.to_string())
    }
    fn get_bool(&self, idx: usize) -> Result<bool, String> {
        // SQLite 中 bool 存储为 0 或 1
        let val: i64 = self.0.get(idx).map_err(|e| e.to_string())?;
        Ok(val != 0)
    }
}

// PostgreSQL 行映射实现
struct PostgresRowWrapper<'a>(&'a postgres::Row);
impl<'a> RowWrapper for PostgresRowWrapper<'a> {
    fn get_str(&self, idx: usize) -> Result<String, String> {
        self.0.try_get(idx).map_err(|e| e.to_string())
    }
    fn get_opt_str(&self, idx: usize) -> Result<Option<String>, String> {
        self.0.try_get(idx).map_err(|e| e.to_string())
    }
    fn get_i64(&self, idx: usize) -> Result<i64, String> {
        self.0.try_get(idx).map_err(|e| e.to_string())
    }
    fn get_opt_i64(&self, idx: usize) -> Result<Option<i64>, String> {
        self.0.try_get(idx).map_err(|e| e.to_string())
    }
    fn get_f64(&self, idx: usize) -> Result<f64, String> {
        self.0.try_get(idx).map_err(|e| e.to_string())
    }
    fn get_opt_f64(&self, idx: usize) -> Result<Option<f64>, String> {
        self.0.try_get(idx).map_err(|e| e.to_string())
    }
    fn get_bool(&self, idx: usize) -> Result<bool, String> {
        self.0.try_get(idx).map_err(|e| e.to_string())
    }
}

// PostgreSQL 执行期参数持久容器，保障借用生命周期
enum PgOwned {
    Text(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Null,
}

// 占位符运行时动态转译：? 转换为 $1, $2, $3 ...
fn normalize_sql(sql: &str) -> String {
    let mut normalized = String::new();
    let mut param_idx = 1;
    for c in sql.chars() {
        if c == '?' {
            normalized.push_str(&format!("${}", param_idx));
            param_idx += 1;
        } else {
            normalized.push(c);
        }
    }
    normalized
}

// 参数转换辅助
fn to_sqlite_params(params: &[SqlParam]) -> Vec<rusqlite::types::Value> {
    params.iter().map(|p| match p {
        SqlParam::Text(s) => rusqlite::types::Value::Text(s.clone()),
        SqlParam::Int(i) => rusqlite::types::Value::Integer(*i),
        SqlParam::Float(f) => rusqlite::types::Value::Real(*f),
        SqlParam::Bool(b) => rusqlite::types::Value::Integer(if *b { 1 } else { 0 }),
        SqlParam::Null => rusqlite::types::Value::Null,
    }).collect()
}

fn to_pg_owned(params: &[SqlParam]) -> Vec<PgOwned> {
    params.iter().map(|p| match p {
        SqlParam::Text(s) => PgOwned::Text(s.clone()),
        SqlParam::Int(i) => PgOwned::Int(*i),
        SqlParam::Float(f) => PgOwned::Float(*f),
        SqlParam::Bool(b) => PgOwned::Bool(*b),
        SqlParam::Null => PgOwned::Null,
    }).collect()
}

fn to_pg_refs<'a>(owned: &'a [PgOwned]) -> Vec<&'a (dyn postgres::types::ToSql + Sync)> {
    owned.iter().map(|p| match p {
        PgOwned::Text(s) => s as &(dyn postgres::types::ToSql + Sync),
        PgOwned::Int(i) => i as &(dyn postgres::types::ToSql + Sync),
        PgOwned::Float(f) => f as &(dyn postgres::types::ToSql + Sync),
        PgOwned::Bool(b) => b as &(dyn postgres::types::ToSql + Sync),
        PgOwned::Null => &None::<i32> as &(dyn postgres::types::ToSql + Sync),
    }).collect()
}


// 4. 统一数据库交互接口实现
impl DbConn {
    pub fn execute(&self, sql: &str, params: &[SqlParam]) -> Result<usize, String> {
        match self {
            DbConn::Sqlite(conn_lock) => {
                let conn = conn_lock.lock().unwrap();
                let r_params = to_sqlite_params(params);
                conn.execute(sql, rusqlite::params_from_iter(r_params))
                    .map_err(|e| e.to_string())
            }
            DbConn::Postgres(client_lock) => {
                let mut client = client_lock.lock().unwrap();
                let normalized_sql = normalize_sql(sql);
                let owned = to_pg_owned(params);
                let pg_params = to_pg_refs(&owned);
                client.execute(&normalized_sql, &pg_params[..])
                    .map_err(|e| e.to_string())
                    .map(|v| v as usize)
            }
        }
    }

    pub fn query<T, F>(&self, sql: &str, params: &[SqlParam], mut mapper: F) -> Result<Vec<T>, String>
    where
        F: FnMut(&dyn RowWrapper) -> Result<T, String>
    {
        match self {
            DbConn::Sqlite(conn_lock) => {
                let conn = conn_lock.lock().unwrap();
                let r_params = to_sqlite_params(params);
                let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
                let mut rows = stmt.query(rusqlite::params_from_iter(r_params)).map_err(|e| e.to_string())?;
                
                let mut results = Vec::new();
                while let Some(row) = rows.next().map_err(|e| e.to_string())? {
                    let wrapper = SqliteRowWrapper(&row);
                    let val = mapper(&wrapper)?;
                    results.push(val);
                }
                Ok(results)
            }
            DbConn::Postgres(client_lock) => {
                let mut client = client_lock.lock().unwrap();
                let normalized_sql = normalize_sql(sql);
                let owned = to_pg_owned(params);
                let pg_params = to_pg_refs(&owned);
                
                let rows = client.query(&normalized_sql, &pg_params[..]).map_err(|e| e.to_string())?;
                let mut results = Vec::new();
                for row in rows {
                    let wrapper = PostgresRowWrapper(&row);
                    let val = mapper(&wrapper)?;
                    results.push(val);
                }
                Ok(results)
            }
        }
    }

    pub fn query_row<T, F>(&self, sql: &str, params: &[SqlParam], mapper: F) -> Result<T, String>
    where
        F: FnOnce(&dyn RowWrapper) -> Result<T, String>
    {
        let mut mapper_opt = Some(mapper);
        let results = self.query(sql, params, |r| {
            if let Some(m) = mapper_opt.take() {
                m(r)
            } else {
                Err("query_row mapper called multiple times".to_string())
            }
        })?;
        
        if results.is_empty() {
            Err("No row found".to_string())
        } else {
            Ok(results.into_iter().next().unwrap())
        }
    }
}

// 5. 自动 Schema DDL 创表迁移脚本
pub fn init_tables(conn: &DbConn) -> Result<(), String> {
    match conn {
        DbConn::Sqlite(conn_lock) => {
            let conn = conn_lock.lock().unwrap();
            conn.execute(
                "CREATE TABLE IF NOT EXISTS sessions (
                    source TEXT NOT NULL,
                    uuid TEXT NOT NULL,
                    title TEXT,
                    created_at TEXT,
                    last_parsed_idx INTEGER DEFAULT -1,
                    last_mtime REAL DEFAULT 0.0,
                    project_path TEXT,
                    PRIMARY KEY (source, uuid)
                )",
                [],
            ).map_err(|e| e.to_string())?;

            conn.execute(
                "CREATE TABLE IF NOT EXISTS turns (
                    source TEXT NOT NULL,
                    uuid TEXT NOT NULL,
                    idx INTEGER NOT NULL,
                    model TEXT,
                    input_tokens INTEGER DEFAULT 0,
                    cached_input_tokens INTEGER DEFAULT 0,
                    output_tokens INTEGER DEFAULT 0,
                    thinking_tokens INTEGER DEFAULT 0,
                    cost_usd REAL DEFAULT 0.0,
                    message_id TEXT,
                    request_id TEXT,
                    timestamp TEXT,
                    PRIMARY KEY (source, uuid, idx),
                    FOREIGN KEY(source, uuid) REFERENCES sessions(source, uuid) ON DELETE CASCADE
                )",
                [],
            ).map_err(|e| e.to_string())?;
        }
        DbConn::Postgres(client_lock) => {
            let mut client = client_lock.lock().unwrap();
            client.execute(
                "CREATE TABLE IF NOT EXISTS sessions (
                    source VARCHAR(50) NOT NULL,
                    uuid VARCHAR(255) NOT NULL,
                    title TEXT,
                    created_at VARCHAR(100),
                    last_parsed_idx BIGINT DEFAULT -1,
                    last_mtime DOUBLE PRECISION DEFAULT 0.0,
                    project_path TEXT,
                    PRIMARY KEY (source, uuid)
                )",
                &[],
            ).map_err(|e| e.to_string())?;

            client.execute(
                "CREATE TABLE IF NOT EXISTS turns (
                    source VARCHAR(50) NOT NULL,
                    uuid VARCHAR(255) NOT NULL,
                    idx BIGINT NOT NULL,
                    model VARCHAR(255),
                    input_tokens BIGINT DEFAULT 0,
                    cached_input_tokens BIGINT DEFAULT 0,
                    output_tokens BIGINT DEFAULT 0,
                    thinking_tokens BIGINT DEFAULT 0,
                    cost_usd DOUBLE PRECISION DEFAULT 0.0,
                    message_id VARCHAR(255),
                    request_id VARCHAR(255),
                    timestamp VARCHAR(100),
                    PRIMARY KEY (source, uuid, idx),
                    FOREIGN KEY(source, uuid) REFERENCES sessions(source, uuid) ON DELETE CASCADE
                )",
                &[],
            ).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

// 6. 全局静态连接句柄与懒加载重载器
pub static GLOBAL_CONN: OnceLock<Mutex<Option<Arc<DbConn>>>> = OnceLock::new();

// 获取当前默认的本地 SQLite 物理文件路径
pub fn get_user_profile_dir() -> String {
    std::env::var("USERPROFILE").unwrap_or_else(|_| r"C:\Users\cearn".to_string())
}

pub fn get_default_sqlite_path() -> PathBuf {
    Path::new(&get_user_profile_dir())
        .join(".ai_token_monitor")
        .join("token_stats.db")
}

// 根据配置创建全新物理连接，并静默初始化表结构
pub fn init_new_conn() -> Result<DbConn, String> {
    // 强制加载项目目录下的 .env
    let _ = dotenvy::dotenv();

    let db_type = std::env::var("DATABASE_TYPE").unwrap_or_else(|_| "sqlite".to_string());
    
    if db_type.to_lowercase() == "postgres" {
        let db_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://postgres:password@localhost:5432/token_monitor".to_string());
        
        let client = postgres::Client::connect(&db_url, postgres::NoTls)
            .map_err(|e| format!("Failed to connect Postgres: {}", e))?;
        
        let conn = DbConn::Postgres(Mutex::new(client));
        init_tables(&conn)?;
        Ok(conn)
    } else {
        let db_path_str = std::env::var("DB_SQLITE_PATH").unwrap_or_default();
        let db_path = if db_path_str.trim().is_empty() {
            get_default_sqlite_path()
        } else {
            PathBuf::from(db_path_str)
        };

        if let Some(parent) = db_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let sqlite_conn = rusqlite::Connection::open(&db_path)
            .map_err(|e| format!("Failed to open SQLite: {}", e))?;
        
        let conn = DbConn::Sqlite(Mutex::new(sqlite_conn));
        init_tables(&conn)?;
        Ok(conn)
    }
}

// 外部核心入口：获取全局活动连接句柄
pub fn get_active_conn() -> Result<Arc<DbConn>, String> {
    let lock = GLOBAL_CONN.get_or_init(|| Mutex::new(None));
    let mut guard = lock.lock().unwrap();
    if let Some(conn) = &*guard {
        return Ok(Arc::clone(conn));
    }
    
    let new_conn = init_new_conn()?;
    let shared = Arc::new(new_conn);
    *guard = Some(Arc::clone(&shared));
    Ok(shared)
}

// 外部核心重载命令：清空全局连接池缓存，以便后续 API 调用按最新的环境变量重新建池
pub fn reset_conn_pool() {
    let lock = GLOBAL_CONN.get_or_init(|| Mutex::new(None));
    let mut guard = lock.lock().unwrap();
    *guard = None;
}
