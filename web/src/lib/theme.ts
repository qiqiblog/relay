import { useEffect, useState } from "react";

type Theme = "light" | "dark";

function getInitial(): Theme {
  const stored = localStorage.getItem("theme");
  if (stored === "light" || stored === "dark") return stored;
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

function apply(theme: Theme) {
  document.documentElement.classList.toggle("dark", theme === "dark");
}

// 从按钮中心向四周扩散的 clip-path 动画（View Transitions API）
function toggleWithAnimation(next: Theme, originX: number, originY: number) {
  const maxR = Math.hypot(
    Math.max(originX, window.innerWidth - originX),
    Math.max(originY, window.innerHeight - originY),
  );

  if (!document.startViewTransition) {
    apply(next);
    return;
  }

  const transition = document.startViewTransition(() => {
    apply(next);
  });

  transition.ready.then(() => {
    const clipPath = [
      `circle(0px at ${originX}px ${originY}px)`,
      `circle(${maxR}px at ${originX}px ${originY}px)`,
    ];
    document.documentElement.animate(
      { clipPath },
      {
        duration: 400,
        easing: "ease-in-out",
        pseudoElement: "::view-transition-new(root)",
      },
    );
  });
}

export function useTheme() {
  const [theme, setTheme] = useState<Theme>(() => {
    const t = getInitial();
    apply(t);
    return t;
  });

  useEffect(() => {
    localStorage.setItem("theme", theme);
  }, [theme]);

  const toggle = (e: React.MouseEvent) => {
    const next: Theme = theme === "dark" ? "light" : "dark";
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const x = rect.left + rect.width / 2;
    const y = rect.top + rect.height / 2;
    setTheme(next);
    toggleWithAnimation(next, x, y);
  };

  return { theme, toggle };
}
