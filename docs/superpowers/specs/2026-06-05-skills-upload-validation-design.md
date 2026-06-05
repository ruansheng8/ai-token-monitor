# 诊断技能管理器文件夹上传与前置校验设计规约

## 1. 目标与背景
本设计旨在为 "诊断技能管理器" 提供：
1. 直接拖入文件夹或点击选择文件夹上传的功能支持。
2. 在技能持久化（真正保存到 `skills/user` 目录）之前，对技能包（无论是文件夹形式还是 .zip/.7z 压缩包形式）进行严格的有效性、安全性前置校验。
3. 确保校验失败时完全回滚（不破坏已有技能，不残留临时文件）。

---

## 2. 详细设计

### 2.1 前端设计 (`SkillManagerModal.tsx`)

#### 2.1.1 拖拽与选择文件夹接口
- **界面按钮并列**：
  将上传区域改为点击触发两个选项：
  - "选择压缩文件 (.zip/.7z)" (使用普通文件 `<input type="file" accept=".zip,.7z">`)
  - "选择技能文件夹" (使用带有 `webkitdirectory directory multiple` 属性的 `<input type="file">`)
- **拖拽文件与文件夹识别**：
  在 `handleDrop` 中：
  - 遍历 `e.dataTransfer.items`。
  - 使用 `item.webkitGetAsEntry()` 识别每一项。
  - 如果是目录，使用异步递归读取器 `traverseDirectory` 读取出所有子文件，并为它们附带相对路径属性 `relativePath`。
  - 如果是单个压缩包文件，则作为压缩文件上传。
- **文件数据包装与发送**：
  - 单个 `.zip` 或 `.7z` 压缩包：使用原有 `FormData.append('file', file)` 上传。
  - 文件夹：遍历遍历出来的所有子文件，调用 `FormData.append('files', file, file.relativePath || file.webkitRelativePath || file.name)` 连同其相对路径名一起上传。

#### 2.1.2 拖拽识别递归辅助函数
```typescript
const traverseDirectory = async (entry: FileSystemEntry, path = ""): Promise<File[]> => {
  return new Promise((resolve) => {
    if (entry.isFile) {
      (entry as FileSystemFileEntry).file((file) => {
        // 创建带有相对路径的新 File 对象
        const fileWithPath = new File([file], file.name, { type: file.type });
        Object.defineProperty(fileWithPath, 'relativePath', {
          value: path + entry.name,
          writable: false
        });
        resolve([fileWithPath]);
      });
    } else if (entry.isDirectory) {
      const dirReader = (entry as FileSystemDirectoryEntry).createReader();
      const readAllEntries = async (): Promise<FileSystemEntry[]> => {
        let allEntries: FileSystemEntry[] = [];
        const read = (): Promise<FileSystemEntry[]> => {
          return new Promise((res) => {
            dirReader.readEntries((entries) => res(entries));
          });
        };
        let entries = await read();
        while (entries.length > 0) {
          allEntries = allEntries.concat(entries);
          entries = await read();
        }
        return allEntries;
      };

      readAllEntries().then(async (entries) => {
        const promises = entries.map(e => traverseDirectory(e, path + entry.name + "/"));
        const results = await Promise.all(promises);
        resolve(results.flat());
      });
    }
  });
};
```

---

### 2.2 后端设计 (`review.rs`)

#### 2.2.1 统一的 `/api/review/skills/upload` 路由逻辑
接口不再仅支持单字段单个压缩包，而是同时处理：
1. **单个字段 `"file"`（且文件名为 `.zip` 或 `.7z`）**：
   解压到临时目录 `temp_upload/temp_xxx/`。
2. **多字段 `"files"`（或多个文件项）**：
   在 Multipart 中读取出所有子文件，获取每个字段的文件名（包含相对路径，如 `my-skill/SKILL.md`）。
   - **安全性路径过滤**：对文件名作合法性及防遍历校验，不允许以 `/` 或 `\` 开头，不允许包含 `..` 字符。
   - 在临时目录 `temp_upload/temp_xxx/` 下创建子目录结构，写入二进制文件数据。

#### 2.2.2 统一校验逻辑 (`validate_uploaded_skills`)
在解密/解压/写入完毕后，对临时目录 `temp_dir` 进行统一前置校验：
1. **扫描有效技能目录**：
   使用已有的 `find_skills_directories(&temp_dir, &mut found_dirs)` 递归定位所有包含 `SKILL.md` 的技能子目录。
   - 如果没有找到任何包含 `SKILL.md` 的子目录，校验失败，返回错误 `"压缩包或目录中未检测到符合 Claude 技能规范的 SKILL.md 文件"`。
2. **读取并校验 `SKILL.md`**：
   - 对每个 `skill_dir`，读取 `SKILL.md` 文本内容。
   - 解析 YAML 前言。
   - **硬性约束**：前言中必须包含有效的 `name` 字段（非空，非空白字符）。如果 `name` 为空，校验失败，返回错误 `"技能「xxx」的 SKILL.md 缺少有效的 name 前言字段"`。

#### 2.2.3 持久化移入
- 只有**校验全部通过**后，才开始遍历 `found_dirs` 进行物理复制：
  - 提取技能的最终目录名 `folder_name`。
  - 清理已存在的同名自定义技能目录。
  - 调用 `copy_dir_all(&skill_dir, &target_dir)` 写入 `skills/user/` 持久化目录。
- **沙箱清理**：在 `fn handle_upload_skills` 的末尾（无论成功还是报错退出），一律强制删除临时目录 `temp_upload/temp_xxx/`。

---

## 3. 验证方案

### 3.1 单元测试与集成测试
- 新增 Rust 测试用例，测试 `validate_uploaded_skills` 的过滤能力：
  - 测试空技能目录（无 SKILL.md）。
  - 测试包含 `SKILL.md` 但没有 name 前言或 YAML 格式损坏的情况。
  - 确认路径穿越校验是否起效。

### 3.2 手动验证
1. **压缩包上传验证**：
   - 上传一个带有有效 `SKILL.md` 的 `.zip` 或 `.7z` 压缩包，校验成功，安装正常。
   - 上传一个缺少 `SKILL.md` 或 `SKILL.md` 缺少 `name` 前言的 `.zip` 压缩包，提示校验失败，原技能目录无影响。
2. **拖入/选择文件夹验证**：
   - 直接拖入一个有效的技能文件夹，或选择它，校验成功，安装正常。
   - 直接拖入一个无效的文件夹，页面弹出提示“缺少有效的 name 前言字段”或“未检测到符合规范的 SKILL.md”，安装终止，原技能目录无影响。
