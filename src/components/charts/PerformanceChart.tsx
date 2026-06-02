import { useMemo } from 'react';
import { ECharts } from '../ECharts';

interface PerformanceTrend {
  date: string;
  avg_latency: number;
  avg_tps: number;
}

interface PerformanceChartProps {
  data: PerformanceTrend[];
  theme: 'light' | 'dark';
}

const PALETTE_COLORS = [
  '#06b6d4', // 1. TPS -> 明亮青
  '#6366f1', // 2. Latency -> 睿智靛蓝
];

function hexToRgba(hex: string, alpha: number) {
  const r = parseInt(hex.slice(1, 3), 16);
  const g = parseInt(hex.slice(3, 5), 16);
  const b = parseInt(hex.slice(5, 7), 16);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

export function PerformanceChart({ data = [], theme }: PerformanceChartProps) {
  const isDark = theme === 'dark';

  const chartOption = useMemo(() => {
    const dates = data.map(t => t.date);
    const tpsData = data.map(t => t.avg_tps);
    const latencyData = data.map(t => t.avg_latency);

    const borderColor = isDark ? '#0b1528' : '#ffffff';

    const colors = isDark
      ? PALETTE_COLORS
      : [
          '#2563eb', // 1. TPS -> 皇家蓝
          '#10b981', // 2. Latency -> 薄荷绿
        ];

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
          
          params.forEach((item: any) => {
            const val = item.value || 0;
            const unit = item.seriesName.includes('TPS') ? ' Token/s' : ' 秒';
            
            html += `<div style="display:flex;align-items:center;justify-content:space-between;gap:20px;margin-bottom:4px;font-size:11px;">
              <span style="display:inline-flex;align-items:center;gap:4px;color:${isDark ? '#9ca3af' : '#64748b'};">
                ${item.marker} ${item.seriesName}
              </span>
              <span style="font-weight:600;color:${isDark ? '#f3f4f6' : '#0f172a'};font-family:'JetBrains Mono', monospace;margin-left:auto;">
                ${val.toFixed(2)}${unit}
              </span>
            </div>`;
          });
          
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
      grid: { left: 45, right: 45, top: 40, bottom: 25 },
      xAxis: {
        type: 'category',
        data: dates,
        axisLabel: { color: isDark ? '#9ca3af' : '#64748b', fontSize: 10, fontFamily: 'Outfit' },
        axisLine: { lineStyle: { color: isDark ? 'rgba(255,255,255,0.08)' : '#e2e8f0' } },
        axisTick: { show: false },
      },
      yAxis: [
        {
          type: 'value',
          name: '生成速率',
          position: 'left',
          axisLabel: {
            color: isDark ? '#9ca3af' : '#64748b',
            fontSize: 10,
            fontFamily: 'JetBrains Mono',
            formatter: '{value} T/s'
          },
          splitLine: { lineStyle: { color: isDark ? 'rgba(255,255,255,0.04)' : '#f1f5f9' } },
          nameTextStyle: { color: isDark ? '#9ca3af' : '#64748b', fontSize: 9 }
        },
        {
          type: 'value',
          name: '交互延迟',
          position: 'right',
          axisLabel: {
            color: isDark ? '#9ca3af' : '#64748b',
            fontSize: 10,
            fontFamily: 'JetBrains Mono',
            formatter: '{value} s'
          },
          splitLine: { show: false },
          nameTextStyle: { color: isDark ? '#9ca3af' : '#64748b', fontSize: 9 }
        }
      ],
      series: [
        {
          name: '生成速率 (TPS)',
          type: 'line',
          yAxisIndex: 0,
          data: tpsData,
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
                { offset: 0, color: hexToRgba(colors[0], 0.16) },
                { offset: 1, color: hexToRgba(colors[0], 0.01) },
              ],
            },
          },
        },
        {
          name: '交互延迟 (Latency)',
          type: 'line',
          yAxisIndex: 1,
          data: latencyData,
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
                { offset: 0, color: hexToRgba(colors[1], 0.16) },
                { offset: 1, color: hexToRgba(colors[1], 0.01) },
              ],
            },
          },
        }
      ]
    };
  }, [data, isDark]);

  return (
    <div style={{ height: '300px', width: '100%' }}>
      <ECharts option={chartOption as any} />
    </div>
  );
}
