/**
 * ReviewDrawer.tsx — 使用复盘与建议 侧滑抽屉组件
 *
 * 实现原理（参考 open-design 的 Claude Code 集成方案）：
 *  1. 调用 GET /api/review/detect 检测宿主机是否安装了 claude/codex CLI
 *  2. 用户点击「开始分析」后，通过 EventSource 连接 GET /api/review/stream（SSE）
 *  3. 后端在 Rust 中 spawn claude -p CLI，将输出逐行推流，前端实时渲染
 *  4. 支持 Markdown 渲染（标题、列表、代码块、表格、加粗等）
 *  5. 新增：用户可选时间范围（1天/7天/30天）、IDE 数据来源（多选）、自定义提示词
 */

import { useState, useEffect, useRef, useCallback } from 'react';
import {
  X,
  Sparkles,
  RefreshCw,
  Copy,
  Check,
  Terminal,
  AlertCircle,
  ChevronDown,
  Zap,
  BarChart2,
  Lightbulb,
  ExternalLink,
  Clock,
  Monitor,
  Edit3,
  ChevronUp,
} from 'lucide-react';
import { apiUrl, readJsonResponse } from '../lib/api';

// ============================================================
// 类型定义
// ============================================================

interface CliToolInfo {
  name: string;
  available: boolean;
  version: string | null;
  path: string | null;
}

interface DetectResponse {
  tools: CliToolInfo[];
  recommended: string | null;
}

interface ReviewMetrics {
  timeRange: string;
  totalTokens: number;
  totalCostUsd: number;
  totalSessions: number;
  cacheHitRate: number;
  thinkingRatio: number;
  sourceBreakdown?: string;
  modelDistribution?: string;
  dailyTrendSummary?: string;
  /** 数据库中所有可用的 IDE 来源列表 */
  availableSources?: string[];
}

interface ReviewDrawerProps {
  isOpen: boolean;
  onClose: () => void;
  metrics: ReviewMetrics | null;
}

// ============================================================
// 常量
// ============================================================

const TIME_RANGE_OPTIONS = [
  { value: '1天', label: '最近 1 天', days: 1 },
  { value: '7天', label: '最近 7 天', days: 7 },
  { value: '30天', label: '最近 30 天', days: 30 },
] as const;

/** IDE 来源 → 显示名映射 */
const SOURCE_DISPLAY: Record<string, string> = {
  all: '全部 IDE',
  antigravity: 'Antigravity',
  claude_code: 'Claude Code',
  codex: 'Codex CLI',
  cursor: 'Cursor',
  trae: 'Trae',
  trae_cn: 'Trae CN',
  gemini: 'Gemini CLI',
};

/** 默认提示词模板，{{IDE}} 会被替换为用户选中的 IDE */
const DEFAULT_PROMPT_TEMPLATE = `请帮我审查我最近7天在 {{IDE}} 中的实际使用情况，并输出一份周度复盘报告。

分析范围：
1. 检查这一周 {{IDE}} 产生的缓存文件、会话记录、技能目录和项目产物。
2. 统计这一周的总使用时长，并按主要项目估算投入时间和精力分布。
3. 按项目梳理我做过的事情、产出的文件、以及中间是否有重复劳动或低效步骤。
4. 审查几个主要项目中的用户提示词、对话推进过程和任务拆解方式，指出哪里提问不清楚、约束不完整、目标定义不准确。
5. 总结我这一周使用 {{IDE}} 时最常见的思维问题和协作问题，并给出可执行的优化建议。

输出要求：
1. 先给总览：总时长、主要项目、缓存和产物概况。
2. 再按项目分别分析：做了什么、花了多久、哪里效率低、提示词哪里可以改。
3. 最后给一个"我的 {{IDE}} 使用优化建议"，重点讲我的提问方式、任务拆解习惯、定义方式和思维逻辑如何提升。
4. 如果时间统计无法做到精确，请明确说明你的估算口径。`;

// ============================================================
// 工具函数
// ============================================================

/** 将 CLI 名称转换为友好的显示名 */
function getCliDisplayName(name: string): string {
  const map: Record<string, string> = {
    claude: 'Claude Code',
    codex: 'Codex CLI',
    gemini: 'Gemini CLI',
  };
  return map[name] || name;
}

/** 将来源标识转换为友好的显示名 */
function getSourceDisplayName(source: string): string {
  return SOURCE_DISPLAY[source] || source;
}

/** 根据选中 IDE 列表，替换提示词模板中的 {{IDE}} */
function buildPromptFromTemplate(template: string, selectedIdes: string[]): string {
  const ideLabel =
    selectedIdes.length === 0 || selectedIdes.includes('all')
      ? '所有 AI IDE'
      : selectedIdes.map(getSourceDisplayName).join('、');
  return template.replace(/\{\{IDE\}\}/g, ideLabel);
}

/** 将复盘的时间范围选项转换为具体的起止日期 */
function getReviewDateBounds(range: string) {
  const now = new Date();
  const formatDateStr = (d: Date) => {
    const y = d.getFullYear();
    const m = String(d.getMonth() + 1).padStart(2, '0');
    const day = String(d.getDate()).padStart(2, '0');
    return `${y}-${m}-${day}`;
  };

  if (range === '1天') {
    const dStr = formatDateStr(now);
    return { start: dStr, end: dStr };
  } else if (range === '30天') {
    const past = new Date(now.getTime() - 29 * 24 * 60 * 60 * 1000);
    return { start: formatDateStr(past), end: formatDateStr(now) };
  } else {
    // 默认 7天
    const past = new Date(now.getTime() - 6 * 24 * 60 * 60 * 1000);
    return { start: formatDateStr(past), end: formatDateStr(now) };
  }
}

/** 简单的 Markdown → HTML 转换（不引入外部库） */
function renderMarkdown(text: string): string {
  return text
    // 代码块
    .replace(/```[\w]*\n([\s\S]*?)```/g, '<pre class="md-code"><code>$1</code></pre>')
    // 行内代码
    .replace(/`([^`]+)`/g, '<code class="md-inline-code">$1</code>')
    // ### 标题
    .replace(/^### (.+)$/gm, '<h3 class="md-h3">$1</h3>')
    // ## 标题
    .replace(/^## (.+)$/gm, '<h2 class="md-h2">$1</h2>')
    // # 标题
    .replace(/^# (.+)$/gm, '<h1 class="md-h1">$1</h1>')
    // **加粗**
    .replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
    // *斜体*
    .replace(/\*(.+?)\*/g, '<em>$1</em>')
    // 无序列表
    .replace(/^[-*] (.+)$/gm, '<li class="md-li">$1</li>')
    // 有序列表
    .replace(/^\d+\. (.+)$/gm, '<li class="md-li md-oli">$1</li>')
    // 分割线
    .replace(/^---$/gm, '<hr class="md-hr"/>')
    // 换行（两个空行为段落分隔）
    .replace(/\n\n/g, '</p><p class="md-p">')
    // 单换行
    .replace(/\n/g, '<br/>');
}

// ============================================================
// 子组件：CLI 状态徽章
// ============================================================

function CliBadge({ tool }: { tool: CliToolInfo }) {
  return (
    <div
      className={`flex items-center gap-2 px-3 py-1.5 rounded-full text-xs font-medium border transition-all ${
        tool.available
          ? 'bg-green-500/10 border-green-500/30 text-green-600 dark:text-green-400'
          : 'bg-gray-500/10 border-gray-400/20 text-gray-400'
      }`}
    >
      <span
        className={`w-1.5 h-1.5 rounded-full ${tool.available ? 'bg-green-500 animate-pulse' : 'bg-gray-400'}`}
      />
      <span>{getCliDisplayName(tool.name)}</span>
      {tool.version && (
        <span className="opacity-60 font-mono">{tool.version.slice(0, 20)}</span>
      )}
    </div>
  );
}

// ============================================================
// 子组件：时间范围选择器（三个按钮）
// ============================================================

function TimeRangeSelector({
  value,
  onChange,
}: {
  value: string;
  onChange: (v: string) => void;
}) {
  return (
    <div className="flex gap-2">
      {TIME_RANGE_OPTIONS.map((opt) => (
        <button
          key={opt.value}
          onClick={() => onChange(opt.value)}
          className="flex-1 py-2 rounded-xl text-xs font-semibold transition-all"
          style={
            value === opt.value
              ? {
                  background: 'linear-gradient(135deg, rgba(8,145,178,0.2), rgba(124,58,237,0.2))',
                  border: '1px solid rgba(8,145,178,0.5)',
                  color: 'var(--neon-cyan)',
                  boxShadow: '0 0 10px rgba(8,145,178,0.15)',
                }
              : {
                  background: 'var(--card-bg)',
                  border: '1px solid var(--card-border)',
                  color: 'var(--text-muted)',
                }
          }
        >
          {opt.label}
        </button>
      ))}
    </div>
  );
}

// ============================================================
// 子组件：IDE 多选选择器
// ============================================================

function IdeSelector({
  availableSources,
  selectedIdes,
  onChange,
}: {
  availableSources: string[];
  selectedIdes: string[];
  onChange: (ides: string[]) => void;
}) {
  const [isOpen, setIsOpen] = useState(false);
  const isAll = selectedIdes.length === 0 || selectedIdes.includes('all');
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(event.target as Node)) {
        setIsOpen(false);
      }
    };

    if (isOpen) {
      document.addEventListener('mousedown', handleClickOutside);
    }
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, [isOpen]);

  const toggleAll = () => {
    onChange(['all']);
    setIsOpen(false);
  };

  const toggleIde = (ide: string) => {
    if (isAll) {
      // 从全部切换为只选这一个
      onChange([ide]);
      return;
    }
    if (selectedIdes.includes(ide)) {
      const next = selectedIdes.filter((s) => s !== ide);
      onChange(next.length === 0 ? ['all'] : next);
    } else {
      onChange([...selectedIdes, ide]);
    }
  };

  const displayLabel = isAll
    ? '全部 IDE'
    : selectedIdes.map(getSourceDisplayName).join('、');

  return (
    <div className="relative" ref={containerRef}>
      <button
        onClick={() => setIsOpen(!isOpen)}
        className="w-full flex items-center justify-between px-3 py-2.5 rounded-xl text-sm transition-all"
        style={{
          background: 'var(--card-bg)',
          border: '1px solid var(--card-border)',
          color: 'var(--text-primary)',
        }}
      >
        <div className="flex items-center gap-2 min-w-0">
          <Monitor className="w-3.5 h-3.5 flex-shrink-0" style={{ color: 'var(--neon-cyan)' }} />
          <span className="truncate text-xs">{displayLabel}</span>
        </div>
        <ChevronDown
          className={`w-3.5 h-3.5 flex-shrink-0 transition-transform ${isOpen ? 'rotate-180' : ''}`}
          style={{ color: 'var(--text-muted)' }}
        />
      </button>
      {isOpen && (
        <div
          className="absolute top-full mt-1 left-0 right-0 rounded-xl overflow-hidden z-20"
          style={{
            background: 'var(--bg-secondary)',
            border: '1px solid var(--card-border)',
            boxShadow: '0 12px 40px rgba(0,0,0,0.25)',
          }}
        >
          {/* 全部选项 */}
          <button
            onClick={toggleAll}
            className="w-full flex items-center gap-2 px-3 py-2.5 text-xs text-left transition-colors hover:bg-black/5 dark:hover:bg-white/5"
            style={{ color: isAll ? 'var(--neon-cyan)' : 'var(--text-primary)' }}
          >
            <span
              className={`w-4 h-4 rounded flex items-center justify-center border text-[10px] flex-shrink-0 ${
                isAll ? 'bg-cyan-500/20 border-cyan-500' : 'border-gray-400/40'
              }`}
            >
              {isAll && '✓'}
            </span>
            全部 IDE（默认）
          </button>
          {/* 分割线 */}
          <div style={{ height: '1px', background: 'var(--card-border)', margin: '2px 12px' }} />
          {/* 各 IDE 选项 */}
          {availableSources.map((src) => {
            const checked = !isAll && selectedIdes.includes(src);
            return (
              <button
                key={src}
                onClick={() => toggleIde(src)}
                className="w-full flex items-center gap-2 px-3 py-2.5 text-xs text-left transition-colors hover:bg-black/5 dark:hover:bg-white/5"
                style={{ color: checked ? 'var(--neon-cyan)' : 'var(--text-primary)' }}
              >
                <span
                  className={`w-4 h-4 rounded flex items-center justify-center border text-[10px] flex-shrink-0 ${
                    checked ? 'bg-cyan-500/20 border-cyan-500' : 'border-gray-400/40'
                  }`}
                >
                  {checked && '✓'}
                </span>
                {getSourceDisplayName(src)}
              </button>
            );
          })}
          {availableSources.length === 0 && (
            <p className="px-3 py-3 text-xs italic" style={{ color: 'var(--text-muted)' }}>
              暂无可用数据来源
            </p>
          )}
        </div>
      )}
    </div>
  );
}

// ============================================================
// 子组件：可编辑提示词面板
// ============================================================

function PromptEditor({
  value,
  onChange,
  isExpanded,
  onToggle,
}: {
  value: string;
  onChange: (v: string) => void;
  isExpanded: boolean;
  onToggle: () => void;
}) {
  return (
    <div
      className="rounded-xl overflow-hidden"
      style={{
        border: '1px solid var(--card-border)',
        background: 'var(--card-bg)',
      }}
    >
      {/* 折叠头部 */}
      <button
        onClick={onToggle}
        className="w-full flex items-center justify-between px-3 py-2.5 transition-all hover:bg-black/5 dark:hover:bg-white/5"
      >
        <div className="flex items-center gap-2">
          <Edit3 className="w-3.5 h-3.5" style={{ color: 'var(--neon-gold, #f59e0b)' }} />
          <span className="text-xs font-semibold" style={{ color: 'var(--text-secondary)' }}>
            分析提示词
          </span>
          <span
            className="text-[10px] px-1.5 py-0.5 rounded-full"
            style={{
              background: 'rgba(245,158,11,0.1)',
              color: 'var(--neon-gold, #f59e0b)',
              border: '1px solid rgba(245,158,11,0.2)',
            }}
          >
            可编辑
          </span>
        </div>
        {isExpanded ? (
          <ChevronUp className="w-3.5 h-3.5" style={{ color: 'var(--text-muted)' }} />
        ) : (
          <ChevronDown className="w-3.5 h-3.5" style={{ color: 'var(--text-muted)' }} />
        )}
      </button>
      {/* 文本框（可折叠） */}
      {isExpanded && (
        <div style={{ borderTop: '1px solid var(--card-border)' }}>
          <textarea
            value={value}
            onChange={(e) => onChange(e.target.value)}
            rows={12}
            spellCheck={false}
            className="w-full px-3 py-3 text-xs font-mono resize-y outline-none"
            style={{
              background: 'transparent',
              color: 'var(--text-primary)',
              lineHeight: '1.7',
              minHeight: '180px',
              maxHeight: '400px',
            }}
          />
        </div>
      )}
    </div>
  );
}

// ============================================================
// 主组件
// ============================================================

export function ReviewDrawer({ isOpen, onClose, metrics }: ReviewDrawerProps) {
  const [detectResult, setDetectResult] = useState<DetectResponse | null>(null);
  const [detectLoading, setDetectLoading] = useState(false);
  const [selectedCli, setSelectedCli] = useState<string>('claude');
  const [isCliSelectorOpen, setIsCliSelectorOpen] = useState(false);

  // 时间范围（独立于主页面的 timeRange 状态）
  const [reviewTimeRange, setReviewTimeRange] = useState<string>('7天');

  // IDE 多选
  const [selectedIdes, setSelectedIdes] = useState<string[]>(['all']);

  // 动态拉取的指标数据
  const [activeMetrics, setActiveMetrics] = useState<ReviewMetrics | null>(null);
  const [metricsLoading, setMetricsLoading] = useState(false);

  // 提示词
  const [customPrompt, setCustomPrompt] = useState<string>('');
  const [isPromptExpanded, setIsPromptExpanded] = useState(false);

  const [isAnalyzing, setIsAnalyzing] = useState(false);
  const [outputText, setOutputText] = useState('');
  const [isDone, setIsDone] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  const outputRef = useRef<HTMLDivElement>(null);
  const esRef = useRef<EventSource | null>(null);
  const outputTextRef = useRef('');

  // 当 IDE 选择或时间范围变化时，自动更新提示词（仅在未被手动修改时）
  const lastAutoPromptRef = useRef('');
  useEffect(() => {
    // 替换模板中的 {{IDE}} 和天数
    const templateWithTime = DEFAULT_PROMPT_TEMPLATE.replace('最近7天', `最近${reviewTimeRange}`);
    const newPrompt = buildPromptFromTemplate(templateWithTime, selectedIdes);

    // 如果当前提示词是上次自动生成的，或者是空的，则自动更新
    if (customPrompt === '' || customPrompt === lastAutoPromptRef.current) {
      setCustomPrompt(newPrompt);
      lastAutoPromptRef.current = newPrompt;
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedIdes, reviewTimeRange]);

  // 抽屉打开时检测 CLI 工具
  useEffect(() => {
    if (isOpen && !detectResult) {
      detectCliTools();
    }
  }, [isOpen]);

  // 抽屉打开时初始化提示词
  useEffect(() => {
    if (isOpen && customPrompt === '') {
      const templateWithTime = DEFAULT_PROMPT_TEMPLATE.replace('最近7天', `最近${reviewTimeRange}`);
      const newPrompt = buildPromptFromTemplate(templateWithTime, selectedIdes);
      setCustomPrompt(newPrompt);
      lastAutoPromptRef.current = newPrompt;
    }
  }, [isOpen]);

  // 当 IDE 选择、时间范围变化或抽屉打开时，动态请求指标数据并智能聚合
  useEffect(() => {
    if (!isOpen) return;

    let isMounted = true;

    async function updateMetrics() {
      setMetricsLoading(true);
      try {
        const bounds = getReviewDateBounds(reviewTimeRange);

        // 确定需要请求的 sources
        const sourcesToFetch = selectedIdes.includes('all') || selectedIdes.length === 0
          ? ['all']
          : selectedIdes;

        // 并行请求所有 source 的 metrics
        const promises = sourcesToFetch.map(async (src) => {
          const res = await fetch(
            apiUrl(`/metrics?source=${src}&start_date=${bounds.start}&end_date=${bounds.end}&t=${Date.now()}`)
          );
          if (!res.ok) throw new Error(`HTTP error! status: ${res.status}`);
          return readJsonResponse(res) as Promise<any>;
        });

        const results = await Promise.all(promises);

        if (!isMounted) return;

        // 如果只有一个结果 (比如 'all' 或单个 IDE)，直接使用
        if (results.length === 1) {
          const data = results[0];
          setActiveMetrics({
            timeRange: reviewTimeRange,
            totalTokens: data.totals.total_tokens,
            totalCostUsd: data.totals.total_cost,
            totalSessions: data.totals.total_sessions,
            cacheHitRate: data.totals.cache_hit_rate,
            thinkingRatio: data.totals.thinking_ratio,
            sourceBreakdown: data.source_trends?.length > 0
              ? JSON.stringify(
                  Object.entries(
                    data.source_trends.reduce((acc: Record<string, number>, item: any) => {
                      acc[item.source] = (acc[item.source] || 0) + item.tokens;
                      return acc;
                    }, {})
                  ).map(([source, tokens]) => ({ source, tokens })).slice(0, 8)
                )
              : undefined,
            modelDistribution: data.model_distribution?.length > 0
              ? JSON.stringify(
                  data.model_distribution.slice(0, 6).map((m: any) => ({
                    model: m.model,
                    total_tokens: m.total_tokens,
                  }))
                )
              : undefined,
            dailyTrendSummary: data.daily_trends?.length > 0
              ? JSON.stringify(
                  data.daily_trends.slice(-7).map((d: any) => ({
                    date: d.date,
                    tokens: d.input + d.output + d.cached + d.thinking,
                    sessions: d.sessions,
                  }))
                )
              : undefined,
          });
        } else {
          // 多个结果，手动合并
          let totalInput = 0;
          let totalOutput = 0;
          let totalCached = 0;
          let totalThinking = 0;
          let totalSessions = 0;
          let totalCost = 0;

          const combinedSourceBreakdownMap: Record<string, number> = {};
          const combinedModelDistMap: Record<string, number> = {};
          const combinedDailyTrendMap: Record<string, { tokens: number; sessions: number }> = {};

          results.forEach((data) => {
            const t = data.totals;
            totalInput += t.total_input;
            totalOutput += t.total_output;
            totalCached += t.total_cached;
            totalThinking += t.total_thinking;
            totalSessions += t.total_sessions;
            totalCost += t.total_cost;

            // 合并 source_trends
            if (data.source_trends) {
              data.source_trends.forEach((item: any) => {
                combinedSourceBreakdownMap[item.source] = (combinedSourceBreakdownMap[item.source] || 0) + item.tokens;
              });
            }

            // 合并 model_distribution
            if (data.model_distribution) {
              data.model_distribution.forEach((m: any) => {
                combinedModelDistMap[m.model] = (combinedModelDistMap[m.model] || 0) + m.total_tokens;
              });
            }

            // 合并 daily_trends
            if (data.daily_trends) {
              data.daily_trends.forEach((d: any) => {
                if (!combinedDailyTrendMap[d.date]) {
                  combinedDailyTrendMap[d.date] = { tokens: 0, sessions: 0 };
                }
                combinedDailyTrendMap[d.date].tokens += d.input + d.output + d.cached + d.thinking;
                combinedDailyTrendMap[d.date].sessions += d.sessions;
              });
            }
          });

          const cacheHitRate = totalInput > 0 ? totalCached / totalInput : 0;
          const thinkingRatio = totalOutput > 0 ? totalThinking / totalOutput : 0;

          const sourceBreakdown = Object.entries(combinedSourceBreakdownMap)
            .map(([source, tokens]) => ({ source, tokens }))
            .slice(0, 8);

          const modelDistribution = Object.entries(combinedModelDistMap)
            .map(([model, total_tokens]) => ({ model, total_tokens }))
            .sort((a, b) => b.total_tokens - a.total_tokens)
            .slice(0, 6);

          const dailyTrendSummary = Object.entries(combinedDailyTrendMap)
            .map(([date, val]) => ({ date, tokens: val.tokens, sessions: val.sessions }))
            .sort((a, b) => a.date.localeCompare(b.date))
            .slice(-7);

          setActiveMetrics({
            timeRange: reviewTimeRange,
            totalTokens: totalInput + totalOutput,
            totalCostUsd: totalCost,
            totalSessions,
            cacheHitRate,
            thinkingRatio,
            sourceBreakdown: sourceBreakdown.length > 0 ? JSON.stringify(sourceBreakdown) : undefined,
            modelDistribution: modelDistribution.length > 0 ? JSON.stringify(modelDistribution) : undefined,
            dailyTrendSummary: dailyTrendSummary.length > 0 ? JSON.stringify(dailyTrendSummary) : undefined,
          });
        }
      } catch (e) {
        console.error('动态拉取复盘指标失败', e);
      } finally {
        if (isMounted) {
          setMetricsLoading(false);
        }
      }
    }

    updateMetrics();

    return () => {
      isMounted = false;
    };
  }, [isOpen, reviewTimeRange, selectedIdes]);

  // 自动滚动到最新输出
  useEffect(() => {
    if (outputRef.current && isAnalyzing) {
      outputRef.current.scrollTop = outputRef.current.scrollHeight;
    }
  }, [outputText, isAnalyzing]);

  // 关闭时停止分析
  useEffect(() => {
    if (!isOpen) {
      stopAnalysis();
    }
  }, [isOpen]);

  const detectCliTools = async () => {
    setDetectLoading(true);
    try {
      const res = await fetch('/api/review/detect');
      if (res.ok) {
        const data: DetectResponse = await res.json();
        setDetectResult(data);
        if (data.recommended) {
          setSelectedCli(data.recommended);
        }
      }
    } catch (e) {
      console.error('CLI 检测失败', e);
    } finally {
      setDetectLoading(false);
    }
  };

  const stopAnalysis = () => {
    if (esRef.current) {
      esRef.current.close();
      esRef.current = null;
    }
    setIsAnalyzing(false);
  };

  const startAnalysis = useCallback(() => {
    if (isAnalyzing) return;

    setOutputText('');
    outputTextRef.current = '';
    setIsDone(false);
    setError(null);
    setIsAnalyzing(true);

    // 构建 query 参数（将指标数据传给后端）
    const metricsToUse = activeMetrics || metrics;

    const params = new URLSearchParams({
      cli: selectedCli,
      time_range: reviewTimeRange,
      total_tokens: String(metricsToUse?.totalTokens ?? 0),
      total_cost_usd: String(metricsToUse?.totalCostUsd ?? 0),
      total_sessions: String(metricsToUse?.totalSessions ?? 0),
      cache_hit_rate: String(metricsToUse?.cacheHitRate ?? 0),
      thinking_ratio: String(metricsToUse?.thinkingRatio ?? 0),
    });

    if (metricsToUse?.sourceBreakdown) {
      params.set('source_breakdown', metricsToUse.sourceBreakdown);
    }
    if (metricsToUse?.modelDistribution) {
      params.set('model_distribution', metricsToUse.modelDistribution);
    }
    if (metricsToUse?.dailyTrendSummary) {
      params.set('daily_trend_summary', metricsToUse.dailyTrendSummary);
    }

    // 传入自定义提示词（如果有内容）
    if (customPrompt.trim()) {
      params.set('custom_prompt', customPrompt.trim());
    }

    // 传入用户选择的 IDE 列表
    if (!selectedIdes.includes('all') && selectedIdes.length > 0) {
      params.set('selected_ides', selectedIdes.join(','));
    }

    // 创建 SSE 连接（参考 open-design 的 SSE 推流模式）
    const es = new EventSource(`/api/review/stream?${params.toString()}`);
    esRef.current = es;

    es.onmessage = (event) => {
      if (event.data === '[DONE]') {
        setIsDone(true);
        setIsAnalyzing(false);
        es.close();
        esRef.current = null;
        return;
      }
      // 逐块追加输出
      outputTextRef.current += event.data;
      setOutputText(outputTextRef.current);
    };

    es.addEventListener('done', () => {
      setIsDone(true);
      setIsAnalyzing(false);
      es.close();
      esRef.current = null;
    });

    es.onerror = () => {
      if (es.readyState === EventSource.CLOSED) return;
      setError('SSE 连接断开，分析可能已完成或发生错误');
      setIsAnalyzing(false);
      es.close();
      esRef.current = null;
    };
  }, [isAnalyzing, selectedCli, activeMetrics, metrics, reviewTimeRange, selectedIdes, customPrompt]);

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(outputText);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      /* ignore */
    }
  };

  const availableTools = detectResult?.tools.filter((t) => t.available) ?? [];
  const hasAnyCli = availableTools.length > 0;

  // 可用的 IDE 来源列表（从 metrics.availableSources 获取）
  const availableSources = metrics?.availableSources ?? [];

  // ============================================================
  // 渲染
  // ============================================================

  return (
    <>
      {/* 背景遮罩 */}
      <div
        className={`fixed inset-0 z-40 transition-opacity duration-300 ${
          isOpen ? 'opacity-100 pointer-events-auto' : 'opacity-0 pointer-events-none'
        }`}
        style={{ background: 'rgba(3, 7, 18, 0.5)', backdropFilter: 'blur(4px)' }}
        onClick={onClose}
      />

      {/* 抽屉主体 */}
      <div
        className={`fixed top-0 right-0 h-full z-50 flex flex-col transition-transform duration-300 ease-[cubic-bezier(0.25,0.8,0.25,1)] ${
          isOpen ? 'translate-x-0' : 'translate-x-full'
        }`}
        style={{
          width: 'min(720px, 96vw)',
          background: 'var(--bg-secondary)',
          borderLeft: '1px solid var(--card-border)',
          boxShadow: '-20px 0 60px rgba(0,0,0,0.25)',
        }}
      >
        {/* ── 头部 ── */}
        <div
          className="flex items-center justify-between px-6 py-4 flex-shrink-0"
          style={{
            borderBottom: '1px solid var(--card-border)',
            background:
              'linear-gradient(135deg, rgba(6,182,212,0.08) 0%, transparent 60%)',
          }}
        >
          <div className="flex items-center gap-3">
            <div
              className="w-9 h-9 rounded-xl flex items-center justify-center"
              style={{
                background: 'linear-gradient(135deg, #0891b2, #7c3aed)',
                boxShadow: '0 4px 16px rgba(8,145,178,0.4)',
              }}
            >
              <Sparkles className="w-4 h-4 text-white" />
            </div>
            <div>
              <h2 className="text-base font-semibold" style={{ color: 'var(--text-primary)' }}>
                使用复盘与建议
              </h2>
              <p className="text-xs" style={{ color: 'var(--text-muted)' }}>
                由本机 AI CLI 驱动的智能分析
              </p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="w-8 h-8 rounded-lg flex items-center justify-center transition-all hover:bg-black/10 dark:hover:bg-white/10"
          >
            <X className="w-4 h-4" style={{ color: 'var(--text-muted)' }} />
          </button>
        </div>

        {/* ── 内容区 ── */}
        <div className="flex-1 overflow-y-auto">

          {/* ── 分析参数配置区 ── */}
          <div className="px-6 pt-5 pb-1">
            <p
              className="text-xs font-semibold uppercase tracking-wider mb-3"
              style={{ color: 'var(--text-muted)' }}
            >
              分析参数
            </p>

            {/* 时间范围选择 */}
            <div className="mb-3">
              <div className="flex items-center gap-2 mb-2">
                <Clock className="w-3.5 h-3.5" style={{ color: 'var(--neon-cyan)' }} />
                <span className="text-xs" style={{ color: 'var(--text-secondary)' }}>
                  时间范围
                </span>
              </div>
              <TimeRangeSelector value={reviewTimeRange} onChange={setReviewTimeRange} />
            </div>

            {/* IDE 来源选择 */}
            <div className="mb-3">
              <div className="flex items-center gap-2 mb-2">
                <Monitor className="w-3.5 h-3.5" style={{ color: 'var(--neon-cyan)' }} />
                <span className="text-xs" style={{ color: 'var(--text-secondary)' }}>
                  数据来源 IDE
                </span>
              </div>
              <IdeSelector
                availableSources={availableSources}
                selectedIdes={selectedIdes}
                onChange={setSelectedIdes}
              />
            </div>

            {/* 可编辑提示词 */}
            <div className="mb-4">
              <PromptEditor
                value={customPrompt}
                onChange={setCustomPrompt}
                isExpanded={isPromptExpanded}
                onToggle={() => setIsPromptExpanded(!isPromptExpanded)}
              />
            </div>
          </div>

          {/* 工具检测区 */}
          <div className="px-6 pb-4">
            <div className="flex items-center justify-between mb-3">
              <span
                className="text-xs font-semibold uppercase tracking-wider"
                style={{ color: 'var(--text-muted)' }}
              >
                可用 CLI 工具
              </span>
              <button
                onClick={detectCliTools}
                disabled={detectLoading}
                className="flex items-center gap-1.5 text-xs px-2.5 py-1 rounded-lg transition-all"
                style={{
                  color: 'var(--neon-cyan)',
                  border: '1px solid rgba(6,182,212,0.25)',
                  background: 'rgba(6,182,212,0.06)',
                }}
              >
                <RefreshCw
                  className={`w-3 h-3 ${detectLoading ? 'animate-spin' : ''}`}
                />
                重新检测
              </button>
            </div>

            {detectLoading ? (
              <div className="flex items-center gap-2 py-3">
                <div
                  className="w-4 h-4 rounded-full border-2 animate-spin"
                  style={{ borderColor: 'var(--neon-cyan)', borderTopColor: 'transparent' }}
                />
                <span className="text-xs" style={{ color: 'var(--text-muted)' }}>
                  正在检测宿主机已安装的 AI CLI…
                </span>
              </div>
            ) : detectResult ? (
              <div className="flex flex-wrap gap-2">
                {detectResult.tools.map((tool) => (
                  <CliBadge key={tool.name} tool={tool} />
                ))}
              </div>
            ) : (
              <div className="text-xs" style={{ color: 'var(--text-muted)' }}>
                点击「重新检测」扫描已安装工具
              </div>
            )}

            {/* 未安装引导 */}
            {detectResult && !hasAnyCli && (
              <div
                className="mt-4 p-4 rounded-xl flex items-start gap-3"
                style={{
                  background: 'rgba(249,115,22,0.08)',
                  border: '1px solid rgba(249,115,22,0.2)',
                }}
              >
                <AlertCircle className="w-4 h-4 mt-0.5 flex-shrink-0 text-orange-500" />
                <div>
                  <p className="text-sm font-medium text-orange-600 dark:text-orange-400 mb-1">
                    未检测到任何 AI CLI 工具
                  </p>
                  <p className="text-xs mb-2" style={{ color: 'var(--text-secondary)' }}>
                    本功能需要宿主机安装 Claude Code CLI 或 Codex CLI。
                  </p>
                  <a
                    href="https://docs.anthropic.com/claude-code"
                    target="_blank"
                    rel="noopener noreferrer"
                    className="inline-flex items-center gap-1.5 text-xs font-medium text-orange-500 hover:text-orange-400 transition-colors"
                  >
                    <ExternalLink className="w-3 h-3" />
                    安装 Claude Code
                  </a>
                  <span className="text-xs mx-2" style={{ color: 'var(--text-muted)' }}>
                    或
                  </span>
                  <a
                    href="https://github.com/openai/codex"
                    target="_blank"
                    rel="noopener noreferrer"
                    className="inline-flex items-center gap-1.5 text-xs font-medium text-orange-500 hover:text-orange-400 transition-colors"
                  >
                    <ExternalLink className="w-3 h-3" />
                    安装 Codex CLI
                  </a>
                </div>
              </div>
            )}

            {/* CLI 选择器（有多个可用工具时显示） */}
            {hasAnyCli && availableTools.length > 1 && (
              <div className="mt-4">
                <span
                  className="text-xs font-semibold uppercase tracking-wider block mb-2"
                  style={{ color: 'var(--text-muted)' }}
                >
                  分析引擎
                </span>
                <div className="relative">
                  <button
                    onClick={() => setIsCliSelectorOpen(!isCliSelectorOpen)}
                    className="w-full flex items-center justify-between px-3 py-2.5 rounded-xl text-sm transition-all"
                    style={{
                      background: 'var(--card-bg)',
                      border: '1px solid var(--card-border)',
                      color: 'var(--text-primary)',
                    }}
                  >
                    <div className="flex items-center gap-2">
                      <Terminal className="w-3.5 h-3.5" style={{ color: 'var(--neon-cyan)' }} />
                      {getCliDisplayName(selectedCli)}
                    </div>
                    <ChevronDown
                      className={`w-3.5 h-3.5 transition-transform ${isCliSelectorOpen ? 'rotate-180' : ''}`}
                      style={{ color: 'var(--text-muted)' }}
                    />
                  </button>
                  {isCliSelectorOpen && (
                    <div
                      className="absolute top-full mt-1 left-0 right-0 rounded-xl overflow-hidden z-10"
                      style={{
                        background: 'var(--bg-secondary)',
                        border: '1px solid var(--card-border)',
                        boxShadow: '0 12px 40px rgba(0,0,0,0.2)',
                      }}
                    >
                      {availableTools.map((tool) => (
                        <button
                          key={tool.name}
                          onClick={() => {
                            setSelectedCli(tool.name);
                            setIsCliSelectorOpen(false);
                          }}
                          className="w-full flex items-center gap-2 px-3 py-2.5 text-sm text-left transition-colors hover:bg-black/5 dark:hover:bg-white/5"
                          style={{ color: 'var(--text-primary)' }}
                        >
                          <Terminal className="w-3.5 h-3.5" style={{ color: 'var(--neon-cyan)' }} />
                          {getCliDisplayName(tool.name)}
                          {tool.version && (
                            <span className="text-xs font-mono ml-auto" style={{ color: 'var(--text-muted)' }}>
                              {tool.version.slice(0, 16)}
                            </span>
                          )}
                        </button>
                      ))}
                    </div>
                  )}
                </div>
              </div>
            )}
          </div>

          {/* 数据预览区 */}
          {(activeMetrics || metrics) && (
            <div className="px-6 pb-4">
              <div
                className="p-4 rounded-xl"
                style={{
                  background: 'rgba(6,182,212,0.04)',
                  border: '1px solid rgba(6,182,212,0.12)',
                }}
              >
                <div className="flex items-center gap-2 mb-3">
                  <BarChart2 className="w-3.5 h-3.5" style={{ color: 'var(--neon-cyan)' }} />
                  <span className="text-xs font-semibold" style={{ color: 'var(--text-secondary)' }}>
                    将分析以下数据（近{reviewTimeRange}）
                  </span>
                </div>
                <div className="grid grid-cols-2 gap-2 relative">
                  {metricsLoading && (
                    <div className="absolute inset-0 flex items-center justify-center bg-black/5 dark:bg-white/5 backdrop-blur-[1px] rounded-xl z-10">
                      <RefreshCw className="w-4 h-4 text-neon-cyan animate-spin" />
                    </div>
                  )}
                  {[
                    { label: 'Token 消耗', value: activeMetrics ? activeMetrics.totalTokens.toLocaleString('zh-CN') : (metrics?.totalTokens ?? 0).toLocaleString('zh-CN') },
                    { label: '总费用 (USD)', value: activeMetrics ? `$${activeMetrics.totalCostUsd.toFixed(4)}` : `$${(metrics?.totalCostUsd ?? 0).toFixed(4)}` },
                    { label: '会话总数', value: activeMetrics ? activeMetrics.totalSessions.toLocaleString('zh-CN') : (metrics?.totalSessions ?? 0).toLocaleString('zh-CN') },
                    { label: '缓存命中率', value: activeMetrics ? `${(activeMetrics.cacheHitRate * 100).toFixed(1)}%` : `${((metrics?.cacheHitRate ?? 0) * 100).toFixed(1)}%` },
                  ].map(({ label, value }) => (
                    <div key={label} className={metricsLoading ? 'opacity-30 transition-opacity duration-300' : 'transition-opacity duration-300'}>
                      <p className="text-xs" style={{ color: 'var(--text-muted)' }}>{label}</p>
                      <p className="text-sm font-semibold font-mono" style={{ color: 'var(--text-primary)' }}>
                        {value}
                      </p>
                    </div>
                  ))}
                </div>
              </div>
            </div>
          )}

          {/* 操作按钮 */}
          <div className="px-6 pb-5">
            {!isAnalyzing && !isDone && (
              <button
                onClick={startAnalysis}
                disabled={!hasAnyCli || detectLoading || metricsLoading}
                className="w-full flex items-center justify-center gap-2.5 py-3 rounded-xl font-semibold text-sm transition-all disabled:opacity-40 disabled:cursor-not-allowed"
                style={{
                  background: hasAnyCli
                    ? 'linear-gradient(135deg, #0891b2, #7c3aed)'
                    : 'var(--card-bg)',
                  color: hasAnyCli ? '#ffffff' : 'var(--text-muted)',
                  boxShadow: hasAnyCli ? '0 4px 20px rgba(8,145,178,0.35)' : 'none',
                }}
              >
                <Zap className="w-4 h-4" />
                开始智能分析
              </button>
            )}

            {isAnalyzing && (
              <div className="flex items-center gap-3">
                <button
                  onClick={stopAnalysis}
                  className="flex-1 flex items-center justify-center gap-2 py-3 rounded-xl text-sm font-medium transition-all"
                  style={{
                    background: 'rgba(239,68,68,0.1)',
                    border: '1px solid rgba(239,68,68,0.25)',
                    color: '#ef4444',
                  }}
                >
                  <X className="w-4 h-4" />
                  停止分析
                </button>
                <div className="flex items-center gap-2 px-3 py-2 rounded-xl text-xs"
                  style={{ color: 'var(--neon-cyan)', background: 'rgba(6,182,212,0.08)' }}
                >
                  <div
                    className="w-1.5 h-1.5 rounded-full bg-cyan-400 animate-pulse"
                  />
                  正在分析…
                </div>
              </div>
            )}

            {isDone && (
              <div className="flex items-center gap-3">
                <button
                  onClick={() => {
                    setOutputText('');
                    outputTextRef.current = '';
                    setIsDone(false);
                    setError(null);
                  }}
                  className="flex-1 flex items-center justify-center gap-2 py-3 rounded-xl text-sm font-medium transition-all"
                  style={{
                    background: 'linear-gradient(135deg, #0891b2, #7c3aed)',
                    color: '#ffffff',
                    boxShadow: '0 4px 20px rgba(8,145,178,0.3)',
                  }}
                >
                  <RefreshCw className="w-3.5 h-3.5" />
                  重新生成
                </button>
                <button
                  onClick={handleCopy}
                  className="px-4 py-3 rounded-xl text-sm font-medium transition-all flex items-center gap-2"
                  style={{
                    background: 'var(--card-bg)',
                    border: '1px solid var(--card-border)',
                    color: 'var(--text-secondary)',
                  }}
                >
                  {copied ? (
                    <Check className="w-3.5 h-3.5 text-green-500" />
                  ) : (
                    <Copy className="w-3.5 h-3.5" />
                  )}
                  {copied ? '已复制' : '复制'}
                </button>
              </div>
            )}
          </div>

          {/* 错误提示 */}
          {error && (
            <div
              className="mx-6 mb-4 p-3 rounded-xl flex items-start gap-2"
              style={{
                background: 'rgba(239,68,68,0.08)',
                border: '1px solid rgba(239,68,68,0.2)',
              }}
            >
              <AlertCircle className="w-4 h-4 text-red-500 flex-shrink-0 mt-0.5" />
              <p className="text-xs text-red-500">{error}</p>
            </div>
          )}

          {/* 分析输出区 */}
          {(outputText || isAnalyzing) && (
            <div className="px-6 pb-6">
              <div className="flex items-center gap-2 mb-3">
                <Lightbulb className="w-3.5 h-3.5" style={{ color: 'var(--neon-gold)' }} />
                <span
                  className="text-xs font-semibold uppercase tracking-wider"
                  style={{ color: 'var(--text-muted)' }}
                >
                  分析结果
                </span>
              </div>
              <div
                ref={outputRef}
                className="review-output"
                style={{
                  background: 'var(--card-bg)',
                  border: '1px solid var(--card-border)',
                  borderRadius: '16px',
                  padding: '20px',
                  maxHeight: '600px',
                  overflowY: 'auto',
                  fontSize: '13px',
                  lineHeight: '1.7',
                  color: 'var(--text-primary)',
                }}
              >
                {outputText ? (
                  <div
                    dangerouslySetInnerHTML={{
                      __html: `<p class="md-p">${renderMarkdown(outputText)}</p>`,
                    }}
                  />
                ) : (
                  <div className="flex items-center gap-3 py-4">
                    <div
                      className="w-5 h-5 rounded-full border-2 animate-spin flex-shrink-0"
                      style={{
                        borderColor: 'var(--neon-cyan)',
                        borderTopColor: 'transparent',
                      }}
                    />
                    <span className="text-xs italic" style={{ color: 'var(--text-muted)' }}>
                      正在等待 {getCliDisplayName(selectedCli)} 响应…
                    </span>
                  </div>
                )}

                {/* 流式光标 */}
                {isAnalyzing && outputText && (
                  <span
                    className="inline-block w-0.5 h-4 ml-0.5 animate-pulse align-middle"
                    style={{ background: 'var(--neon-cyan)', borderRadius: '1px' }}
                  />
                )}
              </div>
            </div>
          )}

          {/* 空状态提示 */}
          {!outputText && !isAnalyzing && !error && hasAnyCli && (
            <div
              className="mx-6 mb-6 p-6 rounded-xl text-center"
              style={{
                background: 'rgba(6,182,212,0.03)',
                border: '1px dashed rgba(6,182,212,0.2)',
              }}
            >
              <Sparkles
                className="w-8 h-8 mx-auto mb-3"
                style={{ color: 'var(--neon-cyan)', opacity: 0.5 }}
              />
              <p className="text-sm font-medium mb-1" style={{ color: 'var(--text-secondary)' }}>
                准备好了！
              </p>
              <p className="text-xs" style={{ color: 'var(--text-muted)' }}>
                点击「开始智能分析」，{getCliDisplayName(selectedCli)} 将基于你的
                <br />
                历史 Token 数据生成专属复盘报告
              </p>
            </div>
          )}
        </div>
      </div>

      {/* Markdown 渲染样式（注入到 head） */}
      <style>{`
        .review-output .md-h1 {
          font-size: 1.1rem;
          font-weight: 700;
          margin: 1rem 0 0.5rem;
          color: var(--text-primary);
          padding-bottom: 0.4rem;
          border-bottom: 1px solid var(--card-border);
        }
        .review-output .md-h2 {
          font-size: 1rem;
          font-weight: 700;
          margin: 1.2rem 0 0.4rem;
          color: var(--neon-cyan);
        }
        .review-output .md-h3 {
          font-size: 0.875rem;
          font-weight: 600;
          margin: 0.8rem 0 0.3rem;
          color: var(--text-secondary);
        }
        .review-output .md-p {
          margin-bottom: 0.8rem;
        }
        .review-output .md-li {
          display: list-item;
          margin-left: 1.2rem;
          margin-bottom: 0.25rem;
          list-style-type: disc;
        }
        .review-output .md-oli {
          list-style-type: decimal;
        }
        .review-output .md-hr {
          border: none;
          border-top: 1px solid var(--card-border);
          margin: 1rem 0;
        }
        .review-output .md-code {
          background: rgba(15,23,42,0.06);
          border: 1px solid var(--card-border);
          border-radius: 8px;
          padding: 0.75rem 1rem;
          margin: 0.5rem 0;
          overflow-x: auto;
          font-size: 12px;
          font-family: 'JetBrains Mono', monospace;
          white-space: pre;
        }
        .dark .review-output .md-code {
          background: rgba(255,255,255,0.04);
        }
        .review-output .md-inline-code {
          background: rgba(6,182,212,0.1);
          color: var(--neon-cyan);
          border-radius: 4px;
          padding: 0.1em 0.4em;
          font-size: 0.875em;
          font-family: 'JetBrains Mono', monospace;
        }
        .review-output strong {
          font-weight: 700;
          color: var(--text-primary);
        }
        .review-output em {
          font-style: italic;
          color: var(--text-secondary);
        }
      `}</style>
    </>
  );
}
