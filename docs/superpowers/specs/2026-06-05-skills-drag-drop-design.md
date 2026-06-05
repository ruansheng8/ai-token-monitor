# 诊断技能管理器拖拽导入功能设计规范

本设计规范针对“诊断技能管理器”拖拽导入文件/文件夹没有效果的问题，提出了在 Tauri v2 环境下启用原生拖拽并支持本地绝对路径导入的设计方案。

## 1. 背景与问题分析

当前“诊断技能管理器”采用了标准的 HTML5 Drag and Drop API (`onDragOver`, `onDrop` 等) 进行实现。但在 Tauri v2 中：
1. 默认配置 `dragDropEnabled` 为 `false` 时，操作系统级的文件拖拽被 WebView2 拦截并禁用，用户无法将本地文件拖拽进应用窗口。
2. 即使将 `dragDropEnabled` 设为 `true`，标准 HTML5 拖拽事件在 WebView 容器中对外部系统文件通常不会触发或被 native 拦截。
3. 因此，必须使用 Tauri 窗口级别的 `onDragDropEvent` 监听原生拖拽事件，获取绝对路径，并由后端直接读取这些路径完成解压或拷贝导入。

---

## 2. 详细设计

### 2.1 Tauri 配置层变更

修改 `src-tauri/tauri.conf.json`，启用 `dragDropEnabled`：

```json
{
  "app": {
    "windows": [
      {
        "title": "AI 用量统计仪表盘",
        "width": 1350,
        "height": 850,
        "dragDropEnabled": true
      }
    ]
  }
}
```

### 2.2 后端 API 设计 (Rust)

在 `src-tauri/src/review.rs` 中新增处理绝对路径导入的 Axum 接口：

* **路由**：`POST /api/review/skills/import`
* **请求结构**：
  ```rust
  #[derive(serde::Deserialize)]
  pub struct ImportSkillsRequest {
      pub paths: Vec<String>,
  }
  ```
* **核心业务逻辑**：
  1. 确保目标自定义技能目录（`get_user_skills_dir()`）存在。
  2. 遍历 `paths`：
     - 若路径指向文件：
       - 验证后缀必须是 `.zip` 或 `.7z`；
       - 读取文件字节数据；
       - 在 `temp_upload` 下创建一个以随机 UUID 命名的临时文件夹，将数据解包至该目录；
       - 对临时文件夹运行校验函数 `validate_uploaded_skills`；
       - 将校验通过的技能目录递归复制到自定义技能目标目录中；
       - 清理临时文件夹。
     - 若路径指向目录：
       - 对该路径直接运行校验函数 `validate_uploaded_skills`；
       - 将校验通过的技能目录递归复制到自定义技能目标目录中。
  3. 收集并返回已成功导入的所有技能详情（`Vec<SkillInfo>`），格式与手动上传接口完全一致。
  4. 若有任何校验失败或 IO 异常，返回 `400 Bad Request` 并附带具体失败原因。

在 `src-tauri/src/main.rs` 中注册此接口路由：

```rust
.route("/api/review/skills/import", post(handle_import_skills))
```

### 2.3 前端 React 组件集成 (`SkillManagerModal.tsx`)

* **检测运行时**：使用全局导出的 `isTauriRuntime()` 判断是否为 Tauri 客户端。
* **原生拖拽监听**：
  在 Modal 打开后，启动一个 `useEffect`：
  - 动态导入 `@tauri-apps/api/webviewWindow` 获取 `getCurrentWebviewWindow`。
  - 调用 `appWindow.onDragDropEvent(event => { ... })` 监听系统拖拽事件。
  - 处理事件类型：
    - `hover`: 设置 `setDragActive(true)`；
    - `cancel`: 设置 `setDragActive(false)`；
    - `drop`: 设置 `setDragActive(false)`，获取 `event.payload.paths` 并调用 `importPaths` 发起 POST 请求；
  - 卸载时，在 cleanup 函数中执行 `unlisten()` 取消事件绑定，防止内存泄露。
* **数据提交通道**：
  实现 `importPaths(paths: string[])` 方法，调用接口 `/api/review/skills/import`，并妥善处理 loading 状态、成功提醒 (`setSuccessSkills`) 和错误提示 (`setErrorMsg`)。

---

## 3. 校验与测试计划

### 3.1 自动化测试
* 新增对 `/api/review/skills/import` 逻辑的单元测试，涵盖：
  - 模拟文件夹直接导入校验。
  - 模拟无效后缀名文件的拦截。

### 3.2 手动功能测试
1. **压缩包拖入测试**：拖拽一个符合 Claude Skill 规范的 `.zip` 压缩包到 Modal 的拖拽区域，验证是否提示成功并展示技能信息。
2. **文件夹拖入测试**：拖拽一个符合 Claude Skill 规范的文件夹到 Modal 的拖拽区域，验证是否提示成功。
3. **多选拖入测试**：同时选中一个压缩包和一个技能文件夹拖入，验证是否能一次性解析导入。
4. **异常格式测试**：拖入一个没有 `SKILL.md` 的文件夹或 `.txt` 文件，验证是否能正确报错拦截，不破坏已有数据。
