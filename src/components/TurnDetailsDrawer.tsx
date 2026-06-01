import { useEffect, useState } from 'react';

interface TurnDetailsDrawerProps {
  isOpen: boolean;
  onClose: () => void;
  source: string;
  uuid: string;
  idx: number;
}

interface FailedCommand {
  command: string;
  exit_code: number;
  stderr: string;
}

interface TurnDetailsData {
  source: string;
  uuid: string;
  idx: number;
  user_prompt: string | null;
  executed_commands: string | null; // JSON string Array
  failed_commands: string | null;   // JSON string of FailedCommand Array
  modified_files: string | null;    // JSON string Array
}

export function TurnDetailsDrawer({ isOpen, onClose, source, uuid, idx }: TurnDetailsDrawerProps) {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [data, setData] = useState<TurnDetailsData | null>(null);

  useEffect(() => {
    if (!isOpen || !source || !uuid) return;

    const fetchDetails = async () => {
      setLoading(true);
      setError(null);
      setData(null);
      try {
        const response = await fetch(
          `http://localhost:19362/api/review/turns/details?source=${source}&uuid=${uuid}&idx=${idx}`
        );
        if (!response.ok) {
          throw new Error('未找到该轮次的执行细节记录');
        }
        const json = await response.json();
        setData(json);
      } catch (err: any) {
        setError(err.message || '获取执行明细失败');
      } finally {
        setLoading(false);
      }
    };

    fetchDetails();
  }, [isOpen, source, uuid, idx]);

  // 解析 JSON 字段的辅助方法
  const getParsedList = (jsonStr: string | null | undefined): string[] => {
    if (!jsonStr) return [];
    try {
      return JSON.parse(jsonStr);
    } catch {
      return [];
    }
  };

  const getParsedFailedCommands = (jsonStr: string | null | undefined): FailedCommand[] => {
    if (!jsonStr) return [];
    try {
      return JSON.parse(jsonStr);
    } catch {
      return [];
    }
  };

  const executedCommands = getParsedList(data?.executed_commands);
  const failedCommands = getParsedFailedCommands(data?.failed_commands);
  const modifiedFiles = getParsedList(data?.modified_files);

  return (
    <>
      {/* 遮罩层 */}
      {isOpen && (
        <div
          className="fixed inset-0 bg-black/40 backdrop-blur-[2px] z-[99] transition-opacity duration-300"
          onClick={onClose}
        />
      )}

      {/* 抽屉容器 */}
      <div
        className={`fixed right-0 top-0 h-full w-[460px] max-w-[90vw] bg-slate-900/95 dark:bg-slate-950/98 backdrop-blur-lg border-l border-white/10 z-[100] shadow-2xl transition-transform duration-300 ease-out flex flex-col ${
          isOpen ? 'translate-x-0' : 'translate-x-full'
        }`}
      >
        {/* 头部 */}
        <div className="p-4 border-b border-white/10 flex justify-between items-center bg-slate-900/60 dark:bg-slate-950/60">
          <div>
            <h3 className="text-sm font-semibold text-white flex items-center gap-2">
              <span className="w-2 h-2 rounded-full bg-neon-cyan animate-pulse"></span>
              调试溯源明细
            </h3>
            <p className="text-[10px] text-text-secondary mt-0.5 font-mono">
              Turn #{idx} • {source === 'claude_code' ? 'Claude Code' : source}
            </p>
          </div>
          <button
            onClick={onClose}
            className="w-7 h-7 rounded-lg hover:bg-white/10 text-text-secondary hover:text-white flex items-center justify-center transition-colors"
          >
            ✕
          </button>
        </div>

        {/* 内容区 */}
        <div className="flex-1 overflow-y-auto p-4 space-y-4">
          {loading && (
            <div className="h-40 flex flex-col items-center justify-center gap-2">
              <div className="w-6 h-6 border-2 border-neon-cyan border-t-transparent rounded-full animate-spin"></div>
              <p className="text-xs text-text-secondary">正在获取本地执行日志...</p>
            </div>
          )}

          {error && (
            <div className="p-4 rounded-xl border border-red-500/20 bg-red-500/5 text-center space-y-2">
              <p className="text-xs text-red-400 font-semibold">⚠️ 读取异常</p>
              <p className="text-[11px] text-text-secondary">{error}</p>
            </div>
          )}

          {!loading && !error && data && (
            <>
              {/* 用户提问卡片 */}
              {data.user_prompt && (
                <div className="space-y-1">
                  <div className="text-[10px] text-text-secondary font-semibold uppercase tracking-wider">
                    💬 用户原始 Prompt
                  </div>
                  <div className="p-3 rounded-xl border border-white/5 bg-white/[0.02] text-xs text-text-primary whitespace-pre-wrap max-h-[160px] overflow-y-auto leading-relaxed scrollbar-thin">
                    {data.user_prompt}
                  </div>
                </div>
              )}

              {/* 修改过的文件 */}
              {modifiedFiles.length > 0 && (
                <div className="space-y-1">
                  <div className="text-[10px] text-text-secondary font-semibold uppercase tracking-wider">
                    📁 影响/修改的文件 ({modifiedFiles.length})
                  </div>
                  <div className="flex flex-wrap gap-1.5">
                    {modifiedFiles.map((file, i) => {
                      const baseName = file.split(/[/\\]/).pop() || file;
                      return (
                        <span
                          key={i}
                          title={file}
                          className="px-2 py-1 rounded bg-neon-cyan/5 border border-neon-cyan/10 text-[10px] text-neon-cyan font-mono hover:bg-neon-cyan/10 transition-colors"
                        >
                          {baseName}
                        </span>
                      );
                    })}
                  </div>
                </div>
              )}

              {/* 终端报错日志 (Failed Commands) */}
              {failedCommands.length > 0 && (
                <div className="space-y-2.5">
                  <div className="text-[10px] text-text-secondary font-semibold uppercase tracking-wider">
                    🚨 终端执行报错明细
                  </div>
                  {failedCommands.map((fail, i) => (
                    <div key={i} className="rounded-xl border border-red-500/10 overflow-hidden shadow-sm">
                      <div className="bg-red-500/10 px-3 py-1.5 border-b border-red-500/10 flex justify-between items-center">
                        <span className="text-[11px] font-mono text-red-400 font-semibold">
                          $ {fail.command}
                        </span>
                        <span className="text-[9px] bg-red-500/20 text-red-400 px-1.5 py-0.5 rounded font-mono font-bold">
                          Exit Code: {fail.exit_code}
                        </span>
                      </div>
                      <div className="bg-slate-950 p-3 font-mono text-[10px] text-red-400/90 overflow-x-auto whitespace-pre leading-relaxed border-t border-black/40 max-h-[300px]">
                        {fail.stderr || '[未截获到具体错误终端输出]'}
                      </div>
                    </div>
                  ))}
                </div>
              )}

              {/* 执行过的所有命令字汇总 */}
              {executedCommands.length > 0 && (
                <div className="space-y-1">
                  <div className="text-[10px] text-text-secondary font-semibold uppercase tracking-wider">
                    🛠️ 本轮运行的全部命令
                  </div>
                  <div className="flex flex-wrap gap-1.5 font-mono text-[10px]">
                    {executedCommands.map((cmd, i) => (
                      <span
                        key={i}
                        className="px-2 py-0.5 rounded bg-white/5 border border-white/5 text-text-primary"
                      >
                        {cmd}
                      </span>
                    ))}
                  </div>
                </div>
              )}
            </>
          )}
        </div>

        {/* 底部 */}
        <div className="p-3 border-t border-white/10 bg-slate-900/60 dark:bg-slate-950/60 text-center">
          <p className="text-[10px] text-text-muted">
            数据提取自本地 IDE 缓冲层 • 保证代码隐私安全
          </p>
        </div>
      </div>
    </>
  );
}
