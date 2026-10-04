"use client";

// Files kept on any record (a deal's data room, a turnover step's photos):
// the newest version of each file up front, older versions behind a toggle,
// signed downloads, uploads (a file with the same name becomes a new
// version) and deletes for people who manage documents.

import { useMemo, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Download, FileText, History, Trash2, Upload } from "lucide-react";
import { toast } from "sonner";
import { api, ApiError, type DocumentEntry } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { Badge, type Tone } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { cn, formatBytes } from "@/lib/utils";

export function Documents({
  ownerType,
  ownerId,
  title = "Documents",
  description,
  accept,
  bare,
}: {
  ownerType: string;
  ownerId: string;
  title?: string;
  description?: string;
  accept?: string;
  /** Render without the panel around it (inside another panel). */
  bare?: boolean;
}) {
  const { can } = useAuth();
  const manage = can("document:manage");
  const qc = useQueryClient();
  const key = ["documents", ownerType, ownerId];
  const docs = useQuery({
    queryKey: key,
    queryFn: async () => {
      try {
        return await api.documents({
          owner_type: ownerType,
          owner_id: ownerId,
        });
      } catch (e) {
        // Documents switched off or not allowed: show nothing rather than fail.
        if (e instanceof ApiError && e.status === 403) return [];
        throw e;
      }
    },
  });
  const [busy, setBusy] = useState<string | null>(null);
  const [open, setOpen] = useState<Record<string, boolean>>({});
  const file = useRef<HTMLInputElement>(null);

  const groups = useMemo(() => {
    const by = new Map<string, DocumentEntry[]>();
    for (const d of docs.data ?? []) {
      const list = by.get(d.filename) ?? [];
      list.push(d);
      by.set(d.filename, list);
    }
    for (const list of by.values()) list.sort((a, b) => b.version - a.version);
    return [...by.entries()];
  }, [docs.data]);

  const refresh = () => qc.invalidateQueries({ queryKey: key });

  async function run(tag: string, fn: () => Promise<unknown>, ok?: string) {
    setBusy(tag);
    try {
      await fn();
      if (ok) toast.success(ok);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "That didn't work");
    } finally {
      setBusy(null);
    }
  }

  const upload = (f: File) =>
    run(
      "upload",
      async () => {
        await api.uploadDocument(
          {
            owner_type: ownerType,
            owner_id: ownerId,
            filename: f.name,
            mime_type: f.type || "application/octet-stream",
          },
          f
        );
        await refresh();
      },
      "Uploaded"
    ).finally(() => {
      if (file.current) file.current.value = "";
    });

  const download = (id: string) =>
    run(`dl-${id}`, async () => {
      const { url } = await api.documentDownloadUrl(id);
      window.open(url, "_blank", "noopener,noreferrer");
    });

  const remove = (d: DocumentEntry) => {
    if (!confirm(`Delete version ${d.version} of ${d.filename}?`)) return;
    void run(
      `rm-${d.id}`,
      async () => {
        await api.deleteDocument(d.id);
        await refresh();
      },
      "Deleted"
    );
  };

  const action = manage && (
    <>
      <input
        ref={file}
        type="file"
        accept={accept}
        className="hidden"
        onChange={(e) => {
          const f = e.target.files?.[0];
          if (f) void upload(f);
        }}
      />
      <Button
        size="sm"
        variant="secondary"
        loading={busy === "upload"}
        onClick={() => file.current?.click()}
      >
        <Upload />
        Upload
      </Button>
    </>
  );

  const body = (
    <div className={bare ? "" : "p-2 pt-3"}>
      {docs.isLoading && <Skeleton className="m-3 h-14" />}
      {docs.error && (
        <p className="px-3 py-2 text-[13px] text-bad">
          Couldn&apos;t load files: {docs.error.message}
        </p>
      )}
      {docs.data && groups.length === 0 && (
        <EmptyState
          icon={<FileText />}
          title="No files yet"
          description={manage ? "Upload one to keep it here." : undefined}
          className="py-8"
        />
      )}
      <ul className="divide-y divide-line">
        {groups.map(([name, versions]) => {
          const older = versions.slice(1);
          const shown = open[name];
          return (
            <li key={name}>
              <Row
                doc={versions[0]}
                manage={manage}
                busy={busy}
                onDownload={download}
                onRemove={remove}
              />
              {older.length > 0 && (
                <button
                  type="button"
                  onClick={() => setOpen((s) => ({ ...s, [name]: !shown }))}
                  className="ml-3 inline-flex items-center gap-1 pb-2 text-xs text-fg-3 transition hover:text-fg"
                >
                  <History className="size-3.5" />
                  {shown ? "Hide" : "Show"} {older.length} older version
                  {older.length > 1 ? "s" : ""}
                </button>
              )}
              {shown &&
                older.map((v) => (
                  <div key={v.id} className="rounded-xl bg-fill/60">
                    <Row
                      doc={v}
                      manage={manage}
                      busy={busy}
                      onDownload={download}
                      onRemove={remove}
                    />
                  </div>
                ))}
            </li>
          );
        })}
      </ul>
    </div>
  );

  if (bare)
    return (
      <div className="rounded-xl border border-line">
        <div className="flex items-center justify-between gap-3 px-3 py-2">
          <span className="text-[13px] font-medium text-fg">{title}</span>
          {action}
        </div>
        {body}
      </div>
    );

  return (
    <Panel>
      <PanelHeader
        title={title}
        description={description}
        action={action ? <div className="flex gap-2">{action}</div> : null}
      />
      {body}
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
  onRemove: (d: DocumentEntry) => void;
}) {
  const expiry = expiryInfo(doc.retention_expires_at);
  const stored = doc.status === "stored";
  return (
    <div className="flex flex-wrap items-center gap-2 px-3 py-2.5">
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-2">
          <span className="truncate text-[13px] font-medium text-fg">
            {doc.filename}
          </span>
          {doc.category && (
            <Badge tone="info">{doc.category.replace(/_/g, " ")}</Badge>
          )}
          {doc.requires_wet_ink && <Badge tone="warn">wet ink</Badge>}
        </div>
        <div className="text-xs text-fg-3">
          {formatBytes(doc.size_bytes)} · {doc.mime_type} ·{" "}
          {doc.created_at.slice(0, 10)}
        </div>
      </div>
      <Badge>v{doc.version}</Badge>
      <Badge tone={stored ? "good" : "warn"}>
        {stored ? "stored" : "upload pending"}
      </Badge>
      {expiry && <Badge tone={expiry.tone}>{expiry.label}</Badge>}
      <Button
        size="icon"
        variant="ghost"
        aria-label={`Download ${doc.filename}`}
        disabled={!stored || busy === `dl-${doc.id}`}
        onClick={() => onDownload(doc.id)}
      >
        <Download />
      </Button>
      {manage && (
        <Button
          size="icon"
          variant="ghost"
          aria-label={`Delete version ${doc.version} of ${doc.filename}`}
          disabled={busy === `rm-${doc.id}`}
          onClick={() => onRemove(doc)}
          className={cn("hover:text-bad")}
        >
          <Trash2 />
        </Button>
      )}
    </div>
  );
}

function expiryInfo(iso: string | null): { label: string; tone: Tone } | null {
  if (!iso) return null;
  const t = new Date(iso).getTime();
  if (Number.isNaN(t)) return null;
  const days = Math.ceil((t - Date.now()) / 86_400_000);
  if (days <= 0) return { label: "expired", tone: "bad" };
  if (days <= 30) return { label: `expires in ${days}d`, tone: "warn" };
  return { label: `expires ${iso.slice(0, 10)}`, tone: "neutral" };
}
