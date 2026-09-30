"use client";

// The parts loop on one work order: findings ("add finding" with the parts
// it needs), potential and needed parts, "generate a parts list" (from stock
// / to buy / might need), the printable pick list, and per-part moves as
// parts arrive and go in. Close-out decisions live on /console/maintenance/closeout.

import { useState } from "react";
import Link from "next/link";
import { toast } from "sonner";
import type { InventoryItem, TicketDetail, TicketPart } from "@/lib/types";
import {
  parts as partsApi,
  PART_LABELS,
  partTone,
  type PartInput,
  type PartsList,
} from "@/lib/parts";
import { openPdf } from "@/lib/backoffice";
import { Badge, Button, Card } from "@/components/ui";

const field =
  "rounded-xl border border-line bg-surface px-3 py-2 text-sm outline-none focus:border-accent";

interface Row {
  itemId: string;
  name: string;
  quantity: string;
  note: string;
}

const emptyRow = (): Row => ({ itemId: "", name: "", quantity: "1", note: "" });

function rowsToParts(rows: Row[], items: InventoryItem[]): PartInput[] {
  return rows
    .filter((r) => r.itemId || r.name.trim())
    .map((r) => ({
      inventory_item_id: r.itemId || undefined,
      name: r.itemId
        ? items.find((i) => i.id === r.itemId)?.name
        : r.name.trim(),
      quantity: Math.max(1, parseInt(r.quantity, 10) || 1),
      note: r.note.trim() || undefined,
    }));
}

function PartRows({
  rows,
  setRows,
  items,
}: {
  rows: Row[];
  setRows: (r: Row[]) => void;
  items: InventoryItem[];
}) {
  const update = (i: number, patch: Partial<Row>) =>
    setRows(rows.map((r, j) => (j === i ? { ...r, ...patch } : r)));
  return (
    <div className="space-y-2">
      {rows.map((r, i) => (
        <div key={i} className="flex flex-wrap items-center gap-2">
          <select
            className={field}
            value={r.itemId}
            onChange={(e) => update(i, { itemId: e.target.value })}
          >
            <option value="">Not a stock item…</option>
            {items.map((it) => (
              <option key={it.id} value={it.id}>
                {it.name} ({it.quantity} on hand)
              </option>
            ))}
          </select>
          {!r.itemId && (
            <input
              className={`${field} w-56`}
              placeholder="Part, e.g. Baseboard 8ft primed MDF"
              value={r.name}
              onChange={(e) => update(i, { name: e.target.value })}
            />
          )}
          <input
            className={`${field} w-16`}
            type="number"
            min={1}
            value={r.quantity}
            onChange={(e) => update(i, { quantity: e.target.value })}
          />
          <input
            className={`${field} w-44`}
            placeholder="Note (size, color…)"
            value={r.note}
            onChange={(e) => update(i, { note: e.target.value })}
          />
          <Button
            variant="ghost"
            type="button"
            onClick={() => setRows(rows.filter((_, j) => j !== i))}
          >
            ×
          </Button>
        </div>
      ))}
      <Button
        variant="outline"
        type="button"
        onClick={() => setRows([...rows, emptyRow()])}
      >
        + part
      </Button>
    </div>
  );
}

export function PartsCard({
  ticket,
  items,
  manage,
  reload,
}: {
  ticket: TicketDetail;
  items: InventoryItem[];
  manage: boolean;
  reload: () => void;
}) {
  const [busy, setBusy] = useState(false);
  const [mode, setMode] = useState<"none" | "finding" | "part">("none");
  const [findingBody, setFindingBody] = useState("");
  const [visibility, setVisibility] = useState<"internal" | "public">(
    "internal"
  );
  const [rows, setRows] = useState<Row[]>([emptyRow()]);
  const [list, setList] = useState<PartsList | null>(null);

  async function run(fn: () => Promise<unknown>, ok?: string) {
    setBusy(true);
    try {
      await fn();
      if (ok) toast.success(ok);
      reload();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Request failed");
    } finally {
      setBusy(false);
    }
  }

  const findings = ticket.comments.filter((c) => c.kind === "finding");
  const active = ticket.parts.filter((p) => p.status !== "skipped");
  const used = active.filter((p) => p.status === "used");

  function submitFinding(e: React.FormEvent) {
    e.preventDefault();
    if (!findingBody.trim()) {
      toast.error("Describe what you found.");
      return;
    }
    void run(async () => {
      await partsApi.addFinding(ticket.id, {
        body: findingBody.trim(),
        visibility,
        parts: rowsToParts(rows, items),
      });
      setFindingBody("");
      setRows([emptyRow()]);
      setMode("none");
    }, "Finding added.");
  }

  function submitPart(e: React.FormEvent) {
    e.preventDefault();
    const list = rowsToParts(rows, items);
    if (list.length === 0) {
      toast.error("Add at least one part.");
      return;
    }
    void run(async () => {
      for (const p of list)
        await partsApi.add(ticket.id, { ...p, status: "needed" });
      setRows([emptyRow()]);
      setMode("none");
    }, "Parts added.");
  }

  const partLine = (p: TicketPart) => (
    <div
      key={p.id}
      className="flex flex-wrap items-center justify-between gap-2 py-1.5"
    >
      <div className="min-w-0">
        <span className="font-semibold">
          {p.quantity} × {p.name}
        </span>
        <span className="ml-2 text-xs text-ink-3">
          {p.source === "asset"
            ? "from the appliance"
            : p.source === "finding"
              ? "from a finding"
              : p.source === "plan"
                ? "routine"
                : ""}
          {p.in_stock != null ? ` · ${p.in_stock} on the shelf` : ""}
          {p.vendor ? ` · ${p.vendor}` : ""}
          {p.tracking ? ` · ${p.tracking}` : ""}
          {p.ship_to ? ` · to the ${p.ship_to}` : ""}
          {p.note ? ` · ${p.note}` : ""}
        </span>
      </div>
      <div className="flex items-center gap-2">
        <Badge tone={partTone(p.status)}>
          {PART_LABELS[p.status] ?? p.status}
        </Badge>
        {manage && p.status === "potential" && (
          <Button
            variant="outline"
            disabled={busy}
            onClick={() =>
              void run(() => partsApi.update(p.id, { status: "needed" }))
            }
          >
            Need it
          </Button>
        )}
        {manage && (p.status === "ordered" || p.status === "pick_up") && (
          <Button
            variant="outline"
            disabled={busy}
            onClick={() => void run(() => partsApi.receive(p.id), "Received.")}
          >
            Received
          </Button>
        )}
        {manage && ["received", "from_stock"].includes(p.status) && (
          <Button
            disabled={busy}
            onClick={() =>
              void run(() => partsApi.use(p.id), "On the work order.")
            }
          >
            Used
          </Button>
        )}
        {manage && ["potential", "needed", "to_order"].includes(p.status) && (
          <Button
            variant="ghost"
            disabled={busy}
            onClick={() =>
              void run(() => partsApi.update(p.id, { status: "skipped" }))
            }
          >
            Skip
          </Button>
        )}
      </div>
    </div>
  );

  return (
    <Card>
      <div className="flex flex-wrap items-center justify-between gap-2 border-b border-line px-5 py-4">
        <h2 className="font-display text-lg font-bold">
          Findings & parts
          <span className="ml-2 text-sm font-normal text-ink-3">
            {active.length} part{active.length === 1 ? "" : "s"} · {used.length}{" "}
            used
          </span>
        </h2>
        <div className="flex flex-wrap gap-2">
          {manage && (
            <>
              <Button
                variant="outline"
                disabled={busy}
                onClick={() => setMode(mode === "finding" ? "none" : "finding")}
              >
                Add finding
              </Button>
              <Button
                variant="outline"
                disabled={busy}
                onClick={() => setMode(mode === "part" ? "none" : "part")}
              >
                Add part
              </Button>
              <Button
                disabled={busy}
                onClick={() =>
                  void run(async () => {
                    setList(await partsApi.generate(ticket.id));
                  })
                }
              >
                Generate parts list
              </Button>
            </>
          )}
          <Button
            variant="ghost"
            onClick={() => void openPdf(partsApi.pdfPath(ticket.id))}
          >
            Print pick list
          </Button>
        </div>
      </div>
      <div className="space-y-4 p-5 text-sm">
        {mode === "finding" && (
          <form
            onSubmit={submitFinding}
            className="space-y-3 rounded-xl border border-line p-3"
          >
            <div className="flex flex-wrap gap-2">
              <textarea
                className={`${field} min-h-16 flex-1`}
                placeholder="What did you find? e.g. Baseboard behind the dryer is water damaged."
                value={findingBody}
                onChange={(e) => setFindingBody(e.target.value)}
              />
              <select
                className={field}
                value={visibility}
                onChange={(e) =>
                  setVisibility(e.target.value as "internal" | "public")
                }
              >
                <option value="internal">Internal</option>
                <option value="public">Resident sees it</option>
              </select>
            </div>
            <div>
              <div className="mb-1 text-xs font-semibold uppercase tracking-wide text-ink-3">
                Parts it needs
              </div>
              <PartRows rows={rows} setRows={setRows} items={items} />
            </div>
            <div className="flex gap-2">
              <Button type="submit" disabled={busy}>
                Save finding
              </Button>
              <Button
                variant="ghost"
                type="button"
                onClick={() => setMode("none")}
              >
                Cancel
              </Button>
            </div>
          </form>
        )}
        {mode === "part" && (
          <form
            onSubmit={submitPart}
            className="space-y-3 rounded-xl border border-line p-3"
          >
            <PartRows rows={rows} setRows={setRows} items={items} />
            <div className="flex gap-2">
              <Button type="submit" disabled={busy}>
                Add to the list
              </Button>
              <Button
                variant="ghost"
                type="button"
                onClick={() => setMode("none")}
              >
                Cancel
              </Button>
            </div>
          </form>
        )}

        {list && (
          <div className="grid gap-3 rounded-xl border border-accent/40 bg-accent-soft/30 p-3 sm:grid-cols-3">
            {[
              ["From stock", list.from_stock],
              ["To buy", list.to_buy],
              ["Might need", list.maybe],
            ].map(([h, rows]) => (
              <div key={h as string}>
                <div className="text-xs font-semibold uppercase tracking-wide text-ink-3">
                  {h as string}
                </div>
                {(rows as TicketPart[]).length === 0 ? (
                  <div className="text-ink-3">—</div>
                ) : (
                  (rows as TicketPart[]).map((p) => (
                    <div key={p.id}>
                      {p.quantity} × {p.name}
                    </div>
                  ))
                )}
              </div>
            ))}
            {list.to_buy.length > 0 && (
              <div className="sm:col-span-3 text-xs text-ink-3">
                The office decides on the shopping list at{" "}
                <Link
                  href="/console/maintenance/closeout"
                  className="font-semibold text-accent-2 hover:underline"
                >
                  close-out
                </Link>
                : order it to the property or office, pick it up, or pull it
                from stock.
              </div>
            )}
          </div>
        )}

        {findings.length > 0 && (
          <div className="space-y-2">
            {findings.map((f) => (
              <div key={f.id} className="rounded-xl bg-surface-2 px-3 py-2">
                <div className="text-xs text-ink-3">
                  Finding · {f.author_name ?? "tech"} ·{" "}
                  {f.created_at.slice(0, 16).replace("T", " ")}
                </div>
                <div className="whitespace-pre-wrap">{f.body}</div>
                <div className="mt-1 divide-y divide-line">
                  {ticket.parts
                    .filter((p) => p.finding_comment_id === f.id)
                    .map(partLine)}
                </div>
              </div>
            ))}
          </div>
        )}

        <div className="divide-y divide-line">
          {ticket.parts.filter((p) => !p.finding_comment_id).map(partLine)}
          {ticket.parts.length === 0 && (
            <p className="text-ink-3">
              No parts yet. Add a finding with the parts it needs, or list a
              part by hand — then generate the parts list before the truck
              leaves.
            </p>
          )}
        </div>
      </div>
    </Card>
  );
}
