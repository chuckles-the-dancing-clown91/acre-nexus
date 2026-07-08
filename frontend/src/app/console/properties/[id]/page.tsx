"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { useParams } from "next/navigation";
import Link from "next/link";
import {
  api,
  type DocumentEntry,
  type PropertyDocuments,
  type PropertyMedia,
  type PropertyMediaItem,
} from "@/lib/api";
import type {
  EnrichmentRun,
  Lease,
  Lien,
  MaintenanceTicket,
  Ownership,
  PropertyFinancials,
  PropertyIntel,
  PropertyMaintenance,
  PropertyProfile,
  Unit,
  Workflow,
} from "@/lib/types";
import { Badge, Button, Card, StatTile, statusTone } from "@/components/ui";
import { Icon } from "@/components/Icon";
import { AssetsCard } from "@/components/AssetsCard";
import { AssignmentsCard } from "@/components/AssignmentsCard";
import { DocumentsCard } from "@/components/DocumentsCard";
import { useAuth } from "@/lib/auth";
import { logError } from "@/lib/log";

type TabKey = "overview" | "financials" | "maintenance" | "media" | "documents";

const TABS: { key: TabKey; label: string }[] = [
  { key: "overview", label: "Overview" },
  { key: "financials", label: "Financials" },
  { key: "maintenance", label: "Maintenance" },
  { key: "media", label: "Media" },
  { key: "documents", label: "Documents" },
];

export default function PropertyProfilePage() {
  const params = useParams<{ id: string }>();
  const { can } = useAuth();
  const [p, setP] = useState<PropertyProfile | null>(null);
  const [intel, setIntel] = useState<PropertyIntel | null>(null);
  const [runs, setRuns] = useState<EnrichmentRun[]>([]);
  const [financials, setFinancials] = useState<PropertyFinancials | null>(null);
  const [workflow, setWorkflow] = useState<Workflow | null>(null);
  const [units, setUnits] = useState<Unit[]>([]);
  const [leases, setLeases] = useState<Lease[]>([]);
  const [maint, setMaint] = useState<PropertyMaintenance | null>(null);
  const [ownership, setOwnership] = useState<Ownership[]>([]);
  const [liens, setLiens] = useState<Lien[]>([]);
  const [propDocs, setPropDocs] = useState<PropertyDocuments | null>(null);
  const [media, setMedia] = useState<PropertyMedia | null>(null);
  const [tab, setTab] = useState<TabKey>("overview");
  const [error, setError] = useState<string | null>(null);
  const [enriching, setEnriching] = useState(false);

  const id = params.id;

  const loadIntel = useCallback(() => {
    if (!id) return;
    api
      .propertyIntel(id)
      .then(setIntel)
      .catch((e) => logError("failed to load property intel", e));
    api
      .propertyEnrichment(id)
      .then(setRuns)
      .catch((e) => logError("failed to load enrichment runs", e));
  }, [id]);

  const loadFinancing = useCallback(() => {
    if (!id) return;
    api
      .propertyFinancials(id)
      .then(setFinancials)
      .catch((e) => logError("failed to load financials", e));
    api
      .workflow(id)
      .then(setWorkflow)
      .catch((e) => logError("failed to load workflow", e));
  }, [id]);

  const loadOps = useCallback(() => {
    if (!id) return;
    const logFailure = (what: string) => (e: unknown) =>
      logError(`failed to load ${what}`, e);
    api.units(id).then(setUnits).catch(logFailure("units"));
    api.propertyLeases(id).then(setLeases).catch(logFailure("leases"));
    api.propertyMaintenance(id).then(setMaint).catch(logFailure("maintenance"));
    api.ownership(id).then(setOwnership).catch(logFailure("ownership"));
    api.liens(id).then(setLiens).catch(logFailure("liens"));
  }, [id]);

  const loadDocs = useCallback(() => {
    if (!id) return;
    api
      .propertyDocuments(id)
      .then(setPropDocs)
      .catch((e) => logError("failed to load documents", e));
  }, [id]);

  const loadMedia = useCallback(() => {
    if (!id) return;
    api
      .propertyMedia(id)
      .then(setMedia)
      .catch((e) => logError("failed to load media", e));
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
    loadDocs();
    loadMedia();
  }, [id, loadIntel, loadFinancing, loadOps, loadDocs, loadMedia]);

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
          .catch((e) =>
            logError("failed to refresh property after advance", e)
          );
      } catch (e) {
        // The stage tracker stays as-is on failure — but still log it.
        logError("failed to advance workflow", e);
      }
    },
    [id]
  );

  // Trigger enrichment, then poll a couple of times as the queue works through
  // the fanned-out jobs (the scheduler ticks every few seconds). The header
  // (home breakdown, address status) is refreshed too as data lands.
  const enrich = useCallback(async () => {
    if (!id) return;
    setEnriching(true);
    try {
      await api.enrichProperty(id);
      for (const delay of [3500, 8000]) {
        await new Promise((r) => setTimeout(r, delay));
        loadIntel();
        api
          .property(id)
          .then(setP)
          .catch((e) => logError("failed to refresh property", e));
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
  const home = p.home;
  const addr = p.address_status;
  const rental = p.rental_status;

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
          {can("rehab:read") && (
            <Link
              href={`/console/properties/${params.id}/rehab`}
              className="rounded-lg border border-line px-3 py-2 text-sm font-semibold"
            >
              Rehab
            </Link>
          )}
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
        {addr.matched_address ?? addr.address} · {addr.city}
      </p>

      {/* Header dossier: photo (upper-left) + home / address / rental breakdown */}
      <div className="grid gap-6 lg:grid-cols-[minmax(0,20rem)_1fr]">
        <div
          role="img"
          aria-label={`${p.name} photo`}
          className="aspect-[4/3] w-full overflow-hidden rounded-2xl border border-line bg-surface-2 bg-cover bg-center"
          style={
            p.image_url ? { backgroundImage: `url(${p.image_url})` } : undefined
          }
        >
          {!p.image_url && (
            <div className="flex h-full items-center justify-center text-sm text-ink-3">
              No photo
            </div>
          )}
        </div>

        <div className="grid gap-4 sm:grid-cols-2">
          {/* Home breakdown */}
          <Card className="p-5">
            <h2 className="mb-3 font-display text-base font-bold">Home</h2>
            <dl className="space-y-2 text-sm">
              <Row
                k="Beds / baths"
                v={`${home.beds ?? "—"} / ${home.baths ?? "—"}`}
              />
              <Row
                k="Living area"
                v={home.sqft ? `${home.sqft.toLocaleString()} sqft` : "—"}
              />
              <Row
                k="Lot size"
                v={
                  home.lot_size_sqft
                    ? `${home.lot_size_sqft.toLocaleString()} sqft`
                    : "—"
                }
              />
              <Row
                k="Type"
                v={home.property_type ? humanize(home.property_type) : "—"}
              />
              <Row
                k="Stories / parking"
                v={`${home.stories ?? "—"} / ${home.parking_spaces ?? "—"}`}
              />
              <Row
                k="Heating / cooling"
                v={`${home.heating ?? "—"} / ${home.cooling ?? "—"}`}
              />
              <Row
                k="Year built"
                v={home.year_built != null ? `${home.year_built}` : "—"}
              />
            </dl>
          </Card>

          {/* Address status */}
          <Card className="p-5">
            <div className="mb-3 flex items-center justify-between">
              <h2 className="font-display text-base font-bold">Address</h2>
              <Badge tone={addr.verified ? "good" : "neutral"}>
                {addr.verified ? "Verified" : "Unverified"}
              </Badge>
            </div>
            <dl className="space-y-2 text-sm">
              <Row k="Street" v={addr.address} />
              <Row k="City" v={addr.city} />
              {addr.matched_address && (
                <Row k="Matched" v={addr.matched_address} />
              )}
              {addr.geocode_accuracy && (
                <Row k="Geocode" v={humanize(addr.geocode_accuracy)} />
              )}
              <Row k="County" v={addr.county ?? "—"} />
              <Row k="APN" v={addr.apn ?? "—"} mono />
              {addr.latitude != null && addr.longitude != null && (
                <Row
                  k="Coordinates"
                  v={`${addr.latitude.toFixed(5)}, ${addr.longitude.toFixed(5)}`}
                  mono
                />
              )}
            </dl>
          </Card>

          {/* Rental status */}
          <Card className="p-5 sm:col-span-2">
            <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
              <h2 className="font-display text-base font-bold">
                Rental status
              </h2>
              <div className="flex flex-wrap items-center gap-2">
                <Badge tone={statusTone(rental.status)}>{rental.status}</Badge>
                <Badge tone="info">{rental.occupancy} occupied</Badge>
                {rental.vacant_units > 0 && (
                  <Badge tone="warn">{rental.vacant_units} vacant</Badge>
                )}
                {rental.delinquent_leases > 0 && (
                  <Badge tone="bad">{rental.delinquent_leases} behind</Badge>
                )}
              </div>
            </div>
            {rental.active_leases.length > 0 ? (
              <div className="space-y-2">
                {rental.active_leases.map((l) => (
                  <div
                    key={l.lease_id}
                    className="flex items-center justify-between border-b border-line pb-2 text-sm last:border-0"
                  >
                    <div className="min-w-0">
                      <div className="truncate font-semibold">
                        {l.tenant_name}
                      </div>
                      <div className="text-xs text-ink-3">
                        {l.rent_label}/mo
                        {l.balance_cents > 0 ? ` · ${l.balance_label} due` : ""}
                      </div>
                    </div>
                    <Badge tone={leasePaymentTone(l.payment_status)}>
                      {l.payment_status}
                    </Badge>
                  </div>
                ))}
              </div>
            ) : (
              <p className="text-sm text-ink-3">No active tenancies.</p>
            )}
          </Card>
        </div>
      </div>

      {/* Tab bar */}
      <div className="flex flex-wrap gap-1 border-b border-line">
        {TABS.map((t) => (
          <button
            key={t.key}
            onClick={() => setTab(t.key)}
            className={`-mb-px rounded-t-lg border-b-2 px-4 py-2 text-sm font-bold transition ${
              tab === t.key
                ? "border-accent text-ink"
                : "border-transparent text-ink-3 hover:text-ink-2"
            }`}
          >
            {t.label}
          </button>
        ))}
      </div>

      {tab === "overview" && (
        <OverviewTab
          p={p}
          intel={intel}
          d={d}
          latestValue={latestValue}
          workflow={workflow}
          advance={advance}
          canWrite={can("property:write")}
          units={units}
          leases={leases}
          ownership={ownership}
          liens={liens}
          runs={runs}
          id={id}
        />
      )}
      {tab === "financials" && <FinancialsTab data={financials} />}
      {tab === "maintenance" && <MaintenanceTab data={maint} />}
      {tab === "media" && (
        <MediaTab
          id={id}
          data={media}
          reload={loadMedia}
          canWrite={can("property:write")}
        />
      )}
      {tab === "documents" && (
        <DocumentsTab id={id} data={propDocs} reload={loadDocs} />
      )}
    </div>
  );
}

// ---- Overview tab ----------------------------------------------------------

function OverviewTab({
  p,
  intel,
  d,
  latestValue,
  workflow,
  advance,
  canWrite,
  units,
  leases,
  ownership,
  liens,
  runs,
  id,
}: {
  p: PropertyProfile;
  intel: PropertyIntel | null;
  d: PropertyIntel["detail"] | undefined;
  latestValue: PropertyIntel["valuations"][number] | undefined;
  workflow: Workflow | null;
  advance: (stage: string) => void;
  canWrite: boolean;
  units: Unit[];
  leases: Lease[];
  ownership: Ownership[];
  liens: Lien[];
  runs: EnrichmentRun[];
  id: string;
}) {
  return (
    <div className="space-y-6">
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

        {/* Valuation + acquisition */}
        <Card className="p-5">
          <h2 className="mb-4 font-display text-lg font-bold">Valuation</h2>
          <dl className="space-y-3 text-sm">
            <Row k="Manager" v={p.manager || "—"} />
            <Row k="Monthly rent" v={`${p.monthly_rent_label}/mo`} />
            {p.purchase_price_cents != null && (
              <Row
                k="Purchase price"
                v={`$${p.purchase_price_cents.toLocaleString()}`}
              />
            )}
            {p.acquired_on && <Row k="Acquired" v={p.acquired_on} />}
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

      {/* Team / assignments */}
      <AssignmentsCard
        subjectType="property"
        subjectId={id}
        writePermission="property:write"
      />

      {/* Process tracker */}
      {workflow && workflow.stages.length > 0 && (
        <Card className="p-5">
          <div className="mb-4 flex flex-wrap items-center justify-between gap-2">
            <div>
              <h2 className="font-display text-lg font-bold">
                Process — {workflow.strategy_label || "Workflow"}
              </h2>
              <p className="text-sm text-ink-3">
                {workflow.strategy_description}
              </p>
            </div>
            {!canWrite && <Badge tone="neutral">view only</Badge>}
          </div>
          <div className="flex flex-wrap gap-2">
            {workflow.stages.map((s) =>
              canWrite ? (
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
              ) : (
                <span
                  key={s.key}
                  className={`rounded-full px-3 py-1.5 text-xs font-bold ${
                    s.current
                      ? "bg-accent text-on-accent"
                      : s.reached
                        ? "bg-good-soft text-good"
                        : "bg-surface-2 text-ink-2"
                  }`}
                >
                  {s.reached && !s.current ? "✓ " : ""}
                  {s.label}
                </span>
              )
            )}
          </div>
          {workflow.history.length > 0 && (
            <div className="mt-4 space-y-2 border-t border-line pt-3 text-xs text-ink-3">
              {workflow.history.slice(0, 8).map((h) => (
                <div key={h.id}>
                  <div className="flex items-center gap-2">
                    <span className="font-semibold text-ink-2">
                      {h.to_stage}
                    </span>
                    {h.from_stage && <span>← {h.from_stage}</span>}
                    <span>· {h.actor_name ?? "automated"}</span>
                    <span className="ml-auto font-mono">
                      {formatTimestamp(h.created_at)}
                    </span>
                  </div>
                  {h.note && <div className="pl-1 italic">{h.note}</div>}
                </div>
              ))}
            </div>
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

// ---- Financials tab --------------------------------------------------------

function FinancialsTab({ data }: { data: PropertyFinancials | null }) {
  if (!data)
    return (
      <Card className="p-5">
        <p className="text-sm text-ink-3">
          No financials available for this property.
        </p>
      </Card>
    );
  return (
    <div className="space-y-6">
      <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
        <StatTile
          label="Net revenue"
          value={data.net_revenue_label}
          icon="dollar"
        />
        <StatTile
          label="Debt service"
          value={data.debt_service_label}
          icon="dollar"
        />
        <StatTile
          label="Cash flow"
          value={data.cash_flow_label}
          icon="dollar"
        />
        <StatTile label="Equity" value={data.equity_label} icon="dollar" />
      </div>

      {/* Loans, each with the bank that owns it + the contact there */}
      <Card className="p-5">
        <div className="mb-4 flex flex-wrap items-center justify-between gap-2">
          <h2 className="font-display text-lg font-bold">Loans</h2>
          <Badge tone="info">
            Loan balance {data.total_loan_balance_label}
          </Badge>
        </div>
        {data.loans.length > 0 ? (
          <div className="space-y-4">
            {data.loans.map((m) => (
              <div key={m.id} className="rounded-xl border border-line p-4">
                <div className="flex flex-wrap items-center justify-between gap-2">
                  <div>
                    <span className="font-semibold">{humanize(m.kind)}</span>
                    <span className="ml-1 text-ink-3">· lien {m.position}</span>
                    {m.loan_number && (
                      <span className="ml-2 font-mono text-xs text-ink-3">
                        {m.loan_number}
                      </span>
                    )}
                  </div>
                  <Badge tone={loanStatusTone(m.status)}>
                    {humanize(m.status)}
                  </Badge>
                </div>
                <dl className="mt-3 grid gap-x-8 gap-y-2 text-sm sm:grid-cols-2">
                  <Row k="Balance" v={m.current_balance_label ?? "—"} mono />
                  <Row
                    k="Rate"
                    v={
                      m.interest_rate_pct != null
                        ? `${m.interest_rate_pct.toFixed(2)}%`
                        : "—"
                    }
                    mono
                  />
                  <Row
                    k="Payment"
                    v={
                      m.monthly_payment_label
                        ? `${m.monthly_payment_label}/mo`
                        : "—"
                    }
                    mono
                  />
                  <Row
                    k="Escrow"
                    v={
                      m.escrow_monthly_label
                        ? `${m.escrow_monthly_label}/mo`
                        : "—"
                    }
                    mono
                  />
                  {m.maturity_date && <Row k="Matures" v={m.maturity_date} />}
                </dl>
                {/* The bank that owns the loan + contact */}
                <div className="mt-3 rounded-lg bg-surface-2 px-3 py-2 text-sm">
                  {m.lender ? (
                    <>
                      <div className="font-semibold">{m.lender.name}</div>
                      <div className="text-xs text-ink-3">
                        {[m.lender.contact_name, m.lender.phone, m.lender.email]
                          .filter(Boolean)
                          .join(" · ") || "No contact on file"}
                      </div>
                    </>
                  ) : (
                    <span className="text-ink-3">
                      No bank linked to this loan.
                    </span>
                  )}
                </div>
              </div>
            ))}
          </div>
        ) : (
          <p className="text-sm text-ink-3">No loans recorded.</p>
        )}
      </Card>

      {/* Banking: the owning entity's accounts */}
      <Card className="p-5">
        <h2 className="mb-4 font-display text-lg font-bold">Banking</h2>
        {data.bank_accounts.length > 0 ? (
          <div className="space-y-2.5">
            {data.bank_accounts.map((b) => (
              <div
                key={b.id}
                className="flex items-center justify-between border-b border-line pb-2.5 text-sm last:border-0"
              >
                <div>
                  <span className="font-semibold">{b.institution}</span>
                  <span className="ml-2 text-xs text-ink-3">
                    {humanize(b.kind)}
                    {b.masked_number ? ` · ${b.masked_number}` : ""}
                  </span>
                </div>
                <div className="flex items-center gap-2">
                  {b.linked && (
                    <Badge tone="info">{b.provider ?? "linked"}</Badge>
                  )}
                  <Badge tone={b.status === "active" ? "good" : "neutral"}>
                    {b.status}
                  </Badge>
                </div>
              </div>
            ))}
          </div>
        ) : (
          <p className="text-sm text-ink-3">
            No bank accounts for the owning entity.
          </p>
        )}
      </Card>
    </div>
  );
}

// ---- Maintenance tab -------------------------------------------------------

function MaintenanceTab({ data }: { data: PropertyMaintenance | null }) {
  if (!data)
    return (
      <Card className="p-5">
        <p className="text-sm text-ink-3">No maintenance data.</p>
      </Card>
    );
  return (
    <div className="space-y-6">
      <div className="grid grid-cols-2 gap-3 lg:grid-cols-3">
        <StatTile label="Open" value={`${data.open_count}`} />
        <StatTile label="Total" value={`${data.total_count}`} />
        <StatTile
          label="Open cost"
          value={data.open_cost_label}
          icon="dollar"
        />
      </div>
      <Card className="p-5">
        <h2 className="mb-4 font-display text-lg font-bold">
          Open work orders
        </h2>
        {data.open.length > 0 ? (
          <TicketList tickets={data.open} />
        ) : (
          <p className="text-sm text-ink-3">No open work orders.</p>
        )}
      </Card>
      <Card className="p-5">
        <h2 className="mb-4 font-display text-lg font-bold">History</h2>
        {data.history.length > 0 ? (
          <TicketList tickets={data.history} />
        ) : (
          <p className="text-sm text-ink-3">No resolved work orders yet.</p>
        )}
      </Card>
    </div>
  );
}

function TicketList({ tickets }: { tickets: MaintenanceTicket[] }) {
  return (
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
              {t.reporter ? ` · ${t.reporter}` : ""}
              {t.cost_label ? ` · ${t.cost_label}` : ""} ·{" "}
              {t.created_at.slice(0, 10)}
            </div>
          </div>
          <div className="flex items-center gap-2">
            <Badge tone={ticketPriorityTone(t.priority)}>{t.priority}</Badge>
            <Badge tone="info">{humanize(t.status)}</Badge>
          </div>
        </div>
      ))}
    </div>
  );
}

// ---- Documents tab ---------------------------------------------------------

function MediaTab({
  id,
  data,
  reload,
  canWrite,
}: {
  id: string;
  data: PropertyMedia | null;
  reload: () => void;
  canWrite: boolean;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const fileInput = useRef<HTMLInputElement>(null);

  async function onUpload(e: React.ChangeEvent<HTMLInputElement>) {
    const file = e.target.files?.[0];
    if (!file || !id) return;
    setBusy(true);
    setError(null);
    try {
      await api.uploadDocument(
        {
          owner_type: "property",
          owner_id: id,
          filename: file.name,
          mime_type: file.type || "application/octet-stream",
          category: "photo",
        },
        file
      );
      reload();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Upload failed.");
      logError("failed to upload photo", err);
    } finally {
      setBusy(false);
      if (fileInput.current) fileInput.current.value = "";
    }
  }

  async function makeHero(item: PropertyMediaItem) {
    if (!id) return;
    try {
      await api.setPropertyHero(id, item.is_hero ? null : item.document_id);
      reload();
    } catch (err) {
      logError("failed to set hero", err);
    }
  }

  const items = data?.items ?? [];

  return (
    <Card className="space-y-4 p-5">
      <div className="flex items-center justify-between">
        <div>
          <h3 className="font-display text-lg font-bold">
            Photos & floorplans
          </h3>
          <p className="text-sm text-ink-3">
            Property media, stored in the document service. Set one as the hero
            shown on the profile header.
          </p>
        </div>
        {canWrite && (
          <>
            <input
              ref={fileInput}
              type="file"
              accept="image/*"
              className="hidden"
              onChange={onUpload}
            />
            <Button
              variant="outline"
              disabled={busy}
              onClick={() => fileInput.current?.click()}
            >
              {busy ? "Uploading…" : "Upload photo"}
            </Button>
          </>
        )}
      </div>

      {error && <div className="text-sm text-bad">{error}</div>}

      {items.length === 0 ? (
        <div className="rounded-xl border border-dashed border-line px-4 py-10 text-center text-sm text-ink-3">
          No photos yet.
          {canWrite && " Upload one to build the property gallery."}
        </div>
      ) : (
        <div className="grid grid-cols-2 gap-4 md:grid-cols-3">
          {items.map((item) => (
            <div key={item.document_id} className="space-y-2">
              <div className="relative aspect-[4/3] overflow-hidden rounded-xl border border-line bg-surface-2">
                {item.url ? (
                  // eslint-disable-next-line @next/next/no-img-element
                  <img
                    src={item.url}
                    alt={item.filename}
                    className="h-full w-full object-cover"
                  />
                ) : (
                  <div className="flex h-full items-center justify-center text-xs text-ink-3">
                    {item.filename}
                  </div>
                )}
                {item.is_hero && (
                  <span className="absolute left-2 top-2">
                    <Badge tone="accent">Hero</Badge>
                  </span>
                )}
              </div>
              <div className="flex items-center justify-between gap-2">
                <span className="truncate text-xs text-ink-3">
                  {item.filename}
                </span>
                {canWrite && (
                  <button
                    onClick={() => makeHero(item)}
                    className="shrink-0 text-xs font-semibold text-accent-2 hover:underline"
                  >
                    {item.is_hero ? "Unset hero" : "Set as hero"}
                  </button>
                )}
              </div>
            </div>
          ))}
        </div>
      )}
    </Card>
  );
}

function DocumentsTab({
  id,
  data,
  reload,
}: {
  id: string;
  data: PropertyDocuments | null;
  reload: () => void;
}) {
  const { can } = useAuth();
  const manage = can("document:manage");
  return (
    <div className="space-y-6">
      {data && data.categories.length > 0 && (
        <div className="flex flex-wrap gap-2">
          {data.categories.map((c) => (
            <Badge key={c.category ?? "unfiled"} tone="neutral">
              {(c.category ? humanize(c.category) : "Unfiled") +
                ` · ${c.count}`}
            </Badge>
          ))}
        </div>
      )}

      {/* Wet-ink originals: where each paper original is filed */}
      <Card className="p-5">
        <h2 className="font-display text-lg font-bold">Wet-ink originals</h2>
        <p className="mb-4 text-sm text-ink-3">
          Documents whose paper original is the record of truth — and where it
          is filed.
        </p>
        {data && data.wet_ink_originals.length > 0 ? (
          <div className="space-y-3">
            {data.wet_ink_originals.map((doc) => (
              <WetInkRow
                key={doc.id}
                doc={doc}
                manage={manage}
                onSaved={reload}
              />
            ))}
          </div>
        ) : (
          <p className="text-sm text-ink-3">No wet-ink originals recorded.</p>
        )}
      </Card>

      {/* Full document drawer: list, upload, versions, download */}
      <DocumentsCard ownerType="property" ownerId={id} title="All documents" />

      {/* Equipment registry: AC units, water heaters, appliances… */}
      <AssetsCard propertyId={id} />
    </div>
  );
}

function WetInkRow({
  doc,
  manage,
  onSaved,
}: {
  doc: DocumentEntry;
  manage: boolean;
  onSaved: () => void;
}) {
  const [editing, setEditing] = useState(false);
  const [loc, setLoc] = useState(doc.physical_location ?? "");
  const [saving, setSaving] = useState(false);

  async function save() {
    setSaving(true);
    try {
      await api.updateDocument(doc.id, { physical_location: loc });
      setEditing(false);
      onSaved();
    } catch (e) {
      logError("failed to update document storage location", e);
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="rounded-xl border border-line p-3">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="min-w-0">
          <div className="truncate font-semibold">{doc.filename}</div>
          {doc.category && (
            <span className="text-xs text-ink-3">{humanize(doc.category)}</span>
          )}
        </div>
        {manage && !editing && (
          <button
            onClick={() => {
              setLoc(doc.physical_location ?? "");
              setEditing(true);
            }}
            className="rounded-lg border border-line px-3 py-1.5 text-xs font-semibold"
          >
            {doc.physical_location ? "Edit location" : "Set location"}
          </button>
        )}
      </div>
      {editing ? (
        <div className="mt-2 flex flex-wrap items-center gap-2">
          <input
            value={loc}
            onChange={(e) => setLoc(e.target.value)}
            placeholder="e.g. Fireproof safe — HQ, Drawer 3"
            className="min-w-0 flex-1 rounded-lg border border-line bg-surface px-3 py-1.5 text-sm"
          />
          <button
            onClick={save}
            disabled={saving}
            className="rounded-lg bg-accent px-3 py-1.5 text-xs font-bold text-on-accent disabled:opacity-50"
          >
            {saving ? "Saving…" : "Save"}
          </button>
          <button
            onClick={() => setEditing(false)}
            className="text-xs text-ink-3"
          >
            Cancel
          </button>
        </div>
      ) : (
        <p className="mt-1 text-sm">
          <span className="text-ink-3">Stored at: </span>
          <span className={doc.physical_location ? "text-ink-2" : "text-ink-3"}>
            {doc.physical_location ?? "—"}
          </span>
        </p>
      )}
    </div>
  );
}

// ---- shared bits -----------------------------------------------------------

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

function loanStatusTone(status: string): "good" | "bad" | "neutral" {
  if (status === "active") return "good";
  if (status === "in_default") return "bad";
  return "neutral";
}

function ticketPriorityTone(priority: string): "bad" | "warn" | "neutral" {
  if (priority === "urgent") return "bad";
  if (priority === "high") return "warn";
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
