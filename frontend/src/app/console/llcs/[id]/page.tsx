"use client";

import { useEffect, useState } from "react";
import { useParams } from "next/navigation";
import Link from "next/link";
import {
  api,
  type CapTable,
  type BankAccount,
  type LegalEntity,
} from "@/lib/api";
import { Badge, Card, statusTone } from "@/components/ui";
import { AssignmentsCard } from "@/components/AssignmentsCard";

const OWNER_KINDS = ["individual", "company", "firm"];
const OWNER_ROLES = ["investor", "member", "manager"];

export default function LlcDetailPage() {
  const params = useParams<{ id: string }>();
  const id = params.id;

  const [entity, setEntity] = useState<LegalEntity | null>(null);
  const [capTable, setCapTable] = useState<CapTable | null>(null);
  const [accounts, setAccounts] = useState<BankAccount[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Add-owner form.
  const [ownerName, setOwnerName] = useState("");
  const [ownerKind, setOwnerKind] = useState("individual");
  const [ownerPct, setOwnerPct] = useState("");
  const [ownerRole, setOwnerRole] = useState("investor");
  const [ownerBusy, setOwnerBusy] = useState(false);

  // Add-account form.
  const [acctKind, setAcctKind] = useState("operating");
  const [institution, setInstitution] = useState("");
  const [acctNumber, setAcctNumber] = useState("");
  const [acctBusy, setAcctBusy] = useState(false);

  const load = () =>
    Promise.all([api.legalEntities(), api.capTable(id), api.bankAccounts(id)])
      .then(([entities, ct, accts]) => {
        setEntity(entities.find((e) => e.id === id) ?? null);
        setCapTable(ct);
        setAccounts(accts);
      })
      .catch((e) => setError(e.message));

  useEffect(() => {
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id]);

  async function addOwner(e: React.FormEvent) {
    e.preventDefault();
    const pct = parseFloat(ownerPct);
    if (!ownerName.trim() || Number.isNaN(pct)) return;
    setOwnerBusy(true);
    setError(null);
    try {
      await api.addOwnership(id, {
        owner_name: ownerName.trim(),
        owner_kind: ownerKind,
        ownership_bps: Math.round(pct * 100),
        role: ownerRole,
      });
      setOwnerName("");
      setOwnerPct("");
      await load();
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setOwnerBusy(false);
    }
  }

  async function addAccount(e: React.FormEvent) {
    e.preventDefault();
    if (!institution.trim()) return;
    setAcctBusy(true);
    setError(null);
    try {
      await api.createBankAccount(id, {
        kind: acctKind,
        institution: institution.trim(),
        account_number: acctNumber.trim() || undefined,
      });
      setInstitution("");
      setAcctNumber("");
      await load();
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setAcctBusy(false);
    }
  }

  const hasOperating = accounts?.some((a) => a.kind === "operating");
  const hasTrust = accounts?.some((a) => a.kind === "trust");

  return (
    <div className="space-y-6">
      <div>
        <Link href="/console/llcs" className="text-sm text-ink-3">
          ← Back to LLCs
        </Link>
        <div className="mt-1 flex flex-wrap items-center gap-3">
          <h1 className="font-display text-3xl font-extrabold tracking-tight">
            {entity?.name ?? "Legal entity"}
          </h1>
          {entity && (
            <>
              <Badge tone="neutral">{entity.entity_type.toUpperCase()}</Badge>
              <Badge tone={statusTone(entity.status)}>{entity.status}</Badge>
            </>
          )}
        </div>
        {entity && (
          <p className="text-ink-3">
            EIN {entity.ein || "—"} · {entity.state || "—"}
            {entity.registered_agent
              ? ` · Agent: ${entity.registered_agent}`
              : ""}
          </p>
        )}
      </div>

      {error && <p className="text-bad">{error}</p>}

      {/* Cap table */}
      <Card className="overflow-hidden">
        <div className="flex items-center justify-between border-b border-line px-5 py-4">
          <h2 className="font-display text-lg font-bold">Cap table</h2>
          {capTable && (
            <Badge tone={capTable.total_bps === 10000 ? "good" : "warn"}>
              {capTable.total_label} allocated
            </Badge>
          )}
        </div>
        <div className="divide-y divide-line">
          {capTable?.rows.map((r) => (
            <div
              key={r.ownership_id}
              className="flex items-center gap-4 px-5 py-3 text-sm"
            >
              <span className="flex-1 font-semibold">{r.owner_name}</span>
              <Badge tone="neutral">{r.owner_kind}</Badge>
              <span className="text-ink-3 capitalize">{r.role}</span>
              <span className="font-mono font-bold">{r.ownership_label}</span>
            </div>
          ))}
          {capTable?.rows.length === 0 && (
            <div className="px-5 py-3 text-sm text-ink-3">
              No owners yet — the firm itself can hold a stake.
            </div>
          )}
        </div>
        <form
          onSubmit={addOwner}
          className="flex flex-wrap items-end gap-3 border-t border-line bg-surface-2 px-5 py-4"
        >
          <label className="flex-1 min-w-[160px] text-sm">
            <span className="mb-1 block text-ink-3">Owner name</span>
            <input
              value={ownerName}
              onChange={(e) => setOwnerName(e.target.value)}
              className="w-full rounded-lg border border-line bg-surface px-3 py-2"
            />
          </label>
          <label className="text-sm">
            <span className="mb-1 block text-ink-3">Kind</span>
            <select
              value={ownerKind}
              onChange={(e) => setOwnerKind(e.target.value)}
              className="rounded-lg border border-line bg-surface px-3 py-2 capitalize"
            >
              {OWNER_KINDS.map((k) => (
                <option key={k} value={k}>
                  {k}
                </option>
              ))}
            </select>
          </label>
          <label className="text-sm">
            <span className="mb-1 block text-ink-3">Role</span>
            <select
              value={ownerRole}
              onChange={(e) => setOwnerRole(e.target.value)}
              className="rounded-lg border border-line bg-surface px-3 py-2 capitalize"
            >
              {OWNER_ROLES.map((r) => (
                <option key={r} value={r}>
                  {r}
                </option>
              ))}
            </select>
          </label>
          <label className="text-sm">
            <span className="mb-1 block text-ink-3">Stake %</span>
            <input
              value={ownerPct}
              onChange={(e) => setOwnerPct(e.target.value)}
              inputMode="decimal"
              placeholder="40"
              className="w-24 rounded-lg border border-line bg-surface px-3 py-2"
            />
          </label>
          <button
            type="submit"
            disabled={ownerBusy}
            className="rounded-lg bg-accent px-4 py-2 font-semibold text-white disabled:opacity-50"
          >
            Add owner
          </button>
        </form>
      </Card>

      {/* Bank accounts */}
      <Card className="overflow-hidden">
        <div className="flex flex-wrap items-center gap-3 border-b border-line px-5 py-4">
          <h2 className="flex-1 font-display text-lg font-bold">
            Bank accounts
          </h2>
          <Badge tone={hasOperating ? "good" : "warn"}>
            {hasOperating ? "operating ✓" : "no operating"}
          </Badge>
          <Badge tone={hasTrust ? "good" : "warn"}>
            {hasTrust ? "trust ✓" : "no trust"}
          </Badge>
        </div>
        <div className="divide-y divide-line">
          {accounts?.map((a) => (
            <div
              key={a.id}
              className="flex items-center gap-4 px-5 py-3 text-sm"
            >
              <Badge tone={a.kind === "trust" ? "info" : "neutral"}>
                {a.kind}
              </Badge>
              <span className="flex-1 font-semibold">{a.institution}</span>
              <span className="font-mono text-ink-3">{a.masked_number}</span>
              <Badge tone={statusTone(a.status)}>{a.status}</Badge>
            </div>
          ))}
          {accounts?.length === 0 && (
            <div className="px-5 py-3 text-sm text-ink-3">
              No accounts yet — add an operating and a trust (escrow) account.
            </div>
          )}
        </div>
        <form
          onSubmit={addAccount}
          className="flex flex-wrap items-end gap-3 border-t border-line bg-surface-2 px-5 py-4"
        >
          <label className="text-sm">
            <span className="mb-1 block text-ink-3">Type</span>
            <select
              value={acctKind}
              onChange={(e) => setAcctKind(e.target.value)}
              className="rounded-lg border border-line bg-surface px-3 py-2 capitalize"
            >
              <option value="operating">operating</option>
              <option value="trust">trust</option>
            </select>
          </label>
          <label className="flex-1 min-w-[160px] text-sm">
            <span className="mb-1 block text-ink-3">Institution</span>
            <input
              value={institution}
              onChange={(e) => setInstitution(e.target.value)}
              className="w-full rounded-lg border border-line bg-surface px-3 py-2"
            />
          </label>
          <label className="text-sm">
            <span className="mb-1 block text-ink-3">
              Account # (last 4 kept)
            </span>
            <input
              value={acctNumber}
              onChange={(e) => setAcctNumber(e.target.value)}
              className="w-40 rounded-lg border border-line bg-surface px-3 py-2"
            />
          </label>
          <button
            type="submit"
            disabled={acctBusy}
            className="rounded-lg bg-accent px-4 py-2 font-semibold text-white disabled:opacity-50"
          >
            Add account
          </button>
        </form>
      </Card>

      {/* Team / assignments (entity-scope; covers the LLC's properties) */}
      <AssignmentsCard
        subjectType="entity"
        subjectId={id}
        writePermission="entity:manage"
      />
    </div>
  );
}
