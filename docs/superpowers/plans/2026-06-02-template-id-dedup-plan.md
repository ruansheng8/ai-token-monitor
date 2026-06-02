# 校验引入模板ID并清理历史报告实现计划

为 AI 复盘的“相同参数历史报告已存在”校验引入模板 ID，避免同参数不同模板的报告被误判为重复报告；同时升级数据库结构，并在升级时清理旧报告脏数据。

## 用户确认事项

1. 本次升级将对 SQLite 缓存和 Postgres 进行 Schema 变更，添加 `template_id` 字段。
2. 为响应“清理掉历史报告”的指示，本计划会在执行数据库结构变更（即第一次运行新代码）时**自动清空** `review_tasks` 及 `review_task_events` 历史数据，以防止遗留的 `NULL` 模板ID对新版本去重逻辑产生干扰。

## 变更文件列表

### 后端存储与路由校验

#### `src-tauri/src/db.rs`
- 在 `init_cache_db` 中，为 `review_tasks` 的 `CREATE TABLE` 语句中加入 `template_id TEXT DEFAULT NULL`。
- 在 SQLite 增量升级校验段，检测若不存在 `template_id` 字段，则执行以下 SQL：
  - 清空旧报告事件：`DELETE FROM review_task_events;`
  - 清空旧报告：`DELETE FROM review_tasks;`
  - 添加新字段：`ALTER TABLE review_tasks ADD COLUMN template_id TEXT DEFAULT NULL;`

#### `src-tauri/migrations/postgres/V5__add_template_id_to_review_tasks.sql`
- 创建生产 Postgres 迁移脚本：
  ```sql
  ALTER TABLE review_tasks ADD COLUMN template_id VARCHAR(255) DEFAULT NULL;
  TRUNCATE TABLE review_task_events CASCADE;
  TRUNCATE TABLE review_tasks CASCADE;
  ```

#### `src-tauri/src/review.rs`
- 结构体扩展：
  - `CreateTaskRequest` 添加 `pub template_id: Option<String>`
  - `ReviewTask` 添加 `pub template_id: Option<String>`
- 路由处理器升级：
  - `handle_create_task`：
    - 查询 6 小时内报告时，SQL 中加入 `AND COALESCE(template_id, '') = ?` 比对，并在绑定参数中填入 `req.template_id.clone().unwrap_or_default()` 作为参数。
    - 将 `template_id` 加入 `dedupe_str` 生成哈希，计算更精准的 `dedupe_key`。
    - 插入任务时，将 `req.template_id` 保存至 `template_id` 列。
  - `query_task_by_id`：读取 SQL 查询段中加入 `template_id`，并序列化到 `ReviewTask`。
  - `handle_list_tasks`：读取 SQL 查询段中加入 `template_id`，并序列化到 `ReviewTask`。
  - `handle_get_active_task`：读取 SQL 查询段中加入 `template_id`，并序列化到 `ReviewTask`。

---

### 前端表单请求

#### `src/components/ReviewDrawer.tsx`
- 在 `handleStartAnalysis` 发送的 POST Payload 中传入 `template_id: selectedTemplateId`。

---

## 验证与测试方案

### 自动化编译检查
1. 运行前端类型检查：
   ```powershell
   npx tsc -b --noEmit
   ```
2. 运行后端编译检查：
   ```powershell
   cd src-tauri
   cargo check
   ```

### 手动行为验证
1. 启动项目，切换到“历史报告”视图，验证是否已被完全清空。
2. 在“新建复盘”面板中：
   - 数据天数选择 `最近7天`，IDE 勾选 `全部`，模板选择 `综合效能评估`，点击运行复盘。
   - 完成后，再次点击“新建复盘”，不修改参数继续用 `综合效能评估` 模板运行，应当触发“相同参数历史报告已存在”的拦截并提示去重。
   - 将模板切换为 `成本节流专项`，再次点击运行，验证报告能够成功重新开始生成，证明去重已能够按模板 ID 隔离。
