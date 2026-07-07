"use client";

// Equipment & assets registry for one property: AC units, water heaters,
// appliances and other serviceable utilities, with make/model/serial and
// warranty tracking. Work orders reference these; manuals/photos ride the
// document service (owner_type "asset"). Reads gate on `maintenance:read`
// upstream; writes need `maintenance:manage`.

import { useCallback, useEffect, useState } from "react";
import { api } from "@/lib/api";
import type { Asset, Unit } from "@/lib/types";
import { toast } from "sonner";
import { useAuth } from "@/lib/auth";
import { Badge, Button, Card } from "@/components/ui";
import { DocumentsCard } from "@/components/DocumentsCard";

const KINDS = [
  "hvac",
  "appliance",
  "plumbing",
  "electrical",
  "safety",
  "structural",
  "other",
];

const field =
  "rounded-xl border border-line bg-surface px-3 py-2 text-sm outline-none focus:border-accent";

function warrantyTone(state: string): "good" | "bad" | "neutral" {
  if (state === "active") return "good";
  if (state === "expired") return "bad";
  return "neutral";
}

export function AssetsCard({ propertyId }: { propertyId: string }) {
  const { can } = useAuth();
  const manage = can("maintenance:manage");
  const [assets, setAssets] = useState<Asset[]>([]);
  const [units, setUnits] = useState<Unit[]>([]);
  const [openId, setOpenId] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);
  const [busy, setBusy] = useState(false);

  const load = useCallback(() => {
    api
      .assets({ property_id: propertyId })
      .then(setAssets)
      .catch(() => setAssets([]));
  }, [propertyId]);

  useEffect(() => {
    load();
    api
      .units(propertyId)
      .then(setUnits)
      .catch(() => setUnits([]));
  }, [load, propertyId]);

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

  const unitLabel = (id: string | null) =>
    units.find((u) => u.id === id)?.unit_number;

  return (
    <Card>
      <div className="flex items-center justify-between border-b border-line px-5 py-4">
        <h2 className="font-display text-lg font-bold">Equipment & assets</h2>
        {manage && !adding && (
          <Button
            variant="outline"
            disabled={busy}
            onClick={() => setAdding(true)}
          >
            Register asset
          </Button>
        )}
      </div>
      <div className="space-y-3 p-5 text-sm">
        {adding && (
          <NewAssetForm
            propertyId={propertyId}
            units={units}
            busy={busy}
            onSave={(input) =>
              void run(async () => {
                await api.createAsset(input);
                setAdding(false);
              }, "Asset registered.")
            }
            onCancel={() => setAdding(false)}
          />
        )}
        {assets.length === 0 && !adding && (
          <p className="text-ink-3">
            No equipment registered yet — add the AC units, water heater, and
            appliances so work orders can reference them.
          </p>
        )}
        {assets.map((a) => (
          <div key={a.id} className="rounded-xl border border-line px-4 py-3">
            <button
              className="flex w-full flex-wrap items-center justify-between gap-3 text-left"
              onClick={() => setOpenId(openId === a.id ? null : a.id)}
            >
              <div className="min-w-0">
                <div className="font-semibold">
                  {a.name}
                  {a.status === "retired" && (
                    <span className="ml-2 text-xs text-ink-3">(retired)</span>
                  )}
                </div>
                <div className="truncate text-xs text-ink-3">
                  {a.kind}
                  {unitLabel(a.unit_id)
                    ? ` · Unit ${unitLabel(a.unit_id)}`
                    : ""}
                  {a.make ? ` · ${a.make}` : ""}
                  {a.model ? ` ${a.model}` : ""}
                </div>
              </div>
              <Badge tone={warrantyTone(a.warranty_state)}>
                {a.warranty_state === "none"
                  ? "no warranty"
                  : `warranty ${a.warranty_state}`}
              </Badge>
            </button>
            {openId === a.id && (
              <div className="mt-3 space-y-3">
                <dl className="grid grid-cols-2 gap-3 sm:grid-cols-4">
                  <div>
                    <dt className="text-xs uppercase tracking-wide text-ink-3">
                      Serial
                    </dt>
                    <dd>{a.serial_number ?? "—"}</dd>
                  </div>
                  <div>
                    <dt className="text-xs uppercase tracking-wide text-ink-3">
                      Installed
                    </dt>
                    <dd>{a.install_date ?? "—"}</dd>
                  </div>
                  <div>
                    <dt className="text-xs uppercase tracking-wide text-ink-3">
                      Warranty until
                    </dt>
                    <dd>{a.warranty_expires ?? "—"}</dd>
                  </div>
                  <div>
                    <dt className="text-xs uppercase tracking-wide text-ink-3">
                      Notes
                    </dt>
                    <dd>{a.notes ?? "—"}</dd>
                  </div>
                </dl>
                {manage && (
                  <Button
                    variant="outline"
                    disabled={busy}
                    onClick={() =>
                      void run(
                        () =>
                          api.updateAsset(a.id, {
                            status:
                              a.status === "active" ? "retired" : "active",
                          }),
                        a.status === "active"
                          ? "Asset retired."
                          : "Asset reactivated."
                      )
                    }
                  >
                    {a.status === "active" ? "Retire" : "Reactivate"}
                  </Button>
                )}
                {/* Manuals, warranty docs, photos. */}
                <DocumentsCard
                  ownerType="asset"
                  ownerId={a.id}
                  title="Manuals & documents"
                />
              </div>
            )}
          </div>
        ))}
      </div>
    </Card>
  );
}

function NewAssetForm({
  propertyId,
  units,
  busy,
  onSave,
  onCancel,
}: {
  propertyId: string;
  units: Unit[];
  busy: boolean;
  onSave: (input: {
    property_id: string;
    unit_id?: string;
    kind?: string;
    name: string;
    make?: string;
    model?: string;
    serial_number?: string;
    install_date?: string;
    warranty_expires?: string;
  }) => void;
  onCancel: () => void;
}) {
  const [name, setName] = useState("");
  const [kind, setKind] = useState("hvac");
  const [unitId, setUnitId] = useState("");
  const [make, setMake] = useState("");
  const [model, setModel] = useState("");
  const [serial, setSerial] = useState("");
  const [installDate, setInstallDate] = useState("");
  const [warranty, setWarranty] = useState("");

  function submit(e: React.FormEvent) {
    e.preventDefault();
    if (!name.trim()) {
      toast.error("Give the asset a name.");
      return;
    }
    onSave({
      property_id: propertyId,
      unit_id: unitId || undefined,
      kind,
      name: name.trim(),
      make: make.trim() || undefined,
      model: model.trim() || undefined,
      serial_number: serial.trim() || undefined,
      install_date: installDate || undefined,
      warranty_expires: warranty || undefined,
    });
  }

  return (
    <form
      onSubmit={submit}
      className="flex flex-wrap items-center gap-2 rounded-xl border border-line p-3"
    >
      <input
        className={`${field} w-56`}
        placeholder="Name, e.g. “AC — living room”"
        value={name}
        onChange={(e) => setName(e.target.value)}
      />
      <select
        className={field}
        value={kind}
        onChange={(e) => setKind(e.target.value)}
      >
        {KINDS.map((k) => (
          <option key={k} value={k}>
            {k}
          </option>
        ))}
      </select>
      <select
        className={field}
        value={unitId}
        onChange={(e) => setUnitId(e.target.value)}
      >
        <option value="">Whole property</option>
        {units.map((u) => (
          <option key={u.id} value={u.id}>
            Unit {u.unit_number}
          </option>
        ))}
      </select>
      <input
        className={`${field} w-28`}
        placeholder="Make"
        value={make}
        onChange={(e) => setMake(e.target.value)}
      />
      <input
        className={`${field} w-36`}
        placeholder="Model"
        value={model}
        onChange={(e) => setModel(e.target.value)}
      />
      <input
        className={`${field} w-36`}
        placeholder="Serial #"
        value={serial}
        onChange={(e) => setSerial(e.target.value)}
      />
      <label className="flex items-center gap-1 text-xs text-ink-3">
        installed
        <input
          type="date"
          className={field}
          value={installDate}
          onChange={(e) => setInstallDate(e.target.value)}
        />
      </label>
      <label className="flex items-center gap-1 text-xs text-ink-3">
        warranty until
        <input
          type="date"
          className={field}
          value={warranty}
          onChange={(e) => setWarranty(e.target.value)}
        />
      </label>
      <Button type="submit" disabled={busy}>
        Save
      </Button>
      <Button variant="ghost" type="button" onClick={onCancel}>
        Cancel
      </Button>
    </form>
  );
}
