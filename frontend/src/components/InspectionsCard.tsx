"use client";

// Move-in / move-out inspections for one lease (Phase 5): create from the
// standard checklist, rate each line, attach photos via the document service,
// and complete to freeze the report. Reads need `lease:read` (the page
// already gates); writes need `lease:manage`.

import { useCallback, useEffect, useState } from "react";
import {
  api,
  type Inspection,
  type InspectionDetail,
  type InspectionItem,
} from "@/lib/api";
import { toast } from "sonner";
import { Badge, Button, Card } from "@/components/ui";
import { DocumentsCard } from "@/components/DocumentsCard";

const CONDITIONS = ["unrated", "good", "fair", "poor", "damaged"];

function conditionTone(c: string): "good" | "warn" | "bad" | "neutral" {
  if (c === "good") return "good";
  if (c === "fair") return "warn";
  if (c === "poor" || c === "damaged") return "bad";
  return "neutral";
}

export function InspectionsCard({
  leaseId,
  manage,
}: {
  leaseId: string;
  manage: boolean;
}) {
  const [inspections, setInspections] = useState<Inspection[]>([]);
  const [openId, setOpenId] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(() => {
    api
      .leaseInspections(leaseId)
      .then(setInspections)
      .catch(() => setInspections([]));
  }, [leaseId]);

  useEffect(() => {
    load();
  }, [load]);

  async function create(kind: "move_in" | "move_out") {
    setBusy(true);
    try {
      const created = await api.createInspection(leaseId, { kind });
      toast.success("Inspection opened with the standard checklist.");
      load();
      setOpenId(created.id);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Request failed");
    } finally {
      setBusy(false);
    }
  }

  return (
    <Card>
      <div className="flex flex-wrap items-center justify-between gap-3 border-b border-line px-5 py-4">
        <h2 className="font-display text-lg font-bold">Inspections</h2>
        {manage && (
          <div className="flex gap-2">
            <Button
              variant="outline"
              disabled={busy}
              onClick={() => void create("move_in")}
            >
              New move-in
            </Button>
            <Button
              variant="outline"
              disabled={busy}
              onClick={() => void create("move_out")}
            >
              New move-out
            </Button>
          </div>
        )}
      </div>
      {inspections.length === 0 ? (
        <div className="px-5 py-4 text-sm text-ink-3">
          No inspections yet — open one at move-in and move-out.
        </div>
      ) : (
        <ul className="divide-y divide-line">
          {inspections.map((i) => (
            <li key={i.id} className="px-5 py-3">
              <button
                className="flex w-full flex-wrap items-center justify-between gap-3 text-left"
                onClick={() => setOpenId(openId === i.id ? null : i.id)}
              >
                <div>
                  <span className="font-semibold">
                    {i.kind === "move_in" ? "Move-in" : "Move-out"}
                  </span>
                  <span className="ml-2 text-xs text-ink-3">
                    {i.scheduled_date ?? i.created_at.slice(0, 10)} ·{" "}
                    {i.rated_count}/{i.item_count} rated
                  </span>
                </div>
                <Badge tone={i.status === "completed" ? "good" : "neutral"}>
                  {i.status}
                </Badge>
              </button>
              {openId === i.id && (
                <InspectionDetailView
                  inspectionId={i.id}
                  manage={manage}
                  onChange={load}
                />
              )}
            </li>
          ))}
        </ul>
      )}
    </Card>
  );
}

function InspectionDetailView({
  inspectionId,
  manage,
  onChange,
}: {
  inspectionId: string;
  manage: boolean;
  onChange: () => void;
}) {
  const [detail, setDetail] = useState<InspectionDetail | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      setDetail(await api.inspection(inspectionId));
    } catch {
      // Row stays collapsed-looking; nothing else to do.
    }
  }, [inspectionId]);

  useEffect(() => {
    void load();
  }, [load]);

  async function run(fn: () => Promise<unknown>) {
    setBusy(true);
    try {
      await fn();
      await load();
      onChange();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Request failed");
    } finally {
      setBusy(false);
    }
  }

  if (!detail) return <div className="py-3 text-sm text-ink-3">Loading…</div>;
  const editable = manage && detail.status === "draft";

  // Group rows by area for display.
  const groups = new Map<string, InspectionItem[]>();
  for (const item of detail.items) {
    const list = groups.get(item.area) ?? [];
    list.push(item);
    groups.set(item.area, list);
  }

  return (
    <div className="mt-3 space-y-3 rounded-xl border border-line p-4 text-sm">
      {detail.notes && <p className="text-ink-2">{detail.notes}</p>}
      {[...groups.entries()].map(([area, items]) => (
        <div key={area}>
          <div className="mb-1 text-xs font-semibold uppercase tracking-wide text-ink-3">
            {area}
          </div>
          <ul className="space-y-1">
            {items.map((item) => (
              <li
                key={item.id}
                className="flex flex-wrap items-center justify-between gap-2"
              >
                <span className="text-ink-2">
                  {item.item}
                  {item.notes ? (
                    <span className="text-ink-3"> · {item.notes}</span>
                  ) : null}
                </span>
                {editable ? (
                  <select
                    className="rounded-lg border border-line bg-surface px-2 py-1 text-xs"
                    value={item.condition}
                    disabled={busy}
                    onChange={(e) =>
                      void run(() =>
                        api.updateInspectionItem(item.id, {
                          condition: e.target.value,
                        })
                      )
                    }
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
        <div className="flex gap-2 pt-1">
          <Button
            disabled={busy}
            onClick={() => void run(() => api.completeInspection(detail.id))}
          >
            Complete inspection
          </Button>
        </div>
      )}

      {/* Photos ride the document service against the inspection. */}
      <DocumentsCard
        ownerType="inspection"
        ownerId={detail.id}
        title="Photos & attachments"
      />
    </div>
  );
}
