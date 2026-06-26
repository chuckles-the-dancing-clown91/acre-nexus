"use client";

// Tenant software settings → Modules.
//
// Lists every pluggable platform module with its description and the permissions
// it governs, and lets a tenant admin (`tenant:manage`) switch each on or off.
// Toggling goes through `useModules().setEnabled`, which persists to the backend
// and updates the shared context so the sidebar reflects the change instantly.

import { useEffect, useState } from "react";
import { useAuth } from "@/lib/auth";
import { useModules } from "@/lib/modules";
import { api, type ModuleInfo } from "@/lib/api";
import { Badge, Card } from "@/components/ui";

export default function ModulesPage() {
  const { can } = useAuth();
  const { enabled, setEnabled } = useModules();
  const [list, setList] = useState<ModuleInfo[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  useEffect(() => {
    api
      .modules()
      .then(setList)
      .catch((e) => setError(e.message));
  }, []);

  if (!can("tenant:manage")) {
    return (
      <div className="text-ink-3">
        You need the <code>tenant:manage</code> permission to manage modules.
      </div>
    );
  }

  async function toggle(key: string, next: boolean) {
    setBusy(key);
    setError(null);
    try {
      await setEnabled(key, next);
      setList(
        (prev) =>
          prev?.map((m) => (m.key === key ? { ...m, enabled: next } : m)) ??
          prev
      );
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(null);
    }
  }

  return (
    <div className="mx-auto max-w-3xl space-y-5">
      <header>
        <h1 className="font-display text-2xl font-bold">Modules</h1>
        <p className="mt-1 text-sm text-ink-3">
          Turn platform capabilities on or off for your workspace. Disabled
          modules hide from navigation and reject their API calls.
        </p>
      </header>

      {error && (
        <div className="rounded-xl border border-bad-soft bg-bad-soft/40 px-4 py-3 text-sm text-bad">
          {error}
        </div>
      )}

      <div className="space-y-3">
        {(list ?? []).map((m) => {
          const on = enabled[m.key] ?? m.enabled;
          return (
            <Card key={m.key} className="flex items-start gap-4 p-4">
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <h2 className="font-display text-base font-bold">{m.name}</h2>
                  {m.preview && <Badge tone="info">Preview</Badge>}
                </div>
                <p className="mt-0.5 text-sm text-ink-3">{m.description}</p>
                {m.permissions.length > 0 && (
                  <div className="mt-2 flex flex-wrap gap-1.5">
                    {m.permissions.map((p) => (
                      <span
                        key={p}
                        className="rounded-md bg-surface-2 px-1.5 py-0.5 font-mono text-[11px] text-ink-2"
                      >
                        {p}
                      </span>
                    ))}
                  </div>
                )}
              </div>
              <Toggle
                on={on}
                disabled={busy === m.key}
                onChange={(next) => toggle(m.key, next)}
              />
            </Card>
          );
        })}
        {!list && !error && <div className="text-ink-3">Loading modules…</div>}
      </div>
    </div>
  );
}

/** Accessible on/off switch. */
function Toggle({
  on,
  disabled,
  onChange,
}: {
  on: boolean;
  disabled?: boolean;
  onChange: (next: boolean) => void;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={on}
      disabled={disabled}
      onClick={() => onChange(!on)}
      className={[
        "relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition disabled:opacity-50",
        on ? "bg-accent" : "bg-line-2",
      ].join(" ")}
    >
      <span
        className={[
          "inline-block h-5 w-5 transform rounded-full bg-white shadow transition",
          on ? "translate-x-5" : "translate-x-0.5",
        ].join(" ")}
      />
    </button>
  );
}
