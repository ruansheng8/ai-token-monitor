#!/usr/bin/env python3
# -*- coding: utf-8 -*-

import os
import sys
import re
import subprocess

# ANSI 颜色定义，提升终端交互视觉体验
GREEN = "\033[92m"
YELLOW = "\033[93m"
RED = "\033[91m"
BLUE = "\033[94m"
BOLD = "\033[1m"
RESET = "\033[0m"

def log_info(msg):
    print(f"{BLUE}[INFO]{RESET} {msg}")

def log_success(msg):
    print(f"{GREEN}[SUCCESS]{RESET} {BOLD}{msg}{RESET}")

def log_warn(msg):
    print(f"{YELLOW}[WARNING]{RESET} {msg}")

def log_error(msg):
    print(f"{RED}[ERROR]{RESET} {msg}")

def run_cmd(args, cwd=None, error_msg="命令执行失败"):
    """执行系统命令，并处理错误"""
    try:
        # 在 Windows 上，shell=True 可以更好地支持 npm/cargo 等脚本命令
        result = subprocess.run(args, cwd=cwd, shell=True, check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, encoding='utf-8')
        return result.stdout.strip()
    except subprocess.CalledProcessError as e:
        log_error(f"{error_msg}")
        if e.stdout:
            print(f"Stdout:\n{e.stdout}")
        if e.stderr:
            print(f"Stderr:\n{e.stderr}")
        sys.exit(1)

def get_current_version():
    """从 package.json 获取当前版本号"""
    package_path = os.path.join(os.path.dirname(__file__), "..", "package.json")
    if not os.path.exists(package_path):
        log_error("未找到 package.json 文件，请确保在项目根目录或 scripts 目录下运行此脚本")
        sys.exit(1)
        
    with open(package_path, "r", encoding="utf-8") as f:
        content = f.read()
        
    match = re.search(r'"version"\s*:\s*"([^"]+)"', content)
    if not match:
        log_error("在 package.json 中未找到 version 字段")
        sys.exit(1)
        
    return match.group(1)

def auto_increment_version(version):
    """自动将 Patch 版本号加 1 (例如 1.0.2 -> 1.0.3)"""
    parts = version.split('.')
    if len(parts) != 3:
        log_error(f"当前版本号格式非规范的语义化版本 (SemVer): {version}")
        sys.exit(1)
    
    try:
        major, minor, patch = map(int, parts)
        return f"{major}.{minor}.{patch + 1}"
    except ValueError:
        log_error(f"版本号中包含非数字字符，无法自动递增: {version}")
        sys.exit(1)

def update_file_content(file_path, pattern, replacement):
    """正则替换文件内容"""
    if not os.path.exists(file_path):
        log_error(f"未找到文件: {file_path}")
        sys.exit(1)
        
    with open(file_path, "r", encoding="utf-8") as f:
        content = f.read()
        
    new_content, count = re.subn(pattern, replacement, content, count=1)
    if count == 0:
        log_warn(f"在 {os.path.basename(file_path)} 中未找到匹配项，版本可能未变更")
        return
        
    with open(file_path, "w", encoding="utf-8", newline="\n") as f:
        f.write(new_content)

def clean_prefix(commit_msg):
    """剥离 commit message 的类型前缀，例如 'feat(api): add user' -> 'add user'"""
    return re.sub(r'^[a-zA-Z_-]+(?:\([^)]*\))?\s*:\s*', '', commit_msg).strip()

def tokenize(text):
    """分词：将非中文前缀剥离后，中文按单字，英文/数字按单词，全转为小写"""
    text = clean_prefix(text).lower()
    tokens = re.findall(r'[\u4e00-\u9fff]|[a-zA-Z0-9]+', text)
    return set(tokens)

def should_keep_commit(commit_msg):
    """过滤掉以 chore、test、docs 等前缀开头的提交"""
    pattern = r'^(chore|test|docs)\b'
    if re.match(pattern, commit_msg.lower().strip()):
        return False
    return True

def filter_similar_commits(commits):
    """如果超过20个，按重合度50%压缩，保留最新的"""
    if len(commits) <= 20:
        return commits
    
    filtered = []
    for c in commits:
        tokens = tokenize(c)
        is_duplicate = False
        for kept_msg, kept_tokens in filtered:
            if not tokens or not kept_tokens:
                continue
            intersection = tokens.intersection(kept_tokens)
            min_len = min(len(tokens), len(kept_tokens))
            if min_len > 0 and (len(intersection) / min_len) >= 0.5:
                is_duplicate = True
                break
        if not is_duplicate:
            filtered.append((c, tokens))
            
    return [msg for msg, _ in filtered]

def get_last_tag():
    """使用 git describe 获取最近的 tag。如果失败，则返回 None。"""
    try:
        # 在 Windows 上，shell=True 可以更好地支持命令
        result = subprocess.run(["git", "describe", "--tags", "--abbrev=0"], shell=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, encoding='utf-8')
        if result.returncode == 0:
            return result.stdout.strip()
    except Exception:
        pass
    return None

def get_commits_since_tag(last_tag, tag_name_to_be_created):
    """获取两个 tag 之间的 commit 信息"""
    # 如果最近的 tag 刚好等于我们马上要打的 tag（如因之前失败或重复运行，导致本地已有该 tag）
    # 那么需要使用 HEAD~1 的 describe 来向上找上一个 tag
    if last_tag == tag_name_to_be_created:
        try:
            result = subprocess.run(["git", "describe", "--tags", "--abbrev=0", "HEAD~1"], shell=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, encoding='utf-8')
            if result.returncode == 0:
                last_tag = result.stdout.strip()
            else:
                last_tag = None
        except Exception:
            last_tag = None

    if last_tag:
        cmd = ["git", "log", f"{last_tag}..HEAD", "--format=%s"]
    else:
        cmd = ["git", "log", "--format=%s"]
        
    try:
        result = subprocess.run(cmd, shell=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, encoding='utf-8')
        if result.returncode == 0:
            return [line.strip() for line in result.stdout.split('\n') if line.strip()]
    except Exception as e:
        log_warn(f"获取 commit 历史失败: {e}")
    return []

def get_release_description(tag_name):
    """获取过滤、去重并拼接后的 tag 描述"""
    last_tag = get_last_tag()
    commits = get_commits_since_tag(last_tag, tag_name)
    
    # 1. 过滤掉无用的提交
    filtered_commits = [c for c in commits if should_keep_commit(c)]
    
    # 2. 如果超过 20 个，则进行分词去重
    if len(filtered_commits) > 20:
        log_info(f"检测到有效提交数为 {len(filtered_commits)} 个，已超过 20 个，开始进行相似度压缩...")
        original_count = len(filtered_commits)
        filtered_commits = filter_similar_commits(filtered_commits)
        log_info(f"去重完成：已将 {original_count} 个提交压缩至 {len(filtered_commits)} 个。")
        
    if not filtered_commits:
        return f"Release {tag_name}"
        
    # 3. 汇总成多行文本描述
    desc_lines = [f"Release {tag_name}", ""]
    for c in filtered_commits:
        desc_lines.append(f"- {c}")
    return "\n".join(desc_lines)

def run_self_tests():
    """测试核心分词、清理、过滤和去重算法"""
    log_info("开始执行内置单元测试...")
    
    # 1. 测试 clean_prefix
    assert clean_prefix("feat(auth): add login button") == "add login button"
    assert clean_prefix("fix: fix error bug") == "fix error bug"
    assert clean_prefix("refactor(core/db): update schema") == "update schema"
    assert clean_prefix("no prefix msg") == "no prefix msg"
    log_success("clean_prefix 测试通过")
    
    # 2. 测试 tokenize
    assert tokenize("feat(auth): add login button") == {"add", "login", "button"}
    assert tokenize("修复ECharts图表") == {"修", "复", "echarts", "图", "表"}
    log_success("tokenize 测试通过")
    
    # 3. 测试 should_keep_commit
    assert should_keep_commit("feat: hello") is True
    assert should_keep_commit("chore: update README") is False
    assert should_keep_commit("test(api): add unit test") is False
    assert should_keep_commit("docs: change format") is False
    assert should_keep_commit("chore version bump") is False
    log_success("should_keep_commit 测试通过")
    
    # 4. 测试 filter_similar_commits
    commits = []
    # 前 5 个不重复的（最新的）
    commits.append("feat: add login feature")
    commits.append("feat: add logout button")
    commits.append("fix: page crash on dashboard")
    commits.append("style: change margin of card")
    commits.append("refactor: rename adapter")
    # 后续 16 个与前面有 50% 文本重复的（较旧的）
    commits.append("feat: login feature bug")       # 剥离后 "login feature bug" -> 与 "add login feature" 重合度 2/3 >= 50%
    commits.append("feat: add login page")          # 剥离后 "add login page" -> 与 "add login feature" 重合度 2/3 >= 50%
    commits.append("feat: logout button fix")       # 剥离后 "logout button fix" -> 与 "add logout button" 重合度 2/3 >= 50%
    commits.append("fix: dashboard page crash")     # 剥离后 "dashboard page crash" -> 与 "page crash on dashboard" 重合度 100% >= 50%
    for i in range(12):
        commits.append(f"feat: unique feature number {i}")
        
    assert len(commits) == 21
    filtered = filter_similar_commits(commits)
    
    assert "feat: add login feature" in filtered
    assert "feat: login feature bug" not in filtered
    assert "feat: add login page" not in filtered
    assert "feat: logout button fix" not in filtered
    assert "fix: dashboard page crash" not in filtered
    assert "feat: unique feature number 0" in filtered
    
    log_success("filter_similar_commits 测试通过")
    log_success("全部单元测试成功通过！")

def main():
    # 确保当前工作目录是项目根目录
    project_root = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
    os.chdir(project_root)
    
    # 检测是否为测试运行模式
    if len(sys.argv) > 1 and sys.argv[1] == '--test':
        run_self_tests()
        sys.exit(0)
    
    # 1. 获取当前版本
    current_version = get_current_version()
    log_info(f"当前项目版本号为: {BOLD}{current_version}{RESET}")
    
    # 2. 决定目标版本
    target_version = None
    is_auto = False
    
    if len(sys.argv) > 1:
        # 用户指定了版本号
        target_version = sys.argv[1].strip()
        # 移除可能误输入的 'v' 前缀
        if target_version.lower().startswith('v'):
            target_version = target_version[1:]
        # 简单验证 SemVer 格式
        if not re.match(r'^\d+\.\d+\.\d+$', target_version):
            log_error(f"指定的版本号格式不正确 (必须是 X.Y.Z 格式): {sys.argv[1]}")
            sys.exit(1)
    else:
        # 自动递增 patch 版本号
        target_version = auto_increment_version(current_version)
        is_auto = True
        
    # 3. 提示确认
    prompt_str = f"自动递增为 {GREEN}{target_version}{RESET}" if is_auto else f"指定为 {GREEN}{target_version}{RESET}"
    print(f"\n准备发布新版本: {BOLD}{current_version}{RESET} -> {BOLD}{target_version}{RESET} ({prompt_str})")
    
    try:
        confirm = input(f"{YELLOW}是否确认修改版本并推送发布？(y/N): {RESET}").strip().lower()
    except KeyboardInterrupt:
        print("\n已取消发布。")
        sys.exit(0)
        
    if confirm not in ['y', 'yes']:
        log_info("发布已取消。")
        sys.exit(0)
        
    # 4. 执行修改
    log_info("正在修改配置文件版本号...")
    
    # 修改 package.json
    package_json_path = os.path.join(project_root, "package.json")
    update_file_content(
        package_json_path,
        r'"version"\s*:\s*"[^"]+"',
        f'"version": "{target_version}"'
    )
    
    # 修改 src-tauri/Cargo.toml
    cargo_toml_path = os.path.join(project_root, "src-tauri", "Cargo.toml")
    update_file_content(
        cargo_toml_path,
        r'(version\s*=\s*)"[^"]+"',
        f'\\1"{target_version}"'
    )
    
    # 5. 同步锁文件
    log_info("正在同步 Rust 依赖锁文件 (Cargo.lock)...")
    run_cmd(["cargo", "check"], cwd=os.path.join(project_root, "src-tauri"), error_msg="Cargo check 锁文件同步失败")
    
    log_info("正在同步 Node 依赖锁文件 (package-lock.json)...")
    run_cmd(["npm", "install", "--package-lock-only"], cwd=project_root, error_msg="NPM lock 锁文件同步失败")
    
    # 6. 获取当前 Git 分支名称
    branch_name = run_cmd(["git", "rev-parse", "--abbrev-ref", "HEAD"], error_msg="获取 Git 分支失败")
    log_info(f"当前 Git 分支为: {BOLD}{branch_name}{RESET}")
    
    # 7. Git 提交并推送
    log_info("正在添加修改并提交到 Git...")
    run_cmd(["git", "add", "package.json", "package-lock.json", "src-tauri/Cargo.toml", "src-tauri/Cargo.lock"], error_msg="Git add 失败")
    run_cmd(["git", "commit", "-m", f"chore: bump version to {target_version}"], error_msg="Git commit 失败")
    
    log_info(f"正在将代码推送至远程仓库 {branch_name} 分支...")
    # 使用 rtk (或透明代理后的 git)
    run_cmd(["git", "push", "origin", branch_name], error_msg="Git push 代码失败")
    
    # 8. 打 Tag 并推送
    tag_name = f"v{target_version}"
    log_info(f"正在创建本地 Git Tag: {BOLD}{tag_name}{RESET}...")
    
    # 如果本地已经存在该 tag，先尝试删除（防止覆盖发布）
    try:
        subprocess.run(["git", "tag", "-d", tag_name], shell=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    except Exception:
        pass
        
    # 获取汇总去重的描述信息
    tag_description = get_release_description(tag_name)
    
    # 使用临时文件传递描述信息，防止 Windows 命令行转义/换行截断问题
    tag_file = os.path.join(project_root, ".git", "TAG_MSG")
    try:
        with open(tag_file, "w", encoding="utf-8", newline="\n") as f:
            f.write(tag_description)
        run_cmd(["git", "tag", "-a", tag_name, "-F", tag_file], error_msg="Git tag 创建失败")
    finally:
        if os.path.exists(tag_file):
            try:
                os.remove(tag_file)
            except Exception:
                pass
    
    log_info(f"正在推送 Tag {BOLD}{tag_name}{RESET} 到远程 GitHub...")
    run_cmd(["git", "push", "origin", tag_name], error_msg="推送 Git Tag 失败")
    
    print("-" * 50)
    log_success(f"版本发布流程执行完毕！")
    log_success(f"已推送版本号修改提交并创建了 Git Tag {tag_name}。")
    log_info("GitHub Actions 已被触发，请前往 GitHub 仓库的 Actions 页面查看云端打包发布进度。")

if __name__ == "__main__":
    main()
