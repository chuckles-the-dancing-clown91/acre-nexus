"use client";

// One appliance: what it is, its warranty, how to look after it, its
// maintenance schedule, everything done to it, the parts that fit and its
// papers. Reached from the property's appliance list.

import { Suspense, useRef, useState } from "react";
import Link from "next/link";
import { useParams, useRouter, useSearchParams } from "next/navigation";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  ArrowLeft,
  Download,
  ExternalLink,
  FileUp,
  Pencil,
  Phone,
  ShieldCheck,
  Wrench,
} from "lucide-react";
import { toast } from "sonner";
import { api, type AssetHistory } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { day, label, replacementKit } from "@/lib/propertyRecords";
import { CareSchedule } from "@/components/appliance/CareSchedule";
import { EditAppliance } from "@/components/appliance/EditAppliance";
import { Fact, why } from "@/components/property/bits";
import { Badge, statusTone } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Tabs } from "@/components/ui/data-table";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";

const TABS = [
  ["overview", "Overview"],
  ["care", "Care and schedule"],
  ["history", "Service history"],
  ["parts", "Parts"],
  ["files", "Files"],
] as const;
type Tab = (typeof TABS)[number][0];

export default function AppliancePage() {
  return (
    <Suspense fallback={<Skeleton className="h-64 rounded-2xl" />}>
      <Appliance />
    </Suspense>
  );
}

function money(cents: number | null | undefined) {
  return cents == null
    ? null
    : (cents / 100).toLocaleString(undefined, {
        style: "currency",
        currency: "USD",
        maximumFractionDigits: 0,
      });
}

function Appliance() {
  const { id, assetId } = useParams<{ id: string; assetId: string }>();
  const { can } = useAuth();
  const params = useSearchParams();
  const router = useRouter();
  const tab = (TABS.find(([k]) => k === params.get("tab"))?.[0] ??
    "overview") as Tab;
  const q = useQuery({
    queryKey: ["asset", assetId],
    queryFn: () => api.assetHistory(assetId),
  });
  const [editing, setEditing] = useState(false);
  const manage = can("maintenance:manage");
  const a = q.data;

  return (
    <div className="space-y-6">
      <Link
        href={`/console/properties/${id}?tab=systems`}
        className="inline-flex items-center gap-1 text-[13px] text-fg-3 hover:text-fg"
      >
        <ArrowLeft className="size-4" /> {a?.property ?? "Property"}
      </Link>
      {q.isLoading && <Skeleton className="h-64 rounded-2xl" />}
      {q.error && (
        <Panel>
          <EmptyState
            title="Couldn't find this appliance"
            description={q.error.message}
          />
        </Panel>
      )}
      {a && (
        <>
          <Header
            a={a}
            propertyId={id}
            manage={manage}
            canOrder={can("maintenance:manage")}
            onEdit={() => setEditing(true)}
          />
          <Stats a={a} />
          <Tabs
            tabs={TABS}
            value={tab}
            onChange={(k) =>
              router.replace(k === "overview" ? "?" : `?tab=${k}`, {
                scroll: false,
              })
            }
          />
          {tab === "overview" && <Overview a={a} manage={manage} />}
          {tab === "care" && <CareSchedule a={a} manage={manage} />}
          {tab === "history" && <History a={a} />}
          {tab === "parts" && <Parts a={a} />}
          {tab === "files" && <Files a={a} manage={manage} />}
          {editing && (
            <EditAppliance
              asset={a}
              open
              onOpenChange={(o) => !o && setEditing(false)}
            />
          )}
        </>
      )}
    </div>
  );
}

function Header({
  a,
  propertyId,
  manage,
  canOrder,
  onEdit,
}: {
  a: AssetHistory;
  propertyId: string;
  manage: boolean;
  canOrder: boolean;
  onEdit: () => void;
}) {
  const kit = replacementKit(a.name);
  const order = `/console/maintenance/new?property=${propertyId}${kit ? `&kit=${kit}` : ""}&note=${encodeURIComponent(
    [a.name, a.make, a.model, a.location].filter(Boolean).join(" · ")
  )}`;
  return (
    <PageHeader
      eyebrow={label(a.kind)}
      title={a.name}
      description={
        [a.make, a.model].filter(Boolean).join(" ") +
        (a.location ? ` · ${a.location}` : "")
      }
      actions={
        <div className="flex flex-wrap items-center gap-2">
          {a.status === "retired" && <Badge>retired</Badge>}
          {manage && (
            <Button size="sm" variant="secondary" onClick={onEdit}>
              <Pencil />
              Edit
            </Button>
          )}
          {canOrder && (
            <Button size="sm" asChild>
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
      }
    />
  );
}

function Stat({
  label: l,
  value,
  hint,
  tone,
}: {
  label: string;
  value: React.ReactNode;
  hint?: React.ReactNode;
  tone?: "good" | "warn" | "bad";
}) {
  return (
    <div className="glass rounded-2xl p-4">
      <div className="eyebrow">{l}</div>
      <div
        className={`figure mt-1.5 text-[22px] leading-none font-semibold ${
          tone === "good"
            ? "text-good"
            : tone === "warn"
              ? "text-warn"
              : tone === "bad"
                ? "text-bad"
                : "text-fg"
        }`}
      >
        {value}
      </div>
      {hint && <div className="mt-1.5 text-xs text-fg-3">{hint}</div>}
    </div>
  );
}

function ageOf(a: AssetHistory): string | null {
  const from = a.install_date ?? a.purchased_on;
  if (!from) return null;
  const years = (Date.now() - new Date(from).getTime()) / (365.25 * 86400000);
  return years < 1 ? "under a year" : `${Math.floor(years)} yr`;
}

function Stats({ a }: { a: AssetHistory }) {
  const left = a.years_left;
  const w = a.warranty_days_left;
  const price = a.purchase_price_cents;
  const share = price ? a.spend_cents / price : null;
  return (
    <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
      <Stat
        label="Age"
        value={ageOf(a) ?? "Unknown"}
        hint={
          left == null
            ? "Add an install date and life"
            : left <= 0
              ? "Past its expected life"
              : `${left} yr of life left`
        }
        tone={
          left == null
            ? undefined
            : left <= 0
              ? "bad"
              : left <= 2
                ? "warn"
                : undefined
        }
      />
      <Stat
        label="Warranty"
        value={
          w == null
            ? "None"
            : w < 0
              ? "Expired"
              : w <= 90
                ? `${w} days`
                : "Covered"
        }
        hint={a.warranty_expires ? `to ${day(a.warranty_expires)}` : undefined}
        tone={w == null ? undefined : w < 0 ? "bad" : w <= 90 ? "warn" : "good"}
      />
      <Stat
        label="Spent on it"
        value={a.spend_label}
        hint={
          share != null
            ? `${Math.round(share * 100)}% of its ${money(price)} price`
            : undefined
        }
        tone={share != null && share >= 0.5 ? "warn" : undefined}
      />
      <Stat
        label="Last serviced"
        value={a.last_serviced ? day(a.last_serviced) : "Never"}
        hint={`${a.tickets.length} work ${a.tickets.length === 1 ? "order" : "orders"}`}
      />
    </div>
  );
}

function Overview({ a, manage }: { a: AssetHistory; manage: boolean }) {
  const qc = useQueryClient();
  const w = a.warranty_days_left;
  const hasWarranty = !!(
    a.warranty_expires ||
    a.warranty_provider ||
    a.warranty_policy_number
  );
  async function checked() {
    try {
      await api.updateAsset(a.id, {
        recall_checked_on: new Date().toISOString().slice(0, 10),
      });
      void qc.invalidateQueries({ queryKey: ["asset", a.id] });
    } catch (e) {
      toast.error(why(e));
    }
  }
  const share = a.purchase_price_cents
    ? a.spend_cents / a.purchase_price_cents
    : null;
  return (
    <div className="grid gap-6 lg:grid-cols-2">
      <Panel>
        <PanelHeader title="Details" />
        <dl className="divide-y divide-line px-5 pt-2 pb-4">
          <Fact label="Make">{a.make}</Fact>
          <Fact label="Model">{a.model}</Fact>
          <Fact label="Serial number">{a.serial_number}</Fact>
          <Fact label="Where it is">{a.location}</Fact>
          <Fact label="Installed">{a.install_date && day(a.install_date)}</Fact>
          <Fact label="Bought">{a.purchased_on && day(a.purchased_on)}</Fact>
          <Fact label="Price">{money(a.purchase_price_cents)}</Fact>
          <Fact label="Expected life">
            {a.expected_life_years ? `${a.expected_life_years} years` : null}
          </Fact>
          <Fact label="Manual">
            {a.manual_url && (
              <a
                href={a.manual_url}
                target="_blank"
                rel="noopener noreferrer"
                className="inline-flex items-center gap-1 text-accent hover:underline"
              >
                Open <ExternalLink className="size-3.5" />
              </a>
            )}
          </Fact>
          <Fact label="Notes">{a.notes}</Fact>
        </dl>
      </Panel>

      <div className="space-y-6">
        <Panel>
          <PanelHeader
            title="Warranty"
            action={
              hasWarranty && (
                <Badge
                  tone={
                    w == null
                      ? "neutral"
                      : w < 0
                        ? "bad"
                        : w <= 90
                          ? "warn"
                          : "good"
                  }
                >
                  {w == null
                    ? "no end date"
                    : w < 0
                      ? "expired"
                      : w <= 90
                        ? `ends in ${w} days`
                        : "covered"}
                </Badge>
              )
            }
          />
          {hasWarranty ? (
            <dl className="divide-y divide-line px-5 pt-2 pb-4">
              <Fact label="Provider">{a.warranty_provider}</Fact>
              <Fact label="Policy number">{a.warranty_policy_number}</Fact>
              <Fact label="Covers">{a.warranty_coverage}</Fact>
              <Fact label="Starts">
                {a.warranty_starts_on && day(a.warranty_starts_on)}
              </Fact>
              <Fact label="Ends">
                {a.warranty_expires && day(a.warranty_expires)}
              </Fact>
              <Fact label="Transfers to a buyer">
                {a.warranty_transferable ? "Yes" : null}
              </Fact>
              <Fact label="To make a claim">
                {a.warranty_phone && (
                  <a
                    href={`tel:${a.warranty_phone}`}
                    className="inline-flex items-center gap-1 text-accent hover:underline"
                  >
                    <Phone className="size-3.5" /> {a.warranty_phone}
                  </a>
                )}
              </Fact>
              <Fact label="Notes">{a.warranty_notes}</Fact>
            </dl>
          ) : (
            <p className="px-5 pt-3 pb-5 text-[13px] text-fg-3">
              No warranty on file.
              {manage && " Add the provider and dates with Edit."}
            </p>
          )}
        </Panel>

        <Panel>
          <PanelHeader
            title="Recalls"
            description={
              a.recall_checked_on
                ? `Last checked ${day(a.recall_checked_on)}.`
                : "Never checked."
            }
            action={
              manage && (
                <Button
                  size="sm"
                  variant="secondary"
                  onClick={() => void checked()}
                >
                  <ShieldCheck />
                  Mark checked today
                </Button>
              )
            }
          />
          <p className="px-5 pt-2 pb-5 text-[13px] text-fg-3">
            Look up the make and model at{" "}
            <a
              href="https://www.cpsc.gov/Recalls"
              target="_blank"
              rel="noopener noreferrer"
              className="text-accent hover:underline"
            >
              cpsc.gov/Recalls
            </a>{" "}
            and mark it checked.
          </p>
        </Panel>

        {share != null && share >= 0.5 && (
          <Panel className="border-warn/40 bg-warn/10 p-4 text-[13px] text-fg-2">
            Repairs have cost {Math.round(share * 100)}% of what this cost new.
            Replacing it may be cheaper than the next repair.
          </Panel>
        )}
      </div>
    </div>
  );
}

function History({ a }: { a: AssetHistory }) {
  return (
    <Panel>
      <PanelHeader
        title="Work done on it"
        description={`${a.spend_label} spent in total.`}
      />
      <ul className="divide-y divide-line px-2 pt-2 pb-2">
        {a.tickets.length === 0 && (
          <li className="px-3 py-3 text-[13px] text-fg-3">
            No work orders yet.
          </li>
        )}
        {a.tickets.map((t) => (
          <li key={t.id}>
            <Link
              href={`/console/maintenance/${t.id}`}
              className="flex flex-wrap items-center gap-3 rounded-lg px-3 py-2.5 text-[13px] hover:bg-fill-2"
            >
              <span className="min-w-[200px] flex-1 font-medium text-fg">
                {t.title}
              </span>
              <span className="text-fg-3">{day(t.created_at)}</span>
              {t.cost_label && (
                <span className="figure text-fg-2">{t.cost_label}</span>
              )}
              <Badge tone={statusTone(t.status)}>{t.status}</Badge>
            </Link>
          </li>
        ))}
      </ul>
    </Panel>
  );
}

function Parts({ a }: { a: AssetHistory }) {
  return (
    <Panel>
      <PanelHeader
        title="Parts that fit"
        description="Filters, belts and the like. They pre-list on this appliance's work orders."
      />
      <ul className="divide-y divide-line px-5 pt-2 pb-3">
        {a.parts.length === 0 && (
          <li className="py-3 text-[13px] text-fg-3">No parts listed.</li>
        )}
        {a.parts.map((p) => (
          <li
            key={p.id}
            className="flex flex-wrap items-center gap-3 py-2.5 text-[13px]"
          >
            <div className="min-w-[200px] flex-1">
              <div className="font-medium text-fg">{p.name}</div>
              <div className="text-xs text-fg-3">
                {[p.role, p.sku && `SKU ${p.sku}`].filter(Boolean).join(" · ")}
              </div>
            </div>
            <span className="text-fg-3">needs {p.quantity}</span>
            <Badge tone={p.in_stock >= p.quantity ? "good" : "warn"}>
              {p.in_stock} in stock
            </Badge>
          </li>
        ))}
      </ul>
    </Panel>
  );
}

const FILE_KINDS = [
  ["manual", "Manual"],
  ["warranty", "Warranty"],
  ["receipt", "Receipt"],
  ["photo", "Photo"],
  ["other", "Other"],
] as const;

function Files({ a, manage }: { a: AssetHistory; manage: boolean }) {
  const qc = useQueryClient();
  const file = useRef<HTMLInputElement>(null);
  const [kind, setKind] = useState<string>("manual");
  const [busy, setBusy] = useState(false);

  async function upload(list: FileList | null) {
    if (!list?.length) return;
    setBusy(true);
    try {
      for (const f of Array.from(list)) {
        await api.uploadDocument(
          {
            owner_type: "asset",
            owner_id: a.id,
            filename: f.name,
            mime_type: f.type || "application/octet-stream",
            category: kind,
          },
          f
        );
      }
      toast.success("Added");
      void qc.invalidateQueries({ queryKey: ["asset", a.id] });
    } catch (e) {
      toast.error(why(e, "Upload failed"));
    } finally {
      setBusy(false);
      if (file.current) file.current.value = "";
    }
  }

  async function open(id: string) {
    try {
      const { url } = await api.documentDownloadUrl(id);
      window.open(url, "_blank", "noopener");
    } catch (e) {
      toast.error(why(e, "Download failed"));
    }
  }

  return (
    <Panel>
      <PanelHeader
        title="Manuals, warranty papers and receipts"
        action={
          manage && (
            <div className="flex items-center gap-2">
              <select
                aria-label="Kind of file"
                className="rounded-xl border border-line bg-surface px-2 py-1.5 text-[13px] text-fg"
                value={kind}
                onChange={(e) => setKind(e.target.value)}
              >
                {FILE_KINDS.map(([k, l]) => (
                  <option key={k} value={k}>
                    {l}
                  </option>
                ))}
              </select>
              <Button
                size="sm"
                variant="secondary"
                loading={busy}
                onClick={() => file.current?.click()}
              >
                <FileUp />
                Add
              </Button>
              <input
                ref={file}
                type="file"
                multiple
                hidden
                onChange={(e) => void upload(e.target.files)}
              />
            </div>
          )
        }
      />
      <ul className="divide-y divide-line px-5 pt-2 pb-3">
        {a.documents.length === 0 && (
          <li className="py-3 text-[13px] text-fg-3">Nothing filed yet.</li>
        )}
        {a.documents.map((d) => (
          <li
            key={d.id}
            className="flex flex-wrap items-center gap-3 py-2.5 text-[13px]"
          >
            <span className="min-w-[200px] flex-1 truncate font-medium text-fg">
              {d.filename}
            </span>
            {d.category && <Badge>{d.category}</Badge>}
            <span className="text-fg-3">{day(d.created_at)}</span>
            <Button
              size="icon"
              variant="ghost"
              aria-label={`Download ${d.filename}`}
              onClick={() => void open(d.id)}
            >
              <Download />
            </Button>
          </li>
        ))}
      </ul>
    </Panel>
  );
}
