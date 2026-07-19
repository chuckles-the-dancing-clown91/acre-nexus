"use client";

// Lease renewals card (issue #44) — the ongoing-tenancy motion. Lives on the
// lease page under the lease document. Propose renewed terms (a rent increase +
// extended term), which generates an addendum; send it out for e-signature
// (resident + landlord); and when every party signs, the new terms are applied
// to the lease automatically. Mirrors EsignCard's load()/run() pattern.

import { useCallback, useEffect, useState } from "react";
import { api, type EsignSignerLink, type Renewal } from "@/lib/api";
import type { LeaseDetail } from "@/lib/types";
import { Badge, Card } from "@/components/ui";
import { logError } from "@/lib/log";

export function RenewalsCard({
  lease,
  manage,
  onChanged,
}: {
  lease: LeaseDetail;
  manage: boolean;
  /** Called after a renewal mutation so the parent can refresh the lease. */
  onChanged: () => void;
}) {
  const [renewals, setRenewals] = useState<Renewal[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [proposing, setProposing] = useState(false);
  // Signing links returned by "send" are shown once, keyed by renewal id.
  const [links, setLinks] = useState<Record<string, EsignSignerLink[]>>({});

  const load = useCallback(() => {
    api
      .leaseRenewals(lease.id)
      .then(setRenewals)
      .catch((e) => {
        setRenewals([]);
        logError("failed to load renewals", e);
      });
  }, [lease.id]);

  useEffect(() => {
    load();
  }, [load]);

  async function run(key: string, fn: () => Promise<void>) {
    setBusy(key);
    setError(null);
    try {
      await fn();
      load();
      onChanged();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(null);
    }
  }

  if (renewals === null) return null;

  const hasOpen = renewals.some((r) =>
    ["proposed", "sent", "signed"].includes(r.status)
  );

  return (
    <Card className="overflow-hidden">
      <div className="flex flex-wrap items-center gap-3 border-b border-line px-5 py-4">
        <div className="flex-1">
          <h2 className="font-display text-lg font-bold">Renewals</h2>
          <p className="text-sm text-ink-3">
            Current rent {lease.rent_label ?? ""} · term ends{" "}
            {lease.end_date ?? "month-to-month"}
          </p>
        </div>
        {manage && !proposing && !hasOpen && (
          <button
            onClick={() => setProposing(true)}
            className="rounded-lg bg-accent px-3 py-1.5 text-sm font-semibold text-on-accent"
          >
            Propose renewal
          </button>
        )}
      </div>

      {error && <p className="px-5 py-3 text-sm text-bad">{error}</p>}

      {proposing && (
        <ProposeForm
          currentRentCents={lease.rent_cents}
          busy={busy === "propose"}
          onCancel={() => setProposing(false)}
          onSubmit={(body) =>
            run("propose", async () => {
              await api.proposeRenewal(lease.id, body);
              setProposing(false);
            })
          }
        />
      )}

      <div className="divide-y divide-line">
        {renewals.map((r) => (
          <RenewalRow
            key={r.id}
            renewal={r}
            manage={manage}
            busy={busy}
            links={links[r.id]}
            onSend={() =>
              run(`send-${r.id}`, async () => {
                const resp = await api.sendRenewal(r.id, {});
                setLinks((m) => ({ ...m, [r.id]: resp.sign_links }));
              })
            }
            onCancel={() =>
              run(`cancel-${r.id}`, async () => {
                await api.cancelRenewal(r.id);
              })
            }
          />
        ))}
        {renewals.length === 0 && !proposing && (
          <div className="px-5 py-6 text-sm text-ink-3">
            No renewals yet.{" "}
            {manage
              ? "Propose one to offer a rent change and extended term the resident can e-sign."
              : "Nothing to renew right now."}
          </div>
        )}
      </div>
    </Card>
  );
}

function RenewalRow({
  renewal: r,
  manage,
  busy,
  links,
  onSend,
  onCancel,
}: {
  renewal: Renewal;
  manage: boolean;
  busy: string | null;
  links?: EsignSignerLink[];
  onSend: () => void;
  onCancel: () => void;
}) {
  const term = r.new_end_date
    ? `${r.new_start_date} → ${r.new_end_date}`
    : `${r.new_start_date} → month-to-month`;
  const open = ["proposed", "sent", "signed"].includes(r.status);

  return (
    <div className="space-y-3 px-5 py-4">
      <div className="flex flex-wrap items-center gap-3">
        <Badge tone={renewalTone(r.status)}>{r.status}</Badge>
        <span className="font-semibold">{r.new_rent_label} / mo</span>
        <span className="text-sm text-ink-3">{r.rent_change_label}</span>
        <span className="ml-auto text-sm text-ink-3">{term}</span>
      </div>

      {r.notes && <p className="text-sm text-ink-2">{r.notes}</p>}

      {r.envelope && r.envelope.signers.length > 0 && (
        <div className="divide-y divide-line rounded-lg border border-line">
          {r.envelope.signers.map((s) => (
            <div
              key={s.id}
              className="flex flex-wrap items-center gap-2 px-3 py-2 text-sm"
            >
              <span className="min-w-0 flex-1 truncate">
                <span className="font-semibold">{s.name}</span>{" "}
                <span className="text-xs text-ink-3">{s.email}</span>
              </span>
              <Badge tone="neutral">{s.role}</Badge>
              <Badge tone={signerTone(s.status)}>{s.status}</Badge>
            </div>
          ))}
        </div>
      )}

      {links && links.length > 0 && (
        <div className="space-y-1 rounded-lg border border-line bg-surface-2 p-3">
          {links.map((l) => (
            <CopyLink key={l.signer_id} link={l} />
          ))}
          <p className="text-xs text-ink-3">
            Links are shown once and were also emailed to each signer.
          </p>
        </div>
      )}

      {r.status === "activated" && (
        <p className="text-sm text-good">
          Signed &amp; applied
          {r.activated_at ? ` on ${r.activated_at.slice(0, 10)}` : ""} — the
          lease now reflects the new rent and term.
        </p>
      )}
      {r.status === "cancelled" && (
        <p className="text-sm text-ink-3">Cancelled.</p>
      )}
      {r.status === "declined" && (
        <p className="text-sm text-bad">A signer declined this renewal.</p>
      )}

      {manage && open && (
        <div className="flex gap-2">
          {r.status === "proposed" && (
            <button
              onClick={onSend}
              disabled={busy === `send-${r.id}`}
              className="rounded-lg bg-accent px-3 py-1.5 text-sm font-semibold text-on-accent disabled:opacity-50"
            >
              {busy === `send-${r.id}` ? "Sending…" : "Send for signature"}
            </button>
          )}
          <button
            onClick={onCancel}
            disabled={busy === `cancel-${r.id}`}
            className="rounded-lg border border-line px-3 py-1.5 text-sm font-semibold text-bad disabled:opacity-50"
          >
            Cancel
          </button>
        </div>
      )}
    </div>
  );
}

function CopyLink({ link }: { link: EsignSignerLink }) {
  const [copied, setCopied] = useState(false);
  return (
    <div className="flex items-center gap-2 text-sm">
      <span className="min-w-0 flex-1 truncate text-ink-2">{link.name}</span>
      <button
        onClick={() => {
          navigator.clipboard?.writeText(link.sign_url);
          setCopied(true);
          setTimeout(() => setCopied(false), 1500);
        }}
        className="rounded-lg border border-line px-2 py-1 text-xs font-semibold"
      >
        {copied ? "Copied!" : "Copy link"}
      </button>
    </div>
  );
}

function ProposeForm({
  currentRentCents,
  busy,
  onCancel,
  onSubmit,
}: {
  currentRentCents: number;
  busy: boolean;
  onCancel: () => void;
  onSubmit: (body: {
    new_rent_cents: number;
    term_months?: number;
    new_start_date?: string;
    new_end_date?: string;
    notes?: string;
  }) => void;
}) {
  const [rent, setRent] = useState(String(Math.round(currentRentCents / 100)));
  const [termMonths, setTermMonths] = useState("12");
  const [startDate, setStartDate] = useState("");
  const [notes, setNotes] = useState("");

  const rentNum = parseFloat(rent);
  const valid = !Number.isNaN(rentNum) && rentNum > 0;
  const months = parseInt(termMonths, 10);

  return (
    <div className="space-y-3 border-b border-line bg-surface-2 px-5 py-4">
      <div className="grid gap-3 sm:grid-cols-3">
        <label className="block text-sm">
          <span className="mb-1 block text-ink-3">New rent ($ / month)</span>
          <input
            type="number"
            min="0"
            step="10"
            value={rent}
            onChange={(e) => setRent(e.target.value)}
            className="w-full rounded-lg border border-line bg-surface px-3 py-2"
          />
        </label>
        <label className="block text-sm">
          <span className="mb-1 block text-ink-3">Term (months)</span>
          <input
            type="number"
            min="0"
            step="1"
            value={termMonths}
            onChange={(e) => setTermMonths(e.target.value)}
            placeholder="0 = month-to-month"
            className="w-full rounded-lg border border-line bg-surface px-3 py-2"
          />
        </label>
        <label className="block text-sm">
          <span className="mb-1 block text-ink-3">Effective (optional)</span>
          <input
            type="date"
            value={startDate}
            onChange={(e) => setStartDate(e.target.value)}
            className="w-full rounded-lg border border-line bg-surface px-3 py-2"
          />
        </label>
      </div>
      <label className="block text-sm">
        <span className="mb-1 block text-ink-3">Notes (optional)</span>
        <input
          value={notes}
          onChange={(e) => setNotes(e.target.value)}
          placeholder="Anything the addendum should note"
          className="w-full rounded-lg border border-line bg-surface px-3 py-2"
        />
      </label>
      <div className="flex gap-2">
        <button
          onClick={() =>
            onSubmit({
              new_rent_cents: Math.round(rentNum * 100),
              term_months: months > 0 ? months : undefined,
              new_start_date: startDate || undefined,
              notes: notes.trim() || undefined,
            })
          }
          disabled={!valid || busy}
          className="rounded-lg bg-accent px-4 py-2 text-sm font-semibold text-on-accent disabled:opacity-50"
        >
          {busy ? "Generating…" : "Generate addendum"}
        </button>
        <button
          onClick={onCancel}
          className="rounded-lg border border-line px-4 py-2 text-sm font-semibold"
        >
          Cancel
        </button>
      </div>
      <p className="text-xs text-ink-3">
        Generates a renewal addendum you can then send for e-signature. The
        lease&apos;s rent and end date update automatically once it&apos;s
        signed.
      </p>
    </div>
  );
}

function renewalTone(
  status: string
): "good" | "warn" | "bad" | "info" | "neutral" | "accent" {
  switch (status) {
    case "activated":
      return "good";
    case "sent":
      return "info";
    case "proposed":
      return "warn";
    case "declined":
      return "bad";
    default:
      return "neutral";
  }
}

function signerTone(
  status: string
): "good" | "warn" | "bad" | "info" | "neutral" {
  switch (status) {
    case "signed":
      return "good";
    case "viewed":
      return "info";
    case "declined":
      return "bad";
    case "sent":
      return "warn";
    default:
      return "neutral";
  }
}
