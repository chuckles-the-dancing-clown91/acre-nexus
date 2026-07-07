"use client";

// The parts/supplies stockroom: quantity on hand, unit cost, reorder level
// (low-stock alerts ride the helpdesk scan), storage location, and serial
// pools for serialized stock. Ticket lines draw from here.

import { useEffect, useState } from "react";
import { api } from "@/lib/api";
import type { InventoryItem } from "@/lib/types";
import { toast } from "sonner";
import { Badge, Button, Card } from "@/components/ui";

const field =
  "rounded-xl border border-line bg-surface px-3 py-2 text-sm outline-none focus:border-accent";

export function InventoryCard({ manage }: { manage: boolean }) {
  const [items, setItems] = useState<InventoryItem[]>([]);
  const [adding, setAdding] = useState(false);
  const [busy, setBusy] = useState(false);
  const [name, setName] = useState("");
  const [sku, setSku] = useState("");
  const [quantity, setQuantity] = useState("0");
  const [unitCost, setUnitCost] = useState("");
  const [reorder, setReorder] = useState("0");
  const [serials, setSerials] = useState("");

  const load = () => {
    api
      .inventory({ status: "active" })
      .then(setItems)
      .catch(() => setItems([]));
  };
  useEffect(load, []);

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

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    const qty = parseInt(quantity, 10);
    if (!name.trim() || !Number.isFinite(qty) || qty < 0) {
      toast.error("An item needs a name and a non-negative quantity.");
      return;
    }
    const serialList = serials
      .split(",")
      .map((s) => s.trim())
      .filter(Boolean);
    if (serialList.length > 0 && serialList.length !== qty) {
      toast.error("Serialized stock needs one serial per unit.");
      return;
    }
    const cents = unitCost.trim()
      ? Math.round(parseFloat(unitCost) * 100)
      : undefined;
    await run(async () => {
      await api.createInventory({
        name: name.trim(),
        sku: sku.trim() || undefined,
        quantity: qty,
        unit_cost_cents: cents,
        reorder_level: parseInt(reorder, 10) || 0,
        serial_numbers: serialList.length ? serialList : undefined,
      });
      setAdding(false);
      setName("");
      setSku("");
      setQuantity("0");
      setUnitCost("");
      setReorder("0");
      setSerials("");
    }, "Item stocked.");
  }

  async function restock(item: InventoryItem) {
    const answer = window.prompt(
      `New quantity on hand for “${item.name}” (currently ${item.quantity}):`,
      String(item.quantity)
    );
    if (answer == null) return;
    const qty = parseInt(answer, 10);
    if (!Number.isFinite(qty) || qty < 0) {
      toast.error("Quantity must be a non-negative number.");
      return;
    }
    if (item.serial_numbers.length > 0) {
      toast.error(
        "This item is serialized — edit its serial pool instead of the raw count."
      );
      return;
    }
    await run(
      () => api.updateInventory(item.id, { quantity: qty }),
      "Quantity updated."
    );
  }

  return (
    <Card>
      <div className="flex items-center justify-between border-b border-line px-5 py-4">
        <h2 className="font-display text-lg font-bold">Inventory</h2>
        {manage && !adding && (
          <Button
            variant="outline"
            disabled={busy}
            onClick={() => setAdding(true)}
          >
            Stock item
          </Button>
        )}
      </div>
      <div className="space-y-3 p-5 text-sm">
        {adding && (
          <form onSubmit={submit} className="flex flex-wrap items-center gap-2">
            <input
              className={`${field} w-56`}
              placeholder="Name, e.g. “HVAC filter 20x25x1”"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
            <input
              className={`${field} w-28`}
              placeholder="SKU"
              value={sku}
              onChange={(e) => setSku(e.target.value)}
            />
            <label className="flex items-center gap-1 text-xs text-ink-3">
              qty
              <input
                className={`${field} w-16`}
                inputMode="numeric"
                value={quantity}
                onChange={(e) => setQuantity(e.target.value)}
              />
            </label>
            <input
              className={`${field} w-24`}
              placeholder="$/unit"
              inputMode="decimal"
              value={unitCost}
              onChange={(e) => setUnitCost(e.target.value)}
            />
            <label className="flex items-center gap-1 text-xs text-ink-3">
              reorder at
              <input
                className={`${field} w-16`}
                inputMode="numeric"
                value={reorder}
                onChange={(e) => setReorder(e.target.value)}
              />
            </label>
            <input
              className={`${field} w-64`}
              placeholder="Serials (comma-separated, if serialized)"
              value={serials}
              onChange={(e) => setSerials(e.target.value)}
            />
            <Button type="submit" disabled={busy}>
              Save
            </Button>
            <Button
              variant="ghost"
              type="button"
              onClick={() => setAdding(false)}
            >
              Cancel
            </Button>
          </form>
        )}
        {items.length === 0 && !adding && (
          <p className="text-ink-3">
            Nothing stocked yet — add the filters, cartridges, and parts the
            team burns through, with reorder levels so you hear before you run
            out.
          </p>
        )}
        {items.map((i) => (
          <div
            key={i.id}
            className="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-line px-4 py-2.5"
          >
            <div className="min-w-0">
              <span className="font-semibold">{i.name}</span>
              <span className="ml-2 text-xs text-ink-3">
                {i.sku ? `${i.sku} · ` : ""}
                {i.unit_cost_label ? `${i.unit_cost_label}/unit · ` : ""}
                {i.storage_location ?? "no location"}
                {i.serial_numbers.length > 0
                  ? ` · serialized (${i.serial_numbers.length})`
                  : ""}
              </span>
            </div>
            <div className="flex items-center gap-2">
              <Badge tone={i.low_stock ? "bad" : "neutral"}>
                {i.quantity} on hand
                {i.low_stock ? " · low" : ""}
              </Badge>
              {manage && (
                <Button
                  variant="outline"
                  disabled={busy}
                  onClick={() => void restock(i)}
                >
                  Restock
                </Button>
              )}
            </div>
          </div>
        ))}
      </div>
    </Card>
  );
}
