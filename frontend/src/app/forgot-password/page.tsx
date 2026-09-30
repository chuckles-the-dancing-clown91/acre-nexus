"use client";

// "Forgot your password?" — ask for a reset link. The answer is the same
// whether or not the address has an account, so it can't be used to probe.

import { Suspense, useState } from "react";
import { useSearchParams } from "next/navigation";
import { api } from "@/lib/api";
import { Button } from "@/components/ui";
import { AuthShell, authField } from "@/components/AuthShell";

function ForgotPasswordForm() {
  const params = useSearchParams();
  const [email, setEmail] = useState(params.get("email") ?? "");
  const [sent, setSent] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      await api.passwordForgot(email.trim());
      setSent(true);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Something went wrong");
    } finally {
      setBusy(false);
    }
  }

  if (sent) {
    return (
      <>
        <h1 className="mb-1 font-display text-2xl font-extrabold">
          Check your email
        </h1>
        <p className="text-sm text-ink-2">
          If <strong>{email.trim()}</strong> has an account, a link to choose a
          new password is on its way (and a text, if we have your phone). It
          works once and expires in 24 hours.
        </p>
      </>
    );
  }

  return (
    <>
      <h1 className="mb-1 font-display text-2xl font-extrabold">
        Forgot your password?
      </h1>
      <p className="mb-6 text-sm text-ink-3">
        Enter your email and we&apos;ll send you a link to choose a new one.
      </p>
      <form onSubmit={submit} className="space-y-3">
        <input
          className={authField}
          type="email"
          autoComplete="email"
          placeholder="Email"
          value={email}
          onChange={(e) => setEmail(e.target.value)}
          autoFocus
          required
        />
        {error && <p className="text-sm text-bad">{error}</p>}
        <Button
          type="submit"
          disabled={busy || !email.trim()}
          className="w-full"
        >
          {busy ? "Sending…" : "Send me a link"}
        </Button>
      </form>
    </>
  );
}

export default function ForgotPasswordPage() {
  return (
    <AuthShell>
      <Suspense>
        <ForgotPasswordForm />
      </Suspense>
    </AuthShell>
  );
}
