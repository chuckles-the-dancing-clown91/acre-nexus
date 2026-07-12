"use client";

// Federated-login landing (issue #63). The provider (or the sandbox) redirects
// the browser here with ?provider&code&state; we complete the exchange and
// either apply the session, run the MFA step-up, or confirm a link.

import { Suspense, useEffect, useRef, useState } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import { toast } from "sonner";
import { useAuth } from "@/lib/auth";
import { api } from "@/lib/api";
import type { MfaChallenge } from "@/lib/api";
import { Button, Card } from "@/components/ui";

function Callback() {
  const params = useSearchParams();
  const router = useRouter();
  const { establishSession } = useAuth();
  const ran = useRef(false);

  const [error, setError] = useState<string | null>(null);
  const [challenge, setChallenge] = useState<MfaChallenge | null>(null);
  const [code, setCode] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (ran.current) return;
    ran.current = true;
    const provider = params.get("provider") ?? "";
    const codeParam = params.get("code") ?? "";
    const state = params.get("state") ?? "";
    const run = async () => {
      if (!provider || !codeParam || !state) {
        throw new Error("Missing callback parameters.");
      }
      const res = await api.oauthCallback(provider, codeParam, state);
      if (res.outcome === "session" && res.session) {
        establishSession(res.session);
        router.replace("/console");
      } else if (res.outcome === "mfa" && res.mfa) {
        setChallenge(res.mfa);
      } else if (res.outcome === "linked") {
        toast.success(`Linked your ${res.provider ?? "account"}`);
        router.replace("/console/security");
      } else {
        throw new Error("Unexpected response from the sign-in provider.");
      }
    };
    run().catch((e) => setError(e instanceof Error ? e.message : "Sign-in failed"));
  }, [params, router, establishSession]);

  async function submitMfa(e: React.FormEvent) {
    e.preventDefault();
    if (!challenge) return;
    setBusy(true);
    setError(null);
    try {
      const tokens = await api.mfaVerify(challenge.mfa_token, code.trim());
      establishSession(tokens);
      router.replace("/console");
    } catch {
      setError("That code isn't valid — try again.");
    } finally {
      setBusy(false);
    }
  }

  const field =
    "w-full rounded-xl border border-line bg-surface-2 px-3.5 py-3 text-sm outline-none focus:border-accent";

  return (
    <main className="flex min-h-screen items-center justify-center px-6">
      <Card className="w-full max-w-md p-8">
        {challenge ? (
          <>
            <h1 className="mb-1 font-display text-2xl font-extrabold">
              Two-factor
            </h1>
            <p className="mb-6 text-sm text-ink-3">
              Enter the 6-digit code from your authenticator app to finish
              signing in.
            </p>
            <form onSubmit={submitMfa} className="space-y-3">
              <input
                className={`${field} text-center font-mono text-lg tracking-widest`}
                inputMode="numeric"
                autoComplete="one-time-code"
                placeholder="000000"
                value={code}
                onChange={(e) => setCode(e.target.value)}
                autoFocus
              />
              {error && <p className="text-sm text-bad">{error}</p>}
              <Button
                type="submit"
                disabled={busy || code.trim().length < 6}
                className="w-full"
              >
                {busy ? "Verifying…" : "Verify"}
              </Button>
            </form>
          </>
        ) : error ? (
          <>
            <h1 className="mb-1 font-display text-2xl font-extrabold">
              Sign-in failed
            </h1>
            <p className="mb-6 text-sm text-bad">{error}</p>
            <Button onClick={() => router.replace("/login")} className="w-full">
              Back to sign in
            </Button>
          </>
        ) : (
          <p className="text-center text-ink-3">Completing sign-in…</p>
        )}
      </Card>
    </main>
  );
}

export default function OauthCallbackPage() {
  return (
    <Suspense
      fallback={
        <main className="flex min-h-screen items-center justify-center">
          <p className="text-ink-3">Completing sign-in…</p>
        </main>
      }
    >
      <Callback />
    </Suspense>
  );
}
