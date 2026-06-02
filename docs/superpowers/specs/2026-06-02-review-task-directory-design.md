# 2026-06-02-review-task-directory-design

## 1. 背景与目标
在 `Token Insight` 系统的复盘（Review）模块中，分析任务是通过启动宿主机的 AI CLI 引擎（例如 `claude`, `aider`, `codex` 等）进行的。目前，这些 CLI 进程缺少独立隔离的工作目录，导致运行过程中产生的临时文件、日志和缓存会散落在程序当前的工作目录中，容易污染项目根目录，也存在多任务并发时的文件冲突隐患。

本设计的目标是：
- 将复盘模块所调用 CLI 的执行环境进行沙盒化隔离。
- CLI 在运行过程中产生的所有临时文件，默认归档到用户个人配置目录下的 `tasks/reports/<task_id>` 目录中。
- 在对应的任务目录下，同步备份任务的输入（提示词）和输出（最终的 Markdown 报告 / 错误日志），供离线查阅。
- 实现任务删除与重试时的自动清理逻辑，保持磁盘空间的整洁。

## 2. 详细设计

### 2.1 任务专属目录的路径解析
在 `src-tauri/src/review.rs` 中定义一个辅助函数，确定任务在磁盘上的归档目录：
```rust
/// 获取指定任务的报告和临时文件目录
/// 路径为：~/.token-insight/tasks/reports/{task_id}
fn get_task_reports_dir(task_id: &str) -> std::path::PathBuf {
    std::path::Path::new(&crate::db::get_user_profile_dir())
        .join(".token-insight")
        .join("tasks")
        .join("reports")
        .join(task_id)
}
```

### 2.2 CLI 执行环境隔离与归档 (`run_cli_task_background`)
在后台执行协程 `run_cli_task_background` 中：
1. **创建工作空间**：根据 `task_id` 获取 `task_dir` 并调用 `std::fs::create_dir_all(&task_dir)`，确保目录存在。
2. **备份输入 Prompt**：在拉起 CLI 之前，将生成的提示词写入该目录下的 `prompt.md` 中：
   ```rust
   let _ = std::fs::write(task_dir.join("prompt.md"), &prompt);
   ```
3. **隔离执行环境**：配置 `Command` 启动参数时，调用 `.current_dir(&task_dir)` 将子进程的当前工作目录设置为该目录。
4. **归档执行结果**：
   - 如果 CLI 成功执行：将生成的 `output_markdown`（或累积的流输出）同步写入 `task_dir.join("report.md")`。
   - 如果 CLI 执行失败：将收集到的 `stderr_buffer` 写入 `task_dir.join("error.log")`。

### 2.3 任务删除时级联清理 (`handle_delete_task`)
在 `handle_delete_task` 中，从数据库删除任务记录并校验状态通过后，执行文件系统的级联删除：
```rust
let task_dir = get_task_reports_dir(&id_clone);
if task_dir.exists() {
    let _ = std::fs::remove_dir_all(&task_dir);
}
```

### 2.4 任务重试时初始化清理 (`handle_retry_task`)
在 `handle_retry_task` 中，将数据库状态重置为 `pending` 并且启动后台执行之前，清理可能残留的旧文件：
```rust
let task_dir = get_task_reports_dir(&id_clone);
if task_dir.exists() {
    let _ = std::fs::remove_dir_all(&task_dir);
}
let _ = std::fs::create_dir_all(&task_dir);
```

## 3. 验证方案
1. **构建与编译**：
   - 运行 `cargo check` 确保无编译和语法错误。
2. **功能与沙盒验证**：
   - 新建一个复盘任务，观察在 `~/.token-insight/tasks/reports/<task_id>/` 下是否成功生成 `prompt.md` 以及是否正常调起 CLI。
   - 待任务完成后，观察是否正常生成 `report.md`。
   - 如果 CLI 本身在运行期间产生额外文件，检查这些文件是否确实被限定在 `<task_id>` 子目录中，而没有污染外部目录。
3. **清理验证**：
   - 点击重试任务，验证历史报告文件夹是否被重新初始化清空。
   - 点击删除任务，验证磁盘上该 `<task_id>` 目录是否已被彻底移除。
