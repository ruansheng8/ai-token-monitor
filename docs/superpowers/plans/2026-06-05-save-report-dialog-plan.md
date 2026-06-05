# 实现计划：保存报表图片对话框选择目录与保存

本项目采用 React 19 + Tauri v2 + Axum 双模架构。我们将引入 `rfd` 与 `base64` 库，在 Axum 后端提供图片保存及文件浏览服务，并优化前端“保存图片报表”的交互流程。

## 变更文件列表

### 后端依赖
- [MODIFY] [Cargo.toml](file:///d:/VibeCoding/ai-token-monitor/src-tauri/Cargo.toml) (引入 `rfd` 和 `base64`)

### 后端代码
- [MODIFY] [main.rs](file:///d:/VibeCoding/ai-token-monitor/src-tauri/src/main.rs) (注册 `/api/report/save` 与 `/api/report/open` 路由)
- [MODIFY] [server.rs](file:///d:/VibeCoding/ai-token-monitor/src-tauri/src/server.rs) (实现 `handle_report_save` 和 `handle_report_open` 函数)

### 前端代码
- [MODIFY] [App.tsx](file:///d:/VibeCoding/ai-token-monitor/src/App.tsx) (修改保存图片按钮，对接后端接口，增加保存成功后的二次确认并调用打开文件夹)

---

## 详细变更方案

### 1. 后端依赖配置 `src-tauri/Cargo.toml`
在 `[dependencies]` 下添加：
```toml
rfd = "0.15"
base64 = "0.22.1"
```

### 2. 后端服务实现 `src-tauri/src/server.rs`
新增用于接收 Base64 并在本地唤起对话框保存的接口处理器，以及在文件资源管理器中打开并高亮定位文件的处理器：
```rust
#[derive(serde::Deserialize)]
pub struct ReportSaveReq {
    pub image_base64: String,
}

#[derive(serde::Deserialize)]
pub struct ReportOpenReq {
    pub path: String,
}

pub async fn handle_report_save(
    axum::Json(req): axum::Json<ReportSaveReq>,
) -> impl axum::response::IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        // 1. 动态读取用户 Downloads 目录
        let default_dir = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .map(|p| std::path::PathBuf::from(p).join("Downloads"))
            .unwrap_or_else(|_| std::path::PathBuf::from("."));

        // 2. 唤起原生文件夹选择框
        let folder = rfd::FileDialog::new()
            .set_directory(&default_dir)
            .pick_folder();

        let path = match folder {
            Some(p) => p,
            None => return Ok(serde_json::json!({ "success": false, "cancelled": true })),
        };

        // 3. 构建不重名的默认文件名
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let mut file_path = path.join(format!("Token_Insight_图片报表_{}.png", today));
        let mut counter = 1;
        while file_path.exists() {
            file_path = path.join(format!("Token_Insight_图片报表_{}_{}.png", today, counter));
            counter += 1;
        }

        // 4. 解析并解码 base64
        let base64_str = if req.image_base64.contains(',') {
            req.image_base64.split(',').nth(1).unwrap_or(&req.image_base64)
        } else {
            &req.image_base64
        };

        use base64::{Engine as _, engine::general_purpose::STANDARD};
        let bytes = STANDARD.decode(base64_str)
            .map_err(|e| format!("图片 Base64 解码失败: {}", e))?;

        // 5. 写入文件
        std::fs::write(&file_path, bytes)
            .map_err(|e| format!("图片文件写入磁盘失败: {}", e))?;

        Ok(serde_json::json!({
            "success": true,
            "file_path": file_path.to_string_lossy().to_string()
        }))
    }).await;

    // 返回 JSON 响应
    // ...
}

pub async fn handle_report_open(
    axum::Json(req): axum::Json<ReportOpenReq>,
) -> impl axum::response::IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let path = std::path::Path::new(&req.path);
        if !path.exists() {
            return Err("文件路径不存在".to_string());
        }

        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("explorer")
                .arg(format!("/select,\"{}\"", req.path))
                .spawn()
                .map_err(|e| e.to_string())?;
        }

        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open")
                .arg("-R")
                .arg(&req.path)
                .spawn()
                .map_err(|e| e.to_string())?;
        }

        #[cfg(target_os = "linux")]
        {
            if let Some(parent) = path.parent() {
                std::process::Command::new("xdg-open")
                    .arg(parent)
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
        }

        Ok(())
    }).await;

    // 返回 JSON 响应
    // ...
}
```

### 3. 后端路由配置 `src-tauri/src/main.rs`
在 `axum::Router::new()` 中添加路由：
```rust
.route("/api/report/save", post(handle_report_save))
.route("/api/report/open", post(handle_report_open))
```
并在头部引入 `handle_report_save, handle_report_open`。

### 4. 前端交互修改 `src/App.tsx`
在 `src/App.tsx` 中定义 `isSavingReport` 状态，用于控制保存按钮的禁用与加载提示。
重构 "📥 保存图片报表" 按钮的 `onClick` 方法：
- 调用后端 `/api/report/save` 进行保存。
- 若保存成功，弹出提示问用户是否立即在文件夹中打开该图片。
- 若用户确认，发送请求到后端 `/api/report/open` 打开文件夹。

---

## 验证计划

### 编译验证
- 前往 `src-tauri` 目录执行 `cargo check` 确保后端编译无报错。
- 前端运行 `tsc -b --noEmit` 确保无 TypeScript 类型错误。

### 功能验证
1. 点击“生成图片报表”大盘，进入生成成功预览弹窗。
2. 点击“📥 保存图片报表”按钮，应弹出原生文件选择对话框。
3. 对话框默认定位到 `%USERPROFILE%\Downloads`。
4. 选择目录并点击确认：
   - 检查该目录下是否已生成 `Token_Insight_图片报表_YYYY-MM-DD.png`。
   - 页面应弹出 confirm 二次确认弹窗，显示保存的完整路径，询问是否打开对应文件夹。
5. 点击确认：
   - 验证是否弹出系统资源管理器，并且高亮选中刚才保存的文件。
6. 重复保存，验证多次保存是否会自动重命名为 `(1)`, `(2)` 而非直接覆盖已存文件。
