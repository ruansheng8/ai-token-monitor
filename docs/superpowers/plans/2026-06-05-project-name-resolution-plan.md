# 精确项目名解析实现计划

本计划的目标是为 `token-insight` 项目实现与 `claude-code-history-viewer` 相同的精确项目名解析逻辑。

## 用户审核要求

> [!NOTE]
> 我们需要新建 `src-tauri/src/utils.rs` 并在 `main.rs` 中声明它。
> 由于您指示不需要考虑历史数据兼容性，我们将直接重写 `db.rs` 中的 `detect_project_name`。重写后，用户只需要删除旧的缓存数据库 `~/.token-insight/db/token_stats.db`，或者等待重新扫描，即可看到正确格式的项目名称。

## 方案细节与主要改动

### 1. 新建 `utils` 模块

#### [NEW] [utils.rs](file:///d:/VibeCoding/ai-token-monitor/src-tauri/src/utils.rs)
实现以下函数：
- `pub fn decode_project_path(session_storage_path: &str) -> String`
  通过 `sessions-index.json` 或磁盘探测还原项目路径。
- `fn decode_with_filesystem_check(encoded: &str) -> Option<String>`
  文件系统匹配解码。
- `fn decode_recursive_inner(encoded: &str, base_path: &str, depth: usize) -> Option<String>`
  递归路径切分与校验。
- `pub fn extract_project_name(raw_project_name: &str) -> String`
  提取叶子项目名称的公共入口。

在 `utils.rs` 中编写单元测试验证各种边界情况（如前缀剔除、本地目录存在时的解析、不存在时的 fallback）。

---

### 2. 在 `main.rs` 中注册模块

#### [MODIFY] [main.rs](file:///d:/VibeCoding/ai-token-monitor/src-tauri/src/main.rs)
在文件顶部注册 `utils` 模块：
```rust
mod utils;
```

---

### 3. 重构数据库字段提取

#### [MODIFY] [db.rs](file:///d:/VibeCoding/ai-token-monitor/src-tauri/src/db.rs)
在 `db.rs` 中修改 `detect_project_name`：
```rust
fn detect_project_name(project_path: Option<&str>) -> String {
    let path_str = match project_path {
        Some(p) => p,
        None => return "unknown-project".to_string(),
    };
    
    let path = std::path::Path::new(path_str);
    
    // 如果输入是文件，我们提取出其父目录（即形如 -d-VibeCoding-ai-token-monitor 的编码文件夹）
    // 如果输入已经是目录，则直接提取该目录名
    let raw_project_name = if path.is_file() || path.extension().is_some() {
        path.parent()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
    } else {
        path.file_name().map(|n| n.to_string_lossy().to_string())
    };

    let raw_name = match raw_project_name {
        Some(name) => name,
        None => return "unknown-project".to_string(),
    };

    // 过滤一些公共的特殊目录名
    if raw_name.is_empty() || raw_name == "sessions" || raw_name == "workspaceStorage" || raw_name == "globalStorage" {
        return "unknown-project".to_string();
    }

    // 调用移植过来的解密提取逻辑
    let parsed_name = crate::utils::extract_project_name(&raw_name);
    
    if parsed_name.is_empty() {
        "unknown-project".to_string()
    } else {
        parsed_name
    }
}
```

---

## 验证计划

### 自动化测试
运行已有的测试和新增的测试以进行验证：
```powershell
cd src-tauri
cargo test
```

### 手动验证
1. 在 Windows 上运行测试，验证新编写的 `utils.rs` 解析是否正确。
2. 确认 `cargo test` 运行成功，所有测试无报错。
3. （由于是本地分析，本计划暂不启动 Tauri 界面，只在后端逻辑进行验证确保编译和测试完全成功）。
