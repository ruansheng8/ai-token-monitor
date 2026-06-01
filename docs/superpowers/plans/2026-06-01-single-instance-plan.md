# 2026-06-01 单实例模式与窗口激活实现计划

本文档定义了防止 AI Token Monitor 桌面端应用被多次启动，并在重复启动时自动激活已运行实例的实现计划。

## 变更内容

### 后端依赖变更
#### [MODIFY] [Cargo.toml](file:///d:/VibeCoding/ai-token-monitor/src-tauri/Cargo.toml)
- 添加 `tauri-plugin-single-instance = "2.0.0"` 到 `[dependencies]`。

### 后端逻辑变更
#### [MODIFY] [main.rs](file:///d:/VibeCoding/ai-token-monitor/src-tauri/src/main.rs)
- 引入单实例插件注册逻辑。
- 捕获重复启动信号并获取 `"main"` 窗口。
- 依次调用 `show()`、`unminimize()` 和 `set_focus()` 激活主窗口。

## 验证计划
1. **编译检查**：
   - 进入 `src-tauri` 目录执行 `cargo check`。
2. **构建与测试**：
   - 运行本地构建：`pnpm build` 后在 `src-tauri` 执行 `cargo build --release`。
   - 启动生成的 `.exe` 实体。
   - 最小化或关闭主窗口到托盘。
   - 再次双击打开生成的 `.exe`，观察是否能正常唤醒并聚焦前台，同时第二个实例自动退出。
