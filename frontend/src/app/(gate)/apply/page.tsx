"use client";

// The public application a prospect opens from the landlord's invitation.
// Their details are on the link, so it's a few taps on a phone.

import { Suspense, useState } from "react";
import { useSearchParams } from "next/navigation";
import { CheckCircle2 } from "lucide-react";
import { toast } from "sonner";
import { api, DEFAULT_TENANT } from "@/lib/api";
import { browserLanguage, type Language } from "@/lib/language";
import { dollarsToCents } from "@/lib/showings";
import { Button } from "@/components/ui/button";
import { Panel } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

const field =
  "rounded-xl border border-line bg-surface px-3 py-2 text-[14px] text-fg outline-none focus:border-accent";

export default function ApplyPage() {
  return (
    <Suspense fallback={null}>
      <ApplyForm />
    </Suspense>
  );
}

function ApplyForm() {
  const q = useSearchParams();
  const tenant = q.get("tenant") ?? DEFAULT_TENANT;
  const lead = q.get("lead");
  const listing = q.get("listing");
  const [name, setName] = useState(q.get("name") ?? "");
  const [email, setEmail] = useState(q.get("email") ?? "");
  const [phone, setPhone] = useState(q.get("phone") ?? "");
  const [moveIn, setMoveIn] = useState("");
  const [income, setIncome] = useState("");
  const [pet, setPet] = useState(false);
  const [petDetails, setPetDetails] = useState("");
  const [military, setMilitary] = useState(false);
  const [consent, setConsent] = useState(false);
  const [lang, setLang] = useState<Language>(() =>
    q.get("lang") === "es" ? "es" : browserLanguage()
  );
  const [busy, setBusy] = useState(false);
  const [done, setDone] = useState<string | null>(null);
  const incomeCents = dollarsToCents(income);
  const ready = name.trim() && email.includes("@") && consent;

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    if (!ready) return;
    setBusy(true);
    try {
      const r = await api.apply(
        {
          listing_id: listing || undefined,
          lead_id: lead || undefined,
          applicant_name: name.trim(),
          email: email.trim(),
          phone: phone.trim() || undefined,
          move_in: moveIn || undefined,
          annual_income_cents: incomeCents ? incomeCents * 12 : undefined,
          has_pet: pet,
          pet_details: pet ? petDetails.trim() || undefined : undefined,
          is_military: military,
          screening_consent: consent,
          language: lang,
        },
        tenant
      );
      setDone(r.message);
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Couldn't send it");
    } finally {
      setBusy(false);
    }
  }

  return (
    <main className="mx-auto min-h-dvh max-w-lg px-4 py-8">
      <Panel className="p-6">
        <div className="eyebrow">Rental application</div>
        <h1 className="mt-1 text-[22px] leading-tight font-semibold text-fg">
          {done ? "Thanks, it's in" : "Apply"}
        </h1>
        {done ? (
          <div className="mt-5 flex items-start gap-3 rounded-xl border border-good/30 bg-good/10 p-4">
            <CheckCircle2 className="mt-0.5 size-5 text-good" />
            <p className="text-[14px] text-fg">{done}</p>
          </div>
        ) : (
          <form className="mt-4 space-y-3" onSubmit={submit}>
            <input
              className={cn(field, "w-full")}
              placeholder="Full name"
              aria-label="Full name"
              autoComplete="name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              required
            />
            <input
              className={cn(field, "w-full")}
              placeholder="Email"
              aria-label="Email"
              type="email"
              autoComplete="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              required
            />
            <input
              className={cn(field, "w-full")}
              placeholder="Phone"
              aria-label="Phone"
              type="tel"
              autoComplete="tel"
              value={phone}
              onChange={(e) => setPhone(e.target.value)}
            />
            <label className="block text-[12px] text-fg-3">
              We&apos;ll write to you in
              <select
                className={cn(field, "mt-1 w-full")}
                value={lang}
                onChange={(e) => setLang(e.target.value as Language)}
              >
                <option value="en">English</option>
                <option value="es">Español</option>
              </select>
            </label>
            <div className="grid grid-cols-2 gap-2">
              <label className="text-[12px] text-fg-3">
                Move-in date
                <input
                  className={cn(field, "mt-1 w-full")}
                  type="date"
                  aria-label="Move-in date"
                  value={moveIn}
                  onChange={(e) => setMoveIn(e.target.value)}
                />
              </label>
              <label className="text-[12px] text-fg-3">
                Monthly income
                <input
                  className={cn(field, "mt-1 w-full")}
                  inputMode="decimal"
                  placeholder="$"
                  aria-label="Monthly income"
                  value={income}
                  onChange={(e) => setIncome(e.target.value)}
                />
              </label>
            </div>
            <label className="flex items-center gap-2 text-[13px] text-fg-2">
              <input
                type="checkbox"
                checked={pet}
                onChange={(e) => setPet(e.target.checked)}
                className="size-4 accent-[var(--accent)]"
              />
              I have a pet
            </label>
            {pet && (
              <input
                className={cn(field, "w-full")}
                placeholder="What kind, how big"
                aria-label="Pet details"
                value={petDetails}
                onChange={(e) => setPetDetails(e.target.value)}
              />
            )}
            <label className="flex items-center gap-2 text-[13px] text-fg-2">
              <input
                type="checkbox"
                checked={military}
                onChange={(e) => setMilitary(e.target.checked)}
                className="size-4 accent-[var(--accent)]"
              />
              Active-duty military
            </label>
            <label className="flex items-start gap-2 rounded-xl bg-fill/60 p-3 text-[12px] text-fg-2">
              <input
                type="checkbox"
                checked={consent}
                onChange={(e) => setConsent(e.target.checked)}
                className="mt-0.5 size-4 shrink-0 accent-[var(--accent)]"
              />
              I authorize a consumer report (credit, criminal and eviction
              history) for this application, as the Fair Credit Reporting Act
              allows.
            </label>
            <Button type="submit" className="w-full" disabled={!ready || busy}>
              {busy ? "Sending…" : "Send my application"}
            </Button>
          </form>
        )}
      </Panel>
    </main>
  );
}
