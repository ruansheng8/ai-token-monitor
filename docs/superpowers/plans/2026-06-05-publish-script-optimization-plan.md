# 2026-06-05 Publish.py 脚本优化实现计划

优化 `scripts/publish.py` 脚本，使其在打 Git Tag 时能够自动汇总上一个版本 Tag 至今的所有 commit 信息作为描述，并进行过滤与智能相似度去重。

## User Review Required

> [!NOTE]
> 1. **临时文件生成**：在打 tag 时，会在项目根目录下的 `.git/` 文件夹临时生成一个 `TAG_MSG` 临时文件，并在打完 tag 后立即被删除，不会污染 Git 状态。
> 2. **自动去重的策略**：去重仅在有效提交数（过滤掉 chore/test/docs 后）大于 20 个时触发，保留时间上最新（git log 列表最前）的那个，去除较旧的相似提交。
> 3. **内置单元测试**：为了方便验证，我们将通过在 `publish.py` 中添加 `--test` 命令行参数来提供核心逻辑的单元测试。运行 `python scripts/publish.py --test` 即可在本地进行算法正确性测试，不影响正常的构建发布流。

## Open Questions

无。前期的澄清问题已达成一致：
- 去除 `feat:`, `fix:` 等类型前缀后再进行相似度比对。
- 相似度定义为：重合词数占较短 commit 词数的比例 >= 50%。

## Proposed Changes

---

### 发布脚本组件 (Scripts Component)

#### [MODIFY] [publish.py](file:///d:/VibeCoding/ai-token-monitor/scripts/publish.py)
在 `scripts/publish.py` 中添加以下功能：
1.  **辅助函数**：
    -   `clean_prefix(commit_msg)`：移除 commit 信息的类型前缀。
    -   `tokenize(text)`：中文单字分词，英文/数字单词分词，转小写。
    -   `should_keep_commit(commit_msg)`：正则检查并过滤以 `chore`、`test`、`docs` 等开头的 commit 信息。
    -   `filter_similar_commits(commits)`：若 commits 大于 20 个，则按重合度 >= 50% 压缩，只保留最新那个。
    -   `get_last_tag()`：使用 `git describe --tags --abbrev=0` 获取上一个 tag，遇到异常（如无 tag）安全返回 `None`。
    -   `get_commits_since_tag(last_tag)`：执行 `git log <last_tag>..HEAD --format=%s` 获取 commit 列表。
    -   `get_release_description(tag_name)`：总装函数，汇编并过滤、去重，返回用于打 tag 的描述文本。
2.  **内置测试流**：
    -   `run_self_tests()`：针对分词、过滤、去重算法的单元测试函数。
3.  **主函数修改 (`main`)**：
    -   在 `main` 启动时，检查 `sys.argv` 中是否包含 `--test`。如果包含，则运行 `run_self_tests()` 并退出。
    -   在打 tag 的步骤，通过 `get_release_description(tag_name)` 生成 tag 描述。
    -   将描述写入 `.git/TAG_MSG`，使用 `git tag -a v{version} -F .git/TAG_MSG` 进行打 tag操作，完成后删除临时文件。

## Verification Plan

### Automated Tests
1. **运行内置单元测试**：
   在 Powershell 中执行以下命令，验证分词、过滤和相似度去重算法：
   ```powershell
   python scripts/publish.py --test
   ```

### Manual Verification
1. **测试空运行 (Dry-run) tag 描述提取**：
   通过临时在 `main` 中增加打印，在实际执行 `git tag` 前将生成的 `description` 打印到控制台，确认汇编后的描述格式和内容符合预期。
