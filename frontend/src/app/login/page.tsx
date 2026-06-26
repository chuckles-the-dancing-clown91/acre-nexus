"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import Link from "next/link";
import { useAuth } from "@/lib/auth";
import { Button, Card } from "@/components/ui";
import { ThemeToggle } from "@/components/ThemeToggle";

const DEMO_ACCOUNTS = [
  { label: "Avery Stone — Platform staff", email: "avery@acrehq.com" },
  { label: "Jordan Mills — Northwind admin", email: "jordan@northwind.com" },
  { label: "Priya Rao — Cascade admin", email: "priya@cascade.com" },
];

export default function LoginPage() {
  const { login } = useAuth();
  const router = useRouter();
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
      router.push("/console");
    } catch (e) {
      setError(e instanceof Error ? e.message : "Login failed");
    } finally {
      setBusy(false);
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
        <h1 className="mb-1 font-display text-2xl font-extrabold">
          Welcome back
        </h1>
        <p className="mb-6 text-sm text-ink-3">Sign in to your workspace.</p>

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
      </Card>
    </main>
  );
}
