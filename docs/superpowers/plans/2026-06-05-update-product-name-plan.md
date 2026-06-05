# 修改 Windows 桌面图标和开始菜单程序名为 Token Insight 实现计划

本计划旨在将项目的 Windows 桌面图标、开始菜单以及系统安装程序中的应用名称修改为 `Token Insight`（目前显示为小写的 `token-insight`）。

## 变更背景与范围

用户希望在 Windows 桌面图标和开始菜单中程序名显示为 `Token Insight`。通过修改 Tauri 配置中的 `productName` 可以实现此目标。

## 提议的变更

### 修改 `src-tauri/tauri.conf.json`

将 `productName` 修改为 `Token Insight`。

文件链接：[tauri.conf.json](file:///d:/VibeCoding/ai-token-monitor/src-tauri/tauri.conf.json)

```json
{
  "productName": "Token Insight",
  "identifier": "com.tokeninsight.app",
  ...
}
```

## 数据迁移与兼容性说明

1. 本项目的本地数据存储路径（如 SQLite 数据库 `token_stats.db` 以及复盘任务报告）在后端代码中硬编码为用户主目录下的 `.token-insight` 目录（例如 `C:\Users\<Username>\.token-insight`）。
2. 修改 `productName` 不会改变后端代码中对 `.token-insight` 目录的访问逻辑。
3. 因此，本次修改**不会影响任何现有用户的数据存储，不需要进行数据迁移**。

## 验证计划

### 1. 编译验证
在项目根目录下，首先进入 `src-tauri` 目录执行代码检查：
```powershell
cd src-tauri
cargo check
```

### 2. 打包与手动验证
在本地打包或开发运行，确认：
- 编译通过且无错误。
- 运行时的窗口标题与任务栏图标名称正确显示为 `Token Insight`。
- 打包安装后，桌面快捷方式和开始菜单项名显示为 `Token Insight`。
