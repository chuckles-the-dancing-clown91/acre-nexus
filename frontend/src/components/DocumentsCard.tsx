"use client";

// Per-record document drawer (roadmap Phase 2 — document tracking UI).
//
// Drop-in card for any owning record (property, lease, deal, …): lists the
// record's stored files with status, version chain, and expirations; offers
// signed-URL downloads, uploads (new version on re-upload of the same
// filename), and deletes for users holding document:manage.

import { useCallback, useEffect, useRef, useState } from "react";
import { api, ApiError, type DocumentEntry } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { Badge, Card } from "@/components/ui";
import { logError } from "@/lib/log";

export function DocumentsCard({
  ownerType,
  ownerId,
  title = "Documents",
}: {
  ownerType: string;
  ownerId: string;
  title?: string;
}) {
  const { can } = useAuth();
  const manage = can("document:manage");
  const [docs, setDocs] = useState<DocumentEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [showHistory, setShowHistory] = useState<Record<string, boolean>>({});
  const [busy, setBusy] = useState<string | null>(null);
  const fileInput = useRef<HTMLInputElement>(null);

  const load = useCallback(() => {
    api
      .documents({ owner_type: ownerType, owner_id: ownerId })
      .then((d) => {
        setDocs(d);
        setError(null);
      })
      .catch((e) => {
        // 403 = documents module off / no permission: hide the card content
        // rather than erroring the page.
        if (e instanceof ApiError && e.status === 403) setDocs([]);
        else setError((e as Error).message);
        logError("failed to load documents", e);
      });
  }, [ownerType, ownerId]);

  useEffect(() => {
    load();
  }, [load]);

  async function download(id: string) {
    setBusy(`dl-${id}`);
    try {
      const { url } = await api.documentDownloadUrl(id);
      window.open(url, "_blank");
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(null);
    }
  }

  async function remove(id: string) {
    setBusy(`rm-${id}`);
    try {
      await api.deleteDocument(id);
      load();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(null);
    }
  }

  async function upload(file: File) {
    setBusy("upload");
    setError(null);
    try {
      await api.uploadDocument(
        {
          owner_type: ownerType,
          owner_id: ownerId,
          filename: file.name,
          mime_type: file.type || "application/octet-stream",
        },
        file
      );
      load();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(null);
      if (fileInput.current) fileInput.current.value = "";
    }
  }

  // Group version chains: newest row per filename up front, history behind a
  // toggle.
  const byFile = new Map<string, DocumentEntry[]>();
  for (const d of docs ?? []) {
    const list = byFile.get(d.filename) ?? [];
    list.push(d);
    byFile.set(d.filename, list);
  }
  for (const list of byFile.values()) {
    list.sort((a, b) => b.version - a.version);
  }

  return (
    <Card className="overflow-hidden">
      <div className="flex flex-wrap items-center gap-3 border-b border-line px-5 py-4">
        <h2 className="flex-1 font-display text-lg font-bold">{title}</h2>
        {manage && (
          <>
            <input
              ref={fileInput}
              type="file"
              className="hidden"
              onChange={(e) => {
                const f = e.target.files?.[0];
                if (f) upload(f);
              }}
            />
            <button
              onClick={() => fileInput.current?.click()}
              disabled={busy === "upload"}
              className="rounded-lg border border-line px-3 py-1.5 text-sm font-semibold disabled:opacity-50"
            >
              {busy === "upload" ? "Uploading…" : "Upload"}
            </button>
          </>
        )}
      </div>
      {error && <p className="px-5 py-3 text-sm text-bad">{error}</p>}
      <div className="divide-y divide-line">
        {docs === null && (
          <div className="px-5 py-3 text-sm text-ink-3">Loading…</div>
        )}
        {docs !== null && byFile.size === 0 && (
          <div className="px-5 py-6 text-sm text-ink-3">
            No documents on file.
          </div>
        )}
        {[...byFile.entries()].map(([filename, versions]) => {
          const latest = versions[0];
          const history = versions.slice(1);
          const open = showHistory[filename];
          return (
            <div key={filename}>
              <DocumentRow
                doc={latest}
                manage={manage}
                busy={busy}
                onDownload={download}
                onRemove={remove}
              />
              {history.length > 0 && (
                <button
                  onClick={() =>
                    setShowHistory((s) => ({ ...s, [filename]: !open }))
                  }
                  className="w-full px-5 pb-2 text-left text-xs text-ink-3 underline"
                >
                  {open ? "Hide" : "Show"} {history.length} previous version
                  {history.length > 1 ? "s" : ""}
                </button>
              )}
              {open &&
                history.map((v) => (
                  <div key={v.id} className="bg-surface-2">
                    <DocumentRow
                      doc={v}
                      manage={manage}
                      busy={busy}
                      onDownload={download}
                      onRemove={remove}
                    />
                  </div>
                ))}
            </div>
          );
        })}
      </div>
    </Card>
  );
}

function DocumentRow({
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
    <div className="flex flex-wrap items-center gap-3 px-5 py-3 text-sm">
      <div className="min-w-0 flex-1">
        <div className="truncate font-semibold">{doc.filename}</div>
        <div className="text-xs text-ink-3">
          {formatBytes(doc.size_bytes)} · {doc.mime_type} ·{" "}
          {doc.created_at.slice(0, 10)}
        </div>
      </div>
      <Badge tone="neutral">v{doc.version}</Badge>
      <Badge tone={doc.status === "stored" ? "good" : "warn"}>
        {doc.status === "stored" ? "stored" : "pending upload"}
      </Badge>
      {expiry && <Badge tone={expiry.tone}>{expiry.label}</Badge>}
      <button
        onClick={() => onDownload(doc.id)}
        disabled={busy === `dl-${doc.id}` || doc.status !== "stored"}
        className="rounded-lg border border-line px-3 py-1.5 text-xs font-semibold disabled:opacity-50"
      >
        Download
      </button>
      {manage && (
        <button
          onClick={() => onRemove(doc.id)}
          disabled={busy === `rm-${doc.id}`}
          className="text-ink-3 disabled:opacity-50"
          title="Delete this version"
        >
          ✕
        </button>
      )}
    </div>
  );
}

function expiryInfo(
  iso: string | null
): { label: string; tone: "warn" | "bad" | "neutral" } | null {
  if (!iso) return null;
  const expires = new Date(iso).getTime();
  if (Number.isNaN(expires)) return null;
  const days = Math.ceil((expires - Date.now()) / (24 * 3600 * 1000));
  if (days <= 0) return { label: "expired", tone: "bad" };
  if (days <= 30) return { label: `expires in ${days}d`, tone: "warn" };
  return { label: `expires ${iso.slice(0, 10)}`, tone: "neutral" };
}

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / (1024 * 1024)).toFixed(1)} MB`;
}
