"use client";

// "My profile" — the white-glove source of truth. Everything an application
// needs lives here (contact details, pets, military status, income, vehicles,
// government ID), so applying is one click and staying current means updating
// this one page. Back-office staff can edit the same record through IAM.

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import {
  api,
  type MyProfileView,
  type ProfileInput,
  type VehicleProfile,
} from "@/lib/api";
import { SiteHeader } from "@/components/SiteHeader";
import { Badge, Card } from "@/components/ui";
import { useAuth } from "@/lib/auth";

export default function MyProfilePage() {
  const { user, loading } = useAuth();
  const [view, setView] = useState<MyProfileView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const [busy, setBusy] = useState(false);

  // Form state mirrors ProfileInput (the backend merges: only fields present
  // in the payload change). Income is kept as the raw string the user typed
  // and validated on save, so "52,000" or a stray letter can't silently
  // corrupt the stored amount.
  const [form, setForm] = useState<ProfileInput>({});
  const [income, setIncome] = useState("");
  const [ssn, setSsn] = useState("");
  const [govId, setGovId] = useState("");

  const load = useCallback(() => {
    api
      .myProfile()
      .then((v) => {
        setView(v);
        const p = v.profile;
        setForm({
          legal_first_name: p.legal_first_name ?? undefined,
          legal_middle_name: p.legal_middle_name ?? undefined,
          legal_last_name: p.legal_last_name ?? undefined,
          preferred_name: p.preferred_name ?? undefined,
          date_of_birth: p.date_of_birth ?? undefined,
          phone: p.phone ?? undefined,
          address_line1: p.address_line1 ?? undefined,
          address_line2: p.address_line2 ?? undefined,
          city: p.city ?? undefined,
          region: p.region ?? undefined,
          postal_code: p.postal_code ?? undefined,
          country: p.country ?? undefined,
          gov_id_type: p.gov_id_type ?? undefined,
          has_pet: p.has_pet,
          pet_details: p.pet_details ?? undefined,
          is_military: p.is_military,
        });
        setIncome(
          p.annual_income_cents != null
            ? String(p.annual_income_cents / 100)
            : ""
        );
        setError(null);
      })
      .catch((e) => setError(e.message));
  }, []);

  useEffect(() => {
    if (user) load();
  }, [user, load]);

  async function save(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    setSaved(false);
    try {
      const body: ProfileInput = { ...form };
      const rawIncome = income.replace(/[$,\s]/g, "");
      if (rawIncome) {
        if (!/^\d+(\.\d{1,2})?$/.test(rawIncome)) {
          setError("Annual income must be a number, e.g. 52000 or 52,000.50");
          setBusy(false);
          return;
        }
        body.annual_income_cents = Math.round(parseFloat(rawIncome) * 100);
      }
      if (ssn.trim()) body.ssn = ssn.trim();
      if (govId.trim()) body.gov_id_number = govId.trim();
      const v = await api.updateMyProfile(body);
      setView(v);
      setSsn("");
      setGovId("");
      setSaved(true);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  }

  const set = (
    k: keyof ProfileInput,
    v: string | boolean | number | undefined
  ) => setForm((f) => ({ ...f, [k]: v }));

  const field =
    "w-full rounded-lg border border-line bg-surface px-3 py-2 text-sm";

  return (
    <>
      <SiteHeader />
      <main className="mx-auto max-w-[860px] px-6 py-8">
        <h1 className="mb-1 font-display text-3xl font-extrabold tracking-tight">
          My profile
        </h1>
        <p className="mb-6 text-ink-3">
          Keep this current and applications fill themselves in — pets,
          vehicles, income, and ID all come from here.
        </p>

        {!loading && !user && (
          <Card className="p-8 text-center">
            <p className="mb-3 text-ink-2">Sign in to manage your profile.</p>
            <Link
              href="/login"
              className="inline-block rounded-xl bg-accent px-5 py-2.5 text-sm font-bold text-on-accent"
            >
              Sign in
            </Link>
          </Card>
        )}

        {error && <p className="mb-4 text-bad">{error}</p>}
        {saved && <p className="mb-4 text-good">Profile saved.</p>}

        {user && view && (
          <form onSubmit={save} className="space-y-6">
            <Card className="p-5">
              <h2 className="mb-3 font-display text-lg font-bold">Contact</h2>
              <p className="mb-3 text-sm text-ink-3">
                Account email:{" "}
                <span className="font-semibold">{view.email}</span>
              </p>
              <div className="grid gap-3 sm:grid-cols-2">
                <label className="text-sm">
                  <span className="mb-1 block text-ink-3">
                    Legal first name
                  </span>
                  <input
                    value={form.legal_first_name ?? ""}
                    onChange={(e) => set("legal_first_name", e.target.value)}
                    className={field}
                  />
                </label>
                <label className="text-sm">
                  <span className="mb-1 block text-ink-3">
                    Legal middle name
                  </span>
                  <input
                    value={form.legal_middle_name ?? ""}
                    onChange={(e) => set("legal_middle_name", e.target.value)}
                    className={field}
                  />
                </label>
                <label className="text-sm">
                  <span className="mb-1 block text-ink-3">Legal last name</span>
                  <input
                    value={form.legal_last_name ?? ""}
                    onChange={(e) => set("legal_last_name", e.target.value)}
                    className={field}
                  />
                </label>
                <label className="text-sm">
                  <span className="mb-1 block text-ink-3">Date of birth</span>
                  <input
                    type="date"
                    value={form.date_of_birth ?? ""}
                    onChange={(e) =>
                      set("date_of_birth", e.target.value || undefined)
                    }
                    className={field}
                  />
                </label>
                <label className="text-sm">
                  <span className="mb-1 block text-ink-3">Preferred name</span>
                  <input
                    value={form.preferred_name ?? ""}
                    onChange={(e) => set("preferred_name", e.target.value)}
                    className={field}
                  />
                </label>
                <label className="text-sm">
                  <span className="mb-1 block text-ink-3">Mobile phone</span>
                  <input
                    value={form.phone ?? ""}
                    onChange={(e) => set("phone", e.target.value)}
                    placeholder="+1 555 555 0100"
                    className={field}
                  />
                </label>
                <label className="text-sm sm:col-span-2">
                  <span className="mb-1 block text-ink-3">Street address</span>
                  <input
                    value={form.address_line1 ?? ""}
                    onChange={(e) => set("address_line1", e.target.value)}
                    className={field}
                  />
                </label>
                <label className="text-sm sm:col-span-2">
                  <span className="mb-1 block text-ink-3">
                    Apt / suite (optional)
                  </span>
                  <input
                    value={form.address_line2 ?? ""}
                    onChange={(e) => set("address_line2", e.target.value)}
                    className={field}
                  />
                </label>
                <label className="text-sm">
                  <span className="mb-1 block text-ink-3">City</span>
                  <input
                    value={form.city ?? ""}
                    onChange={(e) => set("city", e.target.value)}
                    className={field}
                  />
                </label>
                <label className="text-sm">
                  <span className="mb-1 block text-ink-3">State / region</span>
                  <input
                    value={form.region ?? ""}
                    onChange={(e) => set("region", e.target.value)}
                    className={field}
                  />
                </label>
                <label className="text-sm">
                  <span className="mb-1 block text-ink-3">ZIP / postal</span>
                  <input
                    value={form.postal_code ?? ""}
                    onChange={(e) => set("postal_code", e.target.value)}
                    className={field}
                  />
                </label>
              </div>
            </Card>

            <Card className="p-5">
              <h2 className="mb-3 font-display text-lg font-bold">
                Rental details
              </h2>
              <div className="grid gap-3 sm:grid-cols-2">
                <label className="text-sm">
                  <span className="mb-1 block text-ink-3">
                    Annual income (USD)
                  </span>
                  <input
                    value={income}
                    onChange={(e) => setIncome(e.target.value)}
                    placeholder="52,000"
                    inputMode="decimal"
                    className={field}
                  />
                </label>
                <div className="flex items-end gap-5 pb-1 text-sm">
                  <label className="flex items-center gap-2">
                    <input
                      type="checkbox"
                      checked={form.has_pet ?? false}
                      onChange={(e) => set("has_pet", e.target.checked)}
                    />
                    <span>I have pet(s)</span>
                  </label>
                  <label className="flex items-center gap-2">
                    <input
                      type="checkbox"
                      checked={form.is_military ?? false}
                      onChange={(e) => set("is_military", e.target.checked)}
                    />
                    <span>Active military / veteran</span>
                  </label>
                </div>
                {(form.has_pet ?? false) && (
                  <label className="text-sm sm:col-span-2">
                    <span className="mb-1 block text-ink-3">Pet details</span>
                    <input
                      value={form.pet_details ?? ""}
                      onChange={(e) => set("pet_details", e.target.value)}
                      placeholder="e.g. one 30lb corgi, house-trained"
                      className={field}
                    />
                  </label>
                )}
              </div>
            </Card>

            <Card className="p-5">
              <h2 className="mb-1 font-display text-lg font-bold">
                Identification
              </h2>
              <p className="mb-3 text-sm text-ink-3">
                Stored encrypted — only the last four digits are ever shown
                back.
              </p>
              <div className="grid gap-3 sm:grid-cols-3">
                <label className="text-sm">
                  <span className="mb-1 block text-ink-3">ID type</span>
                  <select
                    value={form.gov_id_type ?? ""}
                    onChange={(e) =>
                      set("gov_id_type", e.target.value || undefined)
                    }
                    className={field}
                  >
                    <option value="">Choose…</option>
                    <option value="drivers_license">
                      Driver&apos;s license
                    </option>
                    <option value="passport">Passport</option>
                    <option value="state_id">State ID</option>
                  </select>
                </label>
                <label className="text-sm">
                  <span className="mb-1 block text-ink-3">
                    ID number{" "}
                    {view.profile.gov_id_last4 &&
                      `(on file ••••${view.profile.gov_id_last4})`}
                  </span>
                  <input
                    value={govId}
                    onChange={(e) => setGovId(e.target.value)}
                    placeholder={view.profile.has_gov_id ? "Replace…" : ""}
                    className={field}
                  />
                </label>
                <label className="text-sm">
                  <span className="mb-1 block text-ink-3">
                    SSN{" "}
                    {view.profile.ssn_last4 &&
                      `(on file ••••${view.profile.ssn_last4})`}
                  </span>
                  <input
                    value={ssn}
                    onChange={(e) => setSsn(e.target.value)}
                    placeholder={view.profile.has_ssn ? "Replace…" : ""}
                    className={field}
                  />
                </label>
              </div>
            </Card>

            <button
              type="submit"
              disabled={busy}
              className="rounded-xl bg-accent px-5 py-2.5 text-sm font-bold text-on-accent disabled:opacity-50"
            >
              {busy ? "Saving…" : "Save profile"}
            </button>
          </form>
        )}

        {user && view && (
          <VehiclesCard vehicles={view.vehicles} onChanged={load} />
        )}
      </main>
    </>
  );
}

function VehiclesCard({
  vehicles,
  onChanged,
}: {
  vehicles: VehicleProfile[];
  onChanged: () => void;
}) {
  const [make, setMake] = useState("");
  const [model, setModel] = useState("");
  const [year, setYear] = useState("");
  const [plate, setPlate] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function add(e: React.FormEvent) {
    e.preventDefault();
    if (!make.trim() || !model.trim()) return;
    setBusy(true);
    setError(null);
    try {
      await api.addMyVehicle({
        make: make.trim(),
        model: model.trim(),
        year: year ? parseInt(year, 10) : undefined,
        license_plate: plate.trim() || undefined,
      });
      setMake("");
      setModel("");
      setYear("");
      setPlate("");
      onChanged();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  }

  async function remove(id: string) {
    setBusy(true);
    try {
      await api.deleteMyVehicle(id);
      onChanged();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  }

  return (
    <Card className="mt-6 overflow-hidden">
      <div className="border-b border-line px-5 py-4">
        <h2 className="font-display text-lg font-bold">My vehicles</h2>
        <p className="text-sm text-ink-3">
          Attached to every application automatically — properties use them for
          parking and garage amenities.
        </p>
      </div>
      {error && <p className="px-5 py-2 text-sm text-bad">{error}</p>}
      <div className="divide-y divide-line">
        {vehicles.map((v) => (
          <div key={v.id} className="flex items-center gap-3 px-5 py-3 text-sm">
            <span className="flex-1">{v.label}</span>
            <Badge tone="neutral">on file</Badge>
            <button
              onClick={() => remove(v.id)}
              disabled={busy}
              className="text-ink-3 disabled:opacity-50"
            >
              Remove
            </button>
          </div>
        ))}
        {vehicles.length === 0 && (
          <div className="px-5 py-3 text-sm text-ink-3">
            No vehicles on file.
          </div>
        )}
      </div>
      <form
        onSubmit={add}
        className="flex flex-wrap items-end gap-3 border-t border-line bg-surface-2 px-5 py-4 text-sm"
      >
        <label>
          <span className="mb-1 block text-ink-3">Make</span>
          <input
            value={make}
            onChange={(e) => setMake(e.target.value)}
            className="w-32 rounded-lg border border-line bg-surface px-3 py-2"
          />
        </label>
        <label>
          <span className="mb-1 block text-ink-3">Model</span>
          <input
            value={model}
            onChange={(e) => setModel(e.target.value)}
            className="w-32 rounded-lg border border-line bg-surface px-3 py-2"
          />
        </label>
        <label>
          <span className="mb-1 block text-ink-3">Year</span>
          <input
            value={year}
            onChange={(e) => setYear(e.target.value)}
            className="w-20 rounded-lg border border-line bg-surface px-3 py-2"
          />
        </label>
        <label>
          <span className="mb-1 block text-ink-3">Plate</span>
          <input
            value={plate}
            onChange={(e) => setPlate(e.target.value)}
            className="w-28 rounded-lg border border-line bg-surface px-3 py-2"
          />
        </label>
        <button
          type="submit"
          disabled={busy}
          className="rounded-lg bg-accent px-4 py-2 font-semibold text-on-accent disabled:opacity-50"
        >
          Add vehicle
        </button>
      </form>
    </Card>
  );
}
