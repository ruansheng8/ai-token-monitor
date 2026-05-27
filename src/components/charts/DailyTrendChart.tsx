import { useMemo } from 'react';
import { ECharts } from '../ECharts';

interface DailyTrend {
  date: string;
  input: number;
  output: number;
  cached: number;
  thinking: number;
  sessions: number;
}

interface DailyTrendChartProps {
  data: DailyTrend[];
  theme: 'light' | 'dark';
}

const PALETTE_COLORS = [
  '#14b8a6', // 1. 缓存输入 Token -> 薄荷绿
  '#06b6d4', // 2. 未缓存输入 Token -> 明亮青
  '#ec4899', // 3. 输出 Token -> 柔和粉
  '#8b5cf6', // 4. 推理 Token -> 科技紫
];

function hexToRgba(hex: string, alpha: number) {
  const r = parseInt(hex.slice(1, 3), 16);
  const g = parseInt(hex.slice(3, 5), 16);
  const b = parseInt(hex.slice(5, 7), 16);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

// 格式化数字，带中文大数单位（万、亿），保留最多1位有效小数
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

export function DailyTrendChart({ data = [], theme }: DailyTrendChartProps) {
  const isDark = theme === 'dark';

  const chartOption = useMemo(() => {
    const dates = data.map(t => t.date);
    const cachedData = data.map(t => t.cached);
    const uncachedData = data.map(t => Math.max(0, t.input - t.cached));
    const outputData = data.map(t => t.output);
    const thinkingData = data.map(t => t.thinking);

    const borderColor = isDark ? '#0b1528' : '#ffffff';

    return {
      color: PALETTE_COLORS,
      tooltip: {
        trigger: 'axis',
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
            const isLine = item.seriesType === 'line';
            const val = item.value || 0;
            if (!isLine) {
              totalBar += val;
            }
            
            html += `<div style="display:flex;align-items:center;justify-content:space-between;gap:20px;margin-bottom:4px;font-size:11px;">
              <span style="display:inline-flex;align-items:center;gap:4px;color:${isDark ? '#9ca3af' : '#64748b'};">
                ${item.marker} ${item.seriesName}
              </span>
              <span style="font-weight:600;color:${isDark ? '#f3f4f6' : '#0f172a'};font-family:'JetBrains Mono', monospace;margin-left:auto;">
                ${formatValueWithUnit(val)}
              </span>
            </div>`;
          });
          
          // 总和
          html += `<div style="margin-top:6px;padding-top:6px;border-top:1px solid ${isDark ? 'rgba(255,255,255,0.08)' : '#e2e8f0'};display:flex;align-items:center;justify-content:space-between;gap:20px;font-size:11px;">
            <span style="font-weight:600;color:${isDark ? '#06b6d4' : '#0891b2'};">总消耗 TOKEN</span>
            <span style="font-weight:700;color:${isDark ? '#06b6d4' : '#0891b2'};font-family:'JetBrains Mono', monospace;margin-left:auto;">
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
      series: [
        {
          name: '缓存输入 Token',
          type: 'bar',
          stack: 'total',
          data: cachedData,
          itemStyle: {
            borderRadius: 4,
            borderColor: borderColor,
            borderWidth: 1.5,
          },
        },
        {
          name: '未缓存输入 Token',
          type: 'bar',
          stack: 'total',
          data: uncachedData,
          itemStyle: {
            borderRadius: 4,
            borderColor: borderColor,
            borderWidth: 1.5,
          },
        },
        {
          name: '输出 Token',
          type: 'bar',
          stack: 'total',
          data: outputData,
          itemStyle: {
            borderRadius: 4,
            borderColor: borderColor,
            borderWidth: 1.5,
          },
        },
        {
          name: '推理 Token',
          type: 'line',
          data: thinkingData,
          smooth: true,
          showSymbol: true,
          symbol: 'circle',
          symbolSize: 6,
          itemStyle: {
            borderWidth: 2,
            borderColor: '#ffffff',
          },
          areaStyle: {
            color: {
              type: 'linear',
              x: 0, y: 0, x2: 0, y2: 1,
              colorStops: [
                { offset: 0, color: hexToRgba(PALETTE_COLORS[3], 0.15) },
                { offset: 1, color: hexToRgba(PALETTE_COLORS[3], 0.01) },
              ],
            },
          },
        }
      ]
    };
  }, [data, isDark]);

  return (
    <div style={{ height: '350px', width: '100%' }}>
      <ECharts option={chartOption as any} />
    </div>
  );
}
