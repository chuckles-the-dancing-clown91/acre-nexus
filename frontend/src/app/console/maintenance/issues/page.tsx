"use client";

// Report an issue: pick the problem, say where, click generate. The work order
// opens with its checklist and a shopping list split into on-the-shelf and to-buy.

import { useEffect, useMemo, useState } from "react";
import Link from "next/link";
import { toast } from "sonner";
import { api } from "@/lib/api";
import type { Asset, Property, Unit } from "@/lib/types";
import {
  CATEGORIES,
  issues,
  type Generated,
  type Issue,
  type IssueInput,
} from "@/lib/issues";
import { useAuth } from "@/lib/auth";
import { Badge, Button, Card } from "@/components/ui";

const field =
  "rounded-xl border border-line bg-surface px-3 py-2 text-sm text-ink";

const blank: IssueInput = {
  name: "",
  category: "general",
  priority: "normal",
  checklist: [],
  parts: [],
};

export default function IssuesPage() {
  const { can } = useAuth();
  const manage = can("maintenance:manage");
  const [list, setList] = useState<Issue[]>([]);
  const [q, setQ] = useState("");
  const [cat, setCat] = useState("");
  const [picked, setPicked] = useState<Issue | null>(null);
  const [editing, setEditing] = useState<{
    id: string | null;
    form: IssueInput;
  } | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = () =>
    issues
      .list()
      .then(setList)
      .catch((e: Error) => setError(e.message));
  useEffect(() => {
    void load();
  }, []);

  const shown = useMemo(
    () =>
      list.filter(
        (i) =>
          (!cat || i.category === cat) &&
          (!q || i.name.toLowerCase().includes(q.toLowerCase()))
      ),
    [list, q, cat]
  );

  return (
    <div className="space-y-5">
      <div className="flex flex-wrap items-center gap-3">
        <div>
          <Link href="/console/maintenance" className="text-xs text-ink-3">
            ← Maintenance
          </Link>
          <h1 className="font-display text-2xl font-bold">Report an issue</h1>
          <p className="text-sm text-ink-3">
            Pick the problem and generate the work order and shopping list.
          </p>
        </div>
        {manage && (
          <Button
            className="ml-auto"
            variant="outline"
            onClick={() => setEditing({ id: null, form: blank })}
          >
            Add to catalog
          </Button>
        )}
      </div>

      <div className="flex flex-wrap gap-2">
        <input
          className={`${field} w-64`}
          placeholder="Search issues…"
          value={q}
          onChange={(e) => setQ(e.target.value)}
        />
        <select
          className={field}
          value={cat}
          onChange={(e) => setCat(e.target.value)}
          aria-label="Category"
        >
          <option value="">All categories</option>
          {CATEGORIES.map((c) => (
            <option key={c} value={c}>
              {c}
            </option>
          ))}
        </select>
      </div>
      {error && <p className="text-sm text-bad">{error}</p>}

      <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
        {shown.map((i) => (
          <Card key={i.id} className="flex flex-col gap-2 p-4">
            <div className="flex items-start gap-2">
              <div className="min-w-0 flex-1">
                <div className="font-semibold">{i.name}</div>
                <div className="text-xs text-ink-3">
                  {i.area ?? "Anywhere"}
                  {i.est_minutes ? ` · about ${i.est_minutes} min` : ""}
                </div>
              </div>
              <Badge tone={i.priority === "urgent" ? "bad" : "neutral"}>
                {i.category}
              </Badge>
            </div>
            <div className="text-xs text-ink-3">
              {i.parts.length
                ? `Parts: ${i.parts.map((p) => `${p.quantity}× ${p.name}`).join(", ")}`
                : "No parts listed"}
            </div>
            <div className="mt-auto flex gap-2 pt-1">
              {manage && (
                <Button onClick={() => setPicked(i)}>Generate ticket</Button>
              )}
              {manage && (
                <Button
                  variant="ghost"
                  onClick={() =>
                    setEditing({
                      id: i.id,
                      form: {
                        name: i.name,
                        area: i.area ?? undefined,
                        category: i.category,
                        priority: i.priority,
                        description: i.description ?? undefined,
                        est_minutes: i.est_minutes ?? undefined,
                        checklist: i.checklist,
                        parts: i.parts,
                      },
                    })
                  }
                >
                  Edit
                </Button>
              )}
            </div>
          </Card>
        ))}
        {shown.length === 0 && (
          <p className="text-sm text-ink-3">No matching issues.</p>
        )}
      </div>

      {picked && (
        <GenerateDialog issue={picked} onClose={() => setPicked(null)} />
      )}
      {editing && (
        <EditDialog
          id={editing.id}
          initial={editing.form}
          onClose={() => setEditing(null)}
          onSaved={() => {
            setEditing(null);
            void load();
          }}
        />
      )}
    </div>
  );
}

function Overlay({
  children,
  onClose,
}: {
  children: React.ReactNode;
  onClose: () => void;
}) {
  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4"
      onClick={onClose}
    >
      <div
        className="max-h-[90vh] w-full max-w-xl overflow-y-auto rounded-2xl border border-line bg-surface p-5 shadow-acre"
        onClick={(e) => e.stopPropagation()}
      >
        {children}
      </div>
    </div>
  );
}

function GenerateDialog({
  issue,
  onClose,
}: {
  issue: Issue;
  onClose: () => void;
}) {
  const [properties, setProperties] = useState<Property[]>([]);
  const [units, setUnits] = useState<Unit[]>([]);
  const [assets, setAssets] = useState<Asset[]>([]);
  const [propertyId, setPropertyId] = useState("");
  const [unitId, setUnitId] = useState("");
  const [assetId, setAssetId] = useState("");
  const [note, setNote] = useState("");
  const [busy, setBusy] = useState(false);
  const [out, setOut] = useState<Generated | null>(null);

  useEffect(() => {
    api
      .properties()
      .then(setProperties)
      .catch(() => {});
  }, []);

  const pickProperty = (id: string) => {
    setPropertyId(id);
    setUnitId("");
    setAssetId("");
    setUnits([]);
    setAssets([]);
    if (!id) return;
    api
      .units(id)
      .then(setUnits)
      .catch(() => {});
    api
      .assets({ property_id: id })
      .then(setAssets)
      .catch(() => {});
  };

  const go = async () => {
    setBusy(true);
    try {
      setOut(
        await issues.generate(issue.id, {
          property_id: propertyId,
          unit_id: unitId || undefined,
          asset_id: assetId || undefined,
          note: note.trim() || undefined,
        })
      );
      toast.success("Work order opened");
    } catch (e) {
      toast.error((e as Error).message);
    } finally {
      setBusy(false);
    }
  };

  if (out) {
    const rows = (
      label: string,
      tone: "good" | "warn" | "neutral",
      items: Generated["parts"]["from_stock"]
    ) =>
      items.length > 0 && (
        <div>
          <Badge tone={tone}>{label}</Badge>
          <ul className="mt-1 space-y-0.5 text-sm">
            {items.map((p) => (
              <li key={p.id}>
                {p.quantity}× {p.name}
              </li>
            ))}
          </ul>
        </div>
      );
    return (
      <Overlay onClose={onClose}>
        <h2 className="font-display text-lg font-bold">{out.ticket.title}</h2>
        <p className="mb-3 text-sm text-ink-3">
          Work order opened. Here is what to bring.
        </p>
        <div className="space-y-3">
          {rows("Grab from stock", "good", out.parts.from_stock)}
          {rows("Shopping list", "warn", out.parts.to_buy)}
          {rows("Maybe", "neutral", out.parts.maybe)}
          {out.parts.from_stock.length +
            out.parts.to_buy.length +
            out.parts.maybe.length ===
            0 && <p className="text-sm text-ink-3">No parts needed.</p>}
        </div>
        <div className="mt-5 flex gap-2">
          <Link href={`/console/maintenance/${out.ticket.id}`}>
            <Button>Open work order</Button>
          </Link>
          <Button variant="outline" onClick={onClose}>
            Close
          </Button>
        </div>
      </Overlay>
    );
  }

  return (
    <Overlay onClose={onClose}>
      <h2 className="font-display text-lg font-bold">{issue.name}</h2>
      <div className="mt-3 grid gap-3">
        <select
          className={field}
          value={propertyId}
          onChange={(e) => pickProperty(e.target.value)}
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
          disabled={!units.length}
        >
          <option value="">Whole property / no unit</option>
          {units.map((u) => (
            <option key={u.id} value={u.id}>
              Unit {u.unit_number}
            </option>
          ))}
        </select>
        <select
          className={field}
          value={assetId}
          onChange={(e) => setAssetId(e.target.value)}
          aria-label="Appliance"
          disabled={!assets.length}
        >
          <option value="">No appliance</option>
          {assets.map((a) => (
            <option key={a.id} value={a.id}>
              {a.name}
            </option>
          ))}
        </select>
        <textarea
          className={field}
          rows={3}
          placeholder="What did you see? (optional)"
          value={note}
          onChange={(e) => setNote(e.target.value)}
        />
      </div>
      <div className="mt-4 flex gap-2">
        <Button onClick={go} disabled={!propertyId || busy}>
          Generate ticket
        </Button>
        <Button variant="outline" onClick={onClose}>
          Cancel
        </Button>
      </div>
    </Overlay>
  );
}

function EditDialog({
  id,
  initial,
  onClose,
  onSaved,
}: {
  id: string | null;
  initial: IssueInput;
  onClose: () => void;
  onSaved: () => void;
}) {
  const [f, setF] = useState<IssueInput>(initial);
  const [busy, setBusy] = useState(false);

  const save = async () => {
    setBusy(true);
    try {
      if (id) await issues.update(id, f);
      else await issues.create(f);
      toast.success("Saved");
      onSaved();
    } catch (e) {
      toast.error((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  const retire = async () => {
    if (!id || !window.confirm("Remove this issue from the catalog?")) return;
    try {
      await issues.retire(id);
      onSaved();
    } catch (e) {
      toast.error((e as Error).message);
    }
  };

  return (
    <Overlay onClose={onClose}>
      <h2 className="font-display text-lg font-bold">
        {id ? "Edit issue" : "New issue"}
      </h2>
      <div className="mt-3 grid gap-3">
        <input
          className={field}
          placeholder="Name"
          value={f.name}
          onChange={(e) => setF({ ...f, name: e.target.value })}
        />
        <div className="grid grid-cols-3 gap-2">
          <select
            className={field}
            value={f.category}
            onChange={(e) => setF({ ...f, category: e.target.value })}
            aria-label="Category"
          >
            {CATEGORIES.map((c) => (
              <option key={c}>{c}</option>
            ))}
          </select>
          <select
            className={field}
            value={f.priority}
            onChange={(e) => setF({ ...f, priority: e.target.value })}
            aria-label="Priority"
          >
            {["low", "normal", "high", "urgent"].map((c) => (
              <option key={c}>{c}</option>
            ))}
          </select>
          <input
            className={field}
            type="number"
            placeholder="Minutes"
            value={f.est_minutes ?? ""}
            onChange={(e) =>
              setF({
                ...f,
                est_minutes: e.target.value
                  ? Number(e.target.value)
                  : undefined,
              })
            }
          />
        </div>
        <input
          className={field}
          placeholder="Where (e.g. Kitchen)"
          value={f.area ?? ""}
          onChange={(e) => setF({ ...f, area: e.target.value })}
        />
        <label className="text-xs font-semibold text-ink-3">
          Checklist, one step per line
          <textarea
            className={`${field} mt-1 w-full`}
            rows={4}
            value={f.checklist.join("\n")}
            onChange={(e) =>
              setF({ ...f, checklist: e.target.value.split("\n") })
            }
          />
        </label>
        <div className="space-y-2">
          <div className="text-xs font-semibold text-ink-3">Usual parts</div>
          {f.parts.map((p, i) => (
            <div key={i} className="flex gap-2">
              <input
                className={`${field} flex-1`}
                placeholder="Part name (matches stock by name)"
                value={p.name}
                onChange={(e) =>
                  setF({
                    ...f,
                    parts: f.parts.map((x, j) =>
                      j === i ? { ...x, name: e.target.value } : x
                    ),
                  })
                }
              />
              <input
                className={`${field} w-20`}
                type="number"
                min={1}
                value={p.quantity}
                onChange={(e) =>
                  setF({
                    ...f,
                    parts: f.parts.map((x, j) =>
                      j === i ? { ...x, quantity: Number(e.target.value) } : x
                    ),
                  })
                }
              />
              <button
                className="text-xs text-bad"
                onClick={() =>
                  setF({ ...f, parts: f.parts.filter((_, j) => j !== i) })
                }
              >
                Remove
              </button>
            </div>
          ))}
          <Button
            variant="ghost"
            onClick={() =>
              setF({
                ...f,
                parts: [
                  ...f.parts,
                  { name: "", quantity: 1, inventory_item_id: null },
                ],
              })
            }
          >
            Add part
          </Button>
        </div>
      </div>
      <div className="mt-4 flex gap-2">
        <Button onClick={save} disabled={!f.name.trim() || busy}>
          Save
        </Button>
        <Button variant="outline" onClick={onClose}>
          Cancel
        </Button>
        {id && (
          <Button variant="ghost" className="ml-auto" onClick={retire}>
            Remove
          </Button>
        )}
      </div>
    </Overlay>
  );
}
