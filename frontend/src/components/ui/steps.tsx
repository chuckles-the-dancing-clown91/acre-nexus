// A row of numbered steps for a wizard. Done steps are ticked, the current
// one is highlighted, and the rest wait.

import { Check } from "lucide-react";
import { cn } from "@/lib/utils";

export function Steps({
  steps,
  current,
  className,
}: {
  steps: string[];
  current: number;
  className?: string;
}) {
  return (
    <ol
      className={cn("flex items-center gap-2", className)}
      aria-label="Progress"
    >
      {steps.map((label, i) => {
        const done = i < current;
        const on = i === current;
        return (
          <li
            key={label}
            className="flex min-w-0 items-center gap-2"
            aria-current={on ? "step" : undefined}
          >
            <span
              className={cn(
                "flex size-6 shrink-0 items-center justify-center rounded-full text-[11px] font-semibold",
                done && "bg-accent text-accent-fg",
                on && "bg-accent/15 text-accent ring-1 ring-accent",
                !done && !on && "bg-fill text-fg-3"
              )}
            >
              {done ? <Check className="size-3.5" /> : i + 1}
            </span>
            <span
              className={cn(
                "truncate text-[13px] font-medium",
                on ? "text-fg" : "text-fg-3",
                !on && "hidden sm:inline"
              )}
            >
              {label}
            </span>
            {i < steps.length - 1 && (
              <span className="h-px w-4 shrink-0 bg-line sm:w-8" />
            )}
          </li>
        );
      })}
    </ol>
  );
}
