"use client";

// Workspace settings — the per-tenant configuration catalog (backed by the
// backend `settings` subsystem). Renders each setting generically by kind and
// saves overrides. Gated by `tenant:manage`.

import { useAuth } from "@/lib/auth";
import { useSettings, useSetSetting } from "@/lib/queries";
import type { SettingView } from "@/lib/types";
import { Card } from "@/components/ui";
import { Input } from "@/components/ui/input";

export default function SettingsPage() {
  const { can } = useAuth();
  const { data: settings, isLoading, error } = useSettings();

  if (!can("tenant:manage")) {
    return (
      <Card className="p-6">
        <p className="text-ink-2">
          You don&apos;t have access to workspace settings. Ask an admin for the{" "}
          <span className="font-mono">tenant:manage</span> permission.
        </p>
      </Card>
    );
  }

  // Group settings by their `group` for display.
  const groups = new Map<string, SettingView[]>();
  for (const s of settings ?? []) {
    const list = groups.get(s.group) ?? [];
    list.push(s);
    groups.set(s.group, list);
  }

  return (
    <div className="space-y-6">
      <div>
        <h1 className="font-display text-3xl font-extrabold tracking-tight">
          Settings
        </h1>
        <p className="text-ink-3">
          Workspace-wide configuration for your firm.
        </p>
      </div>

      {error && <p className="text-bad">{error.message}</p>}
      {isLoading && <p className="text-ink-3">Loading…</p>}

      {[...groups.entries()].map(([group, items]) => (
        <Card key={group} className="p-5">
          <h2 className="mb-4 font-display text-lg font-bold">{group}</h2>
          <div className="divide-y divide-line">
            {items.map((s) => (
              <SettingRow key={s.key} setting={s} />
            ))}
          </div>
        </Card>
      ))}

      {settings && settings.length === 0 && (
        <Card className="p-6">
          <p className="text-ink-3">No configurable settings yet.</p>
        </Card>
      )}
    </div>
  );
}

/** One setting rendered by kind: bool → switch, int → number, text → input. */
function SettingRow({ setting }: { setting: SettingView }) {
  const save = useSetSetting();

  const onSave = (value: unknown) => save.mutate({ key: setting.key, value });

  return (
    <div className="flex items-center justify-between gap-4 py-4">
      <div className="min-w-0">
        <div className="font-semibold">{setting.label}</div>
        <div className="text-sm text-ink-3">{setting.description}</div>
      </div>
      <div className="shrink-0">
        {setting.kind === "bool" && (
          <label className="flex cursor-pointer items-center gap-2 text-sm">
            <input
              type="checkbox"
              checked={Boolean(setting.value)}
              disabled={save.isPending}
              onChange={(e) => onSave(e.target.checked)}
              className="h-4 w-4 rounded border-line"
            />
            {Boolean(setting.value) ? "On" : "Off"}
          </label>
        )}
        {setting.kind === "int" && (
          <Input
            type="number"
            defaultValue={String(setting.value ?? "")}
            disabled={save.isPending}
            className="w-28"
            onBlur={(e) => {
              const n = Number(e.target.value);
              if (!Number.isNaN(n) && n !== Number(setting.value)) onSave(n);
            }}
          />
        )}
        {setting.kind === "text" && (
          <Input
            defaultValue={String(setting.value ?? "")}
            disabled={save.isPending}
            className="w-56"
            onBlur={(e) => {
              if (e.target.value !== String(setting.value ?? ""))
                onSave(e.target.value);
            }}
          />
        )}
      </div>
    </div>
  );
}
