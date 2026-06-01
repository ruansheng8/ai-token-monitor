# 智能复盘会话细节提取与诊断实现计划

在「AI 复盘与治理中心」的智能效能分析中支持捕获本地 IDE / CLI（如 `Claude Code`、`Antigravity` 等）的交互细节，并通过前端侧边抽屉（Sidebar Drawer）进行调试细节和错误信息的联动回溯，提升效能复盘报告的精准度。

---

## User Review Required

> [!IMPORTANT]
> **多数据库兼容升级**：
> 本项目支持 SQLite（本地缓存）与 PostgreSQL。本次修改涉及 DDL 变更：
> - **SQLite**：在 `db.rs` 中的 `init_cache_db` 逻辑内进行 DDL 注入。
> - **PostgreSQL**：在 `src-tauri/migrations/postgres/` 下建立 Refinery 迁移脚本 `V4__add_turn_details_table.sql`。
> 请确认数据库连接和权限配置正常。

---

## Open Questions

> [!NOTE]
> 暂无阻碍当前实现的关键性设计疑问。已在脑暴设计中与用户就“方案 C 抽屉式 UI”与“方案 C 异常触发智能采样数据”达成共识。

---

## Proposed Changes

### 后端存储与数据库组件 (Backend DB & Migrations)

#### [MODIFY] [db.rs](file:///d:/VibeCoding/ai-token-monitor/src-tauri/src/db.rs)
- 在 `init_cache_db` 中增加 `turn_details` 表的 SQLite 创表 SQL。
- 增加 `TurnDetails` 数据结构体及相应的序列化/反序列化（Serialize/Deserialize）实现。

#### [NEW] [V4__add_turn_details_table.sql](file:///d:/VibeCoding/ai-token-monitor/src-tauri/migrations/postgres/V4__add_turn_details_table.sql)
- 编写 Refinery 迁移脚本，定义 `turn_details` 在 PostgreSQL 中的表结构，与 SQLite schema 保持一致。

---

### 后端同步与事件提取引擎 (Backend Sync & Core Engine)

#### [MODIFY] [db.rs](file:///d:/VibeCoding/ai-token-monitor/src-tauri/src/db.rs)
- 升级 `sync_claude_code` 等同步解析方法：
  - 在遍历行读取 JSONL 时，遇到异常事件（即 exitCode != 0 的本地命令，或者 10 分钟内同一文件报错超过 3 次的重试循环）时，解析出该交互的 `prompt` 文本、失败命令字及对应的 `stderr` 标准错误。
  - 将上述字段打包插入/更新到 `turn_details` 表中。

#### [MODIFY] [review.rs](file:///d:/VibeCoding/ai-token-monitor/src-tauri/src/review.rs)
- **API Handler**：新增 `handle_get_turn_details` 处理器方法，支持通过 `source`、`uuid`、`idx` 查询特定轮次明细。
- **Prompt 挖掘**：在 `handle_create_task` 中，增加数据挖掘查询，提取最近时段发生的 `failed_commands` 重试及错误事件，将其生成简洁文本注入 Prompt 尾部的 `{{ERROR_EVENTS_SUMMARY}}` 中。

#### [MODIFY] [server.rs](file:///d:/VibeCoding/ai-token-monitor/src-tauri/src/server.rs)
- 注册 API 路由端点：`GET /api/review/turns/details` 映射到 `review::handle_get_turn_details`。

---

### 前端交互组件 (Frontend UI & Visuals)

#### [NEW] [TurnDetailsDrawer.tsx](file:///d:/VibeCoding/ai-token-monitor/src/components/TurnDetailsDrawer.tsx)
- **新建独立组件**：创建侧边抽屉组件 `TurnDetailsDrawer.tsx`。
- **布局设计**：磨砂玻璃半透明效果、暗黑拟态。使用 Monospace 字体渲染终端风格的命令输入与报错日志。
- **API 交互**：当抽屉打开时，通过 `fetch('/api/review/turns/details?source=...&uuid=...&idx=...')` 获取明细并填充。

#### [MODIFY] [App.tsx](file:///d:/VibeCoding/ai-token-monitor/src/App.tsx)
- 在 App 的 Root 或 Review 控制容器中挂载 `<TurnDetailsDrawer />`。
- 升级 `react-markdown` 的渲染配置，拦截 `turn-details://` 形式的链接点击，阻止浏览器默认行为，提取参数并触发 Drawer 的打开。

---

## Verification Plan

### Automated Tests
- 执行编译与语法校验：
  ```powershell
  cd src-tauri
  rtk cargo check
  ```
- 运行前端类型检查：
  ```powershell
  rtk nsc tsc -b --noEmit
  ```

### Manual Verification
1. 启动项目调试大盘。
2. 构造一次 Claude Code 异常轮次（如故意在终端运行一个不存在的命令或产生一次 Rust 编译报错）。
3. 触发“同步历史会话”，确保 SQLite / PostgreSQL 数据库中写入了对应的 `turn_details` 数据。
4. 在“AI 复盘与建议”中生成效能报告，核实报告底部的 Prompt 调试异常摘要信息，生成 Markdown 报告。
5. 点击生成的报告中对应的跳转链接，验证 Sidebar Drawer 从右侧流畅拉出，并且能够成功渲染出当时具体的 Prompt、执行命令以及详细的 stderr 报错日志。
