"use client";

// Workspace document-storage settings. Choose the platform-managed default or
// bring your own bucket (Local / S3 / GCS). Gated by `storage:manage`.

import { useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api";
import type { StorageConfig } from "@/lib/types";
import { useAuth } from "@/lib/auth";
import { Badge, Button, Card } from "@/components/ui";

const FIELD =
  "w-full rounded-xl border border-line bg-surface px-3 py-2 text-sm text-ink";

const PROVIDERS: { value: string; label: string; hint: string }[] = [
  { value: "platform", label: "Platform-managed", hint: "Acre stores your documents — nothing to configure." },
  { value: "local", label: "Local filesystem", hint: "Store on the server's disk (single-node / dev)." },
  { value: "s3", label: "Amazon S3 (or compatible)", hint: "Your own S3 / MinIO / Cloudflare R2 bucket." },
  { value: "gcs", label: "Google Cloud Storage", hint: "Your own GCS bucket." },
];

export default function StorageSettingsPage() {
  const { can } = useAuth();
  const [cfg, setCfg] = useState<StorageConfig | null>(null);
  const [form, setForm] = useState({
    provider: "platform",
    bucket: "",
    region: "",
    prefix: "",
    endpoint: "",
    secret: "",
  });
  const [saving, setSaving] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!can("storage:manage")) return;
    api
      .storageConfig()
      .then((c) => {
        setCfg(c);
        setForm((f) => ({
          ...f,
          provider: c.provider,
          bucket: c.bucket ?? "",
          region: c.region ?? "",
          prefix: c.prefix ?? "",
          endpoint: c.endpoint ?? "",
        }));
      })
      .catch((e) => setError(e.message));
  }, [can]);

  if (!can("storage:manage")) {
    return (
      <Card className="p-6">
        <p className="text-ink-2">
          You need the <span className="font-mono">storage:manage</span>{" "}
          permission to configure storage.
        </p>
      </Card>
    );
  }

  const save = async () => {
    setSaving(true);
    setMsg(null);
    setError(null);
    try {
      const saved = await api.putStorageConfig({
        provider: form.provider,
        bucket: form.bucket || undefined,
        region: form.region || undefined,
        prefix: form.prefix || undefined,
        endpoint: form.endpoint || undefined,
        secret: form.secret || undefined,
      });
      setCfg(saved);
      setForm((f) => ({ ...f, secret: "" }));
      setMsg("Storage settings saved.");
    } catch (e) {
      setError(e instanceof ApiError ? e.message : "Save failed");
    } finally {
      setSaving(false);
    }
  };

  const needsBucket = form.provider === "s3" || form.provider === "gcs";
  const isS3 = form.provider === "s3";
  const isGcs = form.provider === "gcs";

  return (
    <div className="max-w-2xl space-y-6">
      <div>
        <h1 className="font-display text-3xl font-extrabold tracking-tight">
          Document storage
        </h1>
        <p className="text-ink-3">
          Where uploaded logos, LLC documents, and generated PDFs are stored.
        </p>
      </div>

      {cfg && (
        <Card className="flex items-center gap-3 p-4">
          <span className="text-sm text-ink-3">Current backend:</span>
          <Badge tone="info">{cfg.provider}</Badge>
          {cfg.is_default && <Badge tone="neutral">default</Badge>}
          {cfg.has_credentials && <Badge tone="good">credentials set</Badge>}
        </Card>
      )}

      <Card className="space-y-4 p-6">
        <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
          Provider
          <select
            className={FIELD}
            value={form.provider}
            onChange={(e) => setForm({ ...form, provider: e.target.value })}
          >
            {PROVIDERS.map((p) => (
              <option key={p.value} value={p.value}>
                {p.label}
              </option>
            ))}
          </select>
          <span className="text-[11px] font-normal text-ink-3">
            {PROVIDERS.find((p) => p.value === form.provider)?.hint}
          </span>
        </label>

        {form.provider === "local" && (
          <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
            Base directory
            <input
              className={FIELD}
              value={form.prefix}
              placeholder="/var/acre/storage"
              onChange={(e) => setForm({ ...form, prefix: e.target.value })}
            />
          </label>
        )}

        {needsBucket && (
          <>
            <div className="grid grid-cols-2 gap-4">
              <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
                Bucket
                <input
                  className={FIELD}
                  value={form.bucket}
                  onChange={(e) => setForm({ ...form, bucket: e.target.value })}
                />
              </label>
              <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
                Key prefix (optional)
                <input
                  className={FIELD}
                  value={form.prefix}
                  onChange={(e) => setForm({ ...form, prefix: e.target.value })}
                />
              </label>
            </div>
            {isS3 && (
              <div className="grid grid-cols-2 gap-4">
                <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
                  Region
                  <input
                    className={FIELD}
                    value={form.region}
                    placeholder="us-east-1"
                    onChange={(e) =>
                      setForm({ ...form, region: e.target.value })
                    }
                  />
                </label>
                <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
                  Endpoint (MinIO / R2, optional)
                  <input
                    className={FIELD}
                    value={form.endpoint}
                    onChange={(e) =>
                      setForm({ ...form, endpoint: e.target.value })
                    }
                  />
                </label>
              </div>
            )}
            <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
              {isGcs
                ? "Service-account key JSON"
                : "Credentials JSON — {\"access_key_id\":\"…\",\"secret_access_key\":\"…\"}"}
              <textarea
                className={`${FIELD} font-mono`}
                rows={isGcs ? 6 : 3}
                value={form.secret}
                placeholder={
                  cfg?.has_credentials
                    ? "•••••• (leave blank to keep existing)"
                    : ""
                }
                onChange={(e) => setForm({ ...form, secret: e.target.value })}
              />
              <span className="text-[11px] font-normal text-ink-3">
                Encrypted at rest (AES-256-GCM) and never shown again.
              </span>
            </label>
          </>
        )}

        <div className="flex items-center gap-3">
          <Button onClick={save} disabled={saving}>
            {saving ? "Saving…" : "Save settings"}
          </Button>
          {msg && <span className="text-sm text-good">{msg}</span>}
          {error && <span className="text-sm text-bad">{error}</span>}
        </div>
      </Card>
    </div>
  );
}
