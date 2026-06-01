# 2026-06-01 单实例模式与窗口激活设计规范

为了优化用户体验，防止 AI Token Monitor 桌面端应用被多次重复启动导致资源浪费，同时在重复启动时能够自动将已运行的程序窗口唤醒并置于前台，本项目需要实现单实例运行模式。

## 需求背景
目前用户重复双击启动程序时，会创建多个独立的 Tauri 窗口以及启动多个本地后台 Axum 端口，这不仅会导致端口绑定冲突报错，还会导致数据读写混乱（如 SQLite 数据库锁死）。因此，需要限制程序仅允许开启一个实例，并在二次启动时将已有的主窗口唤醒聚焦。

## 方案设计
我们采用 Tauri 2.0 官方提供的 `tauri-plugin-single-instance` 插件实现该功能。该插件基于操作系统的本地命名套接字（Named Pipe/Unix Domain Socket）在进程间通信，能以极低的延迟在启动初期完成单实例检测。

### 1. 依赖变更
在 `src-tauri/Cargo.toml` 中引入 `tauri-plugin-single-instance`：
```toml
[dependencies]
tauri-plugin-single-instance = "2.0.0"
```

### 2. 核心逻辑实现
在 `src-tauri/src/main.rs` 的 `tauri::Builder::default()` 初始化链中，优先注册单实例插件：

```rust
tauri::Builder::default()
    .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
        // 当第二个实例试图启动时，它会向第一个实例发送信号，并触发此回调
        if let Some(window) = app.get_webview_window("main") {
            // 1. 显示窗口（防止被隐藏在系统托盘）
            let _ = window.show();
            // 2. 恢复窗口状态（防止处于最小化状态）
            let _ = window.unminimize();
            // 3. 将窗口聚焦并置顶
            let _ = window.set_focus();
        }
    }))
    // ... 后续的其他插件和配置 ...
```

### 3. 注意事项与自检
- **窗口标识符**：确认主窗口的 Label 为 `"main"`。经查 `tauri.conf.json` 中 `windows` 列表第一个窗口为默认主窗口（即 `"main"`），且在 `main.rs` 的系统托盘事件中也是通过 `app.get_webview_window("main")` 来获取并显示的，标识符完全一致。
- **初始化顺序**：必须在 `tauri::Builder` 链的第一位注册 `tauri-plugin-single-instance` 插件，确保在其余系统组件或长耗时 setup 初始化之前拦截到重复启动信号。

## 验证方案
1. 编译并打包应用。
2. 运行第一个实例，主面板正常显示。
3. 最小化或关闭主面板（隐藏至系统托盘）。
4. 尝试再次双击运行应用。
5. **预期结果**：第二个实例窗口未打开且自动退出，已有的第一个实例面板自动弹出、恢复到正常大小并处于系统最前台聚焦状态。
