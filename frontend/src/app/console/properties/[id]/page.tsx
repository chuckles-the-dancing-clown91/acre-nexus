"use client";

import { useCallback, useEffect, useState } from "react";
import { useParams } from "next/navigation";
import Link from "next/link";
import { api } from "@/lib/api";
import type {
  EnrichmentRun,
  Lease,
  Lien,
  MaintenanceTicket,
  Mortgage,
  Ownership,
  PropertyIntel,
  PropertyProfile,
  Unit,
  Workflow,
} from "@/lib/types";
import { Badge, Button, Card, StatTile, statusTone } from "@/components/ui";
import { Icon } from "@/components/Icon";

export default function PropertyProfilePage() {
  const params = useParams<{ id: string }>();
  const [p, setP] = useState<PropertyProfile | null>(null);
  const [intel, setIntel] = useState<PropertyIntel | null>(null);
  const [runs, setRuns] = useState<EnrichmentRun[]>([]);
  const [mortgages, setMortgages] = useState<Mortgage[]>([]);
  const [workflow, setWorkflow] = useState<Workflow | null>(null);
  const [units, setUnits] = useState<Unit[]>([]);
  const [leases, setLeases] = useState<Lease[]>([]);
  const [tickets, setTickets] = useState<MaintenanceTicket[]>([]);
  const [ownership, setOwnership] = useState<Ownership[]>([]);
  const [liens, setLiens] = useState<Lien[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [enriching, setEnriching] = useState(false);

  const id = params.id;

  const loadIntel = useCallback(() => {
    if (!id) return;
    api
      .propertyIntel(id)
      .then(setIntel)
      .catch(() => {});
    api
      .propertyEnrichment(id)
      .then(setRuns)
      .catch(() => {});
  }, [id]);

  const loadFinancing = useCallback(() => {
    if (!id) return;
    api
      .mortgages(id)
      .then(setMortgages)
      .catch(() => {});
    api
      .workflow(id)
      .then(setWorkflow)
      .catch(() => {});
  }, [id]);

  const loadOps = useCallback(() => {
    if (!id) return;
    const swallow = () => {};
    api.units(id).then(setUnits).catch(swallow);
    api.propertyLeases(id).then(setLeases).catch(swallow);
    api.propertyTickets(id).then(setTickets).catch(swallow);
    api.ownership(id).then(setOwnership).catch(swallow);
    api.liens(id).then(setLiens).catch(swallow);
  }, [id]);

  useEffect(() => {
    if (!id) return;
    api
      .property(id)
      .then(setP)
      .catch((e) => setError(e.message));
    loadIntel();
    loadFinancing();
    loadOps();
  }, [id, loadIntel, loadFinancing, loadOps]);

  // Advance the investment workflow to a chosen stage.
  const advance = useCallback(
    async (toStage: string) => {
      if (!id) return;
      try {
        const wf = await api.advanceWorkflow(id, toStage);
        setWorkflow(wf);
        api
          .property(id)
          .then(setP)
          .catch(() => {});
      } catch {
        // ignore — the stage tracker stays as-is on failure
      }
    },
    [id]
  );

  // Trigger enrichment, then poll a couple of times as the queue works through
  // the fanned-out jobs (the scheduler ticks every few seconds).
  const enrich = useCallback(async () => {
    if (!id) return;
    setEnriching(true);
    try {
      await api.enrichProperty(id);
      for (const delay of [3500, 8000]) {
        await new Promise((r) => setTimeout(r, delay));
        loadIntel();
      }
    } finally {
      setEnriching(false);
    }
  }, [id, loadIntel]);

  if (error)
    return <p className="text-bad">Couldn&apos;t load property: {error}</p>;
  if (!p) return <p className="text-ink-3">Loading…</p>;

  const d = intel?.detail;
  const latestValue = intel?.valuations?.[0];

  return (
    <div className="space-y-6">
      <Link
        href="/console/properties"
        className="inline-flex items-center gap-2 text-sm font-semibold text-ink-2"
      >
        <Icon name="back" size={16} /> All properties
      </Link>

      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex flex-wrap items-center gap-3">
          <h1 className="font-display text-3xl font-extrabold tracking-tight">
            {p.name}
          </h1>
          <Badge tone={statusTone(p.status)}>{p.status}</Badge>
        </div>
        <div className="flex items-center gap-2">
          <Link
            href={`/console/properties/${params.id}/tenants`}
            className="rounded-lg border border-line px-3 py-2 text-sm font-semibold"
          >
            Tenant history
          </Link>
          <Button onClick={enrich} disabled={enriching}>
            {enriching ? "Enriching…" : "Enrich data"}
          </Button>
        </div>
      </div>
      <p className="-mt-3 text-ink-3">
        {d?.matched_address ?? p.address} · {p.city}
        {d?.latitude != null && d?.longitude != null && (
          <span className="ml-2 font-mono text-xs text-ink-3">
            ({d.latitude.toFixed(5)}, {d.longitude.toFixed(5)})
          </span>
        )}
      </p>

      {/* KPI row */}
      <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
        {p.kpis.map((k) => (
          <StatTile
            key={k.label}
            label={k.label}
            value={k.amount_label}
            icon="dollar"
          />
        ))}
      </div>

      <div className="grid gap-6 lg:grid-cols-[1.4fr_1fr]">
        {/* Cost breakdown */}
        <Card className="p-5">
          <h2 className="mb-4 font-display text-lg font-bold">
            Monthly cost &amp; revenue
          </h2>
          <div className="space-y-2.5">
            {p.cost_breakdown.map((line) => (
              <div
                key={line.label}
                className="flex items-center justify-between border-b border-line pb-2.5 last:border-0"
              >
                <span className="text-sm text-ink-2">{line.label}</span>
                <span
                  className={`font-mono text-sm ${
                    line.amount_cents >= 0 ? "text-good" : "text-ink"
                  }`}
                >
                  {line.amount_cents >= 0 ? "+" : "−"}
                  {line.amount_label.replace("-", "")}
                </span>
              </div>
            ))}
            <div className="flex items-center justify-between pt-1">
              <span className="font-bold">Net revenue</span>
              <span className="font-mono text-lg font-bold text-good">
                {p.net_revenue_label}
              </span>
            </div>
          </div>
        </Card>

        {/* Details + valuation */}
        <Card className="p-5">
          <h2 className="mb-4 font-display text-lg font-bold">Details</h2>
          <dl className="space-y-3 text-sm">
            <Row k="Units" v={`${p.units}`} />
            <Row k="Occupancy" v={p.occupancy} />
            <Row k="Year built" v={`${p.year_built}`} />
            <Row k="Manager" v={p.manager} />
            <Row k="Monthly rent" v={`${p.monthly_rent_label}/mo`} />
            {d?.beds != null && (
              <Row k="Beds / baths" v={`${d.beds} / ${d.baths ?? "—"}`} />
            )}
            {d?.sqft != null && (
              <Row k="Living area" v={`${d.sqft.toLocaleString()} sqft`} />
            )}
            {d?.property_type && <Row k="Type" v={humanize(d.property_type)} />}
            {latestValue?.estimated_value_label && (
              <Row
                k="Est. value (AVM)"
                v={`${latestValue.estimated_value_label}${
                  latestValue.confidence != null
                    ? ` · ${latestValue.confidence}% conf.`
                    : ""
                }`}
              />
            )}
            {latestValue?.estimated_rent_label && (
              <Row
                k="Est. market rent"
                v={`${latestValue.estimated_rent_label}/mo`}
              />
            )}
          </dl>
        </Card>
      </div>

      {/* Investment workflow */}
      {workflow && workflow.stages.length > 0 && (
        <Card className="p-5">
          <div className="mb-4 flex flex-wrap items-center justify-between gap-2">
            <div>
              <h2 className="font-display text-lg font-bold">
                {workflow.strategy_label || "Workflow"}
              </h2>
              <p className="text-sm text-ink-3">
                {workflow.strategy_description}
              </p>
            </div>
          </div>
          <div className="flex flex-wrap gap-2">
            {workflow.stages.map((s) => (
              <button
                key={s.key}
                onClick={() => advance(s.key)}
                className={`rounded-full px-3 py-1.5 text-xs font-bold transition ${
                  s.current
                    ? "bg-accent text-on-accent"
                    : s.reached
                      ? "bg-good-soft text-good"
                      : "bg-surface-2 text-ink-2 hover:bg-surface"
                }`}
                title={s.current ? "Current stage" : `Move to ${s.label}`}
              >
                {s.reached && !s.current ? "✓ " : ""}
                {s.label}
              </button>
            ))}
          </div>
          {workflow.history.length > 0 && (
            <div className="mt-4 space-y-1.5 border-t border-line pt-3 text-xs text-ink-3">
              {workflow.history.slice(0, 5).map((h) => (
                <div key={h.id} className="flex items-center gap-2">
                  <span className="font-semibold text-ink-2">{h.to_stage}</span>
                  {h.from_stage && <span>← {h.from_stage}</span>}
                  <span className="ml-auto font-mono">
                    {formatTimestamp(h.created_at)}
                  </span>
                </div>
              ))}
            </div>
          )}
        </Card>
      )}

      {/* Financing */}
      {(mortgages.length > 0 || p.financed) && (
        <Card className="p-5">
          <div className="mb-4 flex flex-wrap items-center justify-between gap-2">
            <h2 className="font-display text-lg font-bold">Financing</h2>
            <div className="flex flex-wrap gap-2">
              <Badge tone="info">
                Loan balance {p.total_loan_balance_label}
              </Badge>
              <Badge tone="good">Equity {p.equity_label}</Badge>
            </div>
          </div>
          {mortgages.length > 0 ? (
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b border-line text-left text-xs font-bold uppercase tracking-wide text-ink-3">
                    <th className="py-2">Loan</th>
                    <th className="py-2 text-right">Balance</th>
                    <th className="py-2 text-right">Rate</th>
                    <th className="py-2 text-right">Payment</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-line">
                  {mortgages.map((m) => (
                    <tr key={m.id}>
                      <td className="py-2">
                        <span className="font-semibold">
                          {humanize(m.kind)}
                        </span>{" "}
                        <span className="text-ink-3">· lien {m.position}</span>
                        {m.loan_number && (
                          <span className="ml-1 font-mono text-xs text-ink-3">
                            {m.loan_number}
                          </span>
                        )}
                      </td>
                      <td className="py-2 text-right font-mono text-ink-2">
                        {m.current_balance_label ?? "—"}
                      </td>
                      <td className="py-2 text-right font-mono text-ink-3">
                        {m.interest_rate_pct != null
                          ? `${m.interest_rate_pct.toFixed(2)}%`
                          : "—"}
                      </td>
                      <td className="py-2 text-right font-mono text-ink-2">
                        {m.monthly_payment_label ?? "—"}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          ) : (
            <p className="text-sm text-ink-3">No loans recorded.</p>
          )}
        </Card>
      )}

      {/* Units & leases */}
      {(units.length > 0 || leases.length > 0) && (
        <div className="grid gap-6 lg:grid-cols-2">
          <Card className="p-5">
            <h2 className="mb-4 font-display text-lg font-bold">Units</h2>
            {units.length > 0 ? (
              <div className="space-y-2.5">
                {units.map((u) => (
                  <div
                    key={u.id}
                    className="flex items-center justify-between border-b border-line pb-2.5 last:border-0"
                  >
                    <div>
                      <span className="font-semibold">
                        Unit {u.unit_number}
                      </span>
                      <span className="ml-2 text-xs text-ink-3">
                        {u.beds ?? "—"} bd / {u.baths ?? "—"} ba
                      </span>
                    </div>
                    <div className="flex items-center gap-2">
                      {u.market_rent_label && (
                        <span className="font-mono text-sm text-ink-2">
                          {u.market_rent_label}
                        </span>
                      )}
                      <Badge tone={u.status === "occupied" ? "good" : "warn"}>
                        {u.status}
                      </Badge>
                    </div>
                  </div>
                ))}
              </div>
            ) : (
              <p className="text-sm text-ink-3">No units recorded.</p>
            )}
          </Card>

          <Card className="p-5">
            <h2 className="mb-4 font-display text-lg font-bold">Leases</h2>
            {leases.length > 0 ? (
              <div className="space-y-2.5">
                {leases.map((l) => (
                  <div
                    key={l.id}
                    className="flex items-center justify-between border-b border-line pb-2.5 last:border-0"
                  >
                    <div className="min-w-0">
                      <div className="truncate font-semibold">
                        {l.tenant_name}
                      </div>
                      <div className="text-xs text-ink-3">
                        {l.rent_label}/mo · since {l.start_date}
                      </div>
                    </div>
                    <Badge tone={leasePaymentTone(l.payment_status)}>
                      {l.payment_status}
                    </Badge>
                  </div>
                ))}
              </div>
            ) : (
              <p className="text-sm text-ink-3">No leases recorded.</p>
            )}
          </Card>
        </div>
      )}

      {/* Maintenance tickets */}
      {tickets.length > 0 && (
        <Card className="p-5">
          <h2 className="mb-4 font-display text-lg font-bold">
            Open maintenance
          </h2>
          <div className="space-y-2.5">
            {tickets.map((t) => (
              <div
                key={t.id}
                className="flex items-center justify-between border-b border-line pb-2.5 last:border-0"
              >
                <div className="min-w-0">
                  <div className="truncate font-semibold">{t.title}</div>
                  <div className="text-xs text-ink-3">
                    {humanize(t.category)}
                  </div>
                </div>
                <div className="flex items-center gap-2">
                  <Badge tone={t.priority === "urgent" ? "bad" : "warn"}>
                    {t.priority}
                  </Badge>
                  <Badge tone="info">{humanize(t.status)}</Badge>
                </div>
              </div>
            ))}
          </div>
        </Card>
      )}

      {/* Title: ownership & liens */}
      {(ownership.length > 0 || liens.length > 0) && (
        <div className="grid gap-6 lg:grid-cols-2">
          <Card className="p-5">
            <h2 className="mb-4 font-display text-lg font-bold">
              Ownership (deed)
            </h2>
            {ownership.length > 0 ? (
              <div className="space-y-2.5">
                {ownership.map((o) => (
                  <div
                    key={o.id}
                    className="flex items-center justify-between border-b border-line pb-2.5 last:border-0"
                  >
                    <div>
                      <div className="font-semibold">{o.owner_name}</div>
                      <div className="text-xs text-ink-3">
                        {o.vesting ?? humanize(o.owner_kind)}
                        {o.deed_recorded_date
                          ? ` · ${o.deed_recorded_date}`
                          : ""}
                      </div>
                    </div>
                    <span className="font-mono text-sm text-ink-2">
                      {(o.percent_bps / 100).toFixed(0)}%
                    </span>
                  </div>
                ))}
              </div>
            ) : (
              <p className="text-sm text-ink-3">No ownership recorded.</p>
            )}
          </Card>

          <Card className="p-5">
            <h2 className="mb-4 font-display text-lg font-bold">Liens</h2>
            {liens.length > 0 ? (
              <div className="space-y-2.5">
                {liens.map((ln) => (
                  <div
                    key={ln.id}
                    className="flex items-center justify-between border-b border-line pb-2.5 last:border-0"
                  >
                    <div>
                      <div className="font-semibold">{ln.lienholder_name}</div>
                      <div className="text-xs text-ink-3">
                        {humanize(ln.kind)}
                        {ln.position != null ? ` · lien ${ln.position}` : ""}
                      </div>
                    </div>
                    <div className="flex items-center gap-2">
                      {ln.amount_label && (
                        <span className="font-mono text-sm text-ink-2">
                          {ln.amount_label}
                        </span>
                      )}
                      <Badge tone={ln.status === "active" ? "warn" : "neutral"}>
                        {ln.status}
                      </Badge>
                    </div>
                  </div>
                ))}
              </div>
            ) : (
              <p className="text-sm text-ink-3">No liens recorded.</p>
            )}
          </Card>
        </div>
      )}

      {/* Parcel / county record */}
      {d && (
        <Card className="p-5">
          <h2 className="mb-4 font-display text-lg font-bold">
            Parcel &amp; county record
          </h2>
          <dl className="grid gap-x-8 gap-y-3 text-sm sm:grid-cols-2">
            <Row k="APN" v={d.apn ?? "—"} mono />
            <Row k="County" v={d.county ?? "—"} />
            <Row k="Zoning" v={d.zoning ?? "—"} />
            <Row
              k="Lot size"
              v={
                d.lot_size_sqft
                  ? `${d.lot_size_sqft.toLocaleString()} sqft`
                  : "—"
              }
            />
            <Row k="Owner of record" v={d.owner_of_record ?? "—"} />
            <Row k="Subdivision" v={d.subdivision ?? "—"} />
            <Row
              k="Last sale"
              v={
                d.last_sale_date
                  ? `${d.last_sale_date}${
                      d.last_sale_price_label
                        ? ` · ${d.last_sale_price_label}`
                        : ""
                    }`
                  : "—"
              }
            />
            <Row k="Flood zone" v={d.flood_zone ?? "—"} />
            <Row
              k="Heating / cooling"
              v={`${d.heating ?? "—"} / ${d.cooling ?? "—"}`}
            />
            <Row
              k="Walk score"
              v={d.walk_score != null ? `${d.walk_score}` : "—"}
            />
          </dl>
          {d.last_enriched_at && (
            <p className="mt-4 text-xs text-ink-3">
              Last enriched {formatTimestamp(d.last_enriched_at)}
            </p>
          )}
        </Card>
      )}

      <div className="grid gap-6 lg:grid-cols-2">
        {/* Schools */}
        <Card className="p-5">
          <h2 className="mb-4 font-display text-lg font-bold">Schools</h2>
          {intel?.schools?.length ? (
            <div className="space-y-2.5">
              {intel.schools.map((s) => (
                <div
                  key={`${s.level}-${s.name}`}
                  className="flex items-center justify-between border-b border-line pb-2.5 last:border-0"
                >
                  <div>
                    <div className="font-semibold">{s.name}</div>
                    <div className="text-xs text-ink-3">
                      {humanize(s.level)}
                      {s.grades ? ` · ${s.grades}` : ""}
                      {s.distance_mi != null ? ` · ${s.distance_mi} mi` : ""}
                    </div>
                  </div>
                  {s.rating != null && (
                    <Badge tone={ratingTone(s.rating)}>{s.rating}/10</Badge>
                  )}
                </div>
              ))}
            </div>
          ) : (
            <Empty />
          )}
        </Card>

        {/* Utilities */}
        <Card className="p-5">
          <h2 className="mb-4 font-display text-lg font-bold">Utilities</h2>
          {intel?.utilities?.length ? (
            <div className="space-y-2.5">
              {intel.utilities.map((u) => (
                <div
                  key={u.utility_type}
                  className="flex items-center justify-between border-b border-line pb-2.5 last:border-0"
                >
                  <div>
                    <div className="font-semibold">
                      {humanize(u.utility_type)}
                    </div>
                    <div className="text-xs text-ink-3">{u.provider}</div>
                  </div>
                  <span className="font-mono text-sm text-ink-2">
                    {u.est_monthly_cost_label
                      ? `${u.est_monthly_cost_label}/mo`
                      : "—"}
                  </span>
                </div>
              ))}
            </div>
          ) : (
            <Empty />
          )}
        </Card>
      </div>

      {/* Tax history */}
      <Card className="p-5">
        <h2 className="mb-4 font-display text-lg font-bold">
          Tax &amp; assessment history
        </h2>
        {intel?.taxes?.length ? (
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-line text-left text-xs font-bold uppercase tracking-wide text-ink-3">
                <th className="py-2">Year</th>
                <th className="py-2">Assessed value</th>
                <th className="py-2 text-right">Tax</th>
                <th className="py-2 text-right">Rate</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-line">
              {intel.taxes.map((t) => (
                <tr key={t.tax_year}>
                  <td className="py-2 font-semibold">{t.tax_year}</td>
                  <td className="py-2 font-mono text-ink-2">
                    {t.assessed_value_label ?? "—"}
                  </td>
                  <td className="py-2 text-right font-mono text-ink-2">
                    {t.tax_amount_label ?? "—"}
                  </td>
                  <td className="py-2 text-right font-mono text-ink-3">
                    {t.tax_rate_bps != null
                      ? `${(t.tax_rate_bps / 100).toFixed(2)}%`
                      : "—"}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : (
          <Empty />
        )}
      </Card>

      {/* Enrichment activity */}
      {runs.length > 0 && (
        <Card className="p-5">
          <h2 className="mb-4 font-display text-lg font-bold">
            Enrichment activity
          </h2>
          <div className="flex flex-wrap gap-2">
            {runs.slice(0, 12).map((r) => (
              <Badge
                key={r.id}
                tone={r.status === "succeeded" ? "good" : "bad"}
              >
                {humanize(r.source)} · {r.provider}
              </Badge>
            ))}
          </div>
        </Card>
      )}
    </div>
  );
}

function Row({ k, v, mono }: { k: string; v: string; mono?: boolean }) {
  return (
    <div className="flex items-center justify-between border-b border-line pb-2.5 last:border-0">
      <dt className="text-ink-3">{k}</dt>
      <dd
        className={mono ? "font-mono text-sm font-semibold" : "font-semibold"}
      >
        {v}
      </dd>
    </div>
  );
}

function Empty() {
  return (
    <p className="text-sm text-ink-3">
      No data yet — run <span className="font-semibold">Enrich data</span> to
      fetch it.
    </p>
  );
}

function ratingTone(rating: number): "good" | "warn" | "neutral" {
  if (rating >= 8) return "good";
  if (rating >= 5) return "warn";
  return "neutral";
}

function leasePaymentTone(status: string): "good" | "warn" | "bad" | "neutral" {
  if (status === "current") return "good";
  if (status === "partial") return "warn";
  if (status === "late") return "bad";
  return "neutral";
}

/** Turn a snake/lower key into a human label, e.g. `single_family` → `Single family`. */
function humanize(key: string): string {
  const s = key.replace(/_/g, " ");
  return s.charAt(0).toUpperCase() + s.slice(1);
}

function formatTimestamp(iso: string): string {
  const dt = new Date(iso);
  if (Number.isNaN(dt.getTime())) return iso;
  return dt.toLocaleString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}
