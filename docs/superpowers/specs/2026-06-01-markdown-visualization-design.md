# 智能复盘分析诊断报告 Markdown 可视化设计规范 (Spec)

## 1. 背景与目标

目前 AI Token Monitor 项目在“智能复盘分析诊断报告”的结果查看上，前端仅通过简单的正则表达式（`renderMarkdown`）配合 `dangerouslySetInnerHTML` 渲染部分 Markdown 语法，导致以下问题：
- **格式支持局限**：不支持复杂的 Markdown 语法，例如表格（`| 指标 | 数值 |`）、无序/有序嵌套列表、复杂引用和代码块。而 AI 生成的诊断报告中包含大量这类表格和列表数据。
- **安全性隐患**：通过 `dangerouslySetInnerHTML` 渲染由本地 AI 接口/CLI 生成的 HTML 字符串，虽然本地数据相对安全，但仍存在潜在的 XSS 注入漏洞。
- **用户体验不足**：超链接点击后可能在 Tauri 窗口内部直接跳转，导致桌面应用无法正常返回；样式和排版比较单一，与整体的玻璃拟态暗黑风格不完全契合。

本设计的**目标**是引入 `react-markdown` 渲染引擎和 `remark-gfm` 扩展插件，实现一个完全贴合 Tauri 桌面应用和 Tailwind v4 玻璃拟态设计规范的 Markdown 可视化渲染方案，以确保专业、精美和安全的阅读体验。

---

## 2. 技术选型与依赖

我们使用以下库来实现 Markdown 解析与渲染：
- **`react-markdown` (v9+)**：React 官方推荐的 Markdown 解析引擎，将 Markdown 解析为虚拟 DOM 节点，完全免于 XSS 风险，且完美支持 React 19。
- **`remark-gfm` (v4+)**：提供对 GitHub 风格 Markdown（如数据表格、链接、任务列表等）的支持。

在根目录的 `package.json` 中，需要安装以下依赖：
```json
"dependencies": {
  "react-markdown": "^9.0.0",
  "remark-gfm": "^4.0.0"
}
```

---

## 3. 详细设计

### 3.1 新建通用 Markdown 渲染器组件

为了避免在已经非常庞大（2378行）的 [ReviewDrawer.tsx](file:///d:/VibeCoding/ai-token-monitor/src/components/ReviewDrawer.tsx) 中直接堆砌解析逻辑，我们将渲染器抽离为一个单独的组件：
[Markdown.tsx](file:///d:/VibeCoding/ai-token-monitor/src/components/Markdown.tsx)

#### 核心代码实现框架：
```tsx
import React from 'react';
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
          // 拦截超链接在系统浏览器打开，防止在 Tauri Webview 中内部跳转
          a: ({ href, children }) => (
            <a
              href={href}
              onClick={(e) => {
                e.preventDefault();
                if (href) {
                  import('@tauri-apps/plugin-shell').then(({ open }) => open(href));
                }
              }}
              className="text-neon-cyan hover:underline inline-flex items-center gap-0.5"
            >
              {children}
            </a>
          ),
          // 玻璃拟态表格设计，数值使用 monospace 对齐
          table: ({ children }) => (
            <div className="table-responsive my-4 rounded-xl border border-white/10 dark:border-white/5 overflow-hidden">
              <table className="w-full border-collapse text-left text-xs">{children}</table>
            </div>
          ),
          thead: ({ children }) => (
            <thead className="bg-slate-100 dark:bg-slate-900/60">{children}</thead>
          ),
          th: ({ children }) => (
            <th className="p-2.5 font-semibold text-text-secondary border-b border-card-border">
              {children}
            </th>
          ),
          td: ({ children }) => (
            <td className="p-2.5 border-b border-card-border/50 text-text-primary font-mono">
              {children}
            </td>
          ),
          tr: ({ children }) => (
            <tr className="hover:bg-slate-50 dark:hover:bg-slate-800/20 transition-colors">{children}</tr>
          ),
          // 标题与修饰线（使用 Outfit 字体和渐变色）
          h1: ({ children }) => (
            <h1 className="text-lg font-bold mt-5 mb-2.5 text-text-primary border-b border-card-border/50 pb-1 flex items-center gap-2">
              {children}
            </h1>
          ),
          h2: ({ children }) => (
            <h2 className="text-base font-bold mt-4 mb-2 text-text-primary flex items-center gap-2">
              <span className="w-1 h-4 rounded bg-neon-cyan inline-block"></span>
              {children}
            </h2>
          ),
          h3: ({ children }) => (
            <h3 className="text-sm font-bold mt-3.5 mb-1.5 text-text-secondary flex items-center gap-1.5">
              {children}
            </h3>
          ),
          // 段落与列表
          p: ({ children }) => (
            <p className="text-xs leading-relaxed text-text-primary mb-3">{children}</p>
          ),
          ul: ({ children }) => (
            <ul className="list-disc pl-5 mb-3 space-y-1 text-xs text-text-secondary">{children}</ul>
          ),
          ol: ({ children }) => (
            <ol className="list-decimal pl-5 mb-3 space-y-1 text-xs text-text-secondary">{children}</ol>
          ),
          li: ({ children }) => (
            <li className="leading-relaxed text-text-primary">{children}</li>
          ),
          // 引用块
          blockquote: ({ children }) => (
            <blockquote className="border-l-4 border-neon-cyan/40 bg-neon-cyan/5 px-4 py-2 my-3 rounded-r-lg text-text-muted italic text-xs leading-relaxed">
              {children}
            </blockquote>
          ),
          // 代码块与行内代码
          code: ({ className, children, ...props }) => {
            const match = /language-(\w+)/.exec(className || '');
            const isInline = !match;
            return isInline ? (
              <code className="px-1.5 py-0.5 rounded font-mono text-[11px] text-neon-cyan bg-neon-cyan/5 border border-neon-cyan/10" {...props}>
                {children}
              </code>
            ) : (
              <pre className="p-3.5 rounded-xl font-mono text-[11px] overflow-x-auto bg-slate-950/80 border border-white/5 my-3 text-emerald-400">
                <code className={className} {...props}>
                  {children}
                </code>
              </pre>
            );
          },
          hr: () => <hr className="border-t border-card-border/50 my-4" />,
        }}
      >
        {content}
      </ReactMarkdown>
    </div>
  );
}
```

---

## 4. 迁移与修改方案

### 4.1 弃用旧有的正则解析
在 [ReviewDrawer.tsx](file:///d:/VibeCoding/ai-token-monitor/src/components/ReviewDrawer.tsx) 中：
1. 移除 `renderMarkdown` 辅助函数的定义。
2. 引入新增的 `Markdown` 组件：
   ```tsx
   import { Markdown } from './Markdown';
   ```
3. 在渲染报告结果的区域，将：
   ```tsx
   <div
     dangerouslySetInnerHTML={{
       __html: `<p class="md-p">${renderMarkdown(outputText)}</p>`,
     }}
   />
   ```
   替换为：
   ```tsx
   <Markdown content={outputText} />
   ```

---

## 5. 验证与测试计划

为了确保修改后前端能正常构建且功能表现完美，我们将执行以下测试步骤：
1. **类型与构建检查**：
   - 执行 `npx tsc -b --noEmit` 进行 TypeScript 类型静态检查。
   - 执行 `npm run build` 进行前端静态资源打包，验证是否存在打包期错误。
2. **样式一致性检查**：
   - 切换系统暗黑模式（Dark Mode）与日间模式（Light Mode），验证 Markdown 渲染的文本颜色、表格背景色、表格分割线是否完美自适应。
   - 检查数值表格在奇偶行背景下的文字对比度。
3. **超链接跳转测试**：
   - 在本地诊断报告中随机生成带有超链接的内容，点击后观察是否调起系统外部默认浏览器，且 Tauri 内部 Webview 页面应不受干扰。
