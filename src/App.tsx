import { useState, useEffect, useMemo } from 'react';
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
  Settings
} from 'lucide-react';
import { DailyTrendChart } from './components/charts/DailyTrendChart';
import { SourceTrendChart } from './components/charts/SourceTrendChart';

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

interface AggregatedMetrics {
  totals: Totals;
  daily_trends: DailyTrend[];
  monthly_summary: MonthlySummary[];
  model_distribution: ModelDistribution[];
  sessions: SessionItem[];
  source_trends: SourceTrendItem[];
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

export default function App() {
  const [data, setData] = useState<AggregatedMetrics | null>(null);
  const [loading, setLoading] = useState(false);
  const [refreshSpin, setRefreshSpin] = useState(false);
  const [lastUpdate, setLastUpdate] = useState('--:--:--');
  const [searchKeyword, setSearchKeyword] = useState('');
  const [hideZero, setHideZero] = useState(true);
  const [sortField, setSortField] = useState<keyof SessionItem | 'total'>('created_at');
  const [sortOrder, setSortOrder] = useState<'asc' | 'desc'>('desc');
  const [currentPage, setCurrentPage] = useState(1);
  const [pageSize, setPageSize] = useState(10);
  const [source, setSource] = useState<'all' | 'antigravity' | 'claude_code' | 'codex'>('all');
  const [isSourceDropdownOpen, setIsSourceDropdownOpen] = useState(false);
  const [timeRange, setTimeRange] = useState<'all' | 'today' | 'week' | '30days' | 'month' | 'quarter' | 'custom'>('30days');
  const [startDate, setStartDate] = useState<string>(getDateBounds('30days').start);
  const [endDate, setEndDate] = useState<string>(getDateBounds('30days').end);
  const [chartDimension, setChartDimension] = useState<'type' | 'source'>('type');

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

  // 当配置弹窗打开时，拉取后端最新配置并回显
  useEffect(() => {
    if (isConfigOpen) {
      const loadConfig = async () => {
        try {
          const res = await fetch(`/api/config?t=${Date.now()}`);
          if (res.ok) {
            const data = await res.json();
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
          }
        } catch (e) {
          console.error("加载数据源配置失败", e);
        }
      };
      loadConfig();
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

  // 数字格式化
  const formatNum = (num: number) => new Intl.NumberFormat('zh-CN').format(num || 0);

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
  } | null>(null);

  // 轮询扫描状态
  const pollScanStatus = async () => {
    try {
      const response = await fetch(`/api/scan/status?t=${Date.now()}`);
      if (response.ok) {
        const status = await response.json();
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
      const response = await fetch(`/api/scan/start?t=${Date.now()}`);
      if (response.ok) {
        const status = await response.json();
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
    setLoading(true);
    setRefreshSpin(true);
    try {
      const response = await fetch(`/api/metrics?source=${currentSource}&start_date=${start}&end_date=${end}&t=${Date.now()}`);
      if (!response.ok) throw new Error(`HTTP error! status: ${response.status}`);
      const result: AggregatedMetrics = await response.json();
      setData(result);
      const now = new Date();
      setLastUpdate(now.toTimeString().split(' ')[0]);
    } catch (error) {
      console.error('Fetch data failed:', error);
    } finally {
      setLoading(false);
      setRefreshSpin(false);
    }
  };

  // 手动点击刷新同步按钮
  const handleSyncClick = async () => {
    if (scanStatus?.is_scanning) return;
    setRefreshSpin(true);
    await startScan();
  };

  useEffect(() => {
    // 自动启动后台扫描
    startScan();
  }, []);

  useEffect(() => {
    fetchData(source, startDate, endDate);
  }, [source, startDate, endDate]);

  // 排序字段切换
  const handleSort = (field: keyof SessionItem | 'total') => {
    if (sortField === field) {
      setSortOrder(sortOrder === 'desc' ? 'asc' : 'desc');
    } else {
      setSortField(field);
      setSortOrder('desc');
    }
  };

  // 会话列表过滤与排序
  const filteredAndSortedSessions = useMemo(() => {
    if (!data?.sessions) return [];
    
    const kw = searchKeyword.toLowerCase().trim();
    
    // 过滤
    let result = data.sessions.filter(s => {
      if (hideZero && (s.input + s.output) === 0) {
        return false;
      }
      return (
        s.title.toLowerCase().includes(kw) ||
        s.uuid.toLowerCase().includes(kw) ||
        s.models.some(m => m.toLowerCase().includes(kw))
      );
    });

    // 排序
    result.sort((a, b) => {
      let valA: any, valB: any;
      if (sortField === 'total') {
        valA = a.input + a.output;
        valB = b.input + b.output;
      } else if (sortField === 'models') {
        valA = a.models.join(',');
        valB = b.models.join(',');
      } else if (sortField === 'created_at') {
        valA = new Date(a.created_at).getTime();
        valB = new Date(b.created_at).getTime();
      } else {
        valA = a[sortField];
        valB = b[sortField];
      }

      if (valA < valB) return sortOrder === 'asc' ? -1 : 1;
      if (valA > valB) return sortOrder === 'asc' ? 1 : -1;
      return 0;
    });

    return result;
  }, [data?.sessions, searchKeyword, hideZero, sortField, sortOrder]);

  // 分页数据切片
  const paginatedSessions = useMemo(() => {
    const startIndex = (currentPage - 1) * pageSize;
    return filteredAndSortedSessions.slice(startIndex, startIndex + pageSize);
  }, [filteredAndSortedSessions, currentPage, pageSize]);

  // 分页计算与辅助函数
  const totalItems = filteredAndSortedSessions.length;
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
      {/* 背景光效 */}
      <div className="background-decor-1 bg-decor-cyan animate-pulse-glow fixed -top-48 -left-24 w-[600px] h-[600px] rounded-full blur-[80px] z-[-1] pointer-events-none"></div>
      <div className="background-decor-2 bg-decor-purple animate-pulse-glow-reverse fixed -bottom-72 -right-24 w-[700px] h-[700px] rounded-full blur-[100px] z-[-1] pointer-events-none"></div>

      <div className="max-w-[1400px] mx-auto p-6 flex flex-col gap-6">
        {/* 头部导航栏 */}
        <header className="dashboard-header-bg glass-card flex flex-col md:flex-row justify-between items-center px-7 py-4 gap-4">
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
              <h1 className="text-2xl font-bold bg-gradient-to-r from-neon-cyan to-neon-purple bg-clip-text text-transparent tracking-tight">AI Token Monitor</h1>
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
                  <span>
                    {source === 'all' && '全部来源 (All)'}
                    {source === 'antigravity' && 'Antigravity'}
                    {source === 'claude_code' && 'Claude Code'}
                    {source === 'codex' && 'Codex CLI'}
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
                  <div className="absolute right-0 mt-2 w-48 bg-bg-secondary/95 dark:bg-[#0f192b]/95 border border-card-border rounded-xl shadow-[0_10px_35px_rgba(0,0,0,0.35)] backdrop-blur-md z-50 py-1.5 flex flex-col gap-0.5 animate-fade-in">
                    {[
                      { value: 'all', label: '全部来源 (All)', icon: <Globe className="w-3.5 h-3.5 text-neon-cyan" /> },
                      { value: 'antigravity', label: 'Antigravity', icon: <GeminiIcon className="w-3.5 h-3.5 text-[#8b5cf6]" /> },
                      { value: 'claude_code', label: 'Claude Code', icon: <ClaudeIcon className="w-3.5 h-3.5 text-[#d97757]" /> },
                      { value: 'codex', label: 'Codex CLI', icon: <OpenAIIcon className="w-3.5 h-3.5 text-[#10a37f]" /> },
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
              className={`flex items-center gap-2 text-sm font-semibold bg-gradient-to-r from-neon-cyan to-neon-purple hover:scale-105 active:scale-100 hover:shadow-neon-cyan/35 text-white px-5 py-2.5 rounded-xl transition-all duration-300 ${
                loading || scanStatus?.is_scanning ? 'opacity-70 cursor-not-allowed' : 'cursor-pointer'
              }`}
            >
              <RefreshCw className={`w-4 h-4 ${refreshSpin || scanStatus?.is_scanning ? 'animate-spin' : ''}`} />
              <span>{scanStatus?.is_scanning ? '正在同步...' : '同步刷新'}</span>
            </button>
          </div>
        </header>

        {/* 时间筛选控制栏 */}
        <section className="glass-card p-4 flex flex-col md:flex-row justify-between items-center gap-4 border border-card-border bg-bg-secondary/20 shadow-sm backdrop-blur-md rounded-[20px]">
          <div className="flex flex-wrap items-center gap-2">
            <span className="text-xs font-semibold text-text-secondary mr-2">🕒 时间区间：</span>
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
                className={`px-4 py-1.5 rounded-xl text-xs font-semibold hover:scale-105 active:scale-100 transition-all duration-200 cursor-pointer ${
                  timeRange === item.key
                    ? 'bg-gradient-to-r from-neon-cyan to-neon-purple text-white shadow-[0_4px_12px_rgba(6,182,212,0.2)]'
                    : 'bg-bg-secondary/40 dark:bg-white/3 border border-card-border text-text-secondary hover:text-text-primary hover:border-neon-cyan/40'
                }`}
              >
                {item.label}
              </button>
            ))}
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
          <div className="glass-card rounded-[24px] p-5 flex flex-col gap-3 border border-neon-cyan/20 bg-neon-cyan/5 shadow-[0_8px_32px_rgba(6,182,212,0.08)]">
            <div className="flex justify-between items-center text-sm">
              <div className="flex items-center gap-2">
                <RefreshCw className="w-4 h-4 text-neon-cyan animate-spin" />
                <span className="font-semibold text-text-primary">正在增量同步历史会话数据...</span>
              </div>
              <span className="font-mono text-xs text-text-secondary font-semibold">
                {scanStatus.scanned_files} / {scanStatus.total_files} ({scanStatus.total_files > 0 ? Math.round((scanStatus.scanned_files / scanStatus.total_files) * 100) : 0}%)
              </span>
            </div>
            <div className="h-2 w-full bg-slate-200/50 dark:bg-white/5 rounded-full overflow-hidden border border-card-border relative">
              <div
                className="h-full rounded-full bg-gradient-to-r from-neon-cyan to-neon-purple shadow-[0_0_8px_rgba(6,182,212,0.4)] transition-all duration-300"
                style={{ width: `${scanStatus.total_files > 0 ? (scanStatus.scanned_files / scanStatus.total_files) * 100 : 0}%` }}
              ></div>
            </div>
          </div>
        )}

        {/* 扫描错误提示 */}
        {scanStatus && scanStatus.error && (
          <div className="glass-card rounded-[24px] p-4 border border-red-500/20 bg-red-500/5 text-red-400 flex items-center justify-between text-sm">
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
        <section className="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 xl:grid-cols-5 gap-5">
          {/* 估计总消费 */}
          <div className="kpi-card kpi-pink glass-card p-5 flex justify-between items-center group bg-gradient-to-br from-neon-pink/10 to-neon-purple/5 border-neon-pink/20 hover:border-neon-pink/40 shadow-[0_8px_30px_rgba(236,72,153,0.04)]">
            <div className="flex flex-col">
              <span className="text-xs text-text-secondary font-medium mb-1">估算总费用</span>
              <h2 className="text-2xl font-bold font-mono tracking-tight text-neon-pink mb-0.5">${totals ? totals.total_cost.toFixed(3) : '0.000'}</h2>
              <span className="text-[9px] font-semibold text-text-muted tracking-wider uppercase">Est. Total Cost</span>
            </div>
            <div className="w-12 h-12 rounded-xl flex items-center justify-center bg-neon-pink/15 text-neon-pink border border-neon-pink/30 group-hover:scale-110 transition-transform duration-300">
              <span className="text-lg font-bold font-mono">$</span>
            </div>
          </div>

          {/* 总消耗 */}
          <div className="kpi-card kpi-blue glass-card p-5 flex justify-between items-center group">
            <div className="flex flex-col">
              <span className="text-xs text-text-secondary font-medium mb-1">总消耗 Token</span>
              <h2 className="text-2xl font-semibold font-mono tracking-tight text-text-primary mb-0.5">{totals ? formatNum(totals.total_tokens) : 0}</h2>
              <span className="text-[9px] font-semibold text-text-muted tracking-wider uppercase">Total Tokens</span>
            </div>
            <div className="w-12 h-12 rounded-xl flex items-center justify-center bg-neon-purple/15 text-neon-purple border border-neon-purple/30 group-hover:scale-110 transition-transform duration-300">
              <Compass className="w-6 h-6" />
            </div>
          </div>

          {/* 输入 */}
          <div className="kpi-card kpi-blue glass-card p-5 flex justify-between items-center group">
            <div className="flex flex-col">
              <span className="text-xs text-text-secondary font-medium mb-1">输入 Token</span>
              <h2 className="text-2xl font-semibold font-mono tracking-tight text-text-primary mb-0.5">{totals ? formatNum(totals.total_input) : 0}</h2>
              <span className="text-[9px] font-semibold text-text-muted tracking-wider uppercase">Total Prompt</span>
            </div>
            <div className="w-12 h-12 rounded-xl flex items-center justify-center bg-neon-cyan/15 text-neon-cyan border border-neon-cyan/30 group-hover:scale-110 transition-transform duration-300">
              <ArrowDown className="w-6 h-6" />
            </div>
          </div>

          {/* 输出 */}
          <div className="kpi-card kpi-blue glass-card p-5 flex justify-between items-center group">
            <div className="flex flex-col">
              <span className="text-xs text-text-secondary font-medium mb-1">输出 Token</span>
              <h2 className="text-2xl font-semibold font-mono tracking-tight text-text-primary mb-0.5">{totals ? formatNum(totals.total_output) : 0}</h2>
              <span className="text-[9px] font-semibold text-text-muted tracking-wider uppercase">Total Candidates</span>
            </div>
            <div className="w-12 h-12 rounded-xl flex items-center justify-center bg-neon-pink/15 text-neon-pink border border-neon-pink/30 group-hover:scale-110 transition-transform duration-300">
              <ArrowUp className="w-6 h-6" />
            </div>
          </div>

          {/* 缓存命中率 */}
          <div className="kpi-card kpi-cyan glass-card p-5 flex justify-between items-center group">
            <div className="flex flex-col">
              <span className="text-xs text-text-secondary font-medium mb-1">缓存命中率</span>
              <h2 className="text-2xl font-semibold font-mono tracking-tight text-text-primary mb-0.5">{totals ? formatPercent(totals.cache_hit_rate) : '0.0%'}</h2>
              <span className="text-[9px] font-semibold text-text-muted tracking-wider uppercase">Cache Hit Rate</span>
            </div>
            <div className="w-12 h-12 rounded-xl flex items-center justify-center bg-neon-blue/15 text-neon-blue border border-neon-blue/30 group-hover:scale-110 transition-transform duration-300">
              <Database className="w-6 h-6" />
            </div>
          </div>

          {/* 推理 Token 占比 */}
          <div className="kpi-card kpi-cyan glass-card p-5 flex justify-between items-center group">
            <div className="flex flex-col">
              <span className="text-xs text-text-secondary font-medium mb-1">推理 Token 占比</span>
              <h2 className="text-2xl font-semibold font-mono tracking-tight text-text-primary mb-0.5">{totals ? formatPercent(totals.thinking_ratio) : '0.0%'}</h2>
              <span className="text-[9px] font-semibold text-text-muted tracking-wider uppercase">Thinking Ratio</span>
            </div>
            <div className="w-12 h-12 rounded-xl flex items-center justify-center bg-neon-purple/15 text-neon-purple border border-neon-purple/30 group-hover:scale-110 transition-transform duration-300">
              <Brain className="w-6 h-6" />
            </div>
          </div>

          {/* 缓存 Token 数 */}
          <div className="kpi-card kpi-cyan glass-card p-5 flex justify-between items-center group">
            <div className="flex flex-col">
              <span className="text-xs text-text-secondary font-medium mb-1">缓存命中数</span>
              <h2 className="text-2xl font-semibold font-mono tracking-tight text-text-primary mb-0.5">{totals ? formatNum(totals.total_cached) : 0}</h2>
              <span className="text-[9px] font-semibold text-text-muted tracking-wider uppercase">Cached Tokens</span>
            </div>
            <div className="w-12 h-12 rounded-xl flex items-center justify-center bg-neon-green/15 text-neon-green border border-neon-green/30 group-hover:scale-110 transition-transform duration-300">
              <Cpu className="w-6 h-6" />
            </div>
          </div>

          {/* 推理 Token 数 */}
          <div className="kpi-card kpi-purple glass-card p-5 flex justify-between items-center group">
            <div className="flex flex-col">
              <span className="text-xs text-text-secondary font-medium mb-1">推理消耗数</span>
              <h2 className="text-2xl font-semibold font-mono tracking-tight text-text-primary mb-0.5">{totals ? formatNum(totals.total_thinking) : 0}</h2>
              <span className="text-[9px] font-semibold text-text-muted tracking-wider uppercase">Thinking Tokens</span>
            </div>
            <div className="w-12 h-12 rounded-xl flex items-center justify-center bg-neon-purple/15 text-neon-purple border border-neon-purple/30 group-hover:scale-110 transition-transform duration-300">
              <Hash className="w-6 h-6" />
            </div>
          </div>

          {/* 总会话数 */}
          <div className="kpi-card kpi-slate glass-card p-5 flex justify-between items-center group">
            <div className="flex flex-col">
              <span className="text-xs text-text-secondary font-medium mb-1">总会话数</span>
              <h2 className="text-2xl font-semibold font-mono tracking-tight text-text-primary mb-0.5">{totals ? formatNum(totals.total_sessions) : 0}</h2>
              <span className="text-[9px] font-semibold text-text-muted tracking-wider uppercase">Total Sessions</span>
            </div>
            <div className="w-12 h-12 rounded-xl flex items-center justify-center bg-neon-teal/15 text-neon-teal border border-neon-teal/30 group-hover:scale-110 transition-transform duration-300">
              <MessageSquare className="w-6 h-6" />
            </div>
          </div>
        </section>

        {/* 每日趋势图 */}
        <section className="chart-section glass-card p-6">
          <div className="section-header flex flex-col sm:flex-row justify-between items-start sm:items-center pb-3 mb-5 border-b border-card-border gap-3">
            <h2 className="text-base font-semibold text-text-primary">
              {source === 'all' && chartDimension === 'source' ? '各引擎每日用量对比走势 (Token 堆叠柱状图)' : '每日用量走势 (Token 堆叠柱状图)'}
            </h2>
            
            {source === 'all' && (
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
                <button
                  onClick={() => setChartDimension('source')}
                  className={`px-3 py-1.5 rounded-lg text-xs font-semibold hover:scale-105 active:scale-100 transition-all duration-200 cursor-pointer ${
                    chartDimension === 'source'
                      ? 'bg-gradient-to-r from-neon-cyan to-neon-purple text-white shadow-[0_4px_10px_rgba(6,182,212,0.15)]'
                      : 'text-text-secondary hover:text-text-primary hover:bg-white/5'
                  }`}
                >
                  🤖 来源维度
                </button>
              </div>
            )}
          </div>
          <div className="w-full">
            {source === 'all' && chartDimension === 'source' ? (
              data?.source_trends && data.source_trends.length > 0 ? (
                <SourceTrendChart data={data.source_trends} theme={theme} />
              ) : (
                <div className="h-[350px] flex items-center justify-center text-text-muted italic">暂无趋势图表数据</div>
              )
            ) : (
              data?.daily_trends && data.daily_trends.length > 0 ? (
                <DailyTrendChart data={data.daily_trends} theme={theme} />
              ) : (
                <div className="h-[350px] flex items-center justify-center text-text-muted italic">暂无趋势图表数据</div>
              )
            )}
          </div>
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
                        <span className="font-mono text-text-secondary">{formatNum(m.total_tokens)} Tokens</span>
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
              <h2 className="text-sm font-semibold text-text-primary">按月用量汇总</h2>
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
                        <td className="font-mono text-right py-2.5">{formatNum(row.sessions)}</td>
                        <td className="font-mono text-right py-2.5">{formatNum(row.input)}</td>
                        <td className="font-mono text-right py-2.5">{formatNum(row.output)}</td>
                        <td className="font-mono text-right py-2.5">{formatNum(row.cached)}</td>
                        <td className="font-mono text-right py-2.5">{formatNum(row.thinking)}</td>
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

        {/* 会话明细 */}
        <section className="glass-card p-6">
          <div className="flex flex-col md:flex-row justify-between items-start md:items-center gap-4 pb-4 mb-5 border-b border-card-border">
            <h2 className="text-base font-semibold text-text-primary">会话用量明细</h2>
            <div className="flex flex-col sm:flex-row items-stretch sm:items-center gap-4 w-full md:w-auto">
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
                    <span className="flex items-center gap-1">统计来源 <ChevronsUpDown className="w-3 h-3 text-text-muted" /></span>
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
                  <th onClick={() => handleSort('cost_usd')} className="sortable text-right py-3 cursor-pointer hover:text-neon-cyan transition-colors">
                    <span className="flex items-center justify-end gap-1">估算费用 <ChevronsUpDown className="w-3 h-3 text-text-muted" /></span>
                  </th>
                  <th onClick={() => handleSort('total')} className="sortable text-right py-3 cursor-pointer hover:text-neon-cyan transition-colors">
                    <span className="flex items-center justify-end gap-1">总计 Token <ChevronsUpDown className="w-3 h-3 text-text-muted" /></span>
                  </th>
                </tr>
              </thead>
              <tbody>
                {filteredAndSortedSessions.length > 0 ? (
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
                        <td className="font-mono text-right py-3">{formatNum(s.input)}</td>
                        <td className="font-mono text-right py-3">{formatNum(s.output)}</td>
                        <td className="font-mono text-right py-3">{formatNum(s.cached)}</td>
                        <td className="font-mono text-right py-3">{formatNum(s.thinking)}</td>
                        <td className="font-mono text-right text-xs text-text-secondary py-3">${s.cost_usd.toFixed(3)}</td>
                        <td className="font-mono text-right font-bold text-neon-cyan py-3">{formatNum(totalTokens)}</td>
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
          {filteredAndSortedSessions.length > 0 && (
            <div className="flex flex-col sm:flex-row justify-between items-center gap-4 mt-6 pt-5 border-t border-card-border select-none">
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
                  <option value={10}>10 条/页</option>
                  <option value={20}>20 条/页</option>
                  <option value={50}>50 条/页</option>
                  <option value={100}>100 条/页</option>
                </select>
              </div>
            </div>
          )}
        </section>
      </div>

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
                正在进行首次数据同步，系统正在扫描并解码历史会话中的 Token 消耗明细，请稍候...
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
            
            <span className="text-[10px] text-text-muted italic">这通常仅在首次启动或有大量新会话时需要较长时间</span>
          </div>
        </div>
      )}

      {/* 全局加载遮罩 */}
      {loading && !data && !(scanStatus && scanStatus.is_scanning) && (
        <div className="fixed inset-0 bg-bg-app/85 backdrop-blur-md z-[9999] flex flex-col items-center justify-center gap-5">
          <div className="w-[50px] h-[50px] border-4 border-neon-cyan/10 rounded-full border-t-neon-cyan border-b-neon-purple animate-spin"></div>
          <p className="text-sm font-semibold text-text-secondary tracking-wide">正在拉取大盘缓存指标数据...</p>
        </div>
      )}

      {/* 数据库数据源配置弹窗 */}
      {isConfigOpen && (
        <div className="fixed inset-0 z-[10000] flex items-center justify-center bg-black/60 dark:bg-black/80 backdrop-blur-sm p-4 animate-fade-in">
          <div className="relative w-full max-w-lg rounded-3xl border border-card-border bg-bg-secondary/95 dark:bg-[#0f192b]/95 backdrop-blur-xl p-6 text-text-primary shadow-2xl overflow-hidden shadow-neon-cyan/5">
            {/* 装饰性背景光效 */}
            <div className="absolute -top-24 -left-24 w-48 h-48 bg-neon-cyan/20 rounded-full blur-3xl pointer-events-none"></div>
            <div className="absolute -bottom-24 -right-24 w-48 h-48 bg-neon-purple/20 rounded-full blur-3xl pointer-events-none"></div>

            <div className="flex justify-between items-center pb-4 border-b border-card-border mb-5 relative z-10">
              <h2 className="text-lg font-bold flex items-center gap-2">
                <Settings className="w-5 h-5 text-neon-cyan animate-spin-slow" />
                <span className="bg-gradient-to-r from-neon-cyan to-neon-purple bg-clip-text text-transparent">系统数据源配置</span>
              </h2>
              <button
                onClick={() => {
                  setIsConfigOpen(false);
                  setConfigMessage(null);
                }}
                className="w-8 h-8 rounded-full flex items-center justify-center bg-bg-secondary/60 dark:bg-white/5 hover:bg-bg-secondary dark:hover:bg-white/10 text-text-secondary hover:text-text-primary transition-all cursor-pointer border border-card-border"
              >
                ×
              </button>
            </div>

            <div className="space-y-5 relative z-10">
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
                <div className="flex flex-col gap-2 animate-fade-in">
                  <label className="text-xs font-semibold text-text-secondary">📂 自定义数据库物理路径</label>
                  <input
                    type="text"
                    value={sqlitePath}
                    onChange={(e) => setSqlitePath(e.target.value)}
                    placeholder="请输入绝对路径，例如 D:\data\stats.db"
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
                          className="absolute right-3 top-2 flex items-center justify-center text-xs text-text-muted hover:text-text-primary focus:outline-none bg-transparent border-none cursor-pointer select-none"
                        >
                          {showPassword ? "👁️" : "👁️‍🗨️"}
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
                      const response = await fetch('/api/config/test', {
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
                        const res = await response.json();
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
                      const response = await fetch('/api/config/save', {
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
                        const res = await response.json();
                        if (res.success) {
                          alert(res.message);
                          setIsConfigOpen(false);
                          // 成功保存后，立即拉取新库看板数据，完成免重启即时切换！
                          fetchData(source, startDate, endDate);
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
          </div>
        </div>
      )}
    </div>
  );
}
