"use client";

// E-signature: send the generated lease out for signature (resident and
// landlord by default, editable), then follow each signer from sent to viewed
// to signed or declined. Signing links show once. Reminders rotate the links.
// Void stops the envelope. The audit trail records every step.

import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { BellRing, Ban, Plus, Send, X } from "lucide-react";
import {
  api,
  ApiError,
  type EsignEnvelope,
  type EsignSignerInput,
  type EsignSignerLink,
} from "@/lib/api";
import { logError } from "@/lib/log";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import {
  CopyLink,
  envelopeTone,
  humanize,
  inputClass,
  signerTone,
  useRun,
} from "../_ui/shared";

const ROLES = ["resident", "landlord", "guarantor", "other"];

export function Esign({
  leaseId,
  manage,
  hasDocument,
  documentSigned,
  defaultSigners,
}: {
  leaseId: string;
  manage: boolean;
  hasDocument: boolean;
  documentSigned: boolean;
  defaultSigners: EsignSignerInput[];
}) {
  const envelope = useQuery({
    queryKey: ["leases", leaseId, "envelope"],
    queryFn: async (): Promise<EsignEnvelope | null> => {
      try {
        return await api.leaseEnvelope(leaseId);
      } catch (e) {
        if (e instanceof ApiError && e.status === 404) return null;
        logError("failed to load envelope", e);
        throw e;
      }
    },
  });
  const { busy, run } = useRun([["leases", leaseId]]);
  const [links, setLinks] = useState<EsignSignerLink[]>([]);
  const [composing, setComposing] = useState(false);
  const [showTrail, setShowTrail] = useState(false);

  if (envelope.isLoading) return <Skeleton className="h-32 rounded-2xl" />;

  const env = envelope.data ?? null;
  const open =
    env !== null &&
    (env.status === "sent" || env.status === "partially_signed");
  const canSend = manage && hasDocument && !documentSigned && !open;

  return (
    <Panel>
      <PanelHeader
        title="E-signature"
        description={
          env
            ? `Sent ${env.sent_at.slice(0, 10)}`
            : "Each signer gets a secure link by email, and by text when a mobile is on file."
        }
        action={
          <div className="flex flex-wrap items-center justify-end gap-2">
            {env && (
              <Badge tone={envelopeTone(env.status)}>
                {env.status.replace("_", " ")}
              </Badge>
            )}
            {open && manage && (
              <>
                <Button
                  size="sm"
                  variant="secondary"
                  loading={busy === "remind"}
                  onClick={() =>
                    run(
                      "remind",
                      async () => {
                        const r = await api.remindEnvelope(env.id);
                        setLinks(r.sign_links);
                      },
                      "Reminder sent with fresh links"
                    )
                  }
                >
                  <BellRing />
                  Remind
                </Button>
                <Button
                  size="sm"
                  variant="danger"
                  loading={busy === "void"}
                  onClick={() => {
                    if (
                      !confirm(
                        "Void this envelope? Its signing links stop working."
                      )
                    )
                      return;
                    void run(
                      "void",
                      async () => {
                        await api.voidEnvelope(env.id);
                        setLinks([]);
                      },
                      "Envelope voided"
                    );
                  }}
                >
                  <Ban />
                  Void
                </Button>
              </>
            )}
            {canSend && !composing && (
              <Button size="sm" onClick={() => setComposing(true)}>
                <Send />
                Send for signature
              </Button>
            )}
          </div>
        }
      />
      <div className="space-y-4 p-5 pt-4">
        {envelope.error && (
          <p className="text-[13px] text-bad">
            Couldn&apos;t load the envelope: {envelope.error.message}
          </p>
        )}

        {!env && !composing && !envelope.error && (
          <p className="text-[13px] text-fg-3">
            {!hasDocument
              ? "Generate the lease document first, then send it for signature."
              : documentSigned
                ? "This document was signed in person."
                : manage
                  ? "Nothing sent yet."
                  : "No signature request has been sent yet."}
          </p>
        )}

        {composing && (
          <Compose
            defaults={defaultSigners}
            busy={busy === "send"}
            onCancel={() => setComposing(false)}
            onSend={(body) =>
              run(
                "send",
                async () => {
                  const r = await api.createEnvelope(leaseId, body);
                  setLinks(r.sign_links);
                  setComposing(false);
                },
                "Signing links sent"
              )
            }
          />
        )}

        {env && (
          <>
            <ul className="divide-y divide-line rounded-xl border border-line">
              {env.signers.map((s) => {
                const link = links.find((l) => l.signer_id === s.id);
                return (
                  <li
                    key={s.id}
                    className="flex flex-wrap items-center gap-2 px-3 py-2.5"
                  >
                    <div className="min-w-0 flex-1">
                      <div className="truncate text-[13px] font-medium text-fg">
                        {s.name}
                      </div>
                      <div className="truncate text-xs text-fg-3">
                        {s.email}
                        {s.phone ? ` · ${s.phone}` : ""}
                      </div>
                    </div>
                    <Badge tone="neutral">{s.role}</Badge>
                    <Badge tone={signerTone(s.status)}>
                      {s.status}
                      {s.status === "signed" && s.signed_at
                        ? ` ${s.signed_at.slice(0, 10)}`
                        : ""}
                    </Badge>
                    {link && <CopyLink url={link.sign_url} />}
                  </li>
                );
              })}
            </ul>

            {links.length > 0 && (
              <p className="text-xs text-fg-3">
                Links show once. Copy them now to hand-deliver. They were also
                emailed{links.length > 1 ? " to each signer" : ""}.
              </p>
            )}
            {env.status === "completed" && (
              <p className="text-[13px] text-good">
                Fully signed
                {env.completed_at ? ` on ${env.completed_at.slice(0, 10)}` : ""}
                . The signed PDF is under Documents and the lease is active.
              </p>
            )}
            {env.status === "voided" && (
              <p className="text-[13px] text-fg-3">
                Voided{env.void_reason ? `: ${env.void_reason}` : ""}.
                Regenerate or send the document again to start a new envelope.
              </p>
            )}
            {env.status === "declined" && (
              <p className="text-[13px] text-bad">
                A signer declined. Revise the document and send a new envelope.
              </p>
            )}

            <div>
              <button
                type="button"
                onClick={() => setShowTrail((v) => !v)}
                className="text-xs font-medium text-fg-3 underline-offset-2 hover:text-fg hover:underline"
              >
                {showTrail ? "Hide" : "Show"} audit trail ({env.events.length})
              </button>
              {showTrail && (
                <ul className="mt-2 space-y-1.5 rounded-xl border border-line bg-fill/40 p-3 text-xs text-fg-3">
                  {env.events.map((e) => (
                    <li
                      key={e.id}
                      className="flex flex-wrap items-center gap-2"
                    >
                      <span className="font-medium text-fg-2">{e.event}</span>
                      <span>
                        {(e.detail?.signer as string) ??
                          (e.detail?.reason as string) ??
                          ""}
                      </span>
                      {e.ip && <span className="font-mono">{e.ip}</span>}
                      <span className="ml-auto font-mono">
                        {e.created_at.replace("T", " ").slice(0, 16)}
                      </span>
                    </li>
                  ))}
                  <li className="pt-1 font-mono">
                    document sha256 {env.body_hash.slice(0, 16)}…
                  </li>
                </ul>
              )}
            </div>
          </>
        )}
      </div>
    </Panel>
  );
}

function Compose({
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

  function set(i: number, field: keyof EsignSignerInput, value: string) {
    setSigners((list) =>
      list.map((s, idx) => (idx === i ? { ...s, [field]: value } : s))
    );
  }

  const valid =
    signers.length > 0 &&
    signers.every((s) => s.name.trim() && s.email.includes("@"));

  return (
    <form
      className="space-y-3 rounded-xl border border-line bg-fill/40 p-3"
      onSubmit={(e) => {
        e.preventDefault();
        if (valid) onSend({ message: message.trim() || undefined, signers });
      }}
    >
      <div className="text-xs font-medium text-fg-3">Signers</div>
      {signers.map((s, i) => (
        <div
          key={i}
          className="grid gap-2 sm:grid-cols-[8rem_1fr_1fr_9rem_auto]"
        >
          <select
            aria-label="Role"
            value={s.role ?? "other"}
            onChange={(e) => set(i, "role", e.target.value)}
            className={inputClass}
          >
            {ROLES.map((r) => (
              <option key={r} value={r}>
                {humanize(r)}
              </option>
            ))}
          </select>
          <input
            aria-label="Full name"
            value={s.name}
            onChange={(e) => set(i, "name", e.target.value)}
            placeholder="Full name"
            className={inputClass}
          />
          <input
            aria-label="Email"
            type="email"
            value={s.email}
            onChange={(e) => set(i, "email", e.target.value)}
            placeholder="Email"
            className={inputClass}
          />
          <input
            aria-label="Mobile"
            value={s.phone ?? ""}
            onChange={(e) => set(i, "phone", e.target.value)}
            placeholder="Mobile (optional)"
            className={inputClass}
          />
          <button
            type="button"
            aria-label="Remove signer"
            disabled={signers.length < 2}
            onClick={() =>
              setSigners((list) => list.filter((_, idx) => idx !== i))
            }
            className="justify-self-end rounded-lg p-2 text-fg-3 transition hover:bg-fill-2 hover:text-bad disabled:invisible"
          >
            <X className="size-4" />
          </button>
        </div>
      ))}
      <Button
        type="button"
        size="sm"
        variant="ghost"
        onClick={() =>
          setSigners((list) => [
            ...list,
            { role: "other", name: "", email: "" },
          ])
        }
      >
        <Plus />
        Add signer
      </Button>
      <label className="block">
        <span className="mb-1 block text-xs font-medium text-fg-3">
          Message to signers (optional)
        </span>
        <input
          value={message}
          onChange={(e) => setMessage(e.target.value)}
          className={inputClass}
        />
      </label>
      <p className="text-xs text-fg-3">
        Each signer gets a single-use link by email
        {signers.some((s) => (s.phone ?? "").trim()) ? " and text" : ""}. The
        lease activates when everyone has signed.
      </p>
      <div className="flex justify-end gap-2">
        <Button type="button" size="sm" variant="ghost" onClick={onCancel}>
          Cancel
        </Button>
        <Button type="submit" size="sm" loading={busy} disabled={!valid}>
          <Send />
          Send signing links
        </Button>
      </div>
    </form>
  );
}
