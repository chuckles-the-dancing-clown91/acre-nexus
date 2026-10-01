"use client";

// Choose a password from an invite ("set up your account") or a reset link,
// then sign straight in: residents land in their portal, staff in the console.

import { Suspense, useEffect, useState } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import { LinkIcon } from "lucide-react";
import { api, isMfaChallenge, type PasswordLinkInfo } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { MIN_PASSWORD_LENGTH } from "@/lib/password";
import { AuthFrame } from "@/components/gate/AuthFrame";
import { BackToSignIn } from "@/components/gate/BackToSignIn";
import { Button } from "@/components/ui/button";
import { Field, Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/misc";

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
        `Use at least ${MIN_PASSWORD_LENGTH} characters. A short phrase is easiest to remember.`
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
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Couldn't set the password"
      );
      setBusy(false);
    }
  }

  if (invalid) {
    return (
      <>
        <div className="mb-5 flex size-11 items-center justify-center rounded-2xl border border-warn/30 bg-warn/10 text-warn">
          <LinkIcon className="size-5" />
        </div>
        <h1 className="text-2xl font-semibold text-fg">
          This link has expired
        </h1>
        <p className="mt-2 mb-6 text-[13px] text-fg-2">
          Links work once and expire: invites after 7 days, resets after 24
          hours. You can ask for a new one.
        </p>
        <Button
          size="lg"
          className="w-full"
          onClick={() => router.push("/forgot-password")}
        >
          Send me a new link
        </Button>
      </>
    );
  }

  if (!info) {
    return (
      <div className="space-y-3" aria-label="Checking your link">
        <Skeleton className="h-7 w-2/3" />
        <Skeleton className="h-4 w-full" />
        <Skeleton className="h-11 w-full" />
        <Skeleton className="h-11 w-full" />
      </div>
    );
  }

  const firstName = info.name.split(" ")[0] || "there";
  const invite = info.purpose === "invite";

  return (
    <form onSubmit={submit} className="space-y-4">
      <div>
        <h1 className="text-2xl font-semibold text-fg">
          {invite ? `Welcome, ${firstName}` : "Choose a new password"}
        </h1>
        <p className="mt-1.5 text-[13px] text-fg-3">
          {invite
            ? "Choose a password to finish setting up your account."
            : "Pick something you haven't used here before."}{" "}
          You&apos;ll sign in as{" "}
          <strong className="text-fg-2">{info.email}</strong>.
        </p>
      </div>
      <input
        type="email"
        hidden
        readOnly
        autoComplete="username"
        value={info.email}
      />
      <Field
        label="New password"
        hint={`At least ${MIN_PASSWORD_LENGTH} characters`}
      >
        {(p) => (
          <Input
            {...p}
            type={show ? "text" : "password"}
            autoComplete="new-password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            autoFocus
          />
        )}
      </Field>
      <Field label="Type it again" error={error}>
        {(p) => (
          <Input
            {...p}
            type={show ? "text" : "password"}
            autoComplete="new-password"
            value={confirm}
            onChange={(e) => setConfirm(e.target.value)}
          />
        )}
      </Field>
      <label className="flex items-center gap-2 text-[13px] text-fg-3">
        <input
          type="checkbox"
          className="size-4 accent-[var(--accent)]"
          checked={show}
          onChange={(e) => setShow(e.target.checked)}
        />
        Show password
      </label>
      <Button
        type="submit"
        size="lg"
        className="w-full"
        loading={busy}
        disabled={!password}
      >
        {invite ? "Set password and sign in" : "Save and sign in"}
      </Button>
    </form>
  );
}

export default function SetPasswordPage() {
  return (
    <AuthFrame>
      <Suspense>
        <SetPasswordForm />
      </Suspense>
      <BackToSignIn />
    </AuthFrame>
  );
}
