"use client";

// Stock: scan-in with the camera or a scanner gun, receive a delivery (tax and
// shipping spread across the lines, weighted-average unit cost), count what's
// on the shelf, the movement ledger, and the reorder list by vendor.

import { useCallback, useEffect, useRef, useState } from "react";
import Link from "next/link";
import { toast } from "sonner";
import { api } from "@/lib/api";
import type { InventoryItem } from "@/lib/types";
import {
  stock,
  type Movement,
  type ReceiveLine,
  type ReorderGroup,
} from "@/lib/parts";
import { money, toCents } from "@/lib/backoffice";
import { useAuth } from "@/lib/auth";
import { Badge, Button, Card, StatTile } from "@/components/ui";

const field =
  "rounded-xl border border-line bg-surface px-3 py-2 text-sm outline-none focus:border-accent";

interface Line {
  key: number;
  item: InventoryItem | null;
  code: string;
  name: string;
  quantity: string;
  cost: string;
}

let nextKey = 1;
const newLine = (patch: Partial<Line> = {}): Line => ({
  key: nextKey++,
  item: null,
  code: "",
  name: "",
  quantity: "1",
  cost: "",
  ...patch,
});

/** Camera barcode reading through the browser's BarcodeDetector. */
function CameraScanner({ onCode }: { onCode: (code: string) => void }) {
  const videoRef = useRef<HTMLVideoElement | null>(null);

  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;
    let stream: MediaStream | null = null;
    let timer: ReturnType<typeof setInterval> | null = null;
    let last = "";
    (async () => {
      try {
        stream = await navigator.mediaDevices.getUserMedia({
          video: { facingMode: "environment" },
        });
        video.srcObject = stream;
        await video.play();
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const Detector = (window as any).BarcodeDetector;
        const detector = new Detector({
          formats: [
            "ean_13",
            "ean_8",
            "upc_a",
            "upc_e",
            "code_128",
            "code_39",
            "qr_code",
          ],
        });
        timer = setInterval(async () => {
          try {
            const codes = await detector.detect(video);
            const raw = codes[0]?.rawValue as string | undefined;
            if (raw && raw !== last) {
              last = raw;
              onCode(raw);
            }
          } catch {
            // a frame that couldn't be read; try the next one
          }
        }, 400);
      } catch (e) {
        toast.error(e instanceof Error ? e.message : "Camera unavailable");
      }
    })();
    return () => {
      if (timer) clearInterval(timer);
      stream?.getTracks().forEach((t) => t.stop());
    };
  }, [onCode]);

  return (
    <video
      ref={videoRef}
      className="max-h-64 w-full rounded-xl bg-black"
      muted
      playsInline
    />
  );
}

function cameraCanScan(): boolean {
  return typeof window !== "undefined" && "BarcodeDetector" in window;
}

export default function StockPage() {
  const { can } = useAuth();
  const manage = can("maintenance:manage");
  const [items, setItems] = useState<InventoryItem[]>([]);
  const [lines, setLines] = useState<Line[]>([]);
  const [vendor, setVendor] = useState("");
  const [total, setTotal] = useState("");
  const [code, setCode] = useState("");
  const [busy, setBusy] = useState(false);
  const [movements, setMovements] = useState<Movement[]>([]);
  const [reorder, setReorder] = useState<ReorderGroup[]>([]);
  const [counting, setCounting] = useState<string | null>(null);
  const [counted, setCounted] = useState("");
  const codeRef = useRef<HTMLInputElement | null>(null);

  const load = useCallback(() => {
    api
      .inventory({ status: "active" })
      .then(setItems)
      .catch(() => setItems([]));
    stock
      .movements()
      .then(setMovements)
      .catch(() => setMovements([]));
    stock
      .reorder()
      .then(setReorder)
      .catch(() => setReorder([]));
  }, []);
  useEffect(load, [load]);

  const addCode = useCallback(async (raw: string) => {
    const c = raw.trim();
    if (!c) return;
    try {
      const item = await stock.lookup(c);
      setLines((ls) => {
        const hit = ls.find((l) => l.item?.id === item.id);
        if (hit)
          return ls.map((l) =>
            l === hit
              ? { ...l, quantity: String((parseInt(l.quantity, 10) || 0) + 1) }
              : l
          );
        return [
          ...ls,
          newLine({
            item,
            code: c,
            cost:
              item.unit_cost_cents != null
                ? (item.unit_cost_cents / 100).toFixed(2)
                : "",
          }),
        ];
      });
      toast.success(`${item.name}`);
    } catch {
      setLines((ls) => [...ls, newLine({ code: c })]);
      toast.message(`New code ${c} — name it`);
    }
    setCode("");
    codeRef.current?.focus();
  }, []);
  const [camera, setCamera] = useState(false);

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

  const subtotal = lines.reduce(
    (s, l) => s + (toCents(l.cost) ?? 0) * (parseInt(l.quantity, 10) || 0),
    0
  );

  function receive(e: React.FormEvent) {
    e.preventDefault();
    const body: ReceiveLine[] = [];
    for (const l of lines) {
      const qty = parseInt(l.quantity, 10);
      const cost = toCents(l.cost);
      if (!qty || qty <= 0 || cost == null) {
        toast.error("Each line needs a quantity and what you paid each.");
        return;
      }
      if (l.item)
        body.push({
          inventory_item_id: l.item.id,
          quantity: qty,
          unit_cost_cents: cost,
        });
      else if (l.name.trim())
        body.push({
          new_item: { name: l.name.trim(), barcode: l.code || undefined },
          quantity: qty,
          unit_cost_cents: cost,
        });
      else {
        toast.error(`Name the new item for code ${l.code}.`);
        return;
      }
    }
    if (body.length === 0) {
      toast.error("Scan or add something first.");
      return;
    }
    const t = toCents(total);
    void run(async () => {
      const r = await stock.receive({
        lines: body,
        vendor: vendor.trim() || undefined,
        total_cents: t ?? undefined,
      });
      setLines([]);
      setTotal("");
      toast.success(
        `Received ${r.items.length} item${r.items.length === 1 ? "" : "s"} — ${money(r.total_cents)}`
      );
    });
  }

  const value = items.reduce((s, i) => s + i.value_cents, 0);
  const low = items.filter((i) => i.low_stock).length;

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
            Stock
          </h1>
          <p className="text-sm text-ink-3">
            Scan it in, count it, see where it went.
          </p>
        </div>
      </div>
      <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
        <StatTile label="Items" value={`${items.length}`} />
        <StatTile label="On the shelf" value={money(value)} icon="dollar" />
        <StatTile label="Low stock" value={`${low}`} />
        <StatTile label="Movements" value={`${movements.length}`} />
      </div>

      {manage && (
        <Card>
          <div className="flex flex-wrap items-center justify-between gap-2 border-b border-line px-5 py-4">
            <h2 className="font-display text-lg font-bold">
              Receive a delivery
            </h2>
            <Button
              variant="outline"
              onClick={() => {
                if (!camera && !cameraCanScan()) {
                  toast.error(
                    "This browser can't read barcodes from the camera — use a scanner gun or type the code."
                  );
                  return;
                }
                setCamera(!camera);
              }}
            >
              {camera ? "Stop camera" : "Scan with camera"}
            </Button>
          </div>
          <form onSubmit={receive} className="space-y-3 p-5 text-sm">
            {camera && <CameraScanner onCode={addCode} />}
            <div className="flex flex-wrap items-center gap-2">
              <input
                ref={codeRef}
                autoFocus
                className={`${field} w-72 font-mono`}
                placeholder="Scan a barcode or type a SKU, then Enter"
                value={code}
                onChange={(e) => setCode(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    e.preventDefault();
                    void addCode(code);
                  }
                }}
              />
              <select
                className={field}
                value=""
                onChange={(e) => {
                  const it = items.find((i) => i.id === e.target.value);
                  if (it)
                    setLines((ls) => [
                      ...ls,
                      newLine({
                        item: it,
                        cost:
                          it.unit_cost_cents != null
                            ? (it.unit_cost_cents / 100).toFixed(2)
                            : "",
                      }),
                    ]);
                }}
              >
                <option value="">…or pick an item</option>
                {items.map((i) => (
                  <option key={i.id} value={i.id}>
                    {i.name}
                  </option>
                ))}
              </select>
              <Button
                variant="outline"
                type="button"
                onClick={() => setLines((ls) => [...ls, newLine()])}
              >
                + new item
              </Button>
            </div>
            {lines.map((l) => (
              <div
                key={l.key}
                className="flex flex-wrap items-center gap-2 rounded-xl border border-line p-2"
              >
                {l.item ? (
                  <span className="w-64 truncate font-semibold">
                    {l.item.name}
                    <span className="ml-1 text-xs font-normal text-ink-3">
                      {l.item.quantity} on hand
                      {l.item.unit_cost_cents != null
                        ? ` @ ${money(l.item.unit_cost_cents)}`
                        : ""}
                    </span>
                  </span>
                ) : (
                  <input
                    className={`${field} w-64`}
                    placeholder={
                      l.code ? `New item for ${l.code}` : "New item name"
                    }
                    value={l.name}
                    onChange={(e) =>
                      setLines((ls) =>
                        ls.map((x) =>
                          x.key === l.key ? { ...x, name: e.target.value } : x
                        )
                      )
                    }
                  />
                )}
                <input
                  className={`${field} w-20`}
                  type="number"
                  min={1}
                  value={l.quantity}
                  onChange={(e) =>
                    setLines((ls) =>
                      ls.map((x) =>
                        x.key === l.key ? { ...x, quantity: e.target.value } : x
                      )
                    )
                  }
                />
                <input
                  className={`${field} w-24`}
                  placeholder="$ each"
                  value={l.cost}
                  onChange={(e) =>
                    setLines((ls) =>
                      ls.map((x) =>
                        x.key === l.key ? { ...x, cost: e.target.value } : x
                      )
                    )
                  }
                />
                <Button
                  variant="ghost"
                  type="button"
                  onClick={() =>
                    setLines((ls) => ls.filter((x) => x.key !== l.key))
                  }
                >
                  ×
                </Button>
              </div>
            ))}
            {lines.length > 0 && (
              <div className="flex flex-wrap items-center gap-2">
                <input
                  className={`${field} w-40`}
                  placeholder="Vendor"
                  value={vendor}
                  onChange={(e) => setVendor(e.target.value)}
                />
                <input
                  className={`${field} w-36`}
                  placeholder={`Receipt total (${money(subtotal)})`}
                  value={total}
                  onChange={(e) => setTotal(e.target.value)}
                />
                <span className="text-xs text-ink-3">
                  Tax and shipping above the line total spread across the items.
                </span>
                <Button type="submit" disabled={busy}>
                  Receive
                </Button>
              </div>
            )}
          </form>
        </Card>
      )}

      <Card>
        <div className="border-b border-line px-5 py-4 font-display text-lg font-bold">
          On the shelf
        </div>
        <div className="divide-y divide-line p-5 text-sm">
          {items.map((i) => (
            <div
              key={i.id}
              className="flex flex-wrap items-center justify-between gap-2 py-2"
            >
              <div className="min-w-0">
                <span className="font-semibold">{i.name}</span>
                <span className="ml-2 text-xs text-ink-3">
                  {i.barcode ?? i.sku ?? ""}
                  {i.vendor ? ` · ${i.vendor}` : ""}
                  {i.storage_location ? ` · ${i.storage_location}` : ""}
                  {i.unit_cost_cents != null
                    ? ` · ${money(i.unit_cost_cents)}/${i.unit}`
                    : ""}
                </span>
              </div>
              <div className="flex items-center gap-2">
                <span className="font-mono">
                  {i.quantity} {i.unit}
                </span>
                {i.low_stock && <Badge tone="warn">reorder</Badge>}
                {manage && (
                  <Button
                    variant="ghost"
                    onClick={() => {
                      setCounting(counting === i.id ? null : i.id);
                      setCounted(String(i.quantity));
                    }}
                  >
                    Count
                  </Button>
                )}
                {counting === i.id && (
                  <form
                    className="flex items-center gap-1"
                    onSubmit={(e) => {
                      e.preventDefault();
                      const n = parseInt(counted, 10);
                      if (!Number.isFinite(n) || n < 0) return;
                      void run(async () => {
                        await stock.count(i.id, n);
                        setCounting(null);
                      }, "Counted.");
                    }}
                  >
                    <input
                      className={`${field} w-20`}
                      type="number"
                      min={0}
                      value={counted}
                      onChange={(e) => setCounted(e.target.value)}
                    />
                    <Button type="submit" disabled={busy}>
                      Save
                    </Button>
                  </form>
                )}
              </div>
            </div>
          ))}
          {items.length === 0 && (
            <p className="text-ink-3">
              Nothing in stock yet — receive a delivery above.
            </p>
          )}
        </div>
      </Card>

      <div className="grid gap-6 lg:grid-cols-2">
        <Card>
          <div className="border-b border-line px-5 py-4 font-display text-lg font-bold">
            Reorder list
          </div>
          <div className="space-y-3 p-5 text-sm">
            {reorder.length === 0 && (
              <p className="text-ink-3">
                Everything is above its reorder level.
              </p>
            )}
            {reorder.map((g) => (
              <div key={g.vendor}>
                <div className="text-xs font-semibold uppercase tracking-wide text-ink-3">
                  {g.vendor}
                </div>
                {g.items.map((i) => (
                  <div key={i.id} className="flex justify-between">
                    <span>{i.name}</span>
                    <span className="font-mono">
                      {i.quantity} / {i.reorder_level}
                    </span>
                  </div>
                ))}
              </div>
            ))}
          </div>
        </Card>
        <Card>
          <div className="border-b border-line px-5 py-4 font-display text-lg font-bold">
            Movements
          </div>
          <div className="divide-y divide-line p-5 text-sm">
            {movements.length === 0 && (
              <p className="text-ink-3">No movements yet.</p>
            )}
            {movements.slice(0, 40).map((m) => (
              <div
                key={m.id}
                className="flex flex-wrap items-center justify-between gap-2 py-1.5"
              >
                <div className="min-w-0">
                  <span className="font-semibold">{m.item_name}</span>
                  <span className="ml-2 text-xs text-ink-3">
                    {m.kind}
                    {m.ticket_title ? ` · ${m.ticket_title}` : ""}
                    {m.note ? ` · ${m.note}` : ""}
                    {m.recorded_by ? ` · ${m.recorded_by}` : ""} ·{" "}
                    {m.created_at.slice(0, 10)}
                  </span>
                </div>
                <span
                  className={`font-mono ${m.quantity < 0 ? "text-bad" : "text-good"}`}
                >
                  {m.quantity > 0 ? "+" : ""}
                  {m.quantity} · {money(m.value_cents)}
                </span>
              </div>
            ))}
          </div>
        </Card>
      </div>
    </div>
  );
}
