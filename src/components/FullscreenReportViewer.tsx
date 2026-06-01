import { useState, useEffect, useRef } from 'react';
import {
  Copy,
  Download,
  Printer,
  X,
  RefreshCw,
  FileText,
} from 'lucide-react';
import { apiUrl, readJsonResponse } from '../lib/api';
import { Markdown } from './Markdown';

interface ReviewTask {
  id: string;
  title: string;
  output_markdown: string;
  cli_name: string;
  created_at: string;
}

interface FullscreenReportViewerProps {
  // 初始 taskId（可为空字符串，等待 Tauri 事件推送）
  taskId: string;
}

export function FullscreenReportViewer({ taskId: initialTaskId }: FullscreenReportViewerProps) {
  const [resolvedTaskId, setResolvedTaskId] = useState<string>(initialTaskId);
  const [task, setTask] = useState<ReviewTask | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const unlistenRef = useRef<(() => void) | null>(null);

  // 强制移出暗黑模式 class 以开启纯净亮色主题
  useEffect(() => {
    document.documentElement.classList.remove('dark');
  }, []);

  // 通过 Tauri 事件监听获取 task_id（处理跨 WebviewWindow 通信）
  useEffect(() => {
    let cancelled = false;

    const setupListener = async () => {
      try {
        const { listen } = await import('@tauri-apps/api/event');
        const unlisten = await listen<string>('fullscreen-task-id', (event) => {
          if (!cancelled && event.payload) {
            setResolvedTaskId(event.payload);
          }
        });
        unlistenRef.current = unlisten;
      } catch (err) {
        console.warn('[FullscreenViewer] 无法注册 Tauri 事件监听:', err);
      }
    };

    // 如果初始 taskId 为空，则等待 Tauri 事件推送
    if (!initialTaskId) {
      // 优先从本地缓存中直接读取，秒开且 100% 可靠
      const cachedTaskId = localStorage.getItem('fullscreen_task_id');
      if (cachedTaskId) {
        setResolvedTaskId(cachedTaskId);
      }

      setupListener();

      // 超时兜底：如果 4 秒内还没收到事件，显示错误
      const timeout = setTimeout(() => {
        if (!cancelled) {
          setResolvedTaskId((prev) => {
            if (!prev) {
              setError('报告 ID 获取超时，请返回主界面重新点击「全屏查看」。');
              setLoading(false);
            }
            return prev;
          });
        }
      }, 4000);

      return () => {
        cancelled = true;
        clearTimeout(timeout);
        unlistenRef.current?.();
      };
    }

    return () => {
      cancelled = true;
      unlistenRef.current?.();
    };
  }, [initialTaskId]);

  // 当 resolvedTaskId 确定后，加载报告数据
  useEffect(() => {
    if (!resolvedTaskId) return;

    let cancelled = false;
    setLoading(true);
    setError(null);

    const loadTask = async () => {
      try {
        const res = await fetch(apiUrl(`/review/tasks/${resolvedTaskId}`));
        if (!res.ok) {
          throw new Error(`加载报告详情失败 (状态码: ${res.status})`);
        }
        const data = await readJsonResponse<ReviewTask>(res);
        if (!cancelled) setTask(data);
      } catch (err: any) {
        console.error('加载全屏报告错误:', err);
        if (!cancelled) setError(err.message || '加载报告时发生未知错误');
      } finally {
        if (!cancelled) setLoading(false);
      }
    };

    loadTask();
    return () => { cancelled = true; };
  }, [resolvedTaskId]);

  const handleCopy = async () => {
    if (!task?.output_markdown) return;
    try {
      await navigator.clipboard.writeText(task.output_markdown);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      alert('复制失败，请手动选择复制。');
    }
  };

  const handleExport = () => {
    if (!task?.output_markdown) return;
    try {
      const blob = new Blob([task.output_markdown], { type: 'text/markdown;charset=utf-8;' });
      const url = URL.createObjectURL(blob);
      const link = document.createElement('a');
      link.href = url;
      const safeTitle = (task.title || 'AI复盘报告').replace(/[\\/:*?"<>|]/g, '_');
      link.setAttribute('download', `${safeTitle}_AI复盘报告.md`);
      document.body.appendChild(link);
      link.click();
      document.body.removeChild(link);
      URL.revokeObjectURL(url);
    } catch {
      alert('导出文件失败。');
    }
  };

  const handlePrint = () => window.print();

  const handleClose = async () => {
    try {
      const { getCurrentWebviewWindow } = await import('@tauri-apps/api/webviewWindow');
      await getCurrentWebviewWindow().close();
    } catch {
      window.close();
    }
  };

  // 等待 task_id 从事件中到来
  if (!resolvedTaskId && loading) {
    return (
      <div className="min-h-screen flex flex-col items-center justify-center bg-slate-50 text-slate-800 gap-3">
        <RefreshCw className="w-8 h-8 text-blue-500 animate-spin" />
        <span className="text-sm font-semibold tracking-wide">正在初始化全屏查看器...</span>
      </div>
    );
  }

  if (loading) {
    return (
      <div className="min-h-screen flex flex-col items-center justify-center bg-slate-50 text-slate-800 gap-3">
        <RefreshCw className="w-8 h-8 text-blue-500 animate-spin" />
        <span className="text-sm font-semibold tracking-wide">正在加载复盘分析诊断报告...</span>
      </div>
    );
  }

  if (error || !task) {
    return (
      <div className="min-h-screen flex flex-col items-center justify-center bg-slate-50 text-slate-800 p-6 text-center gap-4">
        <div className="text-rose-500 font-bold text-lg">⚠️ 加载失败</div>
        <p className="text-sm text-slate-500 max-w-md">{error || '报告数据不存在'}</p>
        <button
          onClick={handleClose}
          className="px-6 py-2.5 bg-rose-50 border border-rose-200 text-rose-600 rounded-xl font-semibold text-xs hover:bg-rose-100 transition-all cursor-pointer"
        >
          关闭窗口
        </button>
      </div>
    );
  }

  const formattedDate = new Date(task.created_at).toLocaleString();

  return (
    <div className="min-h-screen bg-gradient-to-b from-slate-50 to-white text-slate-800 flex flex-col select-text">
      {/* 极简亮色拟态顶栏 */}
      <header className="sticky top-0 z-50 bg-white/80 backdrop-blur-md border-b border-slate-200/80 px-6 py-3.5 flex items-center justify-between shadow-sm no-print">
        <div className="flex items-center gap-2.5">
          <div className="w-8 h-8 rounded-lg bg-blue-500/10 flex items-center justify-center text-blue-600">
            <FileText className="w-4 h-4" />
          </div>
          <div>
            <h1 className="text-xs font-bold text-slate-900 leading-tight">智能复盘分析诊断报告 (全屏查看)</h1>
            <p className="text-[10px] text-slate-400 font-medium font-mono mt-0.5">生成于: {formattedDate}</p>
          </div>
        </div>

        <div className="flex items-center gap-2.5">
          <button
            onClick={handleCopy}
            className="flex items-center gap-1.5 px-3.5 py-2 bg-slate-100/70 border border-slate-200 text-slate-700 rounded-xl font-semibold text-xs hover:bg-slate-200 transition-all duration-200 cursor-pointer"
          >
            <Copy className="w-3.5 h-3.5" />
            {copied ? '已复制！' : '复制全文'}
          </button>
          <button
            onClick={handleExport}
            className="flex items-center gap-1.5 px-3.5 py-2 bg-slate-100/70 border border-slate-200 text-slate-700 rounded-xl font-semibold text-xs hover:bg-slate-200 transition-all duration-200 cursor-pointer"
          >
            <Download className="w-3.5 h-3.5" />
            导出 MD
          </button>
          <button
            onClick={handlePrint}
            className="flex items-center gap-1.5 px-3.5 py-2 bg-slate-100/70 border border-slate-200 text-slate-700 rounded-xl font-semibold text-xs hover:bg-slate-200 transition-all duration-200 cursor-pointer"
          >
            <Printer className="w-3.5 h-3.5" />
            打印/PDF
          </button>
          <button
            onClick={handleClose}
            className="flex items-center gap-1.5 px-3.5 py-2 bg-rose-50 border border-rose-200 text-rose-600 rounded-xl font-bold text-xs hover:bg-rose-100 transition-all duration-200 cursor-pointer"
          >
            <X className="w-3.5 h-3.5" />
            关闭
          </button>
        </div>
      </header>

      {/* 滚动阅读主体区域 */}
      <main className="flex-1 overflow-y-auto px-6 py-10 md:py-14 select-text">
        <div className="max-w-4xl mx-auto bg-white border border-slate-200/60 rounded-[28px] p-8 md:p-12 shadow-[0_12px_48px_rgba(15,23,42,0.03)] transition-all">
          <h2 className="text-xl font-extrabold text-slate-900 border-b-2 border-slate-100 pb-4 mb-6">
            {task.title}
          </h2>
          
          <div className="text-slate-800 leading-relaxed font-sans text-sm">
            <Markdown content={task.output_markdown} />
          </div>
        </div>
      </main>

      <footer className="py-5 text-center text-[10px] text-slate-400 border-t border-slate-100 bg-slate-50/50 no-print select-none">
        提示：您可以按下 Ctrl+P 快捷键或点击顶栏「打印/PDF」按钮将此报告保存为 PDF 电子文档。
      </footer>
    </div>
  );
}
