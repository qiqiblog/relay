import { useEffect, useState } from "react";
import { type PublicStatus, type PublicNodeStatus } from "@/lib/api";
import { CheckCircle, XCircle } from "lucide-react";

function timeAgo(iso: string): string {
  const sec = Math.floor((Date.now() - new Date(iso).getTime()) / 1000);
  if (sec < 60) return `${sec}s 前`;
  const min = Math.floor(sec / 60);
  if (min < 60) return `${min}m 前`;
  return `${Math.floor(min / 60)}h 前`;
}

function fmtBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 ** 3) return `${(n / 1024 / 1024).toFixed(1)} MB`;
  return `${(n / 1024 ** 3).toFixed(1)} GB`;
}

function fmtRate(bps: number): string {
  if (bps < 1024) return `${bps.toFixed(0)} B/s`;
  if (bps < 1024 * 1024) return `${(bps / 1024).toFixed(1)} KB/s`;
  return `${(bps / 1024 / 1024).toFixed(1)} MB/s`;
}

function MiniBar({ pct, color }: { pct: number; color: string }) {
  return (
    <div className="h-1.5 w-full rounded-full bg-black/10 dark:bg-white/10 overflow-hidden">
      <div
        className="h-full rounded-full transition-all duration-500"
        style={{ width: `${Math.min(100, pct)}%`, backgroundColor: color }}
      />
    </div>
  );
}

function hourBarColor(minutes: number | null): string {
  if (minutes === null) return "bg-muted-foreground/20";
  if (minutes === 0) return "bg-red-400/70";
  if (minutes < 50) return "bg-amber-400/80";
  return "bg-emerald-500/70";
}

function hourBarTitle(minutes: number | null): string {
  if (minutes === null) return "暂无数据";
  if (minutes === 0) return "离线";
  return `在线 ${minutes}/60 分钟`;
}

function HistoryBars({
  history,
  recent,
}: {
  history: (number | null)[];
  recent: (boolean | null)[];
}) {
  if (history.length === 0) return null;
  return (
    <div className="relative group/history">
      {/* 90 小时概览 */}
      <div className="flex gap-px h-4">
        {history.map((m, i) => (
          <div
            key={i}
            className={`flex-1 min-w-0 rounded-sm ${hourBarColor(m)}`}
            title={hourBarTitle(m)}
          />
        ))}
      </div>

      {/* 悬停展开：最近 2 小时逐分钟 */}
      {recent.length > 0 && (
        <div className="absolute bottom-full left-0 right-0 mb-2 hidden group-hover/history:block z-10">
          <div className="rounded-lg border bg-popover shadow-md p-3 space-y-1.5">
            <p className="text-xs text-muted-foreground font-medium">最近 2 小时（逐分钟）</p>
            <div className="flex gap-px h-5">
              {recent.map((ok, i) => (
                <div
                  key={i}
                  className={`flex-1 min-w-0 rounded-sm ${
                    ok === null
                      ? "bg-muted-foreground/20"
                      : ok
                        ? "bg-emerald-500/70"
                        : "bg-red-400/70"
                  }`}
                  title={ok === null ? "暂无数据" : ok ? "在线" : "离线"}
                />
              ))}
            </div>
            <div className="flex justify-between text-xs text-muted-foreground/60">
              <span>2h 前</span>
              <span>现在</span>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

function NodeCard({ node }: { node: PublicNodeStatus }) {
  return (
    <div className="rounded-xl border bg-card p-4 space-y-3">
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-2 min-w-0">
          <span
            className={`h-2 w-2 rounded-full shrink-0 ${
              node.online ? "bg-emerald-500" : "bg-muted-foreground/40"
            }`}
          />
          <span className="font-semibold text-sm truncate">{node.hostname || node.id}</span>
        </div>
        <div className="flex items-center gap-3 shrink-0 text-xs text-muted-foreground">
          {node.uptime_90h != null && (
            <span className="tabular-nums">{node.uptime_90h.toFixed(1)}%</span>
          )}
          {node.last_seen_at && <span>{timeAgo(node.last_seen_at)}</span>}
          <span
            className={`font-medium ${
              node.online
                ? "text-emerald-600 dark:text-emerald-400"
                : "text-muted-foreground"
            }`}
          >
            {node.online ? "在线" : "离线"}
          </span>
        </div>
      </div>

      {node.history.length > 0 && (
        <HistoryBars history={node.history} recent={node.recent_minutes} />
      )}

      {node.online && (node.cpu_pct != null || node.mem_pct != null) && (
        <div className="grid grid-cols-2 gap-x-4 gap-y-2 sm:grid-cols-3">
          {node.cpu_pct != null && (
            <div className="space-y-1">
              <div className="flex justify-between text-xs text-muted-foreground">
                <span>CPU</span>
                <span className="tabular-nums">{node.cpu_pct.toFixed(1)}%</span>
              </div>
              <MiniBar
                pct={node.cpu_pct}
                color={node.cpu_pct > 80 ? "#ef4444" : node.cpu_pct > 50 ? "#f59e0b" : "#38bdf8"}
              />
            </div>
          )}
          {node.mem_pct != null && (
            <div className="space-y-1">
              <div className="flex justify-between text-xs text-muted-foreground">
                <span>内存</span>
                <span className="tabular-nums">
                  {node.mem_pct.toFixed(1)}%
                  {node.mem_total_bytes > 0 && (
                    <span className="ml-1 opacity-60">
                      {fmtBytes(node.mem_used_bytes)} / {fmtBytes(node.mem_total_bytes)}
                    </span>
                  )}
                </span>
              </div>
              <MiniBar
                pct={node.mem_pct}
                color={node.mem_pct > 85 ? "#ef4444" : "#f59e0b"}
              />
            </div>
          )}
          {node.active_connections != null && (
            <div className="flex items-end gap-1 text-xs text-muted-foreground">
              <span className="font-semibold text-sm text-foreground tabular-nums">
                {node.active_connections}
              </span>
              活跃连接
            </div>
          )}
          {(node.net_rx_bps > 0 || node.net_tx_bps > 0) && (
            <div className="col-span-2 sm:col-span-3 flex gap-3 text-xs text-muted-foreground tabular-nums">
              <span>↓ {fmtRate(node.net_rx_bps)}</span>
              <span>↑ {fmtRate(node.net_tx_bps)}</span>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

export default function StatusPage() {
  const [data, setData] = useState<PublicStatus | null>(null);
  const [error, setError] = useState(false);

  useEffect(() => {
    const es = new EventSource("/api/v1/status/stream");
    es.onmessage = (e) => {
      try { setData(JSON.parse(e.data)); } catch {}
    };
    es.onerror = () => setError(true);
    return () => es.close();
  }, []);

  const onlineCount = data?.nodes.filter((n) => n.online).length ?? 0;
  const totalCount = data?.nodes.length ?? 0;
  const allOk = totalCount > 0 && onlineCount === totalCount;

  return (
    <div className="min-h-screen bg-background text-foreground">
      <div className="max-w-2xl mx-auto px-4 py-12 space-y-8">
        <div>
          <h1 className="text-2xl font-bold tracking-tight">服务状态</h1>
        </div>

        {error ? (
          <div className="rounded-lg border border-red-200 bg-red-50 p-4 flex items-center gap-3 dark:border-red-800/30 dark:bg-red-950/30">
            <XCircle className="h-4 w-4 text-red-500 shrink-0" />
            <span className="text-sm text-red-700 dark:text-red-300">无法获取状态数据</span>
          </div>
        ) : !data ? (
          <div className="rounded-lg border p-4 text-sm text-muted-foreground animate-pulse">
            加载中…
          </div>
        ) : (
          <>
            <div
              className={`rounded-lg border p-4 flex items-center gap-3 ${
                allOk
                  ? "border-emerald-200 bg-emerald-50 dark:border-emerald-800/30 dark:bg-emerald-950/30"
                  : "border-red-200 bg-red-50 dark:border-red-800/30 dark:bg-red-950/30"
              }`}
            >
              {allOk ? (
                <CheckCircle className="h-4 w-4 text-emerald-500 shrink-0" />
              ) : (
                <XCircle className="h-4 w-4 text-red-500 shrink-0" />
              )}
              <span
                className={`font-medium text-sm ${
                  allOk
                    ? "text-emerald-900 dark:text-emerald-200"
                    : "text-red-900 dark:text-red-200"
                }`}
              >
                {allOk
                  ? "所有节点运行正常"
                  : `${totalCount - onlineCount} 个节点离线`}
              </span>
              <span className="ml-auto text-xs text-muted-foreground tabular-nums">
                {onlineCount} / {totalCount} 在线
              </span>
            </div>

            <div className="space-y-3">
              <h2 className="text-xs font-semibold uppercase tracking-widest text-muted-foreground/60">
                节点
              </h2>
              {data.nodes.length === 0 ? (
                <p className="text-sm text-muted-foreground">暂无节点。</p>
              ) : (
                data.nodes.map((n) => <NodeCard key={n.id} node={n} />)
              )}
            </div>
          </>
        )}
      </div>
    </div>
  );
}
