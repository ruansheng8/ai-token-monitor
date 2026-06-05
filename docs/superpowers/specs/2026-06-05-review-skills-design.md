# 2026-06-05 AI 复盘与治理中心支持 SKILLS 设计规范

本文档详述了在 `AI 复盘与治理中心` 分析报告时支持技能规范（SKILLS），并支持用户自己上传（`.zip` 或 `.7z` 压缩包）维护自定义技能的设计方案。

---

## 1. 目标与用例

### 目标
1. 允许用户维护自定义 AI 分析技能（Skills），通过上传压缩包（`.zip` / `.7z`）的形式在本地持久化。
2. 允许用户在新建 AI 复盘任务时勾选一个或多个技能。
3. 后端在生成复盘 Prompt 时，自动载入这些启用的技能规范作为上下文，使复盘分析更具针对性。
4. 追溯历史复盘任务时，能够获取其运行阶段勾选的技能。

### 用例
* **用例 1**：用户编写了自己的代码审查或 Token 消耗诊断规范 `SKILL.md`，打成 `.zip` 包并上传到系统中。系统自动解包并保存为自定义技能。
* **用例 2**：用户在复盘页面勾选了“内置脑暴规范 (brainstorming)”和“自定义节流规范”，并生成复盘报告，AI 引擎结合这些技能给出分析。
* **用例 3**：用户删除已经不需要的自定义技能。

---

## 2. 系统架构与数据存储

### 存储路径
所有技能文件存放在本地文件系统，物理区分为：
1. **项目内置技能 (Read-Only)**：当前工作空间根目录下的 `.agents/skills/` 目录。
2. **用户全局技能 (Read-Write)**：用户全局配置目录 `%USERPROFILE%/.token-insight/skills/` 下。

上传解压后的技能目录树如下：
```
%USERPROFILE%/.token-insight/skills/
├── custom-skill-a/
│   └── SKILL.md
└── custom-skill-b/
    ├── SKILL.md
    └── examples/
```

### 数据库升级
在 `review_tasks` 表中，新增 `selected_skills_json` 字段，以存储任务执行时勾选启用的技能 ID 列表（JSON Array 格式）。

**升级机制**：在 `src-tauri/src/db.rs` 中的 `init_cache_db` 自动执行增量升级：
```sql
ALTER TABLE review_tasks ADD COLUMN selected_skills_json TEXT DEFAULT NULL;
```

---

## 3. 后端 API 接口设计

### 3.1 获取所有可用技能
* **端点**：`GET /api/review/skills`
* **逻辑**：
  1. 扫描当前工作空间目录下的 `.agents/skills`（若存在），读取子目录的 `SKILL.md`，解析 Frontmatter。将其标记为 `is_builtin: true`。
  2. 扫描全局配置目录 `%USERPROFILE%/.token-insight/skills`，读取其子目录的 `SKILL.md`，解析 Frontmatter。标记为 `is_builtin: false`。
  3. 整合并返回技能数组。
* **响应**：
  ```json
  [
    {
      "id": "brainstorming",
      "name": "brainstorming",
      "description": "脑暴流程规范说明",
      "is_builtin": true
    },
    {
      "id": "my-cost-rules",
      "name": "我的降本规范",
      "description": "关于如何拦截高额消耗的自定义诊断规范",
      "is_builtin": false
    }
  ]
  ```

### 3.2 上传自定义技能压缩包
* **端点**：`POST /api/review/skills/upload`
* **格式**：`multipart/form-data`
* **逻辑**：
  1. 引入 Rust 第三方解压库 `zip` (Deflate 支持) 与 `sevenz-rust`。
  2. 提取 multipart 中的压缩包字节流，并写入临时文件。
  3. 解压至 `%USERPROFILE%/.token-insight/temp_upload/<UUID>/` 临时目录下。
  4. 递归遍历该目录，寻找所有包含 `SKILL.md` 的文件夹。
  5. 将包含 `SKILL.md` 的文件夹整体移动或覆盖写入 `%USERPROFILE%/.token-insight/skills/<技能ID>/`（技能 ID 为文件夹名，若在根目录则以压缩包去后缀名作为 ID）。
  6. 删除临时目录及临时压缩包文件。

### 3.3 删除自定义技能
* **端点**：`DELETE /api/review/skills/:id`
* **逻辑**：
  1. 校验 `:id` 的合法性（防止路径遍历攻击），确保其仅包含英文字母、数字、下划线及横杠。
  2. 物理删除目录 `%USERPROFILE%/.token-insight/skills/:id/`。
  3. 内置技能不支持删除操作（返回 403 Forbidden）。

### 3.4 复盘任务接口升级
* **端点**：`POST /api/review/tasks`
* **请求体升级**：
  ```json
  {
    "time_range": "7天",
    "cli": "claude",
    "metrics_snapshot": { ... },
    "selected_ides": ["all"],
    "custom_prompt": "...",
    "skills": ["brainstorming", "my-cost-rules"] // 新增字段
  }
  ```
* **Prompt 拼接逻辑**：
  在生成 `prompt_text` 的尾部，读取所勾选技能对应的 `SKILL.md`，格式化后追加拼接进 Prompt 中：
  ```markdown
  ## 🛠️ 必须遵循的 AI 诊断技能与行为准则 (Skills Context)
  
  ### 技能: [技能名称]
  [SKILL.md 的完整文本内容]
  ```

---

## 4. 前端 UI 交互设计

### 4.1 主配置卡片
在 `ReviewDrawer.tsx` 中配置分析参数板块，新增**分析技能（Skills）**卡片：
* 以勾选卡片列表展示当前可用技能，支持多选。
* 勾选状态缓存于前端 `localStorage` 中。
* 右上角配置“管理自定义技能”按钮，点击触发技能管理器。

### 4.2 技能管理器模态框 (SkillManagerModal)
* 提供 Drag & Drop / 点击上传控件，只接收 `.zip` 或 `.7z` 扩展名。
* 列表展示所有技能，已标记“项目内置”的技能删除按钮置灰，用户上传的技能显示“红色删除”图标。
* 删除时进行二次确认弹窗提示。

---

## 5. 验证与测试用例

1. **测试用例 1：内置技能扫描**
   * 系统启动后，直接请求 `GET /api/review/skills`，应当能够正确扫描并解析项目 `.agents/skills` 下已有的 `brainstorming`。
2. **测试用例 2：自定义技能上传 (Zip)**
   * 打包一个包含 `SKILL.md` 文件的 zip 包，通过界面上传，验证解压是否成功，且 `GET /api/review/skills` 列表刷新并正确显示该自定义技能。
3. **测试用例 3：自定义技能上传 (7z)**
   * 打包一个包含 `SKILL.md` 的 7z 压缩包并上传，验证 `sevenz-rust` 是否能正确解压。
4. **测试用例 4：Prompt 包含技能**
   * 创建任务并勾选该自定义技能，在 SQLite 数据库中检查 `prompt_text` 字段，确认技能内容已成功拼入，且 `selected_skills_json` 存储了该技能 ID。
5. **测试用例 5：自定义技能删除**
   * 点击删除自定义技能，验证对应的文件夹已被物理清除，列表重新刷新。
