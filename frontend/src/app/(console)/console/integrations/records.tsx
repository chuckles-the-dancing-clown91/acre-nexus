"use client";

// Stored documents (attached to a record, fetched through short-lived signed
// links, versioned by filename) and the log of email and texts the platform
// has sent.

import { useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Download,
  FileText,
  Mail,
  MessageSquare,
  Send,
  Trash2,
  Upload,
} from "lucide-react";
import { toast } from "sonner";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { useHasTenantScope } from "@/components/shell/tenant-scope";
import { Badge, type Tone } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { fieldClass, Input, Label } from "@/components/ui/input";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { cn, formatBytes } from "@/lib/utils";
import { errorText } from "./bits";

const OWNER_TYPES: [string, string][] = [
  ["property", "Property"],
  ["lease", "Lease"],
  ["application", "Application"],
  ["entity", "Entity"],
  ["deal", "Deal"],
  ["unit", "Unit"],
  ["maintenance_ticket", "Work order"],
  ["tenant", "Resident"],
];

const ownerLabel = (t: string) =>
  OWNER_TYPES.find(([k]) => k === t)?.[1] ?? t.replace(/_/g, " ");

function tone(status: string): Tone {
  if (status === "sent" || status === "stored") return "good";
  if (status === "failed") return "bad";
  if (status === "queued" || status === "pending_upload") return "warn";
  return "neutral";
}

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

export function DocumentsTab() {
  const { can } = useAuth();
  const scoped = useHasTenantScope();
  const manage = can("document:manage");
  const qc = useQueryClient();
  const [filter, setFilter] = useState("");
  const docs = useQuery({
    queryKey: ["documents", filter],
    queryFn: () => api.documents(filter ? { owner_type: filter } : {}),
    enabled: scoped && can("document:read"),
  });
  const refresh = () => qc.invalidateQueries({ queryKey: ["documents"] });

  const download = useMutation({
    mutationFn: (id: string) => api.documentDownloadUrl(id),
    onSuccess: ({ url }) => window.open(url, "_blank", "noopener"),
    onError: (e) => toast.error(errorText(e, "Couldn't get a download link")),
  });
  const remove = useMutation({
    mutationFn: (id: string) => api.deleteDocument(id),
    onSuccess: () => {
      toast.success("Document deleted");
      refresh();
    },
    onError: (e) => toast.error(errorText(e, "Couldn't delete it")),
  });

  return (
    <div className="space-y-6">
      {manage && <UploadPanel onDone={refresh} />}
      <Panel>
        <PanelHeader
          title="Documents"
          description="Links to download are signed and expire after a short while."
          action={
            <select
              aria-label="Attached to"
              className={fieldClass}
              value={filter}
              onChange={(e) => setFilter(e.target.value)}
            >
              <option value="">Everything</option>
              {OWNER_TYPES.map(([k, l]) => (
                <option key={k} value={k}>
                  {l}
                </option>
              ))}
            </select>
          }
        />
        <div className="pt-3">
          {docs.isLoading && (
            <div className="px-5 pb-5">
              <Skeleton className="h-32" />
            </div>
          )}
          {docs.error && (
            <p className="px-5 pb-5 text-[13px] text-bad">
              Couldn&apos;t load documents: {docs.error.message}
            </p>
          )}
          {docs.data?.length === 0 && (
            <EmptyState
              icon={<FileText />}
              title="No documents yet"
              description="Files you attach to properties, leases and other records show up here."
            />
          )}
          <ul className="divide-y divide-line">
            {docs.data?.map((d) => (
              <li
                key={d.id}
                className="flex flex-col gap-2 px-5 py-3 sm:flex-row sm:items-center"
              >
                <div className="flex min-w-0 flex-1 items-start gap-3">
                  <span className="mt-0.5 flex size-8 shrink-0 items-center justify-center rounded-lg border border-line bg-fill text-fg-2">
                    <FileText className="size-4" />
                  </span>
                  <div className="min-w-0">
                    <div className="flex flex-wrap items-center gap-2">
                      <span className="truncate text-[14px] font-medium text-fg">
                        {d.filename}
                      </span>
                      <Badge tone={tone(d.status)}>
                        {d.status.replace(/_/g, " ")}
                      </Badge>
                      {d.version > 1 && <Badge tone="info">v{d.version}</Badge>}
                    </div>
                    <div className="mt-0.5 text-xs text-fg-3">
                      {ownerLabel(d.owner_type)} · {formatBytes(d.size_bytes)} ·{" "}
                      {new Date(d.created_at).toLocaleDateString()}
                      {d.category && ` · ${d.category}`}
                    </div>
                  </div>
                </div>
                <div className="flex shrink-0 gap-2">
                  <Button
                    size="sm"
                    variant="secondary"
                    disabled={download.isPending}
                    onClick={() => download.mutate(d.id)}
                  >
                    <Download />
                    Download
                  </Button>
                  {manage && (
                    <Button
                      size="sm"
                      variant="ghost"
                      aria-label={`Delete ${d.filename}`}
                      disabled={remove.isPending}
                      onClick={() => {
                        if (
                          window.confirm(
                            `Delete ${d.filename}? Earlier versions stay.`
                          )
                        )
                          remove.mutate(d.id);
                      }}
                    >
                      <Trash2 />
                    </Button>
                  )}
                </div>
              </li>
            ))}
          </ul>
        </div>
      </Panel>
    </div>
  );
}

function UploadPanel({ onDone }: { onDone: () => void }) {
  const [ownerType, setOwnerType] = useState("property");
  const [ownerId, setOwnerId] = useState("");
  const fileInput = useRef<HTMLInputElement | null>(null);
  const idOk = UUID.test(ownerId.trim());
  const upload = useMutation({
    mutationFn: (file: File) =>
      api.uploadDocument(
        {
          owner_type: ownerType,
          owner_id: ownerId.trim(),
          filename: file.name,
          mime_type: file.type || "application/octet-stream",
        },
        file
      ),
    onSuccess: (d) => {
      toast.success(
        d.version > 1 ? `Uploaded as version ${d.version}` : "Uploaded"
      );
      onDone();
    },
    onError: (e) => toast.error(errorText(e, "Upload failed")),
    onSettled: () => {
      if (fileInput.current) fileInput.current.value = "";
    },
  });

  return (
    <Panel>
      <PanelHeader
        title="Upload a file"
        description="Attach it to a record by its id. Uploading the same filename again makes a new version."
      />
      <div className="grid gap-3 p-5 sm:grid-cols-[12rem_1fr_auto] sm:items-end">
        <div className="space-y-1.5">
          <Label htmlFor="doc-owner-type">Attach to</Label>
          <select
            id="doc-owner-type"
            className={cn(fieldClass, "block h-11 w-full")}
            value={ownerType}
            onChange={(e) => setOwnerType(e.target.value)}
          >
            {OWNER_TYPES.map(([k, l]) => (
              <option key={k} value={k}>
                {l}
              </option>
            ))}
          </select>
        </div>
        <div className="space-y-1.5">
          <Label htmlFor="doc-owner-id">Record id</Label>
          <Input
            id="doc-owner-id"
            className="font-mono"
            placeholder="The record's id"
            value={ownerId}
            onChange={(e) => setOwnerId(e.target.value)}
            aria-invalid={ownerId.trim() !== "" && !idOk ? true : undefined}
          />
        </div>
        <Button
          size="lg"
          disabled={!idOk}
          loading={upload.isPending}
          onClick={() => fileInput.current?.click()}
        >
          {!upload.isPending && <Upload />}
          Choose file
        </Button>
        <input
          ref={fileInput}
          type="file"
          className="hidden"
          onChange={(e) => {
            const f = e.target.files?.[0];
            if (f) upload.mutate(f);
          }}
        />
      </div>
    </Panel>
  );
}

// ---- Sent log ----

export function SentTab() {
  const { can } = useAuth();
  const scoped = useHasTenantScope();
  const [channel, setChannel] = useState("");
  const log = useQuery({
    queryKey: ["integration-notifications"],
    queryFn: () => api.notifications(),
    enabled: scoped && can("integrations:manage"),
  });
  const rows = (log.data ?? []).filter(
    (n) => !channel || n.channel === channel
  );
  const channels = [...new Set((log.data ?? []).map((n) => n.channel))];
  const failed = (log.data ?? []).filter((n) => n.status === "failed").length;

  return (
    <Panel>
      <PanelHeader
        title="Sent log"
        description="Email and texts the platform sent, like welcome emails and reminders, with how delivery went."
        action={
          <div className="flex items-center gap-2">
            {failed > 0 && <Badge tone="bad">{failed} failed</Badge>}
            {channels.length > 1 && (
              <select
                aria-label="Channel"
                className={fieldClass}
                value={channel}
                onChange={(e) => setChannel(e.target.value)}
              >
                <option value="">All channels</option>
                {channels.map((c) => (
                  <option key={c} value={c}>
                    {c}
                  </option>
                ))}
              </select>
            )}
          </div>
        }
      />
      <div className="pt-3">
        {log.isLoading && (
          <div className="px-5 pb-5">
            <Skeleton className="h-32" />
          </div>
        )}
        {log.error && (
          <p className="px-5 pb-5 text-[13px] text-bad">
            Couldn&apos;t load the log: {log.error.message}
          </p>
        )}
        {log.data && rows.length === 0 && (
          <EmptyState
            icon={<Send />}
            title="Nothing sent yet"
            description="Messages show up here as the platform sends them."
          />
        )}
        <ul className="divide-y divide-line">
          {rows.map((n) => (
            <li key={n.id} className="flex items-start gap-3 px-5 py-3">
              <span className="mt-0.5 flex size-8 shrink-0 items-center justify-center rounded-lg border border-line bg-fill text-fg-2">
                {n.channel === "email" ? (
                  <Mail className="size-4" />
                ) : (
                  <MessageSquare className="size-4" />
                )}
              </span>
              <div className="min-w-0 flex-1">
                <div className="flex flex-wrap items-center gap-2">
                  <span className="truncate text-[14px] font-medium text-fg">
                    {n.subject ?? n.template_key.replace(/_/g, " ")}
                  </span>
                  <Badge tone="neutral">{n.channel}</Badge>
                </div>
                <div className="mt-0.5 text-xs text-fg-3">
                  To {n.recipient} · {new Date(n.created_at).toLocaleString()}
                </div>
                {n.last_error && (
                  <p className="mt-1 text-xs text-bad">{n.last_error}</p>
                )}
              </div>
              <Badge tone={tone(n.status)}>{n.status}</Badge>
            </li>
          ))}
        </ul>
      </div>
    </Panel>
  );
}
