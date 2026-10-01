"use client";

// Build or change a job kit: the work line by line (trade, time, contractor
// or in-house) and the parts it takes with typical costs. Totals follow along
// as you type; the labor estimate uses the workspace's rates once saved.

import { useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { useQueryClient } from "@tanstack/react-query";
import {
  ArrowDown,
  ArrowLeft,
  ArrowUp,
  HardHat,
  Package,
  Plus,
  Trash2,
  Wrench,
} from "lucide-react";
import { toast } from "sonner";
import {
  desk,
  dollars,
  draftToReq,
  draftTotals,
  KIT_CATEGORIES,
  minutesLabel,
  PRIORITIES,
  TRADES,
  tradeLabel,
  type KitDraft,
} from "@/lib/servicedesk";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/menu";
import { EmptyState } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

const field =
  "w-full rounded-xl border border-line bg-surface px-3 py-2 text-[13px] text-fg outline-none transition focus:border-accent";

type DraftTask = KitDraft["tasks"][number];
type DraftPart = KitDraft["parts"][number];

function move<T>(rows: T[], i: number, by: number): T[] {
  const j = i + by;
  if (j < 0 || j >= rows.length) return rows;
  const next = rows.slice();
  [next[i], next[j]] = [next[j], next[i]];
  return next;
}

export function KitEditor({
  kitId,
  initial,
}: {
  /** The kit being changed; none for a new one. */
  kitId?: string;
  initial: KitDraft;
}) {
  const router = useRouter();
  const qc = useQueryClient();
  const [d, setD] = useState<KitDraft>(initial);
  const [busy, setBusy] = useState(false);
  const [retiring, setRetiring] = useState(false);
  const totals = draftTotals(d);
  const trades = [
    ...new Set(d.tasks.filter((t) => t.title.trim()).map((t) => t.trade)),
  ];

  const set = <K extends keyof KitDraft>(k: K, v: KitDraft[K]) =>
    setD((x) => ({ ...x, [k]: v }));
  const setTask = (i: number, patch: Partial<DraftTask>) =>
    set(
      "tasks",
      d.tasks.map((t, j) => (j === i ? { ...t, ...patch } : t))
    );
  const setPart = (i: number, patch: Partial<DraftPart>) =>
    set(
      "parts",
      d.parts.map((p, j) => (j === i ? { ...p, ...patch } : p))
    );
  const addTask = () =>
    set("tasks", [
      ...d.tasks,
      {
        title: "",
        trade: d.tasks.at(-1)?.trade ?? "general",
        minutes: "",
        needs_contractor: false,
      },
    ]);
  const addPart = () =>
    set("parts", [
      ...d.parts,
      { name: "", quantity: "1", cost: "", inventory_item_id: null },
    ]);

  async function save() {
    const body = draftToReq(d);
    if (!body.name) {
      toast.error("Name the kit.");
      return;
    }
    if (body.tasks.length === 0) {
      toast.error("Add at least one task.");
      return;
    }
    setBusy(true);
    try {
      const saved = kitId
        ? await desk.updateKit(kitId, body)
        : await desk.createKit(body);
      await qc.invalidateQueries({ queryKey: ["kits"] });
      toast.success(`${saved.name} saved · estimate ${saved.est_total_label}`);
      router.push("/console/maintenance/kits");
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't save it");
      setBusy(false);
    }
  }

  async function retire() {
    if (!kitId) return;
    setBusy(true);
    try {
      await desk.retireKit(kitId);
      await qc.invalidateQueries({ queryKey: ["kits"] });
      toast.success(`${d.name} retired`);
      router.push("/console/maintenance/kits");
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't retire it");
      setBusy(false);
    }
  }

  return (
    <div className="space-y-6">
      <Link
        href="/console/maintenance/kits"
        className="inline-flex items-center gap-1.5 text-[13px] text-fg-3 transition hover:text-fg"
      >
        <ArrowLeft className="size-4" />
        Job kits
      </Link>

      <header className="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between">
        <div>
          <div className="eyebrow mb-1">Job kit</div>
          <h1 className="text-[26px] leading-tight font-semibold text-fg sm:text-[30px]">
            {kitId ? d.name || "Untitled kit" : "New job kit"}
          </h1>
        </div>
        <div className="flex gap-2">
          {kitId && (
            <Button
              variant="ghost"
              onClick={() => setRetiring(true)}
              disabled={busy}
            >
              <Trash2 />
              Retire
            </Button>
          )}
          <Button onClick={save} disabled={busy}>
            Save kit
          </Button>
        </div>
      </header>

      <div className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_300px]">
        <div className="space-y-4">
          <Panel>
            <PanelHeader title="Details" />
            <div className="grid gap-3 p-5 md:grid-cols-2">
              <input
                className={cn(field, "md:col-span-2")}
                placeholder="Name, e.g. Shower replacement"
                aria-label="Name"
                value={d.name}
                onChange={(e) => set("name", e.target.value)}
              />
              <input
                className={field}
                placeholder="Where, e.g. Bathroom"
                aria-label="Area"
                value={d.area}
                onChange={(e) => set("area", e.target.value)}
              />
              <div className="grid grid-cols-2 gap-3">
                <select
                  className={field}
                  aria-label="Category"
                  value={d.category}
                  onChange={(e) => set("category", e.target.value)}
                >
                  {KIT_CATEGORIES.map((c) => (
                    <option key={c} value={c}>
                      {tradeLabel(c)}
                    </option>
                  ))}
                </select>
                <select
                  className={field}
                  aria-label="Priority"
                  value={d.priority}
                  onChange={(e) => set("priority", e.target.value)}
                >
                  {PRIORITIES.map((p) => (
                    <option key={p} value={p}>
                      {p} priority
                    </option>
                  ))}
                </select>
              </div>
              <textarea
                className={cn(field, "min-h-20 md:col-span-2")}
                placeholder="What this job covers (goes on the work order)"
                aria-label="Description"
                value={d.description}
                onChange={(e) => set("description", e.target.value)}
              />
            </div>
          </Panel>

          <Panel>
            <PanelHeader
              title="Tasks"
              description="In order. Mark the ones a contractor does."
              action={
                <Button size="sm" variant="secondary" onClick={addTask}>
                  <Plus />
                  Add task
                </Button>
              }
            />
            <div className="p-3 pt-4">
              {d.tasks.length === 0 && (
                <EmptyState
                  icon={<Wrench />}
                  title="No tasks yet"
                  description="Break the job into steps: demo, rough-in, drywall, paint."
                  className="py-6"
                />
              )}
              <ol className="space-y-2">
                {d.tasks.map((t, i) => (
                  <li
                    key={i}
                    className="grid grid-cols-[1.5rem_minmax(0,1fr)_4.5rem_auto] items-center gap-2 rounded-xl border border-line bg-fill/40 p-2 md:grid-cols-[1.5rem_minmax(0,1fr)_9rem_5.5rem_auto_auto]"
                  >
                    <span className="figure text-right text-xs text-fg-4">
                      {i + 1}
                    </span>
                    <input
                      className={cn(field, "col-span-2 md:col-span-1")}
                      placeholder="What gets done"
                      aria-label={`Task ${i + 1}`}
                      value={t.title}
                      onChange={(e) => setTask(i, { title: e.target.value })}
                      onKeyDown={(e) => {
                        if (e.key === "Enter" && i === d.tasks.length - 1) {
                          e.preventDefault();
                          addTask();
                        }
                      }}
                    />
                    <button
                      type="button"
                      onClick={() =>
                        setTask(i, { needs_contractor: !t.needs_contractor })
                      }
                      aria-pressed={t.needs_contractor}
                      aria-label={`Task ${i + 1} needs a contractor`}
                      title={t.needs_contractor ? "Contractor" : "In-house"}
                      className={cn(
                        "rounded-lg border p-2 transition md:order-last",
                        t.needs_contractor
                          ? "border-warn/40 bg-warn/10 text-warn"
                          : "border-line text-fg-4 hover:text-fg"
                      )}
                    >
                      <HardHat className="size-4" />
                    </button>
                    <select
                      className={cn(field, "col-start-2 md:col-start-auto")}
                      aria-label={`Task ${i + 1} trade`}
                      value={t.trade}
                      onChange={(e) => setTask(i, { trade: e.target.value })}
                    >
                      {TRADES.map((tr) => (
                        <option key={tr} value={tr}>
                          {tradeLabel(tr)}
                        </option>
                      ))}
                    </select>
                    <input
                      className={field}
                      inputMode="numeric"
                      placeholder="min"
                      aria-label={`Task ${i + 1} minutes`}
                      value={t.minutes}
                      onChange={(e) =>
                        setTask(i, {
                          minutes: e.target.value.replace(/[^0-9]/g, ""),
                        })
                      }
                    />
                    <div className="flex gap-0.5">
                      <RowButton
                        label={`Move task ${i + 1} up`}
                        onClick={() => set("tasks", move(d.tasks, i, -1))}
                        disabled={i === 0}
                      >
                        <ArrowUp />
                      </RowButton>
                      <RowButton
                        label={`Move task ${i + 1} down`}
                        onClick={() => set("tasks", move(d.tasks, i, 1))}
                        disabled={i === d.tasks.length - 1}
                      >
                        <ArrowDown />
                      </RowButton>
                      <RowButton
                        label={`Remove task ${i + 1}`}
                        onClick={() =>
                          set(
                            "tasks",
                            d.tasks.filter((_, j) => j !== i)
                          )
                        }
                      >
                        <Trash2 />
                      </RowButton>
                    </div>
                  </li>
                ))}
              </ol>
            </div>
          </Panel>

          <Panel>
            <PanelHeader
              title="Parts"
              description="Matched to stock by name when a work order opens. The cost is a typical price each."
              action={
                <Button size="sm" variant="secondary" onClick={addPart}>
                  <Plus />
                  Add part
                </Button>
              }
            />
            <div className="p-3 pt-4">
              {d.parts.length === 0 && (
                <EmptyState
                  icon={<Package />}
                  title="No parts"
                  description="List what the job uses so the shopping list builds itself."
                  className="py-6"
                />
              )}
              <ul className="space-y-2">
                {d.parts.map((p, i) => (
                  <li
                    key={i}
                    className="grid grid-cols-[4rem_minmax(0,1fr)_auto] items-center gap-2 sm:grid-cols-[4rem_minmax(0,1fr)_7rem_auto]"
                  >
                    <input
                      className={cn(field, "text-right")}
                      inputMode="numeric"
                      aria-label={`Part ${i + 1} quantity`}
                      value={p.quantity}
                      onChange={(e) =>
                        setPart(i, {
                          quantity: e.target.value.replace(/[^0-9]/g, ""),
                        })
                      }
                    />
                    <input
                      className={field}
                      placeholder="Part"
                      aria-label={`Part ${i + 1}`}
                      value={p.name}
                      onChange={(e) =>
                        setPart(i, {
                          name: e.target.value,
                          // A renamed part is a different part.
                          inventory_item_id:
                            e.target.value === p.name
                              ? p.inventory_item_id
                              : null,
                        })
                      }
                      onKeyDown={(e) => {
                        if (e.key === "Enter" && i === d.parts.length - 1) {
                          e.preventDefault();
                          addPart();
                        }
                      }}
                    />
                    <input
                      className={cn(field, "col-start-2 sm:col-start-auto")}
                      inputMode="decimal"
                      placeholder="$ each"
                      aria-label={`Part ${i + 1} cost each`}
                      value={p.cost}
                      onChange={(e) => setPart(i, { cost: e.target.value })}
                    />
                    <RowButton
                      label={`Remove part ${i + 1}`}
                      onClick={() =>
                        set(
                          "parts",
                          d.parts.filter((_, j) => j !== i)
                        )
                      }
                    >
                      <Trash2 />
                    </RowButton>
                  </li>
                ))}
              </ul>
            </div>
          </Panel>
        </div>

        <aside className="xl:sticky xl:top-4 xl:self-start">
          <Panel className="space-y-4 p-5">
            <div className="eyebrow">What it takes</div>
            <dl className="space-y-2 text-[13px]">
              <Row label="In-house time">
                {minutesLabel(totals.minutes) || "—"}
              </Row>
              <Row label="Contractor time">
                {minutesLabel(totals.contractorMinutes) || "—"}
              </Row>
              <Row label="Parts">
                {totals.partsCents ? dollars(totals.partsCents) : "—"}
              </Row>
            </dl>
            <p className="text-xs text-fg-3">
              Labor is priced at your in-house and contractor rates when the kit
              is saved.
            </p>
            {trades.length > 0 && (
              <div className="flex flex-wrap gap-1">
                {trades.map((t) => (
                  <Badge
                    key={t}
                    tone={
                      d.tasks.some(
                        (x) =>
                          x.trade === t && x.needs_contractor && x.title.trim()
                      )
                        ? "warn"
                        : "neutral"
                    }
                  >
                    {tradeLabel(t)}
                  </Badge>
                ))}
              </div>
            )}
          </Panel>
        </aside>
      </div>

      <Dialog open={retiring} onOpenChange={setRetiring}>
        <DialogContent>
          <DialogTitle className="text-[17px] font-semibold">
            Retire {d.name || "this kit"}?
          </DialogTitle>
          <DialogDescription className="mt-2 text-[13px] text-fg-3">
            It leaves the catalog. Work orders and routines already made from it
            keep their tasks and parts.
          </DialogDescription>
          <div className="mt-5 flex justify-end gap-2">
            <Button variant="ghost" onClick={() => setRetiring(false)}>
              Keep it
            </Button>
            <Button variant="danger" onClick={retire} disabled={busy}>
              Retire kit
            </Button>
          </div>
        </DialogContent>
      </Dialog>
    </div>
  );
}

function RowButton({
  label,
  onClick,
  disabled,
  children,
}: {
  label: string;
  onClick: () => void;
  disabled?: boolean;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      aria-label={label}
      className="rounded-lg p-1.5 text-fg-3 transition hover:bg-fill-2 hover:text-fg disabled:opacity-30 [&_svg]:size-4"
    >
      {children}
    </button>
  );
}

function Row({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex justify-between gap-3">
      <dt className="text-fg-3">{label}</dt>
      <dd className="figure font-medium text-fg">{children}</dd>
    </div>
  );
}
