import { cn } from "@/lib/utils";
import { ProtocolBadge } from "./protocol-badge";

/// 渲染 tunnel 的协议集合：单协议沿用 ProtocolBadge；双协议合并为一枚 TCP+UDP 渐变徽章。
export function ProtocolSetBadge({
  protocols,
  className,
}: {
  protocols: string[] | null | undefined;
  className?: string;
}) {
  const list = (protocols ?? []).map((p) => (p || "").toLowerCase()).filter(Boolean);
  const norm = Array.from(new Set(list)).sort();
  if (norm.length === 0) {
    return <ProtocolBadge protocol="-" className={className} />;
  }
  if (norm.length === 1) {
    return <ProtocolBadge protocol={norm[0]} className={className} />;
  }
  return (
    <span
      className={cn(
        "inline-flex items-center rounded-full border px-2.5 py-0.5 text-xs font-medium",
        "border-transparent bg-gradient-to-r from-sky-500/15 to-violet-500/15",
        "text-foreground",
        className,
      )}
      title={norm.map((p) => p.toUpperCase()).join(" + ")}
    >
      {norm.map((p) => p.toUpperCase()).join("+")}
    </span>
  );
}
