"use client";

// The lead dialogs: enter a new lead, book a tour, and convert a lead into a
// rental application.

import { useState } from "react";
import type { Lead } from "@/lib/api";
import { useConvertLead, useCreateLead, useScheduleTour } from "@/lib/queries";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/menu";
import { F, humanize, inputClass, toCents } from "../leases/_ui/shared";

const SOURCES = ["manual", "website", "referral", "walk_in"];

function Footer({
  onCancel,
  busy,
  disabled,
  children,
}: {
  onCancel: () => void;
  busy: boolean;
  disabled?: boolean;
  children: React.ReactNode;
}) {
  return (
    <div className="mt-5 flex justify-end gap-2">
      <Button type="button" variant="ghost" onClick={onCancel}>
        Cancel
      </Button>
      <Button type="submit" loading={busy} disabled={disabled}>
        {children}
      </Button>
    </div>
  );
}

export function NewLeadDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (o: boolean) => void;
}) {
  const create = useCreateLead();
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [phone, setPhone] = useState("");
  const [source, setSource] = useState("manual");
  const [notes, setNotes] = useState("");

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogTitle className="text-[17px] font-semibold">
          New lead
        </DialogTitle>
        <DialogDescription className="mt-1 text-[13px] text-fg-3">
          Someone who reached you another way: walk-in, phone or referral.
        </DialogDescription>
        <form
          className="mt-4"
          onSubmit={(e) => {
            e.preventDefault();
            create.mutate(
              {
                name: name.trim(),
                email: email.trim(),
                phone: phone.trim() || undefined,
                source,
                notes: notes.trim() || undefined,
              },
              {
                onSuccess: () => {
                  setName("");
                  setEmail("");
                  setPhone("");
                  setSource("manual");
                  setNotes("");
                  onOpenChange(false);
                },
              }
            );
          }}
        >
          <div className="grid gap-3 sm:grid-cols-2">
            <F label="Name" className="sm:col-span-2">
              <input
                value={name}
                onChange={(e) => setName(e.target.value)}
                className={inputClass}
                required
              />
            </F>
            <F label="Email" className="sm:col-span-2">
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
            <F label="Source">
              <select
                value={source}
                onChange={(e) => setSource(e.target.value)}
                className={inputClass}
              >
                {SOURCES.map((s) => (
                  <option key={s} value={s}>
                    {humanize(s)}
                  </option>
                ))}
              </select>
            </F>
            <F label="Notes" className="sm:col-span-2">
              <textarea
                value={notes}
                onChange={(e) => setNotes(e.target.value)}
                rows={2}
                className={inputClass}
              />
            </F>
          </div>
          <Footer onCancel={() => onOpenChange(false)} busy={create.isPending}>
            Create lead
          </Footer>
        </form>
      </DialogContent>
    </Dialog>
  );
}

export function TourDialog({
  lead,
  onClose,
}: {
  lead: Lead | null;
  onClose: () => void;
}) {
  return (
    <Dialog open={!!lead} onOpenChange={(o) => !o && onClose()}>
      <DialogContent>
        {lead && <TourForm key={lead.id} lead={lead} onClose={onClose} />}
      </DialogContent>
    </Dialog>
  );
}

function TourForm({ lead, onClose }: { lead: Lead; onClose: () => void }) {
  const tour = useScheduleTour();
  const [date, setDate] = useState("");
  const [notes, setNotes] = useState("");
  return (
    <>
      <DialogTitle className="text-[17px] font-semibold">
        Book a tour
      </DialogTitle>
      <DialogDescription className="mt-1 text-[13px] text-fg-3">
        Puts a showing for {lead.name} on the calendar and moves the lead along.
        Staff get a reminder before the day.
      </DialogDescription>
      <form
        className="mt-4 space-y-3"
        onSubmit={(e) => {
          e.preventDefault();
          if (!date) return;
          tour.mutate(
            { id: lead.id, body: { date, notes: notes.trim() || undefined } },
            { onSuccess: onClose }
          );
        }}
      >
        <F label="Tour date">
          <input
            type="date"
            value={date}
            onChange={(e) => setDate(e.target.value)}
            className={inputClass}
            required
          />
        </F>
        <F label="Notes">
          <textarea
            value={notes}
            onChange={(e) => setNotes(e.target.value)}
            rows={2}
            placeholder="Which unit, where to meet"
            className={inputClass}
          />
        </F>
        <Footer onCancel={onClose} busy={tour.isPending} disabled={!date}>
          Book tour
        </Footer>
      </form>
    </>
  );
}

export function ConvertDialog({
  lead,
  onClose,
}: {
  lead: Lead | null;
  onClose: () => void;
}) {
  return (
    <Dialog open={!!lead} onOpenChange={(o) => !o && onClose()}>
      <DialogContent>
        {lead && <ConvertForm key={lead.id} lead={lead} onClose={onClose} />}
      </DialogContent>
    </Dialog>
  );
}

function ConvertForm({ lead, onClose }: { lead: Lead; onClose: () => void }) {
  const convert = useConvertLead();
  const [moveIn, setMoveIn] = useState("");
  const [income, setIncome] = useState("");
  const [consent, setConsent] = useState(true);
  return (
    <>
      <DialogTitle className="text-[17px] font-semibold">
        Convert to application
      </DialogTitle>
      <DialogDescription className="mt-1 text-[13px] text-fg-3">
        Starts a rental application for {lead.name} ({lead.email}). It goes to
        screening like any other, and the lead is marked applied.
      </DialogDescription>
      <form
        className="mt-4 space-y-3"
        onSubmit={(e) => {
          e.preventDefault();
          convert.mutate(
            {
              id: lead.id,
              body: {
                move_in: moveIn || undefined,
                annual_income_cents: income
                  ? (toCents(income) ?? undefined)
                  : undefined,
                screening_consent: consent,
              },
            },
            { onSuccess: onClose }
          );
        }}
      >
        <div className="grid gap-3 sm:grid-cols-2">
          <F label="Move-in">
            <input
              type="date"
              value={moveIn}
              onChange={(e) => setMoveIn(e.target.value)}
              className={inputClass}
            />
          </F>
          <F label="Income ($ a year)">
            <input
              type="number"
              min="0"
              step="1000"
              value={income}
              onChange={(e) => setIncome(e.target.value)}
              className={inputClass}
            />
          </F>
        </div>
        <label className="flex items-start gap-2 text-[13px] text-fg-2">
          <input
            type="checkbox"
            checked={consent}
            onChange={(e) => setConsent(e.target.checked)}
            className="mt-0.5"
          />
          <span>
            The applicant authorized a consumer report (credit, criminal,
            eviction) under FCRA §604(b). Needed to screen.
          </span>
        </label>
        <Footer onCancel={onClose} busy={convert.isPending} disabled={!consent}>
          Convert to application
        </Footer>
      </form>
    </>
  );
}
