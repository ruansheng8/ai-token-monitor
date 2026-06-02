# “相同参数历史报告已存在”校验增加模板ID唯一索引设计方案

## 背景与目标

当前 AI 复盘任务创建时，会进行相同参数历史报告的去重匹配，若 6 小时内存在匹配的成功报告且指标偏差小于 2%，则会提示“相同参数历史报告已存在”，并建议用户直接查看已有报告。

**核心问题**：目前的去重规则仅比对了数据周期（`time_range`）、AI CLI 引擎（`cli_name`）与关联的 IDE 数据源（`selected_ides_json`），未匹配具体的“专家分析提示词模板（`template_id`）”。这导致当用户使用同一时段的数据，切换不同的分析视角（例如从“综合效能评估”切换到“成本节流专项”）时，会被系统判定为重复报告而拦截。

本方案旨在：
1. 在复盘任务表 `review_tasks` 中引入 `template_id` 字段。
2. 修改重复匹配校验逻辑，将 `template_id` 纳入历史去重约束与 `dedupe_key` 计算中。
3. 按照用户指示，自动在数据库升级迁移时清空原有的历史复盘报告，彻底消除脏数据。

---

## 详细设计

### 1. 数据库 Schema 变更

为了适配多数据库环境，我们分别对本地 SQLite 缓存和生产 PostgreSQL 进行表结构升级：

#### A. SQLite 本地缓存表升级 (`src-tauri/src/db.rs`)
- 更新建表语句 `CREATE TABLE IF NOT EXISTS review_tasks`，添加 `template_id TEXT DEFAULT NULL` 字段。
- 在 `init_cache_db` 增量升级阶段增加 `template_id` 字段探针。
- 当检测到没有 `template_id` 字段时，执行升级 `ALTER TABLE`；**并在升级时顺便清空 `review_task_events` 与 `review_tasks` 表**，实现旧报告清理。

#### B. PostgreSQL 迁移脚本 (`src-tauri/migrations/postgres`)
- 新增 Refinery 迁移脚本 `src-tauri/migrations/postgres/V5__add_template_id_to_review_tasks.sql`：
  ```sql
  ALTER TABLE review_tasks ADD COLUMN template_id VARCHAR(255) DEFAULT NULL;
  TRUNCATE TABLE review_task_events CASCADE;
  TRUNCATE TABLE review_tasks CASCADE;
  ```

---

### 2. Rust 后端业务逻辑修改

#### A. 数据结构更新 (`src-tauri/src/review.rs`)
- 在 `CreateTaskRequest` 结构体中添加：
  ```rust
  pub template_id: Option<String>,
  ```
- 在 `ReviewTask` 结构体中添加：
  ```rust
  pub template_id: Option<String>,
  ```

#### B. 接口逻辑升级 (`src-tauri/src/review.rs`)
- **去重校验**：在 `handle_create_task` 处理中，查询最近 6 小时已成功任务时，SQL 增加模板校验条件 `AND COALESCE(template_id, '') = ?`，并传入 `req.template_id.clone().unwrap_or_default()` 作为参数。
- **Dedupe Key 计算**：将 `template_id` 加入 `dedupe_str` 的哈希计算：
  ```rust
  let template_id_str = req.template_id.clone().unwrap_or_default();
  let dedupe_str = format!("{}_{}_{}_{}_{}", req.time_range, selected_ides_json, prompt_hash, metrics_hash, template_id_str);
  ```
- **任务插入**：将 `req.template_id` 字段值写入 `review_tasks` 的 `template_id` 列中。
- **任务详情读取与列表**：更新 `query_task_by_id`、`handle_list_tasks` 以及 `handle_get_active_task`，从数据库中提取 `template_id` 字段并正确反序列化到 `ReviewTask` 结构体中。

---

### 3. 前端界面与 API 调用升级 (`src/components/ReviewDrawer.tsx`)

- 在调用 `handleStartAnalysis` 发送创建任务 POST 请求时，构造 Payload 增加 `template_id` 参数：
  ```typescript
  const payload: any = {
    cli: selectedCli,
    time_range: reviewTimeRange,
    selected_ides: selectedIdes,
    template_id: selectedTemplateId, // 传入当前选中的专家模板 ID
    custom_prompt: customPrompt.trim() ? customPrompt.trim() : undefined,
    force: forceStart,
    // ... metrics_snapshot 与 compare_metrics_snapshot ...
  };
  ```

---

## 验证与测试方案

### 1. 编译验证
在项目根目录运行以下命令以确保前后端编译无误：
```powershell
# 前端类型检查
npx tsc -b --noEmit

# 后端语法检查
cd src-tauri
cargo check
```

### 2. 行为验证
1. 启动项目，进入复盘历史，应当发现历史复盘列表已被自动清空。
2. 使用“综合效能评估”模板，运行一次复盘，成功后再次以相同参数选择“综合效能评估”生成，系统应当正确拦截并提示“相同参数历史报告已存在”。
3. 保持统计参数不变，将模板切换为“成本节流专项”，系统应当能够**成功开启全新分析**而不会发生拦截。
