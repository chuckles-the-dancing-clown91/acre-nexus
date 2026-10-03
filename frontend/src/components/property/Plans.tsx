"use client";

// Drawings of the property: floor plans, blueprints, surveys and permit
// sets. Upload them here; open one to look at it without downloading.

import { useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { FileText, Layers, Upload } from "lucide-react";
import { toast } from "sonner";
import { api, type DocumentEntry } from "@/lib/api";
import { label, PLAN_CATEGORIES } from "@/lib/propertyRecords";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/menu";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { cn } from "@/lib/utils";
import { input, why } from "./bits";

const isPlan = (d: DocumentEntry) =>
  (PLAN_CATEGORIES as readonly string[]).includes(d.category ?? "");

export function Plans({
  propertyId,
  manage,
}: {
  propertyId: string;
  manage: boolean;
}) {
  const qc = useQueryClient();
  const docs = useQuery({
    queryKey: ["property-documents", propertyId],
    queryFn: () => api.propertyDocuments(propertyId),
  });
  const file = useRef<HTMLInputElement>(null);
  const [category, setCategory] = useState<string>("floorplan");
  const [busy, setBusy] = useState(false);
  const [filter, setFilter] = useState<string>("all");
  const [viewing, setViewing] = useState<{
    doc: DocumentEntry;
    url: string;
  } | null>(null);

  async function upload(list: FileList | null) {
    if (!list?.length) return;
    setBusy(true);
    try {
      for (const f of Array.from(list)) {
        await api.uploadDocument(
          {
            owner_type: "property",
            owner_id: propertyId,
            filename: f.name,
            mime_type: f.type || "application/octet-stream",
            category,
          },
          f
        );
      }
      toast.success(list.length === 1 ? "Uploaded" : `${list.length} uploaded`);
      void qc.invalidateQueries({
        queryKey: ["property-documents", propertyId],
      });
    } catch (e) {
      toast.error(why(e, "Upload failed"));
    } finally {
      setBusy(false);
      if (file.current) file.current.value = "";
    }
  }

  async function view(doc: DocumentEntry) {
    try {
      const { url } = await api.documentDownloadUrl(doc.id);
      setViewing({ doc, url });
    } catch (e) {
      toast.error(why(e, "Couldn't open it"));
    }
  }

  const plans = (docs.data?.documents ?? []).filter(isPlan);
  const shown = plans.filter((d) => filter === "all" || d.category === filter);

  return (
    <Panel>
      <PanelHeader
        title="Plans and blueprints"
        description="Floor plans, construction drawings, surveys and permit sets."
        action={
          manage && (
            <div className="flex items-center gap-2">
              <select
                aria-label="Kind of drawing"
                className={cn(input, "w-auto py-1.5")}
                value={category}
                onChange={(e) => setCategory(e.target.value)}
              >
                {PLAN_CATEGORIES.map((c) => (
                  <option key={c} value={c}>
                    {c === "permit" ? "Permit set" : label(c)}
                  </option>
                ))}
              </select>
              <input
                ref={file}
                type="file"
                multiple
                accept="image/*,application/pdf,.dwg,.dxf"
                className="hidden"
                onChange={(e) => upload(e.target.files)}
              />
              <Button
                size="sm"
                variant="secondary"
                disabled={busy}
                onClick={() => file.current?.click()}
              >
                <Upload />
                Upload
              </Button>
            </div>
          )
        }
      />
      <div className="p-5 pt-4">
        {plans.length > 0 && (
          <div className="mb-3 flex flex-wrap gap-1.5">
            {["all", ...PLAN_CATEGORIES].map((c) => {
              const n =
                c === "all"
                  ? plans.length
                  : plans.filter((d) => d.category === c).length;
              if (c !== "all" && n === 0) return null;
              return (
                <button
                  key={c}
                  type="button"
                  onClick={() => setFilter(c)}
                  className={cn(
                    "rounded-full border px-2.5 py-1 text-xs transition",
                    filter === c
                      ? "border-accent bg-accent/10 text-accent"
                      : "border-line text-fg-3 hover:text-fg"
                  )}
                >
                  {c === "all"
                    ? "All"
                    : c === "permit"
                      ? "Permit sets"
                      : label(c)}{" "}
                  · {n}
                </button>
              );
            })}
          </div>
        )}
        {docs.isLoading && <Skeleton className="h-28" />}
        {docs.isSuccess && plans.length === 0 && (
          <EmptyState
            icon={<Layers />}
            title="No drawings yet"
            description="Upload floor plans, blueprints or the survey. PDFs and images open right here."
            className="py-6"
          />
        )}
        <ul className="grid grid-cols-2 gap-3 sm:grid-cols-3 xl:grid-cols-4">
          {shown.map((d) => (
            <li key={d.id}>
              <button
                type="button"
                onClick={() => view(d)}
                className="group flex w-full flex-col rounded-xl border border-line p-3 text-left transition hover:border-accent/50"
              >
                <span className="flex aspect-[4/3] w-full items-center justify-center rounded-lg bg-fill text-fg-3 group-hover:text-accent">
                  {d.mime_type.startsWith("image/") ? (
                    <Layers className="size-8" />
                  ) : (
                    <FileText className="size-8" />
                  )}
                </span>
                <span className="mt-2 truncate text-[13px] font-medium text-fg">
                  {d.filename}
                </span>
                <span className="mt-1 flex items-center gap-1.5 text-[11px] text-fg-3">
                  <Badge>
                    {d.category === "permit"
                      ? "Permit set"
                      : label(d.category ?? "")}
                  </Badge>
                  {new Date(d.created_at).toLocaleDateString()}
                </span>
              </button>
            </li>
          ))}
        </ul>
      </div>

      <Dialog open={!!viewing} onOpenChange={(o) => !o && setViewing(null)}>
        <DialogContent className="flex h-[88dvh] max-w-5xl flex-col">
          {viewing && (
            <>
              <DialogTitle className="truncate pr-8 text-[15px] font-semibold">
                {viewing.doc.filename}
              </DialogTitle>
              <div className="mt-3 min-h-0 flex-1 overflow-auto rounded-lg bg-fill">
                {viewing.doc.mime_type.startsWith("image/") ? (
                  // eslint-disable-next-line @next/next/no-img-element -- signed blob URL
                  <img
                    src={viewing.url}
                    alt={viewing.doc.filename}
                    className="mx-auto max-w-none"
                  />
                ) : viewing.doc.mime_type === "application/pdf" ? (
                  <iframe
                    src={viewing.url}
                    title={viewing.doc.filename}
                    className="size-full"
                  />
                ) : (
                  <div className="flex h-full items-center justify-center p-6 text-center text-[13px] text-fg-3">
                    This file opens in a drawing program.
                  </div>
                )}
              </div>
              <div className="mt-3 flex justify-end">
                <Button variant="secondary" asChild>
                  <a href={viewing.url} target="_blank" rel="noreferrer">
                    Open in a new tab
                  </a>
                </Button>
              </div>
            </>
          )}
        </DialogContent>
      </Dialog>
    </Panel>
  );
}
