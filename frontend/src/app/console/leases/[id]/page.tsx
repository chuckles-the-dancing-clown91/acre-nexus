"use client";

import { useCallback, useEffect, useState } from "react";
import { useParams } from "next/navigation";
import Link from "next/link";
import {
  api,
  ApiError,
  type ChargesResp,
  type LeaseChargeDto,
  type LeaseDocDto,
  type VehicleProfile,
} from "@/lib/api";
import type { LeaseDetail } from "@/lib/types";
import { Badge, Card, statusTone } from "@/components/ui";
import { useAuth } from "@/lib/auth";
import { logError } from "@/lib/log";

const CHARGE_KINDS = ["fee", "discount", "rebate", "amenity"];

export default function LeaseDetailPage() {
  const params = useParams<{ id: string }>();
  const id = params.id;
  const { can } = useAuth();
  const manage = can("lease:manage");

  const [lease, setLease] = useState<LeaseDetail | null>(null);
  const [charges, setCharges] = useState<ChargesResp | null>(null);
  const [vehicles, setVehicles] = useState<VehicleProfile[]>([]);
  const [doc, setDoc] = useState<LeaseDocDto | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  const load = useCallback(() => {
    setError(null);
    api
      .lease(id)
      .then(setLease)
      .catch((e) => setError(e.message));
    api
      .leaseCharges(id)
      .then(setCharges)
      .catch((e) => logError("failed to load lease charges", e));
    api
      .vehicles({ lease_id: id })
      .then(setVehicles)
      .catch((e) => logError("failed to load lease vehicles", e));
    api
      .leaseDoc(id)
      .then(setDoc)
      .catch((e) => {
        setDoc(null);
        // A 404 just means the document hasn't been generated yet; anything
        // else (network, 500, …) is unexpected and worth a trace.
        if (!(e instanceof ApiError && e.status === 404)) {
          logError("failed to load lease document", e);
        }
      });
  }, [id]);

  useEffect(() => {
    load();
  }, [load]);

  async function run(key: string, fn: () => Promise<unknown>) {
    setBusy(key);
    setError(null);
    try {
      await fn();
      load();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(null);
    }
  }

  if (!lease) {
    return (
      <div className="space-y-4">
        {error && <p className="text-bad">{error}</p>}
        {!error && <p className="text-ink-3">Loading…</p>}
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div>
        <Link href="/console/leases" className="text-sm text-ink-3">
          ← Back to leases
        </Link>
        <div className="mt-1 flex flex-wrap items-center gap-3">
          <h1 className="font-display text-3xl font-extrabold tracking-tight">
            {lease.tenant_name}
          </h1>
          <Badge tone={statusTone(lease.status)}>{lease.status}</Badge>
          <Badge tone="neutral">{lease.payment_status}</Badge>
          {lease.application_id && <Badge tone="info">from application</Badge>}
          {lease.has_pet && <Badge tone="warn">pet</Badge>}
          {lease.is_military && <Badge tone="info">military</Badge>}
        </div>
        <p className="text-ink-3">
          {lease.tenant_email ?? "no email"} · {lease.start_date}
          {lease.end_date ? ` → ${lease.end_date}` : " (month-to-month)"} ·{" "}
          <Link
            href={`/console/properties/${lease.property_id}`}
            className="underline"
          >
            property
          </Link>
        </p>
      </div>

      {error && <p className="text-bad">{error}</p>}

      {/* Rent + charges */}
      <Card className="overflow-hidden">
        <div className="flex flex-wrap items-center gap-3 border-b border-line px-5 py-4">
          <h2 className="flex-1 font-display text-lg font-bold">
            Rent &amp; charges
          </h2>
          {charges && (
            <span className="font-mono text-sm">
              {charges.monthly_total_label}/mo total
            </span>
          )}
          {manage && (
            <button
              onClick={() => run("apply", () => api.applyFees(id))}
              disabled={busy === "apply"}
              className="rounded-lg border border-line px-3 py-1.5 text-sm font-semibold disabled:opacity-50"
            >
              Auto-apply fee schedule
            </button>
          )}
        </div>
        <div className="divide-y divide-line">
          <div className="flex items-center gap-4 px-5 py-3 text-sm">
            <span className="flex-1 font-semibold">Base rent</span>
            <span className="font-mono">{charges?.base_rent_label}</span>
          </div>
          {charges?.charges.map((c) => (
            <ChargeRow
              key={c.id}
              charge={c}
              manage={manage}
              onRemove={() =>
                run(`del-${c.id}`, () => api.deleteLeaseCharge(c.id))
              }
            />
          ))}
        </div>
        {manage && <AddChargeForm leaseId={id} onAdded={load} />}
      </Card>

      {/* Vehicles */}
      <Card className="overflow-hidden">
        <div className="border-b border-line px-5 py-4">
          <h2 className="font-display text-lg font-bold">Vehicles</h2>
        </div>
        <div className="divide-y divide-line">
          {vehicles.map((v) => (
            <div
              key={v.id}
              className="flex items-center gap-4 px-5 py-3 text-sm"
            >
              <span className="flex-1">{v.label}</span>
              {can("vehicle:manage") && (
                <button
                  onClick={() =>
                    run(`dv-${v.id}`, () => api.deleteVehicle(v.id))
                  }
                  className="text-ink-3"
                >
                  Remove
                </button>
              )}
            </div>
          ))}
          {vehicles.length === 0 && (
            <div className="px-5 py-3 text-sm text-ink-3">
              No vehicles on file.
            </div>
          )}
        </div>
        {can("vehicle:manage") && (
          <AddVehicleForm leaseId={id} onAdded={load} />
        )}
      </Card>

      {/* Lease document */}
      <Card className="overflow-hidden">
        <div className="flex flex-wrap items-center gap-3 border-b border-line px-5 py-4">
          <h2 className="flex-1 font-display text-lg font-bold">
            Lease document
          </h2>
          {doc && (
            <Badge tone={doc.status === "signed" ? "good" : "warn"}>
              {doc.status}
            </Badge>
          )}
          {doc && (
            <button
              onClick={() => printDoc(doc)}
              className="rounded-lg border border-line px-3 py-1.5 text-sm font-semibold"
            >
              Print / Save PDF
            </button>
          )}
          {manage && (
            <button
              onClick={() => run("gen", () => api.generateLeaseDoc(id))}
              disabled={busy === "gen"}
              className="rounded-lg border border-line px-3 py-1.5 text-sm font-semibold disabled:opacity-50"
            >
              {doc ? "Regenerate" : "Generate"}
            </button>
          )}
        </div>
        {doc ? (
          <div className="space-y-3 p-5">
            <pre className="max-h-96 overflow-auto whitespace-pre-wrap rounded-lg border border-line bg-surface-2 p-4 font-mono text-xs leading-relaxed">
              {doc.body}
            </pre>
            {doc.status === "signed" ? (
              <div className="text-sm text-good">
                Signed by {doc.signed_by} on {doc.signed_at?.slice(0, 10)}.
                {doc.signed_hash && (
                  <div className="mt-1 font-mono text-xs text-ink-3">
                    integrity sha256:{doc.signed_hash.slice(0, 16)}…
                  </div>
                )}
              </div>
            ) : (
              manage && <SignForm leaseId={id} onSigned={load} />
            )}
          </div>
        ) : (
          <div className="px-5 py-6 text-sm text-ink-3">
            No document yet — generate one from your branding templates.
          </div>
        )}
      </Card>

      {/* Payment ledger */}
      <Card className="overflow-hidden">
        <div className="border-b border-line px-5 py-4">
          <h2 className="font-display text-lg font-bold">Payment ledger</h2>
        </div>
        <div className="divide-y divide-line">
          {lease.payments.map((p) => (
            <div
              key={p.id}
              className="flex items-center gap-4 px-5 py-3 text-sm"
            >
              <span className="flex-1">{p.due_date}</span>
              <span className="font-mono">{p.amount_label}</span>
              <Badge tone={statusTone(p.status)}>{p.status}</Badge>
            </div>
          ))}
          {lease.payments.length === 0 && (
            <div className="px-5 py-3 text-sm text-ink-3">No payments yet.</div>
          )}
        </div>
      </Card>
    </div>
  );
}

/** Open the lease text in a print window — the browser's "Save as PDF" exports it. */
function printDoc(doc: LeaseDocDto) {
  const w = window.open("", "_blank", "width=800,height=1000");
  if (!w) return;
  const safe = doc.body
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
  w.document.write(
    `<html><head><title>${doc.title}</title><style>` +
      `body{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;white-space:pre-wrap;` +
      `padding:48px;font-size:12px;line-height:1.6;color:#111}</style></head>` +
      `<body>${safe}</body></html>`
  );
  w.document.close();
  w.focus();
  w.print();
}

function ChargeRow({
  charge,
  manage,
  onRemove,
}: {
  charge: LeaseChargeDto;
  manage: boolean;
  onRemove: () => void;
}) {
  const negative = charge.amount_cents < 0;
  return (
    <div className="flex items-center gap-4 px-5 py-3 text-sm">
      <span className="flex-1">
        {charge.label} <Badge tone="neutral">{charge.kind}</Badge>{" "}
        {charge.source === "auto" && <Badge tone="info">auto</Badge>}
      </span>
      <span className={`font-mono ${negative ? "text-good" : ""}`}>
        {negative ? "−" : "+"}
        {charge.amount_label.replace("-", "")}
        {charge.recurring ? "/mo" : ""}
      </span>
      {manage && (
        <button onClick={onRemove} className="text-ink-3">
          ✕
        </button>
      )}
    </div>
  );
}

function AddChargeForm({
  leaseId,
  onAdded,
}: {
  leaseId: string;
  onAdded: () => void;
}) {
  const [kind, setKind] = useState("fee");
  const [label, setLabel] = useState("");
  const [amount, setAmount] = useState("");
  const [busy, setBusy] = useState(false);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    const dollars = parseFloat(amount);
    if (!label.trim() || Number.isNaN(dollars)) return;
    setBusy(true);
    try {
      await api.addLeaseCharge(leaseId, {
        kind,
        label: label.trim(),
        amount_cents: Math.round(dollars * 100),
      });
      setLabel("");
      setAmount("");
      onAdded();
    } finally {
      setBusy(false);
    }
  }

  return (
    <form
      onSubmit={submit}
      className="flex flex-wrap items-end gap-3 border-t border-line bg-surface-2 px-5 py-4"
    >
      <label className="text-sm">
        <span className="mb-1 block text-ink-3">Kind</span>
        <select
          value={kind}
          onChange={(e) => setKind(e.target.value)}
          className="rounded-lg border border-line bg-surface px-3 py-2 capitalize"
        >
          {CHARGE_KINDS.map((k) => (
            <option key={k} value={k}>
              {k}
            </option>
          ))}
        </select>
      </label>
      <label className="flex-1 min-w-[140px] text-sm">
        <span className="mb-1 block text-ink-3">Label</span>
        <input
          value={label}
          onChange={(e) => setLabel(e.target.value)}
          className="w-full rounded-lg border border-line bg-surface px-3 py-2"
        />
      </label>
      <label className="text-sm">
        <span className="mb-1 block text-ink-3">Amount $</span>
        <input
          value={amount}
          onChange={(e) => setAmount(e.target.value)}
          inputMode="decimal"
          className="w-28 rounded-lg border border-line bg-surface px-3 py-2"
        />
      </label>
      <button
        type="submit"
        disabled={busy}
        className="rounded-lg bg-accent px-4 py-2 font-semibold text-white disabled:opacity-50"
      >
        Add charge
      </button>
    </form>
  );
}

function AddVehicleForm({
  leaseId,
  onAdded,
}: {
  leaseId: string;
  onAdded: () => void;
}) {
  const [make, setMake] = useState("");
  const [model, setModel] = useState("");
  const [year, setYear] = useState("");
  const [plate, setPlate] = useState("");
  const [busy, setBusy] = useState(false);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    if (!make.trim() || !model.trim()) return;
    setBusy(true);
    try {
      await api.createVehicle({
        lease_id: leaseId,
        make: make.trim(),
        model: model.trim(),
        year: year ? parseInt(year, 10) : undefined,
        license_plate: plate.trim() || undefined,
      });
      setMake("");
      setModel("");
      setYear("");
      setPlate("");
      onAdded();
    } finally {
      setBusy(false);
    }
  }

  return (
    <form
      onSubmit={submit}
      className="flex flex-wrap items-end gap-3 border-t border-line bg-surface-2 px-5 py-4"
    >
      <Field label="Make" value={make} set={setMake} />
      <Field label="Model" value={model} set={setModel} />
      <Field label="Year" value={year} set={setYear} w="w-20" />
      <Field label="Plate" value={plate} set={setPlate} w="w-28" />
      <button
        type="submit"
        disabled={busy}
        className="rounded-lg bg-accent px-4 py-2 font-semibold text-white disabled:opacity-50"
      >
        Add vehicle
      </button>
    </form>
  );
}

function Field({
  label,
  value,
  set,
  w,
}: {
  label: string;
  value: string;
  set: (s: string) => void;
  w?: string;
}) {
  return (
    <label className="text-sm">
      <span className="mb-1 block text-ink-3">{label}</span>
      <input
        value={value}
        onChange={(e) => set(e.target.value)}
        className={`${w ?? "w-32"} rounded-lg border border-line bg-surface px-3 py-2`}
      />
    </label>
  );
}

function SignForm({
  leaseId,
  onSigned,
}: {
  leaseId: string;
  onSigned: () => void;
}) {
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    if (!name.trim()) return;
    setBusy(true);
    try {
      await api.signLeaseDoc(leaseId, name.trim());
      onSigned();
    } finally {
      setBusy(false);
    }
  }

  return (
    <form onSubmit={submit} className="flex flex-wrap items-end gap-3">
      <label className="flex-1 min-w-[200px] text-sm">
        <span className="mb-1 block text-ink-3">Type full name to sign</span>
        <input
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="Signature"
          className="w-full rounded-lg border border-line bg-surface px-3 py-2"
        />
      </label>
      <button
        type="submit"
        disabled={busy}
        className="rounded-lg bg-accent px-4 py-2 font-semibold text-white disabled:opacity-50"
      >
        Sign &amp; activate lease
      </button>
    </form>
  );
}
