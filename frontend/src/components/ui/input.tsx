import { forwardRef, useId } from "react";
import * as LabelPrimitive from "@radix-ui/react-label";
import { cn } from "@/lib/utils";

export const Input = forwardRef<
  HTMLInputElement,
  React.InputHTMLAttributes<HTMLInputElement>
>(({ className, ...props }, ref) => (
  <input
    ref={ref}
    className={cn(
      "h-11 w-full rounded-xl border border-line-strong bg-fill px-3.5 text-sm text-fg outline-none transition-[border-color,background-color,box-shadow] duration-200 placeholder:text-fg-4",
      "hover:border-fg-4 focus:border-accent focus:bg-fill-2 focus:shadow-[0_0_0_4px_color-mix(in_oklab,var(--accent)_18%,transparent)] focus-visible:outline-none",
      "aria-[invalid=true]:border-bad/60 disabled:opacity-50",
      className
    )}
    {...props}
  />
));
Input.displayName = "Input";

export function Label({
  className,
  ...props
}: React.ComponentProps<typeof LabelPrimitive.Root>) {
  return (
    <LabelPrimitive.Root
      className={cn("text-[13px] font-medium text-fg-2", className)}
      {...props}
    />
  );
}

/** Label + control + optional hint/error, wired up for screen readers. */
export function Field({
  label,
  hint,
  error,
  trailing,
  children,
}: {
  label: string;
  hint?: string;
  error?: string | null;
  /** Something aligned with the label on the right, e.g. a "Forgot?" link. */
  trailing?: React.ReactNode;
  children: (props: {
    id: string;
    "aria-invalid"?: boolean;
    "aria-describedby"?: string;
  }) => React.ReactNode;
}) {
  const id = useId();
  const noteId = `${id}-note`;
  const note = error ?? hint;
  return (
    <div className="space-y-1.5">
      <div className="flex items-center justify-between">
        <Label htmlFor={id}>{label}</Label>
        {trailing}
      </div>
      {children({
        id,
        "aria-invalid": error ? true : undefined,
        "aria-describedby": note ? noteId : undefined,
      })}
      {note && (
        <p
          id={noteId}
          className={cn("text-xs", error ? "text-bad" : "text-fg-3")}
        >
          {note}
        </p>
      )}
    </div>
  );
}
