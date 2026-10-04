"use client";

// Staff profiles: everyone in the workspace who could work shifts, whether
// their employee profile is set up, hours this week, and (with
// `payroll:read`) pay and bill rates. A profile opens in a dialog to edit.

import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { ChevronRight, IdCard, Search, UserCog } from "lucide-react";
import { toast } from "sonner";
import {
  EMPLOYMENT_LABELS,
  hm,
  money,
  team,
  toCents,
  type Employee,
  type EmploymentType,
  type ProfileInput,
} from "@/lib/backoffice";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Field, Input, fieldClass } from "@/components/ui/input";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/menu";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

function dollars(cents: number | null | undefined) {
  return cents == null ? "" : (cents / 100).toFixed(2);
}

export function People({
  roster,
  loading,
  error,
  manage,
  seePay,
}: {
  roster: Employee[] | undefined;
  loading: boolean;
  error: Error | null;
  manage: boolean;
  seePay: boolean;
}) {
  const [editing, setEditing] = useState<Employee | null>(null);
  const [q, setQ] = useState("");
  const needle = q.trim().toLowerCase();
  const rows = [...(roster ?? [])]
    .filter(
      (p) =>
        !needle ||
        p.name.toLowerCase().includes(needle) ||
        p.email.toLowerCase().includes(needle) ||
        (p.profile?.title ?? "").toLowerCase().includes(needle)
    )
    .sort(
      (a, b) =>
        Number(!a.profile) - Number(!b.profile) ||
        Number(!(a.profile?.current ?? true)) -
          Number(!(b.profile?.current ?? true)) ||
        a.name.localeCompare(b.name)
    );

  return (
    <Panel className="overflow-hidden">
      <div className="flex flex-col gap-3 border-b border-line p-3 sm:flex-row sm:items-center sm:justify-between">
        <div className="px-1 text-xs text-fg-3">
          {manage
            ? "Open someone to edit their profile. People without one can't clock in."
            : "Profiles are edited by people with team:manage."}
        </div>
        <div className="relative sm:w-64">
          <Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-fg-3" />
          <Input
            value={q}
            onChange={(e) => setQ(e.target.value)}
            placeholder="Search people"
            aria-label="Search people"
            className="h-10 pl-9"
          />
        </div>
      </div>
      {loading && (
        <div className="space-y-2 p-3">
          {Array.from({ length: 5 }, (_, i) => (
            <Skeleton key={i} className="h-14" />
          ))}
        </div>
      )}
      {error && (
        <p className="p-4 text-[13px] text-bad">
          Couldn&apos;t load the team: {error.message}
        </p>
      )}
      {roster && rows.length === 0 && (
        <EmptyState
          icon={<IdCard />}
          title={needle ? "Nobody matches that" : "No staff yet"}
          description={
            needle ? undefined : "Invite people from the Members tab first."
          }
        />
      )}
      <ul className="divide-y divide-line">
        {rows.map((p) => {
          const clickable = manage;
          const body = (
            <>
              <span
                aria-hidden
                className="size-3 shrink-0 rounded-full"
                style={{
                  backgroundColor:
                    p.profile?.calendar_color ?? "var(--line-strong)",
                }}
              />
              <div className="min-w-0 flex-1">
                <div className="flex flex-wrap items-center gap-2">
                  <span className="truncate text-[14px] font-medium text-fg">
                    {p.name}
                  </span>
                  {p.clocked_in && (
                    <Badge tone="good" dot>
                      on the clock
                    </Badge>
                  )}
                  {!p.profile && <Badge tone="warn">not set up</Badge>}
                  {p.profile && !p.profile.current && <Badge>ended</Badge>}
                </div>
                <div className="truncate text-xs text-fg-3">
                  {[
                    p.profile?.title,
                    p.profile
                      ? EMPLOYMENT_LABELS[p.profile.employment_type]
                      : null,
                    p.email,
                  ]
                    .filter(Boolean)
                    .join(" · ")}
                </div>
              </div>
              <div className="hidden text-right sm:block">
                <div className="figure text-[13px] font-medium text-fg">
                  {p.profile ? hm(p.week_minutes) : "-"}
                </div>
                <div className="text-[11px] text-fg-4">this week</div>
              </div>
              {seePay && (
                <div className="hidden w-24 text-right md:block">
                  <div className="figure text-[13px] text-fg-2">
                    {p.profile?.pay_rate_cents != null
                      ? money(p.profile.pay_rate_cents)
                      : "-"}
                  </div>
                  <div className="text-[11px] text-fg-4">pay / h</div>
                </div>
              )}
              {seePay && (
                <div className="hidden w-24 text-right md:block">
                  <div className="figure text-[13px] text-fg-2">
                    {p.profile?.bill_rate_cents != null
                      ? money(p.profile.bill_rate_cents)
                      : "-"}
                  </div>
                  <div className="text-[11px] text-fg-4">bill / h</div>
                </div>
              )}
              {clickable &&
                (p.profile ? (
                  <ChevronRight className="size-4 shrink-0 text-fg-4" />
                ) : (
                  <span className="inline-flex h-8 shrink-0 items-center gap-1.5 rounded-xl border border-line-strong bg-fill px-3 text-xs font-medium text-fg">
                    <UserCog className="size-3.5" />
                    Set up
                  </span>
                ))}
            </>
          );
          return (
            <li key={p.user_id}>
              {clickable ? (
                <button
                  type="button"
                  onClick={() => setEditing(p)}
                  className="flex w-full items-center gap-3 px-4 py-3 text-left transition hover:bg-fill-2"
                >
                  {body}
                </button>
              ) : (
                <div className="flex items-center gap-3 px-4 py-3">{body}</div>
              )}
            </li>
          );
        })}
      </ul>

      {editing && (
        <ProfileDialog
          key={editing.user_id}
          person={editing}
          seePay={seePay}
          onClose={() => setEditing(null)}
        />
      )}
    </Panel>
  );
}

function ProfileDialog({
  person,
  seePay,
  onClose,
}: {
  person: Employee;
  seePay: boolean;
  onClose: () => void;
}) {
  const qc = useQueryClient();
  const p = person.profile;
  const [title, setTitle] = useState(p?.title ?? "");
  const [employment, setEmployment] = useState<EmploymentType>(
    p?.employment_type ?? "full_time"
  );
  const [pay, setPay] = useState(dollars(p?.pay_rate_cents));
  const [bill, setBill] = useState(dollars(p?.bill_rate_cents));
  const [hire, setHire] = useState(p?.hire_date ?? "");
  const [endDate, setEndDate] = useState(p?.end_date ?? "");
  const [target, setTarget] = useState(String(p?.weekly_hours_target ?? 40));
  const [vehicle, setVehicle] = useState<"company" | "personal">(
    p?.default_vehicle ?? "company"
  );
  const [mileage, setMileage] = useState(p?.mileage_reimbursed ?? false);
  const [ecName, setEcName] = useState(p?.emergency_contact_name ?? "");
  const [ecPhone, setEcPhone] = useState(p?.emergency_contact_phone ?? "");
  const [color, setColor] = useState(p?.calendar_color ?? "#3b82f6");
  const [notes, setNotes] = useState(p?.notes ?? "");
  const [busy, setBusy] = useState(false);

  async function save() {
    const body: ProfileInput = {
      title: title.trim() || null,
      employment_type: employment,
      hire_date: hire || null,
      end_date: endDate || null,
      weekly_hours_target: Number(target) || 0,
      default_vehicle: vehicle,
      mileage_reimbursed: mileage,
      emergency_contact_name: ecName.trim() || null,
      emergency_contact_phone: ecPhone.trim() || null,
      calendar_color: color,
      notes: notes.trim() || null,
    };
    if (seePay) {
      body.pay_rate_cents = toCents(pay);
      body.bill_rate_cents = toCents(bill);
    }
    setBusy(true);
    try {
      await team.saveProfile(person.user_id, body);
      toast.success(p ? `Saved ${person.name}` : `${person.name} is set up`);
      await qc.invalidateQueries({ queryKey: ["team"] });
      onClose();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't save the profile");
      setBusy(false);
    }
  }

  const select = cn(fieldClass, "h-11 w-full");

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-h-[90dvh] max-w-2xl overflow-y-auto">
        <DialogTitle className="text-[17px] font-semibold">
          {person.name}
        </DialogTitle>
        <DialogDescription className="mt-1 text-[13px] text-fg-3">
          {p
            ? person.email
            : "Set up their employee profile so they can clock in."}
        </DialogDescription>
        <form
          className="mt-4"
          onSubmit={(e) => {
            e.preventDefault();
            void save();
          }}
        >
          <div className="grid gap-4 sm:grid-cols-2">
            <Field label="Title">
              {(f) => (
                <Input
                  {...f}
                  placeholder="e.g. Maintenance technician"
                  value={title}
                  onChange={(e) => setTitle(e.target.value)}
                />
              )}
            </Field>
            <Field label="Employment type">
              {(f) => (
                <select
                  {...f}
                  className={select}
                  value={employment}
                  onChange={(e) =>
                    setEmployment(e.target.value as EmploymentType)
                  }
                >
                  {(Object.keys(EMPLOYMENT_LABELS) as EmploymentType[]).map(
                    (k) => (
                      <option key={k} value={k}>
                        {EMPLOYMENT_LABELS[k]}
                      </option>
                    )
                  )}
                </select>
              )}
            </Field>
            {seePay && (
              <>
                <Field label="Pay rate ($ per hour)">
                  {(f) => (
                    <Input
                      {...f}
                      inputMode="decimal"
                      placeholder="0.00"
                      value={pay}
                      onChange={(e) => setPay(e.target.value)}
                    />
                  )}
                </Field>
                <Field
                  label="Bill rate ($ per hour)"
                  hint="What an hour of their work is charged to owners."
                >
                  {(f) => (
                    <Input
                      {...f}
                      inputMode="decimal"
                      placeholder="0.00"
                      value={bill}
                      onChange={(e) => setBill(e.target.value)}
                    />
                  )}
                </Field>
              </>
            )}
            <Field label="Hire date">
              {(f) => (
                <Input
                  {...f}
                  type="date"
                  value={hire}
                  onChange={(e) => setHire(e.target.value)}
                />
              )}
            </Field>
            <Field label="End date" hint="Leave empty while they work here.">
              {(f) => (
                <Input
                  {...f}
                  type="date"
                  value={endDate}
                  onChange={(e) => setEndDate(e.target.value)}
                />
              )}
            </Field>
            <Field label="Weekly hours target">
              {(f) => (
                <Input
                  {...f}
                  type="number"
                  min={0}
                  value={target}
                  onChange={(e) => setTarget(e.target.value)}
                />
              )}
            </Field>
            <Field label="Usually drives">
              {(f) => (
                <select
                  {...f}
                  className={select}
                  value={vehicle}
                  onChange={(e) =>
                    setVehicle(e.target.value as "company" | "personal")
                  }
                >
                  <option value="company">A company vehicle</option>
                  <option value="personal">Their own vehicle</option>
                </select>
              )}
            </Field>
            <label className="flex items-center gap-2 text-[13px] text-fg-2 sm:col-span-2">
              <input
                type="checkbox"
                checked={mileage}
                onChange={(e) => setMileage(e.target.checked)}
                className="size-4 accent-[var(--accent)]"
              />
              Pay back miles driven in their own vehicle
            </label>
            <Field label="Emergency contact">
              {(f) => (
                <Input
                  {...f}
                  placeholder="Name"
                  value={ecName}
                  onChange={(e) => setEcName(e.target.value)}
                />
              )}
            </Field>
            <Field label="Emergency phone">
              {(f) => (
                <Input
                  {...f}
                  type="tel"
                  value={ecPhone}
                  onChange={(e) => setEcPhone(e.target.value)}
                />
              )}
            </Field>
            <Field label="Calendar color">
              {(f) => (
                <input
                  {...f}
                  type="color"
                  className="h-11 w-20 cursor-pointer rounded-xl border border-line-strong bg-fill"
                  value={color}
                  onChange={(e) => setColor(e.target.value)}
                />
              )}
            </Field>
            <div className="sm:col-span-2">
              <Field label="Notes">
                {(f) => (
                  <textarea
                    {...f}
                    rows={2}
                    className={cn(fieldClass, "w-full")}
                    value={notes}
                    onChange={(e) => setNotes(e.target.value)}
                  />
                )}
              </Field>
            </div>
          </div>
          <div className="mt-5 flex justify-end gap-2">
            <Button type="button" variant="ghost" onClick={onClose}>
              Cancel
            </Button>
            <Button type="submit" loading={busy}>
              {p ? "Save" : "Set up"}
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}
