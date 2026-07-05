"use client";

// Accounting console: per-entity chart of accounts, the double-entry journal,
// financial reports (trial balance / income statement / trust reconciliation),
// and bank-feed reconciliation. Server state rides the TanStack Query hooks.

import { useMemo, useState } from "react";
import { api, type BankTxn, type LedgerTxn } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import {
  queryKeys,
  useBankAccounts,
  useBankTransactions,
  useEntityPicker,
  useLedgerAccounts,
  useLedgerTransactions,
  usePayments,
  useTrialBalance,
  useTrustReconciliation,
} from "@/lib/queries";
import { useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { Badge, Button, Card } from "@/components/ui";

const TABS = ["Accounts", "Journal", "Reports", "Banking"] as const;
type Tab = (typeof TABS)[number];

export default function AccountingPage() {
  const { can } = useAuth();
  const [tab, setTab] = useState<Tab>("Accounts");
  const { entities, defaultId } = useEntityPicker();
  const [entityId, setEntityId] = useState<string | undefined>(undefined);
  const activeEntity = entityId ?? defaultId;

  if (!can("ledger:read")) {
    return (
      <p className="text-ink-3">
        You don&apos;t have permission to view accounting.
      </p>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <h1 className="font-display text-3xl font-extrabold tracking-tight">
            Accounting
          </h1>
          <p className="text-ink-3">
            Double-entry books per legal entity — every dollar traceable to its
            domain event.
          </p>
        </div>
        <label className="flex items-center gap-2 text-sm font-semibold text-ink-2">
          Entity
          <select
            value={activeEntity ?? ""}
            onChange={(e) => setEntityId(e.target.value)}
            className="rounded-xl border border-line bg-surface px-3 py-2 font-semibold"
          >
            {entities?.map((e) => (
              <option key={e.id} value={e.id}>
                {e.name}
              </option>
            ))}
          </select>
        </label>
      </div>

      <div className="flex gap-2">
        {TABS.map((t) => (
          <button
            key={t}
            onClick={() => setTab(t)}
            className={
              t === tab
                ? "rounded-xl bg-accent px-4 py-2 text-sm font-bold text-on-accent"
                : "rounded-xl border border-line px-4 py-2 text-sm font-bold text-ink-2 hover:bg-surface-2"
            }
          >
            {t}
          </button>
        ))}
      </div>

      {activeEntity && tab === "Accounts" && (
        <Accounts entityId={activeEntity} />
      )}
      {activeEntity && tab === "Journal" && <Journal entityId={activeEntity} />}
      {activeEntity && tab === "Reports" && <Reports entityId={activeEntity} />}
      {activeEntity && tab === "Banking" && <Banking entityId={activeEntity} />}
    </div>
  );
}

function Accounts({ entityId }: { entityId: string }) {
  const { data: accounts } = useLedgerAccounts(entityId);
  return (
    <Card className="overflow-x-auto">
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b border-line text-left text-xs uppercase tracking-wide text-ink-3">
            <th className="px-5 py-3">Code</th>
            <th className="px-5 py-3">Account</th>
            <th className="px-5 py-3">Type</th>
            <th className="px-5 py-3 text-right">Balance</th>
          </tr>
        </thead>
        <tbody className="divide-y divide-line">
          {accounts?.map((a) => (
            <tr key={a.id}>
              <td className="px-5 py-3 font-mono text-ink-2">{a.code}</td>
              <td className="px-5 py-3 font-semibold">
                {a.name}
                {a.is_trust && (
                  <Badge tone="info" className="ml-2">
                    trust
                  </Badge>
                )}
              </td>
              <td className="px-5 py-3 capitalize text-ink-2">{a.kind}</td>
              <td className="px-5 py-3 text-right font-mono">
                {a.balance_label}
              </td>
            </tr>
          ))}
          {accounts && accounts.length === 0 && (
            <tr>
              <td colSpan={4} className="px-5 py-10 text-center text-ink-3">
                No accounts yet.
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </Card>
  );
}

function Journal({ entityId }: { entityId: string }) {
  const { data: txns } = useLedgerTransactions(entityId);
  return (
    <div className="space-y-3">
      {txns?.map((t) => (
        <JournalEntry key={t.id} txn={t} />
      ))}
      {txns && txns.length === 0 && (
        <Card className="px-5 py-10 text-center text-ink-3">
          Nothing posted yet.
        </Card>
      )}
    </div>
  );
}

function JournalEntry({ txn }: { txn: LedgerTxn }) {
  return (
    <Card className="px-5 py-4">
      <div className="mb-2 flex flex-wrap items-center gap-3">
        <span className="font-mono text-sm text-ink-3">{txn.txn_date}</span>
        <span className="font-semibold">{txn.memo}</span>
        <Badge tone="neutral">{txn.source_type}</Badge>
      </div>
      <div className="space-y-1 text-sm">
        {txn.entries.map((e) => (
          <div key={e.id} className="flex items-center gap-3">
            <span className="w-14 font-mono text-xs uppercase text-ink-3">
              {e.side}
            </span>
            <span className={e.side === "credit" ? "flex-1 pl-6" : "flex-1"}>
              <span className="font-mono text-ink-3">{e.account_code}</span>{" "}
              {e.account_name}
            </span>
            <span className="font-mono">{e.amount_label}</span>
          </div>
        ))}
      </div>
    </Card>
  );
}

function Reports({ entityId }: { entityId: string }) {
  const { data: tb } = useTrialBalance(entityId);
  const { data: trust } = useTrustReconciliation(entityId);
  return (
    <div className="grid gap-4 lg:grid-cols-2">
      <Card className="overflow-hidden">
        <div className="flex items-center justify-between border-b border-line px-5 py-4">
          <span className="font-display text-lg font-bold">Trial balance</span>
          {tb && (
            <Badge tone={tb.balanced ? "good" : "bad"}>
              {tb.balanced ? "Balanced" : "OUT OF BALANCE"}
            </Badge>
          )}
        </div>
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-line text-left text-xs uppercase tracking-wide text-ink-3">
              <th className="px-5 py-2.5">Account</th>
              <th className="px-5 py-2.5 text-right">Debits</th>
              <th className="px-5 py-2.5 text-right">Credits</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-line">
            {tb?.rows.map((r) => (
              <tr key={r.code}>
                <td className="px-5 py-2.5">
                  <span className="font-mono text-ink-3">{r.code}</span>{" "}
                  {r.name}
                </td>
                <td className="px-5 py-2.5 text-right font-mono">
                  {r.debit_label}
                </td>
                <td className="px-5 py-2.5 text-right font-mono">
                  {r.credit_label}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </Card>

      <Card className="p-5">
        <div className="mb-3 flex items-center justify-between">
          <span className="font-display text-lg font-bold">
            Trust reconciliation
          </span>
          {trust && (
            <Badge tone={trust.reconciled ? "good" : "bad"}>
              {trust.reconciled ? "Reconciled" : "Discrepancy"}
            </Badge>
          )}
        </div>
        {trust && (
          <dl className="space-y-2 text-sm">
            <div className="flex justify-between">
              <dt className="text-ink-2">Escrow cash on hand</dt>
              <dd className="font-mono">{trust.trust_bank_label}</dd>
            </div>
            <div className="flex justify-between">
              <dt className="text-ink-2">Owed back (deposits held)</dt>
              <dd className="font-mono">{trust.trust_liability_label}</dd>
            </div>
            <div className="flex justify-between border-t border-line pt-2 font-bold">
              <dt>Difference</dt>
              <dd className="font-mono">
                {trust.difference_cents === 0
                  ? "$0"
                  : `${trust.difference_cents / 100}`}
              </dd>
            </div>
          </dl>
        )}
        <p className="mt-4 text-xs text-ink-3">
          Escrow funds may only move against trust liabilities — the posting
          engine rejects commingling, so a healthy ledger always shows $0 here.
        </p>
      </Card>
    </div>
  );
}

function Banking({ entityId }: { entityId: string }) {
  const qc = useQueryClient();
  const { can } = useAuth();
  const { data: accounts } = useBankAccounts(entityId);
  const [accountId, setAccountId] = useState<string | undefined>(undefined);
  const active =
    accountId ?? accounts?.find((a) => a.linked)?.id ?? accounts?.[0]?.id;
  const { data: txns } = useBankTransactions(active);
  const { data: paidPayments } = usePayments({ status: "paid" });
  const [busy, setBusy] = useState(false);

  const refresh = () => {
    if (active)
      qc.invalidateQueries({ queryKey: queryKeys.bankTransactions(active) });
    qc.invalidateQueries({ queryKey: queryKeys.bankAccounts(entityId) });
  };

  async function run(fn: () => Promise<unknown>, ok: string) {
    setBusy(true);
    try {
      await fn();
      toast.success(ok);
      refresh();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Request failed");
    } finally {
      setBusy(false);
    }
  }

  const manage = can("payment:manage");

  return (
    <div className="space-y-4">
      <div className="grid gap-3 md:grid-cols-2">
        {accounts?.map((a) => (
          <Card
            key={a.id}
            className={`cursor-pointer p-4 ${a.id === active ? "border-accent" : ""}`}
          >
            <button
              className="w-full text-left"
              onClick={() => setAccountId(a.id)}
            >
              <div className="flex items-center justify-between">
                <span className="font-semibold">
                  {a.institution}{" "}
                  <span className="font-mono text-ink-3">
                    {a.masked_number}
                  </span>
                </span>
                <Badge tone={a.kind === "trust" ? "info" : "neutral"}>
                  {a.kind}
                </Badge>
              </div>
              <div className="mt-1 text-xs text-ink-3">
                {a.linked
                  ? `Linked · last synced ${a.last_synced_at ? a.last_synced_at.slice(0, 10) : "never"}`
                  : "Not linked for feeds"}
              </div>
            </button>
            {manage && (
              <div className="mt-3 flex gap-2">
                {a.linked ? (
                  <Button
                    variant="outline"
                    disabled={busy}
                    onClick={() =>
                      run(() => api.syncBankAccount(a.id), "Sync queued")
                    }
                  >
                    Sync now
                  </Button>
                ) : (
                  <Button
                    variant="outline"
                    disabled={busy}
                    onClick={() =>
                      run(
                        () => api.linkBankAccount(a.id),
                        "Account linked — first sync queued"
                      )
                    }
                  >
                    Link for feeds
                  </Button>
                )}
              </div>
            )}
          </Card>
        ))}
      </div>

      <Card className="overflow-x-auto">
        <div className="border-b border-line px-5 py-4 font-display text-lg font-bold">
          Feed transactions
        </div>
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-line text-left text-xs uppercase tracking-wide text-ink-3">
              <th className="px-5 py-3">Date</th>
              <th className="px-5 py-3">Description</th>
              <th className="px-5 py-3 text-right">Amount</th>
              <th className="px-5 py-3">Status</th>
              {manage && <th className="px-5 py-3" />}
            </tr>
          </thead>
          <tbody className="divide-y divide-line">
            {txns?.map((t) => (
              <BankRow
                key={t.id}
                txn={t}
                manage={manage}
                busy={busy}
                candidates={
                  paidPayments?.filter(
                    (p) => p.amount_cents === t.amount_cents
                  ) ?? []
                }
                onMatch={(paymentId) =>
                  run(
                    () => api.matchBankTransaction(t.id, paymentId),
                    "Matched"
                  )
                }
                onIgnore={() =>
                  run(() => api.ignoreBankTransaction(t.id), "Ignored")
                }
              />
            ))}
            {txns && txns.length === 0 && (
              <tr>
                <td
                  colSpan={manage ? 5 : 4}
                  className="px-5 py-10 text-center text-ink-3"
                >
                  No feed activity yet — link the account and sync.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </Card>
    </div>
  );
}

function BankRow({
  txn,
  manage,
  busy,
  candidates,
  onMatch,
  onIgnore,
}: {
  txn: BankTxn;
  manage: boolean;
  busy: boolean;
  candidates: {
    id: string;
    receipt_number: string | null;
    amount_label: string;
  }[];
  onMatch: (paymentId: string) => void;
  onIgnore: () => void;
}) {
  const tone =
    txn.status === "matched"
      ? "good"
      : txn.status === "ignored"
        ? "neutral"
        : "warn";
  const firstCandidate = useMemo(() => candidates[0], [candidates]);
  return (
    <tr>
      <td className="px-5 py-3 font-mono text-ink-2">{txn.posted_date}</td>
      <td className="px-5 py-3">{txn.description}</td>
      <td
        className={`px-5 py-3 text-right font-mono ${txn.amount_cents < 0 ? "text-bad" : ""}`}
      >
        {txn.amount_label}
      </td>
      <td className="px-5 py-3">
        <Badge tone={tone}>{txn.status}</Badge>
      </td>
      {manage && (
        <td className="px-5 py-3 text-right">
          {txn.status === "unmatched" && (
            <div className="flex justify-end gap-2">
              {txn.amount_cents > 0 && firstCandidate && (
                <Button
                  variant="outline"
                  disabled={busy}
                  onClick={() => onMatch(firstCandidate.id)}
                >
                  Match {firstCandidate.receipt_number ?? "payment"}
                </Button>
              )}
              <Button variant="ghost" disabled={busy} onClick={onIgnore}>
                Ignore
              </Button>
            </div>
          )}
        </td>
      )}
    </tr>
  );
}
