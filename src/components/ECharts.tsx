import { useEffect, useRef } from 'react';
import * as echarts from 'echarts';

interface EChartsProps {
  option: echarts.EChartsOption;
  style?: React.CSSProperties;
  className?: string;
  theme?: string | object;
}

export function ECharts({ option, style, className, theme }: EChartsProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<echarts.ECharts | null>(null);

  // 初始化图表
  useEffect(() => {
    if (!containerRef.current) return;

    const chart = echarts.init(containerRef.current, theme);
    chartRef.current = chart;

    const handleResize = () => {
      chart.resize();
    };

    const resizeObserver = new ResizeObserver(() => {
      chart.resize();
    });

    window.addEventListener('resize', handleResize);
    if (containerRef.current.parentElement) {
      resizeObserver.observe(containerRef.current.parentElement);
    }

    return () => {
      window.removeEventListener('resize', handleResize);
      resizeObserver.disconnect();
      chart.dispose();
      chartRef.current = null;
    };
  }, [theme]);

  // 更新配置项
  useEffect(() => {
    if (chartRef.current && option) {
      chartRef.current.setOption(option, true);
    }
  }, [option]);

  return (
    <div
      ref={containerRef}
      style={{ width: '100%', height: '100%', minHeight: '100px', ...style }}
      className={className}
    />
  );
}
