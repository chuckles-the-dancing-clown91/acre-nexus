"use client";

// "My lease" — the resident's tenancy in one place: lease summary (term,
// rent, deposit, standing), every document filed on the lease (signed lease,
// receipts, statements) with signed-URL downloads, the security-deposit
// picture (including the disposition statement after move-out), and read-only
// move-in / move-out inspection reports.

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import {
  api,
  ApiError,
  type DocumentEntry,
  type InspectionDetail,
  type LeaseDeposit,
  type MyLease,
} from "@/lib/api";
import { toast } from "sonner";
import { SiteHeader } from "@/components/SiteHeader";
import { Badge, Card, statusTone } from "@/components/ui";
import { useAuth } from "@/lib/auth";

function fmtDate(iso: string | null | undefined) {
  if (!iso) return "—";
  return iso.slice(0, 10);
}

export default function MyLeasePage() {
  const { user, loading } = useAuth();
  const [lease, setLease] = useState<MyLease | null>(null);
  const [documents, setDocuments] = useState<DocumentEntry[]>([]);
  const [deposit, setDeposit] = useState<LeaseDeposit | null>(null);
  const [inspections, setInspections] = useState<InspectionDetail[]>([]);
  const [error, setError] = useState(false);

  const load = useCallback(async () => {
    try {
      const l = await api.myLease();
      setLease(l);
      setError(false);
    } catch (e) {
      if (e instanceof ApiError && e.status === 404) setError(true);
      return;
    }
    const [docs, dep, insp] = await Promise.allSettled([
      api.myDocuments(),
      api.myDeposit(),
      api.myInspections(),
    ]);
    if (docs.status === "fulfilled") setDocuments(docs.value);
    if (dep.status === "fulfilled") setDeposit(dep.value);
    if (insp.status === "fulfilled") setInspections(insp.value);
  }, []);

  useEffect(() => {
    if (user) void load();
  }, [user, load]);

  return (
    <>
      <SiteHeader />
      <main className="mx-auto max-w-[860px] px-6 py-8">
        <h1 className="mb-1 font-display text-3xl font-extrabold tracking-tight">
          My lease
        </h1>
        <p className="mb-6 text-ink-3">
          Your tenancy, documents, deposit, and inspection reports.
        </p>

        {!loading && !user && (
          <Card className="p-8 text-center">
            <p className="mb-3 text-ink-2">Sign in to view your lease.</p>
            <Link
              href="/login"
              className="inline-block rounded-xl bg-accent px-5 py-2.5 text-sm font-bold text-on-accent"
            >
              Sign in
            </Link>
          </Card>
        )}

        {user && error && (
          <Card className="p-8 text-center text-ink-3">
            No lease is linked to your account yet. Once you have a lease with
            us, its details and documents appear here.
          </Card>
        )}

        {user && lease && (
          <div className="space-y-5">
            <SummaryCard lease={lease} />
            <DocumentsCard documents={documents} />
            {deposit && <DepositCard deposit={deposit} />}
            {inspections.length > 0 && (
              <InspectionsCard inspections={inspections} />
            )}
          </div>
        )}
      </main>
    </>
  );
}

function SummaryCard({ lease }: { lease: MyLease }) {
  return (
    <Card>
      <div className="flex flex-wrap items-center justify-between gap-3 border-b border-line px-5 py-4">
        <div>
          <div className="font-display text-lg font-bold">
            {lease.property_name}
            {lease.unit_label ? ` · Unit ${lease.unit_label}` : ""}
          </div>
          <div className="text-sm text-ink-3">{lease.property_address}</div>
        </div>
        <div className="flex gap-2">
          <Badge tone={statusTone(lease.status)}>{lease.status}</Badge>
          <Badge tone={statusTone(lease.payment_status)}>
            {lease.payment_status}
          </Badge>
        </div>
      </div>
      <dl className="grid grid-cols-2 gap-4 px-5 py-4 sm:grid-cols-4">
        <div>
          <dt className="text-xs uppercase tracking-wide text-ink-3">
            Resident
          </dt>
          <dd className="font-semibold">{lease.tenant_name}</dd>
        </div>
        <div>
          <dt className="text-xs uppercase tracking-wide text-ink-3">Term</dt>
          <dd className="font-semibold">
            {fmtDate(lease.start_date)} →{" "}
            {lease.end_date ? fmtDate(lease.end_date) : "month-to-month"}
          </dd>
        </div>
        <div>
          <dt className="text-xs uppercase tracking-wide text-ink-3">
            Monthly rent
          </dt>
          <dd className="font-semibold">{lease.rent_label}</dd>
        </div>
        <div>
          <dt className="text-xs uppercase tracking-wide text-ink-3">
            Balance
          </dt>
          <dd className="font-semibold">{lease.balance_label}</dd>
        </div>
      </dl>
    </Card>
  );
}

async function download(id: string) {
  try {
    const { url } = await api.myDocumentDownloadUrl(id);
    window.open(url, "_blank", "noopener");
  } catch {
    toast.error("Download failed — try again in a moment.");
  }
}

function DocumentsCard({ documents }: { documents: DocumentEntry[] }) {
  return (
    <Card>
      <div className="border-b border-line px-5 py-4 font-display text-lg font-bold">
        Documents
      </div>
      {documents.length === 0 ? (
        <div className="px-5 py-6 text-sm text-ink-3">
          Nothing filed yet — your signed lease and payment receipts will appear
          here.
        </div>
      ) : (
        <ul className="divide-y divide-line">
          {documents.map((d) => (
            <li
              key={d.id}
              className="flex flex-wrap items-center justify-between gap-3 px-5 py-3"
            >
              <div className="min-w-0">
                <div className="truncate font-semibold">{d.filename}</div>
                <div className="text-xs text-ink-3">
                  {d.category ?? "document"} · {fmtDate(d.created_at)}
                </div>
              </div>
              <button
                onClick={() => void download(d.id)}
                disabled={d.status !== "stored"}
                className="rounded-xl border border-line px-3 py-1.5 text-sm font-semibold hover:bg-surface-2 disabled:opacity-50"
              >
                Download
              </button>
            </li>
          ))}
        </ul>
      )}
    </Card>
  );
}

function DepositCard({ deposit }: { deposit: LeaseDeposit }) {
  const d = deposit.disposition;
  return (
    <Card>
      <div className="flex items-center justify-between border-b border-line px-5 py-4">
        <div className="font-display text-lg font-bold">Security deposit</div>
        {deposit.deposit_label && (
          <Badge tone={deposit.deposit_paid ? "good" : "warn"}>
            {deposit.deposit_paid ? "held in trust" : "not paid"}
          </Badge>
        )}
      </div>
      <div className="px-5 py-4 text-sm">
        {!deposit.deposit_label && (
          <p className="text-ink-3">This lease has no security deposit.</p>
        )}
        {deposit.deposit_label && (
          <p>
            Deposit:{" "}
            <span className="font-semibold">{deposit.deposit_label}</span>
            {!deposit.deposit_paid && (
              <>
                {" "}
                — pay it from{" "}
                <Link
                  href="/account/payments"
                  className="font-semibold underline"
                >
                  My rent
                </Link>
                .
              </>
            )}
          </p>
        )}
        {d && (
          <div className="mt-3 rounded-xl border border-line p-4">
            <div className="mb-2 flex items-center justify-between">
              <span className="font-semibold">Move-out settlement</span>
              <Badge tone={statusTone(d.status)}>{d.status}</Badge>
            </div>
            {d.deductions.length > 0 && (
              <ul className="mb-2 space-y-1 text-ink-2">
                {d.deductions.map((x) => (
                  <li key={x.id} className="flex justify-between">
                    <span>{x.description}</span>
                    <span>−{x.amount_label}</span>
                  </li>
                ))}
              </ul>
            )}
            <div className="flex justify-between font-semibold">
              <span>Refund to you</span>
              <span>{d.refund_label ?? "—"}</span>
            </div>
            {d.statement_document_id && (
              <button
                onClick={() => void download(d.statement_document_id!)}
                className="mt-3 rounded-xl border border-line px-3 py-1.5 text-sm font-semibold hover:bg-surface-2"
              >
                Download statement
              </button>
            )}
          </div>
        )}
      </div>
    </Card>
  );
}

function InspectionsCard({ inspections }: { inspections: InspectionDetail[] }) {
  const [open, setOpen] = useState<string | null>(null);
  return (
    <Card>
      <div className="border-b border-line px-5 py-4 font-display text-lg font-bold">
        Inspections
      </div>
      <ul className="divide-y divide-line">
        {inspections.map((i) => (
          <li key={i.id} className="px-5 py-3">
            <button
              className="flex w-full flex-wrap items-center justify-between gap-3 text-left"
              onClick={() => setOpen(open === i.id ? null : i.id)}
            >
              <div>
                <span className="font-semibold">
                  {i.kind === "move_in" ? "Move-in" : "Move-out"} inspection
                </span>
                <span className="ml-2 text-xs text-ink-3">
                  {i.scheduled_date ?? fmtDate(i.created_at)}
                </span>
              </div>
              <Badge tone={i.status === "completed" ? "good" : "neutral"}>
                {i.status}
              </Badge>
            </button>
            {open === i.id && (
              <ul className="mt-3 space-y-1 text-sm">
                {i.items.map((item) => (
                  <li
                    key={item.id}
                    className="flex flex-wrap justify-between gap-2"
                  >
                    <span className="text-ink-2">
                      {item.area} — {item.item}
                      {item.notes ? (
                        <span className="text-ink-3"> · {item.notes}</span>
                      ) : null}
                    </span>
                    <Badge
                      tone={
                        item.condition === "good"
                          ? "good"
                          : item.condition === "unrated"
                            ? "neutral"
                            : item.condition === "fair"
                              ? "warn"
                              : "bad"
                      }
                    >
                      {item.condition}
                    </Badge>
                  </li>
                ))}
              </ul>
            )}
          </li>
        ))}
      </ul>
    </Card>
  );
}
