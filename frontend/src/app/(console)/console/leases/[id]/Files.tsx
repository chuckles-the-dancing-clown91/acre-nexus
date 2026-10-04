"use client";

// Stored files for a record (the lease, or one inspection): status, version
// chain and retention. Download through a signed link; upload a file (the same
// filename again makes a new version); delete a version with document:manage.

import { useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Download, FileText, History, Upload, X } from "lucide-react";
import { toast } from "sonner";
import { api, ApiError, type DocumentEntry } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { formatBytes } from "@/lib/utils";
import { Badge, type Tone } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { errorText, useRun } from "../_ui/shared";

export function Files({
  ownerType,
  ownerId,
  title = "Documents",
  description,
  bare,
}: {
  ownerType: string;
  ownerId: string;
  title?: string;
  description?: string;
  /** Leave off the panel, for use inside another one. */
  bare?: boolean;
}) {
  const { can } = useAuth();
  const manage = can("document:manage");
  const key = ["documents", ownerType, ownerId] as const;
  const docs = useQuery({
    queryKey: key,
    queryFn: async (): Promise<DocumentEntry[]> => {
      try {
        return await api.documents({
          owner_type: ownerType,
          owner_id: ownerId,
        });
      } catch (e) {
        // Documents module off, or no permission: show nothing rather than fail.
        if (e instanceof ApiError && e.status === 403) return [];
        throw e;
      }
    },
  });
  const { busy, run } = useRun([key]);
  const [history, setHistory] = useState<Record<string, boolean>>({});
  const fileInput = useRef<HTMLInputElement>(null);

  async function download(id: string) {
    try {
      const { url } = await api.documentDownloadUrl(id);
      window.open(url, "_blank");
    } catch (e) {
      toast.error(errorText(e, "Couldn't download it"));
    }
  }

  // Newest version per filename up front; earlier versions behind a toggle.
  const byFile = new Map<string, DocumentEntry[]>();
  for (const d of docs.data ?? []) {
    const list = byFile.get(d.filename) ?? [];
    list.push(d);
    byFile.set(d.filename, list);
  }
  for (const list of byFile.values())
    list.sort((a, b) => b.version - a.version);

  const uploadButton = manage && (
    <>
      <input
        ref={fileInput}
        type="file"
        className="hidden"
        onChange={(e) => {
          const f = e.target.files?.[0];
          if (!f) return;
          void run(
            "upload",
            () =>
              api.uploadDocument(
                {
                  owner_type: ownerType,
                  owner_id: ownerId,
                  filename: f.name,
                  mime_type: f.type || "application/octet-stream",
                },
                f
              ),
            "Uploaded"
          ).finally(() => {
            if (fileInput.current) fileInput.current.value = "";
          });
        }}
      />
      <Button
        size="sm"
        variant="secondary"
        loading={busy === "upload"}
        onClick={() => fileInput.current?.click()}
      >
        <Upload />
        Upload
      </Button>
    </>
  );

  const body = (
    <>
      {docs.isLoading && <Skeleton className="h-12" />}
      {docs.error && (
        <p className="text-[13px] text-bad">
          Couldn&apos;t load files: {docs.error.message}
        </p>
      )}
      {docs.data && byFile.size === 0 && (
        <EmptyState icon={<FileText />} title="No files yet" className="py-6" />
      )}
      <ul className="space-y-2">
        {[...byFile.entries()].map(([filename, versions]) => {
          const [latest, ...older] = versions;
          const open = history[filename];
          return (
            <li key={filename} className="rounded-xl border border-line">
              <Row
                doc={latest}
                manage={manage}
                busy={busy}
                onDownload={download}
                onRemove={(id) => run(`rm-${id}`, () => api.deleteDocument(id))}
              />
              {older.length > 0 && (
                <button
                  type="button"
                  onClick={() =>
                    setHistory((s) => ({ ...s, [filename]: !open }))
                  }
                  className="flex items-center gap-1.5 px-3 pb-2 text-xs text-fg-3 hover:text-fg"
                >
                  <History className="size-3.5" />
                  {open ? "Hide" : "Show"} {older.length} earlier version
                  {older.length > 1 ? "s" : ""}
                </button>
              )}
              {open &&
                older.map((v) => (
                  <div key={v.id} className="border-t border-line bg-fill/40">
                    <Row
                      doc={v}
                      manage={manage}
                      busy={busy}
                      onDownload={download}
                      onRemove={(id) =>
                        run(`rm-${id}`, () => api.deleteDocument(id))
                      }
                    />
                  </div>
                ))}
            </li>
          );
        })}
      </ul>
    </>
  );

  if (bare) {
    return (
      <div className="space-y-2">
        <div className="flex items-center justify-between gap-2">
          <div className="text-xs font-medium text-fg-3">{title}</div>
          {uploadButton}
        </div>
        {body}
      </div>
    );
  }

  return (
    <Panel>
      <PanelHeader
        title={title}
        description={description}
        action={uploadButton || undefined}
      />
      <div className="space-y-2 p-5 pt-4">{body}</div>
    </Panel>
  );
}

function Row({
  doc,
  manage,
  busy,
  onDownload,
  onRemove,
}: {
  doc: DocumentEntry;
  manage: boolean;
  busy: string | null;
  onDownload: (id: string) => void;
  onRemove: (id: string) => void;
}) {
  const expiry = expiryInfo(doc.retention_expires_at);
  return (
    <div className="flex flex-wrap items-center gap-2 px-3 py-2.5">
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-2">
          <span className="truncate text-[13px] font-medium text-fg">
            {doc.filename}
          </span>
          {doc.category && (
            <Badge tone="info">
              {doc.category.charAt(0).toUpperCase() + doc.category.slice(1)}
            </Badge>
          )}
          {doc.requires_wet_ink && <Badge tone="warn">wet ink</Badge>}
        </div>
        <div className="text-xs text-fg-3">
          {formatBytes(doc.size_bytes)} · {doc.mime_type} ·{" "}
          {doc.created_at.slice(0, 10)}
        </div>
      </div>
      <Badge tone="neutral">v{doc.version}</Badge>
      <Badge tone={doc.status === "stored" ? "good" : "warn"}>
        {doc.status === "stored" ? "stored" : "upload pending"}
      </Badge>
      {expiry && <Badge tone={expiry.tone}>{expiry.label}</Badge>}
      <Button
        size="icon"
        variant="ghost"
        aria-label={`Download ${doc.filename}`}
        disabled={doc.status !== "stored"}
        onClick={() => onDownload(doc.id)}
      >
        <Download />
      </Button>
      {manage && (
        <button
          type="button"
          aria-label="Delete this version"
          disabled={busy === `rm-${doc.id}`}
          onClick={() => {
            if (confirm(`Delete ${doc.filename} v${doc.version}?`))
              onRemove(doc.id);
          }}
          className="rounded-lg p-1.5 text-fg-3 transition hover:bg-fill-2 hover:text-bad"
        >
          <X className="size-4" />
        </button>
      )}
    </div>
  );
}

function expiryInfo(iso: string | null): { label: string; tone: Tone } | null {
  if (!iso) return null;
  const expires = new Date(iso).getTime();
  if (Number.isNaN(expires)) return null;
  const days = Math.ceil((expires - Date.now()) / 86_400_000);
  if (days <= 0) return { label: "expired", tone: "bad" };
  if (days <= 30) return { label: `expires in ${days}d`, tone: "warn" };
  return { label: `expires ${iso.slice(0, 10)}`, tone: "neutral" };
}
