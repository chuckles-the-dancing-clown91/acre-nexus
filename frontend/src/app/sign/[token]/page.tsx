"use client";

// Public e-signature page — the destination of emailed/texted signing links
// (`/sign/<token>?tenant=<slug>`). No login: possession of the token is the
// credential. The signer reviews the exact document text, types their name,
// consents to sign electronically (ESIGN/UETA), and signs — or declines.

import { Suspense, useCallback, useEffect, useState } from "react";
import { useParams, useSearchParams } from "next/navigation";
import { api, DEFAULT_TENANT, type PublicSignView } from "@/lib/api";
import { Badge, Card } from "@/components/ui";

export default function SignPage() {
  return (
    <Suspense
      fallback={
        <Shell>
          <p className="text-ink-3">Loading…</p>
        </Shell>
      }
    >
      <SignPageInner />
    </Suspense>
  );
}

function SignPageInner() {
  const params = useParams<{ token: string }>();
  const search = useSearchParams();
  const token = params.token;
  const tenant = search.get("tenant") ?? DEFAULT_TENANT;

  const [view, setView] = useState<PublicSignView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [name, setName] = useState("");
  const [consent, setConsent] = useState(false);
  const [declining, setDeclining] = useState(false);
  const [declineReason, setDeclineReason] = useState("");

  const load = useCallback(() => {
    api
      .publicSignView(token, tenant)
      .then((v) => {
        setView(v);
        setError(null);
      })
      .catch((e) => setError(e.message));
  }, [token, tenant]);

  useEffect(() => {
    load();
  }, [load]);

  async function submitSignature() {
    setBusy(true);
    setError(null);
    try {
      const v = await api.publicSign(token, name.trim(), tenant);
      setView(v);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  }

  async function submitDecline() {
    setBusy(true);
    setError(null);
    try {
      const v = await api.publicDeclineSign(
        token,
        declineReason.trim() || undefined,
        tenant
      );
      setView(v);
      setDeclining(false);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  }

  if (!view) {
    return (
      <Shell>
        {error ? (
          <Card className="p-8 text-center">
            <h1 className="mb-2 font-display text-xl font-bold">
              This signing link isn&apos;t valid
            </h1>
            <p className="text-sm text-ink-3">
              {error} — the link may have been replaced by a newer one or the
              request may have been cancelled. Contact the sender for a fresh
              link.
            </p>
          </Card>
        ) : (
          <p className="text-ink-3">Loading…</p>
        )}
      </Shell>
    );
  }

  const me = view.signer;
  const envelopeOpen =
    view.envelope_status === "sent" ||
    view.envelope_status === "partially_signed";
  const canSign =
    envelopeOpen && me.status !== "signed" && me.status !== "declined";

  return (
    <Shell company={view.company}>
      <div className="mb-5">
        <div className="flex flex-wrap items-center gap-3">
          <h1 className="font-display text-2xl font-extrabold tracking-tight">
            {view.document_title}
          </h1>
          <Badge
            tone={
              view.envelope_status === "completed"
                ? "good"
                : envelopeOpen
                  ? "info"
                  : "neutral"
            }
          >
            {view.envelope_status.replace("_", " ")}
          </Badge>
        </div>
        <p className="text-sm text-ink-3">
          For {me.name} ({me.role}) · sent by {view.company}
        </p>
        {view.message && (
          <p className="mt-2 rounded-lg border border-line bg-surface-2 px-4 py-3 text-sm">
            “{view.message}”
          </p>
        )}
      </div>

      {me.status === "signed" && (
        <Card className="mb-5 border-good p-5 text-sm text-good">
          You signed{me.signed_at ? ` on ${me.signed_at.slice(0, 10)}` : ""} as
          “{me.signed_name}”.
          {view.envelope_status === "completed"
            ? " All parties have signed — the agreement is fully executed."
            : " We'll let you know when all parties have signed."}
        </Card>
      )}
      {me.status === "declined" && (
        <Card className="mb-5 p-5 text-sm text-ink-3">
          You declined to sign this document. No further action is needed.
        </Card>
      )}
      {view.envelope_status === "voided" && (
        <Card className="mb-5 p-5 text-sm text-ink-3">
          This signature request was cancelled by the sender. No further action
          is needed.
        </Card>
      )}

      {view.document_body && (
        <Card className="mb-5 overflow-hidden">
          <div className="border-b border-line px-5 py-3 text-xs text-ink-3">
            Review the full agreement below. Integrity checksum{" "}
            <span className="font-mono">
              sha256:{view.body_hash.slice(0, 16)}…
            </span>
          </div>
          <pre className="max-h-[28rem] overflow-auto whitespace-pre-wrap p-5 font-mono text-xs leading-relaxed">
            {view.document_body}
          </pre>
        </Card>
      )}

      {view.co_signers.length > 0 && (
        <div className="mb-5 flex flex-wrap items-center gap-2 text-sm text-ink-3">
          <span>Also signing:</span>
          {view.co_signers.map((c, i) => (
            <Badge key={i} tone={c.status === "signed" ? "good" : "neutral"}>
              {c.name} · {c.status}
            </Badge>
          ))}
        </div>
      )}

      {error && <p className="mb-4 text-sm text-bad">{error}</p>}

      {canSign && !declining && (
        <Card className="p-5">
          <h2 className="mb-3 font-display text-lg font-bold">
            Sign this document
          </h2>
          <label className="mb-3 block text-sm">
            <span className="mb-1 block text-ink-3">
              Type your full legal name as your signature
            </span>
            <input
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder={me.name}
              className="w-full max-w-md rounded-lg border border-line bg-surface px-3 py-2 font-semibold"
            />
          </label>
          <label className="mb-4 flex items-start gap-2 text-sm">
            <input
              type="checkbox"
              checked={consent}
              onChange={(e) => setConsent(e.target.checked)}
              className="mt-1"
            />
            <span className="text-ink-2">
              I agree to conduct this transaction electronically and intend the
              name typed above to be my legal signature (ESIGN / UETA). My IP
              address and the time of signing will be recorded in the audit
              trail.
            </span>
          </label>
          <div className="flex flex-wrap gap-3">
            <button
              onClick={submitSignature}
              disabled={busy || !consent || !name.trim()}
              className="rounded-xl bg-accent px-5 py-2.5 text-sm font-bold text-on-accent disabled:opacity-50"
            >
              {busy ? "Signing…" : "Sign agreement"}
            </button>
            <button
              onClick={() => setDeclining(true)}
              disabled={busy}
              className="rounded-xl border border-line px-5 py-2.5 text-sm font-bold text-ink-2"
            >
              Decline
            </button>
          </div>
        </Card>
      )}

      {canSign && declining && (
        <Card className="p-5">
          <h2 className="mb-3 font-display text-lg font-bold">
            Decline to sign
          </h2>
          <label className="mb-4 block text-sm">
            <span className="mb-1 block text-ink-3">Reason (optional)</span>
            <input
              value={declineReason}
              onChange={(e) => setDeclineReason(e.target.value)}
              className="w-full max-w-md rounded-lg border border-line bg-surface px-3 py-2"
            />
          </label>
          <div className="flex flex-wrap gap-3">
            <button
              onClick={submitDecline}
              disabled={busy}
              className="rounded-xl bg-bad px-5 py-2.5 text-sm font-bold text-white disabled:opacity-50"
            >
              {busy ? "Declining…" : "Confirm decline"}
            </button>
            <button
              onClick={() => setDeclining(false)}
              disabled={busy}
              className="rounded-xl border border-line px-5 py-2.5 text-sm font-bold text-ink-2"
            >
              Back
            </button>
          </div>
        </Card>
      )}
    </Shell>
  );
}

function Shell({
  company,
  children,
}: {
  company?: string;
  children: React.ReactNode;
}) {
  return (
    <main className="mx-auto max-w-[860px] px-6 py-10">
      <div className="mb-8 flex items-center gap-3">
        <div className="flex h-9 w-9 items-center justify-center rounded-xl bg-accent font-display text-lg font-extrabold text-on-accent">
          {(company ?? "A").charAt(0)}
        </div>
        <span className="font-display text-lg font-bold">
          {company ?? "Secure signing"}
        </span>
        <span className="ml-auto text-xs text-ink-3">Secure e-signature</span>
      </div>
      {children}
    </main>
  );
}
