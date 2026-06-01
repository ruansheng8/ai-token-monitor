use std::path::{Path, PathBuf};

// 自动 Schema DDL 创表迁移脚本
mod postgres_migrations {
    refinery::embed_migrations!("migrations/postgres");
}

pub fn init_postgres_tables(client: &mut postgres::Client) -> Result<(), String> {
    // 获取 Postgres 会话级排他咨询锁，保证多线程或多进程并发迁移时串行执行，避免 refinery 迁移主键冲突
    let _ = client.execute("SELECT pg_advisory_lock(763529)", &[])
        .map_err(|e| format!("获取 Postgres 数据库迁移排他锁失败: {}", e))?;
    
    // 运行数据库迁移
    let report_res = postgres_migrations::migrations::runner()
        .set_abort_divergent(false)
        .set_abort_missing(false)
        .run(client);
    
    // 释放排他咨询锁
    let _ = client.execute("SELECT pg_advisory_unlock(763529)", &[]);
    
    let report = report_res.map_err(|e| format!("Refinery Postgres migration failed: {:?} (底层详情: {})", e, e))?;
    for migration in report.applied_migrations() {
        println!("[数据库迁移] 应用 Postgres 表结构变更: {} (版本 {})", migration.name(), migration.version());
    }
    Ok(())
}

// 获取当前默认的本地 SQLite 物理文件路径
pub fn get_default_sqlite_path() -> PathBuf {
    Path::new(&crate::config::get_user_profile_dir())
        .join(".ai_token_monitor")
        .join("db")
        .join("token_stats.db")
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
