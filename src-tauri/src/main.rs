#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod proto;
mod db;
mod server;
mod db_adapter;
mod review;
mod config;

use axum::{routing::{get, post}, Router};
use server::{
    handle_metrics, handle_scan_start, handle_scan_status, serve_static_file_fallback,
    handle_config_get, handle_config_test, handle_config_save, handle_app_restart,
    handle_db_clean, handle_sessions_paginated, handle_model_pricing_get,
    handle_model_pricing_save, handle_exchange_rate_refresh,
};
use review::{
    handle_review_detect, handle_create_task, handle_list_tasks,
    handle_get_active_task, handle_get_task, handle_task_events, handle_cancel_task,
    handle_delete_task, handle_retry_task, handle_save_action_items, handle_save_quality_feedback,
    handle_get_turn_details,
};
use std::path::Path;
use notify::{Watcher, RecursiveMode, Event};
use tauri::{Manager, Emitter};

fn start_folder_watcher() {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<()>(100);

    // 1. 在当前 Axum Tokio 运行时中启动异步防抖任务
    tokio::spawn(async move {
        let mut debounce_timer: Option<tokio::time::Instant> = None;
        loop {
            tokio::select! {
                _ = rx.recv() => {
                    let policy = db::current_hot_sync_policy();
                    let mut debounce_ms = policy.delay_ms;
                    if debounce_ms == 5000 {
                        if let Some(env_ms) = std::env::var("HOTSYNC_DEBOUNCE_MS")
                            .ok()
                            .and_then(|s| s.parse::<u64>().ok())
                        {
                            debounce_ms = env_ms;
                        }
                    }
                    debounce_timer = Some(tokio::time::Instant::now() + tokio::time::Duration::from_millis(debounce_ms));
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                    if let Some(deadline) = debounce_timer {
                        if tokio::time::Instant::now() >= deadline {
                            let policy = db::current_hot_sync_policy();
                            if policy.delay_ms >= 60000 {
                                println!("[热同步] 检测到系统处于高负载 (CPU {:.1}%)，将热同步延迟推迟 {} ms", policy.cpu_usage, policy.delay_ms);
                                debounce_timer = Some(tokio::time::Instant::now() + tokio::time::Duration::from_millis(policy.delay_ms));
                            } else {
                                debounce_timer = None;
                                db::start_background_scan(true);
                            }
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
    // 启动时优先初始化配置和环境变量注入
    config::init_config().expect("初始化配置文件失败");
    // 启动时初始化本地缓存数据库，确保表结构完备，避免多请求并发竞争初始化导致的数据库锁死
    let _ = db::init_cache_db();
    let _ = review::recover_interrupted_tasks();

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
                .route("/api/model-pricing", get(handle_model_pricing_get).post(handle_model_pricing_save))
                .route("/api/exchange-rates/refresh", post(handle_exchange_rate_refresh))
                .route("/api/review/detect", get(handle_review_detect))
                .route("/api/review/tasks", get(handle_list_tasks).post(handle_create_task))
                .route("/api/review/tasks/active", get(handle_get_active_task))
                .route("/api/review/tasks/:id", get(handle_get_task).delete(handle_delete_task))
                .route("/api/review/tasks/:id/events", get(handle_task_events))
                .route("/api/review/tasks/:id/cancel", post(handle_cancel_task))
                .route("/api/review/tasks/:id/retry", post(handle_retry_task))
                .route("/api/review/tasks/:id/action-items", post(handle_save_action_items))
                .route("/api/review/tasks/:id/feedback", post(handle_save_quality_feedback))
                .route("/api/review/turns/details", get(handle_get_turn_details))
                .fallback(serve_static_file_fallback)
                .layer(axum::middleware::from_fn(server::cors_middleware));

            // 启动文件监测与热同步服务
            start_folder_watcher();

            // 本地桌面版绑定本地回环地址 127.0.0.1
            let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
            println!("\n==================================================");
            println!(" Token Insight 极速增量缓存用量统计后台服务已成功启动！");
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
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .invoke_handler(tauri::generate_handler![exit_app, hide_window])
        .setup(|app| {
            // 设置全局 AppHandle 以便后台扫描任务成功时可以 emit 广播热更新事件给前端
            db::APP_HANDLE.set(app.handle().clone()).ok();

            // 创建系统托盘，使得左键点击时能重新唤起大盘窗口并支持右键退出菜单
            if let Some(icon) = app.default_window_icon().cloned() {
                use tauri::menu::{Menu, MenuItem};
                let quit_i = MenuItem::with_id(app, "quit", "退出程序", true, None::<&str>)?;
                let show_i = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
                let tray_menu = Menu::with_items(app, &[&show_i, &quit_i])?;

                let _tray = tauri::tray::TrayIconBuilder::new()
                    .icon(icon)
                    .menu(&tray_menu)
                    .on_menu_event(|app, event| {
                        match event.id.as_ref() {
                            "quit" => {
                                app.exit(0);
                            }
                            "show" => {
                                if let Some(window) = app.get_webview_window("main") {
                                    let _ = window.show();
                                    let _ = window.set_focus();
                                }
                            }
                            _ => {}
                        }
                    })
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
                let label = window.label();
                if label == "main" {
                    let behavior = std::env::var("CLOSE_BEHAVIOR").unwrap_or_else(|_| "prompt".to_string());
                    match behavior.as_str() {
                        "close" => {
                            window.app_handle().exit(0);
                        }
                        "minimize" => {
                            api.prevent_close();
                            let _ = window.hide();
                        }
                        _ => {
                            // 阻止默认关闭，并通知前端弹窗
                            api.prevent_close();
                            let _ = window.emit("close-requested", ());
                        }
                    }
                } else {
                    // 显式销毁其他辅助窗口（如 fullscreen-report），确保其能正常关闭，避免卡死或无响应
                    let _ = window.destroy();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
fn exit_app(app_handle: tauri::AppHandle) {
    app_handle.exit(0);
}

#[tauri::command]
fn hide_window(window: tauri::Window) {
    let _ = window.hide();
}



