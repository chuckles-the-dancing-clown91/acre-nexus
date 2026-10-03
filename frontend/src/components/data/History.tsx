"use client";

// Past imports: what each one made, and undo, which keeps anything that's
// been used or changed since (a lease with a payment, a property with a work
// order) and says so.

import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { History as HistoryIcon, Undo2 } from "lucide-react";
import { toast } from "sonner";
import {
  madeSentence,
  transfer,
  type Batch,
  type Preview,
  type UndoReport,
} from "@/lib/datatransfer";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/menu";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";

export function History({ onResume }: { onResume: (p: Preview) => void }) {
  const qc = useQueryClient();
  const batches = useQuery({
    queryKey: ["imports"],
    queryFn: transfer.history,
  });
  const [undoing, setUndoing] = useState<Batch | null>(null);
  const [report, setReport] = useState<UndoReport | null>(null);
  const [busy, setBusy] = useState(false);

  async function undo(b: Batch) {
    setBusy(true);
    try {
      const r = await transfer.undo(b.id);
      setReport(r.report);
      setUndoing(null);
      for (const k of [
        "imports",
        "properties",
        "portfolio-summary",
        "exports",
      ]) {
        void qc.invalidateQueries({ queryKey: [k] });
      }
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't undo it");
    } finally {
      setBusy(false);
    }
  }

  async function resume(b: Batch) {
    try {
      onResume(await transfer.preview(b.id));
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't open it");
    }
  }

  return (
    <Panel>
      <PanelHeader
        title="Import history"
        description="Every import, what it made, and undo."
      />
      <div className="p-2 pt-3">
        {batches.isLoading && <Skeleton className="m-3 h-24" />}
        {batches.data?.length === 0 && (
          <EmptyState
            icon={<HistoryIcon />}
            title="No imports yet"
            className="py-8"
          />
        )}
        <ul className="divide-y divide-line">
          {batches.data?.map((b) => (
            <li
              key={b.id}
              className="flex flex-wrap items-center gap-3 px-3 py-3"
            >
              <div className="min-w-0 flex-1">
                <div className="truncate text-[14px] font-medium text-fg">
                  {b.filename}
                </div>
                <div className="truncate text-xs text-fg-3">
                  {b.kind_label} · {b.source_label} ·{" "}
                  {new Date(b.committed_at ?? b.created_at).toLocaleString()}
                  {b.status !== "draft" && ` · made ${madeSentence(b.summary)}`}
                  {b.status === "undone" && b.summary.undo
                    ? ` · undone${b.summary.undo.kept ? `, ${b.summary.undo.kept} kept` : ""}`
                    : ""}
                </div>
              </div>
              <Badge
                tone={
                  b.status === "done"
                    ? "good"
                    : b.status === "draft"
                      ? "warn"
                      : "neutral"
                }
              >
                {b.status === "done"
                  ? "imported"
                  : b.status === "draft"
                    ? "not run"
                    : "undone"}
              </Badge>
              {b.status === "done" && (
                <Button size="sm" variant="ghost" onClick={() => setUndoing(b)}>
                  <Undo2 />
                  Undo
                </Button>
              )}
              {b.status === "draft" && (
                <>
                  <Button
                    size="sm"
                    variant="secondary"
                    onClick={() => resume(b)}
                  >
                    Continue
                  </Button>
                  <Button
                    size="sm"
                    variant="ghost"
                    onClick={async () => {
                      await transfer.discard(b.id).catch(() => undefined);
                      void qc.invalidateQueries({ queryKey: ["imports"] });
                    }}
                  >
                    Discard
                  </Button>
                </>
              )}
            </li>
          ))}
        </ul>
      </div>

      <Dialog open={!!undoing} onOpenChange={(o) => !o && setUndoing(null)}>
        <DialogContent>
          <DialogTitle className="text-[17px] font-semibold">
            Undo {undoing?.filename}?
          </DialogTitle>
          <DialogDescription className="mt-2 text-[13px] text-fg-3">
            This removes the {undoing ? madeSentence(undoing.summary) : ""} it
            made. Anything used or changed since stays: a lease with a payment,
            a property with a work order or a listing, a vendor on a job.
          </DialogDescription>
          <div className="mt-5 flex justify-end gap-2">
            <Button variant="ghost" onClick={() => setUndoing(null)}>
              Keep it
            </Button>
            <Button
              variant="danger"
              disabled={busy}
              onClick={() => undoing && undo(undoing)}
            >
              Undo import
            </Button>
          </div>
        </DialogContent>
      </Dialog>

      <Dialog open={!!report} onOpenChange={(o) => !o && setReport(null)}>
        <DialogContent>
          <DialogTitle className="text-[17px] font-semibold">
            Undone
          </DialogTitle>
          <DialogDescription className="mt-2 text-[13px] text-fg-3">
            Removed{" "}
            {madeSentence({
              properties: report?.removed.property,
              units: report?.removed.unit,
              leases: report?.removed.lease,
              owners: report?.removed.owner,
              vendors: report?.removed.vendor,
            }).replace("nothing new", "nothing")}
            .
          </DialogDescription>
          {report && report.kept.length > 0 && (
            <div className="mt-4">
              <div className="eyebrow mb-2">Kept</div>
              <ul className="max-h-60 space-y-1 overflow-y-auto text-[13px]">
                {report.kept.map((k) => (
                  <li key={k.id} className="flex gap-2">
                    <span className="w-16 shrink-0 text-fg-3">{k.t}</span>
                    <span className="text-fg-2">{k.reason}</span>
                  </li>
                ))}
              </ul>
            </div>
          )}
          <div className="mt-5 flex justify-end">
            <Button onClick={() => setReport(null)}>Done</Button>
          </div>
        </DialogContent>
      </Dialog>
    </Panel>
  );
}
