#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod proto;
mod db;
mod server;

use axum::{routing::get, Router};
use server::{handle_metrics, handle_scan_start, handle_scan_status, serve_static_file_fallback};

fn main() {
    // 启动时初始化本地缓存数据库，确保表结构完备，避免多请求并发竞争初始化导致的数据库锁死
    let _ = db::init_cache_db();

    let mut port = 19362;
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        if let Ok(p) = args[1].parse::<u16>() {
            port = p;
        }
    }

    // 1. 在后台线程启动 Tokio 运行时来承载 Axum API
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async {
            let app = Router::new()
                .route("/api/metrics", get(handle_metrics))
                .route("/api/scan/start", get(handle_scan_start).post(handle_scan_start))
                .route("/api/scan/status", get(handle_scan_status))
                .fallback(serve_static_file_fallback);

            // 本地桌面版绑定本地回环地址 127.0.0.1
            let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
            println!("\n==================================================");
            println!(" AI Token Monitor 极速增量缓存用量统计后台服务已成功启动！");
            println!(" 接口地址: http://127.0.0.1:{}", port);
            println!("==================================================\n");

            let listener = match tokio::net::TcpListener::bind(addr).await {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("Error binding to port {}: {}", port, e);
                    return;
                }
            };

            if let Err(e) = axum::serve(listener, app).await {
                eprintln!("Server error: {}", e);
            }
        });
    });

    // 稍微等待后台服务完成端口绑定
    std::thread::sleep(std::time::Duration::from_millis(400));

    // 2. 启动 Tauri 应用
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
