"use client";

// To-dos on a property and what needs attention. Suggestions come from what's
// on file (a permit about to lapse, a policy to renew); one press puts a
// suggestion on the list, and "Not needed" files it away for good.

import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  AlertTriangle,
  Check,
  CircleDot,
  ListChecks,
  Plus,
  RotateCcw,
  Trash2,
} from "lucide-react";
import { toast } from "sonner";
import {
  daysUntil,
  dueLabel,
  label,
  records,
  type ActionItem,
  type Suggestion,
} from "@/lib/propertyRecords";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { cn } from "@/lib/utils";
import { input, why } from "./bits";

export function ActionItems({
  propertyId,
  manage,
}: {
  propertyId: string;
  manage: boolean;
}) {
  const qc = useQueryClient();
  const [showDone, setShowDone] = useState(false);
  const items = useQuery({
    queryKey: ["action-items", propertyId, showDone ? "all" : "open"],
    queryFn: () => records.actionItems(propertyId, showDone ? "all" : "open"),
  });
  const attention = useQuery({
    queryKey: ["attention", propertyId],
    queryFn: () => records.attention(propertyId),
  });
  const [title, setTitle] = useState("");
  const [due, setDue] = useState("");
  const [busy, setBusy] = useState(false);

  const refresh = () => {
    void qc.invalidateQueries({ queryKey: ["action-items", propertyId] });
    void qc.invalidateQueries({ queryKey: ["attention", propertyId] });
  };

  async function add() {
    if (!title.trim()) return;
    setBusy(true);
    try {
      await records.createActionItem(propertyId, {
        title: title.trim(),
        due_on: due || null,
      });
      setTitle("");
      setDue("");
      refresh();
    } catch (e) {
      toast.error(why(e));
    } finally {
      setBusy(false);
    }
  }

  async function take(s: Suggestion, dismiss = false) {
    setBusy(true);
    try {
      const item = await records.createActionItem(propertyId, {
        title: s.title,
        subject_type: s.subject_type,
        subject_id: s.subject_id,
        notes: s.detail,
        due_on: s.due_on,
        priority: s.priority,
        suggestion_key: s.key,
      });
      if (dismiss)
        await records.updateActionItem(propertyId, item.id, {
          status: "dismissed",
        });
      refresh();
    } catch (e) {
      toast.error(why(e));
    } finally {
      setBusy(false);
    }
  }

  async function set(i: ActionItem, status: ActionItem["status"]) {
    try {
      await records.updateActionItem(propertyId, i.id, { status });
      refresh();
    } catch (e) {
      toast.error(why(e));
    }
  }

  async function remove(i: ActionItem) {
    try {
      await records.deleteActionItem(propertyId, i.id);
      refresh();
    } catch (e) {
      toast.error(why(e));
    }
  }

  const suggestions = attention.data ?? [];
  const list = items.data ?? [];

  return (
    <Panel>
      <PanelHeader
        title="To do"
        description="What this property needs, from permits to school zones."
        action={
          <button
            type="button"
            onClick={() => setShowDone((v) => !v)}
            className="text-xs font-medium text-fg-3 hover:text-fg"
          >
            {showDone ? "Hide finished" : "Show finished"}
          </button>
        }
      />
      <div className="space-y-4 p-5 pt-4">
        {suggestions.length > 0 && (
          <div className="rounded-xl border border-warn/25 bg-warn/[0.06] p-3">
            <div className="mb-2 flex items-center gap-1.5 text-xs font-semibold text-warn">
              <AlertTriangle className="size-3.5" />
              Needs attention
            </div>
            <ul className="space-y-2">
              {suggestions.map((s) => (
                <li
                  key={s.key}
                  className="flex flex-col gap-2 rounded-lg bg-surface/70 px-3 py-2 sm:flex-row sm:items-center"
                >
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2 text-[13px] font-medium text-fg">
                      {s.priority === "high" && (
                        <span className="size-1.5 shrink-0 rounded-full bg-bad" />
                      )}
                      {s.title}
                    </div>
                    <div className="text-xs text-fg-3">
                      {s.detail}
                      {s.due_on &&
                        daysUntil(s.due_on) <= 14 &&
                        ` ${dueLabel(s.due_on)}.`}
                    </div>
                  </div>
                  {manage && (
                    <div className="flex shrink-0 gap-1.5">
                      <Button
                        size="sm"
                        variant="secondary"
                        disabled={busy}
                        onClick={() => take(s)}
                      >
                        <Plus />
                        Add
                      </Button>
                      <Button
                        size="sm"
                        variant="ghost"
                        disabled={busy}
                        onClick={() => take(s, true)}
                      >
                        Not needed
                      </Button>
                    </div>
                  )}
                </li>
              ))}
            </ul>
          </div>
        )}

        {manage && (
          <form
            className="flex flex-wrap gap-2"
            onSubmit={(e) => {
              e.preventDefault();
              void add();
            }}
          >
            <input
              className={cn(input, "min-w-0 flex-1")}
              placeholder="Add a to-do, e.g. Walk the roof after the storm"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
            />
            <input
              type="date"
              aria-label="Due date"
              className={cn(input, "w-auto")}
              value={due}
              onChange={(e) => setDue(e.target.value)}
            />
            <Button type="submit" disabled={busy || !title.trim()}>
              Add
            </Button>
          </form>
        )}

        {items.isLoading && <Skeleton className="h-16" />}
        {items.isSuccess && list.length === 0 && suggestions.length === 0 && (
          <EmptyState
            icon={<ListChecks />}
            title="Nothing to do"
            description="Permits, policies, warranties and school zones show up here when they need someone."
            className="py-6"
          />
        )}
        <ul className="divide-y divide-line">
          {list.map((i) => {
            const open = i.status === "open";
            return (
              <li key={i.id} className="flex items-start gap-3 py-2.5">
                <button
                  type="button"
                  disabled={!manage}
                  aria-label={
                    open ? `Mark done: ${i.title}` : `Reopen ${i.title}`
                  }
                  onClick={() => set(i, open ? "done" : "open")}
                  className={cn(
                    "mt-0.5 flex size-5 shrink-0 items-center justify-center rounded-full border transition",
                    open
                      ? "border-line-strong text-transparent hover:border-good hover:text-good"
                      : "border-good bg-good text-white"
                  )}
                >
                  <Check className="size-3" />
                </button>
                <div className="min-w-0 flex-1">
                  <div
                    className={cn(
                      "text-[13px]",
                      open ? "text-fg" : "text-fg-3 line-through"
                    )}
                  >
                    {i.title}
                  </div>
                  <div className="mt-0.5 flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-fg-3">
                    {i.priority === "high" && open && (
                      <Badge tone="bad">high</Badge>
                    )}
                    {i.subject_type !== "property" && (
                      <span className="inline-flex items-center gap-1">
                        <CircleDot className="size-3" />
                        {label(i.subject_type)}
                      </span>
                    )}
                    {open && i.due_on && (
                      <span className={cn(i.overdue && "font-medium text-bad")}>
                        {dueLabel(i.due_on)}
                      </span>
                    )}
                    {i.status === "dismissed" && <span>Not needed</span>}
                    {i.assignee_name && <span>{i.assignee_name}</span>}
                  </div>
                  {i.notes && open && (
                    <div className="mt-1 text-xs text-fg-3">{i.notes}</div>
                  )}
                </div>
                {manage && !open && (
                  <button
                    type="button"
                    aria-label={`Reopen ${i.title}`}
                    onClick={() => set(i, "open")}
                    className="rounded-lg p-1 text-fg-4 hover:text-fg"
                  >
                    <RotateCcw className="size-3.5" />
                  </button>
                )}
                {manage && (
                  <button
                    type="button"
                    aria-label={`Delete ${i.title}`}
                    onClick={() => remove(i)}
                    className="rounded-lg p-1 text-fg-4 hover:text-bad"
                  >
                    <Trash2 className="size-3.5" />
                  </button>
                )}
              </li>
            );
          })}
        </ul>
      </div>
    </Panel>
  );
}
