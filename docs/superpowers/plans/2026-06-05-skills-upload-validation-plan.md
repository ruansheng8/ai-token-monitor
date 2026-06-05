# 诊断技能管理器文件夹上传与前置校验实现计划

## 1. 目标与范围
支持“诊断技能管理器”直接拖入文件夹或选择文件夹上传，并在后端保存持久化之前进行统一的 SKILL.md 格式与安全性前置校验。

---

## 2. 拟议变更

### 2.2 前端部分

#### [MODIFY] [SkillManagerModal.tsx](file:///d:/VibeCoding/ai-token-monitor/src/components/SkillManagerModal.tsx)
- 在上传区域添加两个独立的隐藏 input，分别针对普通文件（`.zip,.7z`）和文件夹（带 `webkitdirectory`）。
- 上传区域修改为提供两个按钮：“选择压缩文件”与“选择技能文件夹”。
- 在 `handleDrop` 中，使用 `item.webkitGetAsEntry()` 递归读取拖入的文件夹。
- 实现 `traverseDirectory` 递归辅助函数以扁平化拖入的文件夹文件列表，并填充自定义的 `relativePath`。
- 修改 `uploadFile` 函数为 `uploadFiles(files: File[])`，如果是一个 `.zip` 或 `.7z` 压缩包，通过原表单字段 `file` 上传；如果是文件夹子文件列表，遍历并附加到 `files` 字段，以其相对路径名作为 multipart 的 filename。

---

### 2.3 后端部分

#### [MODIFY] [review.rs](file:///d:/VibeCoding/ai-token-monitor/src-tauri/src/review.rs)
- 重构 `handle_upload_skills` 接口：
  - 读取 multipart 中的全部字段。
  - 判断是单压缩包文件还是文件夹子文件集。
  - 对于压缩包，依旧解压至临时目录。
  - 对于多文件，在沙箱临时目录 `temp_upload/temp_xxx/` 下安全重构目录结构并写入文件。
  - **安全性路径防御**：验证每个写入文件的路径不包含 `..`、不以斜杠开头，防止跳出临时沙箱。
- 提取并实现统一的校验函数 `validate_uploaded_skills(temp_dir: &Path, default_folder_name: &str) -> Result<Vec<(PathBuf, String)>, String>`:
  - 调用 `find_skills_directories`。
  - 若 `found_dirs` 为空，返回错误信息。
  - 读取每个目录下的 `SKILL.md`，使用 `parse_skill_md_frontmatter` 解析，确保其前言有有效的 `name`。若没有，则校验失败返回错误。
- 校验通过后，在持久化步骤前清理已有的同名自定义目录，执行 `copy_dir_all` 拷贝到 `skills/user/` 中。
- 无论最终成功与否，在 `finally` 块（或相应的 `drop` 守卫/清理分支）中强制物理删除临时沙箱目录。

---

## 3. 验证计划

### 3.1 自动化测试
- 编写 Rust 集成测试，对带有损坏 YAML 或缺失 `SKILL.md` 的临时目录进行验证，确保 `validate_uploaded_skills` 返回 `Err`，并且没有文件被复制到 `skills/user/`。

### 3.2 手动验证
- 上传正常 `.zip` 或 `.7z` 包 -> 正常安装。
- 上传不含 `SKILL.md` 或 `SKILL.md` 缺少 `name` 的压缩包 -> 校验报错且不影响已有技能。
- 拖入/选择普通合法技能文件夹 -> 正常安装。
- 拖入/选择不合法的文件夹 -> 校验报错且不影响已有技能。
