"use client";

import { Suspense, useState } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import Link from "next/link";
import {
  ArrowRight,
  Building2,
  Loader2,
  ShieldCheck,
  Sparkles,
  Wrench,
} from "lucide-react";
import { useAuth } from "@/lib/auth";
import { Button } from "@/components/ui/button";

const DEMO_ACCOUNTS = [
  { label: "Avery Stone", role: "Acre platform staff", email: "avery@acrehq.com" },
  { label: "Jordan Mills", role: "Northwind admin", email: "jordan@northwind.com" },
  { label: "Priya Rao", role: "Cascade admin", email: "priya@cascade.com" },
];

const HIGHLIGHTS = [
  { icon: Building2, text: "Portfolio, leasing, and rent roll in one place" },
  { icon: Wrench, text: "Maintenance work orders from request to resolved" },
  { icon: ShieldCheck, text: "Entities, title, and LLC onboarding built in" },
];

function LoginForm() {
  const { login } = useAuth();
  const router = useRouter();
  const params = useSearchParams();
  const next = params.get("next") || "/console";

  const [email, setEmail] = useState("jordan@northwind.com");
  const [password, setPassword] = useState("password");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      await login(email, password);
      router.push(next);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Login failed");
    } finally {
      setBusy(false);
    }
  }

  const fieldCls =
    "w-full rounded-lg border border-line bg-surface px-3.5 py-2.5 text-sm text-ink outline-none transition focus:border-accent focus:ring-2 focus:ring-accent/20 placeholder:text-ink-3";

  return (
    <div className="w-full max-w-sm">
      <div className="mb-7 flex items-center gap-2.5">
        <span
          className="flex h-9 w-9 items-center justify-center rounded-lg font-display text-lg font-bold text-on-accent"
          style={{ background: "var(--accent)" }}
        >
          A
        </span>
        <span className="font-display text-lg font-bold tracking-tight text-ink">
          Acre
        </span>
      </div>

      <h1 className="font-display text-2xl font-bold tracking-tight text-ink">
        Sign in
      </h1>
      <p className="mt-1 text-sm text-ink-2">
        Welcome back. Enter your details to continue.
      </p>

      <form onSubmit={submit} className="mt-6 space-y-3.5">
        <div className="space-y-1.5">
          <label className="text-xs font-semibold text-ink-2">Email</label>
          <input
            className={fieldCls}
            type="email"
            autoComplete="email"
            placeholder="you@company.com"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
          />
        </div>
        <div className="space-y-1.5">
          <label className="text-xs font-semibold text-ink-2">Password</label>
          <input
            className={fieldCls}
            type="password"
            autoComplete="current-password"
            placeholder="••••••••"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
          />
        </div>
        {error && (
          <p className="rounded-lg bg-bad-soft px-3 py-2 text-sm text-bad">
            {error}
          </p>
        )}
        <Button type="submit" disabled={busy} className="w-full" size="lg">
          {busy ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <>
              Sign in
              <ArrowRight className="h-4 w-4" />
            </>
          )}
        </Button>
      </form>

      <div className="mt-7 rounded-xl border border-line bg-surface-2/50 p-3">
        <div className="mb-2 flex items-center gap-1.5 text-xs font-semibold text-ink-3">
          <Sparkles className="h-3.5 w-3.5" />
          Demo accounts · password{" "}
          <code className="rounded bg-surface px-1 font-mono text-[11px] text-ink-2">
            password
          </code>
        </div>
        <div className="space-y-1">
          {DEMO_ACCOUNTS.map((a) => (
            <button
              key={a.email}
              type="button"
              onClick={() => setEmail(a.email)}
              className="flex w-full items-center justify-between rounded-lg px-2.5 py-1.5 text-left text-sm transition hover:bg-surface"
            >
              <span className="font-medium text-ink">{a.label}</span>
              <span className="text-xs text-ink-3">{a.role}</span>
            </button>
          ))}
        </div>
      </div>

      <Link
        href="/"
        className="mt-5 block text-center text-sm font-medium text-ink-3 transition hover:text-ink"
      >
        ← Back to website
      </Link>
    </div>
  );
}

export default function LoginPage() {
  return (
    <main className="flex min-h-screen">
      {/* Brand panel */}
      <div
        className="relative hidden w-1/2 flex-col justify-between overflow-hidden p-12 text-on-accent lg:flex"
        style={{
          background:
            "linear-gradient(155deg, var(--accent-2) 0%, #0a3a23 70%, #07291a 100%)",
        }}
      >
        <div
          className="pointer-events-none absolute inset-0 opacity-[0.07]"
          style={{
            backgroundImage:
              "radial-gradient(circle at 1px 1px, #fff 1px, transparent 0)",
            backgroundSize: "28px 28px",
          }}
        />
        <div className="relative font-display text-lg font-bold">Acre</div>
        <div className="relative max-w-md">
          <h2 className="font-display text-3xl font-bold leading-tight tracking-tight">
            The operating system for property management.
          </h2>
          <p className="mt-3 text-sm text-on-accent/70">
            One workspace for your portfolio, your team, and your tenants —
            built for the companies that run real estate.
          </p>
          <ul className="mt-8 space-y-3">
            {HIGHLIGHTS.map((h) => (
              <li key={h.text} className="flex items-center gap-3 text-sm">
                <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-white/10">
                  <h.icon className="h-4 w-4" />
                </span>
                <span className="text-on-accent/90">{h.text}</span>
              </li>
            ))}
          </ul>
        </div>
        <div className="relative text-xs text-on-accent/50">
          © Acre — multi-tenant property operations
        </div>
      </div>

      {/* Form panel */}
      <div className="flex w-full items-center justify-center px-6 lg:w-1/2">
        <Suspense
          fallback={<Loader2 className="h-5 w-5 animate-spin text-ink-3" />}
        >
          <LoginForm />
        </Suspense>
      </div>
    </main>
  );
}
