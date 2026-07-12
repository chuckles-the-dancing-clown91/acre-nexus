"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import Link from "next/link";
import { useAuth } from "@/lib/auth";
import { api, DEFAULT_TENANT, isMfaChallenge } from "@/lib/api";
import type { MfaChallenge } from "@/lib/api";
import { Button, Card } from "@/components/ui";
import { ThemeToggle } from "@/components/ThemeToggle";

const DEMO_ACCOUNTS = [
  { label: "Avery Stone — Platform staff", email: "avery@acrehq.com" },
  { label: "Jordan Mills — Northwind admin", email: "jordan@northwind.com" },
  { label: "Priya Rao — Cascade admin", email: "priya@cascade.com" },
];

const PROVIDERS = [
  { key: "google", label: "Google" },
  { key: "microsoft", label: "Microsoft" },
  { key: "apple", label: "Apple" },
];

export default function LoginPage() {
  const { login, establishSession } = useAuth();
  const router = useRouter();
  const [email, setEmail] = useState("jordan@northwind.com");
  const [password, setPassword] = useState("password");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  // MFA step-up state.
  const [challenge, setChallenge] = useState<MfaChallenge | null>(null);
  const [code, setCode] = useState("");

  // Sandbox social-login affordance (no real provider credentials).
  const [sandbox, setSandbox] = useState<{ provider: string; url: string } | null>(
    null
  );
  const [sandboxEmail, setSandboxEmail] = useState("new.renter@example.com");

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const res = await login(email, password);
      if (isMfaChallenge(res)) {
        setChallenge(res);
      } else {
        router.push("/console");
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : "Login failed");
    } finally {
      setBusy(false);
    }
  }

  async function submitMfa(e: React.FormEvent) {
    e.preventDefault();
    if (!challenge) return;
    setBusy(true);
    setError(null);
    try {
      const tokens = await api.mfaVerify(challenge.mfa_token, code.trim());
      establishSession(tokens);
      router.push("/console");
    } catch {
      setError("That code isn't valid — try again.");
    } finally {
      setBusy(false);
    }
  }

  async function social(provider: string) {
    setError(null);
    try {
      const res = await api.oauthStart(provider, {
        intent: "login",
        tenant: DEFAULT_TENANT,
      });
      if (res.sandbox) {
        // No live credentials: collect the simulated account email inline.
        setSandbox({ provider, url: res.authorize_url });
      } else {
        window.location.assign(res.authorize_url);
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : "Couldn't start social login");
    }
  }

  const field =
    "w-full rounded-xl border border-line bg-surface-2 px-3.5 py-3 text-sm outline-none focus:border-accent";

  return (
    <main className="flex min-h-screen items-center justify-center px-6">
      <div className="absolute right-5 top-5">
        <ThemeToggle />
      </div>
      <Card className="w-full max-w-md p-8">
        <div className="mb-6 flex items-center gap-2.5">
          <span
            className="flex h-8 w-8 items-center justify-center rounded-[9px] font-display text-lg font-extrabold text-on-accent"
            style={{ background: "var(--accent)" }}
          >
            A
          </span>
          <span className="font-display text-xl font-bold">Acre Console</span>
        </div>

        {challenge ? (
          /* ---- MFA step-up ---- */
          <>
            <h1 className="mb-1 font-display text-2xl font-extrabold">
              Two-factor
            </h1>
            <p className="mb-6 text-sm text-ink-3">
              Enter the 6-digit code from your authenticator app.
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
              <button
                type="button"
                onClick={() => {
                  setChallenge(null);
                  setCode("");
                  setError(null);
                }}
                className="w-full text-center text-sm text-ink-3 hover:text-ink"
              >
                ← Back
              </button>
            </form>
          </>
        ) : sandbox ? (
          /* ---- Sandbox social sign-in ---- */
          <>
            <h1 className="mb-1 font-display text-2xl font-extrabold capitalize">
              {sandbox.provider} (sandbox)
            </h1>
            <p className="mb-6 text-sm text-ink-3">
              No live {sandbox.provider} credentials are configured, so this is a
              simulated sign-in. Enter an email to sign in (or provision) with.
            </p>
            <form
              onSubmit={(e) => {
                e.preventDefault();
                window.location.assign(
                  sandbox.url + "&email=" + encodeURIComponent(sandboxEmail)
                );
              }}
              className="space-y-3"
            >
              <input
                className={field}
                type="email"
                value={sandboxEmail}
                onChange={(e) => setSandboxEmail(e.target.value)}
                autoFocus
              />
              <Button type="submit" className="w-full">
                Continue
              </Button>
              <button
                type="button"
                onClick={() => setSandbox(null)}
                className="w-full text-center text-sm text-ink-3 hover:text-ink"
              >
                ← Back
              </button>
            </form>
          </>
        ) : (
          /* ---- Password + social ---- */
          <>
            <h1 className="mb-1 font-display text-2xl font-extrabold">
              Welcome back
            </h1>
            <p className="mb-6 text-sm text-ink-3">Sign in to your workspace.</p>

            <div className="mb-4 grid grid-cols-3 gap-2">
              {PROVIDERS.map((p) => (
                <button
                  key={p.key}
                  onClick={() => social(p.key)}
                  className="rounded-xl border border-line bg-surface-2 px-2 py-2.5 text-sm font-semibold hover:border-accent"
                >
                  {p.label}
                </button>
              ))}
            </div>
            <div className="mb-4 flex items-center gap-3 text-xs text-ink-3">
              <span className="h-px flex-1 bg-line" />
              or
              <span className="h-px flex-1 bg-line" />
            </div>

            <form onSubmit={submit} className="space-y-3">
              <input
                className={field}
                type="email"
                placeholder="Email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
              />
              <input
                className={field}
                type="password"
                placeholder="Password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
              />
              {error && <p className="text-sm text-bad">{error}</p>}
              <Button type="submit" disabled={busy} className="w-full">
                {busy ? "Signing in…" : "Sign in"}
              </Button>
            </form>

            <div className="mt-6 border-t border-line pt-4">
              <p className="mb-2 text-xs font-semibold uppercase tracking-wide text-ink-3">
                Demo accounts (password: <code>password</code>)
              </p>
              <div className="space-y-1.5">
                {DEMO_ACCOUNTS.map((a) => (
                  <button
                    key={a.email}
                    onClick={() => setEmail(a.email)}
                    className="block w-full rounded-lg px-2 py-1.5 text-left text-sm text-ink-2 hover:bg-surface-2"
                  >
                    {a.label}
                  </button>
                ))}
              </div>
            </div>

            <Link
              href="/"
              className="mt-4 block text-center text-sm font-semibold text-ink-3 hover:text-ink"
            >
              ← Back to website
            </Link>
          </>
        )}
      </Card>
    </main>
  );
}
