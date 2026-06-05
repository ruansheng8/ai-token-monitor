# 诊断技能上传显示描述实现计划

## 1. 目标与范围
在上传自定义 AI 诊断技能规范时，增强后端解析提取 `SKILL.md` 描述的鲁棒性，并在前端上传成功后，将技能名称与描述直观呈现给用户。

---

## 2. 拟议变更

### 2.1 后端部分

#### [MODIFY] [review.rs](file:///d:/VibeCoding/ai-token-monitor/src-tauri/src/review.rs)
- **改进 `parse_skill_md_frontmatter`**:
  - 新增兜底逻辑：若 Frontmatter 中解析不到 `name`，搜索第一个 `#` 标题。
  - 新增兜底逻辑：若 Frontmatter 中解析不到 `description`，遍历提取首个非特殊字符开始的正文段落（限制 300 字符以内）。
- **重构 `handle_upload_skills` 接口返回值**:
  - 从 `validated_skills` 中解析出已被拷贝到持久化目录的技能列表，并将 `SkillInfo` 转化为 JSON 格式序列化后，以 `application/json` 头部返回。

---

### 2.2 前端部分

#### [MODIFY] [SkillManagerModal.tsx](file:///d:/VibeCoding/ai-token-monitor/src/components/SkillManagerModal.tsx)
- 在 `SkillManagerModal` 组件中添加 `successSkills` 和 `setSuccessSkills` 状态。
- 修改 `uploadFiles` 处理成功上传后的逻辑：
  - 调用 `res.json()` 获取已上传的技能列表。
  - 存入 `successSkills`。
  - 开始上传或发生错误时，将 `successSkills` 置空。
- 在页面渲染部分（上传组件下方、技能列表上方）增加一个绿色的提示框：
  - 显示成功消息 `🎉 技能上传并解析成功！` 和关闭按钮。
  - 遍历 `successSkills` 列表，显示每个技能的 `name` 及其 `description`（带“描述：”前缀）。

---

## 3. 验证计划

### 3.1 自动化测试
- 运行后端测试，确保 `parse_skill_md_frontmatter` 能够正确且鲁棒地对无 YAML Frontmatter、多行 YAML 以及常规格式的 `SKILL.md` 提取技能名称与描述：
  - 运行 `cd src-tauri; cargo test` 并验证成功。

### 3.2 手动验证
- 打开 "AI 复盘与治理中心" -> 点击 "第四步：启用 AI 诊断技能规范" -> 点击 "管理技能"。
- 上传包含合法 Frontmatter 的 `.zip` 压缩包：
  - 预期弹出成功提示框，且框中正确显示技能名称与描述。
- 上传不含 Frontmatter 但包含 `# Title` 和正文的自定义文件夹：
  - 预期上传成功，并且成功提示框及技能列表中，技能名称为正文标题，技能描述为正文第一段。
- 确认点击成功提示框的 `✕` 可正常关闭。
