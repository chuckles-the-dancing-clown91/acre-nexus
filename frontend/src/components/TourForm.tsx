"use client";

// "Request a tour" on a listing: contact details, preferred times, consent.
// The hidden `website` field is a honeypot for bots.

import { useState } from "react";
import { tours } from "@/lib/tours";
import { Button } from "@/components/ui";

const field =
  "w-full rounded-xl border border-line bg-surface-2 px-3 py-2 text-sm outline-none focus:border-accent";

export function TourForm({ listingId }: { listingId: string }) {
  const [f, setF] = useState({
    name: "",
    email: "",
    phone: "",
    preferred_times: "",
    message: "",
    consent: false,
    website: "",
  });
  const [state, setState] = useState<"idle" | "busy" | "sent">("idle");
  const [error, setError] = useState<string | null>(null);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setState("busy");
    setError(null);
    try {
      await tours.request({
        listing_id: listingId,
        name: f.name,
        email: f.email,
        phone: f.phone || undefined,
        preferred_times: f.preferred_times || undefined,
        message: f.message || undefined,
        consent: f.consent,
        website: f.website || undefined,
      });
      setState("sent");
    } catch (err) {
      setError((err as Error).message);
      setState("idle");
    }
  };

  if (state === "sent")
    return (
      <p className="rounded-xl bg-good-soft px-4 py-3 text-sm text-good">
        Thanks. We will be in touch to set up a time.
      </p>
    );

  return (
    <form onSubmit={submit} className="space-y-2.5">
      <h3 className="font-display text-lg font-bold">Request a tour</h3>
      <input
        className={field}
        placeholder="Your name"
        required
        value={f.name}
        onChange={(e) => setF({ ...f, name: e.target.value })}
      />
      <input
        className={field}
        type="email"
        placeholder="Email"
        required
        value={f.email}
        onChange={(e) => setF({ ...f, email: e.target.value })}
      />
      <input
        className={field}
        placeholder="Phone (optional)"
        value={f.phone}
        onChange={(e) => setF({ ...f, phone: e.target.value })}
      />
      <input
        className={field}
        placeholder="Best times, e.g. Saturday morning"
        value={f.preferred_times}
        onChange={(e) => setF({ ...f, preferred_times: e.target.value })}
      />
      <textarea
        className={field}
        rows={2}
        placeholder="Anything we should know?"
        value={f.message}
        onChange={(e) => setF({ ...f, message: e.target.value })}
      />
      <input
        tabIndex={-1}
        autoComplete="off"
        aria-hidden="true"
        className="absolute -left-[9999px] h-0 w-0 opacity-0"
        value={f.website}
        onChange={(e) => setF({ ...f, website: e.target.value })}
      />
      <label className="flex items-start gap-2 text-xs text-ink-3">
        <input
          type="checkbox"
          checked={f.consent}
          onChange={(e) => setF({ ...f, consent: e.target.checked })}
        />
        I agree to be contacted about this home by email, phone or text.
      </label>
      {error && <p className="text-sm text-bad">{error}</p>}
      <Button
        type="submit"
        variant="outline"
        disabled={state === "busy" || !f.consent}
      >
        Request a tour
      </Button>
    </form>
  );
}
