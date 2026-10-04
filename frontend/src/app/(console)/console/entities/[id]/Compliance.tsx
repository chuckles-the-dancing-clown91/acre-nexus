"use client";

// A contractor's paperwork: the W-9 for 1099s and insurance certificates with
// their end dates. The taxpayer id is typed once; only the last four come
// back.

import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { FileCheck2, Plus, ShieldCheck, Trash2 } from "lucide-react";
import { toast } from "sonner";
import {
  CLASSIFICATIONS,
  INSURANCE_KINDS,
  vendors,
  type Compliance as ComplianceData,
} from "@/lib/vendors";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { F, Fact, FormDialog, input } from "@/components/property/bits";

const STATE_TONE = {
  current: "good",
  expiring: "warn",
  expired: "bad",
} as const;

const kindLabel = (k: string) =>
  INSURANCE_KINDS.find(([x]) => x === k)?.[1] ?? k;

export function Compliance({
  counterpartyId,
  manage,
}: {
  counterpartyId: string;
  manage: boolean;
}) {
  const qc = useQueryClient();
  const key = ["vendor-compliance", counterpartyId];
  const data = useQuery({
    queryKey: key,
    queryFn: () => vendors.compliance(counterpartyId),
  });
  const [busy, setBusy] = useState(false);
  const [w9Open, setW9Open] = useState(false);
  const [insOpen, setInsOpen] = useState(false);
  const [w9, setW9] = useState({
    legal_name: "",
    business_name: "",
    classification: "individual",
    tin_type: "ein",
    tin: "",
    signed_on: "",
  });
  const [ins, setIns] = useState({
    kind: "general_liability",
    carrier: "",
    policy_number: "",
    limit: "",
    expires_on: "",
  });

  async function run(fn: () => Promise<ComplianceData>, ok: string) {
    setBusy(true);
    try {
      qc.setQueryData(key, await fn());
      toast.success(ok);
      return true;
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "That didn't work");
      return false;
    } finally {
      setBusy(false);
    }
  }

  if (data.isLoading) return <Skeleton className="h-48 rounded-2xl" />;
  const d = data.data;
  if (!d) return null;

  async function saveW9() {
    if (!w9.legal_name.trim() || !w9.tin.trim()) {
      toast.error("The legal name and taxpayer id are needed");
      return;
    }
    const ok = await run(
      () =>
        vendors.saveW9(counterpartyId, {
          legal_name: w9.legal_name.trim(),
          business_name: w9.business_name.trim() || undefined,
          classification: w9.classification,
          tin_type: w9.tin_type,
          tin: w9.tin.trim(),
          signed_on: w9.signed_on || undefined,
        }),
      "W-9 saved"
    );
    if (ok) {
      setW9Open(false);
      setW9((s) => ({ ...s, tin: "" }));
    }
  }

  async function addIns() {
    if (!ins.carrier.trim() || !ins.expires_on) {
      toast.error("The carrier and end date are needed");
      return;
    }
    const dollars = Number(ins.limit.replace(/[$,]/g, ""));
    const ok = await run(
      () =>
        vendors.addInsurance(counterpartyId, {
          kind: ins.kind,
          carrier: ins.carrier.trim(),
          policy_number: ins.policy_number.trim() || undefined,
          limit_cents:
            ins.limit && Number.isFinite(dollars)
              ? Math.round(dollars * 100)
              : undefined,
          expires_on: ins.expires_on,
        }),
      "Certificate added"
    );
    if (ok) {
      setInsOpen(false);
      setIns((s) => ({
        ...s,
        carrier: "",
        policy_number: "",
        limit: "",
        expires_on: "",
      }));
    }
  }

  const w9Class =
    d.w9 &&
    (CLASSIFICATIONS.find(([k]) => k === d.w9?.classification)?.[1] ??
      d.w9.classification);

  return (
    <Panel>
      <PanelHeader
        title="Tax and insurance"
        description="The W-9 for 1099s and certificates of insurance."
        action={
          d.problems.length === 0 ? (
            <Badge tone="good">All set</Badge>
          ) : (
            <Badge tone="warn">
              {d.problems.length === 1
                ? "1 thing needed"
                : `${d.problems.length} things needed`}
            </Badge>
          )
        }
      />
      <div className="space-y-5 p-5 pt-4">
        {d.problems.length > 0 && (
          <ul className="list-disc space-y-1 rounded-xl border border-warn/25 bg-warn/8 py-2.5 pr-3 pl-8 text-[13px] text-fg-2">
            {d.problems.map((p) => (
              <li key={p}>{p}</li>
            ))}
          </ul>
        )}

        <section>
          <div className="mb-1 flex items-center justify-between gap-2">
            <h3 className="text-[13px] font-semibold text-fg">W-9</h3>
            {manage && (
              <Button
                size="sm"
                variant="ghost"
                onClick={() => {
                  setW9((s) => ({
                    ...s,
                    legal_name: d.w9?.legal_name ?? d.name,
                    business_name: d.w9?.business_name ?? "",
                    classification: d.w9?.classification ?? s.classification,
                    tin_type: d.w9?.tin_type ?? s.tin_type,
                    signed_on: d.w9?.signed_on ?? "",
                    tin: "",
                  }));
                  setW9Open(true);
                }}
              >
                <FileCheck2 />
                {d.w9 ? "Replace W-9" : "Add W-9"}
              </Button>
            )}
          </div>
          {d.w9 ? (
            <dl className="divide-y divide-line/60">
              <Fact label="Legal name">{d.w9.legal_name}</Fact>
              <Fact label="Business name">{d.w9.business_name}</Fact>
              <Fact label="Tax class">{w9Class}</Fact>
              <Fact label={d.w9.tin_type.toUpperCase()}>
                <span className="font-mono">{d.w9.tin_masked}</span>
              </Fact>
              <Fact label="Signed">{d.w9.signed_on}</Fact>
            </dl>
          ) : (
            <p className="text-[13px] text-fg-3">No W-9 on file.</p>
          )}
        </section>

        <section>
          <div className="mb-1 flex items-center justify-between gap-2">
            <h3 className="text-[13px] font-semibold text-fg">Insurance</h3>
            {manage && (
              <Button
                size="sm"
                variant="ghost"
                onClick={() => setInsOpen(true)}
              >
                <Plus />
                Add certificate
              </Button>
            )}
          </div>
          {d.insurance.length === 0 ? (
            <EmptyState
              icon={<ShieldCheck />}
              title="No certificates on file"
              className="py-6"
            />
          ) : (
            <ul className="divide-y divide-line">
              {d.insurance.map((i) => (
                <li
                  key={i.id}
                  className="flex flex-wrap items-center justify-between gap-2 py-2.5"
                >
                  <div className="min-w-0">
                    <div className="text-[13px] font-medium text-fg">
                      {kindLabel(i.kind)} · {i.carrier}
                    </div>
                    <div className="text-xs text-fg-3">
                      {i.policy_number ? `Policy ${i.policy_number} · ` : ""}
                      {i.limit_label ? `${i.limit_label} limit · ` : ""}
                      ends {i.expires_on}
                    </div>
                  </div>
                  <div className="flex items-center gap-1.5">
                    <Badge tone={STATE_TONE[i.state]}>{i.state}</Badge>
                    {manage && (
                      <Button
                        size="icon"
                        variant="ghost"
                        aria-label="Remove certificate"
                        disabled={busy}
                        onClick={() => {
                          if (confirm("Remove this certificate?"))
                            void run(
                              () => vendors.removeInsurance(i.id),
                              "Certificate removed"
                            );
                        }}
                      >
                        <Trash2 />
                      </Button>
                    )}
                  </div>
                </li>
              ))}
            </ul>
          )}
        </section>
      </div>

      <FormDialog
        open={w9Open}
        onOpenChange={setW9Open}
        title={d.w9 ? "Replace W-9" : "Add W-9"}
        description="The taxpayer id is stored encrypted. Only the last four show again."
        busy={busy}
        onSave={saveW9}
      >
        <F label="Legal name (line 1)">
          <input
            className={input}
            value={w9.legal_name}
            onChange={(e) => setW9({ ...w9, legal_name: e.target.value })}
            required
          />
        </F>
        <F label="Business name (line 2, optional)">
          <input
            className={input}
            value={w9.business_name}
            onChange={(e) => setW9({ ...w9, business_name: e.target.value })}
          />
        </F>
        <F label="Tax class" className="sm:col-span-2">
          <select
            className={input}
            value={w9.classification}
            onChange={(e) => setW9({ ...w9, classification: e.target.value })}
          >
            {CLASSIFICATIONS.map(([k, l]) => (
              <option key={k} value={k}>
                {l}
              </option>
            ))}
          </select>
        </F>
        <F label="Taxpayer id">
          <div className="flex gap-2">
            <select
              aria-label="Id type"
              className={`${input} w-24`}
              value={w9.tin_type}
              onChange={(e) => setW9({ ...w9, tin_type: e.target.value })}
            >
              <option value="ein">EIN</option>
              <option value="ssn">SSN</option>
            </select>
            <input
              className={`${input} min-w-0 flex-1 font-mono`}
              placeholder={w9.tin_type === "ein" ? "12-3456789" : "123-45-6789"}
              autoComplete="off"
              inputMode="numeric"
              value={w9.tin}
              onChange={(e) => setW9({ ...w9, tin: e.target.value })}
              required
            />
          </div>
        </F>
        <F label="Signed on">
          <input
            type="date"
            className={input}
            value={w9.signed_on}
            onChange={(e) => setW9({ ...w9, signed_on: e.target.value })}
          />
        </F>
      </FormDialog>

      <FormDialog
        open={insOpen}
        onOpenChange={setInsOpen}
        title="Add a certificate"
        busy={busy}
        onSave={addIns}
      >
        <F label="Coverage">
          <select
            className={input}
            value={ins.kind}
            onChange={(e) => setIns({ ...ins, kind: e.target.value })}
          >
            {INSURANCE_KINDS.map(([k, l]) => (
              <option key={k} value={k}>
                {l}
              </option>
            ))}
          </select>
        </F>
        <F label="Carrier">
          <input
            className={input}
            value={ins.carrier}
            onChange={(e) => setIns({ ...ins, carrier: e.target.value })}
            required
          />
        </F>
        <F label="Policy number (optional)">
          <input
            className={input}
            value={ins.policy_number}
            onChange={(e) => setIns({ ...ins, policy_number: e.target.value })}
          />
        </F>
        <F label="Limit (optional)">
          <input
            className={input}
            placeholder="1,000,000"
            inputMode="decimal"
            value={ins.limit}
            onChange={(e) => setIns({ ...ins, limit: e.target.value })}
          />
        </F>
        <F label="Ends">
          <input
            type="date"
            className={input}
            value={ins.expires_on}
            onChange={(e) => setIns({ ...ins, expires_on: e.target.value })}
            required
          />
        </F>
      </FormDialog>
    </Panel>
  );
}
