"use client";

// Choose a password from an invite ("set up your account") or a reset link,
// then sign straight in: residents land in their portal, staff in the console.

import { Suspense, useEffect, useState } from "react";
import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import { api, isMfaChallenge, type PasswordLinkInfo } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { Button } from "@/components/ui";
import { AuthShell, authField } from "@/components/AuthShell";
import { MIN_PASSWORD_LENGTH } from "@/components/ChangePasswordCard";

function SetPasswordForm() {
  const params = useSearchParams();
  const router = useRouter();
  const { login } = useAuth();
  const token = params.get("token") ?? "";

  const [info, setInfo] = useState<PasswordLinkInfo | null>(null);
  const [invalid, setInvalid] = useState(!token);
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [show, setShow] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!token) return;
    api
      .passwordLink(token)
      .then(setInfo)
      .catch(() => setInvalid(true));
  }, [token]);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    if (password.length < MIN_PASSWORD_LENGTH) {
      setError(
        `Use at least ${MIN_PASSWORD_LENGTH} characters — a short phrase is easiest to remember.`
      );
      return;
    }
    if (password !== confirm) {
      setError("The two passwords don't match.");
      return;
    }
    setBusy(true);
    try {
      const res = await api.passwordSet(token, password);
      const session = await login(res.email, password);
      // An account with two-step sign-in finishes on the login page.
      if (isMfaChallenge(session)) {
        router.push("/login");
        return;
      }
      const u = session.user;
      const staff =
        u.is_platform_staff ||
        u.memberships.some((m) => m.profile_type !== "renter");
      router.push(staff ? "/console" : "/account/lease");
    } catch (e) {
      setError(e instanceof Error ? e.message : "Couldn't set the password");
      setBusy(false);
    }
  }

  if (invalid) {
    return (
      <>
        <h1 className="mb-1 font-display text-2xl font-extrabold">
          This link has expired
        </h1>
        <p className="mb-4 text-sm text-ink-2">
          Links work once and expire — invites after 7 days, resets after 24
          hours. You can ask for a new one.
        </p>
        <Link
          href="/forgot-password"
          className="block rounded-xl bg-accent px-5 py-3 text-center text-sm font-bold text-on-accent"
        >
          Send me a new link
        </Link>
      </>
    );
  }

  if (!info) {
    return (
      <p className="py-6 text-center text-sm text-ink-3">Checking your link…</p>
    );
  }

  const firstName = info.name.split(" ")[0] || "there";
  const invite = info.purpose === "invite";

  return (
    <>
      <h1 className="mb-1 font-display text-2xl font-extrabold">
        {invite ? `Welcome, ${firstName}` : "Choose a new password"}
      </h1>
      <p className="mb-6 text-sm text-ink-3">
        {invite
          ? "Choose a password to finish setting up your account."
          : "Pick something you haven't used here before."}{" "}
        You&apos;ll sign in as{" "}
        <strong className="text-ink-2">{info.email}</strong>.
      </p>
      <form onSubmit={submit} className="space-y-3">
        <input
          type="email"
          hidden
          readOnly
          autoComplete="username"
          value={info.email}
        />
        <input
          className={authField}
          type={show ? "text" : "password"}
          autoComplete="new-password"
          placeholder={`New password (${MIN_PASSWORD_LENGTH}+ characters)`}
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          autoFocus
        />
        <input
          className={authField}
          type={show ? "text" : "password"}
          autoComplete="new-password"
          placeholder="Type it again"
          value={confirm}
          onChange={(e) => setConfirm(e.target.value)}
        />
        <label className="flex items-center gap-2 text-sm text-ink-3">
          <input
            type="checkbox"
            checked={show}
            onChange={(e) => setShow(e.target.checked)}
          />
          Show password
        </label>
        {error && <p className="text-sm text-bad">{error}</p>}
        <Button type="submit" disabled={busy || !password} className="w-full">
          {busy
            ? "Saving…"
            : invite
              ? "Set password and sign in"
              : "Save and sign in"}
        </Button>
      </form>
    </>
  );
}

export default function SetPasswordPage() {
  return (
    <AuthShell>
      <Suspense>
        <SetPasswordForm />
      </Suspense>
    </AuthShell>
  );
}
