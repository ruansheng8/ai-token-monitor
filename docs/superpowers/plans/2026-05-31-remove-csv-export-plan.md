# 实现计划：移除导出 CSV 账单功能

本计划详细说明了如何彻底清理 AI Token Monitor 中的“导出 CSV 账单”功能，并提供具体的变更内容及验证计划。

## 变更文件列表

本次变更仅涉及 1 个前端文件：
* `[MODIFY]` [App.tsx](file:///d:/VibeCoding/ai-token-monitor/src/App.tsx)

---

## 详细变更步骤

### 1. 修改 `src/App.tsx`

#### 删除 `exportToCSV` 函数定义
* **定位**：约在第 235 行至第 267 行。
* **要删除的代码**：
  ```typescript
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
  ```

#### 删除 JSX 中的 “导出 CSV 账单” 按钮
* **定位**：约在第 1839 行至第 1845 行。
* **要删除的代码**：
  ```tsx
  <button
    onClick={() => exportToCSV(paginatedSessions)}
    className="px-3 py-1.5 rounded-lg text-xs font-semibold bg-slate-200/60 hover:bg-slate-300/60 dark:bg-white/5 dark:hover:bg-white/10 text-text-primary border border-card-border cursor-pointer transition-all duration-200 flex items-center gap-1 shadow-sm"
    title="导出当前筛选出的账单为高兼容 CSV (Excel/WPS 无缝支持)"
  >
    📥 导出 CSV 账单
  </button>
  ```

---

## 验证计划

为了验证变更的正确性，我们将执行以下步骤：

### 1. 静态验证
* 运行 TypeScript 编译器，确保没有引起类型错误或未定义变量：
  ```powershell
  npx tsc -b --noEmit
  ```
* 运行 ESLint，确保没有 lint 错误（如引入了未使用的变量或引用错误）：
  ```powershell
  npm run lint
  ```

### 2. 构建验证
* 运行打包命令，验证前端资源能成功编译：
  ```powershell
  npm run build
  ```

### 3. 动态验证
* 启动 Vite 开发服务器：
  ```powershell
  npm run dev
  ```
* 打开浏览器，检查“会话用量明细”卡片。
* 确认“导出 CSV 账单”按钮已经被成功移除，而相邻的“生成财务报表”按钮依然正常显示且无样式坍塌或排版错乱。
