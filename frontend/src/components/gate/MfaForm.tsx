"use client";

import { useState } from "react";
import { ArrowLeft, Fingerprint } from "lucide-react";
import { api, type MfaChallenge } from "@/lib/api";
import type { TokenResponse } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

/** The six-digit authenticator step that finishes a sign-in. */
export function MfaForm({
  challenge,
  onVerified,
  onBack,
}: {
  challenge: MfaChallenge;
  onVerified: (tokens: TokenResponse) => void;
  onBack?: () => void;
}) {
  const [code, setCode] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      onVerified(await api.mfaVerify(challenge.mfa_token, code.trim()));
    } catch {
      setError("That code isn't valid. Try the newest one from your app.");
      setBusy(false);
    }
  }

  return (
    <form onSubmit={submit}>
      <div className="mb-5 flex size-11 items-center justify-center rounded-2xl border border-accent/30 bg-accent/10 text-accent">
        <Fingerprint className="size-5" />
      </div>
      <h1 className="text-2xl font-semibold text-fg">Two-step verification</h1>
      <p className="mt-1.5 mb-6 text-[13px] text-fg-3">
        Enter the 6-digit code from your authenticator app.
      </p>
      <Input
        aria-label="Verification code"
        className="h-14 text-center font-mono text-2xl tracking-[0.5em]"
        inputMode="numeric"
        autoComplete="one-time-code"
        maxLength={6}
        placeholder="••••••"
        value={code}
        onChange={(e) => setCode(e.target.value.replace(/\D/g, ""))}
        aria-invalid={error ? true : undefined}
        autoFocus
      />
      {error && <p className="mt-2 text-xs text-bad">{error}</p>}
      <Button
        type="submit"
        size="lg"
        className="mt-5 w-full"
        loading={busy}
        disabled={code.length < 6}
      >
        Verify
      </Button>
      {onBack && (
        <Button
          type="button"
          variant="ghost"
          className="mt-2 w-full"
          onClick={onBack}
        >
          <ArrowLeft />
          Back
        </Button>
      )}
    </form>
  );
}
