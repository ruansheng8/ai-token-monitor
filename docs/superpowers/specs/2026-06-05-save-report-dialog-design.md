# 设计规范：点击保存报表图片时通过对话框选择目录保存

本文档定义了在 Token Insight 中点击“保存图片报表”时弹窗选择目录保存，并提供保存成功后立即打开文件路径的交互设计规范。

## 需求背景
目前点击“保存图片报表”后，系统会默认以类似浏览器 `<a download>` 的机制尝试下载图片，而在 Tauri 桌面客户端环境下，该体验不甚理想且用户无法自由选择保存目录。
我们需要为用户提供在本地桌面端点击保存时，自动弹出一个原生系统目录选择对话框，默认定位到用户下载目录（`C:\Users\<Username>\Downloads`），并支持在保存成功后，弹窗提示并允许一键打开该文件在系统资源管理器中的位置。

## 设计规范

### 1. 默认路径定位
- 严禁硬编码具体的系统用户路径（如 `C:\Users\cearn\Downloads`）。
- 必须通过读取系统环境变量 `%USERPROFILE%`（在 Windows 上）或 `$HOME`（在 macOS/Linux 上）动态构建默认路径：
  - Windows: `%USERPROFILE%\Downloads`
  - macOS/Linux: `$HOME/Downloads`
  - 如果读取失败，回退到当前可执行文件同级目录。

### 2. 重名冲突解决
- 在用户选定的目录下，生成默认文件名称：`Token_Insight_图片报表_YYYY-MM-DD.png`。
- 如果该目录下已存在同名文件，为了防止静默覆盖用户的历史报表，应采用自动递增重命名策略：
  - `Token_Insight_图片报表_YYYY-MM-DD (1).png`
  - `Token_Insight_图片报表_YYYY-MM-DD (2).png`
  - 依此类推。

### 3. API 接口定义

#### 保存图片接口
- **路径**：`POST /api/report/save`
- **请求体**：
  ```json
  {
    "image_base64": "data:image/png;base64,iVBORw0KG..."
  }
  ```
- **响应体（成功）**：
  ```json
  {
    "success": true,
    "file_path": "C:\\Users\\cearn\\Downloads\\Token_Insight_图片报表_2026-06-05.png"
  }
  ```
- **响应体（取消）**：
  ```json
  {
    "success": false,
    "cancelled": true
  }
  ```
- **响应体（失败）**：
  ```json
  {
    "success": false,
    "message": "错误原因描述"
  }
  ```

#### 打开文件夹接口
- **路径**：`POST /api/report/open`
- **请求体**：
  ```json
  {
    "path": "C:\\Users\\cearn\\Downloads\\Token_Insight_图片报表_2026-06-05.png"
  }
  ```
- **响应体**：
  ```json
  {
    "success": true
  }
  ```
- **后端执行行为**：
  - Windows: 执行 `explorer.exe /select,"C:\path\to\file.png"`，使得资源管理器打开并高亮选中保存的文件。
  - macOS: 执行 `open -R /path/to/file.png`。
  - Linux: 执行 `xdg-open /path/to/parent_dir`。

### 4. 前端交互流
1. 用户在“图片报表超清图片预览 Modal”中点击 **📥 保存图片报表** 按钮。
2. 前端显示 Loading 状态，并将 Base64 数据通过 API 请求发送至后端 `/api/report/save`。
3. 后端唤起系统目录选择框。
4. 用户选择好目录并确认后，后端执行写入。
5. 后端写入成功后返回响应，前端结束 Loading。
6. 前端展示二次确认框：
   - 文本内容：`🎉 图片报表保存成功！\n保存位置：C:\Users\...\Downloads\Token_Insight_图片报表_2026-06-05.png\n\n是否立即在文件夹中打开该图片？`
   - 如果用户点击“确认”，前端调用 `/api/report/open` 接口，系统资源管理器自动打开并高亮定位文件。
