"use client";

// Integrations console: the credential vault (write-only — values are stored
// encrypted server-side and only ever shown masked), the document service
// (upload / versioned list / signed-URL download), and the outbound
// notification log. Backed by the backend `integrations` module.

import { useCallback, useEffect, useRef, useState } from "react";
import {
  api,
  type DocumentEntry,
  type IntegrationSecret,
  type NotificationEntry,
} from "@/lib/api";
import { Badge, Card } from "@/components/ui";
import { useAuth } from "@/lib/auth";
import { formatBytes } from "@/lib/utils";

const OWNER_TYPES = [
  "property",
  "lease",
  "application",
  "entity",
  "deal",
  "unit",
  "maintenance_ticket",
  "tenant",
];

function statusToneFor(status: string): "good" | "warn" | "bad" | "neutral" {
  if (status === "sent" || status === "stored") return "good";
  if (status === "failed") return "bad";
  if (status === "queued" || status === "pending_upload") return "warn";
  return "neutral";
}

export default function IntegrationsPage() {
  const { can } = useAuth();
  const manageIntegrations = can("integrations:manage");
  const readDocs = can("document:read");
  const manageDocs = can("document:manage");

  const [error, setError] = useState<string | null>(null);

  // ---- credentials ----
  const [secrets, setSecrets] = useState<IntegrationSecret[] | null>(null);
  const [secretKey, setSecretKey] = useState("");
  const [secretValue, setSecretValue] = useState("");
  const [savingSecret, setSavingSecret] = useState(false);

  // ---- documents ----
  const [docs, setDocs] = useState<DocumentEntry[] | null>(null);
  const [ownerType, setOwnerType] = useState("property");
  const [ownerId, setOwnerId] = useState("");
  const [uploading, setUploading] = useState(false);
  const fileInput = useRef<HTMLInputElement | null>(null);

  // ---- notifications ----
  const [log, setLog] = useState<NotificationEntry[] | null>(null);

  const load = useCallback(() => {
    if (manageIntegrations) {
      api
        .integrationSecrets()
        .then(setSecrets)
        .catch((e) => setError(e.message));
      api
        .notifications()
        .then(setLog)
        .catch((e) => setError(e.message));
    }
    if (readDocs) {
      api
        .documents()
        .then(setDocs)
        .catch((e) => setError(e.message));
    }
  }, [manageIntegrations, readDocs]);

  useEffect(() => {
    load();
  }, [load]);

  async function saveSecret(e: React.FormEvent) {
    e.preventDefault();
    if (!secretKey.trim() || !secretValue.trim()) return;
    setSavingSecret(true);
    setError(null);
    try {
      await api.setIntegrationSecret(secretKey.trim(), secretValue.trim());
      setSecretKey("");
      setSecretValue("");
      load();
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setSavingSecret(false);
    }
  }

  async function removeSecret(key: string) {
    try {
      await api.deleteIntegrationSecret(key);
      load();
    } catch (err) {
      setError((err as Error).message);
    }
  }

  async function uploadFile(file: File) {
    if (!ownerId.trim()) {
      setError("Enter the owning record's id before uploading.");
      return;
    }
    setUploading(true);
    setError(null);
    try {
      await api.uploadDocument(
        {
          owner_type: ownerType,
          owner_id: ownerId.trim(),
          filename: file.name,
          mime_type: file.type || "application/octet-stream",
        },
        file
      );
      load();
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setUploading(false);
      if (fileInput.current) fileInput.current.value = "";
    }
  }

  async function downloadDoc(id: string) {
    try {
      const { url } = await api.documentDownloadUrl(id);
      window.open(url, "_blank", "noopener");
    } catch (err) {
      setError((err as Error).message);
    }
  }

  async function removeDoc(id: string) {
    try {
      await api.deleteDocument(id);
      load();
    } catch (err) {
      setError((err as Error).message);
    }
  }

  return (
    <div className="space-y-6">
      <div>
        <h1 className="font-display text-3xl font-extrabold tracking-tight">
          Integrations
        </h1>
        <p className="text-ink-3">
          Credentials for external services, stored documents, and the outbound
          notification log.
        </p>
      </div>

      {error && <p className="text-bad">{error}</p>}

      {manageIntegrations && (
        <Card className="p-5">
          <h2 className="mb-1 font-display text-lg font-bold">Credentials</h2>
          <p className="mb-3 text-sm text-ink-3">
            Values are encrypted at rest and never shown again — only the last
            four characters. Saving an existing key rotates it. Webhook signing
            secrets use the key <code>webhook.&lt;provider&gt;.secret</code>.
          </p>
          <form
            onSubmit={saveSecret}
            className="mb-4 flex flex-wrap items-end gap-3"
          >
            <label className="text-sm">
              <span className="mb-1 block text-ink-3">Key</span>
              <input
                value={secretKey}
                onChange={(e) => setSecretKey(e.target.value)}
                placeholder="stripe.api_key"
                className="w-56 rounded-lg border border-line bg-surface px-3 py-2 font-mono text-xs"
              />
            </label>
            <label className="flex-1 min-w-[220px] text-sm">
              <span className="mb-1 block text-ink-3">Value</span>
              <input
                value={secretValue}
                onChange={(e) => setSecretValue(e.target.value)}
                type="password"
                placeholder="sk_live_…"
                className="w-full rounded-lg border border-line bg-surface px-3 py-2 font-mono text-xs"
              />
            </label>
            <button
              type="submit"
              disabled={savingSecret}
              className="rounded-lg bg-accent px-4 py-2 font-semibold text-white disabled:opacity-50"
            >
              Save credential
            </button>
          </form>
          <div className="space-y-2">
            {secrets?.map((s) => (
              <div
                key={s.id}
                className="flex flex-wrap items-center gap-3 rounded-lg border border-line px-4 py-2"
              >
                <span className="flex-1 font-mono text-sm">{s.key}</span>
                <span className="font-mono text-sm text-ink-3">
                  ••••{s.last4}
                </span>
                {s.rotated_at && <Badge tone="info">rotated</Badge>}
                <button
                  onClick={() => removeSecret(s.key)}
                  className="text-sm text-ink-3"
                >
                  Remove
                </button>
              </div>
            ))}
            {secrets?.length === 0 && (
              <p className="text-sm text-ink-3">No credentials stored yet.</p>
            )}
          </div>
        </Card>
      )}

      {readDocs && (
        <Card className="p-5">
          <h2 className="mb-1 font-display text-lg font-bold">Documents</h2>
          <p className="mb-3 text-sm text-ink-3">
            Files attach to a record (property, lease, application …) and are
            fetched via short-lived signed URLs. Re-uploading the same filename
            creates a new version.
          </p>
          {manageDocs && (
            <div className="mb-4 flex flex-wrap items-end gap-3">
              <label className="text-sm">
                <span className="mb-1 block text-ink-3">Attach to</span>
                <select
                  value={ownerType}
                  onChange={(e) => setOwnerType(e.target.value)}
                  className="rounded-lg border border-line bg-surface px-3 py-2"
                >
                  {OWNER_TYPES.map((t) => (
                    <option key={t} value={t}>
                      {t.replace("_", " ")}
                    </option>
                  ))}
                </select>
              </label>
              <label className="flex-1 min-w-[220px] text-sm">
                <span className="mb-1 block text-ink-3">Record id</span>
                <input
                  value={ownerId}
                  onChange={(e) => setOwnerId(e.target.value)}
                  placeholder="record UUID"
                  className="w-full rounded-lg border border-line bg-surface px-3 py-2 font-mono text-xs"
                />
              </label>
              <label className="rounded-lg bg-accent px-4 py-2 font-semibold text-white">
                {uploading ? "Uploading…" : "Upload file"}
                <input
                  ref={fileInput}
                  type="file"
                  className="hidden"
                  disabled={uploading}
                  onChange={(e) => {
                    const f = e.target.files?.[0];
                    if (f) uploadFile(f);
                  }}
                />
              </label>
            </div>
          )}
          <div className="space-y-2">
            {docs?.map((d) => (
              <div
                key={d.id}
                className="flex flex-wrap items-center gap-3 rounded-lg border border-line px-4 py-2"
              >
                <div className="flex-1">
                  <span className="font-semibold">{d.filename}</span>
                  <span className="ml-2 text-xs text-ink-3">
                    {d.owner_type} · v{d.version} · {formatBytes(d.size_bytes)}
                  </span>
                </div>
                <Badge tone={statusToneFor(d.status)}>{d.status}</Badge>
                <button
                  onClick={() => downloadDoc(d.id)}
                  className="text-sm font-semibold text-accent"
                >
                  Download
                </button>
                {manageDocs && (
                  <button
                    onClick={() => removeDoc(d.id)}
                    className="text-sm text-ink-3"
                  >
                    Delete
                  </button>
                )}
              </div>
            ))}
            {docs?.length === 0 && (
              <p className="text-sm text-ink-3">No documents uploaded yet.</p>
            )}
          </div>
        </Card>
      )}

      {manageIntegrations && (
        <Card className="p-5">
          <h2 className="mb-1 font-display text-lg font-bold">
            Notification log
          </h2>
          <p className="mb-3 text-sm text-ink-3">
            Outbound email and SMS sent by the platform (welcome emails,
            reminders), with delivery status.
          </p>
          <div className="space-y-2">
            {log?.map((n) => (
              <div
                key={n.id}
                className="flex flex-wrap items-center gap-3 rounded-lg border border-line px-4 py-2"
              >
                <Badge tone="neutral">{n.channel}</Badge>
                <div className="flex-1">
                  <span className="font-semibold">
                    {n.subject ?? n.template_key}
                  </span>
                  <span className="ml-2 text-xs text-ink-3">
                    to {n.recipient} · {new Date(n.created_at).toLocaleString()}
                  </span>
                  {n.last_error && (
                    <p className="text-xs text-bad">{n.last_error}</p>
                  )}
                </div>
                <Badge tone={statusToneFor(n.status)}>{n.status}</Badge>
              </div>
            ))}
            {log?.length === 0 && (
              <p className="text-sm text-ink-3">Nothing sent yet.</p>
            )}
          </div>
        </Card>
      )}
    </div>
  );
}
