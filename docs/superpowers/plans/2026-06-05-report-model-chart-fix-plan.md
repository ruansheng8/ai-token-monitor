# 2026-06-05 “生成图片报表” 底层模型消耗占比修复实现计划

修复 “生成图片报表” 中的 “底层模型消耗占比” 限制展示前 8 个，并解决模型名称在图片导出时下半部被遮挡的问题。

## User Review Required

> [!NOTE]
> 用户已在对话中明确表示“继续，后面不用经过我确认”。本实现计划在创建后将直接进入执行阶段。

## Proposed Changes

### 前端 UI 模板

#### [MODIFY] [App.tsx](file:///d:/VibeCoding/ai-token-monitor/src/App.tsx)

- 限制模型占比在图片报表中只展示前 8 个（使用 `slice(0, 8)`）。
- 调整模型名称 `<span>` 的样式类，加入 `inline-block leading-normal pb-0.5` 以规避 `html2canvas` 渲染阶段的截断问题。

```diff
-              <div className="flex flex-col gap-4 pr-1">
-                {data.model_distribution && data.model_distribution.length > 0 ? (
-                  data.model_distribution.map((m) => {
-                    const pct = maxModelTokens > 0 ? (m.total_tokens / maxModelTokens) * 100 : 0;
-                    return (
-                      <div key={m.model} className="flex flex-col gap-1.5">
-                        <div className="flex justify-between items-center text-xs">
-                          <span className="font-semibold text-text-primary text-[11px] truncate max-w-[150px]">{m.model}</span>
-                          <span className="font-mono text-text-secondary text-[10px]" title={`${formatPreciseNum(m.total_tokens)} Tokens`}>
-                            {formatNum(m.total_tokens)} Tokens
-                          </span>
-                        </div>
+              <div className="flex flex-col gap-4 pr-1">
+                {data.model_distribution && data.model_distribution.length > 0 ? (
+                  data.model_distribution.slice(0, 8).map((m) => {
+                    const pct = maxModelTokens > 0 ? (m.total_tokens / maxModelTokens) * 100 : 0;
+                    return (
+                      <div key={m.model} className="flex flex-col gap-1.5">
+                        <div className="flex justify-between items-center text-xs">
+                          <span className="font-semibold text-text-primary text-[11px] inline-block leading-normal pb-0.5 truncate max-w-[150px]">{m.model}</span>
+                          <span className="font-mono text-text-secondary text-[10px]" title={`${formatPreciseNum(m.total_tokens)} Tokens`}>
+                            {formatNum(m.total_tokens)} Tokens
+                          </span>
+                        </div>
```

## Verification Plan

### Automated Tests
- 运行前端的类型检查：
  ```bash
  npx tsc -b --noEmit
  ```
- 运行 ESLint 检查：
  ```bash
  npm run lint
  ```

### Manual Verification
- 启动应用程序，生成并保存图片报表，检查图片报表中的“底层模型消耗占比”是否最多展示 8 个，且模型名称文本不再发生底部裁切。
