import { cn } from "@/lib/utils";
import { Shield, User as UserIcon } from "lucide-react";
import { useEffect, useState } from "react";

export type Area = "admin" | "user";

interface Props {
  value: Area;
  onChange: (next: Area) => void;
  className?: string;
}

const SEGMENTS: { key: Area; label: string; icon: React.ComponentType<{ className?: string }> }[] = [
  { key: "admin", label: "管理", icon: Shield },
  { key: "user", label: "用户", icon: UserIcon },
];

export default function AreaSwitcher({ value, onChange, className }: Props) {
  const idx = SEGMENTS.findIndex((s) => s.key === value);
  const [pressed, setPressed] = useState(false);
  // Mount entrance: thumb scales from a small dot to its full size — Dynamic-Island vibe.
  const [mounted, setMounted] = useState(false);
  useEffect(() => {
    const id = requestAnimationFrame(() => setMounted(true));
    return () => cancelAnimationFrame(id);
  }, []);

  return (
    <div
      role="tablist"
      aria-label="区域切换"
      className={cn(
        "group relative inline-flex items-center rounded-full p-1",
        // gradient border via background-clip trick
        "border border-transparent bg-clip-padding",
        "bg-background/70 supports-[backdrop-filter]:bg-background/40 backdrop-blur-xl backdrop-saturate-150",
        "shadow-[0_10px_32px_-12px_rgba(0,0,0,0.28),0_2px_6px_-1px_rgba(0,0,0,0.08)]",
        "ring-1 ring-black/[0.04] dark:ring-white/[0.06]",
        "dark:bg-white/[0.04] dark:supports-[backdrop-filter]:bg-white/[0.04]",
        "dark:shadow-[0_10px_32px_-12px_rgba(0,0,0,0.65),0_0_0_1px_rgba(255,255,255,0.04)]",
        // subtle gradient halo
        "before:pointer-events-none before:absolute before:inset-0 before:rounded-full",
        "before:bg-gradient-to-b before:from-white/60 before:to-transparent before:opacity-60 dark:before:from-white/[0.08] dark:before:opacity-100",
        "before:[mask:linear-gradient(#000,transparent_60%)]",
        className,
      )}
    >
      {/* sliding thumb */}
      <div
        aria-hidden
        className={cn(
          "absolute inset-y-1 w-[calc(50%-4px)] rounded-full",
          "bg-foreground",
          "shadow-[0_4px_12px_-2px_rgba(0,0,0,0.28),0_1px_2px_rgba(0,0,0,0.08)]",
          "dark:shadow-[0_4px_12px_-2px_rgba(0,0,0,0.65),inset_0_1px_0_rgba(255,255,255,0.06)]",
          "transition-[transform,width,opacity] duration-[420ms] ease-[cubic-bezier(0.34,1.56,0.64,1)]",
          mounted ? "opacity-100" : "opacity-0",
        )}
        style={{
          transform: `translateX(${idx * 100}%) scale(${mounted ? (pressed ? 0.94 : 1) : 0.4})`,
          transformOrigin: "center",
        }}
      />
      {SEGMENTS.map((s) => {
        const active = s.key === value;
        return (
          <button
            key={s.key}
            type="button"
            role="tab"
            aria-selected={active}
            onPointerDown={() => setPressed(true)}
            onPointerUp={() => setPressed(false)}
            onPointerLeave={() => setPressed(false)}
            onClick={() => onChange(s.key)}
            className={cn(
              "relative z-10 inline-flex items-center gap-2 rounded-full px-5 py-1.5 text-sm font-medium",
              "transition-[color,transform] duration-200",
              active ? "text-background" : "text-muted-foreground hover:text-foreground",
              pressed && active ? "scale-[0.97]" : "scale-100",
            )}
          >
            <s.icon className={cn("h-4 w-4 transition-transform duration-300", active && "scale-110")} />
            {s.label}
          </button>
        );
      })}
    </div>
  );
}
