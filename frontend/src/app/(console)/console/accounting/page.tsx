"use client";

// Accounting: double-entry books per legal entity. The chart of accounts,
// the journal, the trial balance and trust reconciliation, and bank feeds
// matched against settled payments.

import { Suspense, useState } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { BookOpenText, Landmark, Link2, Lock, RefreshCw } from "lucide-react";
import { toast } from "sonner";
import { api, type BankTxn, type LedgerTxn, type Payment } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { queryKeys } from "@/lib/queries";
import { money } from "@/lib/backoffice";
import { errMsg, useEntityChoice, useReady } from "@/lib/money-extra";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { DataTable, Tabs } from "@/components/ui/data-table";
import { fieldClass } from "@/components/ui/input";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

const TABS = [
  ["accounts", "Accounts"],
  ["journal", "Journal"],
  ["reports", "Reports"],
  ["banking", "Banking"],
] as const;
type Key = (typeof TABS)[number][0];

function asKey(t: string | null): Key {
  return TABS.find(([k]) => k === t)?.[0] ?? "accounts";
}

export default function AccountingPage() {
  return (
    <Suspense fallback={<Skeleton className="h-64 rounded-2xl" />}>
      <Accounting />
    </Suspense>
  );
}

function Accounting() {
  const params = useSearchParams();
  const router = useRouter();
  const tab = asKey(params.get("tab"));
  const choose = (k: Key) =>
    router.replace(`/console/accounting?tab=${k}`, { scroll: false });
  const ready = useReady("ledger:read");
  const allowed = useReady();
  const { entities, defaultId, loading } = useEntityChoice(ready);
  const [entityId, setEntityId] = useState<string | undefined>(undefined);
  const active = entityId ?? defaultId;

  if (allowed && !ready)
    return (
      <div className="space-y-6">
        <PageHeader eyebrow="Money" title="Accounting" />
        <Panel>
          <EmptyState
            icon={<Lock />}
            title="You don't have access to accounting"
            description="Ask an admin for the ledger:read permission."
          />
        </Panel>
      </div>
    );

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Money"
        title="Accounting"
        description="Double-entry books for each legal entity. Every dollar traces back to what caused it."
        actions={
          entities &&
          entities.length > 0 && (
            <select
              aria-label="Entity"
              className={fieldClass}
              value={active ?? ""}
              onChange={(e) => setEntityId(e.target.value)}
            >
              {entities.map((e) => (
                <option key={e.id} value={e.id}>
                  {e.name}
                </option>
              ))}
            </select>
          )
        }
      />
      <Tabs tabs={TABS} value={tab} onChange={choose} />
      {loading && <Skeleton className="h-64 rounded-2xl" />}
      {entities && entities.length === 0 && (
        <Panel>
          <EmptyState
            icon={<Landmark />}
            title="No entities yet"
            description="Add an LLC to start its books."
          />
        </Panel>
      )}
      {active && tab === "accounts" && <Accounts entityId={active} />}
      {active && tab === "journal" && <Journal entityId={active} />}
      {active && tab === "reports" && <Reports entityId={active} />}
      {active && tab === "banking" && <Banking entityId={active} />}
    </div>
  );
}

function Accounts({ entityId }: { entityId: string }) {
  const q = useQuery({
    queryKey: queryKeys.ledgerAccounts(entityId),
    queryFn: () => api.ledgerAccounts(entityId),
  });
  if (q.isLoading) return <Skeleton className="h-64 rounded-2xl" />;
  if (q.error) return <LoadError what="accounts" error={q.error} />;
  return (
    <Panel className="overflow-hidden">
      <DataTable
        columns={["Code", "Account", "Type", { label: "Balance", num: true }]}
        rows={(q.data ?? []).map((a) => [
          <span key="c" className="font-mono text-fg-2">
            {a.code}
          </span>,
          <span key="n" className="inline-flex items-center gap-2 text-fg">
            {a.name}
            {a.is_trust && <Badge tone="info">trust</Badge>}
          </span>,
          <span key="k" className="capitalize">
            {a.kind}
          </span>,
          a.balance_label,
        ])}
        empty="No accounts yet."
      />
    </Panel>
  );
}

function Journal({ entityId }: { entityId: string }) {
  const q = useQuery({
    queryKey: queryKeys.ledgerTransactions(entityId),
    queryFn: () => api.ledgerTransactions(entityId),
  });
  if (q.isLoading)
    return (
      <div className="space-y-3">
        {Array.from({ length: 4 }, (_, i) => (
          <Skeleton key={i} className="h-28 rounded-2xl" />
        ))}
      </div>
    );
  if (q.error) return <LoadError what="the journal" error={q.error} />;
  if (q.data?.length === 0)
    return (
      <Panel>
        <EmptyState
          icon={<BookOpenText />}
          title="Nothing posted yet"
          description="Rent, bills and payouts post here as they happen."
        />
      </Panel>
    );
  return (
    <div className="space-y-3">
      {q.data?.map((t) => (
        <JournalEntry key={t.id} txn={t} />
      ))}
    </div>
  );
}

function JournalEntry({ txn }: { txn: LedgerTxn }) {
  return (
    <Panel className="px-5 py-4">
      <div className="mb-2 flex flex-wrap items-center gap-3">
        <span className="figure text-xs text-fg-3">{txn.txn_date}</span>
        <span className="text-[14px] font-medium text-fg">{txn.memo}</span>
        <Badge>{txn.source_type.replace(/_/g, " ")}</Badge>
      </div>
      <div className="space-y-1 text-[13px]">
        {txn.entries.map((e) => (
          <div key={e.id} className="flex items-center gap-3">
            <span className="eyebrow w-14 shrink-0">{e.side}</span>
            <span
              className={cn(
                "min-w-0 flex-1 truncate text-fg-2",
                e.side === "credit" && "pl-6"
              )}
            >
              <span className="font-mono text-fg-3">{e.account_code}</span>{" "}
              {e.account_name}
            </span>
            <span className="figure text-fg">{e.amount_label}</span>
          </div>
        ))}
      </div>
    </Panel>
  );
}

function Reports({ entityId }: { entityId: string }) {
  const tb = useQuery({
    queryKey: queryKeys.trialBalance(entityId),
    queryFn: () => api.trialBalance(entityId),
  });
  const trust = useQuery({
    queryKey: queryKeys.trustReconciliation(entityId),
    queryFn: () => api.trustReconciliation(entityId),
  });
  return (
    <div className="grid gap-4 lg:grid-cols-[3fr_2fr]">
      <Panel className="overflow-hidden">
        <PanelHeader
          title="Trial balance"
          action={
            tb.data && (
              <Badge tone={tb.data.balanced ? "good" : "bad"}>
                {tb.data.balanced ? "Balanced" : "Out of balance"}
              </Badge>
            )
          }
        />
        {tb.isLoading ? (
          <Skeleton className="m-5 h-48 rounded-xl" />
        ) : tb.error ? (
          <p className="p-5 text-[13px] text-bad">{tb.error.message}</p>
        ) : (
          <DataTable
            className="mt-3"
            columns={[
              "Account",
              { label: "Debits", num: true },
              { label: "Credits", num: true },
            ]}
            rows={(tb.data?.rows ?? []).map((r) => [
              <span key="a">
                <span className="font-mono text-fg-3">{r.code}</span> {r.name}
              </span>,
              r.debit_label,
              r.credit_label,
            ])}
            totals={
              tb.data
                ? [
                    "Total",
                    money(tb.data.total_debits_cents),
                    money(tb.data.total_credits_cents),
                  ]
                : undefined
            }
            empty="Nothing posted yet."
          />
        )}
      </Panel>

      <Panel className="p-5">
        <div className="flex items-start justify-between gap-3">
          <h2 className="text-[15px] font-semibold text-fg">
            Trust reconciliation
          </h2>
          {trust.data && (
            <Badge tone={trust.data.reconciled ? "good" : "bad"}>
              {trust.data.reconciled ? "Reconciled" : "Doesn't match"}
            </Badge>
          )}
        </div>
        {trust.isLoading && <Skeleton className="mt-4 h-24" />}
        {trust.error && (
          <p className="mt-4 text-[13px] text-bad">{trust.error.message}</p>
        )}
        {trust.data && (
          <dl className="mt-4 space-y-2 text-[13px]">
            <div className="flex justify-between gap-3">
              <dt className="text-fg-2">Escrow cash on hand</dt>
              <dd className="figure text-fg">{trust.data.trust_bank_label}</dd>
            </div>
            <div className="flex justify-between gap-3">
              <dt className="text-fg-2">Owed back (deposits held)</dt>
              <dd className="figure text-fg">
                {trust.data.trust_liability_label}
              </dd>
            </div>
            <div className="flex justify-between gap-3 border-t border-line pt-2 font-semibold text-fg">
              <dt>Difference</dt>
              <dd
                className={cn(
                  "figure",
                  trust.data.difference_cents !== 0 && "text-bad"
                )}
              >
                {money(trust.data.difference_cents)}
              </dd>
            </div>
          </dl>
        )}
        <p className="mt-4 text-xs text-fg-3">
          Escrow money can only move against what&apos;s owed back. The ledger
          refuses anything else, so healthy books show $0 here.
        </p>
      </Panel>
    </div>
  );
}

function Banking({ entityId }: { entityId: string }) {
  const qc = useQueryClient();
  const { can } = useAuth();
  const manage = can("payment:manage");
  const accounts = useQuery({
    queryKey: queryKeys.bankAccounts(entityId),
    queryFn: () => api.allBankAccounts(entityId),
  });
  const [accountId, setAccountId] = useState<string | undefined>(undefined);
  const list = accounts.data ?? [];
  const active =
    (accountId && list.some((a) => a.id === accountId)
      ? accountId
      : undefined) ??
    list.find((a) => a.linked)?.id ??
    list[0]?.id;
  const txns = useQuery({
    queryKey: queryKeys.bankTransactions(active ?? ""),
    queryFn: () => api.bankTransactions(active!),
    enabled: !!active,
  });
  const paid = useQuery({
    queryKey: queryKeys.payments({ status: "paid" }),
    queryFn: () => api.payments({ status: "paid" }),
    enabled: manage,
  });
  const [busy, setBusy] = useState(false);

  async function run(fn: () => Promise<unknown>, ok: string) {
    setBusy(true);
    try {
      await fn();
      toast.success(ok);
      if (active)
        void qc.invalidateQueries({
          queryKey: queryKeys.bankTransactions(active),
        });
      void qc.invalidateQueries({
        queryKey: queryKeys.bankAccounts(entityId),
      });
    } catch (e) {
      toast.error(errMsg(e, "That didn't work"));
    } finally {
      setBusy(false);
    }
  }

  if (accounts.isLoading) return <Skeleton className="h-64 rounded-2xl" />;
  if (accounts.error)
    return <LoadError what="bank accounts" error={accounts.error} />;
  if (list.length === 0)
    return (
      <Panel>
        <EmptyState
          icon={<Landmark />}
          title="No bank accounts on this entity"
          description="Add one to the entity, then link it here for feeds."
        />
      </Panel>
    );

  return (
    <div className="space-y-4">
      <div className="grid gap-3 md:grid-cols-2">
        {list.map((a) => (
          <Panel
            key={a.id}
            interactive
            className={cn("p-4", a.id === active && "border-accent")}
          >
            <button
              type="button"
              className="w-full text-left"
              aria-pressed={a.id === active}
              onClick={() => setAccountId(a.id)}
            >
              <div className="flex items-center justify-between gap-3">
                <span className="text-[14px] font-medium text-fg">
                  {a.institution}{" "}
                  <span className="font-mono text-fg-3">{a.masked_number}</span>
                </span>
                <Badge tone={a.kind === "trust" ? "info" : "neutral"}>
                  {a.kind}
                </Badge>
              </div>
              <div className="mt-1 text-xs text-fg-3">
                {a.linked
                  ? `Linked · last synced ${a.last_synced_at ? a.last_synced_at.slice(0, 10) : "never"}`
                  : "Not linked for feeds"}
              </div>
            </button>
            {manage && (
              <div className="mt-3">
                {a.linked ? (
                  <Button
                    size="sm"
                    variant="secondary"
                    disabled={busy}
                    onClick={() =>
                      run(() => api.syncBankAccount(a.id), "Sync queued")
                    }
                  >
                    <RefreshCw />
                    Sync now
                  </Button>
                ) : (
                  <Button
                    size="sm"
                    variant="secondary"
                    disabled={busy}
                    onClick={() =>
                      run(
                        () => api.linkBankAccount(a.id),
                        "Linked. The first sync is queued."
                      )
                    }
                  >
                    <Link2 />
                    Link for feeds
                  </Button>
                )}
              </div>
            )}
          </Panel>
        ))}
      </div>

      <Panel className="overflow-hidden">
        <PanelHeader
          title="Feed transactions"
          description={
            manage
              ? "Match deposits to settled payments of the same amount, or ignore what doesn't belong."
              : undefined
          }
        />
        {txns.isLoading ? (
          <Skeleton className="m-5 h-40 rounded-xl" />
        ) : (
          <DataTable
            className="mt-3"
            columns={[
              "Date",
              "Description",
              { label: "Amount", num: true },
              "Status",
              ...(manage ? [""] : []),
            ]}
            rows={(txns.data ?? []).map((t) => [
              <span key="d" className="font-mono text-fg-2">
                {t.posted_date}
              </span>,
              t.description,
              <span key="a" className={cn(t.amount_cents < 0 && "text-bad")}>
                {t.amount_label}
              </span>,
              <Badge
                key="s"
                tone={
                  t.status === "matched"
                    ? "good"
                    : t.status === "ignored"
                      ? "neutral"
                      : "warn"
                }
              >
                {t.status}
              </Badge>,
              ...(manage
                ? [
                    <BankActions
                      key="x"
                      txn={t}
                      busy={busy}
                      candidates={(paid.data ?? []).filter(
                        (p) => p.amount_cents === t.amount_cents
                      )}
                      onMatch={(paymentId) =>
                        run(
                          () => api.matchBankTransaction(t.id, paymentId),
                          "Matched"
                        )
                      }
                      onIgnore={() =>
                        run(() => api.ignoreBankTransaction(t.id), "Ignored")
                      }
                    />,
                  ]
                : []),
            ])}
            empty="No feed activity yet. Link the account and sync."
          />
        )}
      </Panel>
    </div>
  );
}

function BankActions({
  txn,
  busy,
  candidates,
  onMatch,
  onIgnore,
}: {
  txn: BankTxn;
  busy: boolean;
  candidates: Payment[];
  onMatch: (paymentId: string) => void;
  onIgnore: () => void;
}) {
  if (txn.status !== "unmatched") return null;
  const first = candidates[0];
  return (
    <div className="flex justify-end gap-1.5">
      {txn.amount_cents > 0 && first && (
        <Button
          size="sm"
          variant="secondary"
          disabled={busy}
          onClick={() => onMatch(first.id)}
        >
          Match {first.receipt_number ?? "payment"}
        </Button>
      )}
      <Button size="sm" variant="ghost" disabled={busy} onClick={onIgnore}>
        Ignore
      </Button>
    </div>
  );
}

function LoadError({ what, error }: { what: string; error: Error }) {
  return (
    <Panel className="border-bad/30 p-4 text-[13px] text-bad">
      Couldn&apos;t load {what}: {error.message}
    </Panel>
  );
}
