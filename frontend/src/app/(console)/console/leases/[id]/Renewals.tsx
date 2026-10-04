"use client";

// Renewals: propose new terms (rent and term), which generates an addendum;
// send it for e-signature; when everyone signs, the lease takes the new rent
// and end date on its own.

import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Ban, Plus, Repeat, Send } from "lucide-react";
import { api, type EsignSignerLink, type Renewal } from "@/lib/api";
import type { LeaseDetail } from "@/lib/types";
import type { Tone } from "@/components/ui/badge";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { CopyLink, F, inputClass, signerTone, useRun } from "../_ui/shared";

const OPEN = ["proposed", "sent", "signed"];

function renewalTone(status: string): Tone {
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

export function Renewals({
  lease,
  manage,
}: {
  lease: LeaseDetail;
  manage: boolean;
}) {
  const renewals = useQuery({
    queryKey: ["leases", lease.id, "renewals"],
    queryFn: () => api.leaseRenewals(lease.id),
  });
  const { busy, run } = useRun([["leases", lease.id]]);
  const [proposing, setProposing] = useState(false);
  // Signing links from "send" show once, keyed by renewal id.
  const [links, setLinks] = useState<Record<string, EsignSignerLink[]>>({});

  const list = renewals.data ?? [];
  const hasOpen = list.some((r) => OPEN.includes(r.status));

  return (
    <Panel>
      <PanelHeader
        title="Renewals"
        description={`Rent now ${lease.rent_label}. Term ends ${lease.end_date ?? "never, month to month"}.`}
        action={
          manage &&
          !proposing &&
          !hasOpen &&
          renewals.data && (
            <Button size="sm" onClick={() => setProposing(true)}>
              <Plus />
              Propose renewal
            </Button>
          )
        }
      />
      <div className="space-y-3 p-5 pt-4">
        {renewals.isLoading && <Skeleton className="h-20" />}
        {renewals.error && (
          <p className="text-[13px] text-bad">
            Couldn&apos;t load renewals: {renewals.error.message}
          </p>
        )}

        {proposing && (
          <Propose
            currentRentCents={lease.rent_cents}
            busy={busy === "propose"}
            onCancel={() => setProposing(false)}
            onSubmit={(body) =>
              run(
                "propose",
                async () => {
                  await api.proposeRenewal(lease.id, body);
                  setProposing(false);
                },
                "Addendum generated"
              )
            }
          />
        )}

        {renewals.data && list.length === 0 && !proposing && (
          <EmptyState
            icon={<Repeat />}
            title="No renewals yet"
            description={
              manage
                ? "Offer a new rent and term the resident can e-sign."
                : undefined
            }
            className="py-6"
          />
        )}

        {list.map((r) => (
          <Row
            key={r.id}
            renewal={r}
            manage={manage}
            busy={busy}
            links={links[r.id]}
            onSend={() =>
              run(
                `send-${r.id}`,
                async () => {
                  const resp = await api.sendRenewal(r.id, {});
                  setLinks((m) => ({ ...m, [r.id]: resp.sign_links }));
                },
                "Addendum sent for signature"
              )
            }
            onCancel={() => {
              if (!confirm("Cancel this renewal?")) return;
              void run(
                `cancel-${r.id}`,
                () => api.cancelRenewal(r.id),
                "Renewal cancelled"
              );
            }}
          />
        ))}
      </div>
    </Panel>
  );
}

function Row({
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
  const open = OPEN.includes(r.status);
  return (
    <div className="space-y-3 rounded-xl border border-line p-3">
      <div className="flex flex-wrap items-center gap-2">
        <Badge tone={renewalTone(r.status)}>{r.status}</Badge>
        <span className="figure text-[14px] font-semibold text-fg">
          {r.new_rent_label}/mo
        </span>
        <span className="text-xs text-fg-3">{r.rent_change_label}</span>
        <span className="ml-auto text-xs text-fg-3">
          {r.new_start_date} to {r.new_end_date ?? "month to month"}
        </span>
      </div>

      {r.notes && <p className="text-[13px] text-fg-2">{r.notes}</p>}

      {r.envelope && r.envelope.signers.length > 0 && (
        <ul className="divide-y divide-line rounded-lg border border-line">
          {r.envelope.signers.map((s) => (
            <li
              key={s.id}
              className="flex flex-wrap items-center gap-2 px-3 py-2"
            >
              <span className="min-w-0 flex-1 truncate text-[13px]">
                <span className="font-medium text-fg">{s.name}</span>{" "}
                <span className="text-xs text-fg-3">{s.email}</span>
              </span>
              <Badge tone="neutral">{s.role}</Badge>
              <Badge tone={signerTone(s.status)}>{s.status}</Badge>
            </li>
          ))}
        </ul>
      )}

      {links && links.length > 0 && (
        <div className="space-y-2 rounded-lg border border-line bg-fill/40 p-3">
          {links.map((l) => (
            <div key={l.signer_id} className="flex items-center gap-2">
              <span className="min-w-0 flex-1 truncate text-[13px] text-fg-2">
                {l.name}
              </span>
              <CopyLink url={l.sign_url} />
            </div>
          ))}
          <p className="text-xs text-fg-3">
            Links show once and were also emailed to each signer.
          </p>
        </div>
      )}

      {r.status === "activated" && (
        <p className="text-[13px] text-good">
          Signed and applied
          {r.activated_at ? ` on ${r.activated_at.slice(0, 10)}` : ""}. The
          lease has the new rent and term.
        </p>
      )}
      {r.status === "cancelled" && (
        <p className="text-[13px] text-fg-3">Cancelled.</p>
      )}
      {r.status === "declined" && (
        <p className="text-[13px] text-bad">A signer declined this renewal.</p>
      )}

      {manage && open && (
        <div className="flex gap-2">
          {r.status === "proposed" && (
            <Button
              size="sm"
              loading={busy === `send-${r.id}`}
              onClick={onSend}
            >
              <Send />
              Send for signature
            </Button>
          )}
          <Button
            size="sm"
            variant="danger"
            loading={busy === `cancel-${r.id}`}
            onClick={onCancel}
          >
            <Ban />
            Cancel renewal
          </Button>
        </div>
      )}
    </div>
  );
}

function Propose({
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
    notes?: string;
  }) => void;
}) {
  const [rent, setRent] = useState(String(Math.round(currentRentCents / 100)));
  const [termMonths, setTermMonths] = useState("12");
  const [startDate, setStartDate] = useState("");
  const [notes, setNotes] = useState("");

  const rentNum = parseFloat(rent);
  const valid = Number.isFinite(rentNum) && rentNum > 0;
  const months = parseInt(termMonths, 10);

  return (
    <form
      className="space-y-3 rounded-xl border border-line bg-fill/40 p-3"
      onSubmit={(e) => {
        e.preventDefault();
        if (!valid) return;
        onSubmit({
          new_rent_cents: Math.round(rentNum * 100),
          term_months: months > 0 ? months : undefined,
          new_start_date: startDate || undefined,
          notes: notes.trim() || undefined,
        });
      }}
    >
      <div className="grid gap-3 sm:grid-cols-3">
        <F label="New rent ($ a month)">
          <input
            type="number"
            min="0"
            step="10"
            value={rent}
            onChange={(e) => setRent(e.target.value)}
            className={inputClass}
          />
        </F>
        <F label="Term (months, 0 for month to month)">
          <input
            type="number"
            min="0"
            step="1"
            value={termMonths}
            onChange={(e) => setTermMonths(e.target.value)}
            className={inputClass}
          />
        </F>
        <F label="Starts (optional)">
          <input
            type="date"
            value={startDate}
            onChange={(e) => setStartDate(e.target.value)}
            className={inputClass}
          />
        </F>
      </div>
      <F label="Notes (optional)">
        <input
          value={notes}
          onChange={(e) => setNotes(e.target.value)}
          placeholder="Anything the addendum should say"
          className={inputClass}
        />
      </F>
      <p className="text-xs text-fg-3">
        Generates an addendum to send for e-signature. Rent and end date update
        once it&apos;s signed.
      </p>
      <div className="flex justify-end gap-2">
        <Button type="button" size="sm" variant="ghost" onClick={onCancel}>
          Cancel
        </Button>
        <Button type="submit" size="sm" loading={busy} disabled={!valid}>
          Generate addendum
        </Button>
      </div>
    </form>
  );
}
