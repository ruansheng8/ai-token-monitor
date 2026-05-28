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
mod sqlite_migrations {
    refinery::embed_migrations!("migrations/sqlite");
}
mod postgres_migrations {
    refinery::embed_migrations!("migrations/postgres");
}

pub fn init_tables(conn: &DbConn) -> Result<(), String> {
    match conn {
        DbConn::Sqlite(conn_lock) => {
            let mut conn = conn_lock.lock().unwrap();
            let report = sqlite_migrations::migrations::runner()
                .set_abort_divergent(false)
                .set_abort_missing(false)
                .run(&mut *conn)
                .map_err(|e| format!("Refinery SQLite migration failed: {:?} (底层详情: {})", e, e))?;
            for migration in report.applied_migrations() {
                println!("[数据库迁移] 应用 SQLite 表结构变更: {} (版本 {})", migration.name(), migration.version());
            }
        }
        DbConn::Postgres(client_lock) => {
            let mut client = client_lock.lock().unwrap();
            
            // 获取 Postgres 会话级排他咨询锁，保证多线程或多进程并发迁移时串行执行，避免 refinery 迁移主键冲突
            let _ = client.execute("SELECT pg_advisory_lock(763529)", &[])
                .map_err(|e| format!("获取 Postgres 数据库迁移排他锁失败: {}", e))?;
            
            // 运行数据库迁移
            let report_res = postgres_migrations::migrations::runner()
                .set_abort_divergent(false)
                .set_abort_missing(false)
                .run(&mut *client);
            
            // 释放排他咨询锁
            let _ = client.execute("SELECT pg_advisory_unlock(763529)", &[]);
            
            let report = report_res.map_err(|e| format!("Refinery Postgres migration failed: {:?} (底层详情: {})", e, e))?;
            for migration in report.applied_migrations() {
                println!("[数据库迁移] 应用 Postgres 表结构变更: {} (版本 {})", migration.name(), migration.version());
            }
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
    #[cfg(not(test))]
    let _ = dotenvy::dotenv_override();
    #[cfg(test)]
    let _ = dotenvy::dotenv();

    let db_type = std::env::var("DATABASE_TYPE").unwrap_or_else(|_| "sqlite".to_string());
    
    if db_type.to_lowercase() == "postgres" {
        let pg_host = std::env::var("DB_PG_HOST").unwrap_or_default();
        let pg_port = std::env::var("DB_PG_PORT").unwrap_or_default();
        let pg_user = std::env::var("DB_PG_USER").unwrap_or_default();
        let pg_password = std::env::var("DB_PG_PASSWORD").unwrap_or_default();
        let pg_database = std::env::var("DB_PG_DATABASE").unwrap_or_default();

        let db_url = if !pg_host.trim().is_empty() {
            format!(
                "postgresql://{}:{}@{}:{}/{}",
                pg_user, pg_password, pg_host, pg_port, pg_database
            )
        } else {
            std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgresql://postgres:password@localhost:5432/token_monitor".to_string())
        };
        
        // 自动检测并创建数据库
        let _ = ensure_pg_database_exists(&db_url);

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

// 从完整连接串中解析出 (host, port, user, password, database)
pub fn parse_pg_url(url: &str) -> Option<(String, String, String, String, String)> {
    if !url.starts_with("postgresql://") && !url.starts_with("postgres://") {
        return None;
    }
    let rest = if url.starts_with("postgresql://") {
        &url["postgresql://".len()..]
    } else {
        &url["postgres://".len()..]
    };

    let parts: Vec<&str> = rest.splitn(2, '@').collect();
    if parts.len() != 2 {
        return None;
    }

    let user_pass = parts[0];
    let host_port_db = parts[1];

    let up_parts: Vec<&str> = user_pass.splitn(2, ':').collect();
    let user = up_parts.get(0).cloned().unwrap_or("").to_string();
    let pass = up_parts.get(1).cloned().unwrap_or("").to_string();

    let hp_db_parts: Vec<&str> = host_port_db.splitn(2, '/').collect();
    let host_port = hp_db_parts.get(0).cloned().unwrap_or("");
    let database = hp_db_parts.get(1).cloned().unwrap_or("").to_string();

    let hp_parts: Vec<&str> = host_port.splitn(2, ':').collect();
    let host = hp_parts.get(0).cloned().unwrap_or("").to_string();
    let port = hp_parts.get(1).cloned().unwrap_or("5432").to_string();

    Some((host, port, user, pass, database))
}

pub fn ensure_pg_database_exists(url: &str) -> Result<(), String> {
    if let Some((host, port, user, pass, database)) = parse_pg_url(url) {
        if database.trim().is_empty() || database == "postgres" {
            return Ok(());
        }
        
        let admin_url = format!(
            "postgresql://{}:{}@{}:{}/postgres",
            user, pass, host, port
        );

        let mut client = postgres::Client::connect(&admin_url, postgres::NoTls)
            .map_err(|e| format!("无法连接到 PostgreSQL 默认管理库 (postgres) 进行数据库校验: {}", e))?;

        let row = client.query_one(
            "SELECT EXISTS(SELECT 1 FROM pg_database WHERE datname = $1)",
            &[&database],
        ).map_err(|e| format!("查询 pg_database 失败: {}", e))?;

        let exists: bool = row.get(0);
        if !exists {
            println!("[数据库创建] 检测到目标 PostgreSQL 数据库 '{}' 不存在，正在创建...", database);
            if !database.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
                return Err(format!("非法的数据库名称 '{}'，拒绝自动创建以防 SQL 注入", database));
            }
            client.execute(&format!("CREATE DATABASE \"{}\"", database), &[])
                .map_err(|e| format!("创建数据库 '{}' 失败: {}", database, e))?;
            println!("[数据库创建] 目标 PostgreSQL 数据库 '{}' 创建成功！", database);
        }
    }
    Ok(())
}
