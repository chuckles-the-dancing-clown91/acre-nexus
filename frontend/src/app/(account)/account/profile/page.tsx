"use client";

// The resident's profile: the one record applications fill themselves in
// from (contact, work, household, pets, rental history, military, income, ID, vehicles), plus how they sign
// in (password, two-step codes) and push notifications on this device.

import { MyLanguage } from "@/components/language/LanguagePicker";
import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  BellRing,
  Car,
  KeyRound,
  ShieldCheck,
  Trash2,
  UserRound,
} from "lucide-react";
import { toast } from "sonner";
import {
  api,
  ApiError,
  type MyProfileView,
  type ProfileInput,
  type TotpSetupResult,
  type VehicleProfile,
} from "@/lib/api";
import { MIN_PASSWORD_LENGTH } from "@/lib/password";
import {
  currentSubscription,
  disablePush,
  enablePush,
  pushSupported,
} from "@/lib/push";
import { parseIncomeCents } from "@/lib/portal-format";
import { ExtrasForm } from "@/components/resident/ExtrasForm";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";

const field =
  "w-full rounded-xl border border-line bg-surface px-3 py-2.5 text-[14px] text-fg outline-none focus:border-accent";

export default function ProfilePage() {
  const profile = useQuery({
    queryKey: ["my-profile"],
    queryFn: api.myProfile,
    retry: (n, e) => !(e instanceof ApiError && e.status < 500) && n < 2,
  });

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-[24px] font-semibold text-fg">Profile</h1>
        <p className="text-[13px] text-fg-3">
          Keep this current and applications fill themselves in.
        </p>
      </div>

      {profile.isLoading && <Skeleton className="h-64" />}
      {profile.error && (
        <Panel>
          <EmptyState
            icon={<UserRound />}
            title="Couldn't load your profile"
            description={profile.error.message}
          />
        </Panel>
      )}
      {profile.data && (
        <>
          <ProfileForm view={profile.data} />
          <ResidentSections />
          <Vehicles vehicles={profile.data.vehicles} />
        </>
      )}

      <Panel>
        <PanelHeader
          title="Messages in"
          description="Reminders, receipts and updates from us. Mensajes en español."
        />
        <div className="px-5 pb-5">
          <MyLanguage />
        </div>
      </Panel>
      <PasswordPanel />
      <TwoStepPanel />
      <PushPanel />
    </div>
  );
}

function ResidentSections() {
  const qc = useQueryClient();
  const q = useQuery({ queryKey: ["my-resident"], queryFn: api.myResident });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  if (!q.data) return q.isLoading ? <Skeleton className="h-40" /> : null;
  return (
    <ExtrasForm
      value={q.data.extras}
      busy={busy}
      error={error}
      saveLabel="Save household and history"
      onSave={async (x) => {
        setBusy(true);
        setError(null);
        try {
          const v = await api.saveMyResident(x);
          qc.setQueryData(["my-resident"], v);
          qc.invalidateQueries({ queryKey: ["my-profile"] });
          toast.success("Saved.");
        } catch (e) {
          setError(e instanceof Error ? e.message : "Couldn't save");
        } finally {
          setBusy(false);
        }
      }}
    />
  );
}

function Labeled({
  label,
  className,
  children,
}: {
  label: string;
  className?: string;
  children: React.ReactNode;
}) {
  return (
    <label className={className ?? "block"}>
      <span className="mb-1 block text-xs font-medium text-fg-2">{label}</span>
      {children}
    </label>
  );
}

function formFrom(v: MyProfileView): ProfileInput {
  const p = v.profile;
  return {
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
    is_military: p.is_military,
  };
}

/** The editable record. State starts from the loaded profile; the backend
 * merges, so only fields in the payload change. Income stays the raw text
 * typed and is checked on save. SSN and ID number are write-only. */
function ProfileForm({ view }: { view: MyProfileView }) {
  const qc = useQueryClient();
  const [form, setForm] = useState<ProfileInput>(() => formFrom(view));
  const [income, setIncome] = useState(() =>
    view.profile.annual_income_cents != null
      ? String(view.profile.annual_income_cents / 100)
      : ""
  );
  const [ssn, setSsn] = useState("");
  const [govId, setGovId] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const p = view.profile;

  const set = (k: keyof ProfileInput, v: string | boolean | undefined) =>
    setForm((f) => ({ ...f, [k]: v }));

  async function save(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    const body: ProfileInput = { ...form };
    const cents = parseIncomeCents(income);
    if (cents === undefined) {
      setError("Annual income should be a number, like 52000 or 52,000.50.");
      return;
    }
    if (cents !== null) body.annual_income_cents = cents;
    if (ssn.trim()) body.ssn = ssn.trim();
    if (govId.trim()) body.gov_id_number = govId.trim();
    setBusy(true);
    try {
      const v = await api.updateMyProfile(body);
      qc.setQueryData(["my-profile"], v);
      setSsn("");
      setGovId("");
      toast.success("Profile saved.");
    } catch (e) {
      setError(e instanceof Error ? e.message : "Couldn't save your profile");
    } finally {
      setBusy(false);
    }
  }

  const text = (k: keyof ProfileInput) => ({
    className: field,
    value: (form[k] as string | undefined) ?? "",
    onChange: (e: React.ChangeEvent<HTMLInputElement>) =>
      set(k, e.target.value),
  });

  return (
    <form onSubmit={save} className="space-y-6">
      <section>
        <div className="eyebrow mb-2">Contact</div>
        <Panel className="space-y-4 p-4">
          <p className="text-[13px] text-fg-3">
            Signed in as{" "}
            <span className="font-medium text-fg">{view.email}</span>
          </p>
          <div className="grid gap-4 sm:grid-cols-2">
            <Labeled label="Legal first name">
              <input {...text("legal_first_name")} autoComplete="given-name" />
            </Labeled>
            <Labeled label="Legal middle name">
              <input
                {...text("legal_middle_name")}
                autoComplete="additional-name"
              />
            </Labeled>
            <Labeled label="Legal last name">
              <input {...text("legal_last_name")} autoComplete="family-name" />
            </Labeled>
            <Labeled label="Preferred name">
              <input {...text("preferred_name")} autoComplete="nickname" />
            </Labeled>
            <Labeled label="Date of birth">
              <input
                type="date"
                className={field}
                value={form.date_of_birth ?? ""}
                onChange={(e) =>
                  set("date_of_birth", e.target.value || undefined)
                }
                autoComplete="bday"
              />
            </Labeled>
            <Labeled label="Mobile phone">
              <input
                {...text("phone")}
                type="tel"
                placeholder="+1 555 555 0100"
                autoComplete="tel"
              />
            </Labeled>
            <Labeled label="Street address" className="block sm:col-span-2">
              <input {...text("address_line1")} autoComplete="address-line1" />
            </Labeled>
            <Labeled
              label="Apt or suite (optional)"
              className="block sm:col-span-2"
            >
              <input {...text("address_line2")} autoComplete="address-line2" />
            </Labeled>
            <Labeled label="City">
              <input {...text("city")} autoComplete="address-level2" />
            </Labeled>
            <Labeled label="State or region">
              <input {...text("region")} autoComplete="address-level1" />
            </Labeled>
            <Labeled label="ZIP or postal code">
              <input {...text("postal_code")} autoComplete="postal-code" />
            </Labeled>
          </div>
        </Panel>
      </section>

      <section>
        <div className="eyebrow mb-2">Rental details</div>
        <Panel className="space-y-4 p-4">
          <Labeled label="Annual income (USD)">
            <input
              className={field}
              value={income}
              onChange={(e) => setIncome(e.target.value)}
              placeholder="52,000"
              inputMode="decimal"
            />
          </Labeled>
          <div className="space-y-2 rounded-xl border border-line bg-fill/40 p-3 text-[13px]">
            <label className="flex items-start gap-2">
              <input
                type="checkbox"
                className="mt-0.5"
                checked={form.is_military ?? false}
                onChange={(e) => set("is_military", e.target.checked)}
              />
              <span className="text-fg">Active military or veteran</span>
            </label>
          </div>
        </Panel>
      </section>

      <section>
        <div className="eyebrow mb-2">Identification</div>
        <Panel className="space-y-4 p-4">
          <p className="text-[13px] text-fg-3">
            Stored encrypted. We only ever show the last four digits.
          </p>
          <div className="grid gap-4 sm:grid-cols-3">
            <Labeled label="ID type">
              <select
                className={field}
                value={form.gov_id_type ?? ""}
                onChange={(e) =>
                  set("gov_id_type", e.target.value || undefined)
                }
              >
                <option value="">Choose…</option>
                <option value="drivers_license">Driver&apos;s license</option>
                <option value="passport">Passport</option>
                <option value="state_id">State ID</option>
              </select>
            </Labeled>
            <Labeled
              label={
                p.gov_id_last4
                  ? `ID number (on file ••••${p.gov_id_last4})`
                  : "ID number"
              }
            >
              <input
                className={field}
                value={govId}
                onChange={(e) => setGovId(e.target.value)}
                placeholder={p.has_gov_id ? "Enter a new one to replace" : ""}
                autoComplete="off"
              />
            </Labeled>
            <Labeled
              label={p.ssn_last4 ? `SSN (on file ••••${p.ssn_last4})` : "SSN"}
            >
              <input
                className={field}
                value={ssn}
                onChange={(e) => setSsn(e.target.value)}
                placeholder={p.has_ssn ? "Enter a new one to replace" : ""}
                inputMode="numeric"
                autoComplete="off"
              />
            </Labeled>
          </div>
        </Panel>
      </section>

      {error && <p className="text-[13px] text-bad">{error}</p>}
      <div className="flex justify-end">
        <Button type="submit" loading={busy}>
          {busy ? "Saving…" : "Save profile"}
        </Button>
      </div>
    </form>
  );
}

function Vehicles({ vehicles }: { vehicles: VehicleProfile[] }) {
  const qc = useQueryClient();
  const [make, setMake] = useState("");
  const [model, setModel] = useState("");
  const [year, setYear] = useState("");
  const [plate, setPlate] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = () => qc.invalidateQueries({ queryKey: ["my-profile"] });

  async function add(e: React.FormEvent) {
    e.preventDefault();
    if (!make.trim() || !model.trim()) {
      setError("Add the make and model.");
      return;
    }
    const y = year.trim() ? parseInt(year, 10) : undefined;
    if (y !== undefined && (Number.isNaN(y) || y < 1900 || y > 2100)) {
      setError("Year should be four digits, like 2019.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await api.addMyVehicle({
        make: make.trim(),
        model: model.trim(),
        year: y,
        license_plate: plate.trim() || undefined,
      });
      setMake("");
      setModel("");
      setYear("");
      setPlate("");
      await refresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Couldn't add the vehicle");
    } finally {
      setBusy(false);
    }
  }

  async function remove(id: string) {
    setBusy(true);
    setError(null);
    try {
      await api.deleteMyVehicle(id);
      await refresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Couldn't remove it");
    } finally {
      setBusy(false);
    }
  }

  return (
    <section>
      <div className="eyebrow mb-2">Vehicles</div>
      <Panel className="divide-y divide-line">
        <p className="px-4 py-3 text-[13px] text-fg-3">
          Sent with every application. Properties use them for parking.
        </p>
        {vehicles.length === 0 && (
          <p className="px-4 py-3 text-[13px] text-fg-3">
            No vehicles on file.
          </p>
        )}
        {vehicles.map((v) => (
          <div key={v.id} className="flex items-center gap-3 px-4 py-3">
            <Car className="size-4 shrink-0 text-fg-3" />
            <span className="min-w-0 flex-1 truncate text-[14px] text-fg">
              {v.label}
            </span>
            <Button
              size="icon"
              variant="ghost"
              disabled={busy}
              onClick={() => void remove(v.id)}
              aria-label={`Remove ${v.label}`}
            >
              <Trash2 />
            </Button>
          </div>
        ))}
        <form onSubmit={add} className="space-y-3 p-4">
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
            <Labeled label="Make">
              <input
                className={field}
                value={make}
                onChange={(e) => setMake(e.target.value)}
              />
            </Labeled>
            <Labeled label="Model">
              <input
                className={field}
                value={model}
                onChange={(e) => setModel(e.target.value)}
              />
            </Labeled>
            <Labeled label="Year">
              <input
                className={field}
                value={year}
                inputMode="numeric"
                maxLength={4}
                onChange={(e) => setYear(e.target.value)}
              />
            </Labeled>
            <Labeled label="Plate">
              <input
                className={field}
                value={plate}
                onChange={(e) => setPlate(e.target.value)}
              />
            </Labeled>
          </div>
          {error && <p className="text-[13px] text-bad">{error}</p>}
          <div className="flex justify-end">
            <Button type="submit" variant="secondary" disabled={busy}>
              Add vehicle
            </Button>
          </div>
        </form>
      </Panel>
    </section>
  );
}

function PasswordPanel() {
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");
  const [confirm, setConfirm] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    if (next.length < MIN_PASSWORD_LENGTH) {
      setError(`Use at least ${MIN_PASSWORD_LENGTH} characters.`);
      return;
    }
    if (next !== confirm) {
      setError("The two new passwords don't match.");
      return;
    }
    setBusy(true);
    try {
      await api.passwordChange(current, next);
      setCurrent("");
      setNext("");
      setConfirm("");
      toast.success("Password changed. Other devices were signed out.");
    } catch (e) {
      setError(e instanceof Error ? e.message : "Couldn't change the password");
    } finally {
      setBusy(false);
    }
  }

  return (
    <Panel>
      <PanelHeader
        title={
          <span className="flex items-center gap-2">
            <KeyRound className="size-4" />
            Password
          </span>
        }
        description="Changing it signs you out everywhere else."
      />
      <form onSubmit={submit} className="space-y-3 px-5 pt-4 pb-5">
        <div className="grid gap-3 sm:grid-cols-3">
          <input
            className={field}
            type="password"
            aria-label="Current password"
            autoComplete="current-password"
            placeholder="Current password"
            value={current}
            onChange={(e) => setCurrent(e.target.value)}
          />
          <input
            className={field}
            type="password"
            aria-label="New password"
            autoComplete="new-password"
            placeholder="New password"
            value={next}
            onChange={(e) => setNext(e.target.value)}
          />
          <input
            className={field}
            type="password"
            aria-label="New password again"
            autoComplete="new-password"
            placeholder="New password again"
            value={confirm}
            onChange={(e) => setConfirm(e.target.value)}
          />
        </div>
        {error && <p className="text-[13px] text-bad">{error}</p>}
        <div className="flex justify-end">
          <Button
            type="submit"
            variant="secondary"
            disabled={busy || !current || !next}
          >
            {busy ? "Changing…" : "Change password"}
          </Button>
        </div>
      </form>
    </Panel>
  );
}

/** Two-step sign-in with an authenticator app (TOTP). */
function TwoStepPanel() {
  const qc = useQueryClient();
  const status = useQuery({ queryKey: ["mfa-status"], queryFn: api.mfaStatus });
  const [setup, setSetup] = useState<TotpSetupResult | null>(null);
  const [turningOff, setTurningOff] = useState(false);
  const [code, setCode] = useState("");
  const [busy, setBusy] = useState(false);
  const on = !!status.data?.enabled;

  async function start() {
    setBusy(true);
    try {
      setSetup(await api.mfaSetup());
      setCode("");
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't start setup");
    } finally {
      setBusy(false);
    }
  }

  async function finish(e: React.FormEvent) {
    e.preventDefault();
    const c = code.replace(/\s/g, "");
    if (!c) return;
    setBusy(true);
    try {
      const s = on ? await api.mfaDisable(c) : await api.mfaConfirm(c);
      qc.setQueryData(["mfa-status"], s);
      setSetup(null);
      setTurningOff(false);
      setCode("");
      toast.success(
        s.enabled ? "Two-step sign-in is on." : "Two-step sign-in is off."
      );
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "That code didn't work");
    } finally {
      setBusy(false);
    }
  }

  const cancel = () => {
    setSetup(null);
    setTurningOff(false);
    setCode("");
  };
  const asking = !!setup || turningOff;

  return (
    <Panel>
      <PanelHeader
        title={
          <span className="flex items-center gap-2">
            <ShieldCheck className="size-4" />
            Two-step sign-in
          </span>
        }
        description="Ask for a code from an authenticator app when you sign in."
        action={
          status.data && (
            <Badge tone={on ? "good" : "neutral"}>{on ? "On" : "Off"}</Badge>
          )
        }
      />
      <div className="px-5 pt-4 pb-5">
        {status.isLoading ? (
          <Skeleton className="h-10 w-48" />
        ) : status.error ? (
          <p className="text-[13px] text-fg-3">
            Couldn&apos;t check two-step sign-in right now.
          </p>
        ) : !asking ? (
          on ? (
            <Button variant="secondary" onClick={() => setTurningOff(true)}>
              Turn off
            </Button>
          ) : (
            <Button variant="secondary" loading={busy} onClick={start}>
              Turn on
            </Button>
          )
        ) : (
          <form onSubmit={finish} className="space-y-3">
            {setup && (
              <div className="space-y-2 rounded-xl border border-line bg-fill/40 p-3 text-[13px]">
                <p className="text-fg-2">
                  Add this account to your authenticator app, then enter the
                  6-digit code it shows.
                </p>
                <a
                  href={setup.otpauth_uri}
                  className="inline-block text-accent hover:underline"
                >
                  Open in authenticator app
                </a>
                <div>
                  <div className="text-xs text-fg-3">
                    Or type this key by hand
                  </div>
                  <code className="block font-mono text-[13px] break-all text-fg select-all">
                    {setup.secret}
                  </code>
                </div>
              </div>
            )}
            {turningOff && (
              <p className="text-[13px] text-fg-2">
                Enter a code from your authenticator app to turn it off.
              </p>
            )}
            <input
              className={`${field} max-w-[12rem] font-mono tracking-widest`}
              aria-label="6-digit code"
              placeholder="123456"
              inputMode="numeric"
              autoComplete="one-time-code"
              maxLength={8}
              value={code}
              onChange={(e) => setCode(e.target.value)}
              autoFocus
            />
            <div className="flex flex-wrap gap-2">
              <Button
                type="submit"
                variant={turningOff ? "danger" : "primary"}
                disabled={busy || !code.trim()}
              >
                {turningOff ? "Turn off" : "Confirm"}
              </Button>
              <Button type="button" variant="ghost" onClick={cancel}>
                Cancel
              </Button>
            </div>
          </form>
        )}
      </div>
    </Panel>
  );
}

/** Web push on this device: on or off, plus a test. */
function PushPanel() {
  const qc = useQueryClient();
  const [busy, setBusy] = useState(false);
  // Runs only in the browser, so the server render never guesses.
  const push = useQuery({
    queryKey: ["push", "this-device"],
    queryFn: async () => {
      const supported = pushSupported();
      const on = supported ? !!(await currentSubscription()) : false;
      return { supported, on };
    },
  });
  const on = !!push.data?.on;

  async function toggle() {
    setBusy(true);
    try {
      if (on) await disablePush();
      else await enablePush();
      qc.setQueryData(["push", "this-device"], { supported: true, on: !on });
      toast.success(
        on ? "Notifications are off here." : "Notifications are on."
      );
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't change that");
    } finally {
      setBusy(false);
    }
  }

  async function test() {
    try {
      await api.testPush();
      toast.success("Test sent. It arrives in a moment.");
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't send a test");
    }
  }

  return (
    <Panel>
      <PanelHeader
        title={
          <span className="flex items-center gap-2">
            <BellRing className="size-4" />
            Notifications on this device
          </span>
        }
        description="Repair updates and replies from the office, even with this page closed."
        action={
          push.data?.supported && (
            <Badge tone={on ? "good" : "neutral"}>{on ? "On" : "Off"}</Badge>
          )
        }
      />
      <div className="px-5 pt-4 pb-5">
        {push.isLoading ? (
          <Skeleton className="h-10 w-48" />
        ) : push.data?.supported ? (
          <div className="flex flex-wrap gap-2">
            <Button
              variant={on ? "secondary" : "primary"}
              loading={busy}
              onClick={toggle}
            >
              {on ? "Turn off" : "Turn on"}
            </Button>
            {on && (
              <Button variant="ghost" onClick={test}>
                Send a test
              </Button>
            )}
          </div>
        ) : (
          <p className="text-[13px] text-fg-3">
            This browser can&apos;t show notifications. On iPhone, add this page
            to your Home Screen first.
          </p>
        )}
      </div>
    </Panel>
  );
}
