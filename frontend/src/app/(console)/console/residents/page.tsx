"use client";

// One resident, the way a property manager sees them: contact, work, who
// lives there, pets, where they rented before, their tenancies and
// applications, and their ID card. Managers can edit it all.

import { Suspense, useState } from "react";
import Link from "next/link";
import { useSearchParams } from "next/navigation";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowLeft, UserRound } from "lucide-react";
import { toast } from "sonner";
import { api, type ProfileInput, type ResidentDetail } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { parseIncomeCents } from "@/lib/portal-format";
import { useHasTenantScope } from "@/components/shell/tenant-scope";
import { ExtrasForm } from "@/components/resident/ExtrasForm";
import { IdCardView } from "@/components/resident/IdCardView";
import { Badge, statusTone } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Tabs } from "@/components/ui/data-table";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { Fact, F, input } from "@/components/property/bits";
import { NoAccess } from "../leases/_ui/shared";

export default function ResidentPage() {
  return (
    <Suspense fallback={<Skeleton className="h-64" />}>
      <Resident />
    </Suspense>
  );
}

const TABS = [
  ["profile", "Profile"],
  ["household", "Household and pets"],
  ["history", "Rental history"],
  ["applications", "Applications"],
] as const;
type Tab = (typeof TABS)[number][0];

function Resident() {
  const { can } = useAuth();
  const scoped = useHasTenantScope();
  const email = useSearchParams().get("email") ?? "";
  const allowed = can("lease:read");
  const q = useQuery({
    queryKey: ["resident", email],
    queryFn: () => api.residentProfile(email),
    enabled: scoped && allowed && !!email,
    retry: false,
  });
  const [tab, setTab] = useState<Tab>("profile");

  if (!allowed) return <NoAccess what="residents" perm="lease:read" />;
  const d = q.data;

  return (
    <div className="space-y-6">
      <Link
        href="/console/tenant-history"
        className="inline-flex items-center gap-1 text-[13px] text-fg-3 hover:text-fg"
      >
        <ArrowLeft className="size-4" /> Tenant history
      </Link>
      {q.isLoading && <Skeleton className="h-64 rounded-2xl" />}
      {q.error && (
        <Panel>
          <EmptyState
            icon={<UserRound />}
            title="Couldn't find this resident"
            description={q.error.message}
          />
        </Panel>
      )}
      {d && (
        <>
          <PageHeader
            eyebrow="Resident"
            title={d.name}
            description={`${d.email}${d.phone ? ` · ${d.phone}` : ""}`}
            actions={
              <Badge
                tone={
                  d.tenancies.some((t) => t.status === "active")
                    ? "good"
                    : "neutral"
                }
              >
                {d.tenancies.some((t) => t.status === "active")
                  ? "current resident"
                  : "not renting now"}
              </Badge>
            }
          />
          {!d.user_id && (
            <Panel className="p-4 text-[13px] text-fg-2">
              {d.name} has no account yet, so there is no profile to edit.
              Invite them from Team and members, and the profile starts here.
            </Panel>
          )}
          <div className="grid gap-6 lg:grid-cols-[1fr_380px]">
            <div className="min-w-0 space-y-4">
              <Tabs tabs={TABS} value={tab} onChange={setTab} />
              {tab === "profile" && (
                <ProfileTab d={d} canEdit={can("lease:manage")} />
              )}
              {tab === "household" && (
                <HouseholdTab d={d} canEdit={can("lease:manage")} />
              )}
              {tab === "history" && <HistoryTab d={d} />}
              {tab === "applications" && <ApplicationsTab d={d} />}
            </div>
            {d.card && (
              <div className="lg:sticky lg:top-6 lg:self-start">
                <IdCardView card={d.card} />
              </div>
            )}
          </div>
        </>
      )}
    </div>
  );
}

function useSaver(d: ResidentDetail) {
  const qc = useQueryClient();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  async function save(body: {
    profile?: ProfileInput;
    extras?: ResidentDetail["extras"];
    staff_notes?: string;
  }) {
    setBusy(true);
    setError(null);
    try {
      const v = await api.saveResident({ email: d.email, ...body });
      qc.setQueryData(["resident", d.email], v);
      toast.success("Saved.");
    } catch (e) {
      setError(e instanceof Error ? e.message : "Couldn't save");
    } finally {
      setBusy(false);
    }
  }
  return { save, busy, error };
}

function ProfileTab({ d, canEdit }: { d: ResidentDetail; canEdit: boolean }) {
  const p = d.profile;
  const { save, busy, error } = useSaver(d);
  const [f, setF] = useState({
    phone: p?.phone ?? "",
    address_line1: p?.address_line1 ?? "",
    city: p?.city ?? "",
    region: p?.region ?? "",
    postal_code: p?.postal_code ?? "",
    date_of_birth: p?.date_of_birth ?? "",
    income:
      p?.annual_income_cents != null ? String(p.annual_income_cents / 100) : "",
    is_military: p?.is_military ?? false,
  });
  const [notes, setNotes] = useState(d.staff_notes ?? "");
  const [localError, setLocalError] = useState<string | null>(null);
  const set = (k: keyof typeof f, v: string | boolean) =>
    setF((c) => ({ ...c, [k]: v }));
  const editable = canEdit && !!d.user_id;
  const x = d.extras;

  return (
    <div className="space-y-4">
      <Panel>
        <PanelHeader title="Contact and income" />
        {editable ? (
          <form
            className="space-y-4 px-5 pt-3 pb-5"
            onSubmit={(e) => {
              e.preventDefault();
              const cents = parseIncomeCents(f.income);
              if (cents === undefined) {
                setLocalError("Income should be a number.");
                return;
              }
              setLocalError(null);
              save({
                profile: {
                  phone: f.phone,
                  address_line1: f.address_line1,
                  city: f.city,
                  region: f.region,
                  postal_code: f.postal_code,
                  date_of_birth: f.date_of_birth || undefined,
                  annual_income_cents: cents ?? undefined,
                  is_military: f.is_military,
                },
              });
            }}
          >
            <div className="grid gap-3 sm:grid-cols-2">
              <F label="Phone">
                <input
                  className={input}
                  value={f.phone}
                  onChange={(e) => set("phone", e.target.value)}
                />
              </F>
              <F label="Date of birth">
                <input
                  type="date"
                  className={input}
                  value={f.date_of_birth}
                  onChange={(e) => set("date_of_birth", e.target.value)}
                />
              </F>
              <F label="Address" className="block sm:col-span-2">
                <input
                  className={input}
                  value={f.address_line1}
                  onChange={(e) => set("address_line1", e.target.value)}
                />
              </F>
              <F label="City">
                <input
                  className={input}
                  value={f.city}
                  onChange={(e) => set("city", e.target.value)}
                />
              </F>
              <div className="grid grid-cols-2 gap-3">
                <F label="State">
                  <input
                    className={input}
                    value={f.region}
                    onChange={(e) => set("region", e.target.value)}
                  />
                </F>
                <F label="ZIP">
                  <input
                    className={input}
                    value={f.postal_code}
                    onChange={(e) => set("postal_code", e.target.value)}
                  />
                </F>
              </div>
              <F label="Yearly income ($)">
                <input
                  className={input}
                  inputMode="decimal"
                  value={f.income}
                  onChange={(e) => set("income", e.target.value)}
                />
              </F>
              <label className="flex items-center gap-2 self-end pb-2 text-[13px] text-fg">
                <input
                  type="checkbox"
                  checked={f.is_military}
                  onChange={(e) => set("is_military", e.target.checked)}
                />
                Active military or veteran
              </label>
            </div>
            {(localError || error) && (
              <p className="text-[13px] text-bad">{localError ?? error}</p>
            )}
            <div className="flex justify-end">
              <Button type="submit" disabled={busy}>
                {busy ? "Saving..." : "Save contact"}
              </Button>
            </div>
          </form>
        ) : (
          <dl className="divide-y divide-line px-5 pt-2 pb-4">
            <Fact label="Phone">{p?.phone}</Fact>
            <Fact label="Date of birth">{p?.date_of_birth}</Fact>
            <Fact label="Address">
              {[p?.address_line1, p?.city, p?.region, p?.postal_code]
                .filter(Boolean)
                .join(", ")}
            </Fact>
            <Fact label="Yearly income">
              {p?.annual_income_cents != null
                ? `$${(p.annual_income_cents / 100).toLocaleString()}`
                : null}
            </Fact>
            <Fact label="Military">{p?.is_military ? "Yes" : null}</Fact>
          </dl>
        )}
      </Panel>

      <Panel>
        <PanelHeader title="Work and emergency contact" />
        <dl className="divide-y divide-line px-5 pt-2 pb-4">
          <Fact label="Employer">{x.employer}</Fact>
          <Fact label="Job title">{x.job_title}</Fact>
          <Fact label="Work phone">{x.employer_phone}</Fact>
          <Fact label="Emergency contact">
            {x.emergency_contact_name
              ? `${x.emergency_contact_name}${x.emergency_contact_relation ? ` (${x.emergency_contact_relation})` : ""}${x.emergency_contact_phone ? `, ${x.emergency_contact_phone}` : ""}`
              : null}
          </Fact>
          {!x.employer && !x.emergency_contact_name && (
            <p className="py-3 text-[13px] text-fg-3">
              Nothing yet. Add it under Household and pets.
            </p>
          )}
        </dl>
      </Panel>

      {d.user_id && (
        <Panel>
          <PanelHeader
            title="Staff notes"
            description="Only your team sees these."
          />
          <div className="space-y-3 px-5 pt-3 pb-5">
            <textarea
              className={`${input} min-h-24`}
              value={notes}
              readOnly={!editable}
              onChange={(e) => setNotes(e.target.value)}
            />
            {editable && (
              <div className="flex justify-end">
                <Button
                  type="button"
                  variant="secondary"
                  disabled={busy}
                  onClick={() => save({ staff_notes: notes })}
                >
                  Save note
                </Button>
              </div>
            )}
          </div>
        </Panel>
      )}
    </div>
  );
}

function HouseholdTab({ d, canEdit }: { d: ResidentDetail; canEdit: boolean }) {
  const { save, busy, error } = useSaver(d);
  const x = d.extras;
  if (canEdit && d.user_id) {
    return (
      <ExtrasForm
        value={x}
        busy={busy}
        error={error}
        onSave={(extras) => save({ extras })}
      />
    );
  }
  return (
    <Panel>
      <PanelHeader title="Household and pets" />
      <dl className="divide-y divide-line px-5 pt-2 pb-4">
        <Fact label="Others living there">
          {x.occupants.map((o) => o.name).join(", ")}
        </Fact>
        <Fact label="Pets">
          {x.pets.map((p) => `${p.name} (${p.kind})`).join(", ") || "None"}
        </Fact>
      </dl>
    </Panel>
  );
}

function HistoryTab({ d }: { d: ResidentDetail }) {
  return (
    <div className="space-y-4">
      <Panel>
        <PanelHeader title="With you" description="Every lease here." />
        <ul className="divide-y divide-line px-2 pt-2 pb-2">
          {d.tenancies.length === 0 && (
            <li className="px-3 py-3 text-[13px] text-fg-3">No leases yet.</li>
          )}
          {d.tenancies.map((t) => (
            <li key={t.lease_id}>
              <Link
                href={`/console/leases/${t.lease_id}`}
                className="flex flex-wrap items-center gap-3 rounded-lg px-3 py-2.5 text-[13px] hover:bg-fill-2"
              >
                <span className="min-w-[160px] flex-1 font-medium text-fg">
                  {t.property_name ?? "Property"}
                  {t.unit_number && t.unit_number !== "Home"
                    ? `, Unit ${t.unit_number}`
                    : ""}
                </span>
                <span className="text-fg-3">
                  {t.start_date} to {t.end_date ?? "now"}
                </span>
                <span className="figure text-fg-2">{t.rent_label}/mo</span>
                <Badge tone={statusTone(t.status)}>{t.status}</Badge>
              </Link>
            </li>
          ))}
        </ul>
      </Panel>
      <Panel>
        <PanelHeader title="Before" description="Where they rented earlier." />
        <ul className="divide-y divide-line px-5 pt-2 pb-3">
          {d.extras.prior_rentals.length === 0 && (
            <li className="py-3 text-[13px] text-fg-3">Nothing on file.</li>
          )}
          {d.extras.prior_rentals.map((r, i) => (
            <li key={i} className="py-2.5 text-[13px]">
              <div className="font-medium text-fg">{r.address}</div>
              <div className="text-fg-3">
                {[r.from, r.to].filter(Boolean).join(" to ")}
                {r.rent_cents != null
                  ? ` · $${(r.rent_cents / 100).toLocaleString()}/mo`
                  : ""}
              </div>
              <div className="text-fg-3">
                {[r.landlord_name, r.landlord_phone].filter(Boolean).join(", ")}
              </div>
              {r.reason_for_leaving && (
                <div className="text-fg-3">Left: {r.reason_for_leaving}</div>
              )}
            </li>
          ))}
        </ul>
      </Panel>
    </div>
  );
}

function ApplicationsTab({ d }: { d: ResidentDetail }) {
  return (
    <Panel>
      <PanelHeader title="Applications" />
      <ul className="divide-y divide-line px-2 pt-2 pb-2">
        {d.applications.length === 0 && (
          <li className="px-3 py-3 text-[13px] text-fg-3">No applications.</li>
        )}
        {d.applications.map((a) => (
          <li key={a.id}>
            <Link
              href={`/console/applications`}
              className="flex flex-wrap items-center gap-3 rounded-lg px-3 py-2.5 text-[13px] hover:bg-fill-2"
            >
              <span className="min-w-[140px] flex-1 font-medium text-fg">
                {new Date(a.created_at).toLocaleDateString()}
              </span>
              <span className="text-fg-3">
                {a.move_in ? `Move-in ${a.move_in}` : ""}
              </span>
              <span className="figure text-fg-2">{a.annual_income_label}</span>
              {a.screening_status && (
                <Badge tone={a.screening_status === "cleared" ? "good" : "bad"}>
                  {a.screening_status}
                </Badge>
              )}
              <Badge tone={statusTone(a.status)}>{a.status}</Badge>
            </Link>
          </li>
        ))}
      </ul>
    </Panel>
  );
}
