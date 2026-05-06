import * as React from "react";
import { cn } from "@/lib/utils";

interface EmptyStateProps {
  icon?: React.ComponentType<{ className?: string }>;
  title: string;
  description?: string;
  action?: React.ReactNode;
  className?: string;
  compact?: boolean;
}

export function EmptyState({ icon: Icon, title, description, action, className, compact }: EmptyStateProps) {
  return (
    <div
      className={cn(
        "flex flex-col items-center justify-center text-center",
        compact ? "gap-1.5 py-6" : "gap-2 py-10",
        className,
      )}
    >
      {Icon && (
        <div className="rounded-full bg-muted p-2.5 text-muted-foreground">
          <Icon className={compact ? "h-4 w-4" : "h-5 w-5"} />
        </div>
      )}
      <div className={cn("font-medium text-foreground", compact ? "text-sm" : "text-sm")}>{title}</div>
      {description && (
        <div className="text-xs text-muted-foreground max-w-sm">{description}</div>
      )}
      {action && <div className="pt-1">{action}</div>}
    </div>
  );
}
