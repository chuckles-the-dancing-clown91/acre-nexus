"use client";

// One appliance: what it is, what it cost and how long it should last, the
// warranty, the parts that fit (from stock), routine maintenance on it,
// every work order and what was spent, and the manuals and warranty papers.
// "Repair" / "Replace" / "Service" open a work order with its parts pre-listed.

import { useCallback, useEffect, useState } from "react";
import { useParams, useRouter } from "next/navigation";
import Link from "next/link";
import { toast } from "sonner";
import { api } from "@/lib/api";
import type { InventoryItem } from "@/lib/types";
import { appliances, type AssetHistory } from "@/lib/parts";
import { money } from "@/lib/backoffice";
import { useAuth } from "@/lib/auth";
import { Badge, Button, Card, StatTile } from "@/components/ui";
import { DocumentsCard } from "@/components/DocumentsCard";

const field =
  "rounded-xl border border-line bg-surface px-3 py-2 text-sm outline-none focus:border-accent";

function humanize(key: string): string {
  const s = key.replace(/_/g, " ");
  return s.charAt(0).toUpperCase() + s.slice(1);
}

export default function AssetPage() {
  const { id } = useParams<{ id: string }>();
  const router = useRouter();
  const { can } = useAuth();
  const manage = can("maintenance:manage");
  const [a, setA] = useState<AssetHistory | null>(null);
  const [items, setItems] = useState<InventoryItem[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [partItem, setPartItem] = useState("");
  const [partQty, setPartQty] = useState("1");
  const [partRole, setPartRole] = useState("");
  const [editing, setEditing] = useState(false);
  const [form, setForm] = useState({
    location: "",
    purchased_on: "",
    purchase_price: "",
    expected_life_years: "",
    warranty_expires: "",
    warranty_provider: "",
    warranty_notes: "",
  });

  const load = useCallback(() => {
    appliances
      .history(id)
      .then((h) => {
        setA(h);
        setForm({
          location: h.location ?? "",
          purchased_on: h.purchased_on ?? "",
          purchase_price:
            h.purchase_price_cents != null
              ? (h.purchase_price_cents / 100).toFixed(2)
              : "",
          expected_life_years:
            h.expected_life_years != null ? String(h.expected_life_years) : "",
          warranty_expires: h.warranty_expires ?? "",
          warranty_provider: h.warranty_provider ?? "",
          warranty_notes: h.warranty_notes ?? "",
        });
      })
      .catch((e: Error) => setError(e.message));
  }, [id]);
  useEffect(() => {
    load();
    api
      .inventory({ status: "active" })
      .then(setItems)
      .catch(() => setItems([]));
  }, [load]);

  async function run(fn: () => Promise<unknown>, ok?: string) {
    setBusy(true);
    try {
      await fn();
      if (ok) toast.success(ok);
      load();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Request failed");
    } finally {
      setBusy(false);
    }
  }

  if (error) return <p className="text-bad">{error}</p>;
  if (!a) return <p className="text-ink-3">Loading…</p>;

  const openWork = (kind: "repair" | "replace" | "service") =>
    void run(async () => {
      const r = await appliances.workOrder(a.id, { kind });
      toast.success(
        `${r.title} — ${r.potential_parts} part${r.potential_parts === 1 ? "" : "s"} pre-listed`
      );
      router.push(`/console/maintenance/${r.ticket_id}`);
    });

  return (
    <div className="space-y-6">
      <Link
        href={`/console/properties/${a.property_id}`}
        className="text-sm font-semibold text-ink-2 hover:underline"
      >
        ← {a.property}
      </Link>
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex flex-wrap items-center gap-3">
          <h1 className="font-display text-3xl font-extrabold tracking-tight">
            {a.name}
          </h1>
          <Badge tone="info">{humanize(a.kind)}</Badge>
          <Badge
            tone={
              a.warranty_state === "active"
                ? "good"
                : a.warranty_state === "expired"
                  ? "bad"
                  : "neutral"
            }
          >
            {a.warranty_state === "none"
              ? "no warranty"
              : `warranty ${a.warranty_state}`}
          </Badge>
          {a.status === "retired" && <Badge>retired</Badge>}
        </div>
        {manage && (
          <div className="flex gap-2">
            <Button
              variant="outline"
              disabled={busy}
              onClick={() => openWork("service")}
            >
              Service
            </Button>
            <Button
              variant="outline"
              disabled={busy}
              onClick={() => openWork("repair")}
            >
              Repair
            </Button>
            <Button disabled={busy} onClick={() => openWork("replace")}>
              Replace
            </Button>
          </div>
        )}
      </div>

      <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
        <StatTile label="Spent on it" value={a.spend_label} icon="dollar" />
        <StatTile label="Work orders" value={`${a.tickets.length}`} />
        <StatTile label="Last serviced" value={a.last_serviced ?? "—"} />
        <StatTile
          label="Life left"
          value={a.years_left != null ? `${a.years_left} yr` : "—"}
        />
      </div>

      <Card className="p-5">
        <div className="mb-3 flex items-center justify-between">
          <h2 className="font-display text-lg font-bold">Details & warranty</h2>
          {manage && (
            <Button variant="outline" onClick={() => setEditing(!editing)}>
              {editing ? "Cancel" : "Edit"}
            </Button>
          )}
        </div>
        {editing ? (
          <form
            className="grid gap-2 sm:grid-cols-2 lg:grid-cols-4"
            onSubmit={(e) => {
              e.preventDefault();
              const price = form.purchase_price.trim()
                ? Math.round(parseFloat(form.purchase_price) * 100)
                : undefined;
              void run(async () => {
                await api.updateAsset(a.id, {
                  location: form.location,
                  purchased_on: form.purchased_on,
                  purchase_price_cents: Number.isFinite(price)
                    ? price
                    : undefined,
                  expected_life_years: form.expected_life_years
                    ? parseInt(form.expected_life_years, 10)
                    : undefined,
                  warranty_expires: form.warranty_expires,
                  warranty_provider: form.warranty_provider,
                  warranty_notes: form.warranty_notes,
                });
                setEditing(false);
              }, "Saved.");
            }}
          >
            <input
              className={field}
              placeholder="Where (Laundry, Garage…)"
              value={form.location}
              onChange={(e) => setForm({ ...form, location: e.target.value })}
            />
            <label className="flex items-center gap-1 text-xs text-ink-3">
              bought
              <input
                type="date"
                className={field}
                value={form.purchased_on}
                onChange={(e) =>
                  setForm({ ...form, purchased_on: e.target.value })
                }
              />
            </label>
            <input
              className={field}
              placeholder="Price $"
              value={form.purchase_price}
              onChange={(e) =>
                setForm({ ...form, purchase_price: e.target.value })
              }
            />
            <input
              className={field}
              placeholder="Expected life (years)"
              value={form.expected_life_years}
              onChange={(e) =>
                setForm({ ...form, expected_life_years: e.target.value })
              }
            />
            <label className="flex items-center gap-1 text-xs text-ink-3">
              warranty until
              <input
                type="date"
                className={field}
                value={form.warranty_expires}
                onChange={(e) =>
                  setForm({ ...form, warranty_expires: e.target.value })
                }
              />
            </label>
            <input
              className={field}
              placeholder="Warranty provider"
              value={form.warranty_provider}
              onChange={(e) =>
                setForm({ ...form, warranty_provider: e.target.value })
              }
            />
            <input
              className={`${field} sm:col-span-2`}
              placeholder="Warranty notes (claim #, phone, terms)"
              value={form.warranty_notes}
              onChange={(e) =>
                setForm({ ...form, warranty_notes: e.target.value })
              }
            />
            <div className="sm:col-span-2 lg:col-span-4">
              <Button type="submit" disabled={busy}>
                Save
              </Button>
            </div>
          </form>
        ) : (
          <dl className="grid grid-cols-2 gap-3 text-sm sm:grid-cols-4">
            {[
              [
                "Make / model",
                [a.make, a.model].filter(Boolean).join(" ") || "—",
              ],
              ["Serial", a.serial_number ?? "—"],
              ["Where", a.location ?? "—"],
              ["Installed", a.install_date ?? "—"],
              ["Bought", a.purchased_on ?? "—"],
              [
                "Price",
                a.purchase_price_cents != null
                  ? money(a.purchase_price_cents)
                  : "—",
              ],
              [
                "Expected life",
                a.expected_life_years != null
                  ? `${a.expected_life_years} years`
                  : "—",
              ],
              ["Warranty until", a.warranty_expires ?? "—"],
              ["Warranty by", a.warranty_provider ?? "—"],
              ["Warranty notes", a.warranty_notes ?? "—"],
              ["Notes", a.notes ?? "—"],
            ].map(([k, v]) => (
              <div key={k}>
                <dt className="text-xs uppercase tracking-wide text-ink-3">
                  {k}
                </dt>
                <dd>{v}</dd>
              </div>
            ))}
          </dl>
        )}
      </Card>

      <Card>
        <div className="border-b border-line px-5 py-4 font-display text-lg font-bold">
          Parts that fit
        </div>
        <div className="space-y-3 p-5 text-sm">
          {a.parts.length === 0 && (
            <p className="text-ink-3">
              No parts listed. Add the filters, belts and elements that fit so
              every work order pre-lists them.
            </p>
          )}
          <div className="divide-y divide-line">
            {a.parts.map((p) => (
              <div
                key={p.id}
                className="flex flex-wrap items-center justify-between gap-2 py-2"
              >
                <div>
                  <span className="font-semibold">
                    {p.quantity} × {p.name}
                  </span>
                  <span className="ml-2 text-xs text-ink-3">
                    {p.role ?? ""}
                    {p.sku ? ` · ${p.sku}` : ""}
                    {p.unit_cost_cents != null
                      ? ` · ${money(p.unit_cost_cents)}`
                      : ""}{" "}
                    · {p.in_stock} on the shelf
                  </span>
                </div>
                {manage && (
                  <Button
                    variant="ghost"
                    disabled={busy}
                    onClick={() =>
                      void run(() => appliances.removePart(a.id, p.id))
                    }
                  >
                    Remove
                  </Button>
                )}
              </div>
            ))}
          </div>
          {manage && (
            <form
              className="flex flex-wrap items-center gap-2"
              onSubmit={(e) => {
                e.preventDefault();
                if (!partItem) return;
                void run(async () => {
                  await appliances.putPart(a.id, {
                    inventory_item_id: partItem,
                    quantity: parseInt(partQty, 10) || 1,
                    role: partRole.trim() || undefined,
                  });
                  setPartItem("");
                  setPartRole("");
                  setPartQty("1");
                }, "Part added.");
              }}
            >
              <select
                className={field}
                value={partItem}
                onChange={(e) => setPartItem(e.target.value)}
              >
                <option value="">Stock item…</option>
                {items.map((i) => (
                  <option key={i.id} value={i.id}>
                    {i.name}
                  </option>
                ))}
              </select>
              <input
                className={`${field} w-16`}
                type="number"
                min={1}
                value={partQty}
                onChange={(e) => setPartQty(e.target.value)}
              />
              <input
                className={`${field} w-40`}
                placeholder="Role (drive belt, filter…)"
                value={partRole}
                onChange={(e) => setPartRole(e.target.value)}
              />
              <Button type="submit" disabled={busy || !partItem}>
                Add
              </Button>
            </form>
          )}
        </div>
      </Card>

      <div className="grid gap-6 lg:grid-cols-2">
        <Card>
          <div className="border-b border-line px-5 py-4 font-display text-lg font-bold">
            Routine maintenance
          </div>
          <div className="divide-y divide-line p-5 text-sm">
            {a.plans.length === 0 && (
              <p className="text-ink-3">
                No routine on this appliance yet — set one up under Maintenance
                → Preventive plans and pick this appliance.
              </p>
            )}
            {a.plans.map((p) => (
              <div key={p.id} className="flex justify-between py-2">
                <span className="font-semibold">{p.title}</span>
                <span className="text-ink-3">
                  every {p.cadence_days} days · next {p.next_due_date}
                  {!p.active ? " · paused" : ""}
                </span>
              </div>
            ))}
          </div>
        </Card>
        <Card>
          <div className="border-b border-line px-5 py-4 font-display text-lg font-bold">
            Service history
          </div>
          <div className="divide-y divide-line p-5 text-sm">
            {a.tickets.length === 0 && (
              <p className="text-ink-3">No work orders yet.</p>
            )}
            {a.tickets.map((t) => (
              <div
                key={t.id}
                className="flex flex-wrap items-center justify-between gap-2 py-2"
              >
                <Link
                  href={`/console/maintenance/${t.id}`}
                  className="font-semibold hover:underline"
                >
                  {t.title}
                </Link>
                <span className="text-ink-3">
                  {t.created_at.slice(0, 10)} · {humanize(t.status)}
                  {t.cost_label ? ` · ${t.cost_label}` : ""}
                </span>
              </div>
            ))}
          </div>
        </Card>
      </div>

      <DocumentsCard
        ownerType="asset"
        ownerId={a.id}
        title="Warranty, manuals & receipts"
      />
    </div>
  );
}
