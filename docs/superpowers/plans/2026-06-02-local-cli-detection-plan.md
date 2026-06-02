# 本机 CLI 识别与测试检测逻辑迁移实现计划

本计划旨在将 `open-design` 中的本机 CLI 强健探测与检测机制迁移至 `token-insight` 项目中，改进对 PATH 缺失和 Ghost 残留 shim 的处理逻辑。

## 1. 拟修改的文件与变动说明

### 核心后端模块

#### [MODIFY] [review.rs](file:///d:/VibeCoding/ai-token-monitor/src-tauri/src/review.rs)
- **新增** `find_node_version_bin_dirs` 辅助函数：根据提供的 Node 版本管理器安装目录遍历并追加其下的 `bin` 路径。
- **新增** `get_well_known_toolchain_dirs` 函数：实现针对 npm-global, fnm, nvm, volta, cargo, bun 等著名开发工具链在 Windows/Unix 系统下的默认安装路径扫描。
- **重构** `find_cli_in_path`：在原有的系统 `PATH` 查找之上，合并 `get_well_known_toolchain_dirs()` 目录列表，进行全文件匹配（Windows 下自动附带 `.exe`, `.cmd`, `.bat` 后缀）。
- **重构** `probe_cli`：
  - 使用 `tokio::process::Command` 运行 `--version`。
  - 通过 `tokio::time::timeout` 约束 5 秒超时（超时返回 `available: true`, `version: None`）。
  - 精准捕获 `ErrorKind::NotFound` 和 `ErrorKind::PermissionDenied` 等操作系统级无法拉起进程的错误。
  - 检查子进程退出状态码。若状态码为 `126` 或 `127`，判定为不可调用。
  - 检查子进程 stdout/stderr 输出。若在 Windows 环境下包含 "系统找不到指定的路径" 或 "找不到文件" 特征字眼，判定为不可调用。
  - 符合不可调用条件的，其 `available` 一律返回 `false`。

## 2. 验证计划

### 自动化与语法检查
- **构建测试**：在 `src-tauri` 目录下运行 `cargo check`，确保代码零警告、零编译错误。

### 手动验证场景
1. **未安装的 CLI**：
   - 验证检测列表中未安装的引擎（如 `codex` / `gemini`）能够被标记为 `available: false`。
2. **已删除但有残留软链/代理的 Ghost CLI**：
   - 在用户的 `APPDATA/npm` 下临时创建一个损坏的 `claude.cmd`（内部执行一个不存在的命令），启动检测，验证其被标记为 `available: false` 并且不会引起后台程序崩溃。
3. **已正常安装的 CLI**：
   - 确保本机已经部署好的 `agy.exe` (Antigravity CLI 新版) 或 `claude` 被自动识别并展现其正确版本。
