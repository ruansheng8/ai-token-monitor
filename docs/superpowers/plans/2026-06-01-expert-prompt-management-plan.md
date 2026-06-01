# 专家分析提示词维护界面实现计划

本计划用于在 Token Insight 中实现专家分析提示词的增删改查及克隆功能。

## User Review Required

> [!IMPORTANT]
> 1. **数据库表初始化**：为了安全和简单，本项目的 SQLite 数据库并不使用 Refinery，SQLite 表都是在 Rust 初始化阶段的 `db.rs` 中使用 `CREATE TABLE IF NOT EXISTS` 实现。因此，提示词模板表也将直接在 `db.rs` 的 `init_cache_db` 中初始化。
> 2. **内置模板只读保障**：后端会在编辑/删除接口上，通过校验 `is_builtin == 1` 对内置模板进行强拦截报错，以确保系统基本面安全。

---

## Proposed Changes

### 1. 后端数据与 API 模块

#### [MODIFY] [db.rs](file:///d:/VibeCoding/ai-token-monitor/src-tauri/src/db.rs)
- 在 `init_cache_db` 中添加 `prompt_templates` 建表语句。
- 新增 `seed_default_prompt_templates` 函数，在表为空时自动写入原预设的 4 个默认提示词模板，并设定 `is_builtin = 1`。

#### [MODIFY] [review.rs](file:///d:/VibeCoding/ai-token-monitor/src-tauri/src/review.rs)
- 声明并实现 `PromptTemplate`、`CreatePromptTemplateReq` 和 `UpdatePromptTemplateReq`。
- 实现 `handle_list_prompt_templates`、`handle_create_prompt_template`、`handle_update_prompt_template` 和 `handle_delete_prompt_template` 处理器。
- 保证安全拦截：修改或删除内置模板时返回 `StatusCode::BAD_REQUEST`。

#### [MODIFY] [main.rs](file:///d:/VibeCoding/ai-token-monitor/src-tauri/src/main.rs)
- 导入这 4 个新增路由处理器。
- 在 Axum 路由树中配置对应的 `/api/review/prompt_templates` 和 `/api/review/prompt_templates/:id` 路由规则。

---

### 2. 前端界面与交互

#### [NEW] [PromptTemplateManagerModal.tsx](file:///d:/VibeCoding/ai-token-monitor/src/components/PromptTemplateManagerModal.tsx)
- 新建提示词模板管理弹窗组件。
- 磨砂玻璃质感 UI，左侧是模板列表，右侧是表单编辑区（包含模板名、描述、模板内容 textarea）。
- 右侧编辑区根据选中模板是否为 builtin，自动切换只读或编辑态。
- 支持新建、保存修改、克隆副本（克隆内置模板到新自定义模板）、删除（带二次确认）。

#### [MODIFY] [ReviewDrawer.tsx](file:///d:/VibeCoding/ai-token-monitor/src/components/ReviewDrawer.tsx)
- 在原有的“📋 选择专家分析切入点/模板”栏加入“⚙️ 管理模板”按钮，并引入 `PromptTemplateManagerModal`。
- 将原本静态写死的 `TEMPLATE_PRESETS` 升级为从 API 异步加载的列表，状态用 React State 管理。
- 在用户关闭模板管理弹窗或操作完毕后，自动刷新列表并高亮选中最新操作的模板。

---

## Verification Plan

### Automated Tests
- 验证 Rust 后端无编译警告或错误：
  ```powershell
  cd src-tauri; cargo check
  ```
- 验证前端 TypeScript 无类型报错：
  ```powershell
  npx tsc -b --noEmit
  ```

### Manual Verification
1. 启动应用，打卡复盘，确认原预设的 4 个模板自动从 API 加载展示。
2. 点击“管理模板”，选择“综合效能评估”等内置模板，确认编辑框为只读，且有“克隆副本”按钮。
3. 点击“克隆副本”，修改名称和内容，确认可以成功创建自定义模板。
4. 在左侧列表中选中该自定义模板，编辑并点击保存，确认能正常修改。
5. 选中自定义模板，点击删除并确认，确认模板彻底消失。
