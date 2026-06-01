# 专家分析提示词自定义维护设计规范 (Spec)

本设计规范定义了在 Token Insight 项目中引入“专家分析提示词”自定义维护界面的实现方案。用户将能够创建、编辑、克隆和删除复盘提示词模板，并支持在复盘时选用。

---

## 📐 1. 背景与目标

当前系统中，共有 4 个系统内置的“专家分析提示词”模板，直接硬编码在前端 `src/components/ReviewDrawer.tsx` 文件的 `TEMPLATE_PRESETS` 常量中。

**局限性**：
1. 用户无法新增个人偏好的提示词（例如针对特定业务重点进行诊断）。
2. 用户无法修改预设的提示词结构和表达方式。
3. 提示词与前端展示代码强耦合，不利于解耦。

**改进目标**：
在“AI 复盘与治理中心”中引入**提示词模板管理弹窗**，将提示词模板存储至本地 SQLite 中，实现模板的**增、删、改、查、克隆**功能，且为内置模板提供只读和克隆保护。

---

## 💾 2. 数据库设计 (SQLite)

在本地 SQLite 数据库中，新增 `prompt_templates` 表，用于统一存放系统内置与用户自定义的提示词模板。

### 2.1 表结构 Schema

```sql
CREATE TABLE IF NOT EXISTS prompt_templates (
    id TEXT PRIMARY KEY,           -- 唯一标识。内置模板使用固定ID (comprehensive等)；自定义模板使用 UUID
    name TEXT NOT NULL,            -- 模板名称（如: 📊 综合效能评估）
    description TEXT,              -- 模板功能简述
    template TEXT NOT NULL,        -- 提示词 Markdown 正文（包含占位符如 {{IDE}} 等）
    is_builtin INTEGER DEFAULT 0,  -- 是否为系统内置模板。1 = 内置 (只读且不可删除)；0 = 用户自定义 (可编辑和删除)
    created_at TEXT NOT NULL,      -- 创建时间，RFC3339 格式
    updated_at TEXT NOT NULL       -- 更新时间，RFC3339 格式
);
```

### 2.2 内置数据初始化 (Data Seeding)
在 SQLite 数据库初始化 `init_cache_db` 逻辑中，如果检测到 `prompt_templates` 表为空，则将原硬编码在前端的 4 个默认模板作为初始记录刷入数据库，保证开箱即用：

1. `comprehensive` - 📊 综合效能评估
2. `cost_saving` - 🔍 成本节流专项
3. `collaboration` - ⚡ 开发协作质量
4. `project_review` - 💼 项目全景复盘

以上 4 个初始模板的 `is_builtin` 字段将被强制设置为 `1`。

---

## 📡 3. 后端 API 接口设计 (Axum / Rust)

在 `src-tauri/src/server.rs` 中提供以下 RESTful 接口。

### 3.1 接口列表

#### 1. 查询模板列表
- **路径**：`GET /api/review/prompt_templates`
- **响应格式**：`Vec<PromptTemplate>`
- **返回顺序**：内置模板排在最前（`is_builtin DESC`），自定义模板紧随其后且按创建时间排序。

#### 2. 新增自定义模板
- **路径**：`POST /api/review/prompt_templates`
- **请求体**：
  ```json
  {
    "name": "我的自定义模板",
    "description": "自定义复盘视角说明",
    "template": "正文内容，支持 {{IDE}}..."
  }
  ```
- **后端行为**：自动生成 UUID 作为 `id`，将 `is_builtin` 设为 `0`，生成当前的 `created_at` 与 `updated_at` 时间戳。

#### 3. 编辑自定义模板
- **路径**：`PUT /api/review/prompt_templates/:id`
- **请求体**：同上。
- **校验逻辑**：
  - 若目标 `id` 的记录不存在，返回 `404 Not Found`。
  - 若目标记录的 `is_builtin == 1`，则**拦截并报错**（`400 Bad Request`，提示内置模板只读）。

#### 4. 删除自定义模板
- **路径**：`DELETE /api/review/prompt_templates/:id`
- **校验逻辑**：
  - 若目标记录的 `is_builtin == 1`，则**拦截并报错**（`400 Bad Request`，提示不可删除内置模板）。

---

## ⚛️ 4. 前端 UI 与交互设计 (React + Tailwind)

### 4.1 入口与触发
在复盘抽屉的提示词选择标签上方（`src/components/ReviewDrawer.tsx` 中的“📋 选择专家分析切入点/模板”文本右侧），添加一个高质感玻璃拟态的 **⚙️ 管理模板** 按钮。

### 4.2 管理弹窗 (PromptManagerModal)
点击按钮后弹出一个居中 Modal，使用磨砂玻璃质感设计（`backdrop-blur-md bg-opacity-70 bg-bg-primary`）：

- **左侧：模板列表**
  - 分离渲染“内置模板”和“自定义模板”。
  - 内置模板有亮蓝色 `系统` 标签，自定义模板有紫色 `自定义` 标签。
  - 底部提供 **`+ 新建自定义模板`** 按钮。
- **右侧：详细编辑区**
  - **选中系统模板时**：所有输入框（名称、描述、正文）均呈 `disabled` 禁用状态。表单底部显示醒目文案：“系统内置模板不支持直接修改，您可以克隆为副本进行编辑。”，并提供一个亮色的 **`克隆副本`** 按钮。
  - **选中自定义模板时**：所有输入框可编辑。底部提供 **`保存修改`** 按钮与 **`删除模板`** 按钮（删除时需进行二次弹窗确认）。
  - **点击新建/克隆时**：右侧编辑区清空/加载被克隆源的内容，所有框处于可编辑状态，底部呈现 **`创建模板`** 与 **`取消`** 按钮。

### 4.3 占位符交互辅助
在右侧的“提示词内容”编辑框下方，提供常用占位符提示，例如：
- `{{IDE}}`：自动替换为用户在页面选中的开发工具（如 Cursor, Claude Code）
- *注：系统大盘指标数据如 `{{TOTAL_TOKENS}}` 等属于内置硬替换，将自动兼容*

---

## 🧪 5. 验证与测试方案

### 5.1 自动化测试
1. **编译检查**：确保 Rust 编译无误。
   ```powershell
   cd src-tauri; cargo check
   ```
2. **前端类型检查**：确保没有 TypeScript 类型错误。
   ```powershell
   npx tsc -b --noEmit
   ```

### 5.2 手动功能测试用例
1. **内置数据检查**：全新打开或重置数据库，确认数据库已初始化 4 个内置模板记录，且接口成功按顺序吐出。
2. **只读约束校验**：通过 Postman / Curl 发送删除或修改 `comprehensive` 的请求，确认后端正确拦截并拒绝。
3. **新建与克隆**：在界面中创建新模板以及克隆默认模板，编辑并保存，观察大盘下拉列表是否实时同步该新模板。
4. **删除校验**：删除自定义模板，查看该模板是否在下拉列表和数据库中彻底消失。
