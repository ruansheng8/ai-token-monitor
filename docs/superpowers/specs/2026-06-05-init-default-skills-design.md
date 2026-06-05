# Token Insight 默认技能初始化设计文档 (Spec)

本文档定义了在程序运行时以非阻塞方式自动初始化和更新默认技能的机制。

## 需求背景
1. 系统提供了一套系统内置/默认的诊断技能（例如 `token-insight-report.zip`），存放在 `assets/skills-default` 目录下。
2. 在安装程序（NSIS/MSI）时，由于用户家目录路径难以准确预测（UAC 权限提升可能导致 `%USERPROFILE%` 指向管理员用户而不是当前登录用户），且安装器不便集成复杂的 ZIP 解包功能，因此将初始化移至**程序启动运行时**。
3. **性能约束**：初始化动作必须在后台异步线程执行，不允许阻塞程序主界面的渲染与 Axum 接口服务的绑定。
4. **防重复解压与升级更新**：如果已经解压过且软件没有升级，则跳过解压；若软件发生版本更新，需自动覆盖更新默认技能目录。

## 架构与数据流设计

### 1. 资源嵌入 (Compile-time Embedding)
使用 `rust-embed` 将外部 `assets/skills-default/` 下的所有 ZIP 文件在编译期静态打包进二进制可执行文件中：
```rust
#[derive(rust_embed::RustEmbed)]
#[folder = "../assets/skills-default/"]
struct DefaultSkillsAsset;
```

### 2. 后台初始化任务 (Asynchronous Background Task)
在 `src-tauri/src/main.rs` 的 Axum 异步运行时中派生非阻塞任务：
```rust
tokio::spawn(async {
    if let Err(e) = review::init_default_skills().await {
        eprintln!("[技能初始化] 后台初始化默认技能失败: {}", e);
    }
});
```

### 3. 版本校验与状态管理 (State Management via Manifest)
在本地磁盘用户的默认技能目录 `.token-insight/skills/default/` 中，维护一个标记文件 `.extracted_manifest.json`：
```json
{
  "version": "1.0.5"
}
```

**启动校验流程：**
1. 读取 `.extracted_manifest.json` 中的 `version` 字段。
2. 对比 `version` 与当前程序编译版本 `env!("CARGO_PKG_VERSION")`。
3. 若一致，说明当前版本已解压且未升级，直接跳过并结束。
4. 若不一致或文件不存在，说明是首次运行或发生了应用升级，执行解压流程。

### 4. 解压释放逻辑 (Extraction Logic)
1. 清空并重新创建 `skills/default` 物理目录，确保无陈旧冲突文件。
2. 遍历 `DefaultSkillsAsset` 中所有的文件，判断扩展名为 `.zip`。
3. 使用 `std::io::Cursor` 和 `zip::ZipArchive` 从嵌入的文件数据（字节流）中直接解包，并将文件释放到本地的默认技能目录。
4. 释放完成后，写回包含当前软件版本号的 `.extracted_manifest.json`。

## 异常处理与边缘情况
* **创建目录失败**：如果因系统权限问题无法在当前用户家目录下创建 `.token-insight`，记录 `error` 日志，避免进程崩溃。
* **ZIP 损坏**：如果解压出错，跳过当前文件并输出错误，不修改 `.extracted_manifest.json` 以便下次启动重试。
* **自定义技能安全**：默认技能只解压至 `skills/default` 目录，绝不触碰存放自定义技能的 `skills/user` 目录，防止用户资产丢失。
