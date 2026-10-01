"use client";

// The fixed backdrop the glass floats over. Its aura colour follows whatever
// the current page reports about portfolio health (see `useUiStore.aura`), and
// eases between colours via the registered `--aura` custom property.

import { useUiStore } from "@/lib/store";

export function Ambient() {
  const aura = useUiStore((s) => s.aura);
  return (
    <div
      aria-hidden
      className="ambient"
      style={
        aura
          ? ({ "--aura": `var(--${aura})` } as React.CSSProperties)
          : undefined
      }
    >
      <div className="ambient-grid" />
    </div>
  );
}
