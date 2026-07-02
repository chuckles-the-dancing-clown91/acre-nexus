"use client";

// E-signature envelope card (roadmap Phase 2 — contract signing).
//
// Lives on the lease page next to the generated document. Sends the document
// out for signature (resident + landlord by default, editable), then tracks
// the envelope: per-signer status (sent → viewed → signed / declined),
// one-time signing links, reminders (which rotate links), void, and the full
// ESIGN/UETA audit trail.

import { useCallback, useEffect, useState } from "react";
import {
  api,
  ApiError,
  type EsignEnvelope,
  type EsignSigner,
  type EsignSignerInput,
  type EsignSignerLink,
} from "@/lib/api";
import { Badge, Card } from "@/components/ui";
import { logError } from "@/lib/log";

const ROLES = ["resident", "landlord", "guarantor", "other"];

export function EsignCard({
  leaseId,
  manage,
  hasDocument,
  documentSigned,
  defaultSigners,
  onChanged,
}: {
  leaseId: string;
  manage: boolean;
  /** A generated lease document exists (envelopes need one). */
  hasDocument: boolean;
  /** The document is already signed (in person or via a completed envelope). */
  documentSigned: boolean;
  /** Prefill for the signer editor (lease resident, current user, …). */
  defaultSigners: EsignSignerInput[];
  /** Called after any envelope mutation so the parent can refresh the doc. */
  onChanged: () => void;
}) {
  const [envelope, setEnvelope] = useState<EsignEnvelope | null>(null);
  const [loaded, setLoaded] = useState(false);
  const [links, setLinks] = useState<EsignSignerLink[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [showTrail, setShowTrail] = useState(false);
  const [composing, setComposing] = useState(false);

  const load = useCallback(() => {
    api
      .leaseEnvelope(leaseId)
      .then((e) => {
        setEnvelope(e);
        setLoaded(true);
      })
      .catch((e) => {
        setEnvelope(null);
        setLoaded(true);
        if (!(e instanceof ApiError && e.status === 404)) {
          logError("failed to load envelope", e);
        }
      });
  }, [leaseId]);

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

  if (!loaded) return null;

  const open =
    envelope !== null &&
    (envelope.status === "sent" || envelope.status === "partially_signed");
  const canSend = manage && hasDocument && !documentSigned && !open;

  return (
    <Card className="overflow-hidden">
      <div className="flex flex-wrap items-center gap-3 border-b border-line px-5 py-4">
        <h2 className="flex-1 font-display text-lg font-bold">E-signature</h2>
        {envelope && (
          <Badge tone={envelopeTone(envelope.status)}>
            {envelope.status.replace("_", " ")}
          </Badge>
        )}
        {open && manage && (
          <>
            <button
              onClick={() =>
                run("remind", async () => {
                  const r = await api.remindEnvelope(envelope.id);
                  setLinks(r.sign_links);
                })
              }
              disabled={busy === "remind"}
              className="rounded-lg border border-line px-3 py-1.5 text-sm font-semibold disabled:opacity-50"
            >
              Send reminder
            </button>
            <button
              onClick={() =>
                run("void", async () => {
                  await api.voidEnvelope(envelope.id);
                  setLinks([]);
                })
              }
              disabled={busy === "void"}
              className="rounded-lg border border-line px-3 py-1.5 text-sm font-semibold text-bad disabled:opacity-50"
            >
              Void
            </button>
          </>
        )}
        {canSend && !composing && (
          <button
            onClick={() => setComposing(true)}
            className="rounded-lg bg-accent px-3 py-1.5 text-sm font-semibold text-on-accent"
          >
            Send for signature
          </button>
        )}
      </div>

      {error && <p className="px-5 py-3 text-sm text-bad">{error}</p>}

      {!envelope && !composing && (
        <div className="px-5 py-6 text-sm text-ink-3">
          {hasDocument
            ? documentSigned
              ? "This document was signed without an envelope."
              : manage
                ? "Send the generated document out for remote signature — each signer gets a secure link by email (and text, when a mobile is on file)."
                : "No signature request has been sent yet."
            : "Generate a lease document first, then send it for signature."}
        </div>
      )}

      {composing && (
        <ComposeForm
          defaults={defaultSigners}
          busy={busy === "send"}
          onCancel={() => setComposing(false)}
          onSend={(body) =>
            run("send", async () => {
              const r = await api.createEnvelope(leaseId, body);
              setLinks(r.sign_links);
              setComposing(false);
            })
          }
        />
      )}

      {envelope && (
        <div className="space-y-4 p-5">
          <div className="divide-y divide-line rounded-lg border border-line">
            {envelope.signers.map((s) => (
              <SignerRow
                key={s.id}
                signer={s}
                link={links.find((l) => l.signer_id === s.id)}
              />
            ))}
          </div>

          {links.length > 0 && (
            <p className="text-xs text-ink-3">
              Signing links are shown once — copy them now if you want to
              hand-deliver. They were also emailed
              {links.length > 1 ? " to each signer" : ""}.
            </p>
          )}

          {envelope.status === "completed" && (
            <p className="text-sm text-good">
              Fully signed
              {envelope.completed_at
                ? ` on ${envelope.completed_at.slice(0, 10)}`
                : ""}
              . The executed PDF is stored under Documents, and the lease is
              active.
            </p>
          )}
          {envelope.status === "voided" && (
            <p className="text-sm text-ink-3">
              Voided{envelope.void_reason ? ` — ${envelope.void_reason}` : ""}.
              Regenerate or re-send the document to start a new envelope.
            </p>
          )}
          {envelope.status === "declined" && (
            <p className="text-sm text-bad">
              A signer declined. Revise the document and send a new envelope.
            </p>
          )}

          <div>
            <button
              onClick={() => setShowTrail((v) => !v)}
              className="text-xs font-semibold text-ink-3 underline"
            >
              {showTrail ? "Hide" : "Show"} audit trail (
              {envelope.events.length})
            </button>
            {showTrail && (
              <div className="mt-2 space-y-1.5 rounded-lg border border-line bg-surface-2 p-3 text-xs text-ink-3">
                {envelope.events.map((e) => (
                  <div key={e.id} className="flex flex-wrap items-center gap-2">
                    <span className="font-semibold text-ink-2">{e.event}</span>
                    <span>
                      {(e.detail?.signer as string) ??
                        (e.detail?.reason as string) ??
                        ""}
                    </span>
                    {e.ip && <span className="font-mono">{e.ip}</span>}
                    <span className="ml-auto font-mono">
                      {e.created_at.replace("T", " ").slice(0, 16)}
                    </span>
                  </div>
                ))}
                <div className="pt-1 font-mono">
                  document sha256:{envelope.body_hash.slice(0, 16)}…
                </div>
              </div>
            )}
          </div>
        </div>
      )}
    </Card>
  );
}

function SignerRow({
  signer,
  link,
}: {
  signer: EsignSigner;
  link?: EsignSignerLink;
}) {
  const [copied, setCopied] = useState(false);
  return (
    <div className="flex flex-wrap items-center gap-3 px-4 py-3 text-sm">
      <div className="min-w-0 flex-1">
        <span className="font-semibold">{signer.name}</span>{" "}
        <span className="text-xs text-ink-3">
          {signer.email}
          {signer.phone ? ` · ${signer.phone}` : ""}
        </span>
      </div>
      <Badge tone="neutral">{signer.role}</Badge>
      <Badge tone={signerTone(signer.status)}>
        {signer.status}
        {signer.status === "signed" && signer.signed_at
          ? ` ${signer.signed_at.slice(0, 10)}`
          : ""}
      </Badge>
      {link && (
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
      )}
    </div>
  );
}

function ComposeForm({
  defaults,
  busy,
  onCancel,
  onSend,
}: {
  defaults: EsignSignerInput[];
  busy: boolean;
  onCancel: () => void;
  onSend: (body: { message?: string; signers: EsignSignerInput[] }) => void;
}) {
  const [signers, setSigners] = useState<EsignSignerInput[]>(
    defaults.length > 0 ? defaults : [{ role: "resident", name: "", email: "" }]
  );
  const [message, setMessage] = useState("");

  function setField(i: number, field: keyof EsignSignerInput, value: string) {
    setSigners((list) =>
      list.map((s, idx) => (idx === i ? { ...s, [field]: value } : s))
    );
  }

  const valid =
    signers.length > 0 &&
    signers.every((s) => s.name.trim() && s.email.includes("@"));

  return (
    <div className="space-y-3 border-b border-line bg-surface-2 px-5 py-4">
      <p className="text-sm font-semibold">Signers</p>
      {signers.map((s, i) => (
        <div key={i} className="flex flex-wrap items-end gap-2 text-sm">
          <select
            value={s.role ?? "other"}
            onChange={(e) => setField(i, "role", e.target.value)}
            className="rounded-lg border border-line bg-surface px-2 py-2 capitalize"
          >
            {ROLES.map((r) => (
              <option key={r} value={r}>
                {r}
              </option>
            ))}
          </select>
          <input
            value={s.name}
            onChange={(e) => setField(i, "name", e.target.value)}
            placeholder="Full name"
            className="min-w-[140px] flex-1 rounded-lg border border-line bg-surface px-3 py-2"
          />
          <input
            value={s.email}
            onChange={(e) => setField(i, "email", e.target.value)}
            placeholder="Email"
            className="min-w-[160px] flex-1 rounded-lg border border-line bg-surface px-3 py-2"
          />
          <input
            value={s.phone ?? ""}
            onChange={(e) => setField(i, "phone", e.target.value)}
            placeholder="Mobile (optional)"
            className="w-36 rounded-lg border border-line bg-surface px-3 py-2"
          />
          {signers.length > 1 && (
            <button
              onClick={() =>
                setSigners((list) => list.filter((_, idx) => idx !== i))
              }
              className="pb-2 text-ink-3"
              title="Remove signer"
            >
              ✕
            </button>
          )}
        </div>
      ))}
      <button
        onClick={() =>
          setSigners((list) => [
            ...list,
            { role: "other", name: "", email: "" },
          ])
        }
        className="text-xs font-semibold text-ink-3 underline"
      >
        + Add signer
      </button>
      <label className="block text-sm">
        <span className="mb-1 block text-ink-3">
          Message to signers (optional)
        </span>
        <input
          value={message}
          onChange={(e) => setMessage(e.target.value)}
          className="w-full rounded-lg border border-line bg-surface px-3 py-2"
        />
      </label>
      <div className="flex gap-2">
        <button
          onClick={() =>
            onSend({ message: message.trim() || undefined, signers })
          }
          disabled={!valid || busy}
          className="rounded-lg bg-accent px-4 py-2 text-sm font-semibold text-on-accent disabled:opacity-50"
        >
          {busy ? "Sending…" : "Send signing links"}
        </button>
        <button
          onClick={onCancel}
          className="rounded-lg border border-line px-4 py-2 text-sm font-semibold"
        >
          Cancel
        </button>
      </div>
      <p className="text-xs text-ink-3">
        Each signer gets a unique, single-use link by email
        {signers.some((s) => (s.phone ?? "").trim()) ? " and text" : ""}. The
        lease activates automatically when everyone has signed.
      </p>
    </div>
  );
}

function envelopeTone(
  status: string
): "good" | "warn" | "bad" | "info" | "neutral" {
  switch (status) {
    case "completed":
      return "good";
    case "partially_signed":
      return "info";
    case "sent":
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
