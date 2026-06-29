"use client";

// Acre PLATFORM-ADMIN audit explorer: a read-only feed of the security audit
// trail across the whole platform. The query is server-driven — a limit Select
// (50/100/200) and a debounced action filter feed `useAudit({ limit, action })`.
// Rows come in two flavours: per-request "http.request" entries (method + path
// + status_code) and domain events (action + target_type/target_id). Gated by
// the "audit:read" permission.

import { useEffect, useMemo, useState } from "react";
import { toast } from "sonner";
import { ScrollText, Server, ShieldAlert } from "lucide-react";

import { useAuth } from "@/lib/auth";
import { useAudit } from "@/lib/queries";
import type { AuditEntry } from "@/lib/api";
import { formatDateTime, relativeDate, titleCase } from "@/lib/format";

import { PageHeader, StatCard, EmptyState } from "@/components/ui/page";
import { DataTable, type ColumnDef } from "@/components/ui/data-table";
import { Badge } from "@/components/ui";
import { Input } from "@/components/ui/form-field";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

const LIMITS = [50, 100, 200] as const;
type Limit = (typeof LIMITS)[number];

/** Badge tones, mirrored from the `Badge` component (not exported there). */
type BadgeTone = "neutral" | "good" | "warn" | "bad" | "info" | "accent";

/** Badge tone for an HTTP status code (2xx good, 3xx info, 4xx warn, 5xx bad). */
function statusCodeTone(code: number): BadgeTone {
  if (code >= 500) return "bad";
  if (code >= 400) return "warn";
  if (code >= 300) return "info";
  return "good";
}

/** Friendly label for a non-user principal (no actor name available). */
function principalLabel(kind: string | null): string {
  switch (kind) {
    case "api_token":
      return "API token";
    case "public":
      return "Public";
    case "system":
      return "System";
    default:
      return "—";
  }
}

/** Platform-staff audit-log explorer. */
export default function PlatformAuditPage() {
  const { can } = useAuth();
  const canRead = can("audit:read");

  const [limit, setLimit] = useState<Limit>(100);

  // Debounce the action filter: typing updates `term` immediately, but the
  // server query `action` only follows ~300ms later so we don't refetch on
  // every keystroke.
  const [term, setTerm] = useState("");
  const [action, setAction] = useState("");
  useEffect(() => {
    const id = setTimeout(() => setAction(term.trim()), 300);
    return () => clearTimeout(id);
  }, [term]);

  const audit = useAudit(
    { limit, action: action || undefined },
    { enabled: canRead }
  );
  const rows = useMemo(() => audit.data ?? [], [audit.data]);

  // Surface query errors as a toast (the page itself stays usable).
  const errorMessage = audit.error?.message;
  useEffect(() => {
    if (errorMessage) {
      toast.error("Couldn't load audit log", { description: errorMessage });
    }
  }, [errorMessage]);

  const requestCount = useMemo(
    () => rows.filter((e) => e.method != null && e.path != null).length,
    [rows]
  );

  const columns = useMemo<ColumnDef<AuditEntry, unknown>[]>(
    () => [
      {
        accessorKey: "created_at",
        header: "When",
        cell: ({ row }) => {
          const e = row.original;
          return (
            <span
              className="whitespace-nowrap text-ink-2"
              title={formatDateTime(e.created_at)}
            >
              {relativeDate(e.created_at)}
            </span>
          );
        },
      },
      {
        id: "actor",
        accessorKey: "actor_name",
        header: "Actor",
        cell: ({ row }) => {
          const e = row.original;
          if (e.actor_name) {
            return <span className="font-medium text-ink">{e.actor_name}</span>;
          }
          return (
            <span className="text-ink-3">
              {principalLabel(e.principal_kind)}
            </span>
          );
        },
      },
      {
        accessorKey: "action",
        header: "Action",
        cell: ({ row }) => (
          <Badge tone="info" className="font-mono">
            {row.original.action}
          </Badge>
        ),
      },
      {
        id: "target",
        accessorKey: "target_type",
        header: "Target",
        cell: ({ row }) => {
          const e = row.original;
          if (!e.target_type) return <span className="text-ink-3">—</span>;
          return (
            <span className="flex flex-col">
              <span className="text-ink-2">{titleCase(e.target_type)}</span>
              {e.target_id && (
                <span className="font-mono text-xs text-ink-3">
                  {e.target_id}
                </span>
              )}
            </span>
          );
        },
      },
      {
        id: "request",
        header: "Request",
        cell: ({ row }) => {
          const e = row.original;
          if (e.method == null || e.path == null) {
            return <span className="text-ink-3">—</span>;
          }
          return (
            <span className="flex flex-wrap items-center gap-2">
              <span className="font-mono text-xs font-bold text-ink-3">
                {e.method}
              </span>
              <span className="max-w-[22rem] truncate font-mono text-xs text-ink-2">
                {e.path}
              </span>
              {e.status_code != null && (
                <Badge tone={statusCodeTone(e.status_code)} className="font-mono">
                  {e.status_code}
                </Badge>
              )}
            </span>
          );
        },
      },
    ],
    []
  );

  if (!canRead) {
    return (
      <div className="space-y-6">
        <PageHeader
          eyebrow="Platform admin"
          title="Audit log"
          description="The security audit trail across the platform."
        />
        <EmptyState
          icon={ShieldAlert}
          title="No access to the audit log"
          description={
            "You don't have the audit:read permission. Ask a platform admin to grant it."
          }
        />
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Platform admin"
        title="Audit log"
        description="Security audit trail across the platform — request logs and domain events."
      />

      <div className="grid grid-cols-2 gap-4 sm:grid-cols-3">
        {audit.isLoading ? (
          Array.from({ length: 2 }).map((_, i) => (
            <div key={i} className="skeleton h-[104px] rounded-xl" />
          ))
        ) : (
          <>
            <StatCard
              label="Entries"
              value={rows.length}
              sub={action ? `Matching “${action}”` : `Most recent ${limit}`}
              icon={ScrollText}
            />
            <StatCard
              label="Request logs"
              value={requestCount}
              sub={`of ${rows.length} loaded`}
              icon={Server}
              tone="accent"
            />
          </>
        )}
      </div>

      <div className="flex flex-wrap items-end gap-3">
        <label className="flex flex-col gap-1.5">
          <span className="text-xs font-semibold uppercase tracking-wide text-ink-3">
            Action
          </span>
          <Input
            value={term}
            onChange={(e) => setTerm(e.target.value)}
            placeholder="Filter by action…"
            aria-label="Filter by action"
            className="w-64"
          />
        </label>
        <label className="flex flex-col gap-1.5">
          <span className="text-xs font-semibold uppercase tracking-wide text-ink-3">
            Limit
          </span>
          <Select
            value={String(limit)}
            onValueChange={(v) => setLimit(Number(v) as Limit)}
          >
            <SelectTrigger className="h-10 w-28" aria-label="Result limit">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {LIMITS.map((n) => (
                <SelectItem key={n} value={String(n)}>
                  {n}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </label>
      </div>

      <DataTable<AuditEntry>
        columns={columns}
        data={rows}
        isLoading={audit.isLoading}
        searchPlaceholder="Search loaded entries…"
        emptyState={
          <EmptyState
            className="border-0"
            icon={ScrollText}
            title={action ? "No matching entries" : "No audit entries yet"}
            description={
              action
                ? `No entries match the action “${action}”. Try a different filter.`
                : "Actions across the platform and client workspaces will appear here."
            }
          />
        }
      />
    </div>
  );
}
