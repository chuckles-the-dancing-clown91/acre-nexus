"use client";

// Housing assistance on a lease: the voucher (the housing authority pays the
// HAP, the resident pays the rest), the HAP months still owed, and income
// certifications against a percent of area median income.

import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Plus } from "lucide-react";
import { toast } from "sonner";
import {
  CERT_WORDS,
  certTone,
  family,
  incomeLimit,
  qualifies,
  type Assistance as Data,
} from "@/lib/family";
import { usd } from "@/lib/format";
import { dollarsToCents } from "@/lib/showings";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { F, Fact, FormDialog, input, why } from "@/components/property/bits";

const today = () => new Date().toISOString().slice(0, 10);

export function Assistance({
  leaseId,
  manage,
}: {
  leaseId: string;
  manage: boolean;
}) {
  const qc = useQueryClient();
  const key = ["lease-assistance", leaseId];
  const q = useQuery({
    queryKey: key,
    queryFn: () => family.assistance(leaseId),
  });
  const put = (d: Data) => qc.setQueryData(key, d);
  const [vOpen, setVOpen] = useState(false);
  const [cOpen, setCOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [v, setV] = useState({
    authority: "",
    contract_number: "",
    hap: "",
    starts_on: today(),
    ends_on: "",
  });
  const [c, setC] = useState({
    effective_on: today(),
    household_size: "1",
    income: "",
    ami: "",
    limit_pct: "60",
    notes: "",
  });

  async function run(f: () => Promise<Data>, done: string, close?: () => void) {
    setBusy(true);
    try {
      put(await f());
      toast.success(done);
      close?.();
    } catch (e) {
      toast.error(why(e));
    } finally {
      setBusy(false);
    }
  }

  if (q.isLoading) return <Skeleton className="h-48 rounded-2xl" />;
  const d = q.data;
  if (!d) return null;
  const incomeC = dollarsToCents(c.income);
  const amiC = dollarsToCents(c.ami);
  const pct = Number(c.limit_pct);
  const preview =
    incomeC != null && amiC != null && pct > 0
      ? qualifies(incomeC, amiC, pct)
      : null;

  return (
    <div className="grid gap-4 lg:grid-cols-2">
      <Panel>
        <PanelHeader
          title="Housing voucher"
          description="The housing authority pays its part each month; the resident is billed the rest."
          action={
            manage && (
              <Button
                size="sm"
                variant="ghost"
                onClick={() => {
                  setV(
                    d.voucher
                      ? {
                          authority: d.voucher.authority,
                          contract_number: d.voucher.contract_number ?? "",
                          hap: String(d.voucher.hap_cents / 100),
                          starts_on: d.voucher.starts_on,
                          ends_on: d.voucher.ends_on ?? "",
                        }
                      : {
                          authority: "",
                          contract_number: "",
                          hap: "",
                          starts_on: today(),
                          ends_on: "",
                        }
                  );
                  setVOpen(true);
                }}
              >
                {d.voucher ? "Edit" : "Add"}
              </Button>
            )
          }
        />
        <div className="p-5 pt-2">
          {d.voucher ? (
            <dl>
              <Fact label="Housing authority">{d.voucher.authority}</Fact>
              <Fact label="Contract">{d.voucher.contract_number}</Fact>
              <Fact label="Rent">{usd(d.rent_cents)}</Fact>
              <Fact label="Authority pays (HAP)">
                {usd(d.voucher.hap_cents)}
              </Fact>
              <Fact label="Resident pays">{usd(d.resident_share_cents)}</Fact>
              <Fact label="Runs">
                {d.voucher.starts_on} to {d.voucher.ends_on ?? "open"}
              </Fact>
            </dl>
          ) : (
            <p className="text-[13px] text-fg-3">
              No voucher. The resident is billed the full rent.
            </p>
          )}
          {d.hap_due.length > 0 && (
            <div className="mt-4 space-y-2 border-t border-line pt-4">
              <div className="eyebrow">HAP not yet received</div>
              {d.hap_due.map((h) => (
                <div
                  key={h.payment_id}
                  className="flex items-center gap-3 text-[13px]"
                >
                  <span className="flex-1 text-fg-2">
                    {h.due_date} · {usd(h.amount_cents)}
                  </span>
                  {manage && (
                    <Button
                      size="sm"
                      variant="secondary"
                      disabled={busy}
                      onClick={() =>
                        void run(
                          () => family.hapReceived(h.payment_id),
                          "Recorded"
                        )
                      }
                    >
                      Received
                    </Button>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>
      </Panel>

      <Panel>
        <PanelHeader
          title="Income certification"
          description={
            d.foundation
              ? "The household's income against the limit, recertified yearly."
              : "Only needed for leases in a foundation entity."
          }
          action={
            <span className="flex items-center gap-2">
              <Badge tone={certTone(d.certification)}>
                {CERT_WORDS[d.certification]}
              </Badge>
              {manage && (
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => setCOpen(true)}
                >
                  <Plus />
                  Certify
                </Button>
              )}
            </span>
          }
        />
        <div className="p-5 pt-2">
          {d.certifications.length === 0 ? (
            <EmptyState
              title="Not certified yet"
              description="Certify the household's income when they move in and every year after."
            />
          ) : (
            <ul className="divide-y divide-line">
              {d.certifications.map((x) => (
                <li
                  key={x.id}
                  className="flex flex-wrap items-center gap-2 py-2 text-[13px]"
                >
                  <span className="flex-1 text-fg">
                    {x.effective_on} to {x.expires_on}
                    <span className="text-fg-3">
                      {" "}
                      · {x.household_size} in household ·{" "}
                      {usd(x.annual_income_cents)} a year, limit{" "}
                      {usd(x.limit_cents)} ({x.limit_pct}% of AMI)
                    </span>
                  </span>
                  <Badge tone={x.qualified ? "good" : "bad"}>
                    {x.qualified ? "Qualifies" : "Over the limit"}
                  </Badge>
                </li>
              ))}
            </ul>
          )}
        </div>
      </Panel>

      <FormDialog
        open={vOpen}
        onOpenChange={setVOpen}
        title="Housing voucher"
        busy={busy}
        onDelete={
          d.voucher
            ? () =>
                void run(
                  () => family.removeVoucher(leaseId),
                  "Voucher removed",
                  () => setVOpen(false)
                )
            : undefined
        }
        onSave={() => {
          const hap = dollarsToCents(v.hap);
          if (hap == null)
            return void toast.error("Enter what the authority pays");
          void run(
            () =>
              family.setVoucher(leaseId, {
                authority: v.authority,
                contract_number: v.contract_number || undefined,
                hap_cents: hap,
                starts_on: v.starts_on,
                ends_on: v.ends_on || undefined,
              }),
            "Voucher saved",
            () => setVOpen(false)
          );
        }}
      >
        <div className="grid gap-3 sm:grid-cols-2">
          <F label="Housing authority" className="sm:col-span-2">
            <input
              className={input}
              value={v.authority}
              onChange={(e) => setV({ ...v, authority: e.target.value })}
            />
          </F>
          <F label="Contract number">
            <input
              className={input}
              value={v.contract_number}
              onChange={(e) => setV({ ...v, contract_number: e.target.value })}
            />
          </F>
          <F label={`Authority pays each month ($, rent ${usd(d.rent_cents)})`}>
            <input
              className={input}
              inputMode="decimal"
              value={v.hap}
              onChange={(e) => setV({ ...v, hap: e.target.value })}
            />
          </F>
          <F label="Starts">
            <input
              type="date"
              className={input}
              value={v.starts_on}
              onChange={(e) => setV({ ...v, starts_on: e.target.value })}
            />
          </F>
          <F label="Ends (optional)">
            <input
              type="date"
              className={input}
              value={v.ends_on}
              onChange={(e) => setV({ ...v, ends_on: e.target.value })}
            />
          </F>
        </div>
      </FormDialog>

      <FormDialog
        open={cOpen}
        onOpenChange={setCOpen}
        title="Certify income"
        description="Expires a year after it takes effect."
        busy={busy}
        onSave={() => {
          if (incomeC == null || amiC == null)
            return void toast.error("Enter the income and the AMI");
          void run(
            () =>
              family.certify(leaseId, {
                effective_on: c.effective_on,
                household_size: Number(c.household_size),
                annual_income_cents: incomeC,
                ami_cents: amiC,
                limit_pct: pct,
                notes: c.notes || undefined,
              }),
            "Certified",
            () => setCOpen(false)
          );
        }}
      >
        <div className="grid gap-3 sm:grid-cols-2">
          <F label="Takes effect">
            <input
              type="date"
              className={input}
              value={c.effective_on}
              onChange={(e) => setC({ ...c, effective_on: e.target.value })}
            />
          </F>
          <F label="People in household">
            <input
              type="number"
              min={1}
              max={12}
              className={input}
              value={c.household_size}
              onChange={(e) => setC({ ...c, household_size: e.target.value })}
            />
          </F>
          <F label="Household income a year ($)">
            <input
              className={input}
              inputMode="decimal"
              value={c.income}
              onChange={(e) => setC({ ...c, income: e.target.value })}
            />
          </F>
          <F label="Area median income for this size ($)">
            <input
              className={input}
              inputMode="decimal"
              value={c.ami}
              onChange={(e) => setC({ ...c, ami: e.target.value })}
            />
          </F>
          <F label="Limit (% of AMI)">
            <select
              className={input}
              value={c.limit_pct}
              onChange={(e) => setC({ ...c, limit_pct: e.target.value })}
            >
              {[30, 50, 60, 80, 120].map((p) => (
                <option key={p} value={p}>
                  {p}%
                </option>
              ))}
            </select>
          </F>
          <F label="Notes">
            <input
              className={input}
              value={c.notes}
              onChange={(e) => setC({ ...c, notes: e.target.value })}
            />
          </F>
          {preview != null && amiC != null && (
            <p
              className={
                preview
                  ? "text-[13px] text-good sm:col-span-2"
                  : "text-[13px] text-bad sm:col-span-2"
              }
            >
              Limit {usd(incomeLimit(amiC, pct))}.{" "}
              {preview
                ? "This household qualifies."
                : "This household is over the limit."}
            </p>
          )}
        </div>
      </FormDialog>
    </div>
  );
}
