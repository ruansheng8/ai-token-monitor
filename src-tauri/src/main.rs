#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod proto;
mod db;
mod server;
mod db_adapter;

use axum::{routing::{get, post}, Router};
use server::{handle_metrics, handle_scan_start, handle_scan_status, serve_static_file_fallback, handle_config_get, handle_config_test, handle_config_save, handle_app_restart, handle_db_clean, handle_sessions_paginated};
use std::path::Path;
use notify::{Watcher, RecursiveMode, Event};
use tauri::Manager;

fn start_folder_watcher() {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<()>(100);

    // 1. 在当前 Axum Tokio 运行时中启动异步防抖任务
    tokio::spawn(async move {
        let mut debounce_timer: Option<tokio::time::Instant> = None;
        loop {
            tokio::select! {
                _ = rx.recv() => {
                    let debounce_ms = std::env::var("HOTSYNC_DEBOUNCE_MS")
                        .ok()
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(5000); // 默认调整为 5 秒
                    debounce_timer = Some(tokio::time::Instant::now() + tokio::time::Duration::from_millis(debounce_ms));
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                    if let Some(deadline) = debounce_timer {
                        if tokio::time::Instant::now() >= deadline {
                            debounce_timer = None;
                            println!("[热同步] 检测到物理文件写入变动，防抖结束，开始执行增量更新...");
                            db::start_background_scan();
                        }
                    }
                }
            }
        }
    });

    // 2. 启动系统物理文件变动监听线程 (notify watcher)
    std::thread::spawn(move || {
        let tx_clone = tx.clone();
        let mut watcher = match notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                if event.kind.is_modify() || event.kind.is_create() {
                    let _ = tx_clone.blocking_send(());
                }
            }
        }) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("Failed to initialize notify folder watcher: {}", e);
                return;
            }
        };

        let profile_dir = db::get_user_profile_dir();

        // 1) 监听 Gemini 会话目录
        let gemini_dir = Path::new(&profile_dir).join(".gemini").join("antigravity").join("conversations");
        if gemini_dir.exists() {
            let _ = watcher.watch(&gemini_dir, RecursiveMode::Recursive);
        }

        // 2) 监听 Claude Code 项目目录
        let claude_dir = Path::new(&profile_dir).join(".claude").join("projects");
        if claude_dir.exists() {
            let _ = watcher.watch(&claude_dir, RecursiveMode::Recursive);
        }

        // 3) 监听 Codex 会话目录
        let codex_dir = Path::new(&profile_dir).join(".codex").join("sessions");
        if codex_dir.exists() {
            let _ = watcher.watch(&codex_dir, RecursiveMode::Recursive);
        }

        // 4) 监听 Cursor 数据库文件
        let cursor_db = db::get_cursor_db_path();
        if cursor_db.exists() {
            let _ = watcher.watch(&cursor_db, RecursiveMode::NonRecursive);
        }

        println!("[热同步] 后台文件实时自动检测监听服务已在后台成功运行！");

        loop {
            std::thread::sleep(std::time::Duration::from_secs(60));
        }
    });
}

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

    // 1. 在后台线程启动 Tokio 运行时来承载 Axum API 及其监测任务
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async {
            let app = Router::new()
                .route("/api/metrics", get(handle_metrics))
                .route("/api/sessions", get(handle_sessions_paginated))
                .route("/api/scan/start", get(handle_scan_start).post(handle_scan_start))
                .route("/api/scan/status", get(handle_scan_status))
                .route("/api/config", get(handle_config_get))
                .route("/api/config/test", post(handle_config_test))
                .route("/api/config/save", post(handle_config_save))
                .route("/api/app/restart", post(handle_app_restart))
                .route("/api/db/clean", post(handle_db_clean).get(handle_db_clean))
                .fallback(serve_static_file_fallback);

            // 启动文件监测与热同步服务
            start_folder_watcher();

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
        .setup(|app| {
            // 设置全局 AppHandle 以便后台扫描任务成功时可以 emit 广播热更新事件给前端
            db::APP_HANDLE.set(app.handle().clone()).ok();

            // 创建系统托盘，使得左键点击时能重新唤起大盘窗口
            if let Some(icon) = app.default_window_icon().cloned() {
                let _tray = tauri::tray::TrayIconBuilder::new()
                    .icon(icon)
                    .on_tray_icon_event(|tray, event| {
                        if let tauri::tray::TrayIconEvent::Click {
                            button: tauri::tray::MouseButton::Left,
                            ..
                        } = event
                        {
                            let app = tray.app_handle();
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    })
                    .build(app);
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // 拦截关闭请求并隐藏窗口
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
