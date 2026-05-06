import { AreaChart, Area, Tooltip, ResponsiveContainer } from "recharts";

interface SparklineProps {
  data: number[];
  /** 不传 → 100% 响应式（Card 内）；传入 → 固定像素（表格单元格） */
  width?: number;
  height?: number;
  format?: (n: number) => string;
  color?: string;
}

interface TipPayloadItem {
  value?: number;
}

interface TipProps {
  active?: boolean;
  payload?: TipPayloadItem[];
  format?: (n: number) => string;
}

function SparkTooltip({ active, payload, format }: TipProps) {
  if (!active || !payload || payload.length === 0) return null;
  const v = payload[0].value;
  if (typeof v !== "number") return null;
  return (
    <div className="rounded border bg-background px-2 py-1 text-xs shadow-sm tabular-nums">
      {format ? format(v) : v.toFixed(2)}
    </div>
  );
}

export function Sparkline({
  data,
  width,
  height = 48,
  format,
  color = "hsl(var(--primary))",
}: SparklineProps) {
  if (data.length === 0) {
    return (
      <div
        className="flex items-center justify-center text-xs text-muted-foreground"
        style={{ width: width ?? "100%", height }}
      >
        no data
      </div>
    );
  }

  const chartData = data.map((v, i) => ({ i, v }));
  const last = data[data.length - 1];
  const min = Math.min(...data);
  const max = Math.max(...data);

  const chart = (
    <AreaChart data={chartData} margin={{ top: 2, right: 2, bottom: 2, left: 2 }}>
      <defs>
        <linearGradient id="spark-grad" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor={color} stopOpacity={0.35} />
          <stop offset="100%" stopColor={color} stopOpacity={0} />
        </linearGradient>
      </defs>
      <Tooltip
        cursor={false}
        content={(props: unknown) => (
          <SparkTooltip {...(props as TipProps)} format={format} />
        )}
      />
      <Area
        type="monotone"
        dataKey="v"
        stroke={color}
        strokeWidth={1.5}
        fill="url(#spark-grad)"
        isAnimationActive={false}
      />
    </AreaChart>
  );

  return (
    <div className="space-y-1" style={width ? { width } : undefined}>
      <div style={{ width: width ?? "100%", height }}>
        <ResponsiveContainer width="100%" height="100%">
          {chart}
        </ResponsiveContainer>
      </div>
      <div className="flex justify-between text-[10px] text-muted-foreground">
        <span>{format ? format(min) : min.toFixed(0)}</span>
        <span className="font-medium text-foreground">
          now: {format ? format(last) : last.toFixed(0)}
        </span>
        <span>{format ? format(max) : max.toFixed(0)}</span>
      </div>
    </div>
  );
}
