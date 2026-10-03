"use client";

// Taking the data out: each kind of record as a CSV, or everything in one
// zip. The property, tenant, owner and vendor files use the importer's
// columns, so they go straight into another workspace.

import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Download, FileArchive } from "lucide-react";
import { toast } from "sonner";
import { transfer } from "@/lib/datatransfer";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";

export function ExportPanel() {
  const sets = useQuery({ queryKey: ["exports"], queryFn: transfer.exports });
  const [busy, setBusy] = useState<string | null>(null);

  async function get(key: string) {
    setBusy(key);
    try {
      await transfer.download(key);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't download it");
    } finally {
      setBusy(null);
    }
  }

  return (
    <div className="space-y-4">
      <Panel className="flex flex-col gap-4 p-6 sm:flex-row sm:items-center">
        <span className="flex size-12 shrink-0 items-center justify-center rounded-2xl border border-line bg-fill text-fg-2">
          <FileArchive className="size-5" />
        </span>
        <div className="min-w-0 flex-1">
          <div className="text-[15px] font-semibold text-fg">
            Everything, in one zip
          </div>
          <div className="mt-1 text-[13px] text-fg-3">
            One CSV per kind of record, with a note on what&apos;s inside. Opens
            in Excel, Google Sheets and other tools&apos; imports.
          </div>
        </div>
        <Button onClick={() => get("all")} disabled={busy !== null}>
          <Download />
          {busy === "all" ? "Preparing…" : "Download all"}
        </Button>
      </Panel>
      <Panel>
        <PanelHeader title="One file at a time" />
        <ul className="divide-y divide-line p-2 pt-3">
          {sets.isLoading && <Skeleton className="m-3 h-40" />}
          {sets.data?.map((d) => (
            <li key={d.key} className="flex items-center gap-3 px-3 py-3">
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <span className="text-[14px] font-medium text-fg">
                    {d.label}
                  </span>
                  {d.importable && <Badge tone="info">re-importable</Badge>}
                </div>
                <div className="text-xs text-fg-3">{d.description}</div>
              </div>
              <span className="figure w-20 text-right text-xs text-fg-3">
                {d.rows.toLocaleString()} rows
              </span>
              <Button
                size="sm"
                variant="secondary"
                onClick={() => get(d.key)}
                disabled={busy !== null || d.rows === 0}
              >
                <Download />
                CSV
              </Button>
            </li>
          ))}
        </ul>
      </Panel>
    </div>
  );
}
