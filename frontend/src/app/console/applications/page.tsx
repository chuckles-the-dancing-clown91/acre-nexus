"use client";

import { useMemo } from "react";
import { ClipboardList, Inbox } from "lucide-react";
import { useApplications } from "@/lib/queries";
import type { Application } from "@/lib/types";
import { PageHeader, StatCard, EmptyState } from "@/components/ui/page";
import { DataTable, type ColumnDef } from "@/components/ui/data-table";
import { Badge, statusTone } from "@/components/ui";
import { formatDate } from "@/lib/format";

const columns: ColumnDef<Application>[] = [
  {
    accessorKey: "applicant_name",
    header: "Applicant",
    cell: ({ row }) => {
      const a = row.original;
      return (
        <div className="min-w-0">
          <div className="font-medium text-ink">{a.applicant_name}</div>
          <div className="truncate text-xs text-ink-3">{a.email}</div>
        </div>
      );
    },
  },
  {
    accessorKey: "listing_id",
    header: "Listing",
    cell: ({ row }) => (
      <span className="text-ink-2">
        {row.original.listing_id ?? "—"}
      </span>
    ),
  },
  {
    accessorKey: "annual_income_label",
    header: "Income",
    cell: ({ row }) => (
      <span data-numeric className="text-ink-2">
        {row.original.annual_income_label
          ? `${row.original.annual_income_label}/yr`
          : "—"}
      </span>
    ),
  },
  {
    accessorKey: "credit_score",
    header: "Credit",
    cell: ({ row }) => (
      <span data-numeric className="text-ink-2">
        {row.original.credit_score ?? "—"}
      </span>
    ),
  },
  {
    accessorKey: "status",
    header: "Status",
    cell: ({ row }) => (
      <Badge tone={statusTone(row.original.status)}>{row.original.status}</Badge>
    ),
  },
  {
    accessorKey: "move_in",
    header: "Move-in",
    cell: ({ row }) => (
      <span data-numeric className="text-ink-2">
        {formatDate(row.original.move_in) || "—"}
      </span>
    ),
  },
];

export default function ApplicationsPage() {
  const apps = useApplications();
  const data = useMemo(() => apps.data ?? [], [apps.data]);

  const newCount = useMemo(
    () =>
      data.filter((a) => {
        const s = a.status.toLowerCase();
        return s === "new" || s === "pending" || s === "screening";
      }).length,
    [data]
  );
  const approvedCount = useMemo(
    () => data.filter((a) => a.status.toLowerCase() === "approved").length,
    [data]
  );

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Leasing"
        title="Applications"
        description="Rental applications submitted through your public listings, screened automatically."
      />

      <div className="grid grid-cols-2 gap-4 lg:grid-cols-3">
        {apps.isLoading ? (
          Array.from({ length: 3 }).map((_, i) => (
            <div key={i} className="skeleton h-[104px] rounded-xl" />
          ))
        ) : (
          <>
            <StatCard
              label="Applications"
              value={data.length}
              sub="All time"
              icon={ClipboardList}
            />
            <StatCard
              label="Awaiting review"
              value={newCount}
              sub="New, pending or screening"
              icon={Inbox}
              tone={newCount > 0 ? "warn" : "neutral"}
            />
            <StatCard
              label="Approved"
              value={approvedCount}
              sub="Cleared screening"
              icon={ClipboardList}
              tone={approvedCount > 0 ? "good" : "neutral"}
            />
          </>
        )}
      </div>

      <DataTable<Application>
        columns={columns}
        data={data}
        isLoading={apps.isLoading}
        searchPlaceholder="Search applicants…"
        emptyState={
          <EmptyState
            className="border-0"
            icon={Inbox}
            title="No applications yet"
            description="Applications submitted from your public listings will appear here."
          />
        }
      />
    </div>
  );
}
