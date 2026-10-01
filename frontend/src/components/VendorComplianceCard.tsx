"use client";

// A vendor's paperwork: the W-9 for 1099s and insurance certificates with
// their end dates. The taxpayer id is typed once and only the last four come
// back.

import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import {
  CLASSIFICATIONS,
  INSURANCE_KINDS,
  vendors,
  type Compliance,
} from "@/lib/vendors";
import { Badge, Button, Card } from "@/components/ui";

const field =
  "rounded-xl border border-line bg-surface px-3 py-2 text-sm outline-none focus:border-accent";

const STATE_TONE = {
  current: "good",
  expiring: "warn",
  expired: "bad",
} as const;

function kindLabel(kind: string): string {
  return INSURANCE_KINDS.find(([k]) => k === kind)?.[1] ?? kind;
}

export function VendorComplianceCard({
  counterpartyId,
  manage,
}: {
  counterpartyId: string;
  manage: boolean;
}) {
  const [data, setData] = useState<Compliance | null>(null);
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

  const load = useCallback(() => {
    vendors
      .compliance(counterpartyId)
      .then(setData)
      .catch(() => setData(null));
  }, [counterpartyId]);
  useEffect(load, [load]);

  async function run(fn: () => Promise<Compliance>, ok: string) {
    setBusy(true);
    try {
      setData(await fn());
      toast.success(ok);
      return true;
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Request failed");
      return false;
    } finally {
      setBusy(false);
    }
  }

  if (!data) return null;

  const saveW9 = async () => {
    const done = await run(
      () =>
        vendors.saveW9(counterpartyId, {
          legal_name: w9.legal_name,
          business_name: w9.business_name || undefined,
          classification: w9.classification,
          tin_type: w9.tin_type,
          tin: w9.tin,
          signed_on: w9.signed_on || undefined,
        }),
      "W-9 saved"
    );
    if (done) {
      setW9Open(false);
      setW9((s) => ({ ...s, tin: "" }));
    }
  };

  const addIns = async () => {
    const dollars = Number(ins.limit.replace(/[$,]/g, ""));
    const done = await run(
      () =>
        vendors.addInsurance(counterpartyId, {
          kind: ins.kind,
          carrier: ins.carrier,
          policy_number: ins.policy_number || undefined,
          limit_cents:
            ins.limit && Number.isFinite(dollars)
              ? Math.round(dollars * 100)
              : undefined,
          expires_on: ins.expires_on,
        }),
      "Certificate added"
    );
    if (done) {
      setInsOpen(false);
      setIns({
        ...ins,
        carrier: "",
        policy_number: "",
        limit: "",
        expires_on: "",
      });
    }
  };

  return (
    <Card className="p-5">
      <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
        <h2 className="font-display text-lg font-bold">Tax and insurance</h2>
        {data.problems.length === 0 ? (
          <Badge tone="good">All set</Badge>
        ) : (
          <Badge tone="warn">
            {data.problems.length === 1
              ? "1 thing needed"
              : `${data.problems.length} things needed`}
          </Badge>
        )}
      </div>

      {data.problems.length > 0 && (
        <ul className="mb-4 list-disc space-y-1 pl-5 text-sm text-ink-2">
          {data.problems.map((p) => (
            <li key={p}>{p}</li>
          ))}
        </ul>
      )}

      <h3 className="mb-2 text-sm font-semibold">W-9</h3>
      {data.w9 ? (
        <dl className="mb-3 grid grid-cols-[8rem_1fr] gap-y-1 text-sm">
          <dt className="text-ink-3">Legal name</dt>
          <dd>{data.w9.legal_name}</dd>
          <dt className="text-ink-3">Tax class</dt>
          <dd>
            {CLASSIFICATIONS.find(
              ([k]) => k === data.w9?.classification
            )?.[1] ?? data.w9.classification}
          </dd>
          <dt className="text-ink-3">{data.w9.tin_type.toUpperCase()}</dt>
          <dd className="font-mono">{data.w9.tin_masked}</dd>
          {data.w9.signed_on && (
            <>
              <dt className="text-ink-3">Signed</dt>
              <dd>{data.w9.signed_on}</dd>
            </>
          )}
        </dl>
      ) : (
        <p className="mb-3 text-sm text-ink-3">No W-9 on file.</p>
      )}
      {manage && !w9Open && (
        <Button
          variant="ghost"
          onClick={() => {
            setW9((s) => ({
              ...s,
              legal_name: data.w9?.legal_name ?? data.name,
              business_name: data.w9?.business_name ?? "",
              classification: data.w9?.classification ?? s.classification,
              tin_type: data.w9?.tin_type ?? s.tin_type,
              signed_on: data.w9?.signed_on ?? "",
            }));
            setW9Open(true);
          }}
        >
          {data.w9 ? "Replace W-9" : "Add W-9"}
        </Button>
      )}
      {w9Open && (
        <div className="mb-4 grid gap-2 sm:grid-cols-2">
          <input
            className={field}
            placeholder="Legal name (line 1)"
            value={w9.legal_name}
            onChange={(e) => setW9({ ...w9, legal_name: e.target.value })}
          />
          <input
            className={field}
            placeholder="Business name (line 2, optional)"
            value={w9.business_name}
            onChange={(e) => setW9({ ...w9, business_name: e.target.value })}
          />
          <select
            className={field}
            value={w9.classification}
            onChange={(e) => setW9({ ...w9, classification: e.target.value })}
          >
            {CLASSIFICATIONS.map(([k, l]) => (
              <option key={k} value={k}>
                {l}
              </option>
            ))}
          </select>
          <div className="flex gap-2">
            <select
              className={field}
              value={w9.tin_type}
              onChange={(e) => setW9({ ...w9, tin_type: e.target.value })}
            >
              <option value="ein">EIN</option>
              <option value="ssn">SSN</option>
            </select>
            <input
              className={`${field} min-w-0 flex-1 font-mono`}
              placeholder={w9.tin_type === "ein" ? "12-3456789" : "123-45-6789"}
              autoComplete="off"
              inputMode="numeric"
              value={w9.tin}
              onChange={(e) => setW9({ ...w9, tin: e.target.value })}
            />
          </div>
          <label className="flex items-center gap-2 text-sm text-ink-2">
            Signed on
            <input
              type="date"
              className={field}
              value={w9.signed_on}
              onChange={(e) => setW9({ ...w9, signed_on: e.target.value })}
            />
          </label>
          <div className="flex gap-2 sm:justify-end">
            <Button variant="ghost" onClick={() => setW9Open(false)}>
              Cancel
            </Button>
            <Button
              onClick={saveW9}
              disabled={busy || !w9.legal_name.trim() || !w9.tin.trim()}
            >
              Save W-9
            </Button>
          </div>
        </div>
      )}

      <h3 className="mb-2 mt-5 text-sm font-semibold">Insurance</h3>
      {data.insurance.length === 0 ? (
        <p className="mb-3 text-sm text-ink-3">No certificates on file.</p>
      ) : (
        <div className="mb-3 divide-y divide-line">
          {data.insurance.map((i) => (
            <div
              key={i.id}
              className="flex flex-wrap items-center justify-between gap-2 py-2 text-sm"
            >
              <div>
                <p className="font-semibold">
                  {kindLabel(i.kind)} · {i.carrier}
                </p>
                <p className="text-ink-3">
                  {i.policy_number ? `Policy ${i.policy_number} · ` : ""}
                  {i.limit_label ? `${i.limit_label} limit · ` : ""}
                  ends {i.expires_on}
                </p>
              </div>
              <div className="flex items-center gap-2">
                <Badge tone={STATE_TONE[i.state]}>{i.state}</Badge>
                {manage && (
                  <Button
                    variant="ghost"
                    disabled={busy}
                    onClick={() => {
                      if (confirm("Remove this certificate?"))
                        run(
                          () => vendors.removeInsurance(i.id),
                          "Certificate removed"
                        );
                    }}
                  >
                    Remove
                  </Button>
                )}
              </div>
            </div>
          ))}
        </div>
      )}
      {manage && !insOpen && (
        <Button variant="ghost" onClick={() => setInsOpen(true)}>
          Add certificate
        </Button>
      )}
      {insOpen && (
        <div className="grid gap-2 sm:grid-cols-2">
          <select
            className={field}
            value={ins.kind}
            onChange={(e) => setIns({ ...ins, kind: e.target.value })}
          >
            {INSURANCE_KINDS.map(([k, l]) => (
              <option key={k} value={k}>
                {l}
              </option>
            ))}
          </select>
          <input
            className={field}
            placeholder="Carrier"
            value={ins.carrier}
            onChange={(e) => setIns({ ...ins, carrier: e.target.value })}
          />
          <input
            className={field}
            placeholder="Policy number (optional)"
            value={ins.policy_number}
            onChange={(e) => setIns({ ...ins, policy_number: e.target.value })}
          />
          <input
            className={field}
            placeholder="Limit, e.g. 1,000,000 (optional)"
            inputMode="decimal"
            value={ins.limit}
            onChange={(e) => setIns({ ...ins, limit: e.target.value })}
          />
          <label className="flex items-center gap-2 text-sm text-ink-2">
            Ends
            <input
              type="date"
              className={field}
              value={ins.expires_on}
              onChange={(e) => setIns({ ...ins, expires_on: e.target.value })}
            />
          </label>
          <div className="flex gap-2 sm:justify-end">
            <Button variant="ghost" onClick={() => setInsOpen(false)}>
              Cancel
            </Button>
            <Button
              onClick={addIns}
              disabled={busy || !ins.carrier.trim() || !ins.expires_on}
            >
              Add
            </Button>
          </div>
        </div>
      )}
    </Card>
  );
}
