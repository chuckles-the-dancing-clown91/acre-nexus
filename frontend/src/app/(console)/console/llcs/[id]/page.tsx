"use client";

// One LLC: who owns it (the cap table), its operating and trust bank
// accounts, and the team assigned to it.

import { useState } from "react";
import Link from "next/link";
import { useParams } from "next/navigation";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowLeft, Check, Landmark, PieChart, Plus } from "lucide-react";
import { toast } from "sonner";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { useHasTenantScope } from "@/components/shell/tenant-scope";
import { Badge, statusTone } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { fieldClass, Input } from "@/components/ui/input";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { F } from "@/components/property/bits";
import { Foundation } from "./Foundation";
import { Team } from "./Team";

const OWNER_KINDS = ["individual", "company", "firm"];
const OWNER_ROLES = ["investor", "member", "manager"];

export default function LlcPage() {
  const { id } = useParams<{ id: string }>();
  const { can } = useAuth();
  const scoped = useHasTenantScope();
  const entities = useQuery({
    queryKey: ["legal-entities"],
    queryFn: api.legalEntities,
    enabled: scoped && can("property:read"),
  });
  const entity = entities.data?.find((e) => e.id === id) ?? null;

  return (
    <div className="space-y-6">
      <Link
        href="/console/llcs"
        className="inline-flex items-center gap-1.5 text-[13px] text-fg-3 transition hover:text-fg"
      >
        <ArrowLeft className="size-4" />
        LLCs
      </Link>
      {entities.isLoading ? (
        <Skeleton className="h-20 rounded-2xl" />
      ) : (
        <PageHeader
          eyebrow={
            entity ? (
              <span className="inline-flex items-center gap-2">
                {entity.entity_type.toUpperCase()}
                <Badge tone={statusTone(entity.status)}>{entity.status}</Badge>
              </span>
            ) : (
              "Legal entity"
            )
          }
          title={entity?.name ?? "Legal entity"}
          description={
            entity ? (
              <>
                EIN {entity.ein || "—"} · {entity.state || "—"}
                {entity.registered_agent
                  ? ` · Agent: ${entity.registered_agent}`
                  : ""}
              </>
            ) : entities.error ? (
              <span className="text-bad">{entities.error.message}</span>
            ) : undefined
          }
        />
      )}

      <div className="grid gap-4 xl:grid-cols-[minmax(0,1.4fr)_minmax(0,1fr)]">
        <div className="space-y-4">
          <CapTableCard entityId={id} />
          <AccountsCard entityId={id} />
        </div>
        <div className="space-y-4">
          {entity && (
            <Foundation
              key={`${entity.foundation}-${entity.fee_basis}`}
              entity={entity}
              manage={can("tenant:manage")}
            />
          )}
          <Team entityId={id} />
        </div>
      </div>
    </div>
  );
}

function CapTableCard({ entityId }: { entityId: string }) {
  const { can } = useAuth();
  const manage = can("entity:manage");
  const qc = useQueryClient();
  const cap = useQuery({
    queryKey: ["cap-table", entityId],
    queryFn: () => api.capTable(entityId),
    enabled: can("entity:read"),
  });
  const [name, setName] = useState("");
  const [kind, setKind] = useState("individual");
  const [role, setRole] = useState("investor");
  const [pct, setPct] = useState("");
  const [busy, setBusy] = useState(false);

  async function add(e: React.FormEvent) {
    e.preventDefault();
    const n = parseFloat(pct);
    if (!name.trim() || Number.isNaN(n)) return;
    setBusy(true);
    try {
      await api.addOwnership(entityId, {
        owner_name: name.trim(),
        owner_kind: kind,
        ownership_bps: Math.round(n * 100),
        role,
      });
      setName("");
      setPct("");
      toast.success("Owner added");
      await qc.invalidateQueries({ queryKey: ["cap-table", entityId] });
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Couldn't add them");
    } finally {
      setBusy(false);
    }
  }

  if (!can("entity:read")) return null;
  const full = cap.data?.total_bps === 10000;

  return (
    <Panel>
      <PanelHeader
        title="Cap table"
        description="Who owns the entity, and how much."
        action={
          cap.data && (
            <Badge tone={full ? "good" : "warn"}>
              {cap.data.total_label} allocated
            </Badge>
          )
        }
      />
      <div className="p-2 pt-3">
        {cap.isLoading && <Skeleton className="m-3 h-16" />}
        {cap.error && (
          <p className="px-3 py-2 text-[13px] text-bad">{cap.error.message}</p>
        )}
        {cap.data?.rows.length === 0 && (
          <EmptyState
            icon={<PieChart />}
            title="No owners yet"
            description="The firm itself can hold a stake."
            className="py-8"
          />
        )}
        <ul className="divide-y divide-line">
          {cap.data?.rows.map((r) => (
            <li
              key={r.ownership_id}
              className="flex items-center gap-3 px-3 py-2.5"
            >
              <span className="min-w-0 flex-1 truncate text-[13px] font-medium text-fg">
                {r.owner_name}
              </span>
              <Badge>{r.owner_kind}</Badge>
              <span className="w-20 text-xs text-fg-3 capitalize">
                {r.role}
              </span>
              <span className="figure w-16 text-right text-[13px] font-semibold text-fg">
                {r.ownership_label}
              </span>
            </li>
          ))}
        </ul>
      </div>
      {manage && (
        <form
          onSubmit={add}
          className="flex flex-wrap items-end gap-3 border-t border-line px-5 py-4"
        >
          <F label="Owner name" className="min-w-40 flex-1">
            <Input
              value={name}
              onChange={(e) => setName(e.target.value)}
              className="h-9"
            />
          </F>
          <F label="Kind">
            <select
              className={`${fieldClass} capitalize`}
              value={kind}
              onChange={(e) => setKind(e.target.value)}
            >
              {OWNER_KINDS.map((k) => (
                <option key={k} value={k}>
                  {k}
                </option>
              ))}
            </select>
          </F>
          <F label="Role">
            <select
              className={`${fieldClass} capitalize`}
              value={role}
              onChange={(e) => setRole(e.target.value)}
            >
              {OWNER_ROLES.map((r) => (
                <option key={r} value={r}>
                  {r}
                </option>
              ))}
            </select>
          </F>
          <F label="Stake %">
            <Input
              value={pct}
              onChange={(e) => setPct(e.target.value)}
              inputMode="decimal"
              placeholder="40"
              className="h-9 w-20"
            />
          </F>
          <Button
            type="submit"
            size="sm"
            className="h-9"
            loading={busy}
            disabled={!name.trim() || !pct.trim()}
          >
            <Plus />
            Add owner
          </Button>
        </form>
      )}
    </Panel>
  );
}

function AccountsCard({ entityId }: { entityId: string }) {
  const { can } = useAuth();
  const manage = can("finance:manage");
  const qc = useQueryClient();
  const accounts = useQuery({
    queryKey: ["bank-accounts", entityId],
    queryFn: () => api.bankAccounts(entityId),
    enabled: can("finance:read"),
  });
  const [kind, setKind] = useState("operating");
  const [institution, setInstitution] = useState("");
  const [number, setNumber] = useState("");
  const [busy, setBusy] = useState(false);

  async function add(e: React.FormEvent) {
    e.preventDefault();
    if (!institution.trim()) return;
    setBusy(true);
    try {
      await api.createBankAccount(entityId, {
        kind,
        institution: institution.trim(),
        account_number: number.trim() || undefined,
      });
      setInstitution("");
      setNumber("");
      toast.success("Account added");
      await qc.invalidateQueries({ queryKey: ["bank-accounts", entityId] });
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Couldn't add it");
    } finally {
      setBusy(false);
    }
  }

  if (!can("finance:read")) return null;
  const hasOperating = accounts.data?.some((a) => a.kind === "operating");
  const hasTrust = accounts.data?.some((a) => a.kind === "trust");

  return (
    <Panel>
      <PanelHeader
        title="Bank accounts"
        description="Each entity keeps an operating account and a trust account for deposits."
        action={
          accounts.data && (
            <div className="flex shrink-0 gap-1.5">
              <Badge tone={hasOperating ? "good" : "warn"}>
                {hasOperating && <Check className="size-3" />}
                {hasOperating ? "operating" : "no operating"}
              </Badge>
              <Badge tone={hasTrust ? "good" : "warn"}>
                {hasTrust && <Check className="size-3" />}
                {hasTrust ? "trust" : "no trust"}
              </Badge>
            </div>
          )
        }
      />
      <div className="p-2 pt-3">
        {accounts.isLoading && <Skeleton className="m-3 h-16" />}
        {accounts.error && (
          <p className="px-3 py-2 text-[13px] text-bad">
            {accounts.error.message}
          </p>
        )}
        {accounts.data?.length === 0 && (
          <EmptyState
            icon={<Landmark />}
            title="No accounts yet"
            description="Add an operating account and a trust account."
            className="py-8"
          />
        )}
        <ul className="divide-y divide-line">
          {accounts.data?.map((a) => (
            <li key={a.id} className="flex items-center gap-3 px-3 py-2.5">
              <Badge tone={a.kind === "trust" ? "info" : "neutral"}>
                {a.kind}
              </Badge>
              <span className="min-w-0 flex-1 truncate text-[13px] font-medium text-fg">
                {a.institution}
              </span>
              {a.linked && <Badge tone="accent">feed linked</Badge>}
              <span className="font-mono text-xs text-fg-3">
                {a.masked_number ?? ""}
              </span>
              <Badge tone={statusTone(a.status)}>{a.status}</Badge>
            </li>
          ))}
        </ul>
      </div>
      {manage && (
        <form
          onSubmit={add}
          className="flex flex-wrap items-end gap-3 border-t border-line px-5 py-4"
        >
          <F label="Type">
            <select
              className={fieldClass}
              value={kind}
              onChange={(e) => setKind(e.target.value)}
            >
              <option value="operating">Operating</option>
              <option value="trust">Trust</option>
            </select>
          </F>
          <F label="Bank" className="min-w-40 flex-1">
            <Input
              value={institution}
              onChange={(e) => setInstitution(e.target.value)}
              className="h-9"
            />
          </F>
          <F label="Account number (last 4 kept)">
            <Input
              value={number}
              onChange={(e) => setNumber(e.target.value)}
              autoComplete="off"
              className="h-9 w-44"
            />
          </F>
          <Button
            type="submit"
            size="sm"
            className="h-9"
            loading={busy}
            disabled={!institution.trim()}
          >
            <Plus />
            Add account
          </Button>
        </form>
      )}
    </Panel>
  );
}
