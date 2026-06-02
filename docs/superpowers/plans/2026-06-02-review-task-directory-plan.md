# 2026-06-02-review-task-directory-plan

## 1. 目标描述
配置 `复盘` 模块，为每个 AI CLI 分析任务提供独立的沙盒执行目录。所有 CLI 的当前工作目录（Cwd）将被重定向到用户个人配置目录下的 `tasks/reports/<task_id>` 目录中。同时，将任务的提示词归档为 `prompt.md`，将最终生成的报告归档为 `report.md`，并在任务删除或重试时进行相应的级联清理或清空。

## 2. 变更说明

### src-tauri/src/review.rs
#### [MODIFY] [review.rs](file:///d:/VibeCoding/ai-token-monitor/src-tauri/src/review.rs)
1. **新增辅助函数 `get_task_reports_dir`**：
   ```rust
   fn get_task_reports_dir(task_id: &str) -> std::path::PathBuf {
       std::path::Path::new(&crate::db::get_user_profile_dir())
           .join(".token-insight")
           .join("tasks")
           .join("reports")
           .join(task_id)
   }
   ```
2. **修改 `run_cli_task_background`**：
   - 在开始运行前，创建该任务专属的目录：`let task_dir = get_task_reports_dir(task_id); std::fs::create_dir_all(&task_dir)`。
   - 在该目录下写入 `prompt.md`。
   - 定义 `mut output_markdown = String::new();` 并在 stdout 流读取过程中追加保存。
   - 在 `Command::new` 后调用 `cmd.current_dir(&task_dir)`。
   - 任务结束时，根据执行的成功与否，分别将 `output_markdown` 写入 `report.md` 或将 `stderr_buffer` 写入 `error.log`。
3. **修改 `handle_delete_task`**：
   - 成功删除任务记录后，调用 `std::fs::remove_dir_all(&task_dir)` 清理对应的目录。
4. **修改 `handle_retry_task`**：
   - 重置任务前，清理已有的目录并重建，确保执行环境干净。

## 3. 验证方案
### 3.1 编译验证
在 `src-tauri` 目录下运行 `cargo check` 确保无编译报错。

### 3.2 功能与清理验证
- 启动应用，并在前端发起复盘任务。
- 确认用户配置目录下的 `.token-insight/tasks/reports/<task_id>` 成功创建。
- 检查该目录下是否含有 `prompt.md`。
- 任务成功结束后，检查该目录下是否含有完整的 `report.md`。
- 在前端删除该任务，验证对应的磁盘目录是否已被彻底删除。
- 再次创建一个复盘任务并执行，然后在前端对其进行重试，验证重试时是否重建了干净的目录。
