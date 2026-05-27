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
  ChevronRight
} from 'lucide-react';
import { DailyTrendChart } from './components/charts/DailyTrendChart';

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

interface AggregatedMetrics {
  totals: Totals;
  daily_trends: DailyTrend[];
  monthly_summary: MonthlySummary[];
  model_distribution: ModelDistribution[];
  sessions: SessionItem[];
}

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
          fetchData(source);
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
  const fetchData = async (currentSource = source) => {
    setLoading(true);
    setRefreshSpin(true);
    try {
      const response = await fetch(`/api/metrics?source=${currentSource}&t=${Date.now()}`);
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
    fetchData(source);
  }, [source]);

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
            <select
              value={source}
              onChange={(e: any) => setSource(e.target.value)}
              className="bg-bg-secondary/60 dark:bg-[#0b1528] border border-card-border rounded-xl px-3 py-2 text-xs font-semibold text-text-primary outline-none focus:border-neon-cyan focus:shadow-[0_0_10px_rgba(6,182,212,0.25)] hover:border-neon-cyan/50 transition-all duration-300 cursor-pointer h-10"
            >
              <option value="all">🌐 全部来源 (All)</option>
              <option value="antigravity">🤖 Antigravity (Gemini)</option>
              <option value="claude_code">🎯 Claude Code</option>
              <option value="codex">🔮 Codex CLI</option>
            </select>
            
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
            <h2 className="text-base font-semibold text-text-primary">每日用量走势 (Token 堆叠柱状图)</h2>
          </div>
          <div className="w-full">
            {data?.daily_trends && data.daily_trends.length > 0 ? (
              <DailyTrendChart data={data.daily_trends} theme={theme} />
            ) : (
              <div className="h-[350px] flex items-center justify-center text-text-muted italic">暂无趋势图表数据</div>
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
                            <span className="text-[10px] font-bold px-2 py-0.5 rounded-full bg-neon-purple/15 border border-neon-purple/35 text-neon-purple leading-none">
                              Antigravity
                            </span>
                          )}
                          {s.source === 'claude_code' && (
                            <span className="text-[10px] font-bold px-2 py-0.5 rounded-full bg-orange-500/15 border border-orange-500/35 text-orange-500 leading-none">
                              Claude Code
                            </span>
                          )}
                          {s.source === 'codex' && (
                            <span className="text-[10px] font-bold px-2 py-0.5 rounded-full bg-neon-cyan/15 border border-neon-cyan/35 text-neon-cyan leading-none">
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

          <div className="glass-card rounded-[32px] max-w-[500px] w-full p-8 flex flex-col items-center text-center gap-6 border border-white/10 dark:border-white/5 shadow-[0_20px_50px_rgba(0,0,0,0.3)]">
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
    </div>
  );
}
