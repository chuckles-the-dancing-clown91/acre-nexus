"use client";

// The equipment in a property: appliances and systems, each with its age,
// warranty, and a one-press work order to replace or service it.

import { useState } from "react";
import Link from "next/link";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Pencil, Plus, Refrigerator, Wrench } from "lucide-react";
import { toast } from "sonner";
import { api } from "@/lib/api";
import { day, label, replacementKit } from "@/lib/propertyRecords";
import type { Asset } from "@/lib/types";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { F, FormDialog, input, why } from "./bits";

const ORDER = [
  "hvac",
  "plumbing",
  "appliance",
  "electrical",
  "safety",
  "structural",
  "other",
];

function lifeTone(a: Asset): "bad" | "warn" | "good" | "neutral" {
  if (a.years_left == null) return "neutral";
  if (a.years_left <= 0) return "bad";
  if (a.years_left <= 2) return "warn";
  return "good";
}

const KINDS: [string, string][] = [
  ["appliance", "Appliance"],
  ["hvac", "Heating and cooling"],
  ["plumbing", "Plumbing"],
  ["electrical", "Electrical"],
  ["safety", "Safety"],
  ["structural", "Structure"],
  ["other", "Other"],
];

type Draft = {
  kind: string;
  name: string;
  make: string;
  model: string;
  serial_number: string;
  install_date: string;
  warranty_expires: string;
  location: string;
  expected_life_years: string;
  notes: string;
};

const blank: Draft = {
  kind: "appliance",
  name: "",
  make: "",
  model: "",
  serial_number: "",
  install_date: "",
  warranty_expires: "",
  location: "",
  expected_life_years: "",
  notes: "",
};

/**
 * `unitId`: a unit's own equipment. `null`: only what serves the whole
 * building. Left out: everything on the property (a house).
 */
export function Systems({
  propertyId,
  canOrder,
  manage = false,
  unitId,
  title = "Appliances and systems",
  description = "Age, warranty and life left. Replace or service one straight from here.",
}: {
  propertyId: string;
  canOrder: boolean;
  manage?: boolean;
  unitId?: string | null;
  title?: string;
  description?: string;
}) {
  const qc = useQueryClient();
  const assets = useQuery({
    queryKey: ["assets", propertyId],
    queryFn: () => api.assets({ property_id: propertyId }),
  });
  const [editing, setEditing] = useState<{
    id: string | null;
    draft: Draft;
  } | null>(null);
  const [busy, setBusy] = useState(false);
  const rows = (assets.data ?? [])
    .filter((a) => a.status === "active")
    .filter((a) => unitId === undefined || (a.unit_id ?? null) === unitId);

  async function save() {
    if (!editing) return;
    const d = editing.draft;
    if (!d.name.trim()) return void toast.error("Name it first");
    const years = d.expected_life_years
      ? Number(d.expected_life_years)
      : undefined;
    const body = {
      kind: d.kind,
      name: d.name.trim(),
      make: d.make || undefined,
      model: d.model || undefined,
      serial_number: d.serial_number || undefined,
      install_date: d.install_date || undefined,
      warranty_expires: d.warranty_expires || undefined,
      location: d.location || undefined,
      expected_life_years: years,
      notes: d.notes || undefined,
    };
    setBusy(true);
    try {
      if (editing.id) await api.updateAsset(editing.id, body);
      else
        await api.createAsset({
          property_id: propertyId,
          ...(unitId ? { unit_id: unitId } : {}),
          ...body,
        });
      toast.success("Saved");
      setEditing(null);
      void qc.invalidateQueries({ queryKey: ["assets"] });
      void qc.invalidateQueries({
        queryKey: ["properties", propertyId, "units"],
      });
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
      await api.updateAsset(editing.id, { status: "retired" });
      toast.success("Retired");
      setEditing(null);
      void qc.invalidateQueries({ queryKey: ["assets"] });
      void qc.invalidateQueries({
        queryKey: ["properties", propertyId, "units"],
      });
    } catch (e) {
      toast.error(why(e));
    } finally {
      setBusy(false);
    }
  }

  const groups = ORDER.map(
    (k) => [k, rows.filter((a) => a.kind === k)] as const
  ).filter(([, v]) => v.length);

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
              Add
            </Button>
          )
        }
      />
      <div className="space-y-5 p-5 pt-4">
        {assets.isLoading && <Skeleton className="h-28" />}
        {assets.isSuccess && rows.length === 0 && (
          <EmptyState
            icon={<Refrigerator />}
            title="No equipment registered"
            description="Add the water heater, furnace and appliances so warranties and replacements are tracked."
            className="py-6"
          />
        )}
        {groups.map(([kind, list]) => (
          <section key={kind}>
            <div className="eyebrow mb-2">{label(kind)}</div>
            <ul className="divide-y divide-line rounded-xl border border-line">
              {list.map((a) => {
                const kit = replacementKit(a.name);
                const order = `/console/maintenance/new?property=${propertyId}${kit ? `&kit=${kit}` : ""}&note=${encodeURIComponent(
                  [a.name, a.make, a.model, a.location]
                    .filter(Boolean)
                    .join(" · ")
                )}`;
                return (
                  <li
                    key={a.id}
                    className="flex flex-col gap-2 px-3 py-2.5 sm:flex-row sm:items-center"
                  >
                    <div className="min-w-0 flex-1">
                      <div className="text-[13px] font-medium text-fg">
                        <Link
                          href={`/console/properties/${propertyId}/appliances/${a.id}`}
                          className="hover:text-accent"
                        >
                          {a.name}
                        </Link>
                        {a.location && (
                          <span className="font-normal text-fg-3">
                            {" "}
                            · {a.location}
                          </span>
                        )}
                      </div>
                      <div className="text-xs text-fg-3">
                        {[a.make, a.model].filter(Boolean).join(" ")}
                        {a.install_date &&
                          ` · installed ${a.install_date.slice(0, 4)}`}
                        {a.serial_number && ` · SN ${a.serial_number}`}
                      </div>
                    </div>
                    <div className="flex shrink-0 flex-wrap items-center gap-1.5">
                      {a.years_left != null && (
                        <Badge tone={lifeTone(a)}>
                          {a.years_left <= 0
                            ? "past its life"
                            : `${a.years_left} yr left`}
                        </Badge>
                      )}
                      {a.warranty_state === "active" && a.warranty_expires && (
                        <Badge tone="info">
                          warranty to {day(a.warranty_expires)}
                        </Badge>
                      )}
                      {a.warranty_state === "expired" && (
                        <Badge>out of warranty</Badge>
                      )}
                      {a.warranty_state === "active" &&
                        a.warranty_days_left != null &&
                        a.warranty_days_left <= 90 && (
                          <Badge tone="warn">
                            warranty ends in {a.warranty_days_left} days
                          </Badge>
                        )}
                      {manage && (
                        <Button
                          size="icon"
                          variant="ghost"
                          aria-label={`Edit ${a.name}`}
                          onClick={() =>
                            setEditing({
                              id: a.id,
                              draft: {
                                kind: a.kind,
                                name: a.name,
                                make: a.make ?? "",
                                model: a.model ?? "",
                                serial_number: a.serial_number ?? "",
                                install_date:
                                  a.install_date?.slice(0, 10) ?? "",
                                warranty_expires:
                                  a.warranty_expires?.slice(0, 10) ?? "",
                                location: a.location ?? "",
                                expected_life_years: a.expected_life_years
                                  ? String(a.expected_life_years)
                                  : "",
                                notes: a.notes ?? "",
                              },
                            })
                          }
                        >
                          <Pencil />
                        </Button>
                      )}
                      {canOrder && (
                        <Button size="sm" variant="ghost" asChild>
                          <Link href={order}>
                            <Wrench />
                            {kit?.startsWith("service")
                              ? "Service"
                              : kit
                                ? "Replace"
                                : "Work order"}
                          </Link>
                        </Button>
                      )}
                    </div>
                  </li>
                );
              })}
            </ul>
          </section>
        ))}
      </div>
      <FormDialog
        open={!!editing}
        onOpenChange={(o) => !o && setEditing(null)}
        title={editing?.id ? "Edit equipment" : "Add equipment"}
        description={unitId ? "Goes on this unit." : undefined}
        busy={busy}
        onSave={() => void save()}
        onDelete={editing?.id ? () => void retire() : undefined}
      >
        {editing && (
          <div className="grid gap-3 sm:grid-cols-2">
            <F label="Name" className="sm:col-span-2">
              <input
                className={input}
                placeholder="Refrigerator"
                value={editing.draft.name}
                onChange={(e) =>
                  setEditing({
                    ...editing,
                    draft: { ...editing.draft, name: e.target.value },
                  })
                }
              />
            </F>
            <F label="Type">
              <select
                className={input}
                value={editing.draft.kind}
                onChange={(e) =>
                  setEditing({
                    ...editing,
                    draft: { ...editing.draft, kind: e.target.value },
                  })
                }
              >
                {KINDS.map(([k, l]) => (
                  <option key={k} value={k}>
                    {l}
                  </option>
                ))}
              </select>
            </F>
            <F label="Where">
              <input
                className={input}
                placeholder="Kitchen"
                value={editing.draft.location}
                onChange={(e) =>
                  setEditing({
                    ...editing,
                    draft: { ...editing.draft, location: e.target.value },
                  })
                }
              />
            </F>
            {(["make", "model", "serial_number"] as const).map((k) => (
              <F
                key={k}
                label={
                  k === "serial_number"
                    ? "Serial number"
                    : k === "make"
                      ? "Make"
                      : "Model"
                }
              >
                <input
                  className={input}
                  value={editing.draft[k]}
                  onChange={(e) =>
                    setEditing({
                      ...editing,
                      draft: { ...editing.draft, [k]: e.target.value },
                    })
                  }
                />
              </F>
            ))}
            <F label="Expected life (years)">
              <input
                type="number"
                min={1}
                className={input}
                value={editing.draft.expected_life_years}
                onChange={(e) =>
                  setEditing({
                    ...editing,
                    draft: {
                      ...editing.draft,
                      expected_life_years: e.target.value,
                    },
                  })
                }
              />
            </F>
            <F label="Installed">
              <input
                type="date"
                className={input}
                value={editing.draft.install_date}
                onChange={(e) =>
                  setEditing({
                    ...editing,
                    draft: { ...editing.draft, install_date: e.target.value },
                  })
                }
              />
            </F>
            <F label="Warranty ends">
              <input
                type="date"
                className={input}
                value={editing.draft.warranty_expires}
                onChange={(e) =>
                  setEditing({
                    ...editing,
                    draft: {
                      ...editing.draft,
                      warranty_expires: e.target.value,
                    },
                  })
                }
              />
            </F>
            <F label="Notes" className="sm:col-span-2">
              <textarea
                rows={2}
                className={input}
                value={editing.draft.notes}
                onChange={(e) =>
                  setEditing({
                    ...editing,
                    draft: { ...editing.draft, notes: e.target.value },
                  })
                }
              />
            </F>
          </div>
        )}
      </FormDialog>
    </Panel>
  );
}
