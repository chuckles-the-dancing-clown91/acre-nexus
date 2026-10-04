"use client";

// The resident's lease in one place: term, rent and balance, every document
// filed on it (signed lease, receipts, statements), the security deposit and
// its move-out settlement, and the move-in and move-out inspection reports.

import { useState } from "react";
import Link from "next/link";
import { useQuery } from "@tanstack/react-query";
import {
  ChevronDown,
  ClipboardCheck,
  Download,
  FileText,
  Home,
  ShieldCheck,
} from "lucide-react";
import { toast } from "sonner";
import {
  api,
  ApiError,
  type DocumentEntry,
  type InspectionDetail,
  type LeaseDeposit,
  type MyLease,
} from "@/lib/api";
import {
  categoryWords,
  conditionTone,
  day,
  settlementWords,
} from "@/lib/portal-format";
import { Badge, statusTone } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

const noRetry4xx = (n: number, e: Error) =>
  !(e instanceof ApiError && e.status < 500) && n < 2;

async function download(id: string) {
  try {
    const { url } = await api.myDocumentDownloadUrl(id);
    window.open(url, "_blank", "noopener");
  } catch {
    toast.error("Download failed. Try again in a moment.");
  }
}

export default function LeasePage() {
  const lease = useQuery({
    queryKey: ["my-lease"],
    queryFn: api.myLease,
    retry: noRetry4xx,
  });
  const ok = lease.isSuccess;
  const documents = useQuery({
    queryKey: ["my-documents"],
    queryFn: api.myDocuments,
    enabled: ok,
    retry: noRetry4xx,
  });
  const deposit = useQuery({
    queryKey: ["my-deposit"],
    queryFn: api.myDeposit,
    enabled: ok,
    retry: noRetry4xx,
  });
  const inspections = useQuery({
    queryKey: ["my-inspections"],
    queryFn: api.myInspections,
    enabled: ok,
    retry: noRetry4xx,
  });

  if (lease.isLoading) return <Skeleton className="h-64" />;
  if (lease.error || !lease.data) {
    const none = lease.error instanceof ApiError && lease.error.status === 404;
    return (
      <Panel>
        <EmptyState
          icon={<Home />}
          title={none ? "No lease on file yet" : "Couldn't load your lease"}
          description={
            none || !lease.error
              ? "Once your lease is set up with this email, its details and documents show up here."
              : lease.error.message
          }
        />
      </Panel>
    );
  }

  const l = lease.data;
  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-[24px] font-semibold text-fg">Lease</h1>
        <p className="text-[13px] text-fg-3">
          {l.property_name}
          {l.unit_label && ` · Unit ${l.unit_label}`}
        </p>
      </div>

      <Summary lease={l} />

      <section>
        <div className="eyebrow mb-2">Documents</div>
        {documents.isLoading ? (
          <Skeleton className="h-24" />
        ) : (
          <Documents rows={documents.data ?? []} />
        )}
      </section>

      {deposit.data && (
        <section>
          <div className="eyebrow mb-2">Security deposit</div>
          <Deposit deposit={deposit.data} />
        </section>
      )}

      {inspections.data && inspections.data.length > 0 && (
        <section>
          <div className="eyebrow mb-2">Inspections</div>
          <Inspections rows={inspections.data} />
        </section>
      )}
    </div>
  );
}

function Summary({ lease: l }: { lease: MyLease }) {
  return (
    <Panel className="p-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="text-[15px] font-semibold text-fg">
            {l.tenant_name}
          </div>
          <div className="text-[13px] text-fg-3">{l.property_address}</div>
        </div>
        <div className="flex flex-wrap gap-1.5">
          <Badge tone={statusTone(l.status)}>
            {l.status.replace(/_/g, " ")}
          </Badge>
          <Badge tone={statusTone(l.payment_status)}>
            {l.payment_status.replace(/_/g, " ")}
          </Badge>
        </div>
      </div>
      <dl className="mt-4 grid grid-cols-2 gap-x-3 gap-y-4">
        <Figure
          label="Term"
          value={`${day(l.start_date)} to ${
            l.end_date ? day(l.end_date) : "month to month"
          }`}
          wide
        />
        <Figure label="Monthly rent" value={l.rent_label} />
        <Figure
          label="Balance"
          value={l.balance_label}
          tone={l.balance_cents > 0 ? "text-warn" : undefined}
        />
      </dl>
      {l.balance_cents > 0 && (
        <Link
          href="/account/payments"
          className="mt-3 inline-block text-[13px] text-accent hover:underline"
        >
          Pay rent
        </Link>
      )}
    </Panel>
  );
}

function Figure({
  label,
  value,
  wide,
  tone,
}: {
  label: string;
  value: string;
  wide?: boolean;
  tone?: string;
}) {
  return (
    <div className={cn(wide && "col-span-2")}>
      <dt className="text-[11px] text-fg-3">{label}</dt>
      <dd className={cn("figure text-[16px] font-semibold", tone ?? "text-fg")}>
        {value}
      </dd>
    </div>
  );
}

function Documents({ rows }: { rows: DocumentEntry[] }) {
  if (rows.length === 0)
    return (
      <Panel>
        <EmptyState
          icon={<FileText />}
          title="Nothing filed yet"
          description="Your signed lease and rent receipts show up here."
        />
      </Panel>
    );
  return (
    <Panel className="divide-y divide-line">
      {rows.map((d) => (
        <div key={d.id} className="flex items-center gap-3 px-4 py-3">
          <FileText className="size-4 shrink-0 text-fg-3" />
          <div className="min-w-0 flex-1">
            <div className="truncate text-[14px] font-medium text-fg">
              {d.filename}
            </div>
            <div className="text-xs text-fg-3">
              {categoryWords(d.category)} · {day(d.created_at)}
            </div>
          </div>
          <Button
            size="sm"
            variant="secondary"
            disabled={d.status !== "stored"}
            onClick={() => void download(d.id)}
            aria-label={`Download ${d.filename}`}
          >
            <Download />
            <span className="hidden sm:inline">Download</span>
          </Button>
        </div>
      ))}
    </Panel>
  );
}

function Deposit({ deposit }: { deposit: LeaseDeposit }) {
  const d = deposit.disposition;
  if (!deposit.deposit_label)
    return (
      <Panel className="p-4 text-[13px] text-fg-3">
        This lease has no security deposit.
      </Panel>
    );
  return (
    <Panel className="space-y-3 p-4">
      <div className="flex items-start justify-between gap-3">
        <div className="flex min-w-0 items-start gap-3">
          <ShieldCheck className="mt-0.5 size-5 shrink-0 text-fg-3" />
          <div>
            <div className="figure text-[18px] font-semibold text-fg">
              {deposit.deposit_label}
            </div>
            <div className="text-xs text-fg-3">
              {deposit.deposit_paid
                ? "Held in a trust account for you."
                : "Not paid yet."}
            </div>
          </div>
        </div>
        <Badge tone={deposit.deposit_paid ? "good" : "warn"}>
          {deposit.deposit_paid ? "Held in trust" : "Not paid"}
        </Badge>
      </div>
      {!deposit.deposit_paid && (
        <Link
          href="/account/payments"
          className="inline-block text-[13px] text-accent hover:underline"
        >
          Pay it from Rent
        </Link>
      )}
      {d && (
        <div className="space-y-2 rounded-xl border border-line bg-fill/40 p-3">
          <div className="flex items-center justify-between gap-2">
            <span className="text-[14px] font-medium text-fg">
              Move-out settlement
            </span>
            <Badge
              tone={
                d.status === "closed"
                  ? "good"
                  : d.status === "failed"
                    ? "bad"
                    : d.status === "processing"
                      ? "info"
                      : "neutral"
              }
            >
              {settlementWords(d.status)}
            </Badge>
          </div>
          {d.deductions.length > 0 && (
            <ul className="space-y-1 text-[13px] text-fg-2">
              {d.deductions.map((x) => (
                <li key={x.id} className="flex justify-between gap-3">
                  <span className="min-w-0">{x.description}</span>
                  <span className="figure shrink-0">-{x.amount_label}</span>
                </li>
              ))}
            </ul>
          )}
          <div className="flex justify-between gap-3 border-t border-line pt-2 text-[14px] font-semibold text-fg">
            <span>Refund to you</span>
            <span className="figure">{d.refund_label ?? "Not set yet"}</span>
          </div>
          {d.status === "failed" && d.failure_reason && (
            <p className="text-xs text-bad">{d.failure_reason}</p>
          )}
          {d.statement_document_id && (
            <Button
              size="sm"
              variant="secondary"
              onClick={() => void download(d.statement_document_id!)}
            >
              <Download />
              Download statement
            </Button>
          )}
        </div>
      )}
    </Panel>
  );
}

function Inspections({ rows }: { rows: InspectionDetail[] }) {
  const [open, setOpen] = useState<string | null>(null);
  return (
    <Panel className="divide-y divide-line">
      {rows.map((i) => {
        const shown = open === i.id;
        return (
          <div key={i.id}>
            <button
              type="button"
              aria-expanded={shown}
              onClick={() => setOpen(shown ? null : i.id)}
              className="flex w-full items-center gap-3 px-4 py-3 text-left transition hover:bg-fill/50"
            >
              <ClipboardCheck className="size-4 shrink-0 text-fg-3" />
              <div className="min-w-0 flex-1">
                <div className="text-[14px] font-medium text-fg">
                  {i.kind === "move_in" ? "Move-in" : "Move-out"} inspection
                </div>
                <div className="text-xs text-fg-3">
                  {day(i.completed_at ?? i.scheduled_date ?? i.created_at)}
                </div>
              </div>
              <Badge tone={i.status === "completed" ? "good" : "neutral"}>
                {i.status === "completed" ? "Done" : "In progress"}
              </Badge>
              <ChevronDown
                className={cn(
                  "size-4 text-fg-4 transition",
                  shown && "rotate-180"
                )}
              />
            </button>
            {shown && (
              <div className="px-4 pb-4">
                {i.notes && (
                  <p className="mb-2 text-[13px] whitespace-pre-wrap text-fg-2">
                    {i.notes}
                  </p>
                )}
                {i.items.length === 0 ? (
                  <p className="text-[13px] text-fg-3">No items recorded.</p>
                ) : (
                  <ul className="divide-y divide-line rounded-xl border border-line">
                    {i.items.map((item) => (
                      <li
                        key={item.id}
                        className="flex items-start justify-between gap-3 px-3 py-2"
                      >
                        <div className="min-w-0 text-[13px]">
                          <div className="text-fg">
                            {item.area} · {item.item}
                          </div>
                          {item.notes && (
                            <div className="text-xs text-fg-3">
                              {item.notes}
                            </div>
                          )}
                        </div>
                        <Badge tone={conditionTone(item.condition)}>
                          {item.condition}
                        </Badge>
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            )}
          </div>
        );
      })}
    </Panel>
  );
}
