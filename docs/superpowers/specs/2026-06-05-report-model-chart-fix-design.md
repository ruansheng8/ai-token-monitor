# 2026-06-05 “生成图片报表” 底层模型消耗占比修复设计规范

## 1. 背景与现状

在 “生成图片报表” 功能中，“底层模型消耗占比” 模块存在以下两个问题：
1. **模型数量过多**：图表会尝试展示全部底层模型，导致在模型丰富时图片报表排版过于拥挤。
2. **模型名称截断**：使用 `html2canvas` 导出 Canvas 图片时，由于模型名称的 `<span>` 元素具有 `truncate` 属性但未设定行高且为 `display: inline`，导致导出后字母的下半部分（如下坠字符 g, y, p, q）被裁剪遮挡。

## 2. 设计方案

### 2.1 底层模型数量过滤
在离屏渲染图片报表模板中，对模型数据源进行过滤，仅保留前 8 个模型数据：
- 数据源：`data.model_distribution`
- 过滤手段：`.slice(0, 8)`

### 2.2 样式修复
修改名称 `<span>` 的 CSS Class，添加：
- `inline-block`：使 `truncate`（`overflow: hidden; text-overflow: ellipsis; white-space: nowrap`）计算盒尺寸时能被 html2canvas 正确解析为块级盒结构。
- `leading-normal`：提供宽裕的正常行高。
- `pb-0.5`：提供 2px 的底部边距作为裁切缓冲区，避免下半部分文字被遮挡。

## 3. 受影响文件

- [src/App.tsx](file:///d:/VibeCoding/ai-token-monitor/src/App.tsx)
