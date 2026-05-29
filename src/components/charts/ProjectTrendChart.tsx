import { useMemo } from 'react';
import { ECharts } from '../ECharts';

interface ProjectTrendItem {
  date: string;
  project_name: string;
  tokens: number;
  cost_usd: number;
}

interface ProjectTrendChartProps {
  data: ProjectTrendItem[];
  theme: 'light' | 'dark';
  displayCurrency?: string;
  exchangeRate?: number;
}

const PALETTE_COLORS = [
  '#3b82f6', // 1. 活力蓝
  '#06b6d4', // 2. 明亮青
  '#14b8a6', // 3. 薄荷绿
  '#6366f1', // 4. 睿智靛蓝
  '#8b5cf6', // 5. 科技紫
  '#ec4899', // 6. 柔和粉
  '#f59e0b', // 7. 琥珀黄
  '#10b981', // 8. 翠绿
];

function hexToRgba(hex: string, alpha: number) {
  const r = parseInt(hex.slice(1, 3), 16);
  const g = parseInt(hex.slice(3, 5), 16);
  const b = parseInt(hex.slice(5, 7), 16);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

const formatValueWithUnit = (val: number) => {
  const precise = val.toLocaleString('zh-CN');
  if (val >= 10000) {
    let unit = '';
    let formatted = val;
    if (val >= 1e8) {
      formatted = val / 1e8;
      unit = '亿';
    } else if (val >= 1e4) {
      formatted = val / 1e4;
      unit = '万';
    }
    const trimmed = parseFloat(formatted.toFixed(1)).toString();
    return `${precise} (${trimmed}${unit})`;
  }
  return precise;
};

export function ProjectTrendChart({ data = [], theme }: ProjectTrendChartProps) {
  const isDark = theme === 'dark';

  const chartOption = useMemo(() => {
    // 1. 提取所有不重复的日期并排序
    const dates = Array.from(new Set(data.map(t => t.date))).sort();
    
    // 2. 提取所有不重复的项目名
    const projects = Array.from(new Set(data.map(t => t.project_name || 'unknown-project')));
    
    const borderColor = isDark ? '#0b1528' : '#ffffff';

    // 3. 构建映射以提高数据检索性能：project_name -> Map<date, tokens>
    const projectDataMap = new Map<string, Map<string, number>>();
    data.forEach(t => {
      const pName = t.project_name || 'unknown-project';
      if (!projectDataMap.has(pName)) {
        projectDataMap.set(pName, new Map<string, number>());
      }
      projectDataMap.get(pName)!.set(t.date, t.tokens);
    });

    const colors = projects.map((_, idx) => PALETTE_COLORS[idx % PALETTE_COLORS.length]);

    const series = projects.map((project, idx) => {
      const seriesData = dates.map(date => projectDataMap.get(project)?.get(date) || 0);
      const color = PALETTE_COLORS[idx % PALETTE_COLORS.length];
      return {
        name: project,
        type: 'line',
        data: seriesData,
        smooth: true,
        showSymbol: true,
        symbol: 'circle',
        symbolSize: 6,
        itemStyle: {
          borderWidth: 2,
          borderColor: borderColor,
        },
        areaStyle: {
          color: {
            type: 'linear',
            x: 0, y: 0, x2: 0, y2: 1,
            colorStops: [
              { offset: 0, color: hexToRgba(color, 0.16) },
              { offset: 1, color: hexToRgba(color, 0.01) },
            ],
          },
        },
      };
    });

    return {
      color: colors,
      tooltip: {
        trigger: 'axis',
        confine: true,
        backgroundColor: isDark ? 'rgba(11, 21, 40, 0.94)' : 'rgba(255, 255, 255, 0.96)',
        borderColor: isDark ? 'rgba(255, 255, 255, 0.08)' : '#e2e8f0',
        borderWidth: 1,
        textStyle: { color: isDark ? '#f3f4f6' : '#0f172a', fontSize: 11 },
        extraCssText: `box-shadow: 0 10px 30px -5px rgba(0, 0, 0, ${isDark ? '0.35' : '0.08'}); border-radius: 12px; padding: 10px; backdrop-filter: blur(8px);`,
        formatter: (params: any) => {
          if (!params || params.length === 0) return '';
          const dateStr = params[0].name;
          let html = `<span style="font-weight:600;color:${isDark ? '#f3f4f6' : '#0f172a'};display:block;margin-bottom:6px;font-size:12px;">${dateStr}</span>`;
          
          let totalBar = 0;
          params.forEach((item: any) => {
            const val = item.value || 0;
            totalBar += val;
            
            html += `<div style="display:flex;align-items:center;justify-content:space-between;gap:20px;margin-bottom:4px;font-size:11px;">
              <span style="display:inline-flex;align-items:center;gap:4px;color:${isDark ? '#9ca3af' : '#64748b'};">
                ${item.marker} ${item.seriesName}
              </span>
              <span style="font-weight:600;color:${isDark ? '#f3f4f6' : '#0f172a'};font-family:'JetBrains Mono', monospace;margin-left:auto;">
                ${formatValueWithUnit(val)} Tokens
              </span>
            </div>`;
          });
          
          // 总和
          html += `<div style="margin-top:6px;padding-top:6px;border-top:1px solid ${isDark ? 'rgba(255,255,255,0.08)' : '#e2e8f0'};display:flex;align-items:center;justify-content:space-between;gap:20px;font-size:11px;">
            <span style="font-weight:600;color:${isDark ? '#3b82f6' : '#2563eb'};">合计 Token</span>
            <span style="font-weight:700;color:${isDark ? '#3b82f6' : '#2563eb'};font-family:'JetBrains Mono', monospace;margin-left:auto;">
              ${formatValueWithUnit(totalBar)}
            </span>
          </div>`;
          
          return html;
        }
      },
      legend: {
        type: 'scroll',
        top: 0,
        right: 'center',
        icon: 'circle',
        itemGap: 16,
        textStyle: { color: isDark ? '#9ca3af' : '#64748b', fontSize: 10, fontFamily: 'Outfit' },
      },
      grid: { left: 45, right: 18, top: 40, bottom: 25 },
      xAxis: {
        type: 'category',
        data: dates,
        axisLabel: { color: isDark ? '#9ca3af' : '#64748b', fontSize: 10, fontFamily: 'Outfit' },
        axisLine: { lineStyle: { color: isDark ? 'rgba(255,255,255,0.08)' : '#e2e8f0' } },
        axisTick: { show: false },
      },
      yAxis: {
        type: 'value',
        axisLabel: {
          color: isDark ? '#9ca3af' : '#64748b',
          fontSize: 10,
          fontFamily: 'JetBrains Mono',
          formatter: (value: number) => {
            if (value === 0) return '0';
            const absNum = Math.abs(value);
            let unit = '';
            let formatted = absNum;

            if (absNum >= 1e8) {
              formatted = absNum / 1e8;
              unit = '亿';
            } else if (absNum >= 1e4) {
              formatted = absNum / 1e4;
              unit = '万';
            }

            if (unit) {
              const trimmed = parseFloat(formatted.toFixed(1)).toString();
              return (value < 0 ? '-' : '') + trimmed + unit;
            }
            return (value < 0 ? '-' : '') + absNum.toString();
          }
        },
        splitLine: { lineStyle: { color: isDark ? 'rgba(255,255,255,0.04)' : '#f1f5f9' } },
      },
      series: series
    };
  }, [data, isDark]);

  return (
    <div style={{ height: '300px', width: '100%' }}>
      <ECharts option={chartOption as any} />
    </div>
  );
}
