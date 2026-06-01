# 智能复盘分析诊断报告全屏查看设计规范 (Tauri 多窗口亮色模式)

本文档定义了 Token Insight 智能复盘报告全屏查看功能的详细设计规范，采用 Tauri 后端多窗口加前端亮色渲染的架构方案。

## 1. 业务背景与用户痛点
用户在生成 AI 智能复盘分析诊断报告后，需要在应用内部精读长篇的 Markdown 内容。由于目前的报告嵌入在抽屉式（Drawer）详情面板中，宽度受限且排布紧凑，用户阅读体感较差。为提升阅读效能，提供一个独立的、不含任何其他多余 UI（如进度条、大盘快照、日志终端、待办清单等）的全屏查看模式是必不可少的。

## 2. 核心交互方案：方案 B (Tauri 原生多窗口弹出)
- 用户在主窗口的复盘报告详情页面中，点击报告标题右上角新增的 **「全屏查看 ↗」** 按钮。
- 应用保存当前选中报告的任务 ID，并在后台通过 Tauri 命令拉起一个新的独立原生系统窗口，命名为 `fullscreen-report`。
- 新窗口加载轻量、纯净的 **亮色 (Light Mode) 报告渲染页面**。
- 支持双屏并行开发体验：用户可将报告窗口拖拽至副屏，主窗口继续进行其他用量监控或参数调整。

## 3. 详细设计说明

### 3.1 界面设计 (整体采用亮色主题)
- **背景与排版**：采用极简亮色微光渐变背景（`radial-gradient(circle at top, #f8fafc 0%, #ffffff 100%)`）。文字使用 Slate-900（`#0f172a`），正文字体使用 Inter/System-ui，表格和代码数值使用等宽字体（Monospace），保证对齐性。
- **顶栏操作栏**：
  - 按钮包含：📋 复制全文、📥 导出 MD、🖨️ 打印 PDF、✕ 关闭窗口。
  - 使用亮色拟态按钮样式（`bg-slate-100 border-slate-200 hover:bg-slate-200`），关闭按钮使用红色调警示色（`bg-rose-50 border-rose-200 text-rose-600`）。
- **内容布局**：正文限制最大宽度为 `max-w-4xl`（约 850px），水平居中，提供极佳的行长与段落行高（`line-height: 1.8`），完美适配 Markdown 各级标题、无序/有序列表、表格、代码块以及引用。

### 3.2 跨窗口数据共享与 React 路由设计
- **本地存储过渡**：
  - 前端点击「全屏查看」时，写入本地存储：`localStorage.setItem('fullscreen_task_id', activeTask.id)`。
  - 随后触发 Rust 命令：`invoke("open_fullscreen_window")`。
- **React 入口渲染分流**：
  - 在 `src/App.tsx` 或 `src/main.tsx` 的最顶层，检测当前 WebView 窗口的 Label（利用 `@tauri-apps/api/webviewWindow` 的 `getCurrentWebviewWindow().label`）。
  - 如果 `label === 'fullscreen-report'`，直接截断主应用（Dashboard）渲染，只挂载 `<FullscreenReportViewer />` 子组件。
  - 如果 `label === 'main'`，则按常规流程渲染 Token Insight 主控制台。
- **全屏组件数据拉取**：
  - `<FullscreenReportViewer />` 从 `localStorage` 获取 `fullscreen_task_id`，若不存在则提示错误。
  - 异步请求 Axum 接口 `/api/review/tasks/:id`，拉取对应任务的标题、CLI 名称和 `output_markdown` 数据并进行本地状态渲染。

### 3.3 Tauri 原生配置适配
- **权限与能力声明**：
  - 在 `src-tauri/capabilities/default.json` 的 `windows` 列表中添加 `"fullscreen-report"`，赋予该窗口相同的 Tauri API 调用权限（如调用窗口关闭功能）。
- **窗口参数**：
  - 新窗口默认尺寸：`1000px * 750px`。
  - 允许缩放（`resizable: true`）、无默认暗黑限制、设置合适的最小宽度（如 `600px`）。

## 4. 接口设计

### 4.1 Rust 后端 command
在 `src-tauri/src/main.rs` 注册以下接口：
```rust
#[tauri::command]
fn open_fullscreen_window(app_handle: tauri::AppHandle) -> Result<(), String> {
    let window_label = "fullscreen-report";
    
    // 如果已有旧全屏窗口则关闭，保证重新加载
    if let Some(w) = app_handle.get_webview_window(window_label) {
        let _ = w.close();
    }
    
    let _ = tauri::WebviewWindowBuilder::new(
        &app_handle,
        window_label,
        tauri::WebviewUrl::App("index.html".into())
    )
    .title("智能复盘分析诊断报告")
    .inner_size(1000.0, 750.0)
    .min_inner_size(600.0, 500.0)
    .resizable(true)
    .build()
    .map_err(|e| e.to_string())?;
    
    Ok(())
}
```

### 4.2 前端 API 调用与生命周期
全屏窗口中，点击「关闭窗口」：
```typescript
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
const closeWindow = () => {
  getCurrentWebviewWindow().close();
};
```
