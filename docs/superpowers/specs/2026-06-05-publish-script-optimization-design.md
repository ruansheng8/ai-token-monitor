# 2026-06-05 Publish.py 脚本优化设计规范

优化 `scripts/publish.py` 脚本，使其在打 Git Tag 时能够自动汇总上一个版本 Tag 至今的所有 commit 信息作为描述，并进行过滤与智能相似度去重。

## 1. 需求背景与功能定义
目前 `publish.py` 在发布新版本并打 tag 时，使用硬编码的默认描述 `"Release v{version}"`。这导致 GitHub Release 上缺乏具体的变更日志。
为了自动化和细化发布日志，本方案需要：
1. **自动定位上一个 Tag**：通过 Git 命令获取前一个发布版本 Tag，如果没有，则抓取全部 commit。
2. **提取并过滤 Commit 历史**：获取上一个 Tag 到 HEAD 之间的所有提交信息，过滤掉包含 `chore`、`test`、`docs` 等前缀的辅助/工具类提交。
3. **相似性去重（>20个 Commit 时）**：若剩余提交过多（超过 20 个），采用分词对文本进行相似度比对。若两个 commit 的重合词数占较短 commit 词数的 50% 以上，则视为重复，仅保留时间上最新（即最先出现）的那个。
4. **安全打 Tag**：由于 Windows 命令行对换行符的转义可能导致命令执行失败，采用临时文件（`-F` 参数）方式将汇总的描述信息安全传给 `git tag`。

## 2. 详细技术方案

### 2.1 获取上一个 Tag
利用 `git describe --tags --abbrev=0` 获取最近的 tag。如果该命令返回的 tag 就是当前即将打的 `v{target_version}`（例如因异常重试导致本地已存在该 tag），则回退寻找 `HEAD~1` 的最近 tag：
```bash
git describe --tags --abbrev=0 HEAD~1
```
如果整个仓库没有任何 tag，则返回 `None`（此时获取所有提交）。

### 2.2 过滤无用 Commit 类型
定义正则模式 `^(chore|test|docs)\b`，忽略任何以此模式开头的 commit。例如：
- `chore: bump version` (过滤)
- `test: add unit tests` (过滤)
- `docs: update README` (过滤)
- `feat: support new API` (保留)

### 2.3 分词与相似度算法
为了在不依赖第三方包的前提下实现分词：
1. **前缀剥离**：在分词前，剥离常见的类型前缀，例如将 `feat(auth): add login button` 剥离为 `add login button`，只对比实际内容。
2. **文本分词**：中文字符按单字提取，英文和数字按单词提取，全部转换为小写，构成词集（Set）。
3. **相似度判定**：
   $$ \text{similarity} = \frac{|T_A \cap T_B|}{\min(|T_A|, |T_B|)} $$
   如果 $\text{similarity} \ge 0.5$，则判定为重复。
4. **保留最新**：`git log` 输出的 commits 是按时间倒序排列的（最新的在前）。我们依次处理 commits，较旧的 commit 若与已被保留的任意一个 commit 重复，则将其丢弃，从而只保留最新的那个。

### 2.4 安全 Tag 写入
将最终汇总的文本内容写入临时文件 `.git/TAG_MSG`，并通过 `-F` 执行：
```bash
git tag -a v1.0.7 -F .git/TAG_MSG
```
创建完成后，自动删除该临时文件，确保环境干净。

## 3. 验证计划
1. **本地单元测试**：编写一个测试用例，用包含各种情况的 Commit 列表（如包含 chore/test/docs 提交、大于 20 个包含 50% 以上重复文本的提交）验证过滤和去重逻辑的正确性。
2. **试运行发布脚本**：在本地修改后执行，确认在打 tag 环节能够成功生成预期的 commit 列表。
