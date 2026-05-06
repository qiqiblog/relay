import { cn } from "@/lib/utils";

const tones: Record<string, string> = {
  tcp: "bg-sky-500/15 text-sky-600 dark:text-sky-400 border-sky-500/30",
  udp: "bg-violet-500/15 text-violet-600 dark:text-violet-400 border-violet-500/30",
  http: "bg-emerald-500/15 text-emerald-600 dark:text-emerald-400 border-emerald-500/30",
  https: "bg-teal-500/15 text-teal-600 dark:text-teal-400 border-teal-500/30",
  ws: "bg-amber-500/15 text-amber-600 dark:text-amber-400 border-amber-500/30",
  wss: "bg-amber-500/15 text-amber-600 dark:text-amber-400 border-amber-500/30",
};

const fallback = "bg-secondary text-secondary-foreground border-transparent";

export function ProtocolBadge({ protocol, className }: { protocol: string; className?: string }) {
  const key = (protocol || "").toLowerCase();
  const tone = tones[key] ?? fallback;
  return (
    <span
      className={cn(
        "inline-flex items-center rounded-full border px-2.5 py-0.5 text-xs font-medium",
        tone,
        className,
      )}
    >
      {protocol.toUpperCase()}
    </span>
  );
}
