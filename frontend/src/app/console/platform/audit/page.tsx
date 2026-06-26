"use client";

// Audit log: a read-only feed of recent platform/tenant actions. Gated by the
// "audit:read" permission. Supports a simple client-side filter by action via a
// select of the distinct actions present in the loaded page.

import { useMemo, useState } from "react";
import { useAuth } from "@/lib/auth";
import { humanizeKey } from "@/lib/iam";
import { useAudit } from "@/lib/queries";
import { Badge, Card } from "@/components/ui";

/** Format an ISO timestamp into a compact, locale-aware date-time string. */
function formatTimestamp(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

/** Platform audit-log viewer. */
export default function AuditPage() {
  const { can } = useAuth();
  const [action, setAction] = useState<string>("");
  const { data, error, isLoading } = useAudit({ limit: 200 });

  // Distinct actions across the loaded entries, for the filter dropdown.
  const actions = useMemo(() => {
    if (!data) return [] as string[];
    return Array.from(new Set(data.map((e) => e.action))).sort();
  }, [data]);

  const entries = useMemo(
    () => (action ? (data ?? []).filter((e) => e.action === action) : data),
    [data, action]
  );

  if (!can("audit:read")) {
    return (
      <Card className="p-6">
        <p className="text-ink-2">
          You don&apos;t have access to the audit log. Ask a platform admin to
          grant the <span className="font-mono">audit:read</span> permission.
        </p>
      </Card>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 className="font-display text-3xl font-extrabold tracking-tight">
            Audit log
          </h1>
          <p className="text-ink-3">
            Recent actions across the platform and client workspaces.
          </p>
        </div>
        {actions.length > 0 && (
          <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
            Filter by action
            <select
              value={action}
              onChange={(e) => setAction(e.target.value)}
              className="rounded-xl border border-line bg-surface px-3 py-2 text-sm font-normal text-ink"
            >
              <option value="">All actions</option>
              {actions.map((a) => (
                <option key={a} value={a}>
                  {a}
                </option>
              ))}
            </select>
          </label>
        )}
      </div>

      {error && <p className="text-bad">{error.message}</p>}

      <Card className="overflow-hidden">
        {isLoading ? (
          <div className="px-5 py-10 text-center text-ink-3">Loading…</div>
        ) : !entries || entries.length === 0 ? (
          <div className="px-5 py-10 text-center text-ink-3">
            {action ? "No entries match this action." : "No audit entries yet."}
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-line text-left text-xs font-bold uppercase tracking-wide text-ink-3">
                  <th className="px-5 py-3">Action</th>
                  <th className="px-5 py-3">Actor</th>
                  <th className="px-5 py-3">Target</th>
                  <th className="px-5 py-3 text-right">When</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-line">
                {entries.map((e) => (
                  <tr key={e.id}>
                    <td className="px-5 py-3">
                      <Badge tone="info">{e.action}</Badge>
                    </td>
                    <td className="px-5 py-3">
                      {e.actor_name ?? (
                        <span className="text-ink-3">System</span>
                      )}
                    </td>
                    <td className="px-5 py-3 text-ink-2">
                      {e.target_type ? (
                        <span>
                          {humanizeKey(e.target_type)}
                          {e.target_id && (
                            <span className="ml-1 font-mono text-xs text-ink-3">
                              {e.target_id}
                            </span>
                          )}
                        </span>
                      ) : (
                        <span className="text-ink-3">—</span>
                      )}
                    </td>
                    <td className="whitespace-nowrap px-5 py-3 text-right font-mono text-xs text-ink-3">
                      {formatTimestamp(e.created_at)}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </Card>
    </div>
  );
}
