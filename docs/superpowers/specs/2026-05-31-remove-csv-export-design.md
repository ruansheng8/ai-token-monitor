# 设计规约：移除导出 CSV 账单功能

本项目旨在完全移除 AI Token Monitor 看板中现有的“导出 CSV 账单”功能，以简化功能并保持前端代码的整洁。

## 变更背景

“导出 CSV 账单”功能原用于将当前筛选出的会话账单导出为本地 CSV 文件。根据最新产品规划，此功能不再需要，因此需将其彻底从前端代码中清理，避免死代码残留。

## 设计细节

本变更仅涉及前端代码修改，无需改动 Rust 后端或数据库。

### 1. 代码清理

在 `src/App.tsx` 中执行以下两处修改：

* **清理函数定义**：删除 `exportToCSV` 的定义。
  ```typescript
  // 需删除的代码范围：
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

* **清理 UI 按钮**：在“会话用量明细”卡片头部区域，删除 `<button>` 元素。
  ```tsx
  // 需删除的 JSX 元素：
  <button
    onClick={() => exportToCSV(paginatedSessions)}
    className="px-3 py-1.5 rounded-lg text-xs font-semibold bg-slate-200/60 hover:bg-slate-300/60 dark:bg-white/5 dark:hover:bg-white/10 text-text-primary border border-card-border cursor-pointer transition-all duration-200 flex items-center gap-1 shadow-sm"
    title="导出当前筛选出的账单为高兼容 CSV (Excel/WPS 无缝支持)"
  >
    📥 导出 CSV 账单
  </button>
  ```

### 2. 布局微调

原有的“生成财务报表”按钮及其他控件（隐藏 0 消耗会话、搜索框等）保持不变。
删除“导出 CSV 账单”按钮后，按钮容器内仅余下“生成财务报表”按钮，布局会自动收缩，不会对其他布局造成破坏。

## 验证计划

1. **类型检查与构建验证**：
   在前端运行类型检查，确保没有任何遗留引用：
   ```powershell
   npx tsc -b --noEmit
   npm run build
   ```
2. **界面走查**：
   启动前端开发服务器（`npm run dev`），打开浏览器走查“会话用量明细”卡片，确保“导出 CSV 账单”按钮已被成功移除，且“生成财务报表”按钮及其他操作均可正常工作。
