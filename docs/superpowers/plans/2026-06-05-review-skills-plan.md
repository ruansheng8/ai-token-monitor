# 2026-06-05 AI 复盘与治理中心支持 SKILLS 实现计划

本计划详述了在 `AI 复盘与治理中心` 分析报告时支持技能规范（SKILLS）的技术实施步骤，并支持用户自己上传（`.zip` / `.7z` 压缩包）维护自定义技能。

## User Review Required

> [!IMPORTANT]
> **Cargo 依赖新增**：
> 我们需要在 `src-tauri/Cargo.toml` 中引入以下第三方库：
> * `zip` (用于处理 `.zip` 压缩包解压)
> * `sevenz-rust` (用于处理 `.7z` 压缩包解压)
>
> **内置技能 vs 自定义技能**：
> * 内置技能物理路径在项目目录的 `.agents/skills/`，是只读的。
> * 自定义技能物理路径在全局配置 `%USERPROFILE%/.token-insight/skills/`，可写并提供删除接口。

## Open Questions

无。方案均已在设计阶段获得用户确认。

---

## Proposed Changes

### [Backend] Cargo & Database

#### [MODIFY] [Cargo.toml](file:///d:/VibeCoding/ai-token-monitor/src-tauri/Cargo.toml)
* 添加依赖：
  ```toml
  zip = { version = "2.2.0", default-features = false, features = ["deflate"] }
  sevenz-rust = "0.5"
  ```

#### [MODIFY] [db.rs](file:///d:/VibeCoding/ai-token-monitor/src-tauri/src/db.rs)
* 在 `init_cache_db` 中，在对 `review_tasks` 表结构升级的部分，检测并新增 `selected_skills_json TEXT DEFAULT NULL` 字段：
  ```rust
  let mut has_selected_skills = false;
  // ... pragma 遍历 ...
  if name == "selected_skills_json" {
      has_selected_skills = true;
  }
  // ... 遍历结束 ...
  if !has_selected_skills {
      let _ = conn.execute("ALTER TABLE review_tasks ADD COLUMN selected_skills_json TEXT DEFAULT NULL;", []);
  }
  ```

---

### [Backend] API Routing & Review Logic

#### [MODIFY] [review.rs](file:///d:/VibeCoding/ai-token-monitor/src-tauri/src/review.rs)
* **结构体定义**：
  * 新增 `SkillInfo` 结构体：包含 `id`, `name`, `description`, `is_builtin` 字段。
  * 修改 `CreateTaskRequest`，加入 `skills: Option<Vec<String>>`。
  * 修改 `ReviewTask`，加入 `selected_skills_json: Option<String>`。
* **业务逻辑实现**：
  * 新增 `get_user_skills_dir() -> PathBuf` 获取用户全局技能目录。
  * 新增 `parse_skill_md_frontmatter(content: &str) -> (String, String)`，解析 `SKILL.md` 开头的 YAML 元信息。
  * **API 控制器**：
    * `handle_list_skills`：扫描本地 `.agents/skills` 与全局 `skills` 并合并返回。
    * `handle_upload_skills`：使用 Axum 的 `Multipart` 提取器读取文件，解压到临时目录，递归寻找 `SKILL.md`，然后将包含该文件的父目录拷贝或移动至全局技能路径中，重命名技能 ID，最后清理临时目录。
    * `handle_delete_skill`：检查 ID 合法性，防止目录遍历攻击，物理删除该技能文件夹。
  * **分析报告创建与执行**：
    * `handle_create_task`：插入任务时，将 `skills` 作为 JSON 数组存入 `selected_skills_json`。
    * `run_cli_task_background`：生成 Prompt 时，如果存在 `selected_skills_json` 字段，则循环读取对应技能文件夹里的 `SKILL.md`，拼入 Prompt 尾部的专属 Skills 上下文块中。

#### [MODIFY] [main.rs](file:///d:/VibeCoding/ai-token-monitor/src-tauri/src/main.rs)
* 引入新端点路由：
  * `GET /api/review/skills` -> `handle_list_skills`
  * `POST /api/review/skills/upload` -> `handle_upload_skills`
  * `DELETE /api/review/skills/:id` -> `handle_delete_skill`

---

### [Frontend] React UI Components

#### [MODIFY] [ReviewDrawer.tsx](file:///d:/VibeCoding/ai-token-monitor/src/components/ReviewDrawer.tsx)
* **状态定义**：
  * 新增 `selectedSkills` (已选技能 ID 列表) 及本地 `localStorage` 缓存逻辑。
  * 新增 `skillsList` (技能定义列表，加载自 `/api/review/skills`)。
* **UI 调整**：
  * 在参数配置区块下方，增加“诊断技能规范（Skills）”多选配置，可多选，带描述悬浮提示。
  * 新增“管理技能”按钮，点击后唤起 `SkillManagerModal`。
  * 在新建任务 API调用中带上 `skills` 字段。

#### [NEW] [SkillManagerModal.tsx](file:///d:/VibeCoding/ai-token-monitor/src/components/SkillManagerModal.tsx)
* 自定义技能管理组件：
  * 列表显示现有技能（标识内置只读 / 自定义可删除）。
  * 拖拽或点击上传区域，只接收 `.zip` / `.7z` 扩展名。
  * 调用 `/api/review/skills/upload` 执行文件上传，并在成功后 Toast 提示并自动重载列表。
  * 调用 `/api/review/skills/:id` 执行删除自定义技能，包含二次确认弹窗。

---

## Verification Plan

### Automated Tests
* 进入 `src-tauri` 目录执行 `cargo check`，确认编译无错。
* 执行类型检查：`npx tsc -b --noEmit`，确保前端 TS 编译正常。

### Manual Verification
1. **获取内置技能列表**：进入复盘页，确认系统默认显示项目内置的 `brainstorming`。
2. **测试 ZIP 上传**：制作一个含有 `SKILL.md` 的 `.zip` 文件并上传，检查技能管理器中是否出现新技能，且可正常勾选。
3. **测试 7Z 上传**：制作一个含有 `SKILL.md` 的 `.7z` 文件并上传，检查解压是否顺畅。
4. **生成复盘报告**：勾选自定义技能，点击生成复盘报告。报告生成后检查任务对应的 SQLite 存储的 Prompt 是否注入了技能内容，AI 分析是否受到了技能的影响。
5. **测试删除技能**：在管理弹窗中点击删除该自定义技能，确认其物理文件夹已被删除，且勾选列表不再展示。
