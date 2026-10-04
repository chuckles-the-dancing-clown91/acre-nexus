"use client";

// Workspace settings, by group, and the keys that make data live. Each
// setting saves on its own; the vault never shows a key again after it's
// saved, only its last four characters.

import { useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Check, KeyRound, RefreshCw, ShieldCheck, Trash2 } from "lucide-react";
import { toast } from "sonner";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { useSetSetting, useSettings } from "@/lib/queries";
import type { SettingView } from "@/lib/types";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

const field =
  "rounded-xl border border-line bg-surface px-3 py-2 text-[13px] text-fg outline-none focus:border-accent";

/** Settings that are really a choice between a few values. */
const CHOICES: Record<string, { value: string; label: string }[]> = {
  "property_data.crime_provider": [
    { value: "fbi", label: "FBI Crime Data Explorer" },
    { value: "off", label: "Off (sample figures)" },
  ],
  "property_data.records_provider": [
    { value: "simulated", label: "Simulated stand-ins" },
    { value: "rentcast", label: "RentCast" },
  ],
};

/** The keys the vault is for, with where they're used. */
const KNOWN_KEYS: { key: string; label: string; hint: string }[] = [
  {
    key: "fbi.api_key",
    label: "FBI Crime Data Explorer (data.gov)",
    hint: "Free at api.data.gov/signup. Without one, the shared demo key is used, which allows 10 calls an hour.",
  },
  {
    key: "rentcast.api_key",
    label: "RentCast",
    hint: "Parcel, tax years and value estimates. The free plan covers a small portfolio's monthly refresh.",
  },
  {
    key: "google.maps_api_key",
    label: "Google Maps",
    hint: "Street photos, address suggestions and business reviews.",
  },
];

/** The order groups appear in; anything else follows alphabetically. */
const GROUP_ORDER = [
  "Property data",
  "Maintenance",
  "Calendar",
  "Vendors",
  "Applications",
  "Screening",
  "Lease documents",
  "E-signature",
  "Payments",
  "Reminders",
  "Texts",
  "Helpdesk",
  "Team & payroll",
];

export default function SettingsPage() {
  const { can } = useAuth();
  const settings = useSettings();
  const groups = useMemo(() => {
    const m = new Map<string, SettingView[]>();
    for (const s of settings.data ?? [])
      m.set(s.group, [...(m.get(s.group) ?? []), s]);
    return [...m.entries()].sort(([a], [b]) => {
      const ia = GROUP_ORDER.indexOf(a);
      const ib = GROUP_ORDER.indexOf(b);
      if (ia !== -1 || ib !== -1)
        return (ia === -1 ? 99 : ia) - (ib === -1 ? 99 : ib);
      return a.localeCompare(b);
    });
  }, [settings.data]);

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Workspace"
        title="Settings"
        description="How this workspace runs. Each one saves on its own."
      />
      {can("integrations:manage") && <LiveData />}
      {settings.isLoading && <Skeleton className="h-64" />}
      {groups.map(([group, list]) => (
        <Panel key={group}>
          <PanelHeader title={group} />
          <ul className="divide-y divide-line">
            {list.map((s) => (
              <li key={s.key}>
                <SettingRow s={s} />
              </li>
            ))}
          </ul>
        </Panel>
      ))}
    </div>
  );
}

function SettingRow({ s }: { s: SettingView }) {
  const save = useSetSetting();
  const [draft, setDraft] = useState<string>(String(s.value ?? ""));
  const dirty = draft !== String(s.value ?? "");
  const choices = CHOICES[s.key];
  const isDefault = JSON.stringify(s.value) === JSON.stringify(s.default);
  return (
    <div className="flex flex-col gap-2 px-5 py-3 sm:flex-row sm:items-start sm:gap-6">
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-2 text-[13px] font-medium text-fg">
          {s.label}
          {!isDefault && <Badge tone="info">Changed</Badge>}
        </div>
        <p className="mt-0.5 text-[12px] text-fg-3">{s.description}</p>
      </div>
      <div className="flex shrink-0 items-center gap-2">
        {s.kind === "bool" ? (
          <button
            type="button"
            role="switch"
            aria-checked={!!s.value}
            aria-label={s.label}
            disabled={save.isPending}
            onClick={() => save.mutate({ key: s.key, value: !s.value })}
            className={cn(
              "relative h-6 w-11 rounded-full transition",
              s.value ? "bg-accent" : "bg-fill-2"
            )}
          >
            <span
              className={cn(
                "absolute top-0.5 size-5 rounded-full bg-white shadow transition",
                s.value ? "left-[22px]" : "left-0.5"
              )}
            />
          </button>
        ) : choices ? (
          <select
            aria-label={s.label}
            className={field}
            value={String(s.value ?? "")}
            disabled={save.isPending}
            onChange={(e) => save.mutate({ key: s.key, value: e.target.value })}
          >
            {choices.map((c) => (
              <option key={c.value} value={c.value}>
                {c.label}
              </option>
            ))}
          </select>
        ) : (
          <>
            <input
              aria-label={s.label}
              className={cn(field, s.kind === "int" ? "w-28" : "w-64")}
              inputMode={s.kind === "int" ? "numeric" : undefined}
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && dirty) commit();
              }}
            />
            <Button
              size="sm"
              variant="secondary"
              disabled={!dirty || save.isPending}
              onClick={commit}
            >
              <Check />
              Save
            </Button>
          </>
        )}
      </div>
    </div>
  );
  function commit() {
    const value = s.kind === "int" ? Number(draft) : draft;
    if (s.kind === "int" && !Number.isFinite(value as number)) {
      toast.error("That needs to be a number");
      return;
    }
    save.mutate({ key: s.key, value });
  }
}

/** Live data: which sources are real for this workspace, and the vault. */
function LiveData() {
  const qc = useQueryClient();
  const live = useQuery({
    queryKey: ["property-data-live"],
    queryFn: api.propertyDataLive,
  });
  const secrets = useQuery({
    queryKey: ["integration-secrets"],
    queryFn: api.integrationSecrets,
  });
  const [editing, setEditing] = useState<string | null>(null);
  const [value, setValue] = useState("");
  const [custom, setCustom] = useState("");
  const [busy, setBusy] = useState(false);
  const byKey = new Map((secrets.data ?? []).map((s) => [s.key, s]));
  const refresh = () => {
    qc.invalidateQueries({ queryKey: ["integration-secrets"] });
    qc.invalidateQueries({ queryKey: ["property-data-live"] });
  };

  async function saveKey(key: string) {
    if (!value.trim()) return;
    setBusy(true);
    try {
      await api.setIntegrationSecret(key, value.trim());
      toast.success(`Saved ${key}`);
      setEditing(null);
      setValue("");
      setCustom("");
      refresh();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't save the key");
    } finally {
      setBusy(false);
    }
  }
  async function remove(key: string) {
    if (
      !window.confirm(
        `Remove ${key}? Anything using it goes back to sample data.`
      )
    )
      return;
    setBusy(true);
    try {
      await api.deleteIntegrationSecret(key);
      toast.success(`Removed ${key}`);
      refresh();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't remove it");
    } finally {
      setBusy(false);
    }
  }

  const rows = [
    ...KNOWN_KEYS,
    ...(secrets.data ?? [])
      .filter((s) => !KNOWN_KEYS.some((k) => k.key === s.key))
      .map((s) => ({ key: s.key, label: s.key, hint: "" })),
  ];

  return (
    <Panel>
      <PanelHeader
        title={
          <span className="flex items-center gap-2">
            <KeyRound className="size-4" />
            Live data and keys
          </span>
        }
        description="Keys are sealed in the vault; only the last four characters show again."
        action={
          live.data && (
            <div className="flex flex-wrap gap-1.5">
              <Badge tone={live.data.crime_live ? "good" : "neutral"}>
                <ShieldCheck className="mr-1 size-3" />
                Crime stats {live.data.crime_live ? "live" : "sample"}
              </Badge>
              <Badge tone={live.data.records_live ? "good" : "neutral"}>
                Records {live.data.records_live ? "live" : "simulated"}
              </Badge>
            </div>
          )
        }
      />
      <ul className="divide-y divide-line">
        {rows.map((k) => {
          const have = byKey.get(k.key);
          const open = editing === k.key;
          return (
            <li
              key={k.key}
              className="flex flex-col gap-2 px-5 py-3 sm:flex-row sm:items-start sm:gap-6"
            >
              <div className="min-w-0 flex-1">
                <div className="flex flex-wrap items-center gap-2 text-[13px] font-medium text-fg">
                  {k.label}
                  <code className="rounded bg-fill px-1.5 py-0.5 text-[11px] text-fg-3">
                    {k.key}
                  </code>
                  {have ? (
                    <Badge tone="good">Set · ····{have.last4}</Badge>
                  ) : (
                    <Badge tone="neutral">Not set</Badge>
                  )}
                </div>
                {k.hint && (
                  <p className="mt-0.5 text-[12px] text-fg-3">{k.hint}</p>
                )}
              </div>
              <div className="flex shrink-0 items-center gap-2">
                {open ? (
                  <>
                    <input
                      aria-label={`${k.label} key`}
                      className={cn(field, "w-64 font-mono")}
                      type="password"
                      autoComplete="off"
                      placeholder="Paste the key"
                      value={value}
                      onChange={(e) => setValue(e.target.value)}
                      onKeyDown={(e) => e.key === "Enter" && saveKey(k.key)}
                    />
                    <Button
                      size="sm"
                      disabled={busy || !value.trim()}
                      onClick={() => saveKey(k.key)}
                    >
                      Save
                    </Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      onClick={() => setEditing(null)}
                    >
                      Cancel
                    </Button>
                  </>
                ) : (
                  <>
                    <Button
                      size="sm"
                      variant="secondary"
                      onClick={() => {
                        setEditing(k.key);
                        setValue("");
                      }}
                    >
                      {have ? <RefreshCw /> : <KeyRound />}
                      {have ? "Rotate" : "Add key"}
                    </Button>
                    {have && (
                      <Button
                        size="sm"
                        variant="ghost"
                        disabled={busy}
                        onClick={() => remove(k.key)}
                        aria-label={`Remove ${k.key}`}
                      >
                        <Trash2 />
                      </Button>
                    )}
                  </>
                )}
              </div>
            </li>
          );
        })}
        <li className="flex flex-col gap-2 px-5 py-3 sm:flex-row sm:items-center sm:gap-3">
          <input
            aria-label="Another key name"
            className={cn(field, "w-56 font-mono")}
            placeholder="another.key_name"
            value={custom}
            onChange={(e) => setCustom(e.target.value.toLowerCase())}
          />
          <input
            aria-label="Another key value"
            className={cn(field, "w-64 font-mono")}
            type="password"
            autoComplete="off"
            placeholder="Value"
            value={editing === "__custom" ? value : ""}
            onFocus={() => setEditing("__custom")}
            onChange={(e) => setValue(e.target.value)}
          />
          <Button
            size="sm"
            variant="secondary"
            disabled={
              busy ||
              !/^[a-z0-9._-]+$/.test(custom) ||
              !value.trim() ||
              editing !== "__custom"
            }
            onClick={() => saveKey(custom)}
          >
            Add
          </Button>
        </li>
      </ul>
    </Panel>
  );
}
