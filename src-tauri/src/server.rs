use std::path::PathBuf;
use axum::{
    body::Body,
    http::{header, Response, StatusCode, Uri},
    response::IntoResponse,
};

use crate::db::{
    get_aggregated_metrics_from_cache, start_background_scan, get_scan_status,
};

pub async fn handle_metrics(
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Response<Body> {
    let source = params.get("source").cloned();
    let start_date = params.get("start_date").cloned();
    let end_date = params.get("end_date").cloned();
    match tokio::task::spawn_blocking(move || {
        get_aggregated_metrics_from_cache(source.as_deref(), start_date.as_deref(), end_date.as_deref())
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
        start_background_scan();
        
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

#[derive(serde::Deserialize)]
pub struct ConfigReq {
    pub db_type: String,
    pub sqlite_path: Option<String>,
    pub pg_url: Option<String>,
}

pub async fn handle_config_test(
    axum::Json(req): axum::Json<ConfigReq>,
) -> impl axum::response::IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        if req.db_type.to_lowercase() == "postgres" {
            let url = req.pg_url.unwrap_or_default();
            if url.trim().is_empty() {
                return Err("PostgreSQL 连接串不能为空".to_string());
            }
            let client = postgres::Client::connect(&url, postgres::NoTls)
                .map_err(|e| format!("PostgreSQL 连接失败: {}", e))?;
            
            let conn = crate::db_adapter::DbConn::Postgres(std::sync::Mutex::new(client));
            crate::db_adapter::init_tables(&conn)
                .map_err(|e| format!("表结构校验/初始化失败: {}", e))?;
            
            Ok("PostgreSQL 连接测试成功，且空库表结构已校验/初始化完毕！".to_string())
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
            
            let conn = crate::db_adapter::DbConn::Sqlite(std::sync::Mutex::new(conn_sqlite));
            crate::db_adapter::init_tables(&conn)
                .map_err(|e| format!("表结构校验/初始化失败: {}", e))?;
            
            Ok(format!("SQLite 连接测试成功，且表结构已校验/初始化完毕！\n路径: {}", path.to_string_lossy()))
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

        set_env_var(&mut lines, "DATABASE_TYPE", &req.db_type);
        if let Some(path) = &req.sqlite_path {
            set_env_var(&mut lines, "DB_SQLITE_PATH", path);
        } else {
            set_env_var(&mut lines, "DB_SQLITE_PATH", "");
        }
        if let Some(url) = &req.pg_url {
            set_env_var(&mut lines, "DATABASE_URL", url);
        } else {
            set_env_var(&mut lines, "DATABASE_URL", "");
        }

        let new_content = lines.join("\n") + "\n";
        std::fs::write(&env_path, new_content)
            .map_err(|e| format!("写入 .env 配置文件失败: {}", e))?;

        // 触发热连接池重载
        crate::db_adapter::reset_conn_pool();

        Ok::<String, String>("配置参数已成功写入 .env，且后端数据库连接已热重载生效！".to_string())
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

