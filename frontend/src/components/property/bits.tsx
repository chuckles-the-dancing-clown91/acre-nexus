"use client";

// Small pieces the property profile sections share: a labelled form field,
// a fact row, and a form dialog with save and delete.

import { Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/menu";
import { cn } from "@/lib/utils";

export const input =
  "w-full rounded-xl border border-line bg-surface px-3 py-2 text-[13px] text-fg outline-none focus:border-accent";

export function F({
  label,
  className,
  children,
}: {
  label: string;
  className?: string;
  children: React.ReactNode;
}) {
  return (
    <label className={cn("block", className)}>
      <span className="mb-1 block text-xs font-medium text-fg-3">{label}</span>
      {children}
    </label>
  );
}

/** A label and its value; nothing at all when there's no value. */
export function Fact({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  if (children == null || children === "" || children === false) return null;
  return (
    <div className="flex items-baseline justify-between gap-4 py-1.5 text-[13px]">
      <dt className="shrink-0 text-fg-3">{label}</dt>
      <dd className="min-w-0 text-right text-fg">{children}</dd>
    </div>
  );
}

export function FormDialog({
  open,
  onOpenChange,
  title,
  description,
  busy,
  onSave,
  onDelete,
  wide,
  children,
}: {
  open: boolean;
  onOpenChange: (o: boolean) => void;
  title: string;
  description?: string;
  busy?: boolean;
  onSave: () => void;
  onDelete?: () => void;
  wide?: boolean;
  children: React.ReactNode;
}) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        className={cn("max-h-[90dvh] overflow-y-auto", wide && "max-w-2xl")}
      >
        <DialogTitle className="text-[17px] font-semibold">{title}</DialogTitle>
        {description && (
          <DialogDescription className="mt-1 text-[13px] text-fg-3">
            {description}
          </DialogDescription>
        )}
        <form
          className="mt-4"
          onSubmit={(e) => {
            e.preventDefault();
            onSave();
          }}
        >
          <div className="grid gap-3 sm:grid-cols-2">{children}</div>
          <div className="mt-5 flex items-center gap-2">
            {onDelete && (
              <Button
                type="button"
                variant="ghost"
                className="text-bad"
                disabled={busy}
                onClick={onDelete}
              >
                <Trash2 />
                Delete
              </Button>
            )}
            <Button
              type="button"
              variant="ghost"
              className="ml-auto"
              onClick={() => onOpenChange(false)}
            >
              Cancel
            </Button>
            <Button type="submit" disabled={busy}>
              Save
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}

/** Error text from anything thrown. */
export function why(e: unknown, fallback = "Couldn't save it"): string {
  return e instanceof Error ? e.message : fallback;
}
