"use client";

import { useTheme } from "@/lib/theme";
import { Icon } from "./Icon";

export function ThemeToggle() {
  const { dark, toggleDark } = useTheme();
  return (
    <button
      onClick={toggleDark}
      title="Toggle theme"
      className="flex h-9 w-9 items-center justify-center rounded-xl border border-line bg-surface-2 text-ink-2 hover:text-ink"
    >
      <Icon name={dark ? "sun" : "moon"} size={18} />
    </button>
  );
}
