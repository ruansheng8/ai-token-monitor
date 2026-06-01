const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

// 1. 获取当前分支的 Commit 数量
let commitCount = 65; // 默认回退基准
try {
  const gitCount = execSync('git rev-list --count HEAD', { stdio: ['pipe', 'pipe', 'ignore'] })
    .toString()
    .trim();
  commitCount = parseInt(gitCount, 10);
  if (isNaN(commitCount)) {
    commitCount = 65;
  }
} catch (e) {
  console.warn('⚠️ 获取 Git commit 数量失败，将使用默认基准 65', e.message);
}

// 2. 加载或初始化 version-config.json
const configPath = path.resolve(__dirname, '../version-config.json');
let versionConfig = {
  base_version: "0.1.0",
  base_commit_count: 65,
  last_calculated_version: "0.1.0"
};

if (fs.existsSync(configPath)) {
  try {
    versionConfig = JSON.parse(fs.readFileSync(configPath, 'utf8'));
  } catch (e) {
    console.error('❌ 解析 version-config.json 失败，使用默认配置', e.message);
  }
}

// 3. 读取 package.json
const packageJsonPath = path.resolve(__dirname, '../package.json');
let packageJson = {};
if (fs.existsSync(packageJsonPath)) {
  packageJson = JSON.parse(fs.readFileSync(packageJsonPath, 'utf8'));
} else {
  console.error('❌ 未找到 package.json 文件！');
  process.exit(1);
}

const currentPackageVersion = packageJson.version || "0.1.0";
const lastCalculatedVersion = versionConfig.last_calculated_version || versionConfig.base_version;

let targetVersion = '';

// 4. 判断是否手动修改了版本号
if (currentPackageVersion !== lastCalculatedVersion) {
  console.log(`✨ 检测到手动修改了版本号：${lastCalculatedVersion} -> ${currentPackageVersion}`);
  // 将手动修改的版本设为新基准
  versionConfig.base_version = currentPackageVersion;
  versionConfig.base_commit_count = commitCount;
  targetVersion = currentPackageVersion;
} else {
  // 没有手动修改，按 commit 数量的差值自动累加补丁版本 (z)
  const diff = Math.max(0, commitCount - versionConfig.base_commit_count);
  const parts = versionConfig.base_version.split('.');
  if (parts.length === 3) {
    const major = parseInt(parts[0], 10);
    const minor = parseInt(parts[1], 10);
    const patch = parseInt(parts[2], 10);
    
    const newPatch = patch + diff;
    targetVersion = `${major}.${minor}.${newPatch}`;
  } else {
    // 降级处理非标准 SemVer 格式
    targetVersion = versionConfig.base_version;
  }
}

// 5. 写入 version-config.json
versionConfig.last_calculated_version = targetVersion;
fs.writeFileSync(configPath, JSON.stringify(versionConfig, null, 2) + '\n', 'utf8');

// 6. 同步写入 package.json
packageJson.version = targetVersion;
fs.writeFileSync(packageJsonPath, JSON.stringify(packageJson, null, 2) + '\n', 'utf8');

// 7. 同步写入 src-tauri/tauri.conf.json
const tauriConfPath = path.resolve(__dirname, '../src-tauri/tauri.conf.json');
if (fs.existsSync(tauriConfPath)) {
  try {
    const tauriConf = JSON.parse(fs.readFileSync(tauriConfPath, 'utf8'));
    tauriConf.version = targetVersion;
    fs.writeFileSync(tauriConfPath, JSON.stringify(tauriConf, null, 2) + '\n', 'utf8');
    console.log(`✅ 成功同步 tauri.conf.json 版本为 ${targetVersion}`);
  } catch (e) {
    console.error('❌ 更新 tauri.conf.json 失败', e.message);
  }
}

// 8. 同步写入 src-tauri/Cargo.toml
const cargoTomlPath = path.resolve(__dirname, '../src-tauri/Cargo.toml');
if (fs.existsSync(cargoTomlPath)) {
  try {
    let cargoToml = fs.readFileSync(cargoTomlPath, 'utf8');
    // 精确替换 [package] 部分的 version = "x.y.z"
    cargoToml = cargoToml.replace(/^version\s*=\s*"[^"]*"/m, `version = "${targetVersion}"`);
    fs.writeFileSync(cargoTomlPath, cargoToml, 'utf8');
    console.log(`✅ 成功同步 Cargo.toml 版本为 ${targetVersion}`);
  } catch (e) {
    console.error('❌ 更新 Cargo.toml 失败', e.message);
  }
}

console.log(`🚀 版本同步流程完毕！当前最终版本号：${targetVersion}`);
