"use client";

// The two forms on the applications page: back-office intake (an application
// taken by phone or walk-in) and turning an approved application into a lease.

import { useState } from "react";
import { useRouter } from "next/navigation";
import { useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { api } from "@/lib/api";
import type { Application, Property } from "@/lib/types";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/menu";
import { errorText, F, inputClass, toCents } from "../leases/_ui/shared";

export function IntakeDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (o: boolean) => void;
}) {
  const qc = useQueryClient();
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [phone, setPhone] = useState("");
  const [income, setIncome] = useState("");
  const [moveIn, setMoveIn] = useState("");
  const [hasPet, setHasPet] = useState(false);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  async function submit() {
    if (!name.trim() || !email.includes("@")) {
      setErr("Enter a name and a valid email.");
      return;
    }
    setBusy(true);
    setErr(null);
    try {
      await api.createApplication({
        applicant_name: name.trim(),
        email: email.trim(),
        phone: phone.trim() || undefined,
        annual_income_cents: toCents(income) ?? undefined,
        move_in: moveIn || undefined,
        has_pet: hasPet,
      });
      toast.success("Application submitted. Screening has started.");
      void qc.invalidateQueries({ queryKey: ["applications"] });
      setName("");
      setEmail("");
      setPhone("");
      setIncome("");
      setMoveIn("");
      setHasPet(false);
      onOpenChange(false);
    } catch (e) {
      setErr(errorText(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[90dvh] overflow-y-auto">
        <DialogTitle className="text-[17px] font-semibold">
          New application
        </DialogTitle>
        <DialogDescription className="mt-1 text-[13px] text-fg-3">
          For an application taken by phone or in person. It joins the same
          pipeline as the website: the applicant gets a confirmation email and
          screening starts.
        </DialogDescription>
        <form
          className="mt-4"
          onSubmit={(e) => {
            e.preventDefault();
            void submit();
          }}
        >
          <div className="grid gap-3 sm:grid-cols-2">
            <F label="Applicant name" className="sm:col-span-2">
              <input
                value={name}
                onChange={(e) => setName(e.target.value)}
                className={inputClass}
                required
              />
            </F>
            <F label="Email">
              <input
                type="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                className={inputClass}
                required
              />
            </F>
            <F label="Phone">
              <input
                value={phone}
                onChange={(e) => setPhone(e.target.value)}
                className={inputClass}
              />
            </F>
            <F label="Income ($ a year)">
              <input
                value={income}
                onChange={(e) => setIncome(e.target.value)}
                inputMode="decimal"
                className={inputClass}
              />
            </F>
            <F label="Move-in">
              <input
                type="date"
                value={moveIn}
                onChange={(e) => setMoveIn(e.target.value)}
                className={inputClass}
              />
            </F>
            <label className="flex items-center gap-2 text-[13px] text-fg-2 sm:col-span-2">
              <input
                type="checkbox"
                checked={hasPet}
                onChange={(e) => setHasPet(e.target.checked)}
              />
              Has a pet
            </label>
          </div>
          {err && <p className="mt-3 text-[13px] text-bad">{err}</p>}
          <div className="mt-5 flex justify-end gap-2">
            <Button
              type="button"
              variant="ghost"
              onClick={() => onOpenChange(false)}
            >
              Cancel
            </Button>
            <Button type="submit" loading={busy}>
              Submit and screen
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}

export function ConvertDialog({
  app,
  properties,
  onOpenChange,
}: {
  app: Application | null;
  properties: Property[];
  onOpenChange: (o: boolean) => void;
}) {
  return (
    <Dialog open={!!app} onOpenChange={onOpenChange}>
      <DialogContent>
        {app && (
          <ConvertForm
            key={app.id}
            app={app}
            properties={properties}
            onCancel={() => onOpenChange(false)}
          />
        )}
      </DialogContent>
    </Dialog>
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
  const qc = useQueryClient();
  const [propertyId, setPropertyId] = useState(properties[0]?.id ?? "");
  const [rent, setRent] = useState("");
  const [deposit, setDeposit] = useState("");
  const [startDate, setStartDate] = useState(app.move_in ?? "");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  async function submit() {
    const rentCents = toCents(rent);
    if (!propertyId || rentCents === null) {
      setErr("Pick a property and enter the rent.");
      return;
    }
    setBusy(true);
    setErr(null);
    try {
      const lease = await api.convertApplication(app.id, {
        property_id: propertyId,
        rent_cents: rentCents,
        deposit_cents: toCents(deposit) ?? undefined,
        start_date: startDate || undefined,
      });
      void qc.invalidateQueries({ queryKey: ["applications"] });
      void qc.invalidateQueries({ queryKey: ["leases"] });
      toast.success("Lease created");
      router.push(`/console/leases/${lease.id}`);
    } catch (e) {
      setErr(errorText(e));
      setBusy(false);
    }
  }

  return (
    <>
      <DialogTitle className="text-[17px] font-semibold">
        Create lease
      </DialogTitle>
      <DialogDescription className="mt-1 text-[13px] text-fg-3">
        Turns {app.applicant_name}&apos;s approved application into a lease and
        opens it.
      </DialogDescription>
      <form
        className="mt-4"
        onSubmit={(e) => {
          e.preventDefault();
          void submit();
        }}
      >
        <div className="grid gap-3 sm:grid-cols-2">
          <F label="Property" className="sm:col-span-2">
            <select
              value={propertyId}
              onChange={(e) => setPropertyId(e.target.value)}
              className={inputClass}
              required
            >
              {properties.length === 0 && (
                <option value="">No properties</option>
              )}
              {properties.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                </option>
              ))}
            </select>
          </F>
          <F label="Rent ($ a month)">
            <input
              value={rent}
              onChange={(e) => setRent(e.target.value)}
              inputMode="decimal"
              className={inputClass}
              required
            />
          </F>
          <F label="Deposit ($)">
            <input
              value={deposit}
              onChange={(e) => setDeposit(e.target.value)}
              inputMode="decimal"
              className={inputClass}
            />
          </F>
          <F label="Start date">
            <input
              type="date"
              value={startDate}
              onChange={(e) => setStartDate(e.target.value)}
              className={inputClass}
            />
          </F>
        </div>
        {err && <p className="mt-3 text-[13px] text-bad">{err}</p>}
        <div className="mt-5 flex justify-end gap-2">
          <Button type="button" variant="ghost" onClick={onCancel}>
            Cancel
          </Button>
          <Button type="submit" loading={busy}>
            Create and open lease
          </Button>
        </div>
      </form>
    </>
  );
}
