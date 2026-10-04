"use client";

// Utility meters on a property or one unit: who pays for each, the last
// reading, and a way to log the next. What's set here is what the lease's
// utility agreement says.

import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Gauge, History as HistoryIcon, Pencil, Plus } from "lucide-react";
import { toast } from "sonner";
import {
  meters,
  METER_KINDS,
  PAID_BY,
  type Meter,
  type MeterKind,
  type PaidBy,
} from "@/lib/meters";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { F, FormDialog, input, why } from "./bits";

const kindName = (k: string) => METER_KINDS.find(([x]) => x === k)?.[1] ?? k;

type Draft = {
  kind: MeterKind;
  label: string;
  meter_number: string;
  location: string;
  provider: string;
  unit_of_measure: string;
  paid_by: PaidBy;
  billing_note: string;
};

const blank: Draft = {
  kind: "electric",
  label: "",
  meter_number: "",
  location: "",
  provider: "",
  unit_of_measure: "",
  paid_by: "tenant",
  billing_note: "",
};

const today = () => new Date().toISOString().slice(0, 10);

/** `unitId`: a unit's own meters. `null`: the building's. Left out: all. */
export function Meters({
  propertyId,
  unitId,
  manage,
  title = "Meters and utilities",
  description = "Who pays for each, and the last reading.",
}: {
  propertyId: string;
  unitId?: string | null;
  manage: boolean;
  title?: string;
  description?: string;
}) {
  const qc = useQueryClient();
  const key = ["meters", propertyId, unitId ?? "all"];
  const q = useQuery({
    queryKey: key,
    queryFn: () =>
      meters.list({
        property_id: propertyId,
        ...(unitId ? { unit_id: unitId } : {}),
      }),
  });
  const [editing, setEditing] = useState<{
    id: string | null;
    draft: Draft;
  } | null>(null);
  const [reading, setReading] = useState<{
    m: Meter;
    value: string;
    on: string;
    note: string;
  } | null>(null);
  const [history, setHistory] = useState<Meter | null>(null);
  const [busy, setBusy] = useState(false);

  const rows = (q.data ?? [])
    .filter((m) => m.status === "active")
    .filter((m) => unitId === undefined || (m.unit_id ?? null) === unitId);

  const refresh = () => {
    void qc.invalidateQueries({ queryKey: ["meters", propertyId] });
    void qc.invalidateQueries({
      queryKey: ["properties", propertyId, "units"],
    });
  };

  async function save() {
    if (!editing) return;
    const d = editing.draft;
    setBusy(true);
    try {
      if (editing.id)
        await meters.update(editing.id, {
          label: d.label || undefined,
          meter_number: d.meter_number,
          location: d.location,
          provider: d.provider,
          unit_of_measure: d.unit_of_measure,
          paid_by: d.paid_by,
          billing_note: d.billing_note,
        });
      else
        await meters.create({
          property_id: propertyId,
          ...(unitId ? { unit_id: unitId } : {}),
          kind: d.kind,
          label: d.label || undefined,
          meter_number: d.meter_number || undefined,
          location: d.location || undefined,
          provider: d.provider || undefined,
          unit_of_measure: d.unit_of_measure || undefined,
          paid_by: d.paid_by,
          billing_note: d.billing_note || undefined,
        });
      toast.success("Saved");
      setEditing(null);
      refresh();
    } catch (e) {
      toast.error(why(e));
    } finally {
      setBusy(false);
    }
  }

  async function retire() {
    if (!editing?.id) return;
    setBusy(true);
    try {
      await meters.update(editing.id, { status: "retired" });
      toast.success("Retired");
      setEditing(null);
      refresh();
    } catch (e) {
      toast.error(why(e));
    } finally {
      setBusy(false);
    }
  }

  async function log() {
    if (!reading) return;
    const n = Number(reading.value);
    if (reading.value.trim() === "" || !Number.isFinite(n))
      return void toast.error("Enter the number on the meter");
    setBusy(true);
    try {
      await meters.addReading(reading.m.id, {
        reading: n,
        read_on: reading.on,
        note: reading.note || undefined,
      });
      toast.success("Reading saved");
      setReading(null);
      refresh();
    } catch (e) {
      toast.error(why(e));
    } finally {
      setBusy(false);
    }
  }

  const set = (patch: Partial<Draft>) =>
    editing &&
    setEditing({ ...editing, draft: { ...editing.draft, ...patch } });

  return (
    <Panel>
      <PanelHeader
        title={title}
        description={description}
        action={
          manage && (
            <Button
              size="sm"
              variant="secondary"
              onClick={() => setEditing({ id: null, draft: blank })}
            >
              <Plus />
              Add meter
            </Button>
          )
        }
      />
      <div className="p-5 pt-3">
        {q.isLoading && <Skeleton className="h-24" />}
        {q.isSuccess && rows.length === 0 && (
          <EmptyState
            icon={<Gauge />}
            title="No meters yet"
            description="Add the electric, gas and water meters and say who pays, so the lease can say it too."
            className="py-6"
          />
        )}
        <ul className="divide-y divide-line rounded-xl border border-line empty:hidden">
          {rows.map((m) => (
            <li
              key={m.id}
              className="flex flex-col gap-2 px-3 py-2.5 sm:flex-row sm:items-center"
            >
              <div className="min-w-0 flex-1">
                <div className="text-[13px] font-medium text-fg">
                  {m.label}
                  {m.meter_number && (
                    <span className="font-normal text-fg-3">
                      {" "}
                      · #{m.meter_number}
                    </span>
                  )}
                </div>
                <div className="text-xs text-fg-3">
                  {[kindName(m.kind), m.provider, m.location]
                    .filter(Boolean)
                    .join(" · ")}
                  {m.last_reading != null &&
                    ` · last ${m.last_reading.toLocaleString()}${m.unit_of_measure ? ` ${m.unit_of_measure}` : ""} on ${m.last_read_on}`}
                </div>
              </div>
              <div className="flex shrink-0 flex-wrap items-center gap-1.5">
                <Badge
                  tone={
                    m.paid_by === "tenant"
                      ? "info"
                      : m.paid_by === "landlord"
                        ? "neutral"
                        : "warn"
                  }
                >
                  {PAID_BY.find(([k]) => k === m.paid_by)?.[1]}
                </Badge>
                <Button size="sm" variant="ghost" onClick={() => setHistory(m)}>
                  <HistoryIcon />
                  Readings
                </Button>
                {manage && (
                  <>
                    <Button
                      size="sm"
                      variant="secondary"
                      onClick={() =>
                        setReading({ m, value: "", on: today(), note: "" })
                      }
                    >
                      Log reading
                    </Button>
                    <Button
                      size="icon"
                      variant="ghost"
                      aria-label={`Edit ${m.label}`}
                      onClick={() =>
                        setEditing({
                          id: m.id,
                          draft: {
                            kind: m.kind,
                            label: m.label,
                            meter_number: m.meter_number ?? "",
                            location: m.location ?? "",
                            provider: m.provider ?? "",
                            unit_of_measure: m.unit_of_measure ?? "",
                            paid_by: m.paid_by,
                            billing_note: m.billing_note ?? "",
                          },
                        })
                      }
                    >
                      <Pencil />
                    </Button>
                  </>
                )}
              </div>
            </li>
          ))}
        </ul>
      </div>

      <FormDialog
        open={!!editing}
        onOpenChange={(o) => !o && setEditing(null)}
        title={editing?.id ? "Edit meter" : "Add meter"}
        busy={busy}
        onSave={() => void save()}
        onDelete={editing?.id ? () => void retire() : undefined}
      >
        {editing && (
          <div className="grid gap-3 sm:grid-cols-2">
            <F label="Utility">
              <select
                className={input}
                disabled={!!editing.id}
                value={editing.draft.kind}
                onChange={(e) => set({ kind: e.target.value as MeterKind })}
              >
                {METER_KINDS.map(([k, l]) => (
                  <option key={k} value={k}>
                    {l}
                  </option>
                ))}
              </select>
            </F>
            <F label="Who pays">
              <select
                className={input}
                value={editing.draft.paid_by}
                onChange={(e) => set({ paid_by: e.target.value as PaidBy })}
              >
                {PAID_BY.map(([k, l]) => (
                  <option key={k} value={k}>
                    {l}
                  </option>
                ))}
              </select>
            </F>
            <F label="Name (optional)">
              <input
                className={input}
                value={editing.draft.label}
                onChange={(e) => set({ label: e.target.value })}
              />
            </F>
            <F label="Meter number">
              <input
                className={input}
                value={editing.draft.meter_number}
                onChange={(e) => set({ meter_number: e.target.value })}
              />
            </F>
            <F label="Provider">
              <input
                className={input}
                value={editing.draft.provider}
                onChange={(e) => set({ provider: e.target.value })}
              />
            </F>
            <F label="Counts in">
              <input
                className={input}
                placeholder="kWh, therms, gallons"
                value={editing.draft.unit_of_measure}
                onChange={(e) => set({ unit_of_measure: e.target.value })}
              />
            </F>
            <F label="Where it is" className="sm:col-span-2">
              <input
                className={input}
                value={editing.draft.location}
                onChange={(e) => set({ location: e.target.value })}
              />
            </F>
            <F
              label="Billing note (goes in the lease)"
              className="sm:col-span-2"
            >
              <textarea
                rows={2}
                className={input}
                placeholder="Water is split by square footage and billed monthly."
                value={editing.draft.billing_note}
                onChange={(e) => set({ billing_note: e.target.value })}
              />
            </F>
          </div>
        )}
      </FormDialog>

      <FormDialog
        open={!!reading}
        onOpenChange={(o) => !o && setReading(null)}
        title={reading ? `Log a reading: ${reading.m.label}` : "Log a reading"}
        description={
          reading?.m.last_reading != null
            ? `Last was ${reading.m.last_reading.toLocaleString()} on ${reading.m.last_read_on}.`
            : undefined
        }
        busy={busy}
        onSave={() => void log()}
      >
        {reading && (
          <div className="grid gap-3 sm:grid-cols-2">
            <F label="Reading">
              <input
                className={input}
                inputMode="decimal"
                value={reading.value}
                onChange={(e) =>
                  setReading({ ...reading, value: e.target.value })
                }
              />
            </F>
            <F label="Read on">
              <input
                type="date"
                className={input}
                value={reading.on}
                onChange={(e) => setReading({ ...reading, on: e.target.value })}
              />
            </F>
            <F label="Note" className="sm:col-span-2">
              <input
                className={input}
                value={reading.note}
                onChange={(e) =>
                  setReading({ ...reading, note: e.target.value })
                }
              />
            </F>
          </div>
        )}
      </FormDialog>

      {history && <History meter={history} onClose={() => setHistory(null)} />}
    </Panel>
  );
}

function History({ meter, onClose }: { meter: Meter; onClose: () => void }) {
  const q = useQuery({
    queryKey: ["meter-readings", meter.id],
    queryFn: () => meters.readings(meter.id),
  });
  return (
    <FormDialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={`Readings: ${meter.label}`}
      onSave={onClose}
    >
      {q.isLoading && <Skeleton className="h-24" />}
      {q.data?.length === 0 && (
        <p className="text-[13px] text-fg-3">No readings yet.</p>
      )}
      <ul className="divide-y divide-line text-[13px]">
        {q.data?.map((r) => (
          <li key={r.id} className="flex items-center gap-3 py-2">
            <span className="w-24 text-fg-3">{r.read_on}</span>
            <span className="figure flex-1 text-fg">
              {r.reading.toLocaleString()}
              {meter.unit_of_measure ? ` ${meter.unit_of_measure}` : ""}
            </span>
            {r.used != null && (
              <span className="text-fg-3">+{r.used.toLocaleString()}</span>
            )}
            {r.reason !== "routine" && (
              <Badge>{r.reason.replace("_", " ")}</Badge>
            )}
          </li>
        ))}
      </ul>
    </FormDialog>
  );
}
