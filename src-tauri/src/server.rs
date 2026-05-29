use std::path::PathBuf;
use axum::{
    body::Body,
    http::{header, Response, StatusCode, Uri},
    response::IntoResponse,
};

use crate::db::{
    get_aggregated_metrics_from_cache, get_pg_aggregated_metrics, start_background_scan, get_scan_status,
    get_sessions_paginated, get_pg_sessions_paginated,
};

pub async fn handle_sessions_paginated(
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Response<Body> {
    let page = params.get("page").and_then(|s| s.parse::<usize>().ok()).unwrap_or(1);
    let page_size = params.get("page_size").and_then(|s| s.parse::<usize>().ok()).unwrap_or(10);
    let search = params.get("search").filter(|s| !s.is_empty()).cloned();
    let source = params.get("source").filter(|s| !s.is_empty()).cloned();
    let sort_by = params.get("sort_by").filter(|s| !s.is_empty()).cloned();
    let sort_order = params.get("sort_order").filter(|s| !s.is_empty()).cloned();
    let start_date = params.get("start_date").filter(|s| !s.is_empty()).cloned();
    let end_date = params.get("end_date").filter(|s| !s.is_empty()).cloned();
    let hide_zero = params.get("hide_zero").map(|s| s == "true" || s == "1").unwrap_or(true);

    match tokio::task::spawn_blocking(move || {
        let db_type = std::env::var("DATABASE_TYPE").unwrap_or_else(|_| "sqlite".to_string());
        if db_type.to_lowercase() == "postgres" {
            get_pg_sessions_paginated(
                page, 
                page_size, 
                search.as_deref(), 
                source.as_deref(), 
                sort_by.as_deref(), 
                sort_order.as_deref(), 
                start_date.as_deref(), 
                end_date.as_deref(), 
                hide_zero
            )
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(std::io::ErrorKind::Other, e))))
        } else {
            get_sessions_paginated(
                page, 
                page_size, 
                search.as_deref(), 
                source.as_deref(), 
                sort_by.as_deref(), 
                sort_order.as_deref(), 
                start_date.as_deref(), 
                end_date.as_deref(), 
                hide_zero
            )
        }
    })
    .await
    {
        Ok(Ok(data)) => {
            let body = match serde_json::to_vec(&data) {
                Ok(bytes) => Body::from(bytes),
                Err(e) => return Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
                    .body(Body::from(format!("JSON Serialization Error: {}", e)))
                    .unwrap(),
            };
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CACHE_CONTROL, "no-cache, no-store, must-revalidate")
                .header(header::PRAGMA, "no-cache")
                .header(header::EXPIRES, "0")
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(body)
                .unwrap()
        }
        Ok(Err(e)) => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(Body::from(format!("Database Error: {}", e)))
            .unwrap(),
        Err(e) => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(Body::from(format!("Server Thread Error: {}", e)))
            .unwrap(),
    }
}

pub async fn handle_metrics(
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Response<Body> {
    let source = params.get("source").cloned();
    let start_date = params.get("start_date").cloned();
    let end_date = params.get("end_date").cloned();
    match tokio::task::spawn_blocking(move || {
        let db_type = std::env::var("DATABASE_TYPE").unwrap_or_else(|_| "sqlite".to_string());
        if db_type.to_lowercase() == "postgres" {
            get_pg_aggregated_metrics(source.as_deref(), start_date.as_deref(), end_date.as_deref())
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(std::io::ErrorKind::Other, e))))
        } else {
            get_aggregated_metrics_from_cache(source.as_deref(), start_date.as_deref(), end_date.as_deref())
        }
    })
    .await
    {
        Ok(Ok(data)) => {
            let body = match serde_json::to_vec(&data) {
                Ok(bytes) => Body::from(bytes),
                Err(e) => return Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
                    .body(Body::from(format!("JSON Serialization Error: {}", e)))
                    .unwrap(),
            };
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CACHE_CONTROL, "no-cache, no-store, must-revalidate")
                .header(header::PRAGMA, "no-cache")
                .header(header::EXPIRES, "0")
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(body)
                .unwrap()
        }
        Ok(Err(e)) => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(Body::from(format!("Database Error: {}", e)))
            .unwrap(),
        Err(e) => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(Body::from(format!("Server Thread Error: {}", e)))
            .unwrap(),
    }
}

pub async fn handle_scan_start() -> Response<Body> {
    match tokio::task::spawn_blocking(|| {
        start_background_scan(false);
        
        let status = get_scan_status().lock().unwrap().clone();
        serde_json::to_vec(&status)
    })
    .await
    {
        Ok(Ok(bytes)) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .body(Body::from(bytes))
            .unwrap(),
        _ => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(Body::from("Internal Server Error"))
            .unwrap(),
    }
}

pub async fn handle_scan_status() -> Response<Body> {
    let status = get_scan_status().lock().unwrap().clone();
    match serde_json::to_vec(&status) {
        Ok(bytes) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .body(Body::from(bytes))
            .unwrap(),
        _ => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(Body::from("Internal Server Error"))
            .unwrap(),
    }
}

#[derive(rust_embed::RustEmbed)]
#[folder = "../dist/"]
struct Asset;

pub async fn serve_static_file_fallback(uri: Uri) -> impl IntoResponse {
    let path_str = uri.path();
    let clean_path = percent_encoding::percent_decode_str(path_str)
        .decode_utf8_lossy()
        .into_owned();
    let clean_path = clean_path.trim_start_matches('/');

    let file_name = if clean_path.is_empty() {
        "index.html"
    } else {
        clean_path
    };

    // 优先从二进制嵌入的 Asset (即 frontend/dist) 中读取
    if let Some(file) = Asset::get(file_name) {
        let content_type = if file_name.ends_with(".html") {
            "text/html; charset=utf-8"
        } else if file_name.ends_with(".css") {
            "text/css; charset=utf-8"
        } else if file_name.ends_with(".js") || file_name.ends_with(".mjs") {
            "application/javascript; charset=utf-8"
        } else if file_name.ends_with(".svg") {
            "image/svg+xml"
        } else if file_name.ends_with(".png") {
            "image/png"
        } else {
            "application/octet-stream"
        };

        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, content_type)
            .body(Body::from(file.data.into_owned()))
            .unwrap();
    }

    // 回退逻辑：如果内置文件未匹配到，则尝试从磁盘读取（为了支持本地其他图片/文件等静态资源）
    let mut file_path = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|parent| parent.join(file_name)))
        .unwrap_or_else(|| PathBuf::from(file_name));

    if !file_path.exists() {
        file_path = PathBuf::from(file_name);
    }

    if !file_path.exists() || !file_path.is_file() {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(Body::from("File Not Found"))
            .unwrap();
    }

    let content_type = if file_name.ends_with(".html") {
        "text/html; charset=utf-8"
    } else if file_name.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if file_name.ends_with(".js") {
        "application/javascript; charset=utf-8"
    } else if file_name.ends_with(".png") {
        "image/png"
    } else if file_name.ends_with(".jpg") || file_name.ends_with(".jpeg") {
        "image/jpeg"
    } else {
        "text/plain; charset=utf-8"
    };

    match std::fs::read(&file_path) {
        Ok(content) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, content_type)
            .body(Body::from(content))
            .unwrap(),
        Err(_) => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(Body::from("Error reading static file"))
            .unwrap(),
    }
}

// ==================== 数据库数据源配置接口 ====================

#[derive(serde::Deserialize, serde::Serialize)]
pub struct ConfigReq {
    pub db_type: String,
    pub sqlite_path: Option<String>,
    pub pg_host: Option<String>,
    pub pg_port: Option<String>,
    pub pg_user: Option<String>,
    pub pg_password: Option<String>,
    pub pg_database: Option<String>,
    pub device_name: Option<String>,
    pub default_device_name: Option<String>,
    pub display_currency: Option<String>,
    pub close_behavior: Option<String>,
}

pub async fn handle_config_get() -> impl axum::response::IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        // 强制加载项目目录下的 .env
        #[cfg(not(test))]
        let _ = dotenvy::dotenv_override();
        #[cfg(test)]
        let _ = dotenvy::dotenv();

        let db_type = std::env::var("DATABASE_TYPE").unwrap_or_else(|_| "sqlite".to_string());
        let sqlite_path = std::env::var("DB_SQLITE_PATH").ok();
        let device_name = std::env::var("DEVICE_NAME").ok();
        let display_currency = std::env::var("DISPLAY_CURRENCY").ok();

        let default_device_name = std::env::var("COMPUTERNAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "unknown-device".to_string());
        
        let mut pg_host = std::env::var("DB_PG_HOST").unwrap_or_default();
        let mut pg_port = std::env::var("DB_PG_PORT").unwrap_or_default();
        let mut pg_user = std::env::var("DB_PG_USER").unwrap_or_default();
        let mut pg_password = std::env::var("DB_PG_PASSWORD").unwrap_or_default();
        let mut pg_database = std::env::var("DB_PG_DATABASE").unwrap_or_default();
        let close_behavior = std::env::var("CLOSE_BEHAVIOR").unwrap_or_else(|_| "prompt".to_string());

        // 向上兼容：如果拆分的属性为空，但存在 DATABASE_URL，则尝试解析并回显它
        if pg_host.trim().is_empty() {
            if let Ok(db_url) = std::env::var("DATABASE_URL") {
                if let Some((h, p, u, pwd, db)) = crate::db_adapter::parse_pg_url(&db_url) {
                    pg_host = h;
                    pg_port = p;
                    pg_user = u;
                    pg_password = pwd;
                    pg_database = db;
                }
            }
        }

        let resp = ConfigReq {
            db_type,
            sqlite_path,
            pg_host: Some(pg_host),
            pg_port: Some(pg_port),
            pg_user: Some(pg_user),
            pg_password: Some(pg_password),
            pg_database: Some(pg_database),
            device_name,
            default_device_name: Some(default_device_name),
            display_currency,
            close_behavior: Some(close_behavior),
        };

        Ok::<ConfigReq, String>(resp)
    }).await;

    match result {
        Ok(Ok(data)) => {
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from(serde_json::to_vec(&body_json_helper(&data)).unwrap()))
                .unwrap()
        }
        _ => {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
                .body(Body::from("Internal Server Error"))
                .unwrap()
        }
    }
}

// 辅助序列化
fn body_json_helper(data: &ConfigReq) -> serde_json::Value {
    serde_json::to_value(data).unwrap_or(serde_json::Value::Null)
}

pub async fn handle_config_test(
    axum::Json(req): axum::Json<ConfigReq>,
) -> impl axum::response::IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        if req.db_type.to_lowercase() == "postgres" {
            let host = req.pg_host.unwrap_or_default();
            let port = req.pg_port.unwrap_or_default();
            let user = req.pg_user.unwrap_or_default();
            let password = req.pg_password.unwrap_or_default();
            let database = req.pg_database.unwrap_or_default();

            if host.trim().is_empty() {
                return Err("PostgreSQL 主机 (Host) 不能为空".to_string());
            }

            let url = format!(
                "postgresql://{}:{}@{}:{}/{}",
                user, password, host, port, database
            );

            let mut client = postgres::Client::connect(&url, postgres::NoTls)
                .map_err(|e| format!("PostgreSQL 连接失败: {}", e))?;
            
            client.execute("SELECT 1", &[])
                .map_err(|e| format!("PostgreSQL 活性测试 (SELECT 1) 失败: {}", e))?;
            
            Ok("PostgreSQL 连接测试成功！".to_string())
        } else {
            let path_str = req.sqlite_path.unwrap_or_default();
            let path = if path_str.trim().is_empty() {
                crate::db_adapter::get_default_sqlite_path()
            } else {
                std::path::PathBuf::from(path_str)
            };

            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }

            let conn_sqlite = rusqlite::Connection::open(&path)
                .map_err(|e| format!("SQLite 连接失败: {}", e))?;
            
            conn_sqlite.execute("SELECT 1", [])
                .map_err(|e| format!("SQLite 活性测试 (SELECT 1) 失败: {}", e))?;
            
            Ok(format!("SQLite 连接测试成功！\n路径: {}", path.to_string_lossy()))
        }
    }).await;

    match result {
        Ok(Ok(msg)) => {
            let body = serde_json::json!({ "success": true, "message": msg });
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap()
        }
        Ok(Err(err)) => {
            let body = serde_json::json!({ "success": false, "message": err });
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap()
        }
        Err(e) => {
            let body = serde_json::json!({ "success": false, "message": format!("内部线程错误: {}", e) });
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap()
        }
    }
}

pub async fn handle_config_save(
    axum::Json(req): axum::Json<ConfigReq>,
) -> impl axum::response::IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        // 1. 获取旧的配置值 (获取前先确保加载过 dotenv)
        let _ = dotenvy::dotenv();
        let old_db_type = std::env::var("DATABASE_TYPE").unwrap_or_else(|_| "sqlite".to_string());
        let old_sqlite_path = std::env::var("DB_SQLITE_PATH").unwrap_or_default();
        let old_device_name = crate::db::get_device_name();

        let old_pg_host = std::env::var("DB_PG_HOST").unwrap_or_default();
        let old_pg_port = std::env::var("DB_PG_PORT").unwrap_or_default();
        let old_pg_user = std::env::var("DB_PG_USER").unwrap_or_default();
        let old_pg_password = std::env::var("DB_PG_PASSWORD").unwrap_or_default();
        let old_pg_database = std::env::var("DB_PG_DATABASE").unwrap_or_default();

        let new_db_type = req.db_type.trim().to_string();
        let new_sqlite_path = req.sqlite_path.clone().unwrap_or_default().trim().to_string();
        let new_device_name = req.device_name.clone().unwrap_or_default().trim().to_string();
        let new_display_currency = req.display_currency.clone().unwrap_or_else(|| "USD".to_string()).trim().to_string();
        let new_close_behavior = req.close_behavior.clone().unwrap_or_else(|| "prompt".to_string()).trim().to_string();

        let new_pg_host = req.pg_host.clone().unwrap_or_default().trim().to_string();
        let new_pg_port = req.pg_port.clone().unwrap_or_default().trim().to_string();
        let new_pg_user = req.pg_user.clone().unwrap_or_default().trim().to_string();
        let new_pg_password = req.pg_password.clone().unwrap_or_default().trim().to_string();
        let new_pg_database = req.pg_database.clone().unwrap_or_default().trim().to_string();

        // 2. 对比数据库参数是否改变
        let db_type_changed = old_db_type.to_lowercase() != new_db_type.to_lowercase();
        let mut db_params_changed = false;
        if new_db_type.to_lowercase() == "postgres" {
            if old_pg_host != new_pg_host
                || old_pg_port != new_pg_port
                || old_pg_user != new_pg_user
                || old_pg_password != new_pg_password
                || old_pg_database != new_pg_database
            {
                db_params_changed = true;
            }
        } else {
            if old_sqlite_path != new_sqlite_path {
                db_params_changed = true;
            }
        }

        let need_restart = db_type_changed || db_params_changed;

        // 3. 写入 .env 文件
        let env_path = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join(".env");
        
        let mut content = String::new();
        if env_path.exists() {
            content = std::fs::read_to_string(&env_path).unwrap_or_default();
        }

        let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();

        fn set_env_var(lines: &mut Vec<String>, key: &str, val: &str) {
            let prefix = format!("{}=", key);
            let mut found = false;
            for line in lines.iter_mut() {
                if line.trim().starts_with(&prefix) {
                    *line = format!("{}={}", key, val);
                    found = true;
                    break;
                }
            }
            if !found {
                lines.push(format!("{}={}", key, val));
            }
        }

        set_env_var(&mut lines, "DATABASE_TYPE", &new_db_type);
        set_env_var(&mut lines, "DB_SQLITE_PATH", &new_sqlite_path);
        set_env_var(&mut lines, "DEVICE_NAME", &new_device_name);
        set_env_var(&mut lines, "DISPLAY_CURRENCY", &new_display_currency);
        set_env_var(&mut lines, "CLOSE_BEHAVIOR", &new_close_behavior);

        set_env_var(&mut lines, "DB_PG_HOST", &new_pg_host);
        set_env_var(&mut lines, "DB_PG_PORT", &new_pg_port);
        set_env_var(&mut lines, "DB_PG_USER", &new_pg_user);
        set_env_var(&mut lines, "DB_PG_PASSWORD", &new_pg_password);
        set_env_var(&mut lines, "DB_PG_DATABASE", &new_pg_database);
        
        if !new_pg_host.is_empty() {
            let url = format!(
                "postgresql://{}:{}@{}:{}/{}",
                new_pg_user, new_pg_password, new_pg_host, new_pg_port, new_pg_database
            );
            set_env_var(&mut lines, "DATABASE_URL", &url);
        } else {
            set_env_var(&mut lines, "DATABASE_URL", "");
        }

        let new_content = lines.join("\n") + "\n";
        std::fs::write(&env_path, new_content)
            .map_err(|e| format!("写入 .env 配置文件失败: {}", e))?;

        // 4. 立即更新进程内的环境变量以使其在无需重启时立即生效
        std::env::set_var("DATABASE_TYPE", &new_db_type);
        std::env::set_var("DB_SQLITE_PATH", &new_sqlite_path);
        std::env::set_var("DEVICE_NAME", &new_device_name);
        std::env::set_var("DISPLAY_CURRENCY", &new_display_currency);
        std::env::set_var("CLOSE_BEHAVIOR", &new_close_behavior);
        std::env::set_var("DB_PG_HOST", &new_pg_host);
        std::env::set_var("DB_PG_PORT", &new_pg_port);
        std::env::set_var("DB_PG_USER", &new_pg_user);
        std::env::set_var("DB_PG_PASSWORD", &new_pg_password);
        std::env::set_var("DB_PG_DATABASE", &new_pg_database);
        if !new_pg_host.is_empty() {
            let url = format!(
                "postgresql://{}:{}@{}:{}/{}",
                new_pg_user, new_pg_password, new_pg_host, new_pg_port, new_pg_database
            );
            std::env::set_var("DATABASE_URL", &url);
        } else {
            std::env::set_var("DATABASE_URL", "");
        }

        // 5. 如果无需重启且设备名发生了修改，则同步数据库中的设备名记录并刷新大盘趋势缓存
        if !need_restart && old_device_name != new_device_name {
            if let Err(e) = crate::db::update_device_name_in_db(&old_device_name, &new_device_name) {
                eprintln!("[设备配置] 同步更新数据库中设备名报错: {}", e);
                return Err(format!("设备名称保存成功，但同步更新数据库中设备名失败: {}", e));
            }
        }

        let msg = if need_restart {
            "配置已成功保存！为确保新配置生效并避免数据冲突，系统需要重新启动。".to_string()
        } else {
            "配置已成功保存并立即生效，无需重启！".to_string()
        };

        Ok::<(String, bool), String>((msg, need_restart))
    }).await;

    match result {
        Ok(Ok((msg, need_restart))) => {
            let body = serde_json::json!({ "success": true, "message": msg, "need_restart": need_restart });
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap()
        }
        Ok(Err(err)) => {
            let body = serde_json::json!({ "success": false, "message": err });
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap()
        }
        Err(e) => {
            let body = serde_json::json!({ "success": false, "message": format!("内部线程错误: {}", e) });
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap()
        }
    }
}

pub async fn handle_app_restart() -> impl axum::response::IntoResponse {
    // 异步执行重启，给响应留出足够时间返回前端
    tokio::spawn(async {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        println!("[系统重启] 正在重新启动应用...");
        if let Some(app_handle) = crate::db::APP_HANDLE.get() {
            app_handle.restart();
        } else {
            eprintln!("[系统重启] 错误: APP_HANDLE 未被初始化，尝试直接退出程序。");
            std::process::exit(0);
        }
    });

    let body = serde_json::json!({ "success": true, "message": "系统正在重启，请稍候..." });
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

pub async fn handle_db_clean() -> impl axum::response::IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        crate::db::clean_cache_db()
    }).await;

    match result {
        Ok(Ok(msg)) => {
            let body = serde_json::json!({ "success": true, "message": msg });
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap()
        }
        Ok(Err(err)) => {
            let body = serde_json::json!({ "success": false, "message": err });
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap()
        }
        Err(e) => {
            let body = serde_json::json!({ "success": false, "message": format!("内部线程错误: {}", e) });
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap()
        }
    }
}

#[derive(serde::Serialize)]
pub struct ModelPricingResp {
    pub rows: Vec<crate::db::ModelPricingRow>,
    pub display_currency: String,
}

pub async fn handle_model_pricing_get() -> impl axum::response::IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let rows = crate::db::list_model_pricing_rows()?;
        let display_currency = std::env::var("DISPLAY_CURRENCY")
            .unwrap_or_else(|_| "USD".to_string())
            .to_uppercase();
        Ok::<_, rusqlite::Error>(ModelPricingResp { rows, display_currency })
    }).await;

    match result {
        Ok(Ok(data)) => {
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from(serde_json::to_vec(&data).unwrap()))
                .unwrap()
        }
        _ => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .body(Body::from("Internal Server Error"))
            .unwrap(),
    }
}

pub async fn handle_model_pricing_save(
    axum::Json(req): axum::Json<Vec<crate::db::ModelPricingRow>>,
) -> impl axum::response::IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        crate::db::upsert_model_pricing_rows(&req)?;
        Ok::<_, rusqlite::Error>(())
    }).await;

    match result {
        Ok(Ok(())) => {
            let body = serde_json::json!({ "success": true, "message": "模型费率配置已成功保存并重新计算历史成本！" });
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap()
        }
        _ => {
            let body = serde_json::json!({ "success": false, "message": "保存模型费率配置失败" });
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap()
        }
    }
}

pub async fn handle_exchange_rate_refresh() -> impl axum::response::IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let conn = rusqlite::Connection::open(crate::db::get_db_cache_path())?;
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute("INSERT OR REPLACE INTO exchange_rates (currency_code, rate_from_usd, updated_at) VALUES ('CNY', 7.24, ?)", [&now])?;
        conn.execute("INSERT OR REPLACE INTO exchange_rates (currency_code, rate_from_usd, updated_at) VALUES ('JPY', 155.4, ?)", [&now])?;
        conn.execute("INSERT OR REPLACE INTO exchange_rates (currency_code, rate_from_usd, updated_at) VALUES ('EUR', 0.92, ?)", [&now])?;
        Ok::<_, rusqlite::Error>(())
    }).await;

    match result {
        Ok(Ok(())) => {
            let body = serde_json::json!({ "success": true, "message": "汇率数据已成功更新为最新模拟汇率（CNY=7.24, JPY=155.4, EUR=0.92）" });
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap()
        }
        _ => {
            let body = serde_json::json!({ "success": false, "message": "刷新汇率数据失败" });
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap()
        }
    }
}

// ==================== CORS 全局跨域与预检中间件 ====================
pub async fn cors_middleware(
    request: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> impl axum::response::IntoResponse {
    let method = request.method().clone();
    
    // 如果是 OPTIONS 预检请求，直接返回支持 CORS 的响应，不执行后续 Handler
    if method == axum::http::Method::OPTIONS {
        return axum::http::Response::builder()
            .status(axum::http::StatusCode::OK)
            .header(axum::http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .header(axum::http::header::ACCESS_CONTROL_ALLOW_METHODS, "GET, POST, PUT, DELETE, OPTIONS")
            .header(axum::http::header::ACCESS_CONTROL_ALLOW_HEADERS, "Content-Type, Authorization")
            .body(axum::body::Body::empty())
            .unwrap();
    }

    // 调用后续 Handler 处理
    let mut response = next.run(request).await;
    
    // 如果响应头里没有 Access-Control-Allow-Origin，则补上
    if !response.headers().contains_key(axum::http::header::ACCESS_CONTROL_ALLOW_ORIGIN) {
        response.headers_mut().insert(
            axum::http::header::ACCESS_CONTROL_ALLOW_ORIGIN,
            axum::http::HeaderValue::from_static("*"),
        );
    }
    
    response
}

