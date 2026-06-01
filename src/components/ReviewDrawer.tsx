import React, { useState, useEffect, useRef, useCallback } from 'react';
import {
  Sparkles,
  Monitor,
  RefreshCw,
  AlertCircle,
  ExternalLink,
  Terminal,
  BarChart2,
  Zap,
  ChevronDown,
  ChevronUp,
  Edit3,
  History,
  ChevronLeft,
  Trash2,
  Copy,
  FileText,
  CircleDot,
  AlertTriangle,
  Download,
  Maximize2,
} from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { apiUrl, readJsonResponse } from '../lib/api';
import { Markdown } from './Markdown';

// ============================================================
// 常量与辅助配置
// ============================================================

const TIME_RANGE_OPTIONS = [
  { label: '今日', value: '今日' },
  { label: '近 7 天', value: '7天' },
  { label: '近 30 天', value: '30天' },
  { label: '周度对比 (本周 vs 上周)', value: '周度对比' },
  { label: '全部时间', value: 'all' },
];

const IDE_OPTIONS = [
  { label: '全部工具 (All)', value: 'all' },
  { label: 'Antigravity', value: 'antigravity' },
  { label: 'Claude Code', value: 'claude_code' },
  { label: 'Codex CLI', value: 'codex' },
  { label: 'Cursor', value: 'cursor' },
  { label: 'Trae', value: 'trae' },
  { label: 'Trae CN', value: 'trae_cn' },
];

const DEFAULT_PROMPT_TEMPLATE = `你是一位专业的 AI 工具使用顾问。我使用 Token Insight 追踪了我在 {{IDE}} 等工具上的 Token 消耗情况。

请根据下方我的使用数据，为我提供一份**深度使用复盘报告**，用中文回答。

---

## 我的使用数据（最近7天）

| 指标 | 数值 |
|------|------|
| 总 Token 消耗 | {{TOTAL_TOKENS}} tokens |
| 总费用 | \${{TOTAL_COST}} USD |
| 总会话数 | {{TOTAL_SESSIONS}} 次 |
| 缓存命中率 | {{CACHE_HIT_RATE}}% |
| 推理（Thinking）Token 占比 | {{THINKING_RATIO}}% |
{{SOURCE_BREAKDOWN}}{{MODEL_DISTRIBUTION}}{{DAILY_TREND_SUMMARY}}

---

## 请按以下结构输出分析报告（使用 Markdown 格式）：

### 1. 使用模式诊断
分析我的 AI 工具使用习惯，包括：
- 主要使用哪些工具/模型？
- 使用频率是否均匀，有无明显的高峰/低谷？
- 缓存命中率 {{CACHE_HIT_RATE}}% 是否合理？（业界参考：>30% 较好）
- 推理 Token 占比 {{THINKING_RATIO}}% 说明什么？

### 2. 成本优化建议
基于以上数据，给出 3~5 条具体、可操作的成本优化建议，例如：
- 哪些场景可以换用更便宜的模型？
- 如何提升缓存命中率？
- 是否存在明显的低效会话模式？

### 3. 效率评估
- 综合评价我的 AI 使用效率（满分100分，给出评分与理由）
- 与一般开发者的平均水平相比，我的数据表现如何？

### 4. 本周行动清单
列出 3 条我这周可以立刻执行的具体优化行动（要具体到操作步骤，不要泛泛而谈）。

---

请直接开始输出报告，不需要前言。保持语言简洁专业，使用 Markdown 格式。`;

const TEMPLATE_PRESETS = [
  {
    id: 'comprehensive',
    name: '📊 综合效能评估',
    description: '对用量、开销、缓存与提问质量进行全面、通用的效能诊断。',
    template: DEFAULT_PROMPT_TEMPLATE,
  },
  {
    id: 'cost_saving',
    name: '🔍 成本节流专项',
    description: '主攻降本增效，提供低配模型平替、高消耗 Turn 拦截、缓存提升建议。',
    template: `你是一位精通成本优化的 AI 治理专家。我使用 Token Insight 追踪了我在 {{IDE}} 等工具上的 Token 消耗情况。
请根据下方我的使用数据，为我提供一份**成本优化专项复盘报告**，用中文回答。

---

## 我的使用数据（最近7天）

| 指标 | 数值 |
|------|------|
| 总 Token 消耗 | {{TOTAL_TOKENS}} tokens |
| 总费用 | \${{TOTAL_COST}} USD |
| 总会话数 | {{TOTAL_SESSIONS}} 次 |
| 缓存命中率 | {{CACHE_HIT_RATE}}% |
| 推理（Thinking）Token 占比 | {{THINKING_RATIO}}% |
{{SOURCE_BREAKDOWN}}{{MODEL_DISTRIBUTION}}{{DAILY_TREND_SUMMARY}}

---

## 请按以下结构输出分析报告（使用 Markdown 格式）：

### 1. 成本与用量分布诊断
分析本次分析周期中最昂贵的消耗项、最高频的模型偏好，以及费用分布 the 理性。

### 2. 核心痛点与降本瓶颈
找出模型配比不合理（如在简单任务上过度使用昂贵模型）、缓存利用率低下、或者存在超长会话（Context 膨胀导致 Token 浪费）的瓶颈。

### 3. 降本增效平替建议
评估有哪些高频场景可以使用更轻量、更低成本的模型平替，或者如何更好地利用提示词缓存（Prompt Caching）。

### 4. 本周行动清单
针对上述发现，给出 3 条具体、可立即执行的降低 AI 成本的行动项，包括推荐的缓存策略和提问控制。

---

请直接开始输出报告，不需要前言。保持语言简洁专业，使用 Markdown 格式。`,
  },
  {
    id: 'collaboration',
    name: '⚡ 开发协作质量',
    description: '主攻提问艺术、代码迭代轮数合理性、上下文复用情况。',
    template: `你是一位敏捷开发与效能教练。我使用 Token Insight 追踪了我在 {{IDE}} 等工具上的 Token 消耗情况。
请根据下方我的使用数据，为我提供一份**人机协作质量诊断报告**，用中文回答。

---

## 我的使用数据（最近7天）

| 指标 | 数值 |
|------|------|
| 总 Token 消耗 | {{TOTAL_TOKENS}} tokens |
| 总费用 | \${{TOTAL_COST}} USD |
| 总会话数 | {{TOTAL_SESSIONS}} 次 |
| 缓存命中率 | {{CACHE_HIT_RATE}}% |
| 推理（Thinking）Token 占比 | {{THINKING_RATIO}}% |
{{SOURCE_BREAKDOWN}}{{MODEL_DISTRIBUTION}}{{DAILY_TREND_SUMMARY}}

---

## 请按以下结构输出分析报告（使用 Markdown 格式）：

### 1. 协同深度与频次诊断
总结我与 AI 协同的频次、单会话平均消耗和整体交互深度，分析使用习惯的健康度。

### 2. 效率瓶颈与低效会话
找出提问流中是否存在多次无效重试、提示词清晰度不足、或者单次会话包含太多不相关改动导致上下文负荷过重。

### 3. 提问艺术与上下文优化
评估我在提示词编写和 IDE 交互时，是否有效利用了上下文切片，以及如果改进提问习惯可以带来多大的效率增益。

### 4. 本周行动清单
给出 3 条提高提问效率和人机协作质量的黄金行动项（例如，推荐单次会话只关注单一职责，利用更清晰的任务边界等）。

---

请直接开始输出报告，不需要前言。保持语言简洁专业，使用 Markdown 格式。`,
  },
  {
    id: 'project_review',
    name: '💼 项目全景复盘',
    description: '分析跨项目用量分布、Token 集中度风险，为研发管理提供战略建议。',
    template: `你是一位技术总监。我使用 Token Insight 追踪了我在 {{IDE}} 等工具上的 Token 消耗情况。
请根据下方我的使用数据，为我提供一份**项目全景效能复盘报告**，用中文回答。

---

## 我的使用数据（最近7天）

| 指标 | 数值 |
|------|------|
| 总 Token 消耗 | {{TOTAL_TOKENS}} tokens |
| 总费用 | \${{TOTAL_COST}} USD |
| 总会话数 | {{TOTAL_SESSIONS}} 次 |
| 缓存命中率 | {{CACHE_HIT_RATE}}% |
| 推理（Thinking）Token 占比 | {{THINKING_RATIO}}% |
{{SOURCE_BREAKDOWN}}{{MODEL_DISTRIBUTION}}{{DAILY_TREND_SUMMARY}}

---

## 请按以下结构输出分析报告（使用 Markdown 格式）：

### 1. 工具集成与渗透度诊断
从全局视角概括我的项目使用分布、高频工具依赖和 AI 在不同开发环境的渗透情况。

### 2. 项目集中度风险分析
分析是否存在某单一 IDE/项目过度消耗导致资源倾斜，或者某些项目几乎没有使用 AI 辅助的效率断层。

### 3. 研发效能与资产化评估
评估跨工具协同的顺畅度，以及当模型或工具发生切换时，对整体交付速度和成本的战略性影响。

### 4. 本周行动清单
给出 3 条适用于团队或个人在跨项目开发时，规范 AI 工具使用和保护技术资产的宏观行动项。

---

请直接开始输出报告，不需要前言。保持语言简洁专业，使用 Markdown 格式。`,
  },
];

// ============================================================
// 类型定义
// ============================================================

interface CliToolInfo {
  name: string;
  available: boolean;
  version?: string;
  path?: string;
}

interface DetectResponse {
  tools: CliToolInfo[];
  recommended?: string;
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
  availableSources?: string[];
}

interface ReviewPageProps {
  metrics: ReviewMetrics | null;
}


interface ReviewTask {
  id: string;
  title: string;
  status: 'pending' | 'running' | 'succeeded' | 'failed' | 'canceled' | 'interrupted';
  cli_name: string;
  cli_path?: string;
  time_range: string;
  selected_ides_json: string;
  prompt_text: string;
  prompt_hash: string;
  metrics_snapshot_json: string;
  metrics_hash: string;
  dedupe_key: string;
  progress_stage: string;
  progress_percent: number;
  status_message: string;
  output_markdown: string;
  error_message?: string;
  exit_code?: number;
  created_at: string;
  started_at?: string;
  finished_at?: string;
  canceled_at?: string;
  last_heartbeat_at?: string;
  error_type?: string;
  quality_feedback?: string;
  action_items_json?: string;
  compare_metrics_snapshot_json?: string;
}

interface TaskEvent {
  id?: number;
  task_id: string;
  sequence: number;
  kind: 'stage' | 'progress' | 'stdout' | 'stderr' | 'heartbeat' | 'error' | 'done';
  message: string;
  payload_json?: string;
  created_at: string;
}

interface LogLine {
  time: string;
  type: 'stage' | 'stdout' | 'stderr' | 'heartbeat' | 'error' | 'sys';
  text: string;
}

// ============================================================
// 辅助工具方法
// ============================================================

function getCliDisplayName(bin: string): string {
  switch (bin) {
    case 'claude':
      return 'Claude Code';
    case 'codex':
      return 'Codex CLI';
    case 'gemini':
      return 'Gemini CLI';
    default:
      return 'AI CLI';
  }
}

function getReviewDateBounds(range: string) {
  const end = new Date();
  const start = new Date();

  const format = (d: Date) => {
    const y = d.getFullYear();
    const m = String(d.getMonth() + 1).padStart(2, '0');
    const day = String(d.getDate()).padStart(2, '0');
    return `${y}-${m}-${day}`;
  };

  if (range === '今日') {
    // 保持 start 为今天
  } else if (range === '7天') {
    start.setDate(end.getDate() - 7);
  } else if (range === '30天') {
    start.setDate(end.getDate() - 30);
  } else {
    start.setFullYear(end.getFullYear() - 5);
  }

  return { start: format(start), end: format(end) };
}

function buildPromptFromTemplate(template: string, ides: string[]): string {
  let ide_display = '';
  if (ides.includes('all') || ides.length === 0) {
    ide_display = '全部工具 (Antigravity、Claude Code、Codex CLI、Cursor、Trae、Trae CN)';
  } else {
    const mapped = ides.map((s) => {
      switch (s) {
        case 'antigravity':
          return 'Antigravity';
        case 'claude_code':
          return 'Claude Code';
        case 'codex':
          return 'Codex CLI';
        case 'cursor':
          return 'Cursor';
        case 'trae':
          return 'Trae';
        case 'trae_cn':
          return 'Trae CN';
        default:
          return s;
      }
    });
    ide_display = mapped.join('、');
  }
  return template.replace('{{IDE}}', ide_display);
}


// ============================================================
// 子组件：流式打字机效果光标
// ============================================================
function StreamingCursor() {
  return (
    <span
      className="inline-block w-1.5 h-4 ml-1 align-middle bg-cyan-400 animate-pulse"
      style={{ boxShadow: '0 0 6px #22d3ee' }}
    />
  );
}

// ============================================================
// 主组件 ReviewDrawer
// ============================================================

export function ReviewPage({ metrics }: ReviewPageProps) {
  // 核心视图切换: 'new' (新建复盘) | 'history' (任务历史) | 'detail' (详情与报告)
  const [view, setView] = useState<'new' | 'history' | 'detail'>('new');

  // CLI 引擎探测与参数状态
  const [detectResult, setDetectResult] = useState<DetectResponse | null>(null);
  const [detectLoading, setDetectLoading] = useState(false);
  const [selectedCli, setSelectedCli] = useState<string>('claude');

  const [reviewTimeRange, setReviewTimeRange] = useState<string>('7天');
  const [selectedIdes, setSelectedIdes] = useState<string[]>(['all']);
  const [customPrompt, setCustomPrompt] = useState<string>('');
  const [isPromptExpanded, setIsPromptExpanded] = useState(false);
  const [selectedTemplateId, setSelectedTemplateId] = useState<string>('comprehensive');
  const [compareMetrics, setCompareMetrics] = useState<ReviewMetrics | null>(null);


  // 指标快照与异步请求状态
  const [activeMetrics, setActiveMetrics] = useState<ReviewMetrics | null>(null);
  const [metricsLoading, setMetricsLoading] = useState(false);

  // 任务管理与历史记录状态
  const [historyTasks, setHistoryTasks] = useState<ReviewTask[]>([]);
  const [historyLoading, setHistoryLoading] = useState(false);
  const [statusFilter, setStatusFilter] = useState<string>('all');
  const [searchKeyword, setSearchKeyword] = useState<string>('');

  // 运行详情与实时 SSE 状态
  const [activeTask, setActiveTask] = useState<ReviewTask | null>(null);
  const [logLines, setLogLines] = useState<LogLine[]>([]);
  const [isLogsExpanded, setIsLogsExpanded] = useState(false);
  const [isSnapshotExpanded, setIsSnapshotExpanded] = useState(false);
  const [actionItems, setActionItems] = useState<Array<{ id: number; text: string; checked: boolean }>>([]);
  const [_hasNotificationPermission, setHasNotificationPermission] = useState(false);
  const [outputText, setOutputText] = useState('');
  const [copied, setCopied] = useState(false);

  // 去重匹配警告与强制启动控制
  const [dupWarningId, setDupWarningId] = useState<string | null>(null);
  const [conflictTask, setConflictTask] = useState<{ id: string; title: string } | null>(null);

  const esRef = useRef<EventSource | null>(null);
  const outputRef = useRef<HTMLDivElement>(null);
  const logsRef = useRef<HTMLDivElement>(null);

  // ────── 自动定位与提示词模板填充 ──────
  const lastAutoPromptRef = useRef('');
  useEffect(() => {
    const selectedPreset = TEMPLATE_PRESETS.find(p => p.id === selectedTemplateId) || TEMPLATE_PRESETS[0];
    const timeLabel = reviewTimeRange === '今日' ? '今日' : `最近${reviewTimeRange}`;
    const templateWithTime = selectedPreset.template.replace('最近7天', timeLabel);
    const newPrompt = buildPromptFromTemplate(templateWithTime, selectedIdes);

    if (customPrompt === '' || customPrompt === lastAutoPromptRef.current) {
      setCustomPrompt(newPrompt);
      lastAutoPromptRef.current = newPrompt;
    }
  }, [selectedIdes, reviewTimeRange, customPrompt, selectedTemplateId]);

  const handleSelectTemplate = (templateId: string) => {
    setSelectedTemplateId(templateId);
    const selectedPreset = TEMPLATE_PRESETS.find(p => p.id === templateId) || TEMPLATE_PRESETS[0];
    const timeLabel = reviewTimeRange === '今日' ? '今日' : `最近${reviewTimeRange}`;
    const templateWithTime = selectedPreset.template.replace('最近7天', timeLabel);
    const newPrompt = buildPromptFromTemplate(templateWithTime, selectedIdes);
    setCustomPrompt(newPrompt);
    lastAutoPromptRef.current = newPrompt;
  };

  // ────── 检测宿主机 CLI ──────
  const detectCliTools = async (force = false) => {
    setDetectLoading(true);
    try {
      const url = apiUrl(force ? '/review/detect?force=true' : '/review/detect');
      const res = await fetch(url);
      if (res.ok) {
        const data: DetectResponse = await readJsonResponse<DetectResponse>(res);
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

  // ────── 智能数据源提取指标快照 ──────
  useEffect(() => {
    if (view !== 'new') return;

    let isMounted = true;

    const aggregateMetrics = (results: any[], range: string): ReviewMetrics => {
      if (results.length === 1) {
        const data = results[0];
        return {
          timeRange: range,
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
        };
      } else {
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

          if (data.source_trends) {
            data.source_trends.forEach((item: any) => {
              combinedSourceBreakdownMap[item.source] = (combinedSourceBreakdownMap[item.source] || 0) + item.tokens;
            });
          }

          if (data.model_distribution) {
            data.model_distribution.forEach((m: any) => {
              combinedModelDistMap[m.model] = (combinedModelDistMap[m.model] || 0) + m.total_tokens;
            });
          }

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

        return {
          timeRange: range,
          totalTokens: totalInput + totalOutput,
          totalCostUsd: totalCost,
          totalSessions,
          cacheHitRate,
          thinkingRatio,
          sourceBreakdown: sourceBreakdown.length > 0 ? JSON.stringify(sourceBreakdown) : undefined,
          modelDistribution: modelDistribution.length > 0 ? JSON.stringify(modelDistribution) : undefined,
          dailyTrendSummary: dailyTrendSummary.length > 0 ? JSON.stringify(dailyTrendSummary) : undefined,
        };
      }
    };

    async function updateMetrics() {
      setMetricsLoading(true);
      try {
        const sourcesToFetch = selectedIdes.includes('all') || selectedIdes.length === 0 ? ['all'] : selectedIdes;

        if (reviewTimeRange === '周度对比') {
          // 1. 获取本周 (7天) 边界和数据
          const boundsCurrent = getReviewDateBounds('7天');
          const promisesCurrent = sourcesToFetch.map(async (src) => {
            const res = await fetch(
              apiUrl(`/metrics?source=${src}&start_date=${boundsCurrent.start}&end_date=${boundsCurrent.end}&t=${Date.now()}`)
            );
            if (!res.ok) throw new Error(`HTTP error! status: ${res.status}`);
            return readJsonResponse<any>(res);
          });
          const resultsCurrent = await Promise.all(promisesCurrent);

          // 2. 获取上周 (14天到7天前) 边界和数据
          const endCompare = new Date();
          endCompare.setDate(endCompare.getDate() - 7);
          const startCompare = new Date();
          startCompare.setDate(endCompare.getDate() - 14);
          const formatDate = (d: Date) => d.toISOString().slice(0, 10);
          const boundsCompare = { start: formatDate(startCompare), end: formatDate(endCompare) };

          const promisesCompare = sourcesToFetch.map(async (src) => {
            const res = await fetch(
              apiUrl(`/metrics?source=${src}&start_date=${boundsCompare.start}&end_date=${boundsCompare.end}&t=${Date.now()}`)
            );
            if (!res.ok) throw new Error(`HTTP error! status: ${res.status}`);
            return readJsonResponse<any>(res);
          });
          const resultsCompare = await Promise.all(promisesCompare);

          if (!isMounted) return;

          // 3. 聚合两个周期
          const currentMetrics = aggregateMetrics(resultsCurrent, '周度对比');
          const previousMetrics = aggregateMetrics(resultsCompare, '上周');

          setActiveMetrics(currentMetrics);
          setCompareMetrics(previousMetrics);
        } else {
          // 常规时间周期
          const bounds = getReviewDateBounds(reviewTimeRange);
          const promises = sourcesToFetch.map(async (src) => {
            const res = await fetch(
              apiUrl(`/metrics?source=${src}&start_date=${bounds.start}&end_date=${bounds.end}&t=${Date.now()}`)
            );
            if (!res.ok) throw new Error(`HTTP error! status: ${res.status}`);
            return readJsonResponse<any>(res);
          });
          const results = await Promise.all(promises);

          if (!isMounted) return;

          const currentMetrics = aggregateMetrics(results, reviewTimeRange);
          setActiveMetrics(currentMetrics);
          setCompareMetrics(null);
        }
      } catch (e) {
        console.error('动态拉取指标失败', e);
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
  }, [reviewTimeRange, selectedIdes, view]);

  // ────── 探测活跃后台任务 ──────
  const checkActiveTask = useCallback(async () => {
    try {
      const res = await fetch(apiUrl('/review/tasks/active'));
      if (res.ok) {
        const task = await res.json();
        if (task && task.id) {
          setView('detail');
          setActiveTask(task);
          setOutputText(task.output_markdown || '');
          setupLogLinesFromTask(task);
          connectToTaskEvents(task.id, 0);
          return true;
        }
      }
    } catch (e) {
      console.error('查询活跃后台任务错误:', e);
    }
    return false;
  }, []);

  // ────── 页面挂载生命周期控制 ──────
  useEffect(() => {
    detectCliTools();
    // 请求通知权限
    if (typeof window !== 'undefined' && 'Notification' in window) {
      if (Notification.permission === 'default') {
        Notification.requestPermission().then((p) => {
          setHasNotificationPermission(p === 'granted');
        });
      } else {
        setHasNotificationPermission(Notification.permission === 'granted');
      }
    }
    // 优先探测是否有已经在后台运行的任务
    checkActiveTask().then((hasActive) => {
      if (!hasActive) {
        setView('new');
      }
    });
    // 组件卸载时关闭 SSE 连接（后台任务本身不中断）
    return () => {
      disconnectTaskEvents();
    };
  }, [checkActiveTask]);

  // ────── 断开 SSE ──────
  const disconnectTaskEvents = () => {
    if (esRef.current) {
      esRef.current.close();
      esRef.current = null;
    }
  };

  // ────── 获取历史任务 ──────
  const fetchHistoryTasks = async () => {
    setHistoryLoading(true);
    try {
      let query = `?limit=40`;
      if (statusFilter !== 'all') {
        query += `&status=${statusFilter}`;
      }
      if (searchKeyword.trim() !== '') {
        query += `&q=${encodeURIComponent(searchKeyword.trim())}`;
      }
      const res = await fetch(apiUrl(`/review/tasks${query}`));
      if (res.ok) {
        const list = await res.json();
        setHistoryTasks(list);
      }
    } catch (e) {
      console.error('获取历史记录错误:', e);
    } finally {
      setHistoryLoading(false);
    }
  };

  useEffect(() => {
    if (view === 'history') {
      fetchHistoryTasks();
    }
  }, [view, statusFilter, searchKeyword]);

  // ────── 行动项管理与解析机制 ──────
  useEffect(() => {
    if (!activeTask) {
      setActionItems([]);
      return;
    }

    // 1. 如果数据库中已有行动项，直接反序列化读取展示
    if (activeTask.action_items_json) {
      try {
        const parsed = JSON.parse(activeTask.action_items_json);
        setActionItems(parsed);
      } catch {
        setActionItems([]);
      }
      return;
    }

    // 2. 如果任务成功且没有保存的行动项，并且有输出报告，则流式智能匹配提取并保存
    if (activeTask.status === 'succeeded' && outputText) {
      const lines = outputText.split('\n');
      const items: Array<{ id: number; text: string; checked: boolean }> = [];
      let inActionSection = false;
      let id = 1;

      for (const line of lines) {
        const trimmed = line.trim();
        if (trimmed.includes('本周行动清单') || trimmed.includes('行动计划') || trimmed.includes('行动清单')) {
          inActionSection = true;
          continue;
        }
        if (inActionSection) {
          if (trimmed.startsWith('#')) {
            if (!trimmed.includes('行动清单') && !trimmed.includes('本周行动')) {
              inActionSection = false;
              continue;
            }
          }
          const listMatch = trimmed.match(/^(?:[-*+]|\d+\.)\s+(.+)$/);
          if (listMatch) {
            let text = listMatch[1].trim();
            if (text.startsWith('[ ]') || text.startsWith('[x]')) {
              text = text.substring(3).trim();
            }
            items.push({ id: id++, text, checked: false });
          }
        }
      }

      if (items.length > 0) {
        setActionItems(items);
        // 保存提取出的行动项到后端 SQLite
        fetch(apiUrl(`/review/tasks/${activeTask.id}/action-items`), {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ action_items_json: JSON.stringify(items) }),
        }).catch((err) => console.error('保存智能行动项失败:', err));
      }
    }
  }, [activeTask, outputText]);

  // ────── 切换特定行动项勾选状态 ──────
  const handleToggleActionItem = async (itemId: number) => {
    if (!activeTask) return;
    const updated = actionItems.map((item) =>
      item.id === itemId ? { ...item, checked: !item.checked } : item
    );
    setActionItems(updated);
    try {
      await fetch(apiUrl(`/review/tasks/${activeTask.id}/action-items`), {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ action_items_json: JSON.stringify(updated) }),
      });
    } catch (err) {
      console.error('切换行动项失败:', err);
    }
  };



  // ────── 从已有任务初始化控制台日志 ──────
  const setupLogLinesFromTask = (task: ReviewTask) => {
    const lines: LogLine[] = [];
    const timeStr = new Date(task.created_at).toLocaleTimeString();
    lines.push({ time: timeStr, type: 'sys', text: `✨ 任务被载入详情面板。` });
    lines.push({ time: timeStr, type: 'sys', text: `⚙️ 参数配置: 天数=${task.time_range}, 引擎=${getCliDisplayName(task.cli_name)}` });
    
    if (task.status === 'succeeded') {
      lines.push({ time: timeStr, type: 'stage', text: '✅ 历史任务分析成功完成。' });
    } else if (task.status === 'failed') {
      lines.push({ time: timeStr, type: 'error', text: `❌ 历史任务以失败告终: ${task.status_message}` });
    } else if (task.status === 'canceled') {
      lines.push({ time: timeStr, type: 'sys', text: '⚪ 历史任务已被手动取消。' });
    } else if (task.status === 'interrupted') {
      lines.push({ time: timeStr, type: 'sys', text: '⚠️ 历史任务随软件重启而异常中断。' });
    }
    setLogLines(lines);
  };

  // ────── 连接实时 SSE 信号流（核心） ──────
  const connectToTaskEvents = (taskId: string, afterSeq = 0) => {
    disconnectTaskEvents();

    let textBuffer = '';
    const es = new EventSource(apiUrl(`/review/tasks/${taskId}/events?after=${afterSeq}`));
    esRef.current = es;

    es.onmessage = (event) => {
      // 容错兜底
      if (event.data === '[DONE]') {
        disconnectTaskEvents();
        refreshTaskDetail(taskId);
        return;
      }
    };

    const addLog = (type: LogLine['type'], text: string) => {
      const timeStr = new Date().toLocaleTimeString();
      setLogLines((prev) => [...prev, { time: timeStr, type, text }]);
      // 自动滚动控制台
      setTimeout(() => {
        if (logsRef.current) {
          logsRef.current.scrollTop = logsRef.current.scrollHeight;
        }
      }, 50);
    };

    // 监听各类高级事件
    es.addEventListener('stage', (event: any) => {
      try {
        const ev: TaskEvent = JSON.parse(event.data);
        addLog('stage', `🏁 [阶段进度] ${ev.message}`);
        
        // 动态同步更新 task percent
        if (ev.payload_json) {
          const payload = JSON.parse(ev.payload_json);
          setActiveTask((prev) => {
            if (!prev) return null;
            return {
              ...prev,
              progress_stage: payload.stage || prev.progress_stage,
              progress_percent: payload.percent || prev.progress_percent,
              status_message: ev.message,
            };
          });
        }
      } catch (e) {
        /* ignore */
      }
    });

    es.addEventListener('stdout', (event: any) => {
      try {
        const ev: TaskEvent = JSON.parse(event.data);
        textBuffer += ev.message;
        setOutputText(textBuffer);
        
        // 流式追加自动滚动
        setTimeout(() => {
          if (outputRef.current) {
            outputRef.current.scrollTop = outputRef.current.scrollHeight;
          }
        }, 50);
      } catch (e) {
        /* ignore */
      }
    });

    es.addEventListener('stderr', (event: any) => {
      try {
        const ev: TaskEvent = JSON.parse(event.data);
        addLog('stderr', `⚠️ [警告] ${ev.message}`);
      } catch (e) {
        /* ignore */
      }
    });

    es.addEventListener('heartbeat', (event: any) => {
      try {
        const ev: TaskEvent = JSON.parse(event.data);
        addLog('heartbeat', `💓 [引擎心跳] ${ev.message}`);
        setActiveTask((prev) => {
          if (!prev) return null;
          return { ...prev, status_message: ev.message };
        });
      } catch (e) {
        /* ignore */
      }
    });

    es.addEventListener('error', (event: any) => {
      try {
        const ev: TaskEvent = JSON.parse(event.data);
        addLog('error', `❌ [错误崩溃] ${ev.message}`);
        setActiveTask((prev) => {
          if (!prev) return null;
          return { ...prev, status: 'failed', status_message: ev.message };
        });

        if (typeof window !== 'undefined' && 'Notification' in window && Notification.permission === 'granted') {
          new Notification('AI 效能复盘失败', {
            body: `分析异常终止: ${ev.message}`,
            requireInteraction: false
          });
        }
      } catch (e) {
        /* ignore */
      }
    });

    es.addEventListener('done', () => {
      addLog('sys', '🏁 分析流程完毕，SSE 监听成功关闭。');
      disconnectTaskEvents();
      refreshTaskDetail(taskId);

      if (typeof window !== 'undefined' && 'Notification' in window && Notification.permission === 'granted') {
        new Notification('AI 效能复盘报告已完成', {
          body: '您的智能诊断和成本建议报告已准备就绪！',
          requireInteraction: false
        });
      }
    });

    es.onerror = () => {
      if (es.readyState === EventSource.CLOSED) return;
      addLog('stderr', '🔌 信号流连接断开，正在尝试重连中...');
    };
  };

  // ────── 重新拉取特定任务的数据库状态详情 ──────
  const refreshTaskDetail = async (taskId: string) => {
    try {
      const res = await fetch(apiUrl(`/review/tasks/${taskId}`));
      if (res.ok) {
        const task: ReviewTask = await res.json();
        setActiveTask(task);
        setOutputText(task.output_markdown || '');
      }
    } catch (e) {
      console.error(e);
    }
  };

  // ────── 发起新建智能复盘 ──────
  const handleStartAnalysis = async (forceStart = false) => {
    setDupWarningId(null);
    setConflictTask(null);

    const metricsToUse = activeMetrics || metrics;
    if (!metricsToUse) {
      alert('指标快照未准备好，无法开启分析。');
      return;
    }

    const payload: any = {
      cli: selectedCli,
      time_range: reviewTimeRange,
      selected_ides: selectedIdes,
      custom_prompt: customPrompt.trim() ? customPrompt.trim() : undefined,
      force: forceStart,
      metrics_snapshot: {
        totalTokens: metricsToUse.totalTokens,
        totalCostUsd: metricsToUse.totalCostUsd,
        totalSessions: metricsToUse.totalSessions,
        cacheHitRate: metricsToUse.cacheHitRate,
        thinkingRatio: metricsToUse.thinkingRatio,
        sourceBreakdown: metricsToUse.sourceBreakdown,
        modelDistribution: metricsToUse.modelDistribution,
        dailyTrendSummary: metricsToUse.dailyTrendSummary,
      },
      compare_metrics_snapshot: (reviewTimeRange === '周度对比' && compareMetrics) ? {
        totalTokens: compareMetrics.totalTokens,
        totalCostUsd: compareMetrics.totalCostUsd,
        totalSessions: compareMetrics.totalSessions,
        cacheHitRate: compareMetrics.cacheHitRate,
        thinkingRatio: compareMetrics.thinkingRatio,
        sourceBreakdown: compareMetrics.sourceBreakdown,
        modelDistribution: compareMetrics.modelDistribution,
        dailyTrendSummary: compareMetrics.dailyTrendSummary,
      } : undefined,
    };

    try {
      const res = await fetch(apiUrl('/review/tasks'), {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });

      if (res.status === 409) {
        const conflict = await res.json();
        setConflictTask({ id: conflict.task_id, title: conflict.message });
        return;
      }

      if (res.ok) {
        const data = await res.json();
        
        // 命中参数去重提示
        if (data.duplicate_of) {
          setDupWarningId(data.duplicate_of);
          return;
        }

        // 正常创建任务成功
        const task: ReviewTask = data;
        setView('detail');
        setActiveTask(task);
        setOutputText('');
        setLogLines([
          { time: new Date().toLocaleTimeString(), type: 'sys', text: `✨ 新建复盘任务已启动 (ID: ${task.id})` },
          { time: new Date().toLocaleTimeString(), type: 'stage', text: '🏁 任务准备就绪' }
        ]);
        
        // 连接 SSE 广播流
        connectToTaskEvents(task.id, 0);
      } else {
        const text = await res.text();
        alert(`创建分析任务失败: ${text}`);
      }
    } catch (e: any) {
      alert(`创建异常: ${e.message}`);
    }
  };

  // ────── 取消分析 ──────
  const handleCancelTask = async () => {
    if (!activeTask) return;
    if (!confirm('是否确定取消正在进行的 AI 智能复盘分析？\n这将终止后端的命令行子进程。')) return;

    try {
      const res = await fetch(apiUrl(`/review/tasks/${activeTask.id}/cancel`), {
        method: 'POST',
      });
      if (res.ok) {
        refreshTaskDetail(activeTask.id);
      }
    } catch (e) {
      console.error('取消分析出错:', e);
    }
  };

  // ────── 删除历史任务 ──────
  const handleDeleteTask = async (taskId: string, e: React.MouseEvent) => {
    e.stopPropagation();
    if (!confirm('确定彻底删除该条复盘报告历史记录吗？数据删除后将不可恢复。')) return;

    try {
      const res = await fetch(apiUrl(`/review/tasks/${taskId}`), {
        method: 'DELETE',
      });
      const data = await res.json();
      if (data.success) {
        if (view === 'history') {
          fetchHistoryTasks();
        } else if (view === 'detail' && activeTask?.id === taskId) {
          setView('history');
          setActiveTask(null);
        }
      } else {
        alert(data.message);
      }
    } catch (err) {
      alert(`删除异常: ${err instanceof Error ? err.message : String(err)}`);
    }
  };

  // ────── 一键重新运行分析 ──────
  const handleRetryTask = async (taskId: string) => {
    try {
      const res = await fetch(apiUrl(`/review/tasks/${taskId}/retry`), {
        method: 'POST',
      });
      if (res.ok) {
        // 重试成功后，重新获取详情并连接 SSE
        const detailRes = await fetch(apiUrl(`/review/tasks/${taskId}`));
        if (detailRes.ok) {
          const freshTask = await detailRes.json();
          setActiveTask(freshTask);
          setOutputText('');
          setLogLines([
            { time: new Date().toLocaleTimeString(), type: 'sys', text: `🔄 任务重试启动 (ID: ${taskId})` },
            { time: new Date().toLocaleTimeString(), type: 'stage', text: '🏁 任务重新排队中' }
          ]);
          connectToTaskEvents(taskId, 0);
        }
      } else {
        const text = await res.text();
        alert(`重试分析失败: ${text}`);
      }
    } catch (e) {
      alert(`重试异常: ${e instanceof Error ? e.message : String(e)}`);
    }
  };

  // ────── 快捷一键复制 ──────
  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(outputText);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      /* ignore */
    }
  };

  // ────── 导出为 Markdown 文件 ──────
  const handleExportMarkdown = () => {
    if (!activeTask || !outputText) return;
    const blob = new Blob([outputText], { type: 'text/markdown;charset=utf-8;' });
    const url = URL.createObjectURL(blob);
    const link = document.createElement('a');
    link.href = url;
    
    // 生成人性化文件名
    const safeTitle = activeTask.title.replace(new RegExp('[\\\\/:*?"<>|·]', 'g'), '_').trim();
    link.setAttribute('download', `${safeTitle}_AI复盘报告.md`);
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
  };

  // ────── 打开已有报告详情 ──────
  const handleOpenDetail = (task: ReviewTask) => {
    setView('detail');
    setActiveTask(task);
    setOutputText(task.output_markdown || '');
    setupLogLinesFromTask(task);

    // 如果任务仍在运行中，无缝接入 SSE 续连
    if (task.status === 'pending' || task.status === 'running') {
      connectToTaskEvents(task.id, 0);
    } else {
      disconnectTaskEvents();
    }
  };

  const availableTools = detectResult?.tools.filter((t) => t.available) ?? [];
  const hasAnyCli = availableTools.length > 0;

  // ============================================================
  // UI 渲染
  // ============================================================

  return (
    <div className="w-full flex flex-col animate-fade-in" style={{ minHeight: 0 }}>
        {/* ── 头部流光渐变 Header ── */}
        <div
          className="flex items-center justify-between px-6 py-4 flex-shrink-0 relative overflow-hidden"
          style={{
            borderBottom: '1px solid var(--card-border)',
            background:
              'linear-gradient(135deg, rgba(8,145,178,0.1) 0%, rgba(124,58,237,0.05) 50%, transparent 100%)',
          }}
        >
          {/* 流光装饰线 */}
          <div className="absolute top-0 left-0 right-0 h-[1.5px] bg-gradient-to-r from-cyan-500 via-purple-500 to-transparent animate-pulse" />
          
          <div className="flex items-center gap-3 relative z-10">
            <div
              className="w-10 h-10 rounded-2xl flex items-center justify-center"
              style={{
                background: 'linear-gradient(135deg, #0891b2, #7c3aed)',
                boxShadow: '0 4px 18px rgba(8,145,178,0.35)',
              }}
            >
              <Sparkles className="w-4.5 h-4.5 text-white" />
            </div>
            <div>
              <h2 className="text-base font-bold flex items-center gap-2" style={{ color: 'var(--text-primary)' }}>
                AI 复盘与治理中心
                <span className="text-[10px] font-mono px-2 py-0.5 rounded-full border border-neon-cyan/20 bg-neon-cyan/5 text-neon-cyan">
                  v2.0
                </span>
              </h2>
              <p className="text-xs" style={{ color: 'var(--text-muted)' }}>
                基于本机已部署的 AI 客户端引擎为您量身定制效能建议
              </p>
            </div>
          </div>
          
          <div className="flex items-center gap-2 relative z-10">
            {/* 顶栏视图切换 Tabs */}
            {view !== 'detail' && (
              <div className="flex rounded-full border border-card-border p-0.5 bg-black/10 dark:bg-white/5 mr-2">
                <button
                  onClick={() => setView('new')}
                  className={`px-4 py-1.5 rounded-full text-xs font-semibold transition-all duration-200 ${
                    view === 'new'
                      ? 'bg-gradient-to-r from-neon-cyan/20 to-neon-purple/20 border border-neon-cyan/30 text-neon-cyan shadow-sm'
                      : 'text-text-secondary hover:text-text-primary border border-transparent'
                  }`}
                >
                  <Zap className="w-3.5 h-3.5 inline mr-1" />
                  新建复盘
                </button>
                <button
                  onClick={() => setView('history')}
                  className={`px-4 py-1.5 rounded-full text-xs font-semibold transition-all duration-200 ${
                    view === 'history'
                      ? 'bg-gradient-to-r from-neon-cyan/20 to-neon-purple/20 border border-neon-cyan/30 text-neon-cyan shadow-sm'
                      : 'text-text-secondary hover:text-text-primary border border-transparent'
                  }`}
                >
                  <History className="w-3.5 h-3.5 inline mr-1" />
                  历史报告
                </button>
              </div>
            )}
            
          </div>
        </div>

        {/* ── 内容滚轴主体 ── */}
        <div className="flex-1 overflow-y-auto relative">

          {/* ============================================================ */}
          {/* VIEW: 新建复盘面板 */}
          {/* ============================================================ */}
          {view === 'new' && (
            <div className="px-6 py-5 space-y-5 animate-fade-in">
              {/* 并发冲突任务恢复卡片 */}
              {conflictTask && (
                <div className="p-4 rounded-2xl border border-rose-500/30 bg-rose-500/5 text-left flex gap-3.5 items-start">
                  <AlertTriangle className="w-5 h-5 text-rose-500 flex-shrink-0 mt-0.5" />
                  <div className="flex-1">
                    <h4 className="text-sm font-bold text-rose-400 mb-1">全局复盘分析锁冲突</h4>
                    <p className="text-xs text-text-secondary mb-3 leading-relaxed">
                      系统检测到当前已有复盘任务处于运行或等待中（同一时间只允许运行一个任务）。
                    </p>
                    <button
                      onClick={() => {
                        setView('detail');
                        refreshTaskDetail(conflictTask.id);
                        connectToTaskEvents(conflictTask.id, 0);
                        setConflictTask(null);
                      }}
                      className="px-3 py-1.5 rounded-xl bg-rose-500 hover:bg-rose-600 text-xs font-semibold text-white transition-all"
                    >
                      👉 立即前往查看当前运行中的任务
                    </button>
                  </div>
                </div>
              )}

              {/* 去重警告卡片 */}
              {dupWarningId && (
                <div className="p-4 rounded-2xl border border-amber-500/30 bg-amber-500/5 text-left flex gap-3.5 items-start">
                  <CircleDot className="w-5 h-5 text-amber-500 flex-shrink-0 mt-0.5 animate-pulse" />
                  <div className="flex-1">
                    <h4 className="text-sm font-bold text-amber-400 mb-1">相同参数历史报告已存在</h4>
                    <p className="text-xs text-text-secondary mb-3.5 leading-relaxed">
                      系统发现您在此之前已使用完全相同的数据天数、数据源、指标快照和提示词成功生成过复盘报告。为了节省您的 Token 额度和算力开销，建议直接复用。
                    </p>
                    <div className="flex gap-2">
                      <button
                        onClick={async () => {
                          const res = await fetch(apiUrl(`/review/tasks/${dupWarningId}`));
                          if (res.ok) {
                            const task = await res.json();
                            handleOpenDetail(task);
                          }
                          setDupWarningId(null);
                        }}
                        className="px-3.5 py-2 rounded-xl bg-gradient-to-r from-amber-500 to-orange-500 text-xs font-bold text-white transition-all active:scale-95"
                      >
                        📖 直接打开已有报告
                      </button>
                      <button
                        onClick={() => handleStartAnalysis(true)}
                        className="px-3.5 py-2 rounded-xl border border-card-border text-xs font-bold text-text-secondary hover:text-text-primary transition-all active:scale-95 bg-white/5"
                      >
                        ⚡ 强行强制重新生成
                      </button>
                    </div>
                  </div>
                </div>
              )}

              {/* 核心配置表单 */}
              <div className="rounded-[24px] bg-slate-50/2 dark:bg-white/1 border border-card-border p-5 space-y-4 shadow-sm text-left">
                <h3 className="text-xs font-bold text-text-secondary uppercase tracking-wider mb-2 flex items-center gap-1.5">
                  <Monitor className="w-4 h-4 text-neon-cyan" />
                  第一步：指定分析统计范围
                </h3>

                {/* 时间段选择 */}
                <div>
                  <label className="block text-xs font-semibold text-text-muted mb-2">📅 聚合时间周期</label>
                  <div className="flex gap-2">
                    {TIME_RANGE_OPTIONS.map((opt) => (
                      <button
                        key={opt.value}
                        onClick={() => setReviewTimeRange(opt.value)}
                        className="flex-1 py-2.5 rounded-xl text-xs font-semibold transition-all border border-card-border cursor-pointer"
                        style={
                          reviewTimeRange === opt.value
                            ? {
                                background: 'linear-gradient(135deg, rgba(8,145,178,0.16), rgba(124,58,237,0.16))',
                                border: '1px solid rgba(8,145,178,0.6)',
                                color: 'var(--text-primary)',
                                boxShadow: '0 0 12px rgba(8,145,178,0.12)',
                              }
                            : {
                                background: 'var(--card-bg)',
                                color: 'var(--text-muted)',
                              }
                        }
                      >
                        {opt.label}
                      </button>
                    ))}
                  </div>
                </div>

                {/* IDE 数据源 */}
                <div>
                  <label className="block text-xs font-semibold text-text-muted mb-2">💻 关联分析 IDE 数据源</label>
                  <div className="flex flex-wrap gap-2">
                    {IDE_OPTIONS.map((opt) => {
                      const isSelected = selectedIdes.includes(opt.value);
                      return (
                        <button
                          key={opt.value}
                          onClick={() => {
                            if (opt.value === 'all') {
                              setSelectedIdes(['all']);
                            } else {
                              let next = selectedIdes.filter((x) => x !== 'all');
                              if (isSelected) {
                                next = next.filter((x) => x !== opt.value);
                                if (next.length === 0) next = ['all'];
                              } else {
                                next.push(opt.value);
                              }
                              setSelectedIdes(next);
                            }
                          }}
                          className={`px-3 py-1.5 rounded-xl text-xs font-medium border transition-all cursor-pointer ${
                            isSelected
                              ? 'bg-neon-cyan/15 border-neon-cyan/40 text-neon-cyan font-semibold'
                              : 'bg-white/3 hover:bg-white/5 border-card-border text-text-muted hover:text-text-secondary'
                          }`}
                        >
                          {opt.label}
                        </button>
                      );
                    })}
                  </div>
                </div>

                {/* 指标快照微渐变白卡预览 */}
                {(activeMetrics || metrics) && (
                  <div
                    className="p-4 rounded-2xl relative overflow-hidden bg-gradient-to-br from-blue-500/5 via-transparent to-purple-500/5 border border-cyan-500/10"
                  >
                    <div className="absolute top-0 right-0 w-24 h-24 bg-cyan-500/5 rounded-full blur-2xl pointer-events-none" />
                    <div className="flex items-center justify-between mb-3.5 border-b border-card-border pb-2.5">
                      <div className="flex items-center gap-2">
                        <BarChart2 className="w-4 h-4 text-neon-cyan" />
                        <span className="text-xs font-bold" style={{ color: 'var(--text-secondary)' }}>
                          已冻结大盘快照（分析此数据）
                        </span>
                      </div>
                      <span className="text-[10px] font-mono text-neon-cyan uppercase tracking-wider bg-neon-cyan/10 border border-neon-cyan/20 px-2 py-0.5 rounded-full">
                        snapshot ready
                      </span>
                    </div>

                    <div className="relative">
                      {metricsLoading && (
                        <div className="absolute inset-0 flex items-center justify-center bg-black/10 dark:bg-white/5 backdrop-blur-[1px] rounded-xl z-10">
                          <RefreshCw className="w-5 h-5 text-neon-cyan animate-spin" />
                        </div>
                      )}
                      
                      {reviewTimeRange === '周度对比' && compareMetrics ? (
                        <div className="grid grid-cols-2 gap-3">
                          {[
                            { 
                              label: 'Token 环比消耗', 
                              currStr: activeMetrics ? activeMetrics.totalTokens.toLocaleString() : (metrics?.totalTokens ?? 0).toLocaleString(),
                              prevStr: compareMetrics.totalTokens.toLocaleString(),
                              diff: (() => {
                                const curr = activeMetrics ? activeMetrics.totalTokens : (metrics?.totalTokens ?? 0);
                                const prev = compareMetrics.totalTokens;
                                if (!prev) return null;
                                const pct = ((curr - prev) / prev) * 100;
                                return { text: `${pct > 0 ? '+' : ''}${pct.toFixed(1)}%`, color: pct > 0 ? 'text-rose-400' : 'text-emerald-400' };
                              })()
                            },
                            { 
                              label: '预估费用环比 (USD)', 
                              currStr: activeMetrics ? `$${activeMetrics.totalCostUsd.toFixed(4)}` : `$${(metrics?.totalCostUsd ?? 0).toFixed(4)}`,
                              prevStr: `$${compareMetrics.totalCostUsd.toFixed(4)}`,
                              diff: (() => {
                                const curr = activeMetrics ? activeMetrics.totalCostUsd : (metrics?.totalCostUsd ?? 0);
                                const prev = compareMetrics.totalCostUsd;
                                if (!prev) return null;
                                const pct = ((curr - prev) / prev) * 100;
                                return { text: `${pct > 0 ? '+' : ''}${pct.toFixed(1)}%`, color: pct > 0 ? 'text-rose-400' : 'text-emerald-400' };
                              })()
                            },
                            { 
                              label: '会话环比次数', 
                              currStr: activeMetrics ? activeMetrics.totalSessions.toLocaleString() : (metrics?.totalSessions ?? 0).toLocaleString(),
                              prevStr: compareMetrics.totalSessions.toLocaleString(),
                              diff: (() => {
                                const curr = activeMetrics ? activeMetrics.totalSessions : (metrics?.totalSessions ?? 0);
                                const prev = compareMetrics.totalSessions;
                                if (!prev) return null;
                                const pct = ((curr - prev) / prev) * 100;
                                return { text: `${pct > 0 ? '+' : ''}${pct.toFixed(1)}%`, color: pct > 0 ? 'text-rose-400' : 'text-emerald-400' };
                              })()
                            },
                            { 
                              label: '缓存命中率变动', 
                              currStr: activeMetrics ? `${(activeMetrics.cacheHitRate * 100).toFixed(1)}%` : `${((metrics?.cacheHitRate ?? 0) * 100).toFixed(1)}%`,
                              prevStr: `${(compareMetrics.cacheHitRate * 100).toFixed(1)}%`,
                              diff: (() => {
                                const curr = activeMetrics ? activeMetrics.cacheHitRate : (metrics?.cacheHitRate ?? 0);
                                const prev = compareMetrics.cacheHitRate;
                                const diff = (curr - prev) * 100;
                                return { text: `${diff > 0 ? '+' : ''}${diff.toFixed(1)}%`, color: diff > 0 ? 'text-emerald-400' : 'text-rose-400' }; // 命中率上升是绿
                              })()
                            },
                          ].map(({ label, currStr, prevStr, diff }) => (
                            <div key={label} className={`p-3 rounded-xl bg-black/5 dark:bg-white/3 border border-card-border/50 ${metricsLoading ? 'opacity-30' : ''}`}>
                              <p className="text-[10px] text-text-muted font-medium mb-1">{label}</p>
                              <div className="flex items-baseline justify-between gap-1">
                                <span className="text-xs font-bold font-mono text-text-primary">{currStr}</span>
                                {diff && <span className={`text-[10px] font-mono font-bold ${diff.color}`}>{diff.text}</span>}
                              </div>
                              <p className="text-[9px] text-text-muted mt-1 font-mono">上周: {prevStr}</p>
                            </div>
                          ))}
                        </div>
                      ) : (
                        <div className="grid grid-cols-2 gap-3">
                          {[
                            { label: 'Token 总消耗', value: activeMetrics ? activeMetrics.totalTokens.toLocaleString() : (metrics?.totalTokens ?? 0).toLocaleString() },
                            { label: '总预估费用 (USD)', value: activeMetrics ? `$${activeMetrics.totalCostUsd.toFixed(4)}` : `$${(metrics?.totalCostUsd ?? 0).toFixed(4)}` },
                            { label: '会话交互总数', value: activeMetrics ? activeMetrics.totalSessions.toLocaleString() : (metrics?.totalSessions ?? 0).toLocaleString() },
                            { label: '缓存命中率', value: activeMetrics ? `${(activeMetrics.cacheHitRate * 100).toFixed(1)}%` : `${((metrics?.cacheHitRate ?? 0) * 100).toFixed(1)}%` },
                          ].map(({ label, value }) => (
                            <div key={label} className={`p-2 rounded-xl bg-black/5 dark:bg-white/3 border border-card-border/50 ${metricsLoading ? 'opacity-30' : ''}`}>
                              <p className="text-[10px] text-text-muted font-medium mb-0.5">{label}</p>
                              <p className="text-xs font-bold font-mono text-text-primary">
                                {value}
                              </p>
                            </div>
                          ))}
                        </div>
                      )}
                    </div>
                  </div>
                )}
              </div>

              {/* 规则参数步骤二：CLI 引擎 */}
              <div className="rounded-[24px] bg-slate-50/2 dark:bg-white/1 border border-card-border p-5 space-y-4 shadow-sm text-left">
                <div className="flex items-center justify-between">
                  <h3 className="text-xs font-bold text-text-secondary uppercase tracking-wider flex items-center gap-1.5">
                    <Terminal className="w-4 h-4 text-neon-purple" />
                    第二步：选择运行分析引擎 CLI
                  </h3>
                  <button
                    onClick={() => detectCliTools(true)}
                    disabled={detectLoading}
                    className="flex items-center gap-1 text-[10px] px-2 py-1 rounded-lg border border-neon-cyan/25 bg-neon-cyan/5 text-neon-cyan hover:bg-neon-cyan/10 cursor-pointer"
                  >
                    <RefreshCw className={`w-2.5 h-2.5 ${detectLoading ? 'animate-spin' : ''}`} />
                    重新检测
                  </button>
                </div>

                {detectLoading ? (
                  <div className="flex items-center gap-2.5 py-2">
                    <div className="w-4.5 h-4.5 rounded-full border-2 border-neon-cyan border-t-transparent animate-spin" />
                    <span className="text-xs text-text-muted">正在扫描系统 PATH 寻找已安装的 AI CLI 引擎...</span>
                  </div>
                ) : detectResult ? (
                  <div className="flex flex-wrap gap-2">
                    {detectResult.tools.map((tool) => (
                      <div
                        key={tool.name}
                        onClick={() => tool.available && setSelectedCli(tool.name)}
                        className={`flex items-center gap-2 px-3 py-2 rounded-xl text-xs font-semibold border transition-all cursor-pointer ${
                          tool.available
                            ? selectedCli === tool.name
                              ? 'bg-neon-purple/15 border-neon-purple/50 text-neon-purple font-bold'
                              : 'bg-green-500/5 border-green-500/20 text-green-600 dark:text-green-400'
                            : 'bg-gray-500/5 border-gray-400/10 text-gray-400 opacity-60 cursor-not-allowed'
                        }`}
                      >
                        <span className={`w-1.5 h-1.5 rounded-full ${tool.available ? 'bg-green-500 animate-pulse' : 'bg-gray-400'}`} />
                        <span>{getCliDisplayName(tool.name)}</span>
                        {tool.version && <span className="opacity-50 text-[10px] font-mono font-normal">({tool.version.slice(0, 10)})</span>}
                      </div>
                    ))}
                  </div>
                ) : (
                  <p className="text-xs text-text-muted">点击检测加载可用 AI 工具</p>
                )}

                {/* 未探测到 CLI 指向 */}
                {detectResult && !hasAnyCli && (
                  <div className="p-4 rounded-2xl bg-orange-500/5 border border-orange-500/25 flex items-start gap-3">
                    <AlertCircle className="w-5 h-5 text-orange-500 flex-shrink-0 mt-0.5" />
                    <div>
                      <p className="text-xs font-bold text-neon-orange mb-1">未检测到兼容的本地 AI CLI 引擎</p>
                      <p className="text-[11px] text-text-secondary leading-relaxed mb-2.5">
                        本复盘大师需要调用您本机全局配置已登录的 AI 交互工具。请参考安装：
                      </p>
                      <div className="flex gap-3">
                        <a
                          href="https://docs.anthropic.com/claude-code"
                          target="_blank"
                          rel="noopener noreferrer"
                          className="text-[10px] font-bold text-neon-orange hover:underline flex items-center gap-1"
                        >
                          <ExternalLink className="w-3 h-3" /> 安装 Claude Code
                        </a>
                        <span className="text-[10px] text-text-muted">|</span>
                        <a
                          href="https://github.com/openai/codex"
                          target="_blank"
                          rel="noopener noreferrer"
                          className="text-[10px] font-bold text-orange-400 hover:underline flex items-center gap-1"
                        >
                          <ExternalLink className="w-3 h-3" /> 安装 Codex CLI
                        </a>
                      </div>
                    </div>
                  </div>
                )}
              </div>

              {/* 自定义提示词配置 */}
              <div className="rounded-[24px] bg-slate-50/2 dark:bg-white/1 border border-card-border p-5 text-left">
                <button
                  onClick={() => setIsPromptExpanded(!isPromptExpanded)}
                  className="w-full flex items-center justify-between text-xs font-bold text-text-secondary uppercase tracking-wider cursor-pointer"
                >
                  <span className="flex items-center gap-1.5">
                    <Edit3 className="w-4 h-4 text-neon-gold" />
                    第三步：调整专家分析提示词 (可选)
                  </span>
                  {isPromptExpanded ? <ChevronUp className="w-4 h-4 text-text-muted" /> : <ChevronDown className="w-4 h-4 text-text-muted" />}
                </button>

                {isPromptExpanded && (
                  <div className="mt-3.5 border-t border-card-border pt-3.5 space-y-4">
                    {/* 模板选择预设 */}
                    <div className="space-y-2">
                      <label className="block text-[11px] font-bold text-text-secondary uppercase tracking-wider">📋 选择专家分析切入点/模板</label>
                      <div className="grid grid-cols-2 gap-2">
                        {TEMPLATE_PRESETS.map((preset) => (
                          <button
                            key={preset.id}
                            type="button"
                            onClick={() => handleSelectTemplate(preset.id)}
                            className="p-3 rounded-xl border text-left cursor-pointer transition-all flex flex-col justify-between hover:scale-[1.01] duration-150"
                            style={
                              selectedTemplateId === preset.id
                                ? {
                                    background: 'linear-gradient(135deg, rgba(8,145,178,0.12), rgba(124,58,237,0.12))',
                                    borderColor: 'rgba(8,145,178,0.5)',
                                    boxShadow: '0 0 10px rgba(8,145,178,0.08)',
                                  }
                                : {
                                    background: 'var(--card-bg)',
                                    borderColor: 'var(--card-border)',
                                  }
                            }
                          >
                            <span className="text-[11px] font-bold text-text-primary block mb-0.5">{preset.name}</span>
                            <span className="text-[9px] text-text-muted leading-relaxed block">{preset.description}</span>
                          </button>
                        ))}
                      </div>
                    </div>

                    <div className="space-y-1.5">
                      <label className="block text-[11px] font-bold text-text-secondary uppercase tracking-wider">✏️ 自定义提示词骨架编辑</label>
                      <p className="text-[10px] text-text-muted leading-relaxed">
                        * 系统会为 AI 引擎自动附带大盘数据。您可以在此修改提示词骨架，指导 AI 进行更贴切的诊断。
                      </p>
                      <textarea
                        value={customPrompt}
                        onChange={(e) => setCustomPrompt(e.target.value)}
                        rows={10}
                        spellCheck={false}
                        className="w-full p-3 rounded-xl border border-card-border bg-black/10 dark:bg-black/30 font-mono text-[11px] leading-relaxed text-text-primary outline-none focus:border-cyan-500 transition-all"
                      />
                    </div>
                  </div>
                )}
              </div>

              {/* 隐私与安全边界提示横幅 */}
              {hasAnyCli && (
                <div 
                  className="p-3.5 rounded-2xl border text-left flex items-start gap-2.5"
                  style={{
                    border: '1px solid rgba(8, 145, 178, 0.15)',
                    background: 'linear-gradient(135deg, rgba(8, 145, 178, 0.05) 0%, transparent 100%)',
                  }}
                >
                  <Sparkles className="w-4 h-4 text-neon-cyan mt-0.5 flex-shrink-0" />
                  <div className="space-y-1">
                    <h4 className="text-[11px] font-bold text-neon-cyan">🛡️ 本地 AI 诊断隐私与安全边界提示</h4>
                    <p className="text-[10px] text-text-secondary leading-relaxed">
                      AI 诊断工具（如 Claude Code）将完全在您**本机开发环境内**离线运行，仅在生成诊断时引用您勾选的 Token 用量趋势与大盘统计。我们**绝不会**将您的源代码文件、数据库内容或本地隐私配置上传到第三方服务器。
                    </p>
                  </div>
                </div>
              )}

              {/* 动作发起大按钮 */}
              <div className="pt-2">
                <button
                  onClick={() => handleStartAnalysis(false)}
                  disabled={!hasAnyCli || detectLoading || metricsLoading}
                  className="w-full py-3.5 rounded-2xl font-bold text-sm text-white shadow-lg shadow-cyan-500/20 active:scale-[0.98] transition-all flex items-center justify-center gap-2 cursor-pointer disabled:opacity-45 disabled:cursor-not-allowed"
                  style={{
                    background: hasAnyCli ? 'linear-gradient(135deg, #0891b2, #7c3aed)' : 'var(--card-border)',
                  }}
                >
                  <Zap className="w-4.5 h-4.5 text-white" />
                  开始智能效能分析报告
                </button>
              </div>
            </div>
          )}

          {/* ============================================================ */}
          {/* VIEW: 历史任务面板 */}
          {/* ============================================================ */}
          {view === 'history' && (
            <div className="px-6 py-5 space-y-4 animate-fade-in text-left">
              {/* 控制搜索与筛选面板 */}
              <div className="flex gap-2.5 items-center justify-between pb-2 border-b border-card-border">
                <div className="flex gap-2">
                  <select
                    value={statusFilter}
                    onChange={(e) => setStatusFilter(e.target.value)}
                    className="bg-black/10 dark:bg-white/5 border border-card-border rounded-xl px-3 py-1.5 text-xs text-text-primary outline-none"
                  >
                    <option value="all">🔍 全部状态</option>
                    <option value="succeeded">✅ 分析完成</option>
                    <option value="failed">❌ 失败任务</option>
                    <option value="canceled">⚪ 已取消</option>
                    <option value="running">🔄 运行中</option>
                    <option value="pending">⏳ 排队中</option>
                    <option value="interrupted">⚠️ 已中断</option>
                  </select>
                </div>
                
                <input
                  type="text"
                  placeholder="搜索报告关键字..."
                  value={searchKeyword}
                  onChange={(e) => setSearchKeyword(e.target.value)}
                  className="bg-black/10 dark:bg-white/5 border border-card-border rounded-xl px-3 py-1.5 text-xs text-text-primary placeholder-text-muted outline-none w-52 focus:border-cyan-500 focus:w-64 transition-all"
                />
              </div>

              {historyLoading ? (
                <div className="py-20 text-center flex flex-col items-center gap-3">
                  <RefreshCw className="w-8 h-8 text-neon-cyan animate-spin" />
                  <span className="text-xs text-text-muted">正在加载历史复盘记录...</span>
                </div>
              ) : historyTasks.length > 0 ? (
                <div className="space-y-3">
                  {historyTasks.map((task) => {
                    const dateStr = new Date(task.created_at).toLocaleString();
                    const wordsCount = task.output_markdown ? task.output_markdown.length : 0;
                    
                    return (
                      <div
                        key={task.id}
                        onClick={() => handleOpenDetail(task)}
                        className="p-4 rounded-[20px] bg-slate-50/2 dark:bg-white/1 border border-card-border/80 hover:border-cyan-500/30 hover:shadow-md cursor-pointer transition-all duration-200 text-left flex justify-between items-start gap-4"
                      >
                        <div className="space-y-2 flex-1">
                          <div className="flex items-center gap-2 flex-wrap">
                            {/* 状态彩牌 */}
                            {task.status === 'succeeded' && (
                              <span className="text-[9px] font-bold uppercase tracking-wider bg-neon-green/10 text-neon-green border border-neon-green/20 px-2 py-0.5 rounded-full">
                                分析完成
                              </span>
                            )}
                            {task.status === 'running' && (
                              <span className="text-[9px] font-bold uppercase tracking-wider bg-neon-cyan/10 text-neon-cyan border border-neon-cyan/20 px-2 py-0.5 rounded-full animate-pulse">
                                运行中 ({task.progress_percent}%)
                              </span>
                            )}
                            {task.status === 'pending' && (
                              <span className="text-[9px] font-bold uppercase tracking-wider bg-neon-cyan/10 text-neon-cyan border border-neon-cyan/20 px-2 py-0.5 rounded-full animate-pulse">
                                排队中
                              </span>
                            )}
                            {task.status === 'failed' && (
                              <span className="text-[9px] font-bold uppercase tracking-wider bg-neon-pink/10 text-neon-pink border border-neon-pink/20 px-2 py-0.5 rounded-full">
                                分析失败
                              </span>
                            )}
                            {task.status === 'canceled' && (
                              <span className="text-[9px] font-bold uppercase tracking-wider bg-gray-500/10 text-text-muted border border-gray-500/10 px-2 py-0.5 rounded-full">
                                已取消
                              </span>
                            )}
                            {task.status === 'interrupted' && (
                              <span className="text-[9px] font-bold uppercase tracking-wider bg-neon-gold/10 text-neon-gold border border-neon-gold/20 px-2 py-0.5 rounded-full">
                                已中断
                              </span>
                            )}
                            
                            <h4 className="text-xs font-bold text-text-primary">{task.title}</h4>
                          </div>

                          <div className="flex gap-4 text-[10px] text-text-muted font-medium flex-wrap">
                            <span>🕒 {dateStr}</span>
                            {wordsCount > 0 && <span>📝 报告字数: {wordsCount.toLocaleString()}</span>}
                            <span>🛠️ {getCliDisplayName(task.cli_name)}</span>
                          </div>
                        </div>

                        <div className="flex items-center gap-1 flex-shrink-0">
                          <button
                            onClick={(e) => handleDeleteTask(task.id, e)}
                            className="p-1.5 rounded-lg border border-transparent hover:border-rose-500/20 hover:bg-rose-500/10 text-text-muted hover:text-rose-500 transition-all cursor-pointer"
                            title="删除报告"
                          >
                            <Trash2 className="w-3.5 h-3.5" />
                          </button>
                        </div>
                      </div>
                    );
                  })}
                </div>
              ) : (
                <div className="py-24 text-center flex flex-col items-center gap-3">
                  <FileText className="w-12 h-12 text-card-border" />
                  <p className="text-xs text-text-muted">没有找到符合条件的复盘报告历史记录。</p>
                  <button
                    onClick={() => setView('new')}
                    className="text-xs font-bold text-neon-cyan hover:underline bg-transparent border-none cursor-pointer"
                  >
                    👉 立即创建首份 AI 诊断报告
                  </button>
                </div>
              )}
            </div>
          )}

          {/* ============================================================ */}
          {/* VIEW: 任务详情与报告渲染 */}
          {/* ============================================================ */}
          {view === 'detail' && activeTask && (
            <div className="px-6 py-5 space-y-4 animate-fade-in text-left">
              {/* 顶栏控制导航 */}
              <div className="flex items-center justify-between border-b border-card-border pb-3">
                <button
                  onClick={() => {
                    setView(activeTask.status === 'running' || activeTask.status === 'pending' ? 'new' : 'history');
                    disconnectTaskEvents();
                  }}
                  className="flex items-center gap-1.5 text-xs text-neon-cyan hover:text-neon-cyan/80 font-bold bg-transparent border-none cursor-pointer"
                >
                  <ChevronLeft className="w-4.5 h-4.5" />
                  返回{activeTask.status === 'running' || activeTask.status === 'pending' ? '新建配置' : '历史列表'}
                </button>

                <div className="flex gap-2">
                  <button
                    onClick={handleCopy}
                    disabled={!outputText}
                    className="px-3 py-1.5 rounded-xl border border-card-border bg-white/5 text-xs font-semibold text-text-secondary hover:text-neon-cyan hover:border-neon-cyan/30 disabled:opacity-40 transition-all cursor-pointer"
                  >
                    <Copy className="w-3 h-3 inline mr-1" />
                    {copied ? '已复制！' : '复制全文'}
                  </button>
                  <button
                    onClick={handleExportMarkdown}
                    disabled={!outputText}
                    className="px-3 py-1.5 rounded-xl border border-card-border bg-white/5 text-xs font-semibold text-text-secondary hover:text-neon-cyan hover:border-neon-cyan/30 disabled:opacity-40 transition-all cursor-pointer"
                  >
                    <Download className="w-3 h-3 inline mr-1" />
                    导出 MD
                  </button>
                  {activeTask.status === 'succeeded' && (
                    <button
                      onClick={() => window.print()}
                      className="px-3 py-1.5 rounded-xl border border-card-border bg-white/5 text-xs font-semibold text-text-secondary hover:text-neon-cyan hover:border-neon-cyan/30 disabled:opacity-40 transition-all cursor-pointer"
                    >
                      🖨️ 打印/导出 PDF
                    </button>
                  )}
                  {activeTask.status !== 'running' && activeTask.status !== 'pending' && (
                    <button
                      onClick={(e) => handleDeleteTask(activeTask.id, e)}
                      className="px-3 py-1.5 rounded-xl border border-transparent hover:border-rose-500/20 bg-rose-500/10 text-xs font-semibold text-rose-400 hover:bg-rose-500/20 transition-all cursor-pointer"
                    >
                      <Trash2 className="w-3 h-3 inline mr-1" />
                      删除
                    </button>
                  )}
                </div>
              </div>

              {/* 运行阶段进度条面板 (仅当 pending/running 时显示) */}
              {(activeTask.status === 'running' || activeTask.status === 'pending') && (
                <div className="p-4 rounded-2xl bg-neon-cyan/5 border border-neon-cyan/15 space-y-2.5">
                  <div className="flex items-center justify-between text-xs">
                    <span className="font-bold text-neon-cyan animate-pulse flex items-center gap-1.5">
                      <RefreshCw className="w-3.5 h-3.5 animate-spin" />
                      {activeTask.status_message || '任务拉起中...'}
                    </span>
                    <span className="font-mono font-bold text-neon-cyan">{activeTask.progress_percent}%</span>
                  </div>

                  {/* 进度槽 */}
                  <div className="w-full h-2 rounded-full bg-black/20 dark:bg-white/5 overflow-hidden">
                    <div
                      className="h-full rounded-full bg-gradient-to-r from-neon-cyan to-neon-purple transition-all duration-500"
                      style={{ width: `${activeTask.progress_percent}%` }}
                    />
                  </div>

                  <div className="flex items-center justify-between text-[10px] text-text-muted">
                    <span>CLI: {getCliDisplayName(activeTask.cli_name)}</span>
                    <span className="italic">关闭此抽屉不中断分析，您可随时返回</span>
                  </div>
                </div>
              )}

              {/* 可折叠执行日志控制台 */}
              <div className="rounded-2xl border border-card-border overflow-hidden bg-black/10 dark:bg-black/20">
                <button
                  onClick={() => setIsLogsExpanded(!isLogsExpanded)}
                  className="w-full px-4 py-2.5 bg-black/15 dark:bg-white/3 flex items-center justify-between text-xs text-text-secondary font-bold tracking-wide outline-none cursor-pointer border-none"
                >
                  <span className="flex items-center gap-1.5">
                    <Terminal className="w-3.5 h-3.5 text-neon-cyan" />
                    🖥️ CLI 执行控制台日志输出 (共 {logLines.length} 行)
                  </span>
                  {isLogsExpanded ? <ChevronUp className="w-4 h-4 text-text-muted" /> : <ChevronDown className="w-4 h-4 text-text-muted" />}
                </button>

                {isLogsExpanded && (
                  <div
                    ref={logsRef}
                    className="p-4 max-h-48 overflow-y-auto font-mono text-[10px] leading-relaxed text-text-secondary select-text space-y-1.5 border-t border-card-border bg-[#030712] border-none"
                    style={{ maxHeight: '180px' }}
                  >
                    {logLines.map((line, idx) => {
                      let typeColor = 'text-gray-400';
                      if (line.type === 'stage') typeColor = 'text-cyan-400 font-bold';
                      if (line.type === 'error') typeColor = 'text-rose-500 font-bold';
                      if (line.type === 'stderr') typeColor = 'text-amber-500';
                      if (line.type === 'heartbeat') typeColor = 'text-purple-400 italic';
                      if (line.type === 'sys') typeColor = 'text-green-500';

                      return (
                        <div key={idx} className="flex gap-2 text-left">
                          <span className="text-gray-600 flex-shrink-0 select-none">[{line.time}]</span>
                          <span className={`${typeColor} break-all whitespace-pre-wrap`}>{line.text}</span>
                        </div>
                      );
                    })}
                  </div>
                )}
              </div>

              {/* ========================================== */}
              {/* 失败诊断与修复指引卡片 */}
              {/* ========================================== */}
              {activeTask.status === 'failed' && (
                <div 
                  className="p-4 rounded-2xl border text-left space-y-3 relative overflow-hidden backdrop-blur-md"
                  style={{
                    border: '1px solid rgba(244, 63, 94, 0.25)',
                    background: 'linear-gradient(135deg, rgba(244, 63, 94, 0.08) 0%, rgba(244, 63, 94, 0.02) 100%)',
                  }}
                >
                  <div className="flex items-start gap-3">
                    <div className="w-8 h-8 rounded-xl bg-rose-500/10 flex items-center justify-center flex-shrink-0 text-rose-600 dark:text-rose-400">
                      <AlertTriangle className="w-4.5 h-4.5" />
                    </div>
                    <div className="flex-1 space-y-1">
                      <h4 className="text-xs font-bold text-rose-600 dark:text-rose-400">🚨 任务诊断与修复指引</h4>
                      <p className="text-[11px] text-text-secondary leading-relaxed">
                        {activeTask.error_type === 'CLI_NOT_FOUND' && (
                          <>未在系统 PATH 环境变量中检测到 AI 客户端引擎「{getCliDisplayName(activeTask.cli_name)}」。请确保您已全局安装该 CLI 工具。</>
                        )}
                        {activeTask.error_type === 'CLI_NOT_LOGGED_IN' && (
                          <>您的 AI 客户端引擎「{getCliDisplayName(activeTask.cli_name)}」当前处于“未登录”状态，导致分析请求被拒绝。</>
                        )}
                        {activeTask.error_type === 'CLI_PERMISSION_DENIED' && (
                          <>执行权限不足或被系统安全策略拦截。建议以管理员身份运行本程序，或在终端检查相关文件的读写读权限。</>
                        )}
                        {activeTask.error_type === 'CLI_TIMEOUT' && (
                          <>AI 引擎响应超时或本地网络代理阻断。请检查您终端的网络连接或代理状态，并重新运行分析。</>
                        )}
                        {activeTask.error_type === 'CLI_EXECUTION_FAILED' && (
                          <>CLI 分析进程异常退出（Exit Code: {activeTask.exit_code ?? '未知'}）。这通常是由于本地环境配置不兼容或 CLI 参数错误导致。</>
                        )}
                        {!activeTask.error_type && (
                          <>分析任务执行失败。错误消息：{activeTask.error_message || '未知异常崩溃'}</>
                        )}
                      </p>
                    </div>
                  </div>

                  {/* 提供可复制命令 */}
                  {(activeTask.error_type === 'CLI_NOT_FOUND' || activeTask.error_type === 'CLI_NOT_LOGGED_IN') && (
                    <div className="p-2.5 rounded-xl bg-black/30 border border-white/5 flex items-center justify-between gap-3 text-[10px] font-mono select-text">
                      <span className="text-gray-400 truncate flex-1">
                        {activeTask.error_type === 'CLI_NOT_FOUND' && (
                          activeTask.cli_name === 'claude' 
                            ? 'npm install -g @anthropic-ai/claude-code' 
                            : activeTask.cli_name === 'gemini' 
                              ? 'npm install -g @google/gemini-cli' 
                              : 'npm install -g codex-cli'
                        )}
                        {activeTask.error_type === 'CLI_NOT_LOGGED_IN' && (
                          activeTask.cli_name === 'claude' 
                            ? 'claude login' 
                            : activeTask.cli_name === 'gemini' 
                              ? 'gemini login' 
                              : 'codex login'
                        )}
                      </span>
                      <button
                        onClick={async () => {
                          const cmd = activeTask.error_type === 'CLI_NOT_FOUND'
                            ? (activeTask.cli_name === 'claude' ? 'npm install -g @anthropic-ai/claude-code' : 'npm install -g codex-cli')
                            : (activeTask.cli_name === 'claude' ? 'claude login' : 'codex login');
                          await navigator.clipboard.writeText(cmd);
                          alert('修复命令已成功复制到剪贴板！');
                        }}
                        className="px-2 py-1 rounded bg-rose-500/10 hover:bg-rose-500/20 text-[9px] text-rose-600 dark:text-rose-400 border border-rose-500/20 font-bold transition-all cursor-pointer"
                      >
                        复制命令
                      </button>
                    </div>
                  )}

                  {/* 一键重试按钮 */}
                  <div className="flex gap-2 justify-end">
                    <button
                      onClick={() => handleRetryTask(activeTask.id)}
                      className="px-3 py-1.5 rounded-xl border border-rose-500/30 bg-rose-500/10 hover:bg-rose-500/25 text-rose-600 dark:text-rose-400 text-xs font-bold transition-all cursor-pointer flex items-center gap-1.5"
                    >
                      <RefreshCw className="w-3.5 h-3.5" />
                      尝试重新运行
                    </button>
                  </div>
                </div>
              )}

              {/* ========================================== */}
              {/* 可折叠数据快照卡片 */}
              {/* ========================================== */}
              {activeTask.metrics_snapshot_json && (
                <div className="rounded-2xl border border-card-border overflow-hidden bg-black/5 dark:bg-white/2">
                  <button
                    onClick={() => setIsSnapshotExpanded(!isSnapshotExpanded)}
                    className="w-full px-4 py-2.5 bg-black/10 dark:bg-white/3 flex items-center justify-between text-xs text-text-secondary font-bold tracking-wide outline-none cursor-pointer border-none"
                  >
                    <span className="flex items-center gap-1.5">
                      <BarChart2 className="w-3.5 h-3.5 text-neon-cyan" />
                      📊 冻结的分析数据快照 (生成于 {new Date(activeTask.created_at).toLocaleString()})
                    </span>
                    {isSnapshotExpanded ? <ChevronUp className="w-4 h-4 text-text-muted" /> : <ChevronDown className="w-4 h-4 text-text-muted" />}
                  </button>

                  {isSnapshotExpanded && (
                    <div className="p-4 border-t border-card-border bg-black/10 dark:bg-[#0f172a]/20 grid grid-cols-2 sm:grid-cols-4 gap-3 text-left">
                      {(() => {
                        try {
                          const snap = JSON.parse(activeTask.metrics_snapshot_json);
                          const compareSnap = activeTask.compare_metrics_snapshot_json ? JSON.parse(activeTask.compare_metrics_snapshot_json) : null;

                          if (compareSnap) {
                            return (
                              <>
                                <div className="p-2.5 rounded-xl border border-card-border/80 bg-white/1">
                                  <div className="text-[10px] text-text-muted">Token 环比消耗</div>
                                  <div className="flex items-baseline justify-between gap-1">
                                    <span className="text-xs font-bold text-text-primary">
                                      {snap.totalTokens >= 1_000_000 
                                        ? `${(snap.totalTokens / 1_000_000).toFixed(1)}M` 
                                        : snap.totalTokens >= 1_000 
                                          ? `${(snap.totalTokens / 1_000).toFixed(1)}K` 
                                          : snap.totalTokens}
                                    </span>
                                    {(() => {
                                      const prev = compareSnap.totalTokens;
                                      if (!prev) return null;
                                      const pct = ((snap.totalTokens - prev) / prev) * 100;
                                      return <span className={`text-[10px] font-mono font-bold ${pct > 0 ? 'text-rose-400' : 'text-emerald-400'}`}>{pct > 0 ? '+' : ''}{pct.toFixed(1)}%</span>;
                                    })()}
                                  </div>
                                  <div className="text-[9px] text-text-muted mt-0.5 font-mono">上周: {compareSnap.totalTokens.toLocaleString()}</div>
                                </div>
                                <div className="p-2.5 rounded-xl border border-card-border/80 bg-white/1">
                                  <div className="text-[10px] text-text-muted">费用环比 (USD)</div>
                                  <div className="flex items-baseline justify-between gap-1">
                                    <span className="text-xs font-bold text-neon-cyan">${(snap.totalCostUsd ?? 0).toFixed(4)}</span>
                                    {(() => {
                                      const prev = compareSnap.totalCostUsd;
                                      if (!prev) return null;
                                      const pct = ((snap.totalCostUsd - prev) / prev) * 100;
                                      return <span className={`text-[10px] font-mono font-bold ${pct > 0 ? 'text-rose-400' : 'text-emerald-400'}`}>{pct > 0 ? '+' : ''}{pct.toFixed(1)}%</span>;
                                    })()}
                                  </div>
                                  <div className="text-[9px] text-text-muted mt-0.5 font-mono">上周: ${compareSnap.totalCostUsd.toFixed(4)}</div>
                                </div>
                                <div className="p-2.5 rounded-xl border border-card-border/80 bg-white/1">
                                  <div className="text-[10px] text-text-muted">会话环比次数</div>
                                  <div className="flex items-baseline justify-between gap-1">
                                    <span className="text-xs font-bold text-text-primary">{snap.totalSessions} 次</span>
                                    {(() => {
                                      const prev = compareSnap.totalSessions;
                                      if (!prev) return null;
                                      const pct = ((snap.totalSessions - prev) / prev) * 100;
                                      return <span className={`text-[10px] font-mono font-bold ${pct > 0 ? 'text-rose-400' : 'text-emerald-400'}`}>{pct > 0 ? '+' : ''}{pct.toFixed(1)}%</span>;
                                    })()}
                                  </div>
                                  <div className="text-[9px] text-text-muted mt-0.5 font-mono">上周: {compareSnap.totalSessions} 次</div>
                                </div>
                                <div className="p-2.5 rounded-xl border border-card-border/80 bg-white/1">
                                  <div className="text-[10px] text-text-muted">缓存命中率变动</div>
                                  <div className="flex items-baseline justify-between gap-1">
                                    <span className="text-xs font-bold text-emerald-400">{((snap.cacheHitRate ?? 0) * 100).toFixed(1)}%</span>
                                    {(() => {
                                      const prev = compareSnap.cacheHitRate;
                                      const diff = (snap.cacheHitRate - prev) * 100;
                                      return <span className={`text-[10px] font-mono font-bold ${diff > 0 ? 'text-emerald-400' : 'text-rose-400'}`}>{diff > 0 ? '+' : ''}{diff.toFixed(1)}%</span>;
                                    })()}
                                  </div>
                                  <div className="text-[9px] text-text-muted mt-0.5 font-mono">上周: {(compareSnap.cacheHitRate * 100).toFixed(1)}%</div>
                                </div>
                              </>
                            );
                          }

                          return (
                            <>
                              <div className="p-2.5 rounded-xl border border-card-border/80 bg-white/1">
                                <div className="text-[10px] text-text-muted">总 Token 消耗</div>
                                <div className="text-xs font-bold text-text-primary mt-0.5">
                                  {snap.totalTokens >= 1_000_000 
                                    ? `${(snap.totalTokens / 1_000_000).toFixed(1)}M` 
                                    : snap.totalTokens >= 1_000 
                                      ? `${(snap.totalTokens / 1_000).toFixed(1)}K` 
                                      : snap.totalTokens}
                                </div>
                              </div>
                              <div className="p-2.5 rounded-xl border border-card-border/80 bg-white/1">
                                <div className="text-[10px] text-text-muted">消费估算 (USD)</div>
                                <div className="text-xs font-bold text-neon-cyan mt-0.5">${(snap.totalCostUsd ?? 0).toFixed(4)}</div>
                              </div>
                              <div className="p-2.5 rounded-xl border border-card-border/80 bg-white/1">
                                <div className="text-[10px] text-text-muted">总会话数</div>
                                <div className="text-xs font-bold text-text-primary mt-0.5">{snap.totalSessions} 次</div>
                              </div>
                              <div className="p-2.5 rounded-xl border border-card-border/80 bg-white/1">
                                <div className="text-[10px] text-text-muted">缓存命中率</div>
                                <div className="text-xs font-bold text-emerald-400 mt-0.5">{((snap.cacheHitRate ?? 0) * 100).toFixed(1)}%</div>
                              </div>
                            </>
                          );
                        } catch {
                          return <div className="col-span-4 text-xs text-text-muted italic">无法解析的指标快照数据。</div>;
                        }
                      })()}
                    </div>
                  )}
                </div>
              )}

              {/* 核心 Markdown 报告渲染区域 */}
              <div className="space-y-2.5">
                <div className="flex items-center justify-between">
                  <h3 className="text-xs font-bold text-text-secondary uppercase tracking-wider flex items-center gap-1.5">
                    <FileText className="w-4 h-4 text-neon-cyan" />
                    📝 智能复盘分析诊断报告
                  </h3>
                  {outputText && (
                    <button
                      onClick={async () => {
                        try {
                          localStorage.setItem('fullscreen_task_id', activeTask.id);
                          await invoke('open_fullscreen_window', { taskId: activeTask.id });
                        } catch (err) {
                          console.error("无法打开全屏窗口:", err);
                          alert("全屏窗口打开失败: " + err);
                        }
                      }}
                      className="flex items-center gap-1.5 text-[10px] font-bold px-2 py-1 rounded-lg border border-neon-cyan/25 bg-neon-cyan/5 text-neon-cyan hover:bg-neon-cyan/10 cursor-pointer transition-all duration-200"
                    >
                      <Maximize2 className="w-3 h-3" />
                      全屏查看 ↗
                    </button>
                  )}
                </div>

                <div
                  ref={outputRef}
                  className="review-output select-text text-left"
                  style={{
                    background: 'var(--card-bg)',
                    border: '1px solid var(--card-border)',
                    borderRadius: '20px',
                    padding: '24px',
                    minHeight: '260px',
                    maxHeight: 'calc(100vh - 280px)',
                    overflowY: 'auto',
                    fontSize: '13px',
                    lineHeight: '1.8',
                    color: 'var(--text-primary)',
                  }}
                >
                  {outputText ? (
                    <>
                      <Markdown content={outputText} />
                      {(activeTask.status === 'running' || activeTask.status === 'pending') && <StreamingCursor />}
                    </>
                  ) : (
                    <div className="flex flex-col items-center justify-center py-20 gap-3 text-text-muted">
                      <div className="w-8 h-8 rounded-full border-2 border-neon-cyan border-t-transparent animate-spin" />
                      <span className="text-xs italic">
                        正在等待 {getCliDisplayName(activeTask.cli_name)} 引擎分析中...
                      </span>
                    </div>
                  )}
                </div>
              </div>

              {/* ========================================== */}
              {/* 智能行动项 Checklist */}
              {/* ========================================== */}
              {activeTask.status === 'succeeded' && actionItems.length > 0 && (
                <div 
                  className="p-5 rounded-[24px] border text-left space-y-4 shadow-sm"
                  style={{
                    border: '1px solid rgba(8, 145, 178, 0.15)',
                    background: 'linear-gradient(135deg, rgba(8, 145, 178, 0.04) 0%, rgba(124, 58, 237, 0.02) 100%)',
                  }}
                >
                  <div className="flex items-center justify-between">
                    <h3 className="text-xs font-bold text-text-secondary uppercase tracking-wider flex items-center gap-1.5">
                      <CircleDot className="w-4 h-4 text-neon-cyan" />
                      🎯 本周行动计划执行清单 ({actionItems.filter(i => i.checked).length}/{actionItems.length})
                    </h3>
                    <span className="text-[10px] font-mono font-bold px-2 py-0.5 rounded-full bg-neon-cyan/10 text-neon-cyan border border-neon-cyan/20">
                      {Math.round((actionItems.filter(i => i.checked).length / actionItems.length) * 100)}% 完成
                    </span>
                  </div>

                  <div className="space-y-2">
                    {actionItems.map((item) => (
                      <div 
                        key={item.id}
                        onClick={() => handleToggleActionItem(item.id)}
                        className={`flex items-start gap-3 p-3 rounded-xl border transition-all cursor-pointer select-none text-left ${
                          item.checked 
                            ? 'bg-emerald-500/5 border-emerald-500/20 text-emerald-500/70 line-through' 
                            : 'bg-black/10 dark:bg-white/1 border-card-border/80 text-text-primary hover:border-cyan-500/30'
                        }`}
                      >
                        <input
                          type="checkbox"
                          checked={item.checked}
                          readOnly
                          className="mt-0.5 w-3.5 h-3.5 rounded border-card-border accent-cyan-500 cursor-pointer flex-shrink-0"
                        />
                        <span className="text-xs leading-relaxed font-medium">{item.text}</span>
                      </div>
                    ))}
                  </div>
                </div>
              )}



              {/* 动作区 */}
              <div className="pt-2 flex gap-3">
                {(activeTask.status === 'running' || activeTask.status === 'pending') ? (
                  <>
                    <button
                      onClick={() => setView('new')}
                      className="flex-1 py-3 rounded-2xl bg-neon-cyan/10 border border-neon-cyan/30 text-neon-cyan hover:bg-neon-cyan/20 text-xs font-bold transition-all active:scale-[0.98] cursor-pointer"
                    >
                      📊 返回新建复盘
                    </button>
                    <button
                      onClick={handleCancelTask}
                      className="flex-1 py-3 rounded-2xl bg-rose-500/10 border border-rose-500/30 text-rose-600 dark:text-rose-400 hover:bg-rose-500/20 text-xs font-bold transition-all active:scale-[0.98] cursor-pointer"
                    >
                      🛑 终止并取消当前分析
                    </button>
                  </>
                ) : (
                  <button
                    onClick={() => {
                      setView('new');
                      setSelectedCli(activeTask.cli_name);
                      setReviewTimeRange(activeTask.time_range);
                      try {
                        const parsedIdes = JSON.parse(activeTask.selected_ides_json);
                        setSelectedIdes(parsedIdes);
                      } catch {
                        /* ignore */
                      }
                      setCustomPrompt(activeTask.prompt_text);
                    }}
                    className="w-full py-3 rounded-2xl bg-gradient-to-r from-cyan-500 to-purple-500 text-xs font-bold text-white transition-all active:scale-[0.98] cursor-pointer shadow-md"
                  >
                    🔄 用原参数重新开启分析
                  </button>
                )}
              </div>
            </div>
          )}
        </div>
      </div>
    );
}
