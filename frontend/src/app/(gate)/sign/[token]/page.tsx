"use client";

// Where a signing link lands (`/sign/<token>?tenant=<slug>`). No sign-in:
// the token is the credential. The signer reads the exact text, types their
// name, agrees to sign electronically, and signs, or declines.

import { Suspense, useCallback, useEffect, useRef, useState } from "react";
import { useParams, useSearchParams } from "next/navigation";
import { CheckCircle2, FileSignature } from "lucide-react";
import { api, DEFAULT_TENANT, type PublicSignView } from "@/lib/api";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Panel } from "@/components/ui/panel";
import { Skeleton } from "@/components/ui/misc";
import { cn } from "@/lib/utils";

const field =
  "rounded-xl border border-line bg-surface px-3 py-2 text-[14px] text-fg outline-none focus:border-accent";

export default function SignPage() {
  return (
    <Suspense
      fallback={
        <main className="mx-auto max-w-2xl px-4 py-8">
          <Skeleton className="h-64" />
        </main>
      }
    >
      <SignInner />
    </Suspense>
  );
}

function SignInner() {
  const { token } = useParams<{ token: string }>();
  const tenant = useSearchParams().get("tenant") ?? DEFAULT_TENANT;
  const [view, setView] = useState<PublicSignView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [name, setName] = useState("");
  const [consent, setConsent] = useState(false);
  const [declining, setDeclining] = useState(false);
  const [reason, setReason] = useState("");

  useEffect(() => {
    api
      .publicSignView(token, tenant)
      .then(setView)
      .catch((e) => setError(e instanceof Error ? e.message : "not found"));
  }, [token, tenant]);

  // Opening the email isn't "viewed"; touching the document is.
  const viewed = useRef(false);
  const markViewed = useCallback(() => {
    if (viewed.current || view?.signer.status !== "sent") return;
    viewed.current = true;
    api
      .publicMarkViewed(token, tenant)
      .then(setView)
      .catch(() => {});
  }, [token, tenant, view?.signer.status]);

  async function sign() {
    setBusy(true);
    setError(null);
    try {
      setView(await api.publicSign(token, name.trim(), tenant));
    } catch (e) {
      setError(e instanceof Error ? e.message : "Couldn't sign");
    } finally {
      setBusy(false);
    }
  }
  async function decline() {
    setBusy(true);
    setError(null);
    try {
      setView(
        await api.publicDeclineSign(token, reason.trim() || undefined, tenant)
      );
      setDeclining(false);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Couldn't send that");
    } finally {
      setBusy(false);
    }
  }

  if (!view) {
    return (
      <main className="mx-auto max-w-2xl px-4 py-8">
        {error ? (
          <Panel className="p-6 text-center">
            <div className="text-[17px] font-semibold text-fg">
              This signing link isn&apos;t valid
            </div>
            <p className="mt-2 text-[13px] text-fg-3">
              It may have been replaced by a newer one, or the request was
              cancelled. Ask the sender for a fresh link.
            </p>
          </Panel>
        ) : (
          <Skeleton className="h-64" />
        )}
      </main>
    );
  }

  const me = view.signer;
  const open =
    view.envelope_status === "sent" ||
    view.envelope_status === "partially_signed";
  const canSign = open && me.status !== "signed" && me.status !== "declined";

  return (
    <main className="mx-auto max-w-2xl px-4 py-8">
      <div className="eyebrow">{view.company}</div>
      <div className="mt-1 flex flex-wrap items-center gap-2">
        <h1 className="text-[22px] leading-tight font-semibold text-fg">
          {view.document_title}
        </h1>
        <Badge
          tone={
            view.envelope_status === "completed"
              ? "good"
              : open
                ? "info"
                : "neutral"
          }
        >
          {view.envelope_status.replace("_", " ")}
        </Badge>
      </div>
      <p className="mt-1 text-[13px] text-fg-3">
        For {me.name} ({me.role}), sent by {view.company}
      </p>
      {view.message && (
        <p className="mt-3 rounded-xl bg-fill/60 px-3 py-2 text-[13px] text-fg-2">
          &ldquo;{view.message}&rdquo;
        </p>
      )}

      {me.status === "signed" && (
        <div className="mt-4 flex items-start gap-3 rounded-xl border border-good/30 bg-good/10 p-4 text-[14px] text-fg">
          <CheckCircle2 className="mt-0.5 size-5 text-good" />
          <span>
            You signed{me.signed_at ? ` on ${me.signed_at.slice(0, 10)}` : ""}{" "}
            as &ldquo;{me.signed_name}&rdquo;.
            {view.envelope_status === "completed"
              ? " Everyone has signed; the agreement is in force."
              : " We'll let you know when everyone has signed."}
          </span>
        </div>
      )}
      {me.status === "declined" && (
        <p className="mt-4 text-[14px] text-fg-2">
          You declined to sign. Nothing more to do.
        </p>
      )}
      {view.envelope_status === "voided" && (
        <p className="mt-4 text-[14px] text-fg-2">
          The sender cancelled this request. Nothing more to do.
        </p>
      )}

      {view.document_body && (
        <Panel className="mt-4 overflow-hidden">
          <div className="border-b border-line px-4 py-2 text-[11px] text-fg-3">
            Read the whole agreement. Checksum{" "}
            <span className="font-mono">
              sha256:{view.body_hash.slice(0, 16)}…
            </span>
          </div>
          <pre
            onScroll={markViewed}
            onTouchStart={markViewed}
            className="max-h-[50dvh] overflow-auto p-4 font-mono text-[12px] leading-relaxed whitespace-pre-wrap text-fg"
          >
            {view.document_body}
          </pre>
        </Panel>
      )}

      {view.co_signers.length > 0 && (
        <div className="mt-3 flex flex-wrap items-center gap-1.5 text-[12px] text-fg-3">
          Also signing:
          {view.co_signers.map((c, i) => (
            <Badge key={i} tone={c.status === "signed" ? "good" : "neutral"}>
              {c.name} · {c.status}
            </Badge>
          ))}
        </div>
      )}

      {error && <p className="mt-3 text-[13px] text-bad">{error}</p>}

      {canSign && !declining && (
        <Panel className="mt-4 p-5">
          <h2 className="flex items-center gap-2 text-[15px] font-semibold text-fg">
            <FileSignature className="size-4" />
            Sign this document
          </h2>
          <label className="mt-3 block text-[12px] text-fg-3">
            Type your full legal name as your signature
            <input
              className={cn(field, "mt-1 w-full font-semibold")}
              value={name}
              onFocus={markViewed}
              onChange={(e) => setName(e.target.value)}
              placeholder={me.name}
            />
          </label>
          <label className="mt-3 flex items-start gap-2 text-[12px] text-fg-2">
            <input
              type="checkbox"
              checked={consent}
              onChange={(e) => {
                markViewed();
                setConsent(e.target.checked);
              }}
              className="mt-0.5 size-4 shrink-0 accent-[var(--accent)]"
            />
            I agree to do this electronically and mean the name above as my
            legal signature (ESIGN and UETA). The time and my address on the
            network are kept in the audit trail.
          </label>
          <div className="mt-4 flex flex-col gap-2">
            <Button onClick={sign} disabled={busy || !consent || !name.trim()}>
              {busy ? "Signing…" : "Sign the agreement"}
            </Button>
            <Button
              variant="ghost"
              disabled={busy}
              onClick={() => {
                markViewed();
                setDeclining(true);
              }}
            >
              Decline
            </Button>
          </div>
        </Panel>
      )}

      {canSign && declining && (
        <Panel className="mt-4 p-5">
          <h2 className="text-[15px] font-semibold text-fg">Decline to sign</h2>
          <input
            className={cn(field, "mt-3 w-full")}
            placeholder="Why? (optional)"
            value={reason}
            onChange={(e) => setReason(e.target.value)}
          />
          <div className="mt-4 flex flex-col gap-2">
            <Button variant="danger" onClick={decline} disabled={busy}>
              {busy ? "Sending…" : "Confirm: I decline"}
            </Button>
            <Button
              variant="ghost"
              onClick={() => setDeclining(false)}
              disabled={busy}
            >
              Back
            </Button>
          </div>
        </Panel>
      )}
    </main>
  );
}
