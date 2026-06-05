# Token Insight 默认技能运行时初始化实现计划

本项目旨在在程序运行时（后台线程）以非阻塞方式自动初始化和升级 `assets/skills-default` 中的内置技能。

## 待修改/新增文件

### 1. [MODIFY] [review.rs](file:///d:/VibeCoding/ai-token-monitor/src-tauri/src/review.rs)
- 引入 `rust-embed` 定义 `DefaultSkillsAsset`，将 `assets/skills-default/` 嵌入二进制。
- 添加并实现 `pub async fn init_default_skills() -> Result<(), String>` 函数。
  - 获取默认技能目录：`get_default_skills_dir()`。
  - 获取标记文件路径：`get_default_skills_dir().join(".extracted_manifest.json")`。
  - 对比版本号（`env!("CARGO_PKG_VERSION")`）。若一致，直接返回 `Ok(())`。
  - 若不一致，删除旧的 `skills/default`，重新创建它，并在内存中解压 `DefaultSkillsAsset` 下所有 `.zip` 文件到该目录下。
  - 成功后，写入最新的版本号至 `.extracted_manifest.json`。

### 2. [MODIFY] [main.rs](file:///d:/VibeCoding/ai-token-monitor/src-tauri/src/main.rs)
- 在 Axum 的 Tokio 异步运行环境启动后，使用 `tokio::spawn` 异步调用 `review::init_default_skills()`，保证不阻塞主线程。

## 验证计划

### 自动化构建验证
1. 执行 `cd src-tauri; cargo check` 验证代码编译无误。
2. 编写单元测试或集成测试，模拟启动过程中的版本校验逻辑。

### 手动功能验证
1. 启动应用开发服务器，观察控制台输出。
2. 检查用户家目录下是否成功生成 `.token-insight/skills/default/` 并解压了 `token-insight-report` 默认技能及生成了 `.extracted_manifest.json`。
3. 修改 `.extracted_manifest.json` 中的版本号，重新运行程序，检查是否成功自动覆盖重写。
