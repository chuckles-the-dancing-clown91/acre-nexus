"use client";

// The lease document, generated from the branding templates. Print it, sign
// it in person (typing the signer's name activates the lease), or send it out
// for e-signature from the panel below.

import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { FileSignature, FileText, Printer, RefreshCw } from "lucide-react";
import { api, ApiError, type LeaseDocDto } from "@/lib/api";
import { logError } from "@/lib/log";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { inputClass, useRun } from "../_ui/shared";

export function useLeaseDoc(leaseId: string, enabled = true) {
  return useQuery({
    queryKey: ["leases", leaseId, "doc"],
    queryFn: async (): Promise<LeaseDocDto | null> => {
      try {
        return await api.leaseDoc(leaseId);
      } catch (e) {
        // 404 means no document has been generated yet.
        if (e instanceof ApiError && e.status === 404) return null;
        logError("failed to load lease document", e);
        throw e;
      }
    },
    enabled,
  });
}

export function LeaseDocument({
  leaseId,
  manage,
}: {
  leaseId: string;
  manage: boolean;
}) {
  const doc = useLeaseDoc(leaseId);
  const { busy, run } = useRun([["leases", leaseId]]);
  const [name, setName] = useState("");
  const d = doc.data;

  return (
    <Panel>
      <PanelHeader
        title="Lease document"
        description={
          d
            ? `${d.title} · generated ${d.generated_at.slice(0, 10)}`
            : undefined
        }
        action={
          <div className="flex flex-wrap items-center justify-end gap-2">
            {d && (
              <Badge tone={d.status === "signed" ? "good" : "warn"}>
                {d.status}
              </Badge>
            )}
            {d && (
              <Button size="sm" variant="secondary" onClick={() => printDoc(d)}>
                <Printer />
                Print or save PDF
              </Button>
            )}
            {manage && (
              <Button
                size="sm"
                variant={d ? "secondary" : "primary"}
                loading={busy === "gen"}
                onClick={() =>
                  run(
                    "gen",
                    () => api.generateLeaseDoc(leaseId),
                    d ? "Document regenerated" : "Document generated"
                  )
                }
              >
                {d ? <RefreshCw /> : <FileText />}
                {d ? "Regenerate" : "Generate"}
              </Button>
            )}
          </div>
        }
      />
      <div className="space-y-4 p-5 pt-4">
        {doc.isLoading && <Skeleton className="h-40" />}
        {doc.error && (
          <p className="text-[13px] text-bad">
            Couldn&apos;t load the document: {doc.error.message}
          </p>
        )}
        {doc.data === null && (
          <EmptyState
            icon={<FileText />}
            title="No document yet"
            description={
              manage
                ? "Generate one from your branding templates."
                : "Nobody has generated the lease document yet."
            }
            className="py-8"
          />
        )}
        {d && (
          <>
            <pre className="max-h-96 overflow-auto rounded-xl border border-line bg-fill/40 p-4 font-mono text-xs leading-relaxed whitespace-pre-wrap text-fg-2">
              {d.body}
            </pre>
            {d.status === "signed" ? (
              <div className="text-[13px] text-good">
                Signed by {d.signed_by} on {d.signed_at?.slice(0, 10)}.
                {d.signed_hash && (
                  <div className="mt-1 font-mono text-xs text-fg-3">
                    sha256 {d.signed_hash.slice(0, 16)}…
                  </div>
                )}
              </div>
            ) : (
              manage && (
                <form
                  className="flex flex-col gap-2 sm:flex-row sm:items-end"
                  onSubmit={async (e) => {
                    e.preventDefault();
                    if (!name.trim()) return;
                    const ok = await run(
                      "sign",
                      () => api.signLeaseDoc(leaseId, name.trim()),
                      "Signed. The lease is active."
                    );
                    if (ok) setName("");
                  }}
                >
                  <label className="block flex-1">
                    <span className="mb-1 block text-xs font-medium text-fg-3">
                      Signing in person? Type the signer&apos;s full name
                    </span>
                    <input
                      value={name}
                      onChange={(e) => setName(e.target.value)}
                      placeholder="Full name"
                      className={inputClass}
                    />
                  </label>
                  <Button
                    type="submit"
                    loading={busy === "sign"}
                    disabled={!name.trim()}
                  >
                    <FileSignature />
                    Sign and activate
                  </Button>
                </form>
              )
            )}
          </>
        )}
      </div>
    </Panel>
  );
}

/** Opens the lease text in a print window; the browser's Save as PDF exports it. */
function printDoc(doc: LeaseDocDto) {
  const w = window.open("", "_blank", "width=800,height=1000");
  if (!w) return;
  const esc = (s: string) =>
    s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  w.document.write(
    `<html><head><title>${esc(doc.title)}</title><style>` +
      `body{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;white-space:pre-wrap;` +
      `padding:48px;font-size:12px;line-height:1.6;color:#111}</style></head>` +
      `<body>${esc(doc.body)}</body></html>`
  );
  w.document.close();
  w.focus();
  w.print();
}
