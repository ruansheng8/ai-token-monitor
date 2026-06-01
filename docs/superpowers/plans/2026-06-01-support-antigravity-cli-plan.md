# 2026-06-01 支持 Antigravity CLI (agy.exe) 实现计划

我们将根据设计规范，在项目的前后端分别引入对 Antigravity CLI 的支持。

## 待修改的文件

1. **后端 (Rust)**:
   - `src-tauri/src/review.rs`

2. **前端 (React)**:
   - `src/components/ReviewDrawer.tsx`

## 详细步骤

### 1. 修改后端代码 (review.rs)
- 在 [src-tauri/src/review.rs](file:///d:/VibeCoding/ai-token-monitor/src-tauri/src/review.rs) 中：
  - 更新 `candidate_bins` 静态数组或局部变量，追加 `"agy"`。
  - 在 `get_cli_display_name` 匹配中增加 `"agy" => "Antigravity CLI (新版)"`，并将 `"gemini"` 修改为 `"Gemini CLI (旧版)"`。
  - 在 `run_cli_task_background` 的 `Command` 参数判定逻辑中，当为 `"agy"` 时不传递 `"-p"` 参数。

### 2. 修改前端代码 (ReviewDrawer.tsx)
- 在 [src/components/ReviewDrawer.tsx](file:///d:/VibeCoding/ai-token-monitor/src/components/ReviewDrawer.tsx) 中：
  - 更新 `getCliDisplayName` 将 `"agy"` 映射为 `"Antigravity CLI (新版)"`，`"gemini"` 映射为 `"Gemini CLI (旧版)"`。
  - 更新未探测到或未登录 CLI 时的 `npm install` 引导命令及 `login` 引导命令，适配 `agy` 的输出。
  - 在无可用 CLI 时的引导卡片中，补充关于 `Antigravity CLI` 的提示信息。

### 3. 编译与 Mock 验证
- 在 PATH 中配置 mock 版本的 `agy.cmd` 脚本，模拟版本探测与数据诊断。
- 启动前端与后端，观察是否能在“第二步：选择运行分析引擎 CLI”中顺利检测到 `Antigravity CLI (新版)` 且能获取其 Mock 版本。
- 启动复盘，测试它是否能拉起 mock 的 `agy.cmd` 并在控制台打印输出。
