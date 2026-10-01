"use client";

// Close-out: the night before. Every work order due tomorrow (or already
// overdue) with its parts; the office decides each one — order it (vendor,
// tracking, ship to the property or the office), pick it up, pull it from
// stock, or skip it — and marks orders received as they arrive.

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { toast } from "sonner";
import type { TicketPart } from "@/lib/types";
import {
  parts as partsApi,
  PART_LABELS,
  partTone,
  type Closeout,
  type DecideInput,
} from "@/lib/parts";
import { money, toCents } from "@/lib/backoffice";
import { useAuth } from "@/lib/auth";
import { Badge, Button, Card, StatTile } from "@/components/ui";

const field =
  "rounded-xl border border-line bg-surface px-3 py-2 text-sm outline-none focus:border-accent";

function tomorrow() {
  const d = new Date();
  d.setDate(d.getDate() + 1);
  return d.toISOString().slice(0, 10);
}

export default function CloseoutPage() {
  const { can } = useAuth();
  const manage = can("maintenance:manage");
  const [date, setDate] = useState(tomorrow());
  const [data, setData] = useState<Closeout | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [deciding, setDeciding] = useState<string | null>(null);

  const load = useCallback(() => {
    partsApi
      .closeout(date)
      .then(setData)
      .catch((e: Error) => setError(e.message));
  }, [date]);
  useEffect(load, [load]);

  async function run(fn: () => Promise<unknown>, ok?: string) {
    setBusy(true);
    try {
      await fn();
      if (ok) toast.success(ok);
      setDeciding(null);
      load();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Request failed");
    } finally {
      setBusy(false);
    }
  }

  const undecided = (p: TicketPart) =>
    ["needed", "to_order", "potential", "from_stock", "pick_up"].includes(
      p.status
    );

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <Link
            href="/console/maintenance"
            className="text-sm font-semibold text-ink-2 hover:underline"
          >
            ← Maintenance
          </Link>
          <h1 className="font-display text-3xl font-extrabold tracking-tight">
            Close-out
          </h1>
          <p className="text-sm text-ink-3">
            Tomorrow&apos;s work and what it still needs. Decide each part
            tonight so the truck leaves loaded.
          </p>
        </div>
        <label className="flex items-center gap-2 text-sm text-ink-3">
          Work due by
          <input
            type="date"
            className={field}
            value={date}
            onChange={(e) => setDate(e.target.value)}
          />
        </label>
      </div>

      {error && <p className="text-bad">{error}</p>}
      {data && (
        <>
          <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
            <StatTile label="Work orders" value={`${data.tickets.length}`} />
            <StatTile label="Parts to decide" value={`${data.to_decide}`} />
            <StatTile label="On order" value={`${data.on_order.length}`} />
            <StatTile
              label="To buy (est.)"
              value={money(
                data.tickets
                  .flatMap((t) => t.parts)
                  .filter((p) => p.status === "to_order")
                  .reduce(
                    (s, p) => s + (p.unit_cost_cents ?? 0) * p.quantity,
                    0
                  )
              )}
            />
          </div>

          {data.tickets.length === 0 && (
            <Card className="p-5 text-sm text-ink-3">
              Nothing due by {date} needs parts. Good night.
            </Card>
          )}

          {data.tickets.map((t) => (
            <Card key={t.ticket_id}>
              <div className="flex flex-wrap items-center justify-between gap-2 border-b border-line px-5 py-4">
                <div className="min-w-0">
                  <Link
                    href={`/console/maintenance/${t.ticket_id}`}
                    className="font-display text-lg font-bold hover:underline"
                  >
                    {t.title}
                  </Link>
                  <div className="text-sm text-ink-3">
                    {t.property}
                    {t.property_address ? ` · ${t.property_address}` : ""}
                    {t.assignee ? ` · ${t.assignee}` : ""}
                    {t.due_date ? ` · due ${t.due_date}` : ""}
                  </div>
                </div>
                <div className="flex gap-2">
                  <Badge
                    tone={
                      t.priority === "urgent"
                        ? "bad"
                        : t.priority === "high"
                          ? "warn"
                          : "neutral"
                    }
                  >
                    {t.priority}
                  </Badge>
                  <Badge tone="info">{t.status.replace("_", " ")}</Badge>
                </div>
              </div>
              <div className="divide-y divide-line p-5 text-sm">
                {t.parts.length === 0 && (
                  <p className="text-ink-3">
                    No parts listed — the tech hasn&apos;t generated a parts
                    list.
                  </p>
                )}
                {t.parts.map((p) => (
                  <div key={p.id} className="py-2">
                    <div className="flex flex-wrap items-center justify-between gap-2">
                      <div>
                        <span className="font-semibold">
                          {p.quantity} × {p.name}
                        </span>
                        <span className="ml-2 text-xs text-ink-3">
                          {p.in_stock != null
                            ? `${p.in_stock} on the shelf`
                            : "not a stock item"}
                          {p.note ? ` · ${p.note}` : ""}
                          {p.vendor ? ` · ${p.vendor}` : ""}
                          {p.ship_to ? ` · to the ${p.ship_to}` : ""}
                        </span>
                      </div>
                      <div className="flex items-center gap-2">
                        <Badge tone={partTone(p.status)}>
                          {PART_LABELS[p.status] ?? p.status}
                        </Badge>
                        {manage && undecided(p) && (
                          <Button
                            variant="outline"
                            disabled={busy}
                            onClick={() =>
                              setDeciding(deciding === p.id ? null : p.id)
                            }
                          >
                            Decide
                          </Button>
                        )}
                        {manage &&
                          (p.status === "ordered" ||
                            p.status === "pick_up") && (
                            <Button
                              variant="outline"
                              disabled={busy}
                              onClick={() =>
                                void run(
                                  () => partsApi.receive(p.id),
                                  "Received."
                                )
                              }
                            >
                              Received
                            </Button>
                          )}
                      </div>
                    </div>
                    {deciding === p.id && (
                      <DecideForm
                        part={p}
                        busy={busy}
                        onDecide={(input) =>
                          void run(
                            () => partsApi.decide(p.id, input),
                            "Decided."
                          )
                        }
                        onCancel={() => setDeciding(null)}
                      />
                    )}
                  </div>
                ))}
              </div>
            </Card>
          ))}

          {data.on_order.length > 0 && (
            <Card>
              <div className="border-b border-line px-5 py-4 font-display text-lg font-bold">
                On order
              </div>
              <div className="divide-y divide-line p-5 text-sm">
                {data.on_order.map((p) => (
                  <div
                    key={p.id}
                    className="flex flex-wrap items-center justify-between gap-2 py-2"
                  >
                    <div>
                      <span className="font-semibold">
                        {p.quantity} × {p.name}
                      </span>
                      <span className="ml-2 text-xs text-ink-3">
                        {p.vendor ?? ""}
                        {p.tracking ? ` · ${p.tracking}` : ""}
                        {p.ship_to ? ` · to the ${p.ship_to}` : ""}
                        {p.ordered_at
                          ? ` · ordered ${p.ordered_at.slice(0, 10)}`
                          : ""}
                      </span>
                    </div>
                    <div className="flex items-center gap-2">
                      <Badge tone={partTone(p.status)}>
                        {PART_LABELS[p.status]}
                      </Badge>
                      {manage && (
                        <Button
                          variant="outline"
                          disabled={busy}
                          onClick={() =>
                            void run(() => partsApi.receive(p.id), "Received.")
                          }
                        >
                          Received
                        </Button>
                      )}
                    </div>
                  </div>
                ))}
              </div>
            </Card>
          )}
        </>
      )}
    </div>
  );
}

function DecideForm({
  part,
  busy,
  onDecide,
  onCancel,
}: {
  part: TicketPart;
  busy: boolean;
  onDecide: (input: DecideInput) => void;
  onCancel: () => void;
}) {
  const canStock =
    part.inventory_item_id != null && (part.in_stock ?? 0) >= part.quantity;
  const [action, setAction] = useState<DecideInput["action"]>(
    canStock ? "from_stock" : "order"
  );
  const [vendor, setVendor] = useState(part.vendor ?? "Home Depot");
  const [tracking, setTracking] = useState("");
  const [shipTo, setShipTo] = useState<"property" | "office" | "other">(
    "property"
  );
  const [shipNote, setShipNote] = useState("");
  const [cost, setCost] = useState(
    part.unit_cost_cents != null ? (part.unit_cost_cents / 100).toFixed(2) : ""
  );
  const [billable, setBillable] = useState(true);

  function submit(e: React.FormEvent) {
    e.preventDefault();
    const input: DecideInput = { action };
    if (action === "order" || action === "pick_up") {
      input.vendor = vendor.trim() || undefined;
      input.tracking = tracking.trim() || undefined;
      input.ship_to = action === "order" ? shipTo : "office";
      input.ship_to_note = shipNote.trim() || undefined;
      const c = toCents(cost);
      if (c != null) input.unit_cost_cents = c;
      input.billable_to_owner = billable;
    }
    onDecide(input);
  }

  return (
    <form
      onSubmit={submit}
      className="mt-2 flex flex-wrap items-center gap-2 rounded-xl border border-line bg-surface-2 p-3"
    >
      <select
        className={field}
        value={action}
        onChange={(e) => setAction(e.target.value as DecideInput["action"])}
      >
        <option value="order">Order it</option>
        <option value="pick_up">Pick it up</option>
        <option value="from_stock" disabled={!canStock}>
          From stock{canStock ? "" : " (not enough)"}
        </option>
        <option value="skip">Skip</option>
      </select>
      {(action === "order" || action === "pick_up") && (
        <>
          <input
            className={`${field} w-36`}
            placeholder="Vendor"
            value={vendor}
            onChange={(e) => setVendor(e.target.value)}
          />
          <input
            className={`${field} w-32`}
            placeholder="Order # / tracking"
            value={tracking}
            onChange={(e) => setTracking(e.target.value)}
          />
          {action === "order" && (
            <select
              className={field}
              value={shipTo}
              onChange={(e) =>
                setShipTo(e.target.value as "property" | "office" | "other")
              }
            >
              <option value="property">Ship to the property</option>
              <option value="office">Ship to the office</option>
              <option value="other">Somewhere else</option>
            </select>
          )}
          {(shipTo === "other" || action === "pick_up") && (
            <input
              className={`${field} w-40`}
              placeholder={action === "pick_up" ? "Which store" : "Where"}
              value={shipNote}
              onChange={(e) => setShipNote(e.target.value)}
            />
          )}
          <input
            className={`${field} w-24`}
            placeholder="$ each"
            value={cost}
            onChange={(e) => setCost(e.target.value)}
          />
          <label className="flex items-center gap-1 text-xs text-ink-3">
            <input
              type="checkbox"
              checked={billable}
              onChange={(e) => setBillable(e.target.checked)}
            />{" "}
            bill the owner
          </label>
        </>
      )}
      <Button type="submit" disabled={busy}>
        Confirm
      </Button>
      <Button variant="ghost" type="button" onClick={onCancel}>
        Cancel
      </Button>
    </form>
  );
}
