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

def main():
    # 确保当前工作目录是项目根目录
    project_root = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
    os.chdir(project_root)
    
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
        subprocess.run(["git", "tag", "-d", tag_name], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    except Exception:
        pass
        
    run_cmd(["git", "tag", "-a", tag_name, "-m", f"Release {tag_name}"], error_msg="Git tag 创建失败")
    
    log_info(f"正在推送 Tag {BOLD}{tag_name}{RESET} 到远程 GitHub...")
    run_cmd(["git", "push", "origin", tag_name], error_msg="推送 Git Tag 失败")
    
    print("-" * 50)
    log_success(f"版本发布流程执行完毕！")
    log_success(f"已推送版本号修改提交并创建了 Git Tag {tag_name}。")
    log_info("GitHub Actions 已被触发，请前往 GitHub 仓库的 Actions 页面查看云端打包发布进度。")

if __name__ == "__main__":
    main()
