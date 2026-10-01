// Small presentational pieces: skeletons, keyboard hints, tooltips, page
// headers, and empty states.

import * as TooltipPrimitive from "@radix-ui/react-tooltip";
import { cn } from "@/lib/utils";

export function Skeleton({ className }: { className?: string }) {
  return (
    <div
      aria-hidden
      className={cn(
        "rounded-lg bg-[linear-gradient(90deg,var(--fill)_0%,var(--fill-2)_50%,var(--fill)_100%)] bg-[length:200%_100%] animate-[shimmer_1.6s_ease-in-out_infinite]",
        className
      )}
    />
  );
}

export function Kbd({
  children,
  className,
}: {
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <kbd
      className={cn(
        "inline-flex h-5 min-w-5 items-center justify-center rounded-md border border-line-strong bg-fill px-1.5 font-mono text-[10px] font-medium text-fg-3",
        className
      )}
    >
      {children}
    </kbd>
  );
}

export function Tooltip({
  content,
  side = "bottom",
  children,
}: {
  content: React.ReactNode;
  side?: "top" | "right" | "bottom" | "left";
  children: React.ReactNode;
}) {
  return (
    <TooltipPrimitive.Root>
      <TooltipPrimitive.Trigger asChild>{children}</TooltipPrimitive.Trigger>
      <TooltipPrimitive.Portal>
        <TooltipPrimitive.Content
          side={side}
          sideOffset={8}
          className="glass-strong z-50 rounded-lg px-2.5 py-1.5 text-xs text-fg animate-[fade-in_150ms_ease-out]"
        >
          {content}
        </TooltipPrimitive.Content>
      </TooltipPrimitive.Portal>
    </TooltipPrimitive.Root>
  );
}

export function PageHeader({
  eyebrow,
  title,
  description,
  actions,
}: {
  eyebrow?: React.ReactNode;
  title: React.ReactNode;
  description?: React.ReactNode;
  actions?: React.ReactNode;
}) {
  return (
    <header className="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between">
      <div className="min-w-0">
        {eyebrow && <div className="eyebrow mb-2">{eyebrow}</div>}
        <h1 className="text-[28px] leading-tight font-semibold text-fg sm:text-[32px]">
          {title}
        </h1>
        {description && (
          <div className="mt-1.5 max-w-2xl text-[15px] text-fg-2">
            {description}
          </div>
        )}
      </div>
      {actions && (
        <div className="flex shrink-0 items-center gap-2">{actions}</div>
      )}
    </header>
  );
}

export function EmptyState({
  icon,
  title,
  description,
  action,
  className,
}: {
  icon?: React.ReactNode;
  title: string;
  description?: React.ReactNode;
  action?: React.ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "flex flex-col items-center px-6 py-14 text-center",
        className
      )}
    >
      {icon && (
        <div className="mb-4 flex size-12 items-center justify-center rounded-2xl border border-line-strong bg-fill text-fg-2 [&_svg]:size-5">
          {icon}
        </div>
      )}
      <h3 className="text-[15px] font-semibold text-fg">{title}</h3>
      {description && (
        <p className="mt-1 max-w-sm text-[13px] text-fg-3">{description}</p>
      )}
      {action && <div className="mt-5">{action}</div>}
    </div>
  );
}
