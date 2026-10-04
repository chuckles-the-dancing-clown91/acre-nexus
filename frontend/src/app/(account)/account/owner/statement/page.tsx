"use client";

// A month's statement: rent in, expenses out, the fee, the net, per LLC,
// with the work done and the approvals given. Download the PDF.

import { Suspense } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import { useQuery } from "@tanstack/react-query";
import { ChevronLeft, ChevronRight, Download } from "lucide-react";
import { API_BASE, tokenStore } from "@/lib/api";
import { monthName, owner, previousMonth } from "@/lib/owner";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";

export default function StatementPage() {
  return (
    <Suspense fallback={<Skeleton className="h-64" />}>
      <Statement />
    </Suspense>
  );
}

function Statement() {
  const params = useSearchParams();
  const router = useRouter();
  const now = new Date();
  const thisMonth = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}`;
  const month = params.get("month") ?? previousMonth(thisMonth);
  const q = useQuery({
    queryKey: ["owner", "statement", month],
    queryFn: () => owner.statement(month),
  });
  const go = (m: string) =>
    router.replace(`/account/owner/statement?month=${m}`);
  const nextMonth = (() => {
    const [y, m] = month.split("-").map(Number);
    const d = new Date(y, m, 1);
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`;
  })();

  async function download() {
    const res = await fetch(
      `${API_BASE}/my/owner/statement.pdf?month=${month}`,
      {
        headers: { Authorization: `Bearer ${tokenStore.access ?? ""}` },
      }
    );
    if (!res.ok) return;
    const url = URL.createObjectURL(await res.blob());
    const a = document.createElement("a");
    a.href = url;
    a.download = `owner-statement-${month}.pdf`;
    a.click();
    URL.revokeObjectURL(url);
  }

  return (
    <div className="space-y-5">
      <div className="flex items-center justify-between gap-2">
        <Button
          variant="ghost"
          size="sm"
          onClick={() => go(previousMonth(month))}
          aria-label="Previous month"
        >
          <ChevronLeft />
        </Button>
        <h1 className="text-[20px] font-semibold text-fg">
          {monthName(month)}
        </h1>
        <Button
          variant="ghost"
          size="sm"
          onClick={() => go(nextMonth)}
          disabled={nextMonth > thisMonth}
          aria-label="Next month"
        >
          <ChevronRight />
        </Button>
      </div>
      {q.isLoading && <Skeleton className="h-64" />}
      {q.data && (
        <>
          <Panel className="p-4">
            <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
              <Line
                label="Rent collected"
                value={q.data.rent_collected_label}
              />
              <Line label="Expenses" value={q.data.expenses_label} />
              <Line label="Management fee" value={q.data.mgmt_fee_label} />
              <Line label="Net to you" value={q.data.net_label} strong />
            </div>
            <Button
              variant="secondary"
              size="sm"
              className="mt-4"
              onClick={download}
            >
              <Download />
              Download the PDF
            </Button>
          </Panel>
          {q.data.entities.map((e) => (
            <Panel key={e.entity_id} className="p-4">
              <div className="text-[14px] font-semibold text-fg">
                {e.entity_name}
              </div>
              <table className="mt-2 w-full text-[13px]">
                <tbody className="divide-y divide-line">
                  <Row k="Rent collected" v={e.rent_collected_label} />
                  {e.expense_lines.map((l) => (
                    <Row key={l.name} k={l.name} v={`-${l.amount_label}`} />
                  ))}
                  <Row k="Management fee" v={`-${e.mgmt_fee_label}`} />
                  <Row k="Net" v={e.net_label} strong />
                </tbody>
              </table>
            </Panel>
          ))}
          {q.data.work.length > 0 && (
            <Panel className="p-4">
              <div className="text-[14px] font-semibold text-fg">Work done</div>
              <ul className="mt-2 divide-y divide-line text-[13px]">
                {q.data.work.map((w) => (
                  <li
                    key={w.ticket_id}
                    className="flex items-center justify-between gap-3 py-2"
                  >
                    <div className="min-w-0">
                      <div className="truncate text-fg">{w.title}</div>
                      <div className="text-[12px] text-fg-3">
                        {w.property} · {w.resolved_on}
                      </div>
                    </div>
                    <span className="figure shrink-0 text-fg">
                      {w.cost_label}
                    </span>
                  </li>
                ))}
              </ul>
            </Panel>
          )}
          {q.data.approvals.length > 0 && (
            <Panel className="p-4">
              <div className="text-[14px] font-semibold text-fg">
                Your approvals
              </div>
              <ul className="mt-2 divide-y divide-line text-[13px]">
                {q.data.approvals.map((a) => (
                  <li
                    key={a.id}
                    className="flex items-center justify-between gap-3 py-2"
                  >
                    <div className="min-w-0">
                      <div className="truncate text-fg">{a.title}</div>
                      <div className="text-[12px] text-fg-3">
                        {a.kind === "approval" ? "Approval" : "Sign-off"} ·{" "}
                        {a.amount_label}
                      </div>
                    </div>
                    <Badge
                      tone={
                        a.status === "approved"
                          ? "good"
                          : a.status === "pending"
                            ? "warn"
                            : "neutral"
                      }
                    >
                      {a.status}
                    </Badge>
                  </li>
                ))}
              </ul>
            </Panel>
          )}
        </>
      )}
    </div>
  );
}

function Line({
  label,
  value,
  strong,
}: {
  label: string;
  value: string;
  strong?: boolean;
}) {
  return (
    <div>
      <div className="text-[11px] text-fg-3">{label}</div>
      <div
        className={
          strong
            ? "figure text-[18px] font-semibold text-good"
            : "figure text-[18px] font-semibold text-fg"
        }
      >
        {value}
      </div>
    </div>
  );
}

function Row({ k, v, strong }: { k: string; v: string; strong?: boolean }) {
  return (
    <tr>
      <td
        className={strong ? "py-1.5 font-medium text-fg" : "py-1.5 text-fg-2"}
      >
        {k}
      </td>
      <td
        className={
          strong
            ? "figure py-1.5 text-right font-semibold text-fg"
            : "figure py-1.5 text-right text-fg"
        }
      >
        {v}
      </td>
    </tr>
  );
}
