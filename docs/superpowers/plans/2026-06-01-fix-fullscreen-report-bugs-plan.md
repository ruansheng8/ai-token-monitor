# 修复全屏复盘报告窗口无数据及无法关闭的 Bug 实现计划

本项目旨在解决 Token Insight 中“全屏查看”报告时没有数据，以及程序退出/隐藏后独立弹出的报告窗口依然残留且无法通过原生右上角关闭按钮或界面关闭按钮关闭的问题。

## Proposed Changes

### 前端组件优化

---

#### [MODIFY] [ReviewDrawer.tsx](file:///d:/VibeCoding/ai-token-monitor/src/components/ReviewDrawer.tsx)
- 在点击「全屏查看 ↗」时，重新加入 `localStorage.setItem('fullscreen_task_id', activeTask.id)`，确保数据能稳定可靠地在多窗口之间共享。

```diff
                        try {
+                          localStorage.setItem('fullscreen_task_id', activeTask.id);
                           await invoke('open_fullscreen_window', { taskId: activeTask.id });
```

#### [MODIFY] [FullscreenReportViewer.tsx](file:///d:/VibeCoding/ai-token-monitor/src/components/FullscreenReportViewer.tsx)
- 在组件挂载时，优先同步获取 `localStorage` 中的 `fullscreen_task_id` 并设置状态。
- 如果 `localStorage` 中不存在该值，则挂载监听器等待跨窗口事件广播作为 Fallback 兜底。

```diff
    // 如果初始 taskId 为空，则等待 Tauri 事件推送
    if (!initialTaskId) {
+      const cachedTaskId = localStorage.getItem('fullscreen_task_id');
+      if (cachedTaskId) {
+        setResolvedTaskId(cachedTaskId);
+      }
+
       setupListener();
```

### Rust 后端修复

---

#### [MODIFY] [main.rs](file:///d:/VibeCoding/ai-token-monitor/src-tauri/src/main.rs)
- 在 `on_window_event` 闭包中的 `CloseRequested` 处理里，如果是非 `main` 窗口（即 `fullscreen-report` 等辅助窗口），显式调用 `window.destroy()` 强制释放原生窗口及其绑定的 WebView 资源，彻底避免关闭卡死无反应的异常。

```diff
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
-               }
-               // fullscreen-report 窗口直接允许关闭，不做任何拦截
+               } else {
+                   // 显式销毁其他辅助窗口，确保能被正常关闭
+                   let _ = window.destroy();
+               }
            }
        })
```

## Verification Plan

### 自动化验证与构建
- 在项目前端执行 TypeScript 校验与前端打包，验证无编译报错：
  ```bash
  npx tsc -b --noEmit
  npm run build
  ```
- 在后端目录验证 Rust 代码编译无误：
  ```bash
  cd src-tauri
  cargo check
  ```

### 手动验证步骤
1. 启动项目，进入 `AI 复盘与治理中心`，生成或选择一份复盘报告。
2. 点击报告标题右上角的 `全屏查看` 按钮。
3. 检查独立弹出的亮色全屏查看窗口中是否能秒级且成功地渲染出 Markdown 复盘报告内容。
4. 在全屏窗口中，分别点击界面右上角的原生关闭（X）按钮以及界面内部的 `关闭` 按钮，确认窗口是否能干净且无卡顿地立刻关闭。
5. 关闭或隐藏主窗口，在全屏窗口中再次测试关闭行为，确认无残留、无卡死。
