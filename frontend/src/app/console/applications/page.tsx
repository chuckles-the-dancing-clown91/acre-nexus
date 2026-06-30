"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { api, type ConvertInput } from "@/lib/api";
import type { Application, Property } from "@/lib/types";
import { Badge, Card, statusTone } from "@/components/ui";
import { useAuth } from "@/lib/auth";

export default function ApplicationsPage() {
  const { can } = useAuth();
  const canWrite = can("application:write");
  const canLease = can("lease:manage");
  const [apps, setApps] = useState<Application[] | null>(null);
  const [properties, setProperties] = useState<Property[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [converting, setConverting] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  const load = () =>
    api
      .applications()
      .then(setApps)
      .catch((e) => setError(e.message));

  useEffect(() => {
    load();
    api
      .properties()
      .then(setProperties)
      .catch(() => {});
  }, []);

  async function setStatus(id: string, status: string) {
    setBusy(id);
    setError(null);
    try {
      await api.updateApplication(id, status);
      load();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(null);
    }
  }

  return (
    <div className="space-y-6">
      <div>
        <h1 className="font-display text-3xl font-extrabold tracking-tight">
          Applications
        </h1>
        <p className="text-ink-3">
          Applicants submitted through your public website (screened
          automatically). Approve, then convert to a lease.
        </p>
      </div>

      {error && <p className="text-bad">{error}</p>}

      <Card className="overflow-hidden">
        <div className="divide-y divide-line">
          {apps?.map((a) => (
            <div key={a.id} className="px-5 py-3.5">
              <div className="flex flex-wrap items-center gap-4">
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="font-semibold">{a.applicant_name}</span>
                    {a.has_pet && <Badge tone="warn">pet</Badge>}
                    {a.is_military && <Badge tone="info">military</Badge>}
                  </div>
                  <div className="truncate text-sm text-ink-3">{a.email}</div>
                </div>
                <div className="hidden text-sm text-ink-2 sm:block">
                  {a.credit_score ? `Credit ${a.credit_score}` : "—"}
                </div>
                <div className="hidden text-sm text-ink-2 sm:block">
                  {a.annual_income_label}/yr
                </div>
                <Badge tone={statusTone(a.status)}>{a.status}</Badge>
                {canWrite &&
                  a.status !== "Approved" &&
                  a.status !== "Declined" && (
                    <button
                      onClick={() => setStatus(a.id, "Approved")}
                      disabled={busy === a.id}
                      className="rounded-lg border border-line px-3 py-1.5 text-sm font-semibold disabled:opacity-50"
                    >
                      Approve
                    </button>
                  )}
                {canLease && canWrite && a.status === "Approved" && (
                  <button
                    onClick={() =>
                      setConverting(converting === a.id ? null : a.id)
                    }
                    className="rounded-lg bg-accent px-3 py-1.5 text-sm font-semibold text-white"
                  >
                    Create lease
                  </button>
                )}
              </div>
              {converting === a.id && (
                <ConvertForm
                  app={a}
                  properties={properties}
                  onCancel={() => setConverting(null)}
                />
              )}
            </div>
          ))}
          {apps && apps.length === 0 && (
            <div className="px-5 py-10 text-center text-ink-3">
              No applications yet — submit one from the public website.
            </div>
          )}
        </div>
      </Card>
    </div>
  );
}

function ConvertForm({
  app,
  properties,
  onCancel,
}: {
  app: Application;
  properties: Property[];
  onCancel: () => void;
}) {
  const router = useRouter();
  const [propertyId, setPropertyId] = useState(properties[0]?.id ?? "");
  const [rent, setRent] = useState("");
  const [deposit, setDeposit] = useState("");
  const [startDate, setStartDate] = useState(app.move_in ?? "");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    const rentDollars = parseFloat(rent);
    if (!propertyId || Number.isNaN(rentDollars)) {
      setErr("Pick a property and enter rent.");
      return;
    }
    setBusy(true);
    setErr(null);
    try {
      const body: ConvertInput = {
        property_id: propertyId,
        rent_cents: Math.round(rentDollars * 100),
        deposit_cents: deposit
          ? Math.round(parseFloat(deposit) * 100)
          : undefined,
        start_date: startDate || undefined,
      };
      const lease = await api.convertApplication(app.id, body);
      router.push(`/console/leases/${lease.id}`);
    } catch (e) {
      setErr((e as Error).message);
      setBusy(false);
    }
  }

  return (
    <form
      onSubmit={submit}
      className="mt-3 flex flex-wrap items-end gap-3 rounded-lg border border-line bg-surface-2 p-4"
    >
      <label className="text-sm">
        <span className="mb-1 block text-ink-3">Property</span>
        <select
          value={propertyId}
          onChange={(e) => setPropertyId(e.target.value)}
          className="rounded-lg border border-line bg-surface px-3 py-2"
        >
          {properties.map((p) => (
            <option key={p.id} value={p.id}>
              {p.name}
            </option>
          ))}
        </select>
      </label>
      <label className="text-sm">
        <span className="mb-1 block text-ink-3">Rent $/mo</span>
        <input
          value={rent}
          onChange={(e) => setRent(e.target.value)}
          inputMode="decimal"
          className="w-28 rounded-lg border border-line bg-surface px-3 py-2"
        />
      </label>
      <label className="text-sm">
        <span className="mb-1 block text-ink-3">Deposit $</span>
        <input
          value={deposit}
          onChange={(e) => setDeposit(e.target.value)}
          inputMode="decimal"
          className="w-28 rounded-lg border border-line bg-surface px-3 py-2"
        />
      </label>
      <label className="text-sm">
        <span className="mb-1 block text-ink-3">Start date</span>
        <input
          value={startDate}
          onChange={(e) => setStartDate(e.target.value)}
          placeholder="YYYY-MM-DD"
          className="w-36 rounded-lg border border-line bg-surface px-3 py-2"
        />
      </label>
      <button
        type="submit"
        disabled={busy}
        className="rounded-lg bg-accent px-4 py-2 font-semibold text-white disabled:opacity-50"
      >
        Create &amp; open lease
      </button>
      <button
        type="button"
        onClick={onCancel}
        className="rounded-lg border border-line px-3 py-2 text-sm text-ink-3"
      >
        Cancel
      </button>
      {err && <p className="w-full text-sm text-bad">{err}</p>}
    </form>
  );
}
