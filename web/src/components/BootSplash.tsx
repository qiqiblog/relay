interface Props {
  brand?: string;
}

/**
 * Full-screen bootstrap splash. Rendered on top of an empty page while the
 * Layout fetches branding + server info, so the very first visible frame is
 * a stable loading state instead of placeholder content that gets replaced
 * a moment later.
 */
export default function BootSplash({ brand }: Props) {
  return (
    <div className="fixed inset-0 z-[100] flex flex-col items-center justify-center gap-5 bg-background text-foreground">
      <div className="relative">
        <svg
          viewBox="0 0 32 32"
          fill="none"
          xmlns="http://www.w3.org/2000/svg"
          className="h-12 w-12 animate-pulse drop-shadow-[0_4px_14px_hsl(199_78%_44%/0.45)]"
          aria-hidden
        >
          <defs>
            <linearGradient id="boot-r" x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor="hsl(199 78% 56%)" />
              <stop offset="100%" stopColor="hsl(199 78% 38%)" />
            </linearGradient>
          </defs>
          <path
            fill="url(#boot-r)"
            fillRule="evenodd"
            d="M6 4 V28 H10 V18 H13 L21 28 H26 L17 17.5 Q22 16 22 11 Q22 4 14 4 Z M10 8 H14 Q18 8 18 11 Q18 14 14 14 H10 Z"
          />
        </svg>
        {/* subtle ring spinner around the mark */}
        <div className="pointer-events-none absolute -inset-3 rounded-full border-2 border-transparent border-t-[hsl(199_78%_44%)] border-r-[hsl(199_78%_44%/0.4)] animate-spin [animation-duration:1.2s]" />
      </div>
      {brand && (
        <div className="text-sm font-semibold uppercase tracking-[0.25em] text-muted-foreground">
          {brand}
        </div>
      )}
    </div>
  );
}
