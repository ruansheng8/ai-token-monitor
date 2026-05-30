# AGENTS.md — AI Token Monitor 智能体协作规范

> 本文件为 AI 编程助手（Antigravity、Claude Code、Codex、Cursor 等）提供操作本项目时的上下文、约束与最佳实践。读取此文件后，应严格遵循以下所有规范。

---

## 📐 项目概览

**AI Token Monitor** 是一款面向本地 AI IDE 的 Token 消耗统计仪表盘，采用：

- **前端**：React 19 + TypeScript + Tailwind CSS v4 + ECharts — 位于 `src/`
- **后端**：Rust + Tauri v2 + Axum Web 服务器 — 位于 `src-tauri/src/`
- **数据层**：本地 SQLite 缓存 + 可选 PostgreSQL，使用 Refinery 做数据库迁移
- **分发模式**：前端静态资源通过 `rust-embed` 编译进单一 `.exe`，零外部依赖

### 核心文件速查

| 文件 / 目录 | 职责 |
|---|---|
| `src/App.tsx` | 前端主页面，数据交互与图表渲染 |
| `src/index.css` | 全局玻璃拟态暗黑样式 |
| `src/components/` | 可复用 React 组件 |
| `src-tauri/src/main.rs` | 程序入口，启动后台 Axum 服务与 Tauri 窗口 |
| `src-tauri/src/server.rs` | Axum 路由、静态资源分发、配置 API |
| `src-tauri/src/db.rs` | SQLite 缓存管理、增量同步、数据聚合查询 |
| `src-tauri/src/db_adapter.rs` | SQLite / PostgreSQL 多数据库连接路由 |
| `src-tauri/src/proto.rs` | Protobuf 字节流反序列化 |
| `src-tauri/src/review.rs` | 复盘/报告相关业务逻辑 |
| `src-tauri/migrations/` | Refinery SQL 迁移脚本 |
| `src-tauri/.env` | 运行时环境配置（不提交到 Git） |
| `src-tauri/.env.example` | 环境配置模板，务必同步更新 |
| `.agents/skills/` | 项目专属 AI 技能（见下文） |
| `docs/superpowers/specs/` | 设计文档规范存放路径 |
| `docs/superpowers/plans/` | 实现计划存放路径 |

---

## 🚦 开发流程规范

### 必须遵守的工作流（硬性约束）

```
探索项目上下文 → 头脑风暴 (brainstorming skill) → 设计评审 →
编写实现计划 → 用户确认 → 逐步实现 → 构建验证 → 完成
```

1. **先理解，再动手**：修改任何非 trivial 代码前，先用 CodeGraph（`codegraph_*` 工具）或 `grep` 确认影响范围
2. **创意性工作必须走 brainstorming**：新功能、组件、行为变更，强制使用 `.agents/skills/brainstorming/SKILL.md` 技能，禁止跳过
3. **计划先行**：在实现计划获得用户明确批准前，禁止修改任何源码
4. **增量提交**：每完成一个逻辑单元后提交，保持 commit 粒度小且聚焦

### 何时不需要计划

以下属于 trivial 修改，可直接执行：
- 修复明显的编译错误 / 语法错误
- 调整 CSS 颜色、间距等纯样式值
- 补充注释或文档字符串
- 修复单行逻辑 bug（原因明确且影响范围仅限一处）

---

## 🧰 可用技能（Skills）

项目内置以下 AI 技能，使用前必须先用 `view_file` 读取对应 `SKILL.md`：

### 1. `brainstorming`
**路径**：`.agents/skills/brainstorming/SKILL.md`
**触发时机**：任何创意性工作开始前——创建功能、构建组件、修改行为。此技能有**硬性拦截门（HARD-GATE）**，未呈现设计并获得用户批准前不得执行任何实现动作。

### 2. `dashboard-ui-style`
**路径**：`.agents/skills/dashboard-ui-style/SKILL.md`
**触发时机**：创建或修改任何看板页面、ECharts 图表组件、KPI 卡片、统计面板或数据可视化界面时。此技能定义了全局唯一的八色调色板、tooltip 样式、卡片玻璃拟态规范，**必须遵循以确保视觉一致性**。

---

## 🦀 Rust / Tauri 后端规范

### 架构约束

- **主线程不阻塞**：所有 I/O 密集型操作（SQLite 写入、PostgreSQL 查询、文件扫描）必须在 `tokio::spawn` 或 `spawn_blocking` 中执行
- **锁粒度最小化**：`db.rs` 中已有 `Mutex` / `RwLock` 保护，修改时评估是否会引入死锁
- **环境配置热重载**：数据库连接变更后，必须通过现有的热重载机制生效，禁止要求用户重启服务
- **新 API 端点**：在 `server.rs` 中添加路由，在 `db.rs` 或对应模块中实现逻辑，保持职责分离

### 数据库迁移

- 所有 schema 变更必须通过 `src-tauri/migrations/` 下的 Refinery 脚本完成
- 脚本命名格式：`V{版本号}__{描述}.sql`（例：`V3__add_review_table.sql`）
- 禁止在应用代码中手动执行 `CREATE TABLE` 或 `ALTER TABLE`

### 构建验证

修改 Rust 代码后必须验证编译通过：
```powershell
# 快速语法检查（推荐优先使用）
cd src-tauri; cargo check

# 完整 Release 构建
pnpm build && cd src-tauri && cargo build --release
```

---

## ⚛️ 前端（React + TypeScript）规范

### 视觉风格（硬性约束）

本项目采用**流体渐变玻璃拟态暗黑风格**，所有 UI 组件必须：

- 遵循 `.agents/skills/dashboard-ui-style/SKILL.md` 中的设计规范
- ECharts 图表颜色统一使用八色调色板（禁止自造颜色）
- 卡片圆角 ≥ 24px，使用半透明背景和漫反射阴影
- 图表 tooltip 统一使用白底卡片式设计，数值使用 monospace 字体

### 代码规范

- **组件拆分原则**：单个组件超过 300 行时，考虑拆分；`App.tsx` 已较大（~177KB），新功能优先拆成独立组件
- **ECharts 优化**：`option` 对象必须用 `useMemo` 缓存；`ReactECharts` 必须带 `notMerge` 和 `lazyUpdate` 属性
- **TypeScript**：禁止使用 `any`，API 响应必须定义对应的 interface / type
- **样式**：优先使用 Tailwind CSS 工具类；需要 CSS 变量时定义在 `src/index.css`

### API 调用约定

- 所有后端 API 均通过相对路径 `/api/...` 调用（开发环境通过 Vite proxy 转发到 `localhost:19362`）
- 错误处理：API 调用失败时，必须在 UI 上显示用户可理解的错误信息，不能只打 console.error

### 构建验证

修改前端代码后验证：
```powershell
# 类型检查
npx tsc -b --noEmit

# Lint 检查
npm run lint

# 构建前端资源
npm run build
```

---

## 🌍 语言与输出规范

- **代码内部**：变量名、函数名、类名、注释均使用英文
- **用户可见文本**（UI 标签、提示信息、错误消息）：使用**中文**
- **AI 助手回复与工件（Artifacts）**：一律使用**中文**，包括 `task.md`、`implementation_plan.md`、`walkthrough.md`

---

## 📁 文档与工件存放规范

| 文件类型 | 存放路径 |
|---|---|
| 设计规范文档（Spec） | `docs/superpowers/specs/YYYY-MM-DD-<topic>-design.md` |
| 实现计划 | `docs/superpowers/plans/YYYY-MM-DD-<topic>-plan.md` |
| AI 助手任务追踪 | AI 工具的 artifact 目录（非项目 repo 内） |
| 临时调试脚本 | `scratch/` 目录（已在 `.gitignore` 中） |

---

## ✅ 实现前自检清单

在开始编码前，确认以下所有项目：

- [ ] 已阅读本 `AGENTS.md` 全文
- [ ] 已读取相关技能的 `SKILL.md`（brainstorming / dashboard-ui-style）
- [ ] 已用 `codegraph_context` 或 `codegraph_impact` 评估影响范围
- [ ] 已获得用户对实现计划的明确批准
- [ ] 了解本次修改涉及的 Rust 模块职责边界
- [ ] 确认不会破坏现有的热重载机制

## ✅ 提交前自检清单

- [ ] `cargo check`（或 `cargo build --release`）通过，无编译错误
- [ ] `tsc -b --noEmit` 通过，无类型错误
- [ ] `npm run lint` 通过，无 ESLint 错误
- [ ] ECharts 组件遵循 dashboard-ui-style 规范（八色板 / tooltip / notMerge）
- [ ] 新增 API 端点已在 `server.rs` 注册路由
- [ ] 若有 schema 变更，已创建对应的 Refinery 迁移脚本
- [ ] 若修改了 `.env` 相关逻辑，已同步更新 `.env.example`

---

## 约定/规范

- 约定式提交 (Conventional Commits)：使用 `feat:`、`fix:`、`refactor:`、`test:`、`chore:` 等前缀。
- **生成 git commit 消息的时候使用简体中文。**

## 🚫 禁止行为

- **禁止**在未获用户批准前修改生产代码（非 trivial 变更）
- **禁止**硬编码用户路径（`C:\Users\某人\...`），必须使用环境变量（`%USERPROFILE%`）
- **禁止**在主线程或 Axum handler 中执行同步阻塞 I/O
- **禁止**跳过 brainstorming skill 直接实现新功能
- **禁止**在 ECharts 图表中使用调色板以外的颜色
- **禁止**提交包含 `.env`（含真实密码）的内容
- **禁止**在 `App.tsx` 中继续堆砌大量代码，新功能必须拆分为独立组件

---

*最后更新：2026-05-29 | 与项目同步演进，每当架构或规范发生重大变更时请同步修改此文件。*
