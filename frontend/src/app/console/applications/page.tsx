"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { api, type ConvertInput } from "@/lib/api";
import type {
  Application,
  ApplicationWorkflow,
  AppWorkflowCatalog,
  Property,
} from "@/lib/types";
import { Badge, Card, statusTone } from "@/components/ui";
import { useAuth } from "@/lib/auth";

export default function ApplicationsPage() {
  const { can } = useAuth();
  const canWrite = can("application:write");
  const canLease = can("lease:manage");
  const [apps, setApps] = useState<Application[] | null>(null);
  const [properties, setProperties] = useState<Property[]>([]);
  const [catalog, setCatalog] = useState<AppWorkflowCatalog | null>(null);
  const [reuseEnabled, setReuseEnabled] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [converting, setConverting] = useState<string | null>(null);
  const [expanded, setExpanded] = useState<string | null>(null);
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
    api
      .applicationWorkflowCatalog()
      .then(setCatalog)
      .catch(() => {});
    // Reuse affordances only appear when the workspace setting is on.
    api
      .settings()
      .then((s) => {
        const r = s.find((x) => x.key === "application_reuse.enabled");
        setReuseEnabled(Boolean(r?.value));
      })
      .catch(() => {});
  }, []);

  // status -> allowed next statuses, from the workflow catalog.
  const transitionsFor = (status: string): string[] => {
    if (!catalog) return [];
    const all = [...catalog.stages, ...catalog.offramps];
    return all.find((s) => s.key === status)?.transitions ?? [];
  };

  async function advance(id: string, to: string) {
    setBusy(id);
    setError(null);
    try {
      await api.advanceApplication(id, to);
      load();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(null);
    }
  }

  async function reuse(id: string) {
    setBusy(id);
    setError(null);
    try {
      await api.reuseApplication(id);
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
          Applicants submitted through your public website move through the
          pipeline: New → Screening → Approved → Leased. Approve, then convert
          to a lease.
        </p>
      </div>

      {error && <p className="text-bad">{error}</p>}

      <Card className="overflow-hidden">
        <div className="divide-y divide-line">
          {apps?.map((a) => {
            const nexts = transitionsFor(a.status).filter(
              (t) => t !== "Leased"
            );
            const canReuse =
              reuseEnabled &&
              canWrite &&
              a.status !== "Declined" &&
              a.status !== "Withdrawn";
            return (
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

                  <button
                    onClick={() => setExpanded(expanded === a.id ? null : a.id)}
                    className="rounded-lg border border-line px-3 py-1.5 text-sm text-ink-3"
                  >
                    {expanded === a.id ? "Hide" : "Pipeline"}
                  </button>

                  {canWrite &&
                    nexts.map((t) => (
                      <button
                        key={t}
                        onClick={() => advance(a.id, t)}
                        disabled={busy === a.id}
                        className="rounded-lg border border-line px-3 py-1.5 text-sm font-semibold disabled:opacity-50"
                      >
                        {t}
                      </button>
                    ))}

                  {canReuse && (
                    <button
                      onClick={() => reuse(a.id)}
                      disabled={busy === a.id}
                      className="rounded-lg border border-line px-3 py-1.5 text-sm text-ink-2 disabled:opacity-50"
                      title="Duplicate this application so it can be used for another property"
                    >
                      Reuse
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

                {expanded === a.id && <WorkflowPanel applicationId={a.id} />}

                {converting === a.id && (
                  <ConvertForm
                    app={a}
                    properties={properties}
                    onCancel={() => setConverting(null)}
                  />
                )}
              </div>
            );
          })}
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

/** The application's pipeline stepper + transition history. */
function WorkflowPanel({ applicationId }: { applicationId: string }) {
  const [wf, setWf] = useState<ApplicationWorkflow | null>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    api
      .applicationWorkflow(applicationId)
      .then((w) => live && setWf(w))
      .catch((e) => live && setErr(e.message));
    return () => {
      live = false;
    };
  }, [applicationId]);

  if (err) return <p className="mt-3 text-sm text-bad">{err}</p>;
  if (!wf) return <p className="mt-3 text-sm text-ink-3">Loading…</p>;

  return (
    <div className="mt-3 space-y-3 rounded-lg border border-line bg-surface-2 p-4">
      <div className="flex flex-wrap items-center gap-2">
        {wf.stages.map((s, i) => (
          <div key={s.key} className="flex items-center gap-2">
            <span
              className={
                "flex h-6 items-center rounded-full px-2.5 text-xs font-semibold " +
                (s.current
                  ? "bg-accent text-white"
                  : s.reached
                    ? "bg-good-soft text-good"
                    : "bg-surface text-ink-3")
              }
            >
              {s.label}
            </span>
            {i < wf.stages.length - 1 && <span className="h-px w-4 bg-line" />}
          </div>
        ))}
      </div>

      {wf.history.length > 0 && (
        <ul className="space-y-1 text-sm text-ink-2">
          {wf.history.map((e) => (
            <li key={e.id}>
              <span className="text-ink-3">
                {new Date(e.created_at).toLocaleDateString()} ·{" "}
              </span>
              {e.from_status ? `${e.from_status} → ` : ""}
              <span className="font-semibold">{e.to_status}</span>
              {e.note ? ` — ${e.note}` : ""}
            </li>
          ))}
        </ul>
      )}
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
