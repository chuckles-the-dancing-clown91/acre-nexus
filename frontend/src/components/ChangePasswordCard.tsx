"use client";

// Change the signed-in user's password. Needs the current one; the backend
// signs out every other device when it changes.

import { useState } from "react";
import { toast } from "sonner";
import { api } from "@/lib/api";
import { Card } from "@/components/ui";

export const MIN_PASSWORD_LENGTH = 10;

export function ChangePasswordCard({ className }: { className?: string }) {
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");
  const [confirm, setConfirm] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const field =
    "w-full rounded-lg border border-line bg-surface px-3 py-2 text-sm outline-none focus:border-accent";

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    if (next.length < MIN_PASSWORD_LENGTH) {
      setError(`Use at least ${MIN_PASSWORD_LENGTH} characters.`);
      return;
    }
    if (next !== confirm) {
      setError("The two new passwords don't match.");
      return;
    }
    setBusy(true);
    try {
      await api.passwordChange(current, next);
      setCurrent("");
      setNext("");
      setConfirm("");
      toast.success("Password changed — other devices were signed out");
    } catch (e) {
      setError(e instanceof Error ? e.message : "Couldn't change the password");
    } finally {
      setBusy(false);
    }
  }

  return (
    <Card className={className ?? "p-5"}>
      <h2 className="mb-1 font-display text-lg font-bold">Password</h2>
      <p className="mb-4 text-sm text-ink-3">
        Changing it signs you out everywhere else.
      </p>
      <form onSubmit={submit} className="grid gap-3 sm:grid-cols-3">
        <input
          className={field}
          type="password"
          autoComplete="current-password"
          placeholder="Current password"
          value={current}
          onChange={(e) => setCurrent(e.target.value)}
        />
        <input
          className={field}
          type="password"
          autoComplete="new-password"
          placeholder="New password"
          value={next}
          onChange={(e) => setNext(e.target.value)}
        />
        <input
          className={field}
          type="password"
          autoComplete="new-password"
          placeholder="New password again"
          value={confirm}
          onChange={(e) => setConfirm(e.target.value)}
        />
        {error && <p className="text-sm text-bad sm:col-span-3">{error}</p>}
        <div className="sm:col-span-3">
          <button
            type="submit"
            disabled={busy || !current || !next}
            className="rounded-xl bg-accent px-5 py-2.5 text-sm font-bold text-on-accent disabled:opacity-50"
          >
            {busy ? "Changing…" : "Change password"}
          </button>
        </div>
      </form>
    </Card>
  );
}
