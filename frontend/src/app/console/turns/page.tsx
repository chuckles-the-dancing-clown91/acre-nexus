"use client";

// Turnovers: every unit being turned, with progress, days vacant and cost.

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { toast } from "sonner";
import { api } from "@/lib/api";
import type { Property, Unit } from "@/lib/types";
import { turns, type Turn } from "@/lib/turns";
import { useAuth } from "@/lib/auth";
import { Badge, Button, Card, StatTile } from "@/components/ui";

const field =
  "rounded-xl border border-line bg-surface px-3 py-2 text-sm text-ink";

export default function TurnsPage() {
  const { can } = useAuth();
  const manage = can("maintenance:manage");
  const [rows, setRows] = useState<Turn[]>([]);
  const [status, setStatus] = useState("active");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const load = useCallback(() => {
    setLoading(true);
    turns
      .list({ status })
      .then((r) => {
        setRows(r);
        setError(null);
      })
      .catch((e: Error) => setError(e.message))
      .finally(() => setLoading(false));
  }, [status]);

  useEffect(load, [load]);

  const active = rows.filter((r) => r.status === "active");
  const avgDays = active.length
    ? Math.round(active.reduce((n, r) => n + r.days_open, 0) / active.length)
    : 0;

  return (
    <div className="space-y-5">
      <div className="flex flex-wrap items-center gap-3">
        <div>
          <h1 className="font-display text-2xl font-bold">Turnovers</h1>
          <p className="text-sm text-ink-3">
            Every step from move-out to move-in ready, in order.
          </p>
        </div>
        <div className="ml-auto flex items-center gap-2">
          {can("maintenance:manage") && (
            <Link href="/console/turns/templates">
              <Button variant="outline">Edit steps</Button>
            </Link>
          )}
        </div>
      </div>

      <div className="grid gap-3 sm:grid-cols-3">
        <StatTile label="In progress" value={String(active.length)} />
        <StatTile
          label="Past target"
          value={String(active.filter((r) => r.overdue).length)}
        />
        <StatTile label="Avg days vacant" value={String(avgDays)} />
      </div>

      {manage && <StartTurn onStarted={load} />}

      <div className="flex gap-2">
        {["active", "done", "cancelled"].map((s) => (
          <button
            key={s}
            onClick={() => setStatus(s)}
            className={`rounded-full px-3 py-1 text-xs font-bold ${
              status === s
                ? "bg-accent-soft text-accent-2"
                : "bg-surface-2 text-ink-2"
            }`}
          >
            {s}
          </button>
        ))}
      </div>

      {error && <p className="text-sm text-bad">{error}</p>}
      <Card className="divide-y divide-line">
        {rows.map((t) => (
          <Link
            key={t.id}
            href={`/console/turns/${t.id}`}
            className="flex flex-wrap items-center gap-4 px-5 py-4 hover:bg-surface-2"
          >
            <div className="min-w-0 flex-1">
              <div className="font-semibold">
                {t.property_name} · Unit {t.unit_number ?? "—"}
              </div>
              <div className="text-xs text-ink-3">
                Started {t.started_on}
                {t.target_date ? ` · target ${t.target_date}` : ""}
              </div>
            </div>
            <div className="w-40">
              <div className="h-2 overflow-hidden rounded-full bg-surface-2">
                <div
                  className="h-full bg-accent"
                  style={{
                    width: `${t.total ? (t.done / t.total) * 100 : 0}%`,
                  }}
                />
              </div>
              <div className="mt-1 text-xs text-ink-3">
                {t.done} of {t.total} steps
              </div>
            </div>
            <div className="w-24 text-right text-sm">
              {t.days_open} {t.days_open === 1 ? "day" : "days"}
            </div>
            <div className="w-24 text-right text-sm">{t.cost_label}</div>
            {t.overdue ? (
              <Badge tone="bad">Past target</Badge>
            ) : (
              <Badge tone={t.status === "done" ? "good" : "info"}>
                {t.status}
              </Badge>
            )}
          </Link>
        ))}
        {!loading && rows.length === 0 && (
          <p className="px-5 py-8 text-center text-sm text-ink-3">
            No {status} turnovers. A completed move-out inspection starts one on
            its own.
          </p>
        )}
      </Card>
    </div>
  );
}

/** Start a turn by hand: pick the property and the unit. */
function StartTurn({ onStarted }: { onStarted: () => void }) {
  const [properties, setProperties] = useState<Property[]>([]);
  const [units, setUnits] = useState<Unit[]>([]);
  const [propertyId, setPropertyId] = useState("");
  const [unitId, setUnitId] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    api
      .properties()
      .then(setProperties)
      .catch(() => {});
  }, []);
  useEffect(() => {
    if (!propertyId) return;
    api
      .units(propertyId)
      .then(setUnits)
      .catch(() => setUnits([]));
  }, [propertyId]);

  const go = async () => {
    setBusy(true);
    try {
      await turns.start(unitId, {});
      toast.success("Turnover started");
      setUnitId("");
      onStarted();
    } catch (e) {
      toast.error((e as Error).message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Card className="flex flex-wrap items-center gap-3 p-4">
      <span className="text-sm font-semibold">Start a turn</span>
      <select
        className={field}
        value={propertyId}
        onChange={(e) => {
          setPropertyId(e.target.value);
          setUnitId("");
          setUnits([]);
        }}
        aria-label="Property"
      >
        <option value="">Property…</option>
        {properties.map((p) => (
          <option key={p.id} value={p.id}>
            {p.name}
          </option>
        ))}
      </select>
      <select
        className={field}
        value={unitId}
        onChange={(e) => setUnitId(e.target.value)}
        aria-label="Unit"
        disabled={!propertyId}
      >
        <option value="">Unit…</option>
        {units.map((u) => (
          <option key={u.id} value={u.id}>
            {u.unit_number}
          </option>
        ))}
      </select>
      <Button onClick={go} disabled={!unitId || busy}>
        Start
      </Button>
    </Card>
  );
}
