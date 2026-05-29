# 设计文档：使用复盘与建议后台任务化与历史管理

本文档定义「使用复盘与建议」功能从一次性抽屉内 SSE 分析，升级为可恢复、可追踪、可管理的后台复盘任务系统。目标是解决分析进度不可见、关闭界面后任务丢失、缺少历史记录、重复分析不可控等问题，并为后续复盘报告沉淀、行动项追踪和周度对比留下清晰扩展点。

## 1. 背景与现状

当前实现集中在 `src/components/ReviewDrawer.tsx` 与 `src-tauri/src/review.rs`：

- 前端点击「开始智能分析」后，在组件本地创建 `EventSource('/api/review/stream?...')`。
- 分析状态存在抽屉组件的 `isAnalyzing/outputText/isDone/error` 内，组件关闭后不可恢复。
- `useEffect` 在抽屉关闭时调用 `stopAnalysis()`，主动关闭 SSE 连接。
- 后端 `/api/review/stream` 每次请求临时 spawn 一个 CLI 子进程，边读 stdout 边推 SSE。
- 后端没有任务 ID、任务状态、任务历史、互斥锁、持久化输出、取消句柄或恢复协议。

这导致用户看到的体验是：启动后只有简单的“正在分析”，不知道真实阶段；退出界面后状态消失；再次打开只能看到「开始智能分析」；重复点击可能启动多个分析；历史报告无法管理。

## 2. 目标

1. 分析过程后台任务化。离开复盘界面后任务继续运行，重新进入后能恢复状态和输出。
2. 提供可见进度。用户能看到当前阶段、已耗时、最后更新、CLI 输出片段、错误或等待原因。
3. 提供历史复盘记录。用户能查看、搜索、筛选、打开、复制、导出历史报告。
4. 全局同一时间最多一个复盘分析任务处于 `pending/running`。
5. 重复启动时给出明确冲突提示，并引导用户打开冲突任务详情。
6. 对同一数据范围、IDE、提示词、指标快照做去重提示，避免无意义重复分析。
7. 保留当前可选时间范围、IDE 来源、自定义提示词、CLI 引擎能力。
8. 把危险边界补齐：长 prompt 改 POST、stderr 可见、取消可终止子进程、报告渲染安全。

## 3. 非目标

- 不要求应用关闭后继续运行分析。Tauri 应用退出后后端进程也会退出，未完成任务应在下次启动时标记为“已中断”。
- 不把复盘任务同步到远端 PostgreSQL。复盘任务属于本机应用元数据，先持久化到本地 SQLite 缓存库。
- 不在第一阶段实现完整知识库、自动周报订阅或跨设备同步。
- 不要求 LLM 输出真实百分比。进度以确定性阶段和事件时间线表达，生成阶段展示活动状态与输出增量。

## 4. 推荐方案

采用“本地任务管理器 + SQLite 持久化 + SSE 事件恢复”的方案。

后端新增 `ReviewTaskManager`，用全局 `OnceLock<Arc<Mutex<_>>>` 保存当前运行中的子进程句柄和任务互斥状态，任务元数据和输出增量写入本地 SQLite。前端不再直接连接一次性 `/stream` 创建任务，而是先 `POST /api/review/tasks` 创建后台任务，再通过 `GET /api/review/tasks/{id}/events` 订阅事件。进入复盘中心时先请求任务列表和当前 active task，若任务正在运行则自动恢复详情页。

该方案能一次性解决用户提出的三个核心问题，同时改动范围仍集中在 `review.rs`、本地数据库初始化和 `ReviewDrawer/ReviewTaskCenter` 组件内。

## 5. 后端设计

### 5.1 数据表

在本地 SQLite 缓存库增加两张表。

`review_tasks` 保存任务主记录：

```sql
CREATE TABLE IF NOT EXISTS review_tasks (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  status TEXT NOT NULL,
  cli_name TEXT NOT NULL,
  cli_path TEXT,
  time_range TEXT NOT NULL,
  selected_ides_json TEXT NOT NULL,
  prompt_text TEXT NOT NULL,
  prompt_hash TEXT NOT NULL,
  metrics_snapshot_json TEXT NOT NULL,
  metrics_hash TEXT NOT NULL,
  dedupe_key TEXT NOT NULL,
  progress_stage TEXT NOT NULL,
  progress_percent INTEGER NOT NULL DEFAULT 0,
  status_message TEXT NOT NULL DEFAULT '',
  output_markdown TEXT NOT NULL DEFAULT '',
  error_message TEXT,
  exit_code INTEGER,
  created_at TEXT NOT NULL,
  started_at TEXT,
  finished_at TEXT,
  canceled_at TEXT,
  last_heartbeat_at TEXT
);
```

`review_task_events` 保存可回放事件：

```sql
CREATE TABLE IF NOT EXISTS review_task_events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  task_id TEXT NOT NULL,
  sequence INTEGER NOT NULL,
  kind TEXT NOT NULL,
  message TEXT NOT NULL,
  payload_json TEXT,
  created_at TEXT NOT NULL,
  FOREIGN KEY(task_id) REFERENCES review_tasks(id)
);
```

同时创建索引：

```sql
CREATE INDEX IF NOT EXISTS idx_review_tasks_status_created
  ON review_tasks(status, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_review_tasks_dedupe
  ON review_tasks(dedupe_key, status);

CREATE INDEX IF NOT EXISTS idx_review_task_events_task_sequence
  ON review_task_events(task_id, sequence);
```

状态枚举：`pending`、`running`、`succeeded`、`failed`、`canceled`、`interrupted`。

事件类型：`stage`、`progress`、`stdout`、`stderr`、`heartbeat`、`error`、`done`。

### 5.2 API

新增接口：

- `POST /api/review/tasks`：创建任务并启动后台分析。
- `GET /api/review/tasks`：任务列表，支持 `status`、`limit`、`offset`、`q`。
- `GET /api/review/tasks/active`：返回当前 `pending/running` 任务，没有则返回 `null`。
- `GET /api/review/tasks/{id}`：任务详情，包含输出全文和指标快照。
- `GET /api/review/tasks/{id}/events?after={sequence}`：SSE 事件流，先补发 `after` 之后的历史事件，再实时推送。
- `POST /api/review/tasks/{id}/cancel`：取消任务，终止 CLI 子进程，状态置为 `canceled`。
- `DELETE /api/review/tasks/{id}`：删除已完成、失败、取消或中断任务；运行中任务不可删除。

保留 `GET /api/review/detect`。旧 `/api/review/stream` 可以暂时保留兼容，但前端不再使用。

### 5.3 创建任务流程

`POST /api/review/tasks` 请求体使用 JSON，不再用 query 承载长 prompt：

```json
{
  "cli": "claude",
  "time_range": "7天",
  "selected_ides": ["all"],
  "custom_prompt": "...",
  "force": false,
  "metrics_snapshot": {
    "totalTokens": 123,
    "totalCostUsd": 0.12,
    "totalSessions": 8,
    "cacheHitRate": 0.31,
    "thinkingRatio": 0.18,
    "sourceBreakdown": "...",
    "modelDistribution": "...",
    "dailyTrendSummary": "..."
  }
}
```

服务端执行顺序：

1. 获取全局任务锁。
2. 查询是否存在 `pending/running` 任务。
3. 若存在，返回 `409`，响应中包含冲突任务摘要和 `task_id`。
4. 计算 `prompt_hash`、`metrics_hash`、`dedupe_key`。
5. 若存在相同 `dedupe_key` 的成功任务且 `force` 不是 `true`，返回 `200` 并标记 `duplicate_of`，前端提示查看已有报告或强制重新生成。
6. 插入 `review_tasks`，状态为 `pending`。
7. spawn 后台任务，状态改为 `running`。
8. 返回任务详情。

### 5.4 进度阶段

进度不是伪造精确百分比，而是确定性阶段：

| 阶段 | 百分比 | 文案 |
|---|---:|---|
| `created` | 5 | 已创建复盘任务 |
| `snapshot_ready` | 15 | 已冻结分析数据快照 |
| `prompt_ready` | 25 | 已生成分析提示词 |
| `cli_resolved` | 35 | 已定位 AI CLI 引擎 |
| `cli_started` | 45 | 已启动分析进程 |
| `streaming` | 55-90 | 正在接收分析输出 |
| `persisting` | 95 | 正在保存报告 |
| `done` | 100 | 分析完成 |

生成阶段按输出字符数和心跳推进到最高 90%，但不展示为“已完成”。如果 15 秒内没有 stdout，发送 heartbeat：`正在等待 Claude Code 响应，已等待 X 秒`。

### 5.5 子进程与取消

`ReviewTaskManager` 维护当前运行任务的 `task_id` 和 CLI 子进程句柄。取消时：

- 写入 `cancel_requested` 事件。
- 尝试 kill 子进程。
- 状态置为 `canceled`。
- SSE 发送 `done` 事件。

如果前端关闭 SSE，不取消任务；只有用户点击“取消分析”才取消。

### 5.6 启动恢复

应用启动后执行一次任务恢复检查：

- 若数据库里有 `running/pending` 任务，但内存中没有对应子进程句柄，说明应用曾退出或崩溃，将任务标记为 `interrupted`。
- 若内存中有运行任务，`GET /active` 返回该任务，前端可恢复查看。

## 6. 前端设计

### 6.1 页面结构

将现有 `ReviewDrawer` 升级为 `ReviewTaskCenter`，仍从主页面“复盘”按钮打开，但内容变成任务中心。

主要区域：

1. 顶部状态栏：显示当前是否有任务运行、最近完成时间、进入历史按钮。
2. 新建分析页：时间范围、IDE 来源、CLI 引擎、提示词编辑、数据快照预览。
3. 任务历史页：列表管理所有复盘任务。
4. 任务详情页：进度、日志、输出报告、指标快照、复制/导出/重跑/删除。

### 6.2 运行中详情

运行中任务详情展示：

- 阶段进度条与当前阶段文案。
- 已耗时、最后心跳、CLI 引擎。
- 可折叠“执行日志”，混合展示 stage、stdout、stderr、error。
- 报告预览区流式追加输出。
- 操作按钮：“隐藏并后台继续”、“取消分析”。

关闭抽屉等同于“隐藏并后台继续”，不会停止分析。

### 6.3 冲突处理

点击开始时若后端返回 `409 REVIEW_TASK_RUNNING`：

- 不再静默无效。
- 显示冲突提示：`已有复盘任务正在运行：{title}`。
- 主按钮变为“查看正在运行的任务”。
- 点击后打开该任务详情页并恢复日志和输出。

若返回重复报告：

- 提示：`相同数据范围和提示词已有报告`。
- 提供“查看已有报告”和“重新生成”两个动作。

### 6.4 历史管理

任务历史列表字段：

- 状态：运行中、已完成、失败、已取消、已中断。
- 标题：例如 `最近 7 天 · 全部 IDE · Claude Code`。
- 时间范围、IDE 来源、CLI 引擎。
- 创建时间、耗时、输出字数。
- 操作：打开、复制、重新生成、删除。

筛选项：

- 状态筛选。
- 时间范围筛选。
- IDE 来源筛选。
- 标题/报告全文搜索可作为后续增强，第一阶段可只做标题与元数据搜索。

### 6.5 报告渲染安全

当前 `dangerouslySetInnerHTML` 直接渲染模型输出。改造时应至少做 HTML escape 后再渲染 Markdown，推荐引入安全 Markdown 渲染链路；如果暂不加依赖，则实现严格转义，仅支持标题、列表、代码块、加粗等白名单转换。

## 7. 其他问题与可新增模块

除用户已提出的三个核心问题外，建议纳入产品 backlog：

1. 长 prompt 使用 GET query 传输，存在 URL 过长和日志暴露风险。
2. 自定义 prompt 覆盖自动 prompt 后，可能丢失结构化指标上下文。
3. CLI stderr 没有实时展示，登录失败或权限失败很难定位。
4. 前端关闭 SSE 不应等同于取消任务。
5. 取消任务需要真正终止子进程，不能只改前端状态。
6. 缺少同参数去重，容易重复生成成本相同的报告。
7. 缺少数据快照，历史报告无法追溯当时使用了哪些指标。
8. 缺少失败重试，失败任务应支持用原参数重跑。
9. 缺少导出能力，报告应支持 Markdown、PDF 或复制全文。
10. 缺少行动项管理，报告建议可以拆成可勾选的优化任务。
11. 缺少完成通知，后台完成后应有 toast 或托盘通知。
12. 缺少周度对比，可以对比本周与上周的成本、缓存率、模型分布和协作习惯。
13. 缺少模板管理，不同目标可用“成本优化”“协作质量”“项目复盘”等模板。
14. 缺少敏感信息提示，发送给 CLI 前应展示可能读取本地会话/项目产物的边界。
15. 缺少报告质量反馈，用户可以标记“有用/不准确/太泛”，用于调优提示词。
16. 缺少失败诊断面板，把 CLI 未安装、未登录、超时、权限不足分成明确错误类型。

## 8. 测试设计

后端测试：

- 创建任务时没有运行任务，返回新任务并进入 `running`。
- 已有 `running` 任务时再次创建，返回 `409` 和冲突任务。
- 相同 `dedupe_key` 已成功时，返回重复提示。
- 取消任务会写入事件并置为 `canceled`。
- 启动恢复会把孤立 `running/pending` 标记为 `interrupted`。
- 使用 mock CLI 验证 stdout、stderr、exit code、超时能正确落库。

前端验证：

- 开始分析后显示阶段进度、日志和输出。
- 关闭复盘中心再打开，仍显示运行中任务。
- 运行中重复点击会进入冲突任务详情。
- 任务完成后出现在历史列表。
- 失败、取消、中断状态都有明确 UI。
- 报告内容不会执行 HTML 脚本。

## 9. 分阶段实施

第一阶段：后台任务核心

- 新增 SQLite 表和任务 DAO。
- 新增任务 API。
- 后端实现单任务互斥、事件落库、SSE 恢复、取消任务。
- 前端从 `/stream` 迁移到任务 API。

第二阶段：任务中心 UI

- 新建任务历史列表。
- 新建任务详情页。
- 接入冲突提示、恢复回显、重复报告提示。
- 优化进度条、阶段时间线、日志面板。

第三阶段：报告沉淀增强

- 导出 Markdown/PDF。
- 行动项提取与勾选。
- 周度对比、模板管理、报告反馈。

## 10. 验收标准

1. 开始分析后，用户能看到至少 6 个明确阶段和实时日志/输出。
2. 分析运行中关闭复盘中心，再打开仍显示同一个任务的运行状态和已生成内容。
3. 同一时间重复启动分析时不会创建第二个运行任务，而是提示并跳转到冲突任务。
4. 分析完成后，报告能在历史记录中找到并重新打开。
5. 取消分析会终止后端 CLI 子进程，并在历史中显示为已取消。
6. 应用重启后，未完成任务不会误显示为运行中，而是标记为已中断。
7. 长提示词通过 POST JSON 提交，不再依赖 URL query。
8. CLI stderr、退出码和超时错误能在任务详情中看到。
9. 报告渲染不会执行模型输出中的 HTML 或脚本。
10. `npm run build` 与 Rust 编译检查通过。
