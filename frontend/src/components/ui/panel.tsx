import { forwardRef } from "react";
import { cn } from "@/lib/utils";

/**
 * The basic glass surface. `interactive` adds hover lift for clickable panels.
 * In HUD mode a thin accent light runs along the top edge.
 */
export const Panel = forwardRef<
  HTMLDivElement,
  React.HTMLAttributes<HTMLDivElement> & { interactive?: boolean }
>(({ className, interactive, children, ...props }, ref) => (
  <div
    ref={ref}
    className={cn(
      "glass relative rounded-2xl",
      interactive &&
        "transition-[border-color,transform,background-color] duration-300 ease-out-soft hover:-translate-y-px hover:border-line-strong hover:bg-fill-2",
      className
    )}
    {...props}
  >
    <span
      aria-hidden
      className="pointer-events-none absolute inset-x-6 top-0 hidden h-px bg-gradient-to-r from-transparent via-info/50 to-transparent hud:block"
    />
    {children}
  </div>
));
Panel.displayName = "Panel";

export function PanelHeader({
  title,
  description,
  action,
  className,
}: {
  title: React.ReactNode;
  description?: React.ReactNode;
  action?: React.ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "flex items-start justify-between gap-4 px-5 pt-5",
        className
      )}
    >
      <div className="min-w-0">
        <h2 className="text-[15px] font-semibold text-fg">{title}</h2>
        {description && (
          <p className="mt-0.5 text-[13px] text-fg-3">{description}</p>
        )}
      </div>
      {action}
    </div>
  );
}
