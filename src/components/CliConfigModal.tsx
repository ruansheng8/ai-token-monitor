/**
 * CliConfigModal.tsx — CLI 引擎配置弹窗
 *
 * 功能对齐 open-design 的 SettingsDialog "本机 CLI" 面板：
 *  - 左侧：支持 19 个 CLI 引擎的列表，展示检测状态与版本
 *  - 右侧：选中 CLI 的自定义可执行路径与环境变量配置表单
 *  - 底部：⚡ 运行连通测试 + 保存并应用 + 关闭
 *  - 连通测试超时 45 秒，与 open-design 保持一致
 */
import { useState, useEffect, useCallback, useRef } from 'react';
import {
  X,
  Terminal,
  RefreshCw,
  Settings,
  CheckCircle2,
  XCircle,
  Loader2,
  Plus,
  Trash2,
  ChevronRight,
  Zap,
  Save,
  Eye,
  EyeOff,
  FlaskConical,
} from 'lucide-react';
import { apiUrl, readJsonResponse } from '../lib/api';

// ============================================================
// 类型定义
// ============================================================

export interface CliToolInfo {
  name: string;
  available: boolean;
  version?: string;
  path?: string;
}

export interface DetectResult {
  tools: CliToolInfo[];
  recommended?: string;
}

// key = CLI bin 名 (如 "codex")；value = 环境变量字典 (key=VAR_NAME, value=value)
export type AgentCliEnv = Record<string, Record<string, string>>;

interface TestResult {
  ok: boolean;
  latency_ms: number;
  sample?: string;
  stdout: string;
  stderr: string;
  detail: string;
}

interface CliConfigModalProps {
  open: boolean;
  onClose: () => void;
  /** 当前已保存的 agent_cli_env 配置，从 /api/config 获取 */
  initialCliEnv?: AgentCliEnv;
  /** 保存配置回调，父组件负责调用 /api/config/save */
  onSave: (env: AgentCliEnv) => Promise<void>;
  /** 检测结果，用于展示 CLI 状态（可从父组件传入，也可在内部重新检测） */
  detectResult?: DetectResult | null;
  onRefreshDetect?: () => void;
  detectLoading?: boolean;
}

// ============================================================
// 支持的 19 个 CLI 引擎定义
// ============================================================

interface CliDefinition {
  bin: string;
  displayName: string;
  icon: string;
  description: string;
  installUrl: string;
  binEnvKey: string;       // 可执行路径覆盖的环境变量名，如 CODEX_BIN
  defaultEnvVars: string[]; // 该 CLI 常用的环境变量名（可选配置）
}

const CLI_DEFINITIONS: CliDefinition[] = [
  {
    bin: 'claude',
    displayName: 'Claude Code',
    icon: '🤖',
    description: 'Anthropic Claude Code CLI',
    installUrl: 'https://docs.anthropic.com/claude-code',
    binEnvKey: 'CLAUDE_BIN',
    defaultEnvVars: ['ANTHROPIC_API_KEY'],
  },
  {
    bin: 'codex',
    displayName: 'Codex CLI',
    icon: '⚡',
    description: 'OpenAI Codex CLI',
    installUrl: 'https://github.com/openai/codex',
    binEnvKey: 'CODEX_BIN',
    defaultEnvVars: ['OPENAI_API_KEY', 'OPENAI_BASE_URL'],
  },
  {
    bin: 'gemini',
    displayName: 'Gemini CLI',
    icon: '✨',
    description: 'Google Gemini CLI',
    installUrl: 'https://github.com/google-gemini/gemini-cli',
    binEnvKey: 'GEMINI_BIN',
    defaultEnvVars: ['GEMINI_API_KEY', 'GOOGLE_API_KEY'],
  },
  {
    bin: 'agy',
    displayName: 'Antigravity CLI',
    icon: '🚀',
    description: 'Antigravity AI CLI',
    installUrl: 'https://github.com/antigravity',
    binEnvKey: 'AGY_BIN',
    defaultEnvVars: [],
  },
  {
    bin: 'cursor-agent',
    displayName: 'Cursor Agent',
    icon: '🖱️',
    description: 'Cursor AI Agent CLI',
    installUrl: 'https://cursor.sh',
    binEnvKey: 'CURSOR_AGENT_BIN',
    defaultEnvVars: [],
  },
  {
    bin: 'opencode',
    displayName: 'OpenCode CLI',
    icon: '🔓',
    description: 'SST OpenCode CLI',
    installUrl: 'https://opencode.ai',
    binEnvKey: 'OPENCODE_BIN',
    defaultEnvVars: ['OPENAI_API_KEY', 'ANTHROPIC_API_KEY'],
  },
  {
    bin: 'qwen',
    displayName: 'Qwen CLI',
    icon: '🌙',
    description: 'Alibaba Qwen CLI',
    installUrl: 'https://github.com/QwenLM/qwen',
    binEnvKey: 'QWEN_BIN',
    defaultEnvVars: ['DASHSCOPE_API_KEY'],
  },
  {
    bin: 'copilot',
    displayName: 'GitHub Copilot',
    icon: '🐙',
    description: 'GitHub Copilot CLI',
    installUrl: 'https://docs.github.com/copilot',
    binEnvKey: 'COPILOT_BIN',
    defaultEnvVars: ['GITHUB_TOKEN'],
  },
  {
    bin: 'devin',
    displayName: 'Devin CLI',
    icon: '🤝',
    description: 'Cognition Devin CLI',
    installUrl: 'https://devin.ai',
    binEnvKey: 'DEVIN_BIN',
    defaultEnvVars: ['DEVIN_API_KEY'],
  },
  {
    bin: 'kimi',
    displayName: 'Kimi CLI',
    icon: '🌊',
    description: 'Moonshot Kimi CLI',
    installUrl: 'https://kimi.moonshot.cn',
    binEnvKey: 'KIMI_BIN',
    defaultEnvVars: ['MOONSHOT_API_KEY'],
  },
  {
    bin: 'qoder',
    displayName: 'Qoder CLI',
    icon: '🔷',
    description: 'Qoder AI Coding Assistant',
    installUrl: 'https://qoder.ai',
    binEnvKey: 'QODER_BIN',
    defaultEnvVars: [],
  },
  {
    bin: 'pi',
    displayName: 'Pi CLI',
    icon: '🎯',
    description: 'Inflection Pi CLI',
    installUrl: 'https://pi.ai',
    binEnvKey: 'PI_BIN',
    defaultEnvVars: ['PI_API_KEY'],
  },
  {
    bin: 'kiro',
    displayName: 'Kiro Agent',
    icon: '🔮',
    description: 'AWS Kiro Agent CLI',
    installUrl: 'https://kiro.dev',
    binEnvKey: 'KIRO_BIN',
    defaultEnvVars: [],
  },
  {
    bin: 'kilo',
    displayName: 'Kilo Code',
    icon: '🏔️',
    description: 'Kilo Code CLI',
    installUrl: 'https://kilocode.ai',
    binEnvKey: 'KILO_BIN',
    defaultEnvVars: [],
  },
  {
    bin: 'vibe',
    displayName: 'Vibe CLI',
    icon: '🎵',
    description: 'Vibe Coding Assistant',
    installUrl: 'https://vibe.ai',
    binEnvKey: 'VIBE_BIN',
    defaultEnvVars: [],
  },
  {
    bin: 'deepseek',
    displayName: 'DeepSeek CLI',
    icon: '🌊',
    description: 'DeepSeek Coding CLI',
    installUrl: 'https://deepseek.com',
    binEnvKey: 'DEEPSEEK_BIN',
    defaultEnvVars: ['DEEPSEEK_API_KEY'],
  },
  {
    bin: 'hermes',
    displayName: 'Hermes CLI',
    icon: '🪶',
    description: 'Nous Research Hermes CLI',
    installUrl: 'https://nousresearch.com',
    binEnvKey: 'HERMES_BIN',
    defaultEnvVars: [],
  },
  {
    bin: 'grok-build',
    displayName: 'Grok Build',
    icon: '🔴',
    description: 'xAI Grok Build CLI',
    installUrl: 'https://x.ai',
    binEnvKey: 'GROK_BUILD_BIN',
    defaultEnvVars: ['XAI_API_KEY'],
  },
  {
    bin: 'reasonix',
    displayName: 'Reasonix CLI',
    icon: '🧩',
    description: 'Reasonix AI CLI',
    installUrl: 'https://reasonix.ai',
    binEnvKey: 'REASONIX_BIN',
    defaultEnvVars: [],
  },
  {
    bin: 'aider',
    displayName: 'Aider',
    icon: '🛠️',
    description: 'Aider AI Pair Programmer',
    installUrl: 'https://aider.chat',
    binEnvKey: 'AIDER_BIN',
    defaultEnvVars: ['OPENAI_API_KEY', 'ANTHROPIC_API_KEY', 'GEMINI_API_KEY'],
  },
];

// ============================================================
// 组件实现
// ============================================================

export function CliConfigModal({
  open,
  onClose,
  initialCliEnv = {},
  onSave,
  detectResult,
  onRefreshDetect,
  detectLoading,
}: CliConfigModalProps) {
  // 当前选中的 CLI bin 名
  const [selectedBin, setSelectedBin] = useState<string>('claude');
  // 本地编辑中的 CLI 环境变量配置（深拷贝，不影响父组件直到 save）
  const [localEnv, setLocalEnv] = useState<AgentCliEnv>({});
  // 连通测试状态
  const [testState, setTestState] = useState<
    'idle' | 'running' | 'success' | 'error'
  >('idle');
  const [testResult, setTestResult] = useState<TestResult | null>(null);
  // 密码字段可见性
  const [hiddenFields, setHiddenFields] = useState<Set<string>>(new Set());
  // 保存中
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  // 用于自动 focus 输入框
  const firstInputRef = useRef<HTMLInputElement>(null);

  // 当弹窗打开时，同步 initialCliEnv 到本地副本
  useEffect(() => {
    if (open) {
      setLocalEnv(JSON.parse(JSON.stringify(initialCliEnv)));
      setTestState('idle');
      setTestResult(null);
      setSaveError(null);
    }
  }, [open, initialCliEnv]);

  // 切换 CLI 时重置测试状态
  useEffect(() => {
    setTestState('idle');
    setTestResult(null);
  }, [selectedBin]);

  // 获取当前 CLI 的 detect 信息
  const getDetectInfo = (bin: string): CliToolInfo | null => {
    return detectResult?.tools.find((t) => t.name === bin) ?? null;
  };

  // 获取当前 CLI 的本地 env dict
  const getCurrentEnv = (): Record<string, string> => {
    return localEnv[selectedBin] ?? {};
  };

  const setEnvValue = (key: string, value: string) => {
    setLocalEnv((prev) => ({
      ...prev,
      [selectedBin]: {
        ...(prev[selectedBin] ?? {}),
        [key]: value,
      },
    }));
  };

  const removeEnvKey = (key: string) => {
    setLocalEnv((prev) => {
      const existing = { ...(prev[selectedBin] ?? {}) };
      delete existing[key];
      return { ...prev, [selectedBin]: existing };
    });
  };

  const addEnvKey = (key: string) => {
    if (!key.trim()) return;
    setEnvValue(key.trim().toUpperCase(), '');
  };

  // ⚡ 运行连通测试
  const runTest = useCallback(async () => {
    setTestState('running');
    setTestResult(null);
    try {
      const resp = await fetch(apiUrl('/api/review/test-cli'), {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ bin: selectedBin }),
      });
      const result = await readJsonResponse<TestResult>(resp);
      setTestResult(result);
      setTestState(result.ok ? 'success' : 'error');
    } catch (e) {
      setTestResult({
        ok: false,
        latency_ms: 0,
        stdout: '',
        stderr: '',
        detail: `请求失败：${e instanceof Error ? e.message : String(e)}`,
      });
      setTestState('error');
    }
  }, [selectedBin]);

  // 💾 保存配置
  const handleSave = async () => {
    setSaving(true);
    setSaveError(null);
    try {
      await onSave(localEnv);
    } catch (e) {
      setSaveError(`保存失败：${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setSaving(false);
    }
  };

  const selectedDef = CLI_DEFINITIONS.find((d) => d.bin === selectedBin)!;
  const detectInfo = getDetectInfo(selectedBin);
  const currentEnv = getCurrentEnv();
  const binKey = selectedDef.binEnvKey;

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-[100] flex items-center justify-center"
      style={{ backgroundColor: 'rgba(0, 0, 0, 0.75)', backdropFilter: 'blur(8px)' }}
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        className="relative flex flex-col rounded-3xl shadow-2xl border border-white/10 overflow-hidden"
        style={{
          width: 'min(920px, 96vw)',
          height: 'min(680px, 90vh)',
          background: 'linear-gradient(160deg, rgba(18,18,30,0.98) 0%, rgba(14,14,24,0.99) 100%)',
        }}
      >
        {/* ── 顶部标题栏 ── */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-white/8 flex-shrink-0">
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded-xl bg-gradient-to-br from-violet-500/30 to-cyan-500/20 flex items-center justify-center border border-white/10">
              <Terminal className="w-4 h-4 text-violet-400" />
            </div>
            <div>
              <h2 className="text-sm font-bold text-white">配置 AI CLI 引擎</h2>
              <p className="text-xs text-white/40 mt-0.5">自定义各 CLI 的可执行路径与 API Key 环境变量</p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="w-8 h-8 rounded-xl border border-white/10 bg-white/5 hover:bg-white/10 flex items-center justify-center transition-colors cursor-pointer"
          >
            <X className="w-4 h-4 text-white/60" />
          </button>
        </div>

        {/* ── 主体内容：左侧 CLI 列表 + 右侧编辑面板 ── */}
        <div className="flex flex-1 min-h-0">
          {/* ── 左侧 CLI 列表 ── */}
          <div
            className="flex flex-col border-r border-white/8 flex-shrink-0 overflow-y-auto"
            style={{ width: '220px' }}
          >
            {/* 重新检测按钮 */}
            <div className="px-3 py-2.5 border-b border-white/5 flex-shrink-0">
              <button
                onClick={onRefreshDetect}
                disabled={detectLoading}
                className="w-full flex items-center gap-2 px-3 py-1.5 rounded-xl text-xs text-cyan-400 border border-cyan-500/20 bg-cyan-500/5 hover:bg-cyan-500/10 transition-colors cursor-pointer disabled:opacity-50"
              >
                <RefreshCw className={`w-3 h-3 ${detectLoading ? 'animate-spin' : ''}`} />
                重新检测所有 CLI
              </button>
            </div>

            {/* CLI 列表 */}
            <div className="flex flex-col py-1 overflow-y-auto">
              {CLI_DEFINITIONS.map((def) => {
                const info = getDetectInfo(def.bin);
                const hasCustomBin = !!(localEnv[def.bin]?.[def.binEnvKey]?.trim());
                const isSelected = selectedBin === def.bin;
                const isAvailable = info?.available ?? false;

                return (
                  <button
                    key={def.bin}
                    onClick={() => setSelectedBin(def.bin)}
                    className={`w-full flex items-center gap-2.5 px-3 py-2 text-left transition-all cursor-pointer ${
                      isSelected
                        ? 'bg-violet-500/15 border-r-2 border-violet-400'
                        : 'hover:bg-white/5 border-r-2 border-transparent'
                    }`}
                  >
                    <span className="text-sm flex-shrink-0">{def.icon}</span>
                    <div className="flex-1 min-w-0">
                      <div className={`text-xs font-semibold truncate ${isSelected ? 'text-violet-300' : 'text-white/80'}`}>
                        {def.displayName}
                      </div>
                      {info && (
                        <div className={`text-[10px] truncate mt-0.5 ${isAvailable ? 'text-green-400' : 'text-white/30'}`}>
                          {isAvailable ? info.version ?? '已检测' : '未安装'}
                        </div>
                      )}
                    </div>
                    <div className="flex items-center gap-1 flex-shrink-0">
                      {hasCustomBin && (
                        <div className="w-1.5 h-1.5 rounded-full bg-amber-400" title="已配置自定义路径" />
                      )}
                      <div
                        className={`w-1.5 h-1.5 rounded-full ${
                          isAvailable
                            ? 'bg-green-500 animate-pulse'
                            : (info ? 'bg-red-500/60' : 'bg-white/20')
                        }`}
                      />
                    </div>
                  </button>
                );
              })}
            </div>
          </div>

          {/* ── 右侧编辑面板 ── */}
          <div className="flex-1 flex flex-col min-h-0 overflow-y-auto">
            <div className="flex-1 px-6 py-5 space-y-5">
              {/* CLI 头部信息 */}
              <div className="flex items-center gap-3">
                <span className="text-3xl">{selectedDef.icon}</span>
                <div>
                  <h3 className="text-sm font-bold text-white">{selectedDef.displayName}</h3>
                  <p className="text-xs text-white/40 mt-0.5">{selectedDef.description}</p>
                </div>
                {detectInfo && (
                  <div className={`ml-auto flex items-center gap-1.5 px-2.5 py-1 rounded-xl text-xs font-semibold border ${
                    detectInfo.available
                      ? 'bg-green-500/10 border-green-500/25 text-green-400'
                      : 'bg-red-500/10 border-red-500/20 text-red-400/80'
                  }`}>
                    {detectInfo.available ? (
                      <>
                        <span className="w-1.5 h-1.5 rounded-full bg-green-500 animate-pulse" />
                        已检测到
                      </>
                    ) : (
                      <>
                        <span className="w-1.5 h-1.5 rounded-full bg-red-500/60" />
                        未检测到
                      </>
                    )}
                  </div>
                )}
              </div>

              {/* ─ 自定义可执行路径 ─ */}
              <div className="space-y-2">
                <div className="flex items-center gap-1.5">
                  <Settings className="w-3.5 h-3.5 text-violet-400" />
                  <span className="text-xs font-bold text-white/70 uppercase tracking-wider">自定义可执行路径</span>
                </div>
                <div className="relative">
                  <input
                    ref={firstInputRef}
                    type="text"
                    className="w-full px-3 py-2.5 rounded-xl text-xs font-mono bg-white/5 border border-white/10 text-white placeholder-white/25 focus:outline-none focus:border-violet-500/50 focus:bg-violet-500/5 transition-colors"
                    placeholder={`留空则使用系统 PATH 中的 ${selectedBin}，例如：/usr/local/bin/${selectedBin}`}
                    value={currentEnv[binKey] ?? ''}
                    onChange={(e) => setEnvValue(binKey, e.target.value)}
                  />
                  <div className="mt-1 text-[10px] text-white/30">
                    对应环境变量：<code className="text-violet-400/80">{binKey}</code>
                    {detectInfo?.path && (
                      <span className="ml-2">检测路径：<span className="text-cyan-400/60 font-mono">{detectInfo.path}</span></span>
                    )}
                  </div>
                </div>
              </div>

              {/* ─ 环境变量配置 ─ */}
              <div className="space-y-2">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-1.5">
                    <Zap className="w-3.5 h-3.5 text-amber-400" />
                    <span className="text-xs font-bold text-white/70 uppercase tracking-wider">环境变量</span>
                  </div>
                  <AddEnvVarButton
                    suggestedVars={selectedDef.defaultEnvVars.filter((v) => !(v in currentEnv))}
                    onAdd={addEnvKey}
                  />
                </div>

                {/* 已添加的环境变量列表（不含 binKey，已在上方单独展示） */}
                <div className="space-y-2">
                  {Object.entries(currentEnv)
                    .filter(([k]) => k !== binKey)
                    .map(([key, value]) => {
                      const isHidden = hiddenFields.has(key);
                      const isSensitive = key.toLowerCase().includes('key') ||
                        key.toLowerCase().includes('token') ||
                        key.toLowerCase().includes('secret') ||
                        key.toLowerCase().includes('password');
                      return (
                        <div key={key} className="flex items-center gap-2">
                          <div className="w-32 flex-shrink-0">
                            <input
                              type="text"
                              className="w-full px-2.5 py-2 rounded-lg text-[11px] font-mono bg-amber-500/5 border border-amber-500/20 text-amber-300 focus:outline-none focus:border-amber-500/40 transition-colors"
                              value={key}
                              readOnly
                            />
                          </div>
                          <div className="flex-1 relative">
                            <input
                              type={isSensitive && isHidden ? 'password' : 'text'}
                              className="w-full px-2.5 py-2 pr-8 rounded-lg text-[11px] font-mono bg-white/5 border border-white/10 text-white placeholder-white/20 focus:outline-none focus:border-violet-500/40 transition-colors"
                              placeholder={`请输入 ${key} 的值`}
                              value={value}
                              onChange={(e) => setEnvValue(key, e.target.value)}
                            />
                            {isSensitive && (
                              <button
                                onClick={() => setHiddenFields((prev) => {
                                  const next = new Set(prev);
                                  if (next.has(key)) next.delete(key);
                                  else next.add(key);
                                  return next;
                                })}
                                className="absolute right-2.5 top-1/2 -translate-y-1/2 text-white/30 hover:text-white/60 cursor-pointer"
                              >
                                {isHidden ? <EyeOff className="w-3 h-3" /> : <Eye className="w-3 h-3" />}
                              </button>
                            )}
                          </div>
                          <button
                            onClick={() => removeEnvKey(key)}
                            className="w-7 h-7 flex-shrink-0 rounded-lg border border-red-500/20 bg-red-500/5 hover:bg-red-500/15 text-red-400/60 hover:text-red-400 flex items-center justify-center transition-colors cursor-pointer"
                          >
                            <Trash2 className="w-3 h-3" />
                          </button>
                        </div>
                      );
                    })}
                  {Object.keys(currentEnv).filter((k) => k !== binKey).length === 0 && (
                    <p className="text-[11px] text-white/25 py-2 text-center">
                      暂无自定义环境变量，点击右上角「添加变量」
                    </p>
                  )}
                </div>
              </div>

              {/* ─ 连通测试结果 ─ */}
              {testResult && (
                <div className={`rounded-2xl p-4 border ${
                  testResult.ok
                    ? 'bg-green-500/8 border-green-500/20'
                    : 'bg-red-500/8 border-red-500/20'
                }`}>
                  <div className="flex items-start gap-2.5">
                    {testResult.ok ? (
                      <CheckCircle2 className="w-4 h-4 text-green-400 flex-shrink-0 mt-0.5" />
                    ) : (
                      <XCircle className="w-4 h-4 text-red-400 flex-shrink-0 mt-0.5" />
                    )}
                    <div className="flex-1 min-w-0">
                      <div className={`text-xs font-semibold ${testResult.ok ? 'text-green-400' : 'text-red-400'}`}>
                        {testResult.detail}
                      </div>
                      {testResult.latency_ms > 0 && (
                        <div className="text-[10px] text-white/30 mt-0.5">
                          响应耗时：{(testResult.latency_ms / 1000).toFixed(1)}s
                        </div>
                      )}
                      {testResult.sample && (
                        <div className="mt-2 p-2 rounded-lg bg-black/30 border border-white/5 font-mono text-[10px] text-green-400/80 break-all">
                          {testResult.sample}
                        </div>
                      )}
                      {!testResult.ok && testResult.stderr && (
                        <div className="mt-2 p-2 rounded-lg bg-black/30 border border-red-500/10 font-mono text-[10px] text-red-400/70 break-all max-h-24 overflow-y-auto">
                          {testResult.stderr.slice(0, 400)}
                        </div>
                      )}
                    </div>
                  </div>
                </div>
              )}
            </div>

            {/* ── 底部操作区 ── */}
            <div className="flex-shrink-0 flex items-center gap-3 px-6 py-4 border-t border-white/8">
              {/* 连通测试 */}
              <button
                onClick={runTest}
                disabled={testState === 'running'}
                className="flex items-center gap-2 px-4 py-2 rounded-xl text-xs font-semibold border border-violet-500/30 bg-violet-500/10 text-violet-300 hover:bg-violet-500/20 disabled:opacity-50 transition-colors cursor-pointer"
              >
                {testState === 'running' ? (
                  <Loader2 className="w-3.5 h-3.5 animate-spin" />
                ) : (
                  <FlaskConical className="w-3.5 h-3.5" />
                )}
                {testState === 'running' ? '测试中 (45s)...' : '⚡ 测试连通性'}
              </button>

              <div className="flex-1" />

              {saveError && (
                <span className="text-[11px] text-red-400">{saveError}</span>
              )}

              {/* 关闭 */}
              <button
                onClick={onClose}
                className="px-4 py-2 rounded-xl text-xs font-semibold border border-white/10 bg-white/5 text-white/60 hover:bg-white/8 transition-colors cursor-pointer"
              >
                关闭
              </button>

              {/* 保存并应用 */}
              <button
                onClick={handleSave}
                disabled={saving}
                className="flex items-center gap-2 px-4 py-2 rounded-xl text-xs font-semibold bg-gradient-to-r from-violet-600 to-cyan-600 text-white hover:from-violet-500 hover:to-cyan-500 disabled:opacity-50 transition-all cursor-pointer shadow-lg"
              >
                {saving ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : <Save className="w-3.5 h-3.5" />}
                保存并应用
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

// ============================================================
// 添加环境变量按钮（含快速建议）
// ============================================================

function AddEnvVarButton({
  suggestedVars,
  onAdd,
}: {
  suggestedVars: string[];
  onAdd: (key: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const [customKey, setCustomKey] = useState('');

  const handleAdd = (key: string) => {
    onAdd(key);
    setOpen(false);
    setCustomKey('');
  };

  return (
    <div className="relative">
      <button
        onClick={() => setOpen((p) => !p)}
        className="flex items-center gap-1 px-2.5 py-1 rounded-lg text-[10px] font-semibold border border-white/10 bg-white/5 text-white/50 hover:bg-white/10 hover:text-white/70 transition-colors cursor-pointer"
      >
        <Plus className="w-3 h-3" />
        添加变量
      </button>

      {open && (
        <div
          className="absolute right-0 top-full mt-1 z-20 rounded-2xl border border-white/10 shadow-xl overflow-hidden"
          style={{
            width: '220px',
            background: 'rgba(18,18,30,0.98)',
            backdropFilter: 'blur(16px)',
          }}
        >
          {/* 自定义输入 */}
          <div className="p-2.5 border-b border-white/8">
            <div className="flex gap-1.5">
              <input
                autoFocus
                type="text"
                placeholder="VAR_NAME"
                className="flex-1 px-2 py-1.5 rounded-lg text-[11px] font-mono bg-white/5 border border-white/10 text-white placeholder-white/25 focus:outline-none focus:border-violet-500/40"
                value={customKey}
                onChange={(e) => setCustomKey(e.target.value.toUpperCase())}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' && customKey.trim()) handleAdd(customKey.trim());
                  if (e.key === 'Escape') setOpen(false);
                }}
              />
              <button
                onClick={() => customKey.trim() && handleAdd(customKey.trim())}
                disabled={!customKey.trim()}
                className="px-2 py-1.5 rounded-lg bg-violet-500/20 text-violet-300 text-[10px] font-bold disabled:opacity-40 cursor-pointer hover:bg-violet-500/30"
              >
                添加
              </button>
            </div>
          </div>

          {/* 快速建议 */}
          {suggestedVars.length > 0 && (
            <div className="p-1.5">
              <div className="px-2 py-1 text-[9px] text-white/30 uppercase tracking-wider">快速添加</div>
              {suggestedVars.map((v) => (
                <button
                  key={v}
                  onClick={() => handleAdd(v)}
                  className="w-full flex items-center gap-2 px-2.5 py-1.5 rounded-xl text-[11px] font-mono text-white/70 hover:bg-white/8 hover:text-white transition-colors cursor-pointer text-left"
                >
                  <ChevronRight className="w-3 h-3 text-white/30 flex-shrink-0" />
                  {v}
                </button>
              ))}
            </div>
          )}

          {/* 关闭 */}
          <div className="px-2.5 pb-2">
            <button
              onClick={() => setOpen(false)}
              className="w-full text-center text-[10px] text-white/30 hover:text-white/50 py-1 cursor-pointer"
            >
              取消
            </button>
          </div>
        </div>
      )}

      {/* Backdrop */}
      {open && (
        <div className="fixed inset-0 z-10" onClick={() => setOpen(false)} />
      )}
    </div>
  );
}
