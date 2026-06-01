import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';

interface MarkdownProps {
  content: string;
  className?: string;
}

export function Markdown({ content, className = '' }: MarkdownProps) {
  return (
    <div className={`prose-custom max-w-none text-left ${className}`}>
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        components={{
          // 安全外部链接打开
          a: ({ node, href, children, ...props }) => (
            <a
              href={href}
              target="_blank"
              rel="noopener noreferrer"
              className="text-neon-cyan hover:underline inline-flex items-center gap-0.5"
              {...props}
            >
              {children}
            </a>
          ),
          // 玻璃拟态表格设计，数值使用 monospace 字体以对齐
          table: ({ node, children, ...props }) => (
            <div className="table-responsive my-4 rounded-xl border border-white/10 dark:border-white/5 overflow-hidden shadow-sm" {...props}>
              <table className="w-full border-collapse text-left text-xs">{children}</table>
            </div>
          ),
          thead: ({ node, children, ...props }) => (
            <thead className="bg-slate-100/50 dark:bg-slate-900/60" {...props}>{children}</thead>
          ),
          th: ({ node, children, ...props }) => (
            <th className="p-2.5 font-semibold text-text-secondary border-b border-card-border" {...props}>
              {children}
            </th>
          ),
          td: ({ node, children, ...props }) => (
            <td className="p-2.5 border-b border-card-border/50 text-text-primary font-mono" {...props}>
              {children}
            </td>
          ),
          tr: ({ node, children, ...props }) => (
            <tr className="hover:bg-slate-50/50 dark:hover:bg-slate-800/10 transition-colors" {...props}>{children}</tr>
          ),
          // 标题使用 Outfit 字体
          h1: ({ node, children, ...props }) => (
            <h1 className="text-sm font-bold mt-5 mb-2.5 text-text-primary border-b border-card-border/50 pb-1 flex items-center gap-2" {...props}>
              {children}
            </h1>
          ),
          h2: ({ node, children, ...props }) => (
            <h2 className="text-xs font-bold mt-4 mb-2 text-text-secondary flex items-center gap-1.5" {...props}>
              <span className="w-1.5 h-3.5 rounded bg-neon-cyan inline-block"></span>
              {children}
            </h2>
          ),
          h3: ({ node, children, ...props }) => (
            <h3 className="text-xs font-bold mt-3 mb-1.5 text-text-muted flex items-center gap-1" {...props}>
              {children}
            </h3>
          ),
          // 段落与列表
          p: ({ node, children, ...props }) => (
            <p className="text-xs leading-relaxed text-text-primary mb-3" {...props}>{children}</p>
          ),
          ul: ({ node, children, ...props }) => (
            <ul className="list-disc pl-5 mb-3 space-y-1 text-xs text-text-secondary" {...props}>{children}</ul>
          ),
          ol: ({ node, children, ...props }) => (
            <ol className="list-decimal pl-5 mb-3 space-y-1 text-xs text-text-secondary" {...props}>{children}</ol>
          ),
          li: ({ node, children, ...props }) => (
            <li className="leading-relaxed text-text-primary" {...props}>{children}</li>
          ),
          // 引用块
          blockquote: ({ node, children, ...props }) => (
            <blockquote className="border-l-4 border-neon-cyan/40 bg-neon-cyan/5 px-4 py-2.5 my-3 rounded-r-lg text-text-muted italic text-xs leading-relaxed" {...props}>
              {children}
            </blockquote>
          ),
          // 块级代码外部预留容器
          pre: ({ node, children, ...props }) => (
            <pre className="p-3.5 rounded-xl font-mono text-[11px] overflow-x-auto bg-slate-950/80 border border-white/5 my-3 text-emerald-400" {...props}>
              {children}
            </pre>
          ),
          // 代码与行内代码
          code: ({ node, className, children, ...props }) => {
            const isInline = !className || !className.startsWith('language-');
            return isInline ? (
              <code className="px-1.5 py-0.5 rounded font-mono text-[11px] text-neon-cyan bg-neon-cyan/5 border border-neon-cyan/10" {...props}>
                {children}
              </code>
            ) : (
              <code className={className} {...props}>
                {children}
              </code>
            );
          },
          hr: ({ node, ...props }) => <hr className="border-t border-card-border/50 my-4" {...props} />,
        }}
      >
        {content}
      </ReactMarkdown>
    </div>
  );
}
