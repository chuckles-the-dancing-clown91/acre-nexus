"use client";

// "Forgot your password?" asks for a reset link. The answer is the same
// whether or not the address has an account, so it can't be used to probe.

import { Suspense, useState } from "react";
import { useSearchParams } from "next/navigation";
import { MailCheck } from "lucide-react";
import { api } from "@/lib/api";
import { AuthFrame } from "@/components/gate/AuthFrame";
import { BackToSignIn } from "@/components/gate/BackToSignIn";
import { Button } from "@/components/ui/button";
import { Field, Input } from "@/components/ui/input";

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
    } catch (err) {
      setError(err instanceof Error ? err.message : "Something went wrong");
    } finally {
      setBusy(false);
    }
  }

  if (sent) {
    return (
      <>
        <div className="mb-5 flex size-11 items-center justify-center rounded-2xl border border-good/30 bg-good/10 text-good">
          <MailCheck className="size-5" />
        </div>
        <h1 className="text-2xl font-semibold text-fg">Check your email</h1>
        <p className="mt-2 text-[13px] leading-relaxed text-fg-2">
          If <strong className="text-fg">{email.trim()}</strong> has an account,
          a link to choose a new password is on its way (and a text, if we have
          your phone). It works once and expires in 24 hours.
        </p>
      </>
    );
  }

  return (
    <form onSubmit={submit}>
      <h1 className="text-2xl font-semibold text-fg">Reset your password</h1>
      <p className="mt-1.5 mb-6 text-[13px] text-fg-3">
        Enter your email and we&apos;ll send you a link to choose a new one.
      </p>
      <Field label="Email" error={error}>
        {(p) => (
          <Input
            {...p}
            type="email"
            autoComplete="email"
            placeholder="you@company.com"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            autoFocus
            required
          />
        )}
      </Field>
      <Button
        type="submit"
        size="lg"
        className="mt-5 w-full"
        loading={busy}
        disabled={!email.trim()}
      >
        Send me a link
      </Button>
    </form>
  );
}

export default function ForgotPasswordPage() {
  return (
    <AuthFrame>
      <Suspense>
        <ForgotPasswordForm />
      </Suspense>
      <BackToSignIn />
    </AuthFrame>
  );
}
