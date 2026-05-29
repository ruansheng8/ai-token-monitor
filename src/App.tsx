import { useState, useEffect, useMemo, useRef, lazy, Suspense } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { apiUrl, readJsonResponse } from './lib/api';
import { ReviewDrawer } from './components/ReviewDrawer';
import {
  Cpu,
  ArrowDown,
  ArrowUp,
  Database,
  Brain,
  Hash,
  RefreshCw,
  Search,
  MessageSquare,
  ChevronsUpDown,
  Compass,
  Sun,
  Moon,
  ChevronLeft,
  ChevronRight,
  Globe,
  ChevronDown,
  Settings,
  Terminal,
  Monitor,
  Sparkles
} from 'lucide-react';
const DailyTrendChart = lazy(() => import('./components/charts/DailyTrendChart').then((module) => ({ default: module.DailyTrendChart })));
const ProjectTrendChart = lazy(() => import('./components/charts/ProjectTrendChart').then((module) => ({ default: module.ProjectTrendChart })));
const SourceTrendChart = lazy(() => import('./components/charts/SourceTrendChart').then((module) => ({ default: module.SourceTrendChart })));
const PerformanceChart = lazy(() => import('./components/charts/PerformanceChart').then((module) => ({ default: module.PerformanceChart })));
const CalendarHeatmap = lazy(() => import('./components/charts/CalendarHeatmap').then((module) => ({ default: module.CalendarHeatmap })));

const ChartFallback = ({ label = '正在加载图表...' }: { label?: string }) => (
  <div className="h-[300px] flex items-center justify-center text-text-muted text-xs italic">
    {label}
  </div>
);

// 官方 SVG 图标组件
const GeminiIcon = ({ className = "w-4 h-4" }: { className?: string }) => (
  <svg className={className} fill="currentColor" fillRule="evenodd" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
    <path d="M20.616 10.835a14.147 14.147 0 01-4.45-3.001 14.111 14.111 0 01-3.678-6.452.503.503 0 00-.975 0 14.134 14.134 0 01-3.679 6.452 14.155 14.155 0 01-4.45 3.001c-.65.28-1.318.505-2.002.678a.502.502 0 000 .975c.684.172 1.35.397 2.002.677a14.147 14.147 0 014.45 3.001 14.112 14.112 0 013.679 6.453.502.502 0 00.975 0c.172-.685.397-1.351.677-2.003a14.145 14.145 0 013.001-4.45 14.113 14.113 0 016.453-3.678.503.503 0 000-.975 13.245 13.245 0 01-2.003-.678z"></path>
  </svg>
);

const ClaudeIcon = ({ className = "w-4 h-4" }: { className?: string }) => (
  <svg className={className} fill="currentColor" fillRule="evenodd" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
    <path d="M4.709 15.955l4.72-2.647.08-.23-.08-.128H9.2l-.79-.048-2.698-.073-2.339-.097-2.266-.122-.571-.121L0 11.784l.055-.352.48-.321.686.06 1.52.103 2.278.158 1.652.097 2.449.255h.389l.055-.157-.134-.098-.103-.097-2.358-1.596-2.552-1.688-1.336-.972-.724-.491-.364-.462-.158-1.008.656-.722.881.06.225.061.893.686 1.908 1.476 2.491 1.833.365.304.145-.103.019-.073-.164-.274-1.355-2.446-1.446-2.49-.644-1.032-.17-.619a2.97 2.97 0 01-.104-.729L6.283.134 6.696 0l.996.134.42.364.62 1.414 1.002 2.229 1.555 3.03.456.898.243.832.091.255h.158V9.01l.128-1.706.237-2.095.23-2.695.08-.76.376-.91.747-.492.584.28.48.685-.067.444-.286 1.851-.559 2.903-.364 1.942h.212l.243-.242.985-1.306 1.652-2.064.73-.82.85-.904.547-.431h1.033l.76 1.129-.34 1.166-1.064 1.347-.881 1.142-1.264 1.7-.79 1.36.073.11.188-.02 2.856-.606 1.543-.28 1.841-.315.833.388.091.395-.328.807-1.969.486-2.309.462-3.439.813-.042.03.049.061 1.549.146.662.036h1.622l3.02.225.79.522.474.638-.079.485-1.215.62-1.64-.389-3.829-.91-1.312-.329h-.182v.11l1.093 1.068 2.006 1.81 2.509 2.33.127.578-.322.455-.34-.049-2.205-1.657-.851-.747-1.926-1.62h-.128v.17l.444.649 2.345 3.521.122 1.08-.17.353-.608.213-.668-.122-1.374-1.925-1.415-2.167-1.143-1.943-.14.08-.674 7.254-.316.37-.729.28-.607-.461-.322-.747.322-1.476.389-1.924.315-1.53.286-1.9.17-.632-.012-.042-.14.018-1.434 1.967-2.18 2.945-1.726 1.845-.414.164-.717-.37.067-.662.401-.589 2.388-3.036 1.44-1.882.93-1.086-.006-.158h-.055L4.132 18.56l-1.13.146-.487-.456.061-.746.231-.243 1.908-1.312-.006.006z"></path>
  </svg>
);

const OpenAIIcon = ({ className = "w-4 h-4" }: { className?: string }) => (
  <svg className={className} fill="currentColor" fillRule="evenodd" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
    <path d="M9.205 8.658v-2.26c0-.19.072-.333.238-.428l4.543-2.616c.619-.357 1.356-.523 2.117-.523 2.854 0 4.662 2.212 4.662 4.566 0 .167 0 .357-.024.547l-4.71-2.759a.797.797 0 00-.856 0l-5.97 3.473zm10.609 8.8V12.06c0-.333-.143-.57-.429-.737l-5.97-3.473 1.95-1.118a.433.433 0 01.476 0l4.543 2.617c1.309.76 2.189 2.378 2.189 3.948 0 1.808-1.07 3.473-2.76 4.163zM7.802 12.703l-1.95-1.142c-.167-.095-.239-.238-.239-.428V5.899c0-2.545 1.95-4.472 4.591-4.472 1 0 1.927.333 2.712.928L8.23 5.067c-.285.166-.428.404-.428.737v6.898zM12 15.128l-2.795-1.57v-3.33L12 8.658l2.795 1.57v3.33L12 15.128zm1.796 7.23c-1 0-1.927-.332-2.712-.927l4.686-2.712c.285-.166.428-.404.428-.737v-6.898l1.974 1.142c.167.095.238.238.238.428v5.233c0 2.545-1.974 4.472-4.614 4.472zm-5.637-5.303l-4.544-2.617c-1.308-.761-2.188-2.378-2.188-3.948A4.482 4.482 0 014.21 6.327v5.423c0 .333.143.571.428.738l5.947 3.449-1.95 1.118a.432.432 0 01-.476 0zm-.262 3.9c-2.688 0-4.662-2.021-4.662-4.519 0-.19.024-.38.047-.57l4.686 2.71c.286.167.571.167.856 0l5.97-3.448v2.26c0 .19-.07.333-.237.428l-4.543 2.616c-.619.357-1.356.523-2.117.523zm5.899 2.83a5.947 5.947 0 005.827-4.756C22.287 18.339 24 15.84 24 13.296c0-1.665-.713-3.282-1.998-4.448.119-.5.19-.999.19-1.498 0-3.401-2.759-5.947-5.946-5.947-.642 0-1.26.095-1.88.31A5.962 5.962 0 0010.205 0a5.947 5.947 0 00-5.827 4.757C1.713 5.447 0 7.945 0 10.49c0 1.666.713 3.283 1.998 4.448-.119.5-.19 1-.19 1.499 0 3.401 2.759 5.946 5.946 5.946.642 0 1.26-.095 1.88-.309a5.96 5.96 0 004.162 1.713z"></path>
  </svg>
);

const CursorIcon = ({ className = "w-4 h-4" }: { className?: string }) => (
  <svg className={className} fill="currentColor" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
    <path d="M10.3 22.8a1 1 0 01-.8-.4l-3.3-5.7-3.8 3.7c-.5.4-1.2.3-1.4-.2A1 1 0 011 20V2a1 1 0 011.7-.7l13.6 13.6c.4.4.4 1 0 1.4l-4.2 1.3 3.3 5.7c.2.5 0 1.1-.4 1.3l-3.8 1c-.3.2-.6.3-.9.3z" />
  </svg>
);

const TraeIcon = ({ className = "w-4 h-4" }: { className?: string }) => (
  <svg className={className} fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" xmlns="http://www.w3.org/2000/svg">
    <path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z" />
    <polyline points="3.27 6.96 12 12.01 20.73 6.96" />
    <line x1="12" y1="22.08" x2="12" y2="12" />
  </svg>
);

// 类型定义
interface Totals {
  total_input: number;
  total_output: number;
  total_tokens: number;
  total_cached: number;
  total_thinking: number;
  cache_hit_rate: number;
  thinking_ratio: number;
  total_sessions: number;
  total_cost: number;
}

interface DailyTrend {
  date: string;
  input: number;
  output: number;
  cached: number;
  thinking: number;
  sessions: number;
}

interface MonthlySummary {
  month: string;
  input: number;
  output: number;
  cached: number;
  thinking: number;
  sessions: number;
}

interface ModelDistribution {
  model: string;
  input: number;
  output: number;
  cached: number;
  thinking: number;
  total_tokens: number;
}

interface SessionItem {
  source: string;
  uuid: string;
  title: string;
  created_at: string;
  input: number;
  output: number;
  cached: number;
  thinking: number;
  cost_usd: number;
  models: string[];
}

interface SourceTrendItem {
  date: string;
  source: string;
  tokens: number;
  cost: number;
}

interface DeviceTrendItem {
  date: string;
  device_name: string;
  tokens: number;
  cost: number;
}

interface ModelPerformance {
  model: string;
  avg_latency: number;
  avg_tps: number;
  sample_count: number;
}

interface PerformanceTrend {
  date: string;
  avg_latency: number;
  avg_tps: number;
}

interface ProjectTrendItem {
  date: string;
  project_name: string;
  tokens: number;
  cost_usd: number;
}

interface ProjectRankingItem {
  project_name: string;
  project_path: string;
  total_tokens: number;
  total_cost_usd: number;
  sessions_count: number;
}

interface ModelPricingRow {
  id?: number;
  model_pattern: string;
  input_price_per_million: number;
  cached_input_price_per_million: number;
  output_price_per_million: number;
  priority: number;
  enabled: boolean;
  updated_at: string;
}

interface AggregatedMetrics {
  totals: Totals;
  daily_trends: DailyTrend[];
  monthly_summary: MonthlySummary[];
  model_distribution: ModelDistribution[];
  sessions: SessionItem[];
  source_trends: SourceTrendItem[];
  device_trends: DeviceTrendItem[];
  model_performance: ModelPerformance[];
  performance_trends: PerformanceTrend[];
  project_trends: ProjectTrendItem[];
  project_rankings: ProjectRankingItem[];
  display_currency: string;
  usd_exchange_rate: number;
  exchange_rate_updated_at: string;
}

// 获取指定时间区间的起止日期（格式：YYYY-MM-DD）
const getDateBounds = (range: 'all' | 'today' | 'week' | '30days' | 'month' | 'quarter' | 'custom') => {
  const now = new Date();
  const formatDateStr = (d: Date) => {
    const y = d.getFullYear();
    const m = String(d.getMonth() + 1).padStart(2, '0');
    const day = String(d.getDate()).padStart(2, '0');
    return `${y}-${m}-${day}`;
  };

  switch (range) {
    case 'today': {
      const dStr = formatDateStr(now);
      return { start: dStr, end: dStr };
    }
    case 'week': {
      const past = new Date(now.getTime() - 6 * 24 * 60 * 60 * 1000);
      return { start: formatDateStr(past), end: formatDateStr(now) };
    }
    case '30days': {
      const past = new Date(now.getTime() - 29 * 24 * 60 * 60 * 1000);
      return { start: formatDateStr(past), end: formatDateStr(now) };
    }
    case 'month': {
      const firstDay = new Date(now.getFullYear(), now.getMonth(), 1);
      return { start: formatDateStr(firstDay), end: formatDateStr(now) };
    }
    case 'quarter': {
      const currentMonth = now.getMonth();
      const quarterStartMonth = Math.floor(currentMonth / 3) * 3;
      const firstDay = new Date(now.getFullYear(), quarterStartMonth, 1);
      return { start: formatDateStr(firstDay), end: formatDateStr(now) };
    }
    default:
      return { start: '', end: '' };
  }
};

const exportToCSV = (sessions: SessionItem[]) => {
  if (!sessions || sessions.length === 0) return;
  
  const headers = ['工具/引擎', '会话 UUID', '会话标题', '创建时间', '输入 Tokens', '输出 Tokens', '缓存 Tokens', '推理 Tokens', '产生费用 (USD)', '使用模型'];
  const csvRows = [headers.join(',')];
  
  sessions.forEach(s => {
    const row = [
      s.source,
      s.uuid,
      `"${s.title.replace(/"/g, '""')}"`,
      s.created_at,
      s.input,
      s.output,
      s.cached,
      s.thinking,
      s.cost_usd.toFixed(6),
      `"${s.models.join('; ')}"`
    ];
    csvRows.push(row.join(','));
  });
  
  const csvContent = '\uFEFF' + csvRows.join('\n');
  const blob = new Blob([csvContent], { type: 'text/csv;charset=utf-8;' });
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.setAttribute('href', url);
  link.setAttribute('download', `AI_Token_Monitor_Report_${new Date().toISOString().slice(0, 10)}.csv`);
  link.style.visibility = 'hidden';
  document.body.appendChild(link);
  link.click();
  document.body.removeChild(link);
};

export default function App() {
  const [data, setData] = useState<AggregatedMetrics | null>(null);

  // 复盘与建议抽屉状态
  const [isReviewOpen, setIsReviewOpen] = useState(false);

  // 标签页切换状态
  const [activeTab, setActiveTab] = useState<'source' | 'pricing' | 'optimize'>('source');

  // 多币种展示状态
  const [displayCurrency, setDisplayCurrency] = useState('USD');
  const [usdExchangeRate, setUsdExchangeRate] = useState(1.0);
  const [exchangeRateUpdatedAt, setExchangeRateUpdatedAt] = useState('');

  // 费率列表状态
  const [modelPricingRows, setModelPricingRows] = useState<ModelPricingRow[]>([]);
  const [pricingLoading, setPricingLoading] = useState(false);
  const [pricingMessage, setPricingMessage] = useState<{ success: boolean; text: string } | null>(null);
  const [sessionsData, setSessionsData] = useState<{ items: SessionItem[]; total: number } | null>(null);
  const [sessionsLoading, setSessionsLoading] = useState(false);
  const [loading, setLoading] = useState(false);
  const [refreshSpin, setRefreshSpin] = useState(false);

  // 应用初始化与后台握手状态
  const [appInitializing, setAppInitializing] = useState(true);
  const [initError, setInitError] = useState<string | null>(null);

  // 初始化步骤进度状态
  type InitStepStatus = 'pending' | 'loading' | 'done' | 'error';
  const [initSteps, setInitSteps] = useState<{
    checkConfig: InitStepStatus;
    startScan:   InitStepStatus;
    loadMetrics: InitStepStatus;
    loadSessions: InitStepStatus;
  }>({
    checkConfig:  'pending',
    startScan:    'pending',
    loadMetrics:  'pending',
    loadSessions: 'pending',
  });
  const [initElapsed, setInitElapsed] = useState(0);
  const initElapsedRef = useRef<any>(null);

  // 大盘慢查询友好提示与 AbortController 取消机制
  const abortControllerRef = useRef<AbortController | null>(null);
  const loadingTimeoutRef = useRef<any>(null);
  const [showDelayedLoading, setShowDelayedLoading] = useState(false);
  const [lastUpdate, setLastUpdate] = useState('--:--:--');
  const [searchKeyword, setSearchKeyword] = useState('');

  // 离线容灾网络状态监测
  const [isOnline, setIsOnline] = useState(navigator.onLine);

  useEffect(() => {
    const handleOnline = () => setIsOnline(true);
    const handleOffline = () => setIsOnline(false);

    window.addEventListener('online', handleOnline);
    window.addEventListener('offline', handleOffline);

    return () => {
      window.removeEventListener('online', handleOnline);
      window.removeEventListener('offline', handleOffline);
    };
  }, []);
  const [hideZero, setHideZero] = useState(true);
  const [sortField, setSortField] = useState<keyof SessionItem | 'total'>('created_at');
  const [sortOrder, setSortOrder] = useState<'asc' | 'desc'>('desc');
  const [currentPage, setCurrentPage] = useState(1);
  const [pageSize, setPageSize] = useState(5);
  const [source, setSource] = useState<'all' | 'antigravity' | 'claude_code' | 'codex' | 'cursor' | 'trae' | 'trae_cn'>('all');
  const [isSourceDropdownOpen, setIsSourceDropdownOpen] = useState(false);
  const [timeRange, setTimeRange] = useState<'all' | 'today' | 'week' | '30days' | 'month' | 'quarter' | 'custom'>('30days');
  const [startDate, setStartDate] = useState<string>(getDateBounds('30days').start);
  const [endDate, setEndDate] = useState<string>(getDateBounds('30days').end);
  const [chartDimension, setChartDimension] = useState<'type' | 'source' | 'device'>('type');

  // 数据库数据源配置状态
  const [isConfigOpen, setIsConfigOpen] = useState(false);
  const [dbType, setDbType] = useState<'sqlite' | 'postgres'>('sqlite');
  const [sqlitePath, setSqlitePath] = useState('');
  const [pgHost, setPgHost] = useState('');
  const [pgPort, setPgPort] = useState('5432');
  const [pgUser, setPgUser] = useState('');
  const [pgPassword, setPgPassword] = useState('');
  const [pgDatabase, setPgDatabase] = useState('');
  const [showPassword, setShowPassword] = useState(false);
  const [testLoading, setTestLoading] = useState(false);
  const [saveLoading, setSaveLoading] = useState(false);
  const [configMessage, setConfigMessage] = useState<{ success: boolean; text: string } | null>(null);
  
  // 窗口关闭行为配置状态
  const [closeBehavior, setCloseBehavior] = useState<'prompt' | 'close' | 'minimize'>('prompt');
  const [showCloseConfirmModal, setShowCloseConfirmModal] = useState(false);
  const [dontPromptAgain, setDontPromptAgain] = useState(true);

  // 设备名称配置状态
  const [showDeviceModal, setShowDeviceModal] = useState(false);
  const [deviceName, setDeviceName] = useState('');
  const [defaultDeviceName, setDefaultDeviceName] = useState('');

  // 数据库清理与优化瘦身状态
  const [cleanLoading, setCleanLoading] = useState(false);
  const [cleanMessage, setCleanMessage] = useState<string | null>(null);

  // 财务报表生成与预览状态
  const [isGeneratingReport, _setIsGeneratingReport] = useState(false);
  const [isReportModalOpen, setIsReportModalOpen] = useState(false);
  const [reportImgUrl, _setReportImgUrl] = useState('');

  /*
  const _generateReportImage = async () => {
    setIsGeneratingReport(true);
    try {
      const html2canvas = (await import('html2canvas')).default;
      const element = document.getElementById('report-container');
      if (!element) {
        throw new Error('未找到报表容器 #report-container');
      }

      const canvas = await html2canvas(element, {
        useCORS: true,
        allowTaint: false, // 必须为 false！否则 Canvas 会被标记为受污染，导致 toDataURL() 抛出 SecurityError
        backgroundColor: theme === 'dark' ? '#030712' : '#f8fafc',
        scale: 1.5, // 采用 1.5 倍缩放，兼顾高清重绘的同时，降低大面积绘图的内存压力
        logging: false,
        ignoreElements: (el) => {
          return el.classList.contains('no-print');
        }
      });
      const dataUrl = canvas.toDataURL('image/png');
      setReportImgUrl(dataUrl);
      setIsReportModalOpen(true);
    } catch (error: any) {
      console.error('Failed to generate report image:', error);
      alert('生成财务报表图片失败，原因: ' + (error instanceof Error ? error.message : String(error)));
    } finally {
      setIsGeneratingReport(false);
    }
  };
  */

  const handleDbClean = async () => {
    setCleanLoading(true);
    setCleanMessage(null);
    try {
      const res = await fetch(apiUrl('/db/clean'), { method: 'POST' });
      const resData = await readJsonResponse<any>(res);
      if (resData.success) {
        setCleanMessage(`✅ ${resData.message}`);
        // 静默重新拉取大盘最新数据以刷新会话数和数据
        fetchData();
      } else {
        setCleanMessage(`❌ 数据库优化失败: ${resData.message}`);
      }
    } catch (e: any) {
      setCleanMessage(`❌ 接口通信异常: ${e.message || e}`);
    } finally {
      setCleanLoading(false);
    }
  };

  // 当配置弹窗打开时，拉取后端最新配置并回显
  useEffect(() => {
    if (isConfigOpen) {
      const loadConfig = async () => {
        try {
          const res = await fetch(apiUrl(`/config?t=${Date.now()}`));
          if (res.ok) {
            const data = await readJsonResponse<any>(res);
            if (data.db_type) {
              setDbType(data.db_type.toLowerCase() === 'postgres' ? 'postgres' : 'sqlite');
            }
            if (data.sqlite_path) {
              setSqlitePath(data.sqlite_path);
            }
            setPgHost(data.pg_host || '');
            setPgPort(data.pg_port || '5432');
            setPgUser(data.pg_user || '');
            setPgPassword(data.pg_password || '');
            setPgDatabase(data.pg_database || '');
            // 加载设备名配置
            setDeviceName(data.device_name || '');
            setDefaultDeviceName(data.default_device_name || '');
            // 加载窗口关闭行为配置
            if (data.close_behavior) {
              setCloseBehavior(data.close_behavior as 'prompt' | 'close' | 'minimize');
            }
          }
        } catch (e) {
          console.error("加载数据源配置失败", e);
        }
      };

      const loadPricing = async () => {
        setPricingLoading(true);
        try {
          const res = await fetch(apiUrl(`/model-pricing?t=${Date.now()}`));
          if (res.ok) {
            const pricingData = await readJsonResponse<any>(res);
            if (pricingData.rows) {
              setModelPricingRows(pricingData.rows);
            }
            if (pricingData.display_currency) {
              setDisplayCurrency(pricingData.display_currency);
            }
          }
        } catch (e) {
          console.error("加载模型费率配置失败", e);
        } finally {
          setPricingLoading(false);
        }
      };

      loadConfig();
      loadPricing();
      setActiveTab('source');
    }
  }, [isConfigOpen]);

  useEffect(() => {
    if (timeRange !== 'custom') {
      const bounds = getDateBounds(timeRange);
      setStartDate(bounds.start);
      setEndDate(bounds.end);
    }
  }, [timeRange]);

  const [theme, setTheme] = useState<'light' | 'dark'>(() => {
    const saved = localStorage.getItem('theme');
    if (saved === 'dark' || saved === 'light') {
      return saved;
    }
    return 'light';
  });

  useEffect(() => {
    const root = document.documentElement;
    if (theme === 'dark') {
      root.classList.add('dark');
    } else {
      root.classList.remove('dark');
    }
    localStorage.setItem('theme', theme);
  }, [theme]);

  // 当过滤条件、排序字段、排序顺序或每页大小发生变化时，重置当前页码
  useEffect(() => {
    setCurrentPage(1);
  }, [searchKeyword, hideZero, sortField, sortOrder, pageSize, source]);

  // 格式化货币，支持多币种和汇率换算
  const formatCurrency = (valUsd: number) => {
    const rate = usdExchangeRate || 1.0;
    const val = valUsd * rate;
    let symbol = '$';
    switch (displayCurrency) {
      case 'CNY':
        symbol = '￥';
        break;
      case 'JPY':
        symbol = '¥';
        break;
      case 'EUR':
        symbol = '€';
        break;
      default:
        symbol = '$';
    }
    
    if (val === 0) return `${symbol}0.00`;
    
    if (val < 0.01) {
      return `${symbol}${val.toFixed(6)}`;
    } else if (val < 1) {
      return `${symbol}${val.toFixed(4)}`;
    } else {
      return `${symbol}${val.toLocaleString('zh-CN', { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`;
    }
  };

  // 精确数字格式化（带千分位）
  const formatPreciseNum = (num: number) => new Intl.NumberFormat('zh-CN').format(num || 0);

  // 数字格式化，支持中文大数单位（万、亿），保留最多1位有效小数
  const formatNum = (num: number) => {
    if (num === 0) return '0';
    if (!num) return '0';
    const absNum = Math.abs(num);
    let unit = '';
    let formatted = absNum;

    if (absNum >= 1e8) {
      formatted = absNum / 1e8;
      unit = '亿';
    } else if (absNum >= 1e4) {
      formatted = absNum / 1e4;
      unit = '万';
    }

    if (unit) {
      const trimmed = parseFloat(formatted.toFixed(1)).toString();
      return (num < 0 ? '-' : '') + trimmed + unit;
    }

    return (num < 0 ? '-' : '') + new Intl.NumberFormat('zh-CN').format(absNum);
  };

  // 百分比格式化
  const formatPercent = (val: number) => (val * 100).toFixed(1) + '%';

  // 日期格式化
  const formatDate = (isoStr: string) => {
    if (!isoStr) return '--';
    try {
      const date = new Date(isoStr);
      if (isNaN(date.getTime())) return isoStr;
      const y = date.getFullYear();
      const m = String(date.getMonth() + 1).padStart(2, '0');
      const d = String(date.getDate()).padStart(2, '0');
      const hh = String(date.getHours()).padStart(2, '0');
      const mm = String(date.getMinutes()).padStart(2, '0');
      return `${y}-${m}-${d} ${hh}:${mm}`;
    } catch (e) {
      return isoStr;
    }
  };

  const [scanStatus, setScanStatus] = useState<{
    is_scanning: boolean;
    total_files: number;
    scanned_files: number;
    error: string | null;
    logs?: string[];
    status_msg?: string;
  } | null>(null);

  const [showLogConsole, setShowLogConsole] = useState(false);
  const logEndRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (showLogConsole && logEndRef.current) {
      logEndRef.current.scrollIntoView({ behavior: 'smooth' });
    }
  }, [scanStatus?.logs, showLogConsole]);

  // 轮询扫描状态
  const pollScanStatus = async () => {
    try {
      const response = await fetch(apiUrl(`/scan/status?t=${Date.now()}`));
      if (response.ok) {
        const status = await readJsonResponse<NonNullable<typeof scanStatus>>(response);
        setScanStatus(status);
        if (status.is_scanning) {
          setTimeout(pollScanStatus, 1000);
        } else {
          // 扫描完成，重新拉取最新数据
          fetchData(source, startDate, endDate);
        }
      }
    } catch (error) {
      console.error('Failed to poll scan status:', error);
    }
  };

  // 开始扫描并触发轮询
  const startScan = async () => {
    try {
      const response = await fetch(apiUrl(`/scan/start?t=${Date.now()}`));
      if (response.ok) {
        const status = await readJsonResponse<NonNullable<typeof scanStatus>>(response);
        setScanStatus(status);
        if (status.is_scanning) {
          pollScanStatus();
        }
      }
    } catch (error) {
      console.error('Failed to start scan:', error);
    }
  };

  // 获取数据逻辑
  const fetchData = async (currentSource = source, start = startDate, end = endDate) => {
    if (appInitializing) return;
    // 1. 发起新查询前，如果先前有未完成的查询，则主动取消，保证网络竞态安全性
    if (abortControllerRef.current) {
      abortControllerRef.current.abort();
    }

    // 2. 重置并开启 300ms 防闪烁延时定时器
    if (loadingTimeoutRef.current) {
      clearTimeout(loadingTimeoutRef.current);
    }
    setShowDelayedLoading(false);

    loadingTimeoutRef.current = setTimeout(() => {
      setShowDelayedLoading(true);
    }, 300);

    // 3. 实例化当前查询专用控制器
    const controller = new AbortController();
    abortControllerRef.current = controller;

    setLoading(true);
    setRefreshSpin(true);

    try {
      const response = await fetch(
        apiUrl(`/metrics?source=${currentSource}&start_date=${start}&end_date=${end}&t=${Date.now()}`),
        { signal: controller.signal }
      );
      if (!response.ok) throw new Error(`HTTP error! status: ${response.status}`);
      const result: AggregatedMetrics = await readJsonResponse(response);
      setData(result);
      if (result.display_currency) {
        setDisplayCurrency(result.display_currency);
      }
      if (result.usd_exchange_rate !== undefined) {
        setUsdExchangeRate(result.usd_exchange_rate);
      }
      if (result.exchange_rate_updated_at) {
        setExchangeRateUpdatedAt(result.exchange_rate_updated_at);
      }
      const now = new Date();
      setLastUpdate(now.toTimeString().split(' ')[0]);
    } catch (error: any) {
      if (error.name === 'AbortError') {
        console.log('Query aborted successfully by user/system.');
      } else {
        console.error('Fetch data failed:', error);
      }
    } finally {
      // 4. 清除并释放定时器句柄
      if (loadingTimeoutRef.current) {
        clearTimeout(loadingTimeoutRef.current);
        loadingTimeoutRef.current = null;
      }

      // 5. 仅当本次 controller 仍然是最新的网络请求控制器时，才重置 Loading 相关的视觉状态
      if (abortControllerRef.current === controller) {
        setLoading(false);
        setRefreshSpin(false);
        setShowDelayedLoading(false);
        abortControllerRef.current = null;
      }
    }
  };

  // 取消查询处理器
  const handleCancelQuery = () => {
    if (abortControllerRef.current) {
      abortControllerRef.current.abort();
      abortControllerRef.current = null;
    }
    if (loadingTimeoutRef.current) {
      clearTimeout(loadingTimeoutRef.current);
      loadingTimeoutRef.current = null;
    }
    setShowDelayedLoading(false);
    setLoading(false);
    setRefreshSpin(false);
  };

  // 异步拉取真实会话分页与关键字检索数据 (方案一核心)
  const fetchSessions = async (
    page = currentPage,
    size = pageSize,
    search = searchKeyword,
    src = source,
    sortBy = sortField,
    order = sortOrder,
    start = startDate,
    end = endDate,
    hideZeroVal = hideZero
  ) => {
    if (appInitializing) return;
    setSessionsLoading(true);
    try {
      const query = new URLSearchParams({
        page: String(page),
        page_size: String(size),
        search: search.trim(),
        source: src,
        sort_by: String(sortBy),
        sort_order: order,
        start_date: start,
        end_date: end,
        hide_zero: hideZeroVal ? 'true' : 'false',
        t: String(Date.now())
      });
      const res = await fetch(apiUrl(`/sessions?${query.toString()}`));
      if (res.ok) {
        const result = await readJsonResponse<{ items: SessionItem[]; total: number }>(res);
        setSessionsData(result);
      }
    } catch (e) {
      console.error("加载会话分页数据失败", e);
    } finally {
      setSessionsLoading(false);
    }
  };

  // 首次启动后台数据初始化同步与握手
  const performInitialSync = async (skipDeviceCheck = false) => {
    setInitError(null);
    setAppInitializing(true);
    setInitElapsed(0);
    setInitSteps({ checkConfig: 'pending', startScan: 'pending', loadMetrics: 'pending', loadSessions: 'pending' });

    // 启动计时器
    if (initElapsedRef.current) clearInterval(initElapsedRef.current);
    const startTime = Date.now();
    initElapsedRef.current = setInterval(() => {
      setInitElapsed(Math.floor((Date.now() - startTime) / 1000));
    }, 1000);

    try {
      // Step 1. 检查设备名称是否配置
      setInitSteps(s => ({ ...s, checkConfig: 'loading' }));
      if (!skipDeviceCheck) {
        const configRes = await fetch(apiUrl(`/config?t=${Date.now()}`));
        if (configRes.ok) {
          const configData = await readJsonResponse<any>(configRes);
          if (configData.close_behavior) {
            setCloseBehavior(configData.close_behavior as 'prompt' | 'close' | 'minimize');
          }
          // 如果没有配置设备名 (即为 null 或空字符串)
          if (!configData.device_name) {
            setDefaultDeviceName(configData.default_device_name || '');
            setDeviceName(configData.default_device_name || '');
            // 填充已有的数据库配置，以便保存时不会丢失其他配置
            if (configData.db_type) {
              setDbType(configData.db_type.toLowerCase() === 'postgres' ? 'postgres' : 'sqlite');
            }
            if (configData.sqlite_path) {
              setSqlitePath(configData.sqlite_path);
            }
            setPgHost(configData.pg_host || '');
            setPgPort(configData.pg_port || '5432');
            setPgUser(configData.pg_user || '');
            setPgPassword(configData.pg_password || '');
            setPgDatabase(configData.pg_database || '');

            setShowDeviceModal(true);
            setAppInitializing(false);
            clearInterval(initElapsedRef.current);
            return; // 拦截，等配置完再继续
          }
        }
      }
      setInitSteps(s => ({ ...s, checkConfig: 'done' }));

      // Step 2-4. 并行启动后端扫描 + 拉取大盘指标 + 拉取会话列表
      setInitSteps(s => ({ ...s, startScan: 'loading', loadMetrics: 'loading', loadSessions: 'loading' }));

      const scanPromise = fetch(apiUrl(`/scan/start?t=${Date.now()}`));
      const metricsPromise = fetch(apiUrl(`/metrics?source=${source}&start_date=${startDate}&end_date=${endDate}&t=${Date.now()}`));
      const query = new URLSearchParams({
        page: '1',
        page_size: String(pageSize),
        search: searchKeyword.trim(),
        source,
        sort_by: String(sortField),
        sort_order: sortOrder,
        start_date: startDate,
        end_date: endDate,
        hide_zero: hideZero ? 'true' : 'false',
        t: String(Date.now())
      });
      const sessionsPromise = fetch(apiUrl(`/sessions?${query.toString()}`));

      // 用 Promise 包装，单独跟踪每个请求的完成状态
      const [scanRes, metricsRes, sessionsRes] = await Promise.all([
        scanPromise.then(r  => { setInitSteps(s => ({ ...s, startScan: r.ok ? 'done' : 'error' })); return r; }),
        metricsPromise.then(r => { setInitSteps(s => ({ ...s, loadMetrics: r.ok ? 'done' : 'error' })); return r; }),
        sessionsPromise.then(r => { setInitSteps(s => ({ ...s, loadSessions: r.ok ? 'done' : 'error' })); return r; }),
      ]);

      if (!scanRes.ok || !metricsRes.ok || !sessionsRes.ok) {
        throw new Error(`后台服务连接异常 (Scan: ${scanRes.status}, Metrics: ${metricsRes.status}, Sessions: ${sessionsRes.status})`);
      }

      const scanStatusVal = await readJsonResponse<NonNullable<typeof scanStatus>>(scanRes);
      const metricsVal = await readJsonResponse<AggregatedMetrics>(metricsRes);
      const sessionsVal = await readJsonResponse<{ items: SessionItem[]; total: number }>(sessionsRes);

      setScanStatus(scanStatusVal);
      setData(metricsVal);
      setSessionsData(sessionsVal);

      if (scanStatusVal.is_scanning) {
        pollScanStatus();
      }

      const now = new Date();
      setLastUpdate(now.toTimeString().split(' ')[0]);
    } catch (e: any) {
      console.error("首屏初始化同步失败:", e);
      setInitError(e.message || String(e));
      setInitSteps(s => ({
        checkConfig:  s.checkConfig  === 'loading' ? 'error' : s.checkConfig,
        startScan:    s.startScan    === 'loading' ? 'error' : s.startScan,
        loadMetrics:  s.loadMetrics  === 'loading' ? 'error' : s.loadMetrics,
        loadSessions: s.loadSessions === 'loading' ? 'error' : s.loadSessions,
      }));
    } finally {
      clearInterval(initElapsedRef.current);
      setAppInitializing(false);
    }
  };

  // 手动点击刷新同步按钮
  const handleSyncClick = async () => {
    if (scanStatus?.is_scanning) return;
    setRefreshSpin(true);
    await startScan();
  };

  useEffect(() => {
    // 自动启动后台同步初始化
    performInitialSync();
  }, []);

  // 1. 仅当分页列表相关参数发生变化时，增量拉取会话明细，不触发大盘加载
  useEffect(() => {
    fetchSessions(currentPage, pageSize, searchKeyword, source, sortField, sortOrder, startDate, endDate, hideZero);
  }, [currentPage, pageSize, searchKeyword, sortField, sortOrder, hideZero]);

  // 2. 当时间起止和工具变动时，大盘和列表需要同时同步更新，且重置页码
  useEffect(() => {
    setCurrentPage(1);
    fetchData(source, startDate, endDate);
    fetchSessions(1, pageSize, searchKeyword, source, sortField, sortOrder, startDate, endDate, hideZero);
  }, [source, startDate, endDate]);

  // 3. 热更新侦听
  useEffect(() => {
    let unlisten: () => void;
    listen('db-updated', () => {
      console.log('[热同步] 收到后端数据更新通知，正在重载大盘与分页数据...');
      fetchData(source, startDate, endDate);
      fetchSessions(currentPage, pageSize, searchKeyword, source, sortField, sortOrder, startDate, endDate, hideZero);
    }).then((u) => {
      unlisten = u;
    });
    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, [source, startDate, endDate, currentPage, pageSize, searchKeyword, sortField, sortOrder, hideZero]);

  // 4. 监听窗口关闭请求事件
  useEffect(() => {
    let unlisten: () => void;
    listen('close-requested', () => {
      console.log('收到窗口关闭请求，弹出确认框');
      setShowCloseConfirmModal(true);
    }).then((u) => {
      unlisten = u;
    });
    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  // 排序字段切换
  const handleSort = (field: keyof SessionItem | 'total') => {
    if (sortField === field) {
      setSortOrder(sortOrder === 'desc' ? 'asc' : 'desc');
    } else {
      setSortField(field);
      setSortOrder('desc');
    }
  };

  // 会话列表过滤与排序 (方案一：平滑对接后端分页结果)
  const paginatedSessions = useMemo(() => {
    return sessionsData?.items || [];
  }, [sessionsData]);

  // 分页计算与辅助函数
  const totalItems = sessionsData?.total || 0;
  const totalPages = Math.ceil(totalItems / pageSize) || 1;

  const getPageNumbers = () => {
    const pages = [];
    const maxVisiblePages = 5;
    
    if (totalPages <= maxVisiblePages) {
      for (let i = 1; i <= totalPages; i++) {
        pages.push(i);
      }
    } else {
      pages.push(1);
      
      let start = Math.max(2, currentPage - 1);
      let end = Math.min(totalPages - 1, currentPage + 1);
      
      if (currentPage <= 2) {
        end = 4;
      } else if (currentPage >= totalPages - 1) {
        start = totalPages - 3;
      }
      
      if (start > 2) {
        pages.push('...');
      }
      
      for (let i = start; i <= end; i++) {
        pages.push(i);
      }
      
      if (end < totalPages - 1) {
        pages.push('...');
      }
      
      pages.push(totalPages);
    }
    return pages;
  };

  // 模型占用比例计算
  const maxModelTokens = useMemo(() => {
    if (!data?.model_distribution || data.model_distribution.length === 0) return 0;
    return Math.max(...data.model_distribution.map(m => m.total_tokens));
  }, [data?.model_distribution]);



  const totals = data?.totals;

  return (
    <div className="relative min-height-screen text-text-primary">
      {/* 离线脱机模式微光横幅 */}
      {!isOnline && (
        <div className="bg-gradient-to-r from-amber-600/95 to-orange-500/95 text-white text-xs font-semibold py-2 px-4 shadow-[0_4px_20px_rgba(249,115,22,0.15)] flex items-center justify-center gap-2 border-b border-orange-500/50 backdrop-blur-sm z-[9999] animate-pulse-glow no-print">
          <span>⚠️ 物理网络已断开。AI Token Monitor 已平滑启用「本地脱机容灾模式」，自动拦截云端延迟，保障本地大盘 100% 极速可用！</span>
        </div>
      )}
      {/* 背景光效 */}
      <div className="background-decor-1 bg-decor-cyan animate-pulse-glow fixed -top-48 -left-24 w-[600px] h-[600px] rounded-full blur-[80px] z-[-1] pointer-events-none"></div>
      <div className="background-decor-2 bg-decor-purple animate-pulse-glow-reverse fixed -bottom-72 -right-24 w-[700px] h-[700px] rounded-full blur-[100px] z-[-1] pointer-events-none"></div>

      {!appInitializing && !initError && (
        <div className="max-w-[1400px] mx-auto p-4 sm:p-5 flex flex-col gap-4" id="report-container">
        {/* 头部导航栏 */}
        <header className="relative z-30 dashboard-header-bg glass-card flex flex-col md:flex-row justify-between items-center px-5 py-3 gap-3 no-print">
          <div className="flex items-center gap-4">
            <svg className="w-9 h-9" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
              <path d="M12 2L2 7L12 12L22 7L12 2Z" stroke="url(#logo-grad)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/>
              <path d="M2 17L12 22L22 17" stroke="url(#logo-grad)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/>
              <path d="M2 12L12 17L22 12" stroke="url(#logo-grad)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/>
              <defs>
                <linearGradient id="logo-grad" x1="2" y1="2" x2="22" y2="22" gradientUnits="userSpaceOnUse">
                  <stop stopColor="#06b6d4" />
                  <stop offset="1" stopColor="#a855f7" />
                </linearGradient>
              </defs>
            </svg>
            <div className="flex flex-col items-start">
              <h1 className="text-xl font-bold bg-gradient-to-r from-neon-cyan to-neon-purple bg-clip-text text-transparent tracking-tight">AI Token Monitor</h1>
              <span className="text-[10px] font-semibold px-2 py-0.5 rounded-full bg-neon-cyan/15 border border-neon-cyan/35 text-neon-cyan leading-none">Multi-Engine Dashboard</span>
            </div>
          </div>
          <div className="flex items-center gap-4">
            <div className="text-xs text-text-secondary flex items-center gap-2">
              <span className="w-1.5 h-1.5 bg-neon-green rounded-full shadow-[0_0_8px_var(--color-neon-green)]"></span>
              数据同步于: <span className="font-mono text-neon-cyan font-semibold">{lastUpdate}</span>
            </div>

            {/* 数据源选择器 */}
            <div className="relative">
              <button
                onClick={() => setIsSourceDropdownOpen(!isSourceDropdownOpen)}
                className="flex items-center gap-2 bg-bg-secondary/60 dark:bg-[#0b1528] border border-card-border rounded-xl px-3.5 py-2 text-xs font-semibold text-text-primary outline-none focus:border-neon-cyan focus:shadow-[0_0_10px_rgba(6,182,212,0.25)] hover:border-neon-cyan/50 transition-all duration-300 cursor-pointer h-10 min-w-[170px] justify-between shadow-sm select-none"
              >
                <div className="flex items-center gap-2">
                  {source === 'all' && <Globe className="w-4 h-4 text-neon-cyan" />}
                  {source === 'antigravity' && <GeminiIcon className="w-4 h-4 text-[#8b5cf6]" />}
                  {source === 'claude_code' && <ClaudeIcon className="w-4 h-4 text-[#d97757]" />}
                  {source === 'codex' && <OpenAIIcon className="w-4 h-4 text-[#10a37f]" />}
                  {source === 'cursor' && <CursorIcon className="w-4 h-4 text-[#00bcd4]" />}
                  {source === 'trae' && <TraeIcon className="w-4 h-4 text-[#3b82f6]" />}
                  {source === 'trae_cn' && <TraeIcon className="w-4 h-4 text-[#10b981]" />}
                  <span>
                    {source === 'all' && '全部工具 (All)'}
                    {source === 'antigravity' && 'Antigravity'}
                    {source === 'claude_code' && 'Claude Code'}
                    {source === 'codex' && 'Codex CLI'}
                    {source === 'cursor' && 'Cursor'}
                    {source === 'trae' && 'Trae'}
                    {source === 'trae_cn' && 'Trae CN'}
                  </span>
                </div>
                <ChevronDown className={`w-3.5 h-3.5 text-text-secondary transition-transform duration-300 ${isSourceDropdownOpen ? 'rotate-180' : ''}`} />
              </button>

              {isSourceDropdownOpen && (
                <>
                  <div
                    className="fixed inset-0 z-40"
                    onClick={() => setIsSourceDropdownOpen(false)}
                  ></div>
                  <div className="absolute left-0 right-0 mt-2 bg-bg-secondary/95 dark:bg-[#0f192b]/95 border border-card-border rounded-xl shadow-[0_10px_35px_rgba(0,0,0,0.35)] backdrop-blur-md z-50 py-1.5 flex flex-col gap-0.5 animate-fade-in">
                    {[
                      { value: 'all', label: '全部工具 (All)', icon: <Globe className="w-3.5 h-3.5 text-neon-cyan" /> },
                      { value: 'antigravity', label: 'Antigravity', icon: <GeminiIcon className="w-3.5 h-3.5 text-[#8b5cf6]" /> },
                      { value: 'claude_code', label: 'Claude Code', icon: <ClaudeIcon className="w-3.5 h-3.5 text-[#d97757]" /> },
                      { value: 'codex', label: 'Codex CLI', icon: <OpenAIIcon className="w-3.5 h-3.5 text-[#10a37f]" /> },
                      { value: 'cursor', label: 'Cursor', icon: <CursorIcon className="w-3.5 h-3.5 text-[#00bcd4]" /> },
                      { value: 'trae', label: 'Trae', icon: <TraeIcon className="w-3.5 h-3.5 text-[#3b82f6]" /> },
                      { value: 'trae_cn', label: 'Trae CN', icon: <TraeIcon className="w-3.5 h-3.5 text-[#10b981]" /> },
                    ].map((opt) => (
                      <button
                        key={opt.value}
                        onClick={() => {
                          setSource(opt.value as any);
                          setIsSourceDropdownOpen(false);
                        }}
                        className={`flex items-center gap-2.5 px-4 py-2.5 text-xs font-semibold text-left transition-all duration-200 cursor-pointer ${
                          source === opt.value
                            ? 'bg-gradient-to-r from-neon-cyan/15 to-neon-purple/10 text-neon-cyan border-l-2 border-neon-cyan'
                            : 'text-text-secondary hover:text-text-primary hover:bg-white/5 border-l-2 border-transparent'
                        }`}
                      >
                        {opt.icon}
                        <span>{opt.label}</span>
                      </button>
                    ))}
                  </div>
                </>
              )}
            </div>
            {/* 复盘与建议按钮 */}
            <button
              onClick={() => setIsReviewOpen(true)}
              className="flex items-center gap-2 h-10 px-4 rounded-xl border transition-all duration-300 hover:scale-105 active:scale-100 cursor-pointer"
              style={{
                background: 'linear-gradient(135deg, rgba(8,145,178,0.12), rgba(124,58,237,0.08))',
                borderColor: 'rgba(6,182,212,0.35)',
                color: 'var(--neon-cyan)',
                boxShadow: '0 2px 12px rgba(6,182,212,0.1)',
              }}
              title="AI 使用复盘与建议"
            >
              <Sparkles className="w-4 h-4" />
              <span className="text-xs font-semibold">复盘</span>
            </button>

            {/* 数据库数据源配置按钮 */}
            <button
              onClick={() => setIsConfigOpen(true)}
              className="flex items-center justify-center w-10 h-10 rounded-xl bg-bg-secondary/40 dark:bg-white/5 border border-card-border hover:border-neon-cyan/40 hover:scale-105 active:scale-100 transition-all duration-300 cursor-pointer text-text-secondary hover:text-neon-cyan"
              title="系统数据源配置"
            >
              <Settings className="w-5 h-5 hover:rotate-45 transition-transform duration-300" />
            </button>

            {/* 主题切换按钮 */}
            <button
              onClick={() => setTheme(theme === 'light' ? 'dark' : 'light')}
              className="flex items-center justify-center w-10 h-10 rounded-xl bg-bg-secondary/40 dark:bg-white/5 border border-card-border hover:border-neon-cyan/40 hover:scale-105 active:scale-100 transition-all duration-300 cursor-pointer text-text-secondary hover:text-neon-cyan"
              title={theme === 'light' ? '切换至夜间模式' : '切换至日间模式'}
            >
              {theme === 'light' ? <Moon className="w-5 h-5" /> : <Sun className="w-5 h-5" />}
            </button>

            <button
              onClick={handleSyncClick}
              disabled={loading || scanStatus?.is_scanning}
              className={`flex items-center gap-2 text-sm font-semibold bg-gradient-to-r from-neon-cyan to-neon-purple hover:scale-105 active:scale-100 hover:shadow-neon-cyan/35 text-white px-5 rounded-xl transition-all duration-300 h-10 ${
                loading || scanStatus?.is_scanning ? 'opacity-70 cursor-not-allowed' : 'cursor-pointer'
              }`}
            >
              <RefreshCw className={`w-4 h-4 ${refreshSpin || scanStatus?.is_scanning ? 'animate-spin' : ''}`} />
              <span>{scanStatus?.is_scanning ? '正在同步...' : '同步刷新'}</span>
            </button>
          </div>
        </header>

        {/* 时间筛选控制栏 */}
        <section className="glass-card p-3 flex flex-col md:flex-row justify-between items-center gap-3 border border-card-border bg-bg-secondary/20 shadow-sm backdrop-blur-md rounded-[24px] no-print">
          <div className="flex flex-wrap items-center gap-1.5">
            <span className="text-xs font-semibold text-text-secondary mr-2">🕒 时间区间：</span>
            <div className="pill-container">
              {[
                { key: 'all', label: '全部时间' },
                { key: 'today', label: '今日' },
                { key: 'week', label: '最近7天' },
                { key: '30days', label: '最近30天' },
                { key: 'month', label: '本月' },
                { key: 'quarter', label: '本季度' },
                { key: 'custom', label: '自定义' },
              ].map((item) => (
                <button
                  key={item.key}
                  onClick={() => setTimeRange(item.key as any)}
                  style={
                    timeRange === item.key
                      ? {
                          background: "linear-gradient(to right, #3b82f6, #06b6d4)",
                          color: "#ffffff",
                          boxShadow: "0 10px 24px rgba(37, 99, 235, 0.22)",
                        }
                      : undefined
                  }
                  className={`px-3.5 py-1.5 rounded-full text-xs font-semibold hover:scale-105 active:scale-100 transition-all duration-200 cursor-pointer ${
                    timeRange === item.key
                      ? 'text-white'
                      : 'text-slate-600 dark:text-slate-400 hover:bg-slate-100/50 dark:hover:bg-white/5'
                  }`}
                >
                  {item.label}
                </button>
              ))}
            </div>
          </div>

          {timeRange === 'custom' && (
            <div className="flex items-center gap-2 select-none animate-fade-in">
              <input
                type="date"
                value={startDate}
                onChange={(e) => setStartDate(e.target.value)}
                className="bg-bg-secondary/60 dark:bg-[#0b1528] border border-card-border rounded-xl px-3 py-2 text-xs font-semibold text-text-primary outline-none focus:border-neon-cyan focus:shadow-[0_0_10px_rgba(6,182,212,0.25)] hover:border-neon-cyan/50 transition-all duration-300 cursor-pointer"
              />
              <span className="text-xs text-text-secondary font-semibold">至</span>
              <input
                type="date"
                value={endDate}
                onChange={(e) => setEndDate(e.target.value)}
                className="bg-bg-secondary/60 dark:bg-[#0b1528] border border-card-border rounded-xl px-3 py-2 text-xs font-semibold text-text-primary outline-none focus:border-neon-cyan focus:shadow-[0_0_10px_rgba(6,182,212,0.25)] hover:border-neon-cyan/50 transition-all duration-300 cursor-pointer"
              />
            </div>
          )}
        </section>

        {/* 扫描进度条展示 */}
        {scanStatus && scanStatus.is_scanning && (
          <div className="glass-card rounded-[24px] p-5 flex flex-col gap-3 no-print">
            <div className="flex justify-between items-center text-sm">
              <div className="flex items-center gap-2">
                <RefreshCw className="w-4 h-4 text-neon-cyan animate-spin" />
                <span className="font-semibold text-text-primary">正在增量同步历史会话数据...</span>
              </div>
              <span className="font-mono text-xs text-text-secondary font-semibold">
                {scanStatus.scanned_files} / {scanStatus.total_files} ({scanStatus.total_files > 0 ? Math.round((scanStatus.scanned_files / scanStatus.total_files) * 100) : 0}%)
              </span>
            </div>
            <div className="h-1.5 w-full bg-slate-200/50 dark:bg-white/5 rounded-full overflow-hidden border border-card-border relative">
              <div
                className="h-full rounded-full bg-gradient-to-r from-neon-cyan to-neon-purple transition-all duration-300"
                style={{ width: `${scanStatus.total_files > 0 ? (scanStatus.scanned_files / scanStatus.total_files) * 100 : 0}%` }}
              ></div>
            </div>
            <div className="flex justify-between items-center text-xs mt-1 border-t border-neon-cyan/10 pt-2 gap-4">
              <span className="text-text-secondary truncate text-[11px]" title={scanStatus.status_msg}>
                {scanStatus.status_msg || "正在同步..."}
              </span>
              <button
                onClick={() => setShowLogConsole(!showLogConsole)}
                className="flex items-center gap-1 text-neon-cyan hover:text-neon-purple active:scale-95 transition-all duration-200 cursor-pointer font-semibold shrink-0"
              >
                <Terminal className="w-3.5 h-3.5" />
                {showLogConsole ? '隐藏细节' : '查看同步细节'}
              </button>
            </div>

            {showLogConsole && scanStatus.logs && (
              <div className="bg-black/85 dark:bg-[#050b14] rounded-xl p-3 border border-slate-800 shadow-inner max-h-[220px] overflow-y-auto font-mono text-[10px] text-green-400 select-text leading-relaxed scrollbar-thin">
                {scanStatus.logs.map((log, i) => (
                  <div key={i} className="whitespace-pre-wrap py-0.5 border-b border-white/5 last:border-b-0">
                    <span className="text-neon-cyan mr-1.5 font-bold">&gt;</span>
                    {log}
                  </div>
                ))}
                <div ref={logEndRef} />
              </div>
            )}
          </div>
        )}

        {/* 扫描错误提示 */}
        {scanStatus && scanStatus.error && (
          <div className="glass-card rounded-[24px] p-4 border border-red-500/20 bg-red-500/5 text-red-400 flex items-center justify-between text-sm no-print">
            <div className="flex items-center gap-2">
              <span className="w-2 h-2 rounded-full bg-red-500 animate-pulse"></span>
              <span>同步会话数据失败: {scanStatus.error}</span>
            </div>
            <button
              onClick={handleSyncClick}
              className="text-xs font-semibold text-neon-cyan hover:underline cursor-pointer bg-transparent border-none outline-none"
            >
              重新尝试同步
            </button>
          </div>
        )}

        {/* KPI 看板 */}
        <section className="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 xl:grid-cols-3 gap-4 animate-fade-in">

          {/* 估算消费额 */}
          <div className="kpi-card kpi-green glass-card p-3.5 flex justify-between items-center group">
            <div className="flex flex-col">
              <span className="text-xs text-text-secondary font-medium mb-0.5">估算消费额 ({displayCurrency})</span>
              <h2 className="text-xl font-semibold font-mono tracking-tight text-text-primary mb-0.5" title={totals ? `USD $${totals.total_cost.toFixed(6)}` : '0'}>
                {totals ? formatCurrency(totals.total_cost) : '$0.00'}
              </h2>
              <span className="text-[9px] font-semibold text-text-muted tracking-wider uppercase">Estimated Cost</span>
            </div>
            <div className="w-10 h-10 rounded-xl flex items-center justify-center bg-neon-green/15 text-neon-green border border-neon-green/30 group-hover:scale-110 transition-transform duration-300">
              <Globe className="w-5 h-5" />
            </div>
          </div>

          {/* 总消耗 */}
          <div className="kpi-card kpi-blue glass-card p-3.5 flex justify-between items-center group">
            <div className="flex flex-col">
              <span className="text-xs text-text-secondary font-medium mb-0.5">总消耗 Token</span>
              <h2 className="text-xl font-semibold font-mono tracking-tight text-text-primary mb-0.5" title={totals ? formatPreciseNum(totals.total_tokens) : '0'}>{totals ? formatNum(totals.total_tokens) : 0}</h2>
              <span className="text-[9px] font-semibold text-text-muted tracking-wider uppercase">Total Tokens</span>
            </div>
            <div className="w-10 h-10 rounded-xl flex items-center justify-center bg-neon-purple/15 text-neon-purple border border-neon-purple/30 group-hover:scale-110 transition-transform duration-300">
              <Compass className="w-5 h-5" />
            </div>
          </div>

          {/* 输入 */}
          <div className="kpi-card kpi-blue glass-card p-3.5 flex justify-between items-center group">
            <div className="flex flex-col">
              <span className="text-xs text-text-secondary font-medium mb-0.5">输入 Token</span>
              <h2 className="text-xl font-semibold font-mono tracking-tight text-text-primary mb-0.5" title={totals ? formatPreciseNum(totals.total_input) : '0'}>{totals ? formatNum(totals.total_input) : 0}</h2>
              <span className="text-[9px] font-semibold text-text-muted tracking-wider uppercase">Total Prompt</span>
            </div>
            <div className="w-10 h-10 rounded-xl flex items-center justify-center bg-neon-cyan/15 text-neon-cyan border border-neon-cyan/30 group-hover:scale-110 transition-transform duration-300">
              <ArrowDown className="w-5 h-5" />
            </div>
          </div>

          {/* 输出 */}
          <div className="kpi-card kpi-blue glass-card p-3.5 flex justify-between items-center group">
            <div className="flex flex-col">
              <span className="text-xs text-text-secondary font-medium mb-0.5">输出 Token</span>
              <h2 className="text-xl font-semibold font-mono tracking-tight text-text-primary mb-0.5" title={totals ? formatPreciseNum(totals.total_output) : '0'}>{totals ? formatNum(totals.total_output) : 0}</h2>
              <span className="text-[9px] font-semibold text-text-muted tracking-wider uppercase">Total Candidates</span>
            </div>
            <div className="w-10 h-10 rounded-xl flex items-center justify-center bg-neon-pink/15 text-neon-pink border border-neon-pink/30 group-hover:scale-110 transition-transform duration-300">
              <ArrowUp className="w-5 h-5" />
            </div>
          </div>

          {/* 缓存命中率 */}
          <div className="kpi-card kpi-cyan glass-card p-3.5 flex justify-between items-center group">
            <div className="flex flex-col">
              <span className="text-xs text-text-secondary font-medium mb-0.5">缓存命中率</span>
              <h2 className="text-xl font-semibold font-mono tracking-tight text-text-primary mb-0.5">{totals ? formatPercent(totals.cache_hit_rate) : '0.0%'}</h2>
              <span className="text-[9px] font-semibold text-text-muted tracking-wider uppercase">Cache Hit Rate</span>
            </div>
            <div className="w-10 h-10 rounded-xl flex items-center justify-center bg-neon-blue/15 text-neon-blue border border-neon-blue/30 group-hover:scale-110 transition-transform duration-300">
              <Database className="w-5 h-5" />
            </div>
          </div>

          {/* 推理 Token 占比 */}
          <div className="kpi-card kpi-cyan glass-card p-3.5 flex justify-between items-center group">
            <div className="flex flex-col">
              <span className="text-xs text-text-secondary font-medium mb-0.5">推理 Token 占比</span>
              <h2 className="text-xl font-semibold font-mono tracking-tight text-text-primary mb-0.5">{totals ? formatPercent(totals.thinking_ratio) : '0.0%'}</h2>
              <span className="text-[9px] font-semibold text-text-muted tracking-wider uppercase">Thinking Ratio</span>
            </div>
            <div className="w-10 h-10 rounded-xl flex items-center justify-center bg-neon-purple/15 text-neon-purple border border-neon-purple/30 group-hover:scale-110 transition-transform duration-300">
              <Brain className="w-5 h-5" />
            </div>
          </div>

          {/* 缓存 Token 数 */}
          <div className="kpi-card kpi-cyan glass-card p-3.5 flex justify-between items-center group">
            <div className="flex flex-col">
              <span className="text-xs text-text-secondary font-medium mb-0.5">缓存命中数</span>
              <h2 className="text-xl font-semibold font-mono tracking-tight text-text-primary mb-0.5" title={totals ? formatPreciseNum(totals.total_cached) : '0'}>{totals ? formatNum(totals.total_cached) : 0}</h2>
              <span className="text-[9px] font-semibold text-text-muted tracking-wider uppercase">Cached Tokens</span>
            </div>
            <div className="w-10 h-10 rounded-xl flex items-center justify-center bg-neon-green/15 text-neon-green border border-neon-green/30 group-hover:scale-110 transition-transform duration-300">
              <Cpu className="w-5 h-5" />
            </div>
          </div>

          {/* 推理 Token 数 */}
          <div className="kpi-card kpi-purple glass-card p-3.5 flex justify-between items-center group">
            <div className="flex flex-col">
              <span className="text-xs text-text-secondary font-medium mb-0.5">推理消耗数</span>
              <h2 className="text-xl font-semibold font-mono tracking-tight text-text-primary mb-0.5" title={totals ? formatPreciseNum(totals.total_thinking) : '0'}>{totals ? formatNum(totals.total_thinking) : 0}</h2>
              <span className="text-[9px] font-semibold text-text-muted tracking-wider uppercase">Thinking Tokens</span>
            </div>
            <div className="w-10 h-10 rounded-xl flex items-center justify-center bg-neon-purple/15 text-neon-purple border border-neon-purple/30 group-hover:scale-110 transition-transform duration-300">
              <Hash className="w-5 h-5" />
            </div>
          </div>

          {/* 总会话数 */}
          <div className="kpi-card kpi-slate glass-card p-3.5 flex justify-between items-center group">
            <div className="flex flex-col">
              <span className="text-xs text-text-secondary font-medium mb-0.5">总会话数</span>
              <h2 className="text-xl font-semibold font-mono tracking-tight text-text-primary mb-0.5" title={totals ? formatPreciseNum(totals.total_sessions) : '0'}>{totals ? formatNum(totals.total_sessions) : 0}</h2>
              <span className="text-[9px] font-semibold text-text-muted tracking-wider uppercase">Total Sessions</span>
            </div>
            <div className="w-10 h-10 rounded-xl flex items-center justify-center bg-neon-teal/15 text-neon-teal border border-neon-teal/30 group-hover:scale-110 transition-transform duration-300">
              <MessageSquare className="w-5 h-5" />
            </div>
          </div>
        </section>

        {/* 每日趋势图 */}
        <section className="chart-section glass-card p-4 sm:p-5">
          <div className="section-header flex flex-col sm:flex-row justify-between items-start sm:items-center pb-2 mb-3.5 border-b border-card-border gap-2">
            <h2 className="text-base font-semibold text-text-primary">
              {source === 'all' && chartDimension === 'source' 
                ? '各引擎每日用量对比走势 (Token 堆叠柱状图)' 
                : chartDimension === 'device' 
                ? '各设备每日用量对比走势 (Token 堆叠柱状图)' 
                : '每日用量走势 (Token 堆叠柱状图)'}
            </h2>
            
            <div className="rounded-xl border border-card-border bg-bg-secondary/40 dark:bg-white/3 p-0.5 flex items-center gap-0.5 shadow-sm">
              <button
                onClick={() => setChartDimension('type')}
                className={`px-3 py-1.5 rounded-lg text-xs font-semibold hover:scale-105 active:scale-100 transition-all duration-200 cursor-pointer ${
                  chartDimension === 'type'
                    ? 'bg-gradient-to-r from-neon-cyan to-neon-purple text-white shadow-[0_4px_10px_rgba(6,182,212,0.15)]'
                    : 'text-text-secondary hover:text-text-primary hover:bg-white/5'
                }`}
              >
                📊 类型维度
              </button>
              {source === 'all' && (
                <button
                  onClick={() => setChartDimension('source')}
                  className={`px-3 py-1.5 rounded-lg text-xs font-semibold hover:scale-105 active:scale-100 transition-all duration-200 cursor-pointer ${
                    chartDimension === 'source'
                      ? 'bg-gradient-to-r from-neon-cyan to-neon-purple text-white shadow-[0_4px_10px_rgba(6,182,212,0.15)]'
                      : 'text-text-secondary hover:text-text-primary hover:bg-white/5'
                  }`}
                >
                  🤖 工具维度
                </button>
              )}
              <button
                onClick={() => setChartDimension('device')}
                className={`px-3 py-1.5 rounded-lg text-xs font-semibold hover:scale-105 active:scale-100 transition-all duration-200 cursor-pointer ${
                  chartDimension === 'device'
                    ? 'bg-gradient-to-r from-neon-cyan to-neon-purple text-white shadow-[0_4px_10px_rgba(6,182,212,0.15)]'
                    : 'text-text-secondary hover:text-text-primary hover:bg-white/5'
                }`}
              >
                💻 设备维度
              </button>
            </div>
          </div>
          <div className="w-full">
            <Suspense fallback={<ChartFallback />}>
              {source === 'all' && chartDimension === 'source' ? (
                data?.source_trends && data.source_trends.length > 0 ? (
                  <SourceTrendChart data={data.source_trends} theme={theme} />
                ) : (
                  <div className="h-[300px] flex items-center justify-center text-text-muted italic">暂无趋势图表数据</div>
                )
              ) : chartDimension === 'device' ? (
                data?.device_trends && data.device_trends.length > 0 ? (
                  <DailyTrendChart data={data.daily_trends} deviceTrends={data.device_trends} dimension="device" theme={theme} />
                ) : (
                  <div className="h-[300px] flex items-center justify-center text-text-muted italic">暂无趋势图表数据</div>
                )
              ) : (
                data?.daily_trends && data.daily_trends.length > 0 ? (
                  <DailyTrendChart data={data.daily_trends} dimension="type" theme={theme} />
                ) : (
                  <div className="h-[300px] flex items-center justify-center text-text-muted italic">暂无趋势图表数据</div>
                )
              )}
            </Suspense>
          </div>
        </section>

        {/* 项目维度消耗分析 */}
        <section className="animate-fade-in">
          {/* 项目消耗大盘 */}
          <div className="chart-section glass-card p-4 sm:p-5 flex flex-col gap-4">
            <div className="pb-3 border-b border-card-border">
              <h2 className="text-sm font-semibold text-text-primary">项目消耗大盘走势 (Token 折线图)</h2>
            </div>
            <div className="w-full">
              {data?.project_trends && data.project_trends.length > 0 ? (
                <ProjectTrendChart
                  data={data.project_trends}
                  theme={theme}
                  displayCurrency={displayCurrency}
                  exchangeRate={usdExchangeRate}
                />
              ) : (
                <div className="h-[300px] flex items-center justify-center text-text-muted italic">暂无项目维度大盘数据</div>
              )}
            </div>
          </div>
          {/* 项目消耗排行榜 (Top 10) - 暂时隐藏 */}
        </section>

        {/* 分布与汇总 */}
        <section className="grid grid-cols-1 lg:grid-cols-[1fr_1.5fr] gap-6">
          {/* 底层模型排行 */}
          <div className="glass-card p-5 flex flex-col gap-4">
            <div className="pb-3 border-b border-card-border">
              <h2 className="text-sm font-semibold text-text-primary">底层模型消耗占比</h2>
            </div>
            <div className="flex flex-col gap-4 max-h-[350px] overflow-y-auto pr-1">
              {data?.model_distribution && data.model_distribution.length > 0 ? (
                data.model_distribution.map((m) => {
                  const pct = maxModelTokens > 0 ? (m.total_tokens / maxModelTokens) * 100 : 0;
                  return (
                     <div key={m.model} className="flex flex-col gap-1.5">
                      <div className="flex justify-between items-center text-xs">
                        <span className="font-semibold text-text-primary">{m.model}</span>
                        <span className="font-mono text-text-secondary" title={`${formatPreciseNum(m.total_tokens)} Tokens`}>{formatNum(m.total_tokens)} Tokens</span>
                      </div>
                      <div className="h-2 w-full bg-slate-200/50 dark:bg-white/5 rounded-full overflow-hidden border border-card-border relative">
                        <div
                           className="h-full rounded-full bg-gradient-to-r from-neon-cyan to-neon-purple shadow-[0_0_8px_rgba(6,182,212,0.4)] transition-all duration-1000"
                          style={{ width: `${pct}%` }}
                        ></div>
                      </div>
                    </div>
                  );
                })
              ) : (
                <div className="text-center py-6 text-text-muted italic">暂无模型用量占比数据</div>
              )}
            </div>
          </div>

          {/* 按月汇总表 */}
          <div className="glass-card p-5 flex flex-col gap-4">
            <div className="pb-3 border-b border-card-border">
              <h2 className="text-sm font-semibold text-text-primary">
                按月用量汇总（{timeRange === 'custom' ? `${startDate} 至 ${endDate}` : {
                  all: '全部时间',
                  today: '今日',
                  week: '最近7天',
                  '30days': '最近30天',
                  month: '本月',
                  quarter: '本季度'
                }[timeRange] || ''}）
              </h2>
            </div>
            <div className="table-responsive max-h-[350px] overflow-y-auto">
              <table className="data-table">
                <thead>
                  <tr>
                    <th className="text-left py-2.5">月份</th>
                    <th className="text-right py-2.5">会话数</th>
                    <th className="text-right py-2.5">输入 Token</th>
                    <th className="text-right py-2.5">输出 Token</th>
                    <th className="text-right py-2.5">缓存 Token</th>
                    <th className="text-right py-2.5">推理 Token</th>
                  </tr>
                </thead>
                <tbody>
                  {data?.monthly_summary && data.monthly_summary.length > 0 ? (
                    data.monthly_summary.map((row) => (
                      <tr key={row.month} className="hover:bg-table-row-hover transition-colors duration-150">
                        <td className="font-mono text-text-primary py-2.5">{row.month}</td>
                        <td className="font-mono text-right py-2.5" title={formatPreciseNum(row.sessions)}>{formatNum(row.sessions)}</td>
                        <td className="font-mono text-right py-2.5" title={formatPreciseNum(row.input)}>{formatNum(row.input)}</td>
                        <td className="font-mono text-right py-2.5" title={formatPreciseNum(row.output)}>{formatNum(row.output)}</td>
                        <td className="font-mono text-right py-2.5" title={formatPreciseNum(row.cached)}>{formatNum(row.cached)}</td>
                        <td className="font-mono text-right py-2.5" title={formatPreciseNum(row.thinking)}>{formatNum(row.thinking)}</td>
                      </tr>
                    ))
                  ) : (
                    <tr>
                      <td colSpan={6} className="text-center py-6 text-text-muted italic">暂无月度统计数据</td>
                    </tr>
                  )}
                </tbody>
              </table>
            </div>
          </div>
        </section>

        {/* 深度效能诊断面板 */}
        {data?.performance_trends && data.performance_trends.length > 0 && (
          <section className="glass-card p-6 flex flex-col gap-6">
            <div className="section-header flex flex-col sm:flex-row justify-between items-start sm:items-center pb-2 border-b border-card-border gap-2">
              <div>
                <h2 className="text-base font-semibold text-text-primary flex items-center gap-2">
                  ⚡ 深度效能诊断中心 (Performance & Efficiency)
                </h2>
                <p className="text-xs text-text-secondary mt-1">
                  监测交互时的每秒 Token 产出速度 (TPS) 与接口 Turn 级响应延迟 (Latency)
                </p>
              </div>
            </div>
            
            <div className="grid grid-cols-1 lg:grid-cols-[1.8fr_1fr] gap-6">
              {/* 性能趋势折线图 */}
              <div className="w-full bg-slate-500/5 dark:bg-white/[0.01] rounded-2xl p-4 border border-card-border/50">
                <Suspense fallback={<ChartFallback label="正在加载效能图表..." />}>
                  <PerformanceChart data={data.performance_trends} theme={theme} />
                </Suspense>
              </div>
              
              {/* 模型效能排行榜 */}
              <div className="flex flex-col gap-4">
                <div className="bg-bg-secondary/40 dark:bg-white/3 border border-card-border rounded-2xl p-4 flex-1">
                  <h3 className="text-xs font-semibold text-text-primary mb-3">🤖 模型效能诊断评估</h3>
                  <div className="flex flex-col gap-3.5 max-h-[250px] overflow-y-auto pr-1">
                    {data?.model_performance && data.model_performance.length > 0 ? (
                      data.model_performance.map((mp) => (
                        <div key={mp.model} className="flex flex-col gap-1 border-b border-card-border/40 pb-2.5 last:border-0 last:pb-0">
                          <div className="flex justify-between items-center text-xs">
                            <span className="font-semibold text-text-primary truncate max-w-[170px]" title={mp.model}>{mp.model}</span>
                            <span className="text-text-secondary font-mono text-[10px] bg-slate-200/50 dark:bg-white/10 px-1.5 py-0.5 rounded-md">
                              {mp.sample_count} 次交互
                            </span>
                          </div>
                          <div className="grid grid-cols-2 gap-4 mt-1 text-[11px]">
                            <div className="flex flex-col gap-0.5">
                              <span className="text-text-secondary text-[10px]">生成速率 (TPS)</span>
                              <span className="font-semibold font-mono text-neon-cyan">{mp.avg_tps.toFixed(1)} Token/s</span>
                            </div>
                            <div className="flex flex-col gap-0.5">
                              <span className="text-text-secondary text-[10px]">平均延迟 (Latency)</span>
                              <span className="font-semibold font-mono text-neon-purple">{mp.avg_latency.toFixed(2)} 秒</span>
                            </div>
                          </div>
                        </div>
                      ))
                    ) : (
                      <div className="text-center py-6 text-text-muted text-xs italic">暂无模型评估样本，请确保进行了 Cursor 交互</div>
                    )}
                  </div>
                </div>
              </div>
            </div>
          </section>
        )}

        {/* 会话明细 */}
        <section className="glass-card p-6">
          <div className="flex flex-col md:flex-row justify-between items-start md:items-center gap-4 pb-4 mb-5 border-b border-card-border">
            <h2 className="text-base font-semibold text-text-primary">会话用量明细</h2>
            <div className="flex flex-col sm:flex-row items-stretch sm:items-center gap-4 w-full md:w-auto">
              {/* 报表导出操作按钮 */}
              <div className="flex items-center gap-2">
                <button
                  onClick={() => exportToCSV(paginatedSessions)}
                  className="px-3 py-1.5 rounded-lg text-xs font-semibold bg-slate-200/60 hover:bg-slate-300/60 dark:bg-white/5 dark:hover:bg-white/10 text-text-primary border border-card-border cursor-pointer transition-all duration-200 flex items-center gap-1 shadow-sm"
                  title="导出当前筛选出的账单为高兼容 CSV (Excel/WPS 无缝支持)"
                >
                  📥 导出 CSV 账单
                </button>
                {/* 生成财务报表按钮 - 暂时隐藏 */}
              </div>

              {/* 开关 */}
              <label className="flex items-center gap-2 cursor-pointer select-none">
                <span className="text-xs text-text-secondary font-medium">隐藏 0 消耗会话</span>
                <div className="relative">
                  <input
                    type="checkbox"
                    checked={hideZero}
                    onChange={(e) => setHideZero(e.target.checked)}
                    className="sr-only peer"
                  />
                  <div className="w-9 h-5 bg-slate-200 dark:bg-slate-800 rounded-full peer peer-checked:after:translate-x-4 after:content-[''] after:absolute after:top-[3px] after:left-[3px] after:bg-white dark:after:bg-slate-400 peer-checked:after:bg-white after:rounded-full after:h-3.5 after:w-3.5 after:transition-all peer-checked:bg-gradient-to-r peer-checked:from-blue-600 peer-checked:to-cyan-500 shadow-sm border border-slate-300 dark:border-slate-700"></div>
                </div>
              </label>
              {/* 搜索框 */}
              <div className="relative w-full sm:w-[280px]">
                <Search className="w-4 h-4 text-text-muted absolute left-3 top-2.5" />
                <input
                  type="text"
                  placeholder="输入关键字搜索会话..."
                  value={searchKeyword}
                  onChange={(e) => setSearchKeyword(e.target.value)}
                  className="w-full bg-bg-secondary/40 dark:bg-white/3 border border-card-border rounded-xl pl-9 pr-4 py-2 text-xs text-text-primary placeholder-text-muted outline-none focus:border-neon-cyan focus:shadow-[0_0_10px_rgba(6,182,212,0.25)] dark:focus:bg-white/5 hover:border-neon-cyan/50 transition-all duration-300"
                />
              </div>
            </div>
          </div>

          <div className="table-responsive">
            <table className="data-table">
              <thead>
                <tr>
                  <th onClick={() => handleSort('title')} className="sortable text-left py-3 cursor-pointer hover:text-neon-cyan transition-colors">
                    <span className="flex items-center gap-1">会话标题 <ChevronsUpDown className="w-3 h-3 text-text-muted" /></span>
                  </th>
                  <th onClick={() => handleSort('source')} className="sortable text-left py-3 cursor-pointer hover:text-neon-cyan transition-colors">
                    <span className="flex items-center gap-1">统计工具 <ChevronsUpDown className="w-3 h-3 text-text-muted" /></span>
                  </th>
                  <th onClick={() => handleSort('created_at')} className="sortable text-left py-3 cursor-pointer hover:text-neon-cyan transition-colors">
                    <span className="flex items-center gap-1">创建时间 <ChevronsUpDown className="w-3 h-3 text-text-muted" /></span>
                  </th>
                  <th onClick={() => handleSort('models')} className="sortable text-left py-3 cursor-pointer hover:text-neon-cyan transition-colors">
                    <span className="flex items-center gap-1">使用模型 <ChevronsUpDown className="w-3 h-3 text-text-muted" /></span>
                  </th>
                  <th onClick={() => handleSort('input')} className="sortable text-right py-3 cursor-pointer hover:text-neon-cyan transition-colors">
                    <span className="flex items-center justify-end gap-1">输入 Token <ChevronsUpDown className="w-3 h-3 text-text-muted" /></span>
                  </th>
                  <th onClick={() => handleSort('output')} className="sortable text-right py-3 cursor-pointer hover:text-neon-cyan transition-colors">
                    <span className="flex items-center justify-end gap-1">输出 Token <ChevronsUpDown className="w-3 h-3 text-text-muted" /></span>
                  </th>
                  <th onClick={() => handleSort('cached')} className="sortable text-right py-3 cursor-pointer hover:text-neon-cyan transition-colors">
                    <span className="flex items-center justify-end gap-1">缓存 Token <ChevronsUpDown className="w-3 h-3 text-text-muted" /></span>
                  </th>
                  <th onClick={() => handleSort('thinking')} className="sortable text-right py-3 cursor-pointer hover:text-neon-cyan transition-colors">
                    <span className="flex items-center justify-end gap-1">推理 Token <ChevronsUpDown className="w-3 h-3 text-text-muted" /></span>
                  </th>

                  <th onClick={() => handleSort('total')} className="sortable text-right py-3 cursor-pointer hover:text-neon-cyan transition-colors">
                    <span className="flex items-center justify-end gap-1">总计 Token <ChevronsUpDown className="w-3 h-3 text-text-muted" /></span>
                  </th>
                </tr>
              </thead>
              <tbody className={`transition-opacity duration-200 ${sessionsLoading ? 'opacity-40 pointer-events-none' : ''}`}>
                {paginatedSessions.length > 0 ? (
                  paginatedSessions.map((s) => {
                     const totalTokens = s.input + s.output;
                     return (
                      <tr key={s.uuid} className="hover:bg-table-row-hover transition-colors duration-150 border-b border-card-border">
                        <td className="py-3 pr-4 max-w-[280px]">
                          <div className="font-semibold text-text-primary truncate">{s.title}</div>
                          <span className="font-mono text-[9px] text-text-muted block mt-0.5">{s.uuid}</span>
                        </td>
                        <td className="py-3">
                          {s.source === 'antigravity' && (
                            <span className="inline-flex items-center gap-1 text-[10px] font-bold px-2 py-0.5 rounded-full bg-neon-purple/15 border border-neon-purple/35 text-neon-purple leading-none">
                              <GeminiIcon className="w-3 h-3 text-[#8b5cf6]" />
                              Antigravity
                            </span>
                          )}
                          {s.source === 'claude_code' && (
                            <span className="inline-flex items-center gap-1 text-[10px] font-bold px-2 py-0.5 rounded-full bg-orange-500/15 border border-orange-500/35 text-orange-500 leading-none">
                              <ClaudeIcon className="w-3 h-3 text-[#d97757]" />
                              Claude Code
                            </span>
                          )}
                          {s.source === 'codex' && (
                            <span className="inline-flex items-center gap-1 text-[10px] font-bold px-2 py-0.5 rounded-full bg-neon-cyan/15 border border-neon-cyan/35 text-neon-cyan leading-none">
                              <OpenAIIcon className="w-3 h-3 text-[#10a37f]" />
                              Codex CLI
                            </span>
                          )}
                          {s.source === 'cursor' && (
                            <span className="inline-flex items-center gap-1 text-[10px] font-bold px-2 py-0.5 rounded-full bg-blue-500/15 border border-blue-500/35 text-blue-500 leading-none">
                              <CursorIcon className="w-3.5 h-3.5 text-[#00bcd4]" />
                              Cursor
                            </span>
                          )}
                          {s.source === 'trae' && (
                            <span className="inline-flex items-center gap-1 text-[10px] font-bold px-2 py-0.5 rounded-full bg-blue-500/15 border border-blue-500/35 text-blue-500 leading-none">
                              <TraeIcon className="w-3.5 h-3.5 text-[#3b82f6]" />
                              Trae
                            </span>
                          )}
                          {s.source === 'trae_cn' && (
                            <span className="inline-flex items-center gap-1 text-[10px] font-bold px-2 py-0.5 rounded-full bg-emerald-500/15 border border-emerald-500/35 text-emerald-600 dark:text-emerald-500 leading-none">
                              <TraeIcon className="w-3.5 h-3.5 text-[#10b981]" />
                              Trae CN
                            </span>
                          )}
                        </td>
                        <td className="text-xs text-text-secondary py-3">{formatDate(s.created_at)}</td>
                        <td className="py-3">
                          <div className="flex flex-wrap gap-1.5">
                             {s.models && s.models.length > 0 ? (
                              s.models.map((m) => (
                                <span key={m} className="text-[10px] px-2 py-0.5 bg-bg-secondary/40 dark:bg-white/5 border border-card-border text-text-secondary rounded-lg">
                                  {m}
                                </span>
                              ))
                            ) : (
                              <span className="text-[10px] px-2 py-0.5 bg-bg-secondary/40 dark:bg-white/5 border border-card-border text-text-secondary rounded-lg">
                                unknown
                              </span>
                            )}
                          </div>
                        </td>
                        <td className="font-mono text-right py-3" title={formatPreciseNum(s.input)}>{formatNum(s.input)}</td>
                        <td className="font-mono text-right py-3" title={formatPreciseNum(s.output)}>{formatNum(s.output)}</td>
                        <td className="font-mono text-right py-3" title={formatPreciseNum(s.cached)}>{formatNum(s.cached)}</td>
                        <td className="font-mono text-right py-3" title={formatPreciseNum(s.thinking)}>{formatNum(s.thinking)}</td>
                        <td className="font-mono text-right font-bold text-neon-cyan py-3" title={formatPreciseNum(totalTokens)}>{formatNum(totalTokens)}</td>
                        <td className="font-mono text-right font-bold text-neon-green py-3" title={`USD $${s.cost_usd.toFixed(6)}`}>
                          {formatCurrency(s.cost_usd)}
                        </td>
                      </tr>
                    );
                  })
                ) : (
                  <tr>
                    <td colSpan={10} className="text-center py-10 text-text-muted italic">
                      没有符合条件的会话记录
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>

          {/* 分页控制栏 */}
          {paginatedSessions.length > 0 && (
            <div className="flex flex-col sm:flex-row justify-between items-center gap-4 mt-6 pt-5 border-t border-card-border select-none no-print">
              {/* 左侧：记录数信息 */}
              <div className="text-xs text-text-secondary font-medium order-2 sm:order-1 text-center sm:text-left">
                显示第 <span className="font-mono text-neon-cyan font-semibold">{Math.min((currentPage - 1) * pageSize + 1, totalItems)}</span> 到 <span className="font-mono text-neon-cyan font-semibold">{Math.min(currentPage * pageSize, totalItems)}</span> 条记录，共 <span className="font-mono text-neon-cyan font-semibold">{totalItems}</span> 条记录
              </div>

              {/* 中间：翻页页码 */}
              <div className="flex items-center gap-1.5 order-1 sm:order-2">
                <button
                  disabled={currentPage === 1}
                  onClick={() => setCurrentPage(prev => Math.max(1, prev - 1))}
                  className="flex items-center justify-center w-8 h-8 rounded-lg bg-bg-secondary/40 dark:bg-white/3 border border-card-border hover:border-neon-cyan/40 disabled:opacity-40 disabled:pointer-events-none hover:scale-105 active:scale-100 transition-all duration-200 cursor-pointer text-text-secondary hover:text-neon-cyan"
                  title="上一页"
                >
                  <ChevronLeft className="w-4 h-4" />
                </button>

                {getPageNumbers().map((pageNum, idx) => {
                  if (pageNum === '...') {
                    return (
                      <span key={`ellipsis-${idx}`} className="w-8 h-8 flex items-center justify-center text-text-muted font-mono">
                        ...
                      </span>
                    );
                  }
                  const isActive = currentPage === pageNum;
                  return (
                    <button
                      key={`page-${pageNum}`}
                      onClick={() => setCurrentPage(pageNum as number)}
                      className={`w-8 h-8 rounded-lg flex items-center justify-center font-mono text-xs font-semibold border hover:scale-105 active:scale-100 transition-all duration-200 cursor-pointer ${
                        isActive
                          ? 'bg-gradient-to-r from-blue-600 to-cyan-500 text-white border-transparent shadow-[0_4px_12px_rgba(37,99,235,0.25)] font-bold'
                          : 'bg-bg-secondary/40 dark:bg-white/3 border-card-border text-text-secondary hover:border-neon-cyan/40 hover:text-neon-cyan'
                      }`}
                    >
                      {pageNum}
                    </button>
                  );
                })}

                <button
                  disabled={currentPage === totalPages}
                  onClick={() => setCurrentPage(prev => Math.min(totalPages, prev + 1))}
                  className="flex items-center justify-center w-8 h-8 rounded-lg bg-bg-secondary/40 dark:bg-white/3 border border-card-border hover:border-neon-cyan/40 disabled:opacity-40 disabled:pointer-events-none hover:scale-105 active:scale-100 transition-all duration-200 cursor-pointer text-text-secondary hover:text-neon-cyan"
                  title="下一页"
                >
                  <ChevronRight className="w-4 h-4" />
                </button>
              </div>

              {/* 右侧：单页数量选择 */}
              <div className="flex items-center gap-2 order-3 text-center sm:text-right">
                <span className="text-xs text-text-secondary font-medium">每页数量</span>
                <select
                  value={pageSize}
                  onChange={(e) => setPageSize(Number(e.target.value))}
                  className="bg-bg-secondary/60 dark:bg-[#0b1528] border border-card-border rounded-xl px-2.5 py-1 text-xs text-text-primary placeholder-text-muted outline-none focus:border-neon-cyan focus:shadow-[0_0_10px_rgba(6,182,212,0.25)] hover:border-neon-cyan/50 transition-all duration-300 cursor-pointer"
                >
                  <option value={5}>5 条/页</option>
                  <option value={10}>10 条/页</option>
                  <option value={20}>20 条/页</option>
                  <option value={50}>50 条/页</option>
                  <option value={100}>100 条/页</option>
                </select>
              </div>
            </div>
          )}
        </section>

        {/* 日历热力图 */}
        {data?.daily_trends && data.daily_trends.length > 0 && (
          <section className="chart-section glass-card p-4 sm:p-5 hover:-translate-y-0.5 hover:shadow-[0_22px_56px_rgba(15,23,42,0.10)] transition-all duration-200 no-print">
            <Suspense fallback={<ChartFallback label="正在加载日历热力图..." />}>
              <CalendarHeatmap data={data.daily_trends} theme={theme} />
            </Suspense>
          </section>
        )}
      </div>
      )}

      {/* 启动连接错误闪屏 */}
      {initError && (
        <div className="fixed inset-0 bg-bg-app dark:bg-[#030712] z-[9999] flex flex-col items-center justify-center p-6 gap-6 select-none animate-fade-in">
          {/* 背景光效 */}
          <div className="background-decor-1 bg-decor-cyan animate-pulse-glow absolute -top-48 -left-24 w-[600px] h-[600px] rounded-full blur-[80px] z-[-1] pointer-events-none"></div>
          <div className="background-decor-2 bg-decor-purple animate-pulse-glow-reverse absolute -bottom-72 -right-24 w-[700px] h-[700px] rounded-full blur-[100px] z-[-1] pointer-events-none"></div>

          <div className="glass-card rounded-[32px] max-w-[500px] w-full p-8 flex flex-col items-center text-center gap-6 border border-white/10 dark:border-white/5 shadow-[0_20px_50px_rgba(0,0,0,0.3)] relative animate-fade-in">
            {/* 配置源按钮 */}
            <button
              onClick={() => setIsConfigOpen(true)}
              className="absolute top-5 right-5 w-8 h-8 rounded-xl flex items-center justify-center bg-bg-secondary/60 dark:bg-white/5 hover:bg-bg-secondary dark:hover:bg-white/10 text-text-secondary hover:text-neon-cyan transition-all duration-300 hover:rotate-45 active:scale-95 cursor-pointer border border-card-border shadow-sm group"
              title="配置数据源"
            >
              <Settings className="w-4 h-4 text-text-secondary group-hover:text-neon-cyan transition-colors" />
            </button>

            {/* 警告图标 */}
            <div className="w-16 h-16 rounded-2xl flex items-center justify-center bg-red-500/10 text-red-500 border border-red-500/30 shadow-[0_0_20px_rgba(239,68,68,0.2)] animate-pulse mb-2">
              <Settings className="w-8 h-8 text-red-500" />
            </div>

            <div className="flex flex-col gap-2">
              <h2 className="text-xl font-bold bg-gradient-to-r from-red-500 to-orange-500 bg-clip-text text-transparent tracking-tight">
                后台服务连接失败
              </h2>
              <p className="text-xs text-text-secondary max-w-[360px] leading-relaxed">
                无法与本地数据监控服务建立连接，这通常是因为服务尚未启动完毕或数据库配置错误。
              </p>
            </div>

            <div className="w-full bg-black/10 dark:bg-black/40 border border-red-500/15 rounded-2xl p-4 text-left font-mono text-[11px] text-red-400/90 whitespace-pre-wrap max-h-[120px] overflow-y-auto scrollbar-thin">
              {initError}
            </div>

            <button
              onClick={() => performInitialSync()}
              className="w-full py-3 rounded-xl bg-gradient-to-r from-neon-cyan to-neon-purple text-xs font-bold text-white hover:shadow-[0_0_15px_rgba(6,182,212,0.3)] hover:scale-[1.02] active:scale-95 transition-all duration-200 cursor-pointer flex items-center justify-center gap-2"
            >
              <RefreshCw className="w-4 h-4" />
              <span>重新尝试连接</span>
            </button>
          </div>
        </div>
      )}

      {/* 启动加载闪屏 */}
      {appInitializing && !initError && (() => {
        // 计算总体进度百分比
        const stepWeights = { checkConfig: 15, startScan: 30, loadMetrics: 30, loadSessions: 25 };
        const totalProgress = (['checkConfig', 'startScan', 'loadMetrics', 'loadSessions'] as const).reduce((acc, key) => {
          const st = initSteps[key];
          if (st === 'done') return acc + stepWeights[key];
          if (st === 'loading') return acc + stepWeights[key] * 0.5;
          return acc;
        }, 0);

        const INIT_STEP_DEFS = [
          { key: 'checkConfig'  as const, label: '读取系统配置',     icon: '⚙️',  detail: '验证设备名称与数据库配置' },
          { key: 'startScan'   as const, label: '启动后台扫描服务',  icon: '🔍', detail: '初始化 Token 数据扫描引擎' },
          { key: 'loadMetrics' as const, label: '加载大盘统计指标',  icon: '📊', detail: '计算 Token 消耗与成本分析' },
          { key: 'loadSessions'as const, label: '同步会话用量明细',  icon: '📋', detail: '拉取并解析历史会话记录' },
        ];

        return (
          <div className="fixed inset-0 bg-bg-app dark:bg-[#030712] z-[9999] flex flex-col items-center justify-center p-6 select-none animate-fade-in">
            {/* 背景光效 */}
            <div className="background-decor-1 bg-decor-cyan animate-pulse-glow absolute -top-48 -left-24 w-[600px] h-[600px] rounded-full blur-[80px] z-[-1] pointer-events-none"></div>
            <div className="background-decor-2 bg-decor-purple animate-pulse-glow-reverse absolute -bottom-72 -right-24 w-[700px] h-[700px] rounded-full blur-[100px] z-[-1] pointer-events-none"></div>

            <div className="glass-card rounded-[32px] max-w-[420px] w-full p-8 flex flex-col items-center gap-6 border border-white/10 dark:border-white/5 shadow-[0_20px_50px_rgba(0,0,0,0.3)] animate-fade-in">
              {/* Logo + 标题 */}
              <div className="flex flex-col items-center gap-3">
                <div className="relative">
                  <svg className="w-14 h-14 animate-spin" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
                    <path d="M12 2C6.47715 2 2 6.47715 2 12C2 17.5228 6.47715 22 12 22C17.5228 22 22 17.5228 22 12" stroke="url(#spinner-grad-splash)" strokeWidth="2.5" strokeLinecap="round"/>
                    <defs>
                      <linearGradient id="spinner-grad-splash" x1="2" y1="2" x2="22" y2="22" gradientUnits="userSpaceOnUse">
                        <stop stopColor="#06b6d4" />
                        <stop offset="1" stopColor="#a855f7" />
                      </linearGradient>
                    </defs>
                  </svg>
                  <div className="absolute inset-0 flex items-center justify-center text-lg">🤖</div>
                </div>
                <div className="text-center">
                  <h2 className="text-xl font-bold bg-gradient-to-r from-neon-cyan to-neon-purple bg-clip-text text-transparent tracking-tight">AI Token Monitor</h2>
                  <p className="text-xs text-text-muted mt-1">正在初始化后台服务，请稍候...</p>
                </div>
              </div>

              {/* 总体进度条 */}
              <div className="w-full flex flex-col gap-1.5">
                <div className="flex justify-between items-center text-[11px] text-text-secondary font-medium">
                  <span>初始化进度</span>
                  <span className="font-mono text-neon-cyan font-bold">{Math.round(totalProgress)}%</span>
                </div>
                <div className="h-2 w-full bg-slate-200/50 dark:bg-white/5 rounded-full overflow-hidden border border-card-border relative">
                  <div
                    className="h-full rounded-full bg-gradient-to-r from-neon-cyan to-neon-purple shadow-[0_0_10px_rgba(6,182,212,0.4)] transition-all duration-700 ease-out"
                    style={{ width: `${totalProgress}%` }}
                  >
                    {/* 闪光扫过动画 */}
                    <div className="absolute inset-0 bg-gradient-to-r from-transparent via-white/30 to-transparent animate-[shimmer_1.5s_infinite] pointer-events-none" />
                  </div>
                </div>
              </div>

              {/* 步骤列表 */}
              <div className="w-full flex flex-col gap-2">
                {INIT_STEP_DEFS.map(({ key, label, icon, detail }) => {
                  const status = initSteps[key];
                  return (
                    <div
                      key={key}
                      className={`flex items-center gap-3 px-3.5 py-2.5 rounded-xl border transition-all duration-300 ${
                        status === 'done'    ? 'bg-neon-cyan/5 border-neon-cyan/20 text-text-primary' :
                        status === 'loading' ? 'bg-blue-500/8 border-blue-400/30 text-text-primary shadow-[0_0_12px_rgba(59,130,246,0.08)]' :
                        status === 'error'   ? 'bg-red-500/8 border-red-500/25 text-text-secondary' :
                                              'bg-transparent border-card-border/40 text-text-muted'
                      }`}
                    >
                      {/* 状态图标 */}
                      <div className="shrink-0 w-6 h-6 flex items-center justify-center">
                        {status === 'done' && (
                          <svg className="w-5 h-5 text-neon-cyan" viewBox="0 0 20 20" fill="currentColor">
                            <path fillRule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clipRule="evenodd" />
                          </svg>
                        )}
                        {status === 'loading' && (
                          <svg className="w-4 h-4 text-blue-400 animate-spin" viewBox="0 0 24 24" fill="none">
                            <path d="M12 2C6.47715 2 2 6.47715 2 12" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"/>
                            <path d="M12 22C17.5228 22 22 17.5228 22 12" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" opacity="0.3"/>
                          </svg>
                        )}
                        {status === 'error' && (
                          <svg className="w-5 h-5 text-red-400" viewBox="0 0 20 20" fill="currentColor">
                            <path fillRule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zM8.707 7.293a1 1 0 00-1.414 1.414L8.586 10l-1.293 1.293a1 1 0 101.414 1.414L10 11.414l1.293 1.293a1 1 0 001.414-1.414L11.414 10l1.293-1.293a1 1 0 00-1.414-1.414L10 8.586 8.707 7.293z" clipRule="evenodd" />
                          </svg>
                        )}
                        {status === 'pending' && (
                          <div className="w-4 h-4 rounded-full border-2 border-card-border/60 bg-transparent" />
                        )}
                      </div>

                      {/* 文字内容 */}
                      <div className="flex flex-col flex-1 min-w-0">
                        <div className="flex items-center gap-1.5">
                          <span className="text-base leading-none">{icon}</span>
                          <span className={`text-xs font-semibold ${
                            status === 'loading' ? 'text-blue-300' :
                            status === 'done'    ? 'text-text-primary' :
                            status === 'error'   ? 'text-red-400' :
                                                  'text-text-muted'
                          }`}>{label}</span>
                        </div>
                        <span className="text-[10px] text-text-muted mt-0.5 truncate">{detail}</span>
                      </div>

                      {/* 右侧徽章 */}
                      <div className="shrink-0">
                        {status === 'done' && <span className="text-[10px] font-bold text-neon-cyan bg-neon-cyan/10 px-2 py-0.5 rounded-full">完成</span>}
                        {status === 'loading' && <span className="text-[10px] font-bold text-blue-400 bg-blue-400/10 px-2 py-0.5 rounded-full animate-pulse">处理中</span>}
                        {status === 'error' && <span className="text-[10px] font-bold text-red-400 bg-red-400/10 px-2 py-0.5 rounded-full">失败</span>}
                        {status === 'pending' && <span className="text-[10px] text-text-muted">等待</span>}
                      </div>
                    </div>
                  );
                })}
              </div>

              {/* 底部耗时提示 */}
              <div className="flex items-center justify-between w-full pt-1 border-t border-card-border/50">
                <span className="text-[10px] text-text-muted italic">首次启动或大量新会话时可能需要更长时间</span>
                <span className="text-[10px] font-mono text-text-secondary">{initElapsed}s</span>
              </div>
            </div>
          </div>
        );
      })()}

      {/* 首次初始化时的全屏扫描进度遮罩 */}
      {(!data || (data.totals.total_sessions === 0 && data.sessions.length === 0)) && scanStatus && scanStatus.is_scanning && (
        <div className="fixed inset-0 bg-bg-app dark:bg-[#0b1528] z-[9999] flex flex-col items-center justify-center p-6 gap-6 select-none animate-fade-in">
          {/* 背景光效 */}
          <div className="background-decor-1 bg-decor-cyan animate-pulse-glow absolute -top-48 -left-24 w-[600px] h-[600px] rounded-full blur-[80px] z-[-1] pointer-events-none"></div>
          <div className="background-decor-2 bg-decor-purple animate-pulse-glow-reverse absolute -bottom-72 -right-24 w-[700px] h-[700px] rounded-full blur-[100px] z-[-1] pointer-events-none"></div>

          <div className="glass-card rounded-[32px] max-w-[500px] w-full p-8 flex flex-col items-center text-center gap-6 border border-white/10 dark:border-white/5 shadow-[0_20px_50px_rgba(0,0,0,0.3)] relative">
            {/* 右上角系统配置图标，支持在同步挂起或有 BUG 时手动切换回本地 SQLite 模式 */}
            <button
              onClick={() => setIsConfigOpen(true)}
              className="absolute top-5 right-5 w-8 h-8 rounded-xl flex items-center justify-center bg-bg-secondary/60 dark:bg-white/5 hover:bg-bg-secondary dark:hover:bg-white/10 text-text-secondary hover:text-neon-cyan transition-all duration-300 hover:rotate-45 active:scale-95 cursor-pointer border border-card-border shadow-sm group"
              title="配置数据源"
            >
              <Settings className="w-4 h-4 text-text-secondary group-hover:text-neon-cyan transition-colors" />
            </button>

            <svg className="w-16 h-16 animate-spin text-neon-cyan mb-2" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
              <path d="M12 2C6.47715 2 2 6.47715 2 12C2 17.5228 6.47715 22 12 22C17.5228 22 22 17.5228 22 12" stroke="url(#spinner-grad-main)" strokeWidth="3" strokeLinecap="round"/>
              <defs>
                <linearGradient id="spinner-grad-main" x1="2" y1="2" x2="22" y2="22" gradientUnits="userSpaceOnUse">
                  <stop stopColor="#06b6d4" />
                  <stop offset="1" stopColor="#a855f7" />
                </linearGradient>
              </defs>
            </svg>

            <div className="flex flex-col gap-2">
              <h2 className="text-xl font-bold bg-gradient-to-r from-neon-cyan to-neon-purple bg-clip-text text-transparent tracking-tight">
                正在初始化仪表盘
              </h2>
              <p className="text-xs text-text-secondary max-w-[360px] leading-relaxed">
                {scanStatus.status_msg || "正在进行首次数据同步，系统正在扫描并解码历史会话中的 Token 消耗明细，请稍候..."}
              </p>
            </div>

            <div className="w-full flex flex-col gap-2 mt-2">
              <div className="flex justify-between items-center text-xs text-text-secondary font-medium">
                <span>同步进度</span>
                <span className="font-mono text-neon-cyan font-bold">
                  {scanStatus.scanned_files} / {scanStatus.total_files} ({scanStatus.total_files > 0 ? Math.round((scanStatus.scanned_files / scanStatus.total_files) * 100) : 0}%)
                </span>
              </div>
              <div className="h-2.5 w-full bg-slate-200/50 dark:bg-white/5 rounded-full overflow-hidden border border-card-border relative">
                <div
                  className="h-full rounded-full bg-gradient-to-r from-neon-cyan to-neon-purple shadow-[0_0_12px_rgba(6,182,212,0.5)] transition-all duration-300"
                  style={{ width: `${scanStatus.total_files > 0 ? (scanStatus.scanned_files / scanStatus.total_files) * 100 : 0}%` }}
                ></div>
              </div>
            </div>

            <div className="flex justify-between items-center text-xs w-full border-t border-card-border/50 pt-3 gap-4">
              <span className="text-text-muted truncate text-[10px] text-left" title={scanStatus.status_msg}>
                {scanStatus.status_msg || "正在初始化..."}
              </span>
              <button
                onClick={() => setShowLogConsole(!showLogConsole)}
                className="flex items-center gap-1 text-neon-cyan hover:text-neon-purple active:scale-95 transition-all duration-200 cursor-pointer font-semibold shrink-0"
              >
                <Terminal className="w-3.5 h-3.5" />
                {showLogConsole ? '隐藏日志' : '查看同步日志'}
              </button>
            </div>

            {showLogConsole && scanStatus.logs && (
              <div className="w-full bg-black/85 dark:bg-[#050b14] rounded-xl p-3 border border-slate-800 shadow-inner max-h-[160px] overflow-y-auto font-mono text-[9px] text-green-400 text-left select-text leading-relaxed scrollbar-thin">
                {scanStatus.logs.map((log, i) => (
                  <div key={i} className="whitespace-pre-wrap py-0.5 border-b border-white/5 last:border-b-0">
                    <span className="text-neon-cyan mr-1.5 font-bold">&gt;</span>
                    {log}
                  </div>
                ))}
                <div ref={logEndRef} />
              </div>
            )}
            
            <span className="text-[10px] text-text-muted italic">这通常仅在首次启动或有大量新会话时需要较长时间</span>
          </div>
        </div>
      )}

      {/* 全局“正在查询统计中...”及取消遮罩 */}
      {showDelayedLoading && !(scanStatus && scanStatus.is_scanning) && (
        <div className="fixed inset-0 bg-black/60 dark:bg-black/80 backdrop-blur-md z-[9999] flex flex-col items-center justify-center gap-6 animate-fade-in select-none">
          {/* 呼吸发光卡片容器 */}
          <div className="relative flex flex-col items-center bg-bg-secondary/90 dark:bg-[#0f192b]/95 border border-card-border p-8 rounded-3xl shadow-2xl max-w-sm w-full mx-4 shadow-neon-cyan/5">
            {/* 装饰性背景光效 */}
            <div className="absolute -top-12 -left-12 w-24 h-24 bg-neon-cyan/15 rounded-full blur-2xl pointer-events-none"></div>
            <div className="absolute -bottom-12 -right-12 w-24 h-24 bg-neon-purple/15 rounded-full blur-2xl pointer-events-none"></div>

            {/* 渐变双向旋转 Spinner */}
            <div className="relative mb-6">
              <div className="w-[56px] h-[56px] border-4 border-slate-200 dark:border-white/5 rounded-full border-t-neon-cyan border-b-neon-purple animate-spin"></div>
              <div className="absolute inset-0.5 rounded-full border border-dashed border-neon-cyan/20 animate-spin-reverse pointer-events-none"></div>
            </div>

            {/* 标题与副标题 */}
            <h3 className="text-base font-bold text-text-primary mb-2 text-center tracking-wide">
              正在查询统计中...
            </h3>
            <p className="text-xs text-text-secondary text-center mb-6 max-w-[200px] leading-relaxed">
              系统正在努力为您计算指标数据，请稍候
            </p>

            {/* 取消查询按钮 */}
            <button
              onClick={handleCancelQuery}
              className="flex items-center justify-center gap-2 border border-red-500/35 hover:border-red-500 hover:bg-red-500/10 active:scale-95 text-red-500 font-semibold text-xs px-5 py-2.5 rounded-xl transition-all duration-300 cursor-pointer shadow-sm w-full hover:shadow-[0_0_15px_rgba(239,68,68,0.25)] select-none bg-transparent outline-none"
            >
              <svg className="w-3.5 h-3.5 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth="2.5">
                <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
              </svg>
              <span>取消查询</span>
            </button>
          </div>
        </div>
      )}

      {/* 数据库数据源配置弹窗 */}
      {isConfigOpen && (
        <div className="fixed inset-0 z-[10000] flex items-center justify-center bg-black/60 dark:bg-black/80 backdrop-blur-sm p-4 animate-fade-in">
          <div className="relative w-full max-w-2xl rounded-3xl border border-card-border bg-bg-secondary/95 dark:bg-[#0f192b]/95 backdrop-blur-xl p-6 text-text-primary shadow-2xl overflow-hidden shadow-neon-cyan/5 transition-all duration-300">
            {/* 装饰性背景光效 */}
            <div className="absolute -top-24 -left-24 w-48 h-48 bg-neon-cyan/20 rounded-full blur-3xl pointer-events-none"></div>
            <div className="absolute -bottom-24 -right-24 w-48 h-48 bg-neon-purple/20 rounded-full blur-3xl pointer-events-none"></div>

            <div className="flex justify-between items-center pb-4 border-b border-card-border mb-4 relative z-10">
              <h2 className="text-lg font-bold flex items-center gap-2">
                <Settings className="w-5 h-5 text-neon-cyan animate-spin-slow" />
                <span className="bg-gradient-to-r from-neon-cyan to-neon-purple bg-clip-text text-transparent">智业 AI 治理配置大厅</span>
              </h2>
              <button
                onClick={() => {
                  setIsConfigOpen(false);
                  setConfigMessage(null);
                  setPricingMessage(null);
                }}
                className="w-8 h-8 rounded-full flex items-center justify-center bg-bg-secondary/60 dark:bg-white/5 hover:bg-bg-secondary dark:hover:bg-white/10 text-text-secondary hover:text-text-primary transition-all cursor-pointer border border-card-border"
              >
                ×
              </button>
            </div>

            {/* 标签页切换 */}
            <div className="flex border-b border-card-border mb-5 relative z-10">
              <button
                type="button"
                onClick={() => setActiveTab('source')}
                className={`flex-1 py-2 text-xs font-bold border-b-2 transition-all cursor-pointer ${
                  activeTab === 'source'
                    ? 'border-neon-cyan text-neon-cyan'
                    : 'border-transparent text-text-secondary hover:text-text-primary'
                }`}
              >
                🖥️ 数据源与系统设置
              </button>
              <button
                type="button"
                onClick={() => setActiveTab('pricing')}
                className={`flex-1 py-2 text-xs font-bold border-b-2 transition-all cursor-pointer ${
                  activeTab === 'pricing'
                    ? 'border-neon-purple text-neon-purple'
                    : 'border-transparent text-text-secondary hover:text-text-primary'
                }`}
              >
                💵 汇率与模型费率
              </button>
              <button
                type="button"
                onClick={() => setActiveTab('optimize')}
                className={`flex-1 py-2 text-xs font-bold border-b-2 transition-all cursor-pointer ${
                  activeTab === 'optimize'
                    ? 'border-neon-pink text-neon-pink'
                    : 'border-transparent text-text-secondary hover:text-text-primary'
                }`}
              >
                ⚡ 维护与瘦身
              </button>
            </div>

            <div className="space-y-5 relative z-10 max-h-[500px] overflow-y-auto pr-1">
              {activeTab === 'source' && (
                <div className="space-y-5 animate-fade-in">
                  {/* 设备名称配置 */}
                  <div className="flex flex-col gap-2">
                    <label className="text-xs font-semibold text-text-secondary">💻 当前设备名称 (Device Name)</label>
                    <div className="flex gap-2">
                      <input
                        type="text"
                        value={deviceName}
                        onChange={(e) => setDeviceName(e.target.value)}
                        placeholder={`例如: Work-Laptop (建议值: ${defaultDeviceName})`}
                        className="flex-1 bg-bg-secondary/60 dark:bg-black/35 border border-card-border rounded-xl px-4 py-2.5 text-xs text-text-primary placeholder-text-muted outline-none focus:border-neon-cyan focus:shadow-[0_0_10px_rgba(6,182,212,0.25)] transition-all duration-300"
                      />
                      <button
                        type="button"
                        onClick={() => setDeviceName(defaultDeviceName)}
                        className="px-3 rounded-xl border border-card-border text-[11px] font-semibold text-text-secondary hover:text-text-primary hover:border-neon-cyan/40 bg-bg-secondary/40 dark:bg-white/5 transition-all cursor-pointer animate-fade-in"
                      >
                        填入建议值
                      </button>
                    </div>
                    <p className="text-[10px] text-text-muted leading-relaxed">
                      * 用于多设备数据同步时区分用量。如果留空，系统启动时将拦截大盘并提示配置。
                    </p>
                  </div>

                  {/* 窗口关闭行为配置 */}
                  <div className="flex flex-col gap-2 animate-fade-in text-left">
                    <label className="text-xs font-semibold text-text-secondary">🚪 窗口关闭行为 (Close Behavior)</label>
                    <select
                      value={closeBehavior}
                      onChange={(e) => setCloseBehavior(e.target.value as 'prompt' | 'close' | 'minimize')}
                      className="w-full bg-bg-secondary/60 dark:bg-black/35 border border-card-border rounded-xl px-4 py-2.5 text-xs text-text-primary outline-none focus:border-neon-cyan focus:shadow-[0_0_10px_rgba(6,182,212,0.25)] transition-all duration-300"
                    >
                      <option value="prompt">每次关闭时询问确认 (默认)</option>
                      <option value="close">直接关闭并退出程序</option>
                      <option value="minimize">最小化隐藏到系统托盘</option>
                    </select>
                    <p className="text-[10px] text-text-muted leading-relaxed font-sans">
                      * 配置点击主窗口右上角关闭按钮时的动作。若选择最小化，软件将继续在后台驻留运行。
                    </p>
                  </div>

                  <div className="flex flex-col gap-2">
                    <label className="text-xs font-semibold text-text-secondary">🔌 数据库类型 (Database Engine)</label>
                    <div className="grid grid-cols-2 gap-3">
                      <button
                        type="button"
                        onClick={() => {
                          setDbType('sqlite');
                          setConfigMessage(null);
                        }}
                        className={`py-3 rounded-xl border text-xs font-bold transition-all duration-300 cursor-pointer text-center ${
                          dbType === 'sqlite'
                            ? 'bg-neon-cyan/15 border-neon-cyan text-neon-cyan shadow-[0_0_10px_rgba(6,182,212,0.15)]'
                            : 'bg-bg-secondary/40 dark:bg-white/3 border border-card-border text-text-secondary hover:text-text-primary hover:border-neon-cyan/40 hover:bg-bg-secondary/80'
                        }`}
                      >
                        SQLite (本地嵌入式)
                      </button>
                      <button
                        type="button"
                        onClick={() => {
                          setDbType('postgres');
                          setConfigMessage(null);
                        }}
                        className={`py-3 rounded-xl border text-xs font-bold transition-all duration-300 cursor-pointer text-center ${
                          dbType === 'postgres'
                            ? 'bg-neon-purple/15 border-neon-purple text-neon-purple shadow-[0_0_10px_rgba(168,85,247,0.15)]'
                            : 'bg-bg-secondary/40 dark:bg-white/3 border border-card-border text-text-secondary hover:text-text-primary hover:border-neon-cyan/40 hover:bg-bg-secondary/80'
                        }`}
                      >
                        PostgreSQL (远程数据库)
                      </button>
                    </div>
                  </div>

                  {dbType === 'sqlite' ? (
                    <div className="flex flex-col gap-2 animate-fade-in text-left">
                      <label className="text-xs font-semibold text-text-secondary">📂 自定义数据库物理路径</label>
                      <input
                        type="text"
                        value={sqlitePath}
                        onChange={(e) => setSqlitePath(e.target.value)}
                        placeholder="请输入绝对路径，例如 D:\\data\\stats.db"
                        className="w-full bg-bg-secondary/60 dark:bg-black/35 border border-card-border rounded-xl px-4 py-3 text-xs text-text-primary placeholder-text-muted outline-none focus:border-neon-cyan focus:shadow-[0_0_10px_rgba(6,182,212,0.25)] transition-all duration-300"
                      />
                      <p className="text-[10px] text-text-muted leading-relaxed">
                        * 默认路径：<code className="bg-bg-secondary dark:bg-black/30 px-1.5 py-0.5 rounded text-neon-cyan font-mono">USERPROFILE\.ai_token_monitor\token_stats.db</code>。如果留空或使用默认位置，系统会自动管理。若修改为新路径，系统将自动在该目录下创建表。
                      </p>
                    </div>
                  ) : (
                    <div className="flex flex-col gap-3.5 animate-fade-in text-left">
                      {/* 主机与端口并排 */}
                      <div className="grid grid-cols-3 gap-3">
                        <div className="col-span-2 flex flex-col gap-1.5">
                          <label className="text-xs font-semibold text-text-secondary">🖥️ 主机地址 (Host)</label>
                          <input
                            type="text"
                            value={pgHost}
                            onChange={(e) => setPgHost(e.target.value)}
                            placeholder="localhost 或 IP 地址"
                            className="w-full bg-bg-secondary/60 dark:bg-black/35 border border-card-border rounded-xl px-4 py-2.5 text-xs text-text-primary placeholder-text-muted outline-none focus:border-neon-purple focus:shadow-[0_0_10px_rgba(168,85,247,0.25)] transition-all duration-300"
                          />
                        </div>
                        <div className="flex flex-col gap-1.5">
                          <label className="text-xs font-semibold text-text-secondary">🔌 端口 (Port)</label>
                          <input
                            type="text"
                            value={pgPort}
                            onChange={(e) => setPgPort(e.target.value)}
                            placeholder="5432"
                            className="w-full bg-bg-secondary/60 dark:bg-black/35 border border-card-border rounded-xl px-4 py-2.5 text-xs text-text-primary placeholder-text-muted outline-none focus:border-neon-purple focus:shadow-[0_0_10px_rgba(168,85,247,0.25)] transition-all duration-300"
                          />
                        </div>
                      </div>

                      {/* 用户名与密码并排 */}
                      <div className="grid grid-cols-2 gap-3">
                        <div className="flex flex-col gap-1.5">
                          <label className="text-xs font-semibold text-text-secondary">👤 用户名 (Username)</label>
                          <input
                            type="text"
                            value={pgUser}
                            onChange={(e) => setPgUser(e.target.value)}
                            placeholder="postgres"
                            className="w-full bg-bg-secondary/60 dark:bg-black/35 border border-card-border rounded-xl px-4 py-2.5 text-xs text-text-primary placeholder-text-muted outline-none focus:border-neon-purple focus:shadow-[0_0_10px_rgba(168,85,247,0.25)] transition-all duration-300"
                          />
                        </div>
                        <div className="flex flex-col gap-1.5">
                          <label className="text-xs font-semibold text-text-secondary">🔑 密码 (Password)</label>
                          <div className="relative w-full">
                            <input
                              type={showPassword ? "text" : "password"}
                              value={pgPassword}
                              onChange={(e) => setPgPassword(e.target.value)}
                              placeholder="数据库密码"
                              className="w-full bg-bg-secondary/60 dark:bg-black/35 border border-card-border rounded-xl pl-4 pr-10 py-2.5 text-xs text-text-primary placeholder-text-muted outline-none focus:border-neon-purple focus:shadow-[0_0_10px_rgba(168,85,247,0.25)] transition-all duration-300"
                            />
                            <button
                              type="button"
                              onClick={() => setShowPassword(!showPassword)}
                              className="absolute right-3 top-2.5 flex items-center justify-center text-xs text-text-muted hover:text-text-primary focus:outline-none bg-transparent border-none cursor-pointer select-none"
                            >
                              {showPassword ? "👁️" : "👁️‍Q"}
                            </button>
                          </div>
                        </div>
                      </div>

                      {/* 数据库名称 */}
                      <div className="flex flex-col gap-1.5">
                        <label className="text-xs font-semibold text-text-secondary">🗄️ 数据库名称 (Database)</label>
                        <input
                          type="text"
                          value={pgDatabase}
                          onChange={(e) => setPgDatabase(e.target.value)}
                          placeholder="token_monitor"
                          className="w-full bg-bg-secondary/60 dark:bg-black/35 border border-card-border rounded-xl px-4 py-2.5 text-xs text-text-primary placeholder-text-muted outline-none focus:border-neon-purple focus:shadow-[0_0_10px_rgba(168,85,247,0.25)] transition-all duration-300"
                        />
                      </div>
                      
                      <p className="text-[10px] text-text-muted leading-relaxed">
                        * 连接测试成功后，若对方是新空库，系统会自动初始化 sessions 和 turns 表结构。
                      </p>
                    </div>
                  )}

                  {configMessage && (
                    <div
                      className={`p-3 rounded-xl border text-xs leading-relaxed flex gap-2 items-start animate-fade-in ${
                        configMessage.success
                          ? 'bg-emerald-500/10 border-emerald-500/30 text-emerald-400'
                          : 'bg-rose-500/10 border-rose-500/30 text-rose-400'
                      }`}
                    >
                      <span className="text-sm">{configMessage.success ? '✅' : '❌'}</span>
                      <div className="whitespace-pre-wrap font-medium">{configMessage.text}</div>
                    </div>
                  )}

                  <div className="grid grid-cols-2 gap-3 pt-3">
                    <button
                      type="button"
                      onClick={async () => {
                        setTestLoading(true);
                        setConfigMessage(null);
                        try {
                          const response = await fetch(apiUrl('/config/test'), {
                            method: 'POST',
                            headers: { 'Content-Type': 'application/json' },
                            body: JSON.stringify({
                              db_type: dbType,
                              sqlite_path: sqlitePath,
                              pg_host: pgHost,
                              pg_port: pgPort,
                              pg_user: pgUser,
                              pg_password: pgPassword,
                              pg_database: pgDatabase,
                            })
                          });
                          if (response.ok) {
                            const res = await readJsonResponse<any>(response);
                            setConfigMessage({ success: res.success, text: res.message });
                          } else {
                            setConfigMessage({ success: false, text: '服务器返回错误，连接测试失败。' });
                          }
                        } catch (e: any) {
                          setConfigMessage({ success: false, text: `网络错误：${e.message}` });
                        } finally {
                          setTestLoading(false);
                        }
                      }}
                      disabled={testLoading || saveLoading}
                      className="py-3 rounded-xl bg-bg-secondary/40 dark:bg-white/5 border border-card-border text-xs font-bold text-text-primary hover:bg-bg-secondary/80 hover:border-neon-cyan/40 hover:text-neon-cyan transition-all cursor-pointer flex items-center justify-center gap-2 active:scale-95 disabled:opacity-50"
                    >
                      {testLoading ? (
                        <RefreshCw className="w-3.5 h-3.5 animate-spin" />
                      ) : (
                        <span>⚡ 一键测试连接</span>
                      )}
                    </button>
                    <button
                      type="button"
                      onClick={async () => {
                        setSaveLoading(true);
                        setConfigMessage(null);
                        try {
                          const response = await fetch(apiUrl('/config/save'), {
                            method: 'POST',
                            headers: { 'Content-Type': 'application/json' },
                            body: JSON.stringify({
                              db_type: dbType,
                              sqlite_path: sqlitePath,
                              pg_host: pgHost,
                              pg_port: pgPort,
                              pg_user: pgUser,
                              pg_password: pgPassword,
                              pg_database: pgDatabase,
                              device_name: deviceName,
                              close_behavior: closeBehavior,
                            })
                          });
                          if (response.ok) {
                            const res = await readJsonResponse<any>(response);
                            if (res.success) {
                              setIsConfigOpen(false);
                              if (res.need_restart) {
                                if (confirm(`${res.message}\n\n为了使新的数据库配置生效并避免数据冲突，软件需要重新启动。是否立即自动重启软件？`)) {
                                  try {
                                    await fetch(apiUrl('/app/restart'), { method: 'POST' });
                                  } catch (e) {
                                    console.error('重启请求失败:', e);
                                    alert('自动重启失败，请手动关闭并重新打开软件。');
                                  }
                                } else {
                                  alert('配置已保存！新配置将在下次启动软件时生效。');
                                }
                              } else {
                                alert('配置已保存，设备名称修改立即生效，无需重启！');
                                // 静默重新加载大盘数据
                                fetchData(source, startDate, endDate);
                                fetchSessions(1, pageSize, searchKeyword, source, sortField, sortOrder, startDate, endDate, hideZero);
                              }
                            } else {
                              setConfigMessage({ success: false, text: res.message });
                            }
                          } else {
                            setConfigMessage({ success: false, text: '服务器保存失败。' });
                          }
                        } catch (e: any) {
                          setConfigMessage({ success: false, text: `网络保存错误：${e.message}` });
                        } finally {
                          setSaveLoading(false);
                        }
                      }}
                      disabled={testLoading || saveLoading}
                      className="py-3 rounded-xl bg-gradient-to-r from-neon-cyan to-neon-purple hover:shadow-[0_0_15px_rgba(6,182,212,0.3)] hover:scale-105 text-xs font-bold text-white transition-all cursor-pointer flex items-center justify-center gap-2 active:scale-95 disabled:opacity-50"
                    >
                      {saveLoading ? (
                        <RefreshCw className="w-3.5 h-3.5 animate-spin" />
                      ) : (
                        <span>💾 保存并应用配置</span>
                      )}
                    </button>
                  </div>
                </div>
              )}

              {activeTab === 'pricing' && (
                <div className="space-y-5 animate-fade-in text-left">
                  {/* 显示币种设置 */}
                  <div className="flex flex-col gap-2">
                    <label className="text-xs font-semibold text-text-secondary">💵 显示币种 (Display Currency)</label>
                    <select
                      value={displayCurrency}
                      onChange={(e) => setDisplayCurrency(e.target.value)}
                      className="w-full bg-bg-secondary/60 dark:bg-black/35 border border-card-border rounded-xl px-4 py-2.5 text-xs text-text-primary outline-none focus:border-neon-purple transition-all"
                    >
                      <option value="USD">USD ($) - 默认美元</option>
                      <option value="CNY">CNY (￥) - 人民币 (汇率: 7.24)</option>
                      <option value="JPY">JPY (¥) - 日元 (汇率: 155.4)</option>
                      <option value="EUR">EUR (€) - 欧元 (汇率: 0.92)</option>
                    </select>
                    <p className="text-[10px] text-text-muted leading-relaxed">
                      * 汇率根据后台 exchange_rates 本地数据库进行换算。点击保存后，大盘用量将全部切换为选定币种展示。
                    </p>
                    {exchangeRateUpdatedAt && (
                      <p className="text-[9px] text-text-muted leading-relaxed mt-0.5">
                        * 汇率更新时间: {new Date(exchangeRateUpdatedAt).toLocaleString('zh-CN')}
                      </p>
                    )}
                  </div>

                  {/* 模型费率列表管理 */}
                  <div className="flex flex-col gap-2">
                    <div className="flex justify-between items-center">
                      <label className="text-xs font-semibold text-text-secondary">
                        🏷️ 模型计费费率表 (USD / 百万 Token) {pricingLoading && <span className="text-[10px] text-neon-purple animate-pulse ml-2">(正在加载最新费率...)</span>}
                      </label>
                      <div className="flex gap-2">
                        <button
                          type="button"
                          onClick={() => {
                            const newRow = {
                              model_pattern: 'gpt-4*',
                              input_price_per_million: 10.0,
                              cached_input_price_per_million: 5.0,
                              output_price_per_million: 30.0,
                              priority: 10,
                              enabled: true,
                              updated_at: ''
                            };
                            setModelPricingRows([...modelPricingRows, newRow]);
                          }}
                          className="px-2.5 py-1 rounded-lg border border-neon-purple/40 text-[10px] font-bold text-neon-purple hover:bg-neon-purple/5 transition-all cursor-pointer"
                        >
                          ➕ 新增规则
                        </button>
                        <button
                          type="button"
                          onClick={async () => {
                            if (confirm('是否确认恢复默认的模型费率规则？这会清空你当前的自定义修改。')) {
                              try {
                                const res = await fetch(apiUrl('/exchange-rates/refresh'), { method: 'POST' });
                                if (res.ok) {
                                  // 重载费率列表
                                  const pricingRes = await fetch(apiUrl(`/model-pricing?t=${Date.now()}`));
                                  if (pricingRes.ok) {
                                    const pricingData = await readJsonResponse<any>(pricingRes);
                                    if (pricingData.rows) {
                                      setModelPricingRows(pricingData.rows);
                                      alert('恢复默认计费规则成功！');
                                    }
                                  }
                                }
                              } catch (e) {
                                console.error(e);
                              }
                            }
                          }}
                          className="px-2.5 py-1 rounded-lg border border-card-border text-[10px] font-medium text-text-secondary hover:text-text-primary transition-all cursor-pointer"
                        >
                          🔄 恢复默认
                        </button>
                      </div>
                    </div>

                    <div className="border border-card-border rounded-xl overflow-hidden bg-bg-secondary/40 dark:bg-black/20 max-h-[220px] overflow-y-auto">
                      <table className="w-full text-left text-[11px] border-collapse">
                        <thead>
                          <tr className="bg-slate-200/40 dark:bg-white/5 text-text-secondary border-b border-card-border font-bold">
                            <th className="p-2">匹配规则 (Glob)</th>
                            <th className="p-2 w-16">输入 ($)</th>
                            <th className="p-2 w-16">缓存 ($)</th>
                            <th className="p-2 w-16">输出 ($)</th>
                            <th className="p-2 w-14">优先级</th>
                            <th className="p-2 w-12">状态</th>
                            <th className="p-2 w-10">操作</th>
                          </tr>
                        </thead>
                        <tbody>
                          {modelPricingRows.length > 0 ? (
                            modelPricingRows.map((row, idx) => (
                              <tr key={idx} className="border-b border-card-border/60 hover:bg-slate-200/20 dark:hover:bg-white/3">
                                <td className="p-1.5">
                                  <input
                                    type="text"
                                    value={row.model_pattern}
                                    onChange={(e) => {
                                      const next = [...modelPricingRows];
                                      next[idx].model_pattern = e.target.value;
                                      setModelPricingRows(next);
                                    }}
                                    className="w-full bg-transparent border-b border-slate-300 dark:border-slate-700 focus:border-neon-purple outline-none p-0.5"
                                  />
                                </td>
                                <td className="p-1.5">
                                  <input
                                    type="number"
                                    step="0.001"
                                    value={row.input_price_per_million}
                                    onChange={(e) => {
                                      const next = [...modelPricingRows];
                                      next[idx].input_price_per_million = parseFloat(e.target.value) || 0;
                                      setModelPricingRows(next);
                                    }}
                                    className="w-full bg-transparent border-b border-slate-300 dark:border-slate-700 focus:border-neon-purple outline-none p-0.5 font-mono"
                                  />
                                </td>
                                <td className="p-1.5">
                                  <input
                                    type="number"
                                    step="0.001"
                                    value={row.cached_input_price_per_million}
                                    onChange={(e) => {
                                      const next = [...modelPricingRows];
                                      next[idx].cached_input_price_per_million = parseFloat(e.target.value) || 0;
                                      setModelPricingRows(next);
                                    }}
                                    className="w-full bg-transparent border-b border-slate-300 dark:border-slate-700 focus:border-neon-purple outline-none p-0.5 font-mono"
                                  />
                                </td>
                                <td className="p-1.5">
                                  <input
                                    type="number"
                                    step="0.001"
                                    value={row.output_price_per_million}
                                    onChange={(e) => {
                                      const next = [...modelPricingRows];
                                      next[idx].output_price_per_million = parseFloat(e.target.value) || 0;
                                      setModelPricingRows(next);
                                    }}
                                    className="w-full bg-transparent border-b border-slate-300 dark:border-slate-700 focus:border-neon-purple outline-none p-0.5 font-mono"
                                  />
                                </td>
                                <td className="p-1.5">
                                  <input
                                    type="number"
                                    value={row.priority}
                                    onChange={(e) => {
                                      const next = [...modelPricingRows];
                                      next[idx].priority = parseInt(e.target.value) || 0;
                                      setModelPricingRows(next);
                                    }}
                                    className="w-full bg-transparent border-b border-slate-300 dark:border-slate-700 focus:border-neon-purple outline-none p-0.5 font-mono"
                                  />
                                </td>
                                <td className="p-1.5 text-center">
                                  <input
                                    type="checkbox"
                                    checked={row.enabled}
                                    onChange={(e) => {
                                      const next = [...modelPricingRows];
                                      next[idx].enabled = e.target.checked;
                                      setModelPricingRows(next);
                                    }}
                                    className="cursor-pointer accent-neon-purple"
                                  />
                                </td>
                                <td className="p-1.5 text-center">
                                  <button
                                    type="button"
                                    onClick={() => {
                                      const next = modelPricingRows.filter((_, i) => i !== idx);
                                      setModelPricingRows(next);
                                    }}
                                    className="text-rose-500 hover:text-rose-600 font-bold cursor-pointer text-xs"
                                  >
                                    删除
                                  </button>
                                </td>
                              </tr>
                            ))
                          ) : (
                            <tr>
                              <td colSpan={7} className="text-center py-6 text-text-muted italic">暂无计费规则数据</td>
                            </tr>
                          )}
                        </tbody>
                      </table>
                    </div>
                  </div>

                  {pricingMessage && (
                    <div
                      className={`p-3 rounded-xl border text-xs leading-relaxed flex gap-2 items-start animate-fade-in ${
                        pricingMessage.success
                          ? 'bg-emerald-500/10 border-emerald-500/30 text-emerald-400'
                          : 'bg-rose-500/10 border-rose-500/30 text-rose-400'
                      }`}
                    >
                      <span className="text-sm">{pricingMessage.success ? '✅' : '❌'}</span>
                      <div className="whitespace-pre-wrap font-medium">{pricingMessage.text}</div>
                    </div>
                  )}

                  <div className="pt-2 flex justify-end">
                    <button
                      type="button"
                      onClick={async () => {
                        setSaveLoading(true);
                        setPricingMessage(null);
                        try {
                          // 1. 先保存币种设置 (通过 config/save 提交)
                          await fetch(apiUrl('/config/save'), {
                            method: 'POST',
                            headers: { 'Content-Type': 'application/json' },
                            body: JSON.stringify({
                              db_type: dbType,
                              sqlite_path: sqlitePath,
                              pg_host: pgHost,
                              pg_port: pgPort,
                              pg_user: pgUser,
                              pg_password: pgPassword,
                              pg_database: pgDatabase,
                              device_name: deviceName,
                              display_currency: displayCurrency,
                              close_behavior: closeBehavior,
                            })
                          });

                          // 2. 保存模型费率
                          const response = await fetch(apiUrl('/model-pricing'), {
                            method: 'POST',
                            headers: { 'Content-Type': 'application/json' },
                            body: JSON.stringify(modelPricingRows)
                          });
                          if (response.ok) {
                            const res = await readJsonResponse<any>(response);
                            setPricingMessage({ success: res.success, text: res.message });
                            if (res.success) {
                              // 重新加载大盘数据以应用最新计费规则
                              fetchData(source, startDate, endDate);
                              fetchSessions(1, pageSize, searchKeyword, source, sortField, sortOrder, startDate, endDate, hideZero);
                              alert('汇率与计费规则配置已成功应用并重新计算历史数据费用！');
                              setIsConfigOpen(false);
                            }
                          } else {
                            setPricingMessage({ success: false, text: '服务器保存费率失败。' });
                          }
                        } catch (e: any) {
                          setPricingMessage({ success: false, text: `网络保存错误：${e.message}` });
                        } finally {
                          setSaveLoading(false);
                        }
                      }}
                      disabled={saveLoading}
                      className="w-48 py-2.5 rounded-xl bg-gradient-to-r from-neon-cyan to-neon-purple hover:shadow-[0_0_15px_rgba(6,182,212,0.3)] hover:scale-105 text-xs font-bold text-white transition-all cursor-pointer flex items-center justify-center gap-2 active:scale-95 disabled:opacity-50"
                    >
                      {saveLoading ? (
                        <RefreshCw className="w-3.5 h-3.5 animate-spin" />
                      ) : (
                        <span>💾 保存并应用费率</span>
                      )}
                    </button>
                  </div>
                </div>
              )}

              {activeTab === 'optimize' && (
                <div className="space-y-5 animate-fade-in text-center py-4">
                  <div className="flex flex-col items-center gap-2">
                    <RefreshCw className={`w-8 h-8 text-neon-pink ${cleanLoading ? 'animate-spin' : ''}`} />
                    <h3 className="text-xs font-semibold text-text-primary">SQLite 本地缓存优化瘦身</h3>
                    <p className="text-[10px] text-text-muted max-w-[320px] mx-auto leading-relaxed">
                      本操作将执行 SQLite VACUUM 整理数据库物理空间碎片，重新计算预聚合历史统计，并重建 FTS5 会话索引（Sessions Full-Text Search），极致提升大盘加载与关键字搜索速度！
                    </p>
                  </div>

                  <button
                    type="button"
                    onClick={handleDbClean}
                    disabled={cleanLoading}
                    className="w-48 mx-auto py-2.5 rounded-xl border border-neon-pink text-xs font-bold text-neon-pink hover:bg-neon-pink/5 hover:shadow-[0_0_10px_rgba(236,72,153,0.15)] active:scale-95 transition-all disabled:opacity-50 cursor-pointer"
                  >
                    {cleanLoading ? '正在极致优化中...' : '⚡ 立即极致优化并瘦身'}
                  </button>

                  {cleanMessage && (
                    <div className="p-3 rounded-xl border border-card-border bg-bg-secondary/40 text-text-primary text-[11px] font-semibold max-w-[360px] mx-auto whitespace-pre-line leading-relaxed animate-fade-in">
                      {cleanMessage}
                    </div>
                  )}
                </div>
              )}
            </div>
          </div>
        </div>
      )}

      {/* 设备名称未配置时的拦截强制配置弹窗 */}
      {showDeviceModal && (
        <div className="fixed inset-0 z-[20000] flex items-center justify-center bg-black/70 dark:bg-black/90 backdrop-blur-md p-4 animate-fade-in">
          <div className="relative w-full max-w-md rounded-3xl border border-neon-cyan/30 bg-bg-secondary/95 dark:bg-[#080f1e]/95 backdrop-blur-2xl p-6 text-text-primary shadow-[0_0_50px_rgba(6,182,212,0.15)] overflow-hidden">
            {/* 装饰性背景光效 */}
            <div className="absolute -top-20 -left-20 w-44 h-44 bg-neon-cyan/25 rounded-full blur-3xl pointer-events-none"></div>
            <div className="absolute -bottom-20 -right-20 w-44 h-44 bg-neon-purple/25 rounded-full blur-3xl pointer-events-none"></div>

            <div className="text-center pb-3 border-b border-card-border mb-5 relative z-10">
              <h2 className="text-base font-bold bg-gradient-to-r from-neon-cyan to-neon-purple bg-clip-text text-transparent flex items-center justify-center gap-2">
                <Monitor className="w-5 h-5 text-neon-cyan animate-pulse" />
                <span>初始化设备配置</span>
              </h2>
              <p className="text-xs text-text-secondary mt-2">首次使用或多设备同步需要为当前物理设备设置唯一标识</p>
            </div>

            <div className="space-y-5 relative z-10">
              <div className="flex flex-col gap-2 text-left">
                <label className="text-xs font-semibold text-text-secondary">💻 设备名称 (Device Name)</label>
                <input
                  type="text"
                  value={deviceName}
                  onChange={(e) => setDeviceName(e.target.value)}
                  placeholder={`自动识别建议值: ${defaultDeviceName}`}
                  className="w-full bg-bg-secondary/60 dark:bg-black/35 border border-card-border rounded-xl px-4 py-3 text-xs text-text-primary placeholder-text-muted outline-none focus:border-neon-cyan focus:shadow-[0_0_10px_rgba(6,182,212,0.25)] transition-all duration-300"
                />
                <div className="flex gap-2 justify-end mt-1">
                  <button
                    type="button"
                    onClick={() => setDeviceName(defaultDeviceName)}
                    className="text-[10px] text-neon-cyan hover:underline bg-transparent border-none cursor-pointer"
                  >
                    使用系统建议名称: {defaultDeviceName}
                  </button>
                </div>
              </div>

              {configMessage && (
                <div
                  className={`p-3 rounded-xl border text-xs leading-relaxed flex gap-2 items-start animate-fade-in ${
                    configMessage.success
                      ? 'bg-emerald-500/10 border-emerald-500/30 text-emerald-400'
                      : 'bg-rose-500/10 border-rose-500/30 text-rose-400'
                  }`}
                >
                  <span className="text-sm">{configMessage.success ? '✅' : '❌'}</span>
                  <div className="whitespace-pre-wrap font-medium">{configMessage.text}</div>
                </div>
              )}

              <div className="pt-2">
                <button
                  type="button"
                  onClick={async () => {
                    if (!deviceName.trim()) {
                      setConfigMessage({ success: false, text: '设备名称不能为空，请填入名称后再保存。' });
                      return;
                    }
                    setSaveLoading(true);
                    setConfigMessage(null);
                    try {
                      const response = await fetch(apiUrl('/config/save'), {
                        method: 'POST',
                        headers: { 'Content-Type': 'application/json' },
                        body: JSON.stringify({
                          db_type: dbType,
                          sqlite_path: sqlitePath,
                          pg_host: pgHost,
                          pg_port: pgPort,
                          pg_user: pgUser,
                          pg_password: pgPassword,
                          pg_database: pgDatabase,
                          device_name: deviceName.trim(),
                          close_behavior: closeBehavior,
                        })
                      });
                      if (response.ok) {
                        const res = await readJsonResponse<any>(response);
                        if (res.success) {
                          if (res.need_restart) {
                            setConfigMessage({ success: true, text: '配置保存成功！系统正在自动重启以使配置生效...' });
                            setTimeout(async () => {
                              try {
                                await fetch(apiUrl('/app/restart'), { method: 'POST' });
                              } catch (e) {
                                console.error('重启请求失败:', e);
                                setConfigMessage({ success: false, text: '保存成功，但自动重启失败。请手动关闭并重新启动程序。' });
                              }
                            }, 1500);
                          } else {
                            setConfigMessage({ success: true, text: '设备名称配置成功！正在载入系统数据...' });
                            setTimeout(() => {
                              setShowDeviceModal(false);
                              performInitialSync(true); // 跳过设备名检查，直接加载
                            }, 1000);
                          }
                        } else {
                          setConfigMessage({ success: false, text: res.message });
                        }
                      } else {
                        setConfigMessage({ success: false, text: '服务器保存失败，接口错误。' });
                      }
                    } catch (e: any) {
                      setConfigMessage({ success: false, text: `保存异常：${e.message}` });
                    } finally {
                      setSaveLoading(false);
                    }
                  }}
                  disabled={saveLoading}
                  className="w-full py-3.5 rounded-xl bg-gradient-to-r from-neon-cyan to-neon-purple text-xs font-bold text-white shadow-lg hover:shadow-neon-cyan/20 active:scale-[0.98] transition-all cursor-pointer flex items-center justify-center gap-2 disabled:opacity-50"
                >
                  {saveLoading ? (
                    <RefreshCw className="w-3.5 h-3.5 animate-spin" />
                  ) : (
                    <span>🚀 确认保存设备配置</span>
                  )}
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
      {/* 使用复盘与建议抽屉 */}
      <ReviewDrawer
        isOpen={isReviewOpen}
        onClose={() => setIsReviewOpen(false)}
        metrics={data ? {
          timeRange: timeRange === 'all' ? '全部时间' : timeRange === 'today' ? '今天' : timeRange === 'week' ? '近7天' : timeRange === '30days' ? '近30天' : timeRange === 'month' ? '本月' : timeRange === 'quarter' ? '本季度' : `${startDate} ~ ${endDate}`,
          totalTokens: data.totals.total_tokens,
          totalCostUsd: data.totals.total_cost,
          totalSessions: data.totals.total_sessions,
          cacheHitRate: data.totals.cache_hit_rate,
          thinkingRatio: data.totals.thinking_ratio,
          sourceBreakdown: data.source_trends.length > 0
            ? JSON.stringify(
                Object.entries(
                  data.source_trends.reduce((acc: Record<string, number>, item) => {
                    acc[item.source] = (acc[item.source] || 0) + item.tokens;
                    return acc;
                  }, {})
                ).map(([source, tokens]) => ({ source, tokens })).slice(0, 8)
              )
            : undefined,
          modelDistribution: data.model_distribution.length > 0
            ? JSON.stringify(
                data.model_distribution.slice(0, 6).map(m => ({
                  model: m.model,
                  total_tokens: m.total_tokens,
                }))
              )
            : undefined,
          dailyTrendSummary: data.daily_trends.length > 0
            ? JSON.stringify(
                data.daily_trends.slice(-7).map(d => ({
                  date: d.date,
                  tokens: d.input + d.output + d.cached + d.thinking,
                  sessions: d.sessions,
                }))
              )
            : undefined,
          availableSources: data.source_trends.length > 0
            ? Array.from(new Set(data.source_trends.map((item) => item.source)))
            : [],
        } : null}
      />

      {/* 财务报表生成中的 Loading 遮罩 */}
      {isGeneratingReport && (
        <div className="fixed inset-0 bg-black/60 dark:bg-black/80 backdrop-blur-md z-[10001] flex flex-col items-center justify-center gap-6 animate-fade-in select-none">
          <div className="relative flex flex-col items-center bg-bg-secondary/90 dark:bg-[#0f192b]/95 border border-card-border p-8 rounded-3xl shadow-2xl max-w-sm w-full mx-4 shadow-neon-cyan/5">
            <div className="absolute -top-12 -left-12 w-24 h-24 bg-neon-cyan/15 rounded-full blur-2xl pointer-events-none"></div>
            <div className="absolute -bottom-12 -right-12 w-24 h-24 bg-neon-purple/15 rounded-full blur-2xl pointer-events-none"></div>
            <div className="relative mb-6">
              <div className="w-[56px] h-[56px] border-4 border-slate-200 dark:border-white/5 rounded-full border-t-neon-cyan border-b-neon-purple animate-spin"></div>
              <div className="absolute inset-0.5 rounded-full border border-dashed border-neon-cyan/20 animate-spin-reverse pointer-events-none"></div>
            </div>
            <h3 className="text-base font-bold text-text-primary mb-2 text-center tracking-wide">
              正在生成财务报表...
            </h3>
            <p className="text-xs text-text-secondary text-center leading-relaxed">
              系统正在使用 2x 超清高保真模式为您重绘大盘走势图并渲染财务账单，请稍候
            </p>
          </div>
        </div>
      )}

      {/* 财务报表超清图片预览 Modal */}
      {isReportModalOpen && (
        <div className="fixed inset-0 z-[10000] flex items-center justify-center bg-black/70 dark:bg-black/90 backdrop-blur-md p-4 sm:p-6 animate-fade-in">
          <div className="relative w-full max-w-5xl rounded-[32px] border border-card-border bg-bg-secondary/95 dark:bg-[#0f192b]/95 backdrop-blur-2xl p-6 sm:p-8 text-text-primary shadow-[0_24px_60px_rgba(0,0,0,0.55)] overflow-hidden flex flex-col max-h-[90vh]">
            {/* 装饰性背景光效 */}
            <div className="absolute -top-24 -left-24 w-60 h-60 bg-neon-cyan/15 rounded-full blur-3xl pointer-events-none"></div>
            <div className="absolute -bottom-24 -right-24 w-60 h-60 bg-neon-purple/15 rounded-full blur-3xl pointer-events-none"></div>

            {/* 弹窗头部 */}
            <div className="flex justify-between items-center pb-4 border-b border-card-border mb-5 relative z-10">
              <div>
                <h2 className="text-lg font-bold flex items-center gap-2">
                  <span className="bg-gradient-to-r from-neon-cyan to-neon-purple bg-clip-text text-transparent">🧾 财务报表生成成功</span>
                </h2>
                <p className="text-xs text-text-secondary mt-1">
                  已自动为您生成高清财务报表图片（已忽略交互控件，保留核心账单细节）
                </p>
              </div>
              <button
                onClick={() => setIsReportModalOpen(false)}
                className="w-9 h-9 rounded-full flex items-center justify-center bg-bg-secondary/60 dark:bg-white/5 hover:bg-bg-secondary dark:hover:bg-white/10 text-text-secondary hover:text-text-primary transition-all duration-200 cursor-pointer border border-card-border text-lg"
                title="关闭预览"
              >
                ✕
              </button>
            </div>

            {/* 图片展示区 */}
            <div className="flex-1 overflow-auto rounded-2xl border border-card-border bg-slate-500/5 dark:bg-black/30 p-4 flex justify-center items-start mb-6 relative z-10 shadow-inner group">
              {reportImgUrl ? (
                <div className="relative max-w-full">
                  <img
                    src={reportImgUrl}
                    alt="AI Token Monitor 财务报表"
                    className="rounded-xl shadow-[0_12px_40px_rgba(0,0,0,0.15)] dark:shadow-[0_12px_40px_rgba(0,0,0,0.5)] border border-card-border max-w-full h-auto transition-transform duration-300"
                  />
                  <div className="absolute inset-0 bg-black/0 group-hover:bg-black/10 transition-colors duration-300 rounded-xl flex items-center justify-center pointer-events-none">
                    <span className="bg-black/75 backdrop-blur-sm text-white text-[11px] px-3 py-1.5 rounded-full opacity-0 group-hover:opacity-100 transition-opacity duration-300 shadow-lg">
                      💡 提示：您可直接鼠标右键复制或在下方一键下载保存
                    </span>
                  </div>
                </div>
              ) : (
                <div className="h-60 flex items-center justify-center text-text-muted italic text-xs">
                  图片生成失败，请重试
                </div>
              )}
            </div>

            {/* 弹窗底部操作按钮 */}
            <div className="flex flex-col sm:flex-row items-center justify-end gap-3 pt-4 border-t border-card-border relative z-10 shrink-0">
              <button
                onClick={() => setIsReportModalOpen(false)}
                className="px-5 py-2.5 rounded-xl text-xs font-semibold bg-slate-200/60 hover:bg-slate-300/60 dark:bg-white/5 dark:hover:bg-white/10 text-text-primary border border-card-border cursor-pointer transition-all duration-200 min-w-[100px] text-center"
              >
                关闭预览
              </button>
              {reportImgUrl && (
                <a
                  href={reportImgUrl}
                  download={`AI_Token_Monitor_财务报表_${new Date().toISOString().split('T')[0]}.png`}
                  className="px-6 py-2.5 rounded-xl text-xs font-bold bg-gradient-to-r from-neon-cyan to-neon-purple text-white shadow-[0_4px_15px_rgba(6,182,212,0.25)] hover:scale-105 active:scale-100 transition-all duration-200 cursor-pointer min-w-[150px] text-center flex items-center justify-center gap-1.5"
                >
                  📥 保存财务报表图片
                </a>
              )}
            </div>
          </div>
        </div>
      )}

      {/* 窗口关闭确认 Modal */}
      {showCloseConfirmModal && (
        <div className="fixed inset-0 z-[10000] flex items-center justify-center bg-black/60 dark:bg-black/80 backdrop-blur-md p-4 animate-fade-in select-none">
          <div className="relative w-full max-w-md rounded-[28px] border border-card-border bg-bg-secondary/95 dark:bg-[#0f192b]/95 backdrop-blur-xl p-6 text-text-primary shadow-2xl overflow-hidden shadow-neon-cyan/5 transition-all duration-300">
            {/* 装饰性背景光效 */}
            <div className="absolute -top-20 -left-20 w-44 h-44 bg-neon-cyan/10 rounded-full blur-3xl pointer-events-none"></div>
            <div className="absolute -bottom-20 -right-20 w-44 h-44 bg-neon-purple/10 rounded-full blur-3xl pointer-events-none"></div>

            {/* 弹窗头部 */}
            <div className="flex justify-between items-center pb-3 border-b border-card-border mb-4 relative z-10">
              <h2 className="text-sm font-bold flex items-center gap-2">
                <span className="bg-gradient-to-r from-neon-cyan to-neon-purple bg-clip-text text-transparent">🚪 窗口关闭行为确认</span>
              </h2>
              <button
                onClick={() => setShowCloseConfirmModal(false)}
                className="w-7 h-7 rounded-full flex items-center justify-center bg-bg-secondary/60 dark:bg-white/5 hover:bg-bg-secondary dark:hover:bg-white/10 text-text-secondary hover:text-text-primary transition-all cursor-pointer border border-card-border text-xs"
              >
                ×
              </button>
            </div>

            {/* 弹窗文本描述 */}
            <div className="relative z-10 text-left space-y-3 mb-5">
              <p className="text-xs text-text-primary leading-relaxed font-sans font-medium">
                您点击了主窗口的关闭按钮。为了防止程序意外中断，系统已帮您拦截此行为。
              </p>
              <p className="text-[11px] text-text-secondary leading-relaxed font-sans">
                请选择您想要执行的操作。如果您希望在后台继续监视 AI 的 Token 使用量，建议选择“最小化到后台”。
              </p>
            </div>

            {/* 复选框 - 记住选择 */}
            <div className="relative z-10 flex items-center mb-6 text-left">
              <label className="flex items-center gap-2.5 text-[11px] text-text-secondary cursor-pointer select-none group font-medium">
                <input
                  type="checkbox"
                  checked={dontPromptAgain}
                  onChange={(e) => setDontPromptAgain(e.target.checked)}
                  className="w-3.5 h-3.5 rounded border-card-border accent-neon-cyan cursor-pointer transition-all"
                />
                <span className="group-hover:text-text-primary transition-colors">记住我的选择，以后不再提示确认</span>
              </label>
            </div>

            {/* 弹窗操作按钮 */}
            <div className="relative z-10 flex items-center gap-3.5">
              <button
                type="button"
                onClick={async () => {
                  setShowCloseConfirmModal(false);
                  if (dontPromptAgain) {
                    setCloseBehavior('minimize');
                    try {
                      await fetch(apiUrl('/config/save'), {
                        method: 'POST',
                        headers: { 'Content-Type': 'application/json' },
                        body: JSON.stringify({
                          db_type: dbType,
                          sqlite_path: sqlitePath,
                          pg_host: pgHost,
                          pg_port: pgPort,
                          pg_user: pgUser,
                          pg_password: pgPassword,
                          pg_database: pgDatabase,
                          device_name: deviceName,
                          display_currency: displayCurrency,
                          close_behavior: 'minimize',
                        })
                      });
                    } catch (e) {
                      console.error("保存关闭配置失败", e);
                    }
                  }
                  await invoke('hide_window');
                }}
                className="flex-1 py-2.5 rounded-xl border border-card-border text-[11px] font-bold text-text-secondary hover:text-text-primary hover:border-neon-cyan/40 hover:bg-bg-secondary/80 active:scale-95 transition-all cursor-pointer text-center"
              >
                📥 最小化到托盘
              </button>
              <button
                type="button"
                onClick={async () => {
                  setShowCloseConfirmModal(false);
                  if (dontPromptAgain) {
                    setCloseBehavior('close');
                    try {
                      await fetch(apiUrl('/config/save'), {
                        method: 'POST',
                        headers: { 'Content-Type': 'application/json' },
                        body: JSON.stringify({
                          db_type: dbType,
                          sqlite_path: sqlitePath,
                          pg_host: pgHost,
                          pg_port: pgPort,
                          pg_user: pgUser,
                          pg_password: pgPassword,
                          pg_database: pgDatabase,
                          device_name: deviceName,
                          display_currency: displayCurrency,
                          close_behavior: 'close',
                        })
                      });
                    } catch (e) {
                      console.error("保存关闭配置失败", e);
                    }
                  }
                  await invoke('exit_app');
                }}
                className="flex-1 py-2.5 rounded-xl bg-gradient-to-r from-neon-cyan to-neon-purple hover:shadow-[0_0_15px_rgba(6,182,212,0.3)] hover:scale-105 active:scale-95 text-[11px] font-bold text-white transition-all cursor-pointer text-center"
              >
                🚪 直接退出程序
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
