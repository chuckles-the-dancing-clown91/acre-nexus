"use client";

// Move-in and move-out inspections: open one from the standard checklist,
// rate each line, attach photos, and complete it to freeze the report.

import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { ChevronDown, ClipboardCheck, LogIn, LogOut } from "lucide-react";
import { api, type InspectionItem } from "@/lib/api";
import { Badge, type Tone } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { cn } from "@/lib/utils";
import { useRun } from "../_ui/shared";
import { Files } from "./Files";

const CONDITIONS = ["unrated", "good", "fair", "poor", "damaged"];

function conditionTone(c: string): Tone {
  if (c === "good") return "good";
  if (c === "fair") return "warn";
  if (c === "poor" || c === "damaged") return "bad";
  return "neutral";
}

export function Inspections({
  leaseId,
  manage,
}: {
  leaseId: string;
  manage: boolean;
}) {
  const list = useQuery({
    queryKey: ["leases", leaseId, "inspections"],
    queryFn: () => api.leaseInspections(leaseId),
  });
  const { busy, run } = useRun([["leases", leaseId, "inspections"]]);
  const [openId, setOpenId] = useState<string | null>(null);

  async function create(kind: "move_in" | "move_out") {
    await run(
      kind,
      async () => {
        const created = await api.createInspection(leaseId, { kind });
        setOpenId(created.id);
      },
      "Inspection opened with the standard checklist"
    );
  }

  return (
    <Panel>
      <PanelHeader
        title="Inspections"
        description="Open one at move-in and again at move-out."
        action={
          manage && (
            <div className="flex gap-2">
              <Button
                size="sm"
                variant="secondary"
                loading={busy === "move_in"}
                disabled={!!busy}
                onClick={() => void create("move_in")}
              >
                <LogIn />
                Move-in
              </Button>
              <Button
                size="sm"
                variant="secondary"
                loading={busy === "move_out"}
                disabled={!!busy}
                onClick={() => void create("move_out")}
              >
                <LogOut />
                Move-out
              </Button>
            </div>
          )
        }
      />
      <div className="space-y-2 p-5 pt-4">
        {list.isLoading && <Skeleton className="h-14" />}
        {list.error && (
          <p className="text-[13px] text-bad">
            Couldn&apos;t load inspections: {list.error.message}
          </p>
        )}
        {list.data?.length === 0 && (
          <EmptyState
            icon={<ClipboardCheck />}
            title="No inspections yet"
            className="py-6"
          />
        )}
        {list.data?.map((i) => {
          const open = openId === i.id;
          return (
            <div key={i.id} className="rounded-xl border border-line">
              <button
                type="button"
                aria-expanded={open}
                onClick={() => setOpenId(open ? null : i.id)}
                className="flex w-full items-center gap-3 px-3 py-2.5 text-left transition hover:bg-fill-2"
              >
                <div className="min-w-0 flex-1">
                  <div className="text-[13px] font-medium text-fg">
                    {i.kind === "move_in" ? "Move-in" : "Move-out"}
                  </div>
                  <div className="text-xs text-fg-3">
                    {i.scheduled_date ?? i.created_at.slice(0, 10)} ·{" "}
                    {i.rated_count} of {i.item_count} rated
                  </div>
                </div>
                <Badge tone={i.status === "completed" ? "good" : "neutral"}>
                  {i.status}
                </Badge>
                <ChevronDown
                  className={cn(
                    "size-4 text-fg-3 transition",
                    open && "rotate-180"
                  )}
                />
              </button>
              {open && (
                <Detail inspectionId={i.id} leaseId={leaseId} manage={manage} />
              )}
            </div>
          );
        })}
      </div>
    </Panel>
  );
}

function Detail({
  inspectionId,
  leaseId,
  manage,
}: {
  inspectionId: string;
  leaseId: string;
  manage: boolean;
}) {
  const detail = useQuery({
    queryKey: ["leases", leaseId, "inspections", inspectionId],
    queryFn: () => api.inspection(inspectionId),
  });
  const { busy, run } = useRun([["leases", leaseId, "inspections"]]);

  const d = detail.data;
  if (detail.isLoading) return <Skeleton className="m-3 h-24" />;
  if (detail.error)
    return (
      <p className="border-t border-line p-3 text-[13px] text-bad">
        Couldn&apos;t load it: {detail.error.message}
      </p>
    );
  if (!d) return null;
  const editable = manage && d.status === "draft";

  const groups = new Map<string, InspectionItem[]>();
  for (const item of d.items) {
    const list = groups.get(item.area) ?? [];
    list.push(item);
    groups.set(item.area, list);
  }

  return (
    <div className="space-y-4 border-t border-line p-3">
      {d.notes && <p className="text-[13px] text-fg-2">{d.notes}</p>}
      {[...groups.entries()].map(([area, items]) => (
        <div key={area}>
          <div className="eyebrow mb-1.5">{area}</div>
          <ul className="space-y-1">
            {items.map((item) => (
              <li
                key={item.id}
                className="flex flex-wrap items-center justify-between gap-2 text-[13px]"
              >
                <span className="text-fg-2">
                  {item.item}
                  {item.notes && (
                    <span className="text-fg-3"> · {item.notes}</span>
                  )}
                </span>
                {editable ? (
                  <select
                    aria-label={`Condition of ${item.item}`}
                    value={item.condition}
                    disabled={!!busy}
                    onChange={(e) =>
                      void run(item.id, () =>
                        api.updateInspectionItem(item.id, {
                          condition: e.target.value,
                        })
                      )
                    }
                    className="rounded-lg border border-line bg-surface px-2 py-1 text-xs text-fg"
                  >
                    {CONDITIONS.map((c) => (
                      <option key={c} value={c}>
                        {c}
                      </option>
                    ))}
                  </select>
                ) : (
                  <Badge tone={conditionTone(item.condition)}>
                    {item.condition}
                  </Badge>
                )}
              </li>
            ))}
          </ul>
        </div>
      ))}

      {editable && (
        <Button
          size="sm"
          loading={busy === "complete"}
          disabled={!!busy}
          onClick={() =>
            void run(
              "complete",
              () => api.completeInspection(d.id),
              "Inspection completed"
            )
          }
        >
          <ClipboardCheck />
          Complete inspection
        </Button>
      )}

      <Files
        ownerType="inspection"
        ownerId={d.id}
        title="Photos and attachments"
        bare
      />
    </div>
  );
}
