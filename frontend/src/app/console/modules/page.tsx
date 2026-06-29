"use client";

// Tenant software settings → Modules.
//
// Lists every pluggable platform module with its description and the
// permissions it governs, and lets a tenant admin (`tenant:manage`) switch
// each on or off. Reads come from `useModules()` (ModuleInfo[]) and toggles go
// through `useSetModule()`, which persists to the backend and invalidates the
// module list. Without `tenant:manage` the page is read-only.

import { useMemo } from "react";
import { Blocks, Lock, Puzzle, Sparkles } from "lucide-react";
import { useAuth } from "@/lib/auth";
import { useModules, useSetModule } from "@/lib/queries";
import type { ModuleInfo } from "@/lib/api";
import { PageHeader, StatCard, EmptyState } from "@/components/ui/page";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Switch } from "@/components/ui/switch";
import { Skeleton } from "@/components/ui/skeleton";
import { Badge } from "@/components/ui";

export default function ModulesPage() {
  const { can } = useAuth();
  const canManage = can("tenant:manage");

  const modules = useModules();
  const setModule = useSetModule();

  const list = useMemo(() => modules.data ?? [], [modules.data]);

  const enabledCount = list.filter((m) => m.enabled).length;
  const previewCount = list.filter((m) => m.preview).length;

  // Group preview/beta modules below the generally-available ones.
  const groups = useMemo(() => {
    const core = list.filter((m) => !m.preview);
    const preview = list.filter((m) => m.preview);
    return [
      {
        key: "core",
        title: "Modules",
        description: "Generally-available capabilities for your workspace.",
        items: core,
      },
      {
        key: "preview",
        title: "Preview",
        description:
          "Early-access features. Behaviour and APIs may still change.",
        items: preview,
      },
    ].filter((g) => g.items.length > 0);
  }, [list]);

  function toggle(m: ModuleInfo, next: boolean) {
    if (!canManage) return;
    setModule.mutate({ key: m.key, enabled: next });
  }

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Workspace settings"
        title="Modules"
        description="Turn platform capabilities on or off per tenant. Disabled modules hide from navigation and reject their API calls."
      />

      {/* Summary */}
      <div className="grid grid-cols-2 gap-4 lg:grid-cols-3">
        {modules.isLoading ? (
          Array.from({ length: 3 }).map((_, i) => (
            <Skeleton key={i} className="h-[104px] rounded-xl" />
          ))
        ) : (
          <>
            <StatCard
              label="Enabled"
              value={`${enabledCount} / ${list.length}`}
              sub="Active for this workspace"
              icon={Blocks}
              tone="accent"
            />
            <StatCard
              label="Available"
              value={list.length}
              sub="Total platform modules"
              icon={Puzzle}
            />
            <StatCard
              label="Preview"
              value={previewCount}
              sub="Early-access features"
              icon={Sparkles}
            />
          </>
        )}
      </div>

      {/* Read-only notice for users without tenant:manage */}
      {!canManage && !modules.isLoading && (
        <div className="flex items-start gap-3 rounded-xl border border-line bg-surface-2/60 px-4 py-3 text-sm text-ink-2">
          <Lock className="mt-0.5 h-4 w-4 shrink-0 text-ink-3" />
          <span>
            You&rsquo;re viewing modules in read-only mode. The{" "}
            <code className="rounded bg-surface px-1 py-0.5 font-mono text-xs text-ink">
              tenant:manage
            </code>{" "}
            permission is required to change them.
          </span>
        </div>
      )}

      {/* Loading skeletons */}
      {modules.isLoading ? (
        <div className="space-y-6">
          {Array.from({ length: 2 }).map((_, g) => (
            <Card key={g}>
              <CardHeader>
                <Skeleton className="h-4 w-32" />
              </CardHeader>
              <CardContent className="space-y-3 p-4">
                {Array.from({ length: 3 }).map((_, i) => (
                  <Skeleton key={i} className="h-16 rounded-lg" />
                ))}
              </CardContent>
            </Card>
          ))}
        </div>
      ) : list.length === 0 ? (
        <EmptyState
          icon={Puzzle}
          title="No modules available"
          description="There are no pluggable modules configured for this platform yet."
        />
      ) : (
        <div className="space-y-6">
          {groups.map((group) => (
            <Card key={group.key}>
              <CardHeader className="flex-col items-start gap-1">
                <CardTitle>{group.title}</CardTitle>
                <CardDescription>{group.description}</CardDescription>
              </CardHeader>
              <CardContent className="divide-y divide-line p-0">
                {group.items.map((m) => (
                  <ModuleRow
                    key={m.key}
                    module={m}
                    canManage={canManage}
                    pending={
                      setModule.isPending &&
                      setModule.variables?.key === m.key
                    }
                    onToggle={(next) => toggle(m, next)}
                  />
                ))}
              </CardContent>
            </Card>
          ))}
        </div>
      )}
    </div>
  );
}

/** A single module row: name + description + permissions on the left, Switch right. */
function ModuleRow({
  module: m,
  canManage,
  pending,
  onToggle,
}: {
  module: ModuleInfo;
  canManage: boolean;
  pending: boolean;
  onToggle: (next: boolean) => void;
}) {
  return (
    <div className="flex items-start gap-4 px-5 py-4">
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-2">
          <h3 className="font-display text-sm font-semibold text-ink">
            {m.name}
          </h3>
          {m.preview && <Badge tone="info">Preview</Badge>}
          {!m.enabled && <Badge tone="neutral">Off</Badge>}
        </div>
        <p className="mt-0.5 text-sm text-ink-2">{m.description}</p>
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
      <Switch
        checked={m.enabled}
        disabled={!canManage || pending}
        onCheckedChange={onToggle}
        aria-label={`${m.enabled ? "Disable" : "Enable"} ${m.name}`}
      />
    </div>
  );
}
