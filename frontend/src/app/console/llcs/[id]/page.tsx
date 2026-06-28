"use client";

// LLC onboarding hub: the profile, uploaded documents, branding/signature,
// reusable templates, and lease/letter generation for a single holding entity.
// Gated by `llc:read`; write actions require `llc:manage`.

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useParams } from "next/navigation";
import Link from "next/link";
import { api, ApiError } from "@/lib/api";
import type {
  GeneratedDocument,
  Llc,
  LlcBranding,
  LlcDocument,
  LlcTemplate,
} from "@/lib/types";
import { useAuth } from "@/lib/auth";
import { Badge, Button, Card, statusTone } from "@/components/ui";
import { Icon } from "@/components/Icon";

const FIELD =
  "w-full rounded-xl border border-line bg-surface px-3 py-2 text-sm text-ink";

const DOC_KINDS = [
  "logo",
  "articles_of_organization",
  "operating_agreement",
  "ein_letter",
  "w9",
  "business_license",
  "insurance",
  "other",
];

const TABS = ["profile", "documents", "branding", "templates", "generate"] as const;
type Tab = (typeof TABS)[number];

function humanize(s: string): string {
  const t = s.replace(/_/g, " ");
  return t.charAt(0).toUpperCase() + t.slice(1);
}

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(0)} KB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}

/** Trigger a browser download of a fetched Blob. */
function saveBlob(blob: Blob, filename: string) {
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  a.remove();
  URL.revokeObjectURL(url);
}

export default function LlcOnboardingPage() {
  const params = useParams<{ id: string }>();
  const id = params.id;
  const { can } = useAuth();
  const canManage = can("llc:manage");

  const [llc, setLlc] = useState<Llc | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [tab, setTab] = useState<Tab>("profile");

  const loadLlc = useCallback(() => {
    api
      .llc(id)
      .then(setLlc)
      .catch((e) => setError(e.message));
  }, [id]);

  useEffect(() => {
    if (!can("llc:read")) return;
    loadLlc();
  }, [loadLlc, can]);

  if (!can("llc:read")) {
    return (
      <Card className="p-6">
        <p className="text-ink-2">
          You don&apos;t have access to LLC onboarding. Ask an admin for the{" "}
          <span className="font-mono">llc:read</span> permission.
        </p>
      </Card>
    );
  }

  return (
    <div className="space-y-6">
      <div>
        <Link
          href="/console/llcs"
          className="mb-2 inline-flex items-center gap-1 text-sm text-ink-3 hover:text-ink"
        >
          <Icon name="back" size={16} /> All LLCs
        </Link>
        <div className="flex flex-wrap items-center gap-3">
          <h1 className="font-display text-3xl font-extrabold tracking-tight">
            {llc?.name ?? "LLC"}
          </h1>
          {llc && (
            <Badge tone={statusTone(llc.status)}>
              {llc.onboarded ? "active" : llc.status}
            </Badge>
          )}
        </div>
        {llc && (
          <p className="text-ink-3">
            {llc.entity_type} · EIN {llc.ein || "—"} · {llc.state || "—"}
          </p>
        )}
      </div>

      {error && <p className="text-bad">{error}</p>}

      <div className="flex flex-wrap gap-1 border-b border-line">
        {TABS.map((t) => (
          <button
            key={t}
            onClick={() => setTab(t)}
            className={`-mb-px border-b-2 px-4 py-2 text-sm font-bold transition ${
              tab === t
                ? "border-accent text-ink"
                : "border-transparent text-ink-3 hover:text-ink"
            }`}
          >
            {humanize(t)}
          </button>
        ))}
      </div>

      {tab === "profile" && llc && (
        <ProfileTab llc={llc} canManage={canManage} onSaved={setLlc} />
      )}
      {tab === "documents" && <DocumentsTab id={id} canManage={canManage} />}
      {tab === "branding" && <BrandingTab id={id} canManage={canManage} />}
      {tab === "templates" && <TemplatesTab id={id} canManage={canManage} />}
      {tab === "generate" && <GenerateTab id={id} canManage={canManage} />}
    </div>
  );
}

// ---- Profile -----------------------------------------------------------------

function ProfileTab({
  llc,
  canManage,
  onSaved,
}: {
  llc: Llc;
  canManage: boolean;
  onSaved: (l: Llc) => void;
}) {
  const [form, setForm] = useState<Llc>(llc);
  const [saving, setSaving] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);

  const set = (k: keyof Llc, v: string) => setForm({ ...form, [k]: v });

  const save = async (markActive: boolean) => {
    setSaving(true);
    setMsg(null);
    try {
      const saved = await api.updateLlc(llc.id, {
        name: form.name,
        ein: form.ein,
        state: form.state,
        entity_type: form.entity_type,
        formation_date: form.formation_date ?? undefined,
        registered_agent: form.registered_agent ?? undefined,
        principal_address: form.principal_address ?? undefined,
        mailing_address: form.mailing_address ?? undefined,
        contact_name: form.contact_name ?? undefined,
        contact_email: form.contact_email ?? undefined,
        contact_phone: form.contact_phone ?? undefined,
        website: form.website ?? undefined,
        status: markActive ? "active" : form.status,
      });
      onSaved(saved);
      setForm(saved);
      setMsg("Saved.");
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : "Save failed");
    } finally {
      setSaving(false);
    }
  };

  const F = (label: string, k: keyof Llc, placeholder = "") => (
    <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
      {label}
      <input
        className={FIELD}
        value={(form[k] as string) ?? ""}
        placeholder={placeholder}
        disabled={!canManage}
        onChange={(e) => set(k, e.target.value)}
      />
    </label>
  );

  return (
    <Card className="space-y-5 p-6">
      <div className="grid gap-4 sm:grid-cols-2">
        {F("Legal name", "name")}
        <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
          Entity type
          <select
            className={FIELD}
            value={form.entity_type}
            disabled={!canManage}
            onChange={(e) => set("entity_type", e.target.value)}
          >
            {["LLC", "C-Corp", "S-Corp", "LP", "Sole Proprietorship", "Trust"].map(
              (t) => (
                <option key={t} value={t}>
                  {t}
                </option>
              )
            )}
          </select>
        </label>
        {F("EIN / Tax ID", "ein", "12-3456789")}
        {F("State of registration", "state", "OR")}
        {F("Formation date", "formation_date", "YYYY-MM-DD")}
        {F("Registered agent", "registered_agent")}
        {F("Principal address", "principal_address")}
        {F("Mailing address", "mailing_address")}
        {F("Contact name", "contact_name")}
        {F("Contact email", "contact_email")}
        {F("Contact phone", "contact_phone")}
        {F("Website", "website")}
      </div>

      {canManage && (
        <div className="flex flex-wrap items-center gap-3">
          <Button onClick={() => save(false)} disabled={saving}>
            {saving ? "Saving…" : "Save profile"}
          </Button>
          {!llc.onboarded && (
            <Button variant="outline" onClick={() => save(true)} disabled={saving}>
              Mark onboarding complete
            </Button>
          )}
          {msg && <span className="text-sm text-ink-3">{msg}</span>}
        </div>
      )}
    </Card>
  );
}

// ---- Documents ---------------------------------------------------------------

function DocumentsTab({ id, canManage }: { id: string; canManage: boolean }) {
  const [docs, setDocs] = useState<LlcDocument[]>([]);
  const [kind, setKind] = useState("articles_of_organization");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const fileRef = useRef<HTMLInputElement>(null);

  const load = useCallback(() => {
    api
      .llcDocuments(id)
      .then(setDocs)
      .catch((e) => setError(e.message));
  }, [id]);
  useEffect(load, [load]);

  const onUpload = async (file: File) => {
    setBusy(true);
    setError(null);
    try {
      await api.uploadLlcDocument(id, file, kind, file.name);
      if (fileRef.current) fileRef.current.value = "";
      load();
    } catch (e) {
      setError(e instanceof ApiError ? e.message : "Upload failed");
    } finally {
      setBusy(false);
    }
  };

  const onDownload = async (d: LlcDocument) => {
    try {
      const blob = await api.downloadLlcDocument(id, d.id);
      saveBlob(blob, d.original_filename);
    } catch (e) {
      setError(e instanceof ApiError ? e.message : "Download failed");
    }
  };

  const onDelete = async (d: LlcDocument) => {
    if (!confirm(`Delete ${d.original_filename}?`)) return;
    try {
      await api.deleteLlcDocument(id, d.id);
      load();
    } catch (e) {
      setError(e instanceof ApiError ? e.message : "Delete failed");
    }
  };

  return (
    <div className="space-y-4">
      {canManage && (
        <Card className="flex flex-wrap items-end gap-3 p-5">
          <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
            Document type
            <select
              className={FIELD}
              value={kind}
              onChange={(e) => setKind(e.target.value)}
            >
              {DOC_KINDS.map((k) => (
                <option key={k} value={k}>
                  {humanize(k)}
                </option>
              ))}
            </select>
          </label>
          <label className="inline-flex cursor-pointer items-center gap-2 rounded-xl border border-line-2 bg-surface px-4 py-2.5 text-sm font-bold text-ink hover:bg-surface-2">
            <Icon name="upload" size={16} />
            {busy ? "Uploading…" : "Upload file"}
            <input
              ref={fileRef}
              type="file"
              className="hidden"
              disabled={busy}
              onChange={(e) => {
                const f = e.target.files?.[0];
                if (f) onUpload(f);
              }}
            />
          </label>
          <span className="text-xs text-ink-3">PDF, PNG, JPG up to 25 MB.</span>
        </Card>
      )}

      {error && <p className="text-bad">{error}</p>}

      <Card className="overflow-hidden">
        {docs.length === 0 ? (
          <div className="px-5 py-10 text-center text-ink-3">
            No documents uploaded yet.
          </div>
        ) : (
          <div className="divide-y divide-line">
            {docs.map((d) => (
              <div key={d.id} className="flex items-center gap-3 px-5 py-3.5">
                <Icon name="file" size={18} className="text-ink-3" />
                <div className="min-w-0 flex-1">
                  <div className="truncate font-semibold">
                    {d.title || d.original_filename}
                  </div>
                  <div className="text-xs text-ink-3">
                    {humanize(d.kind)} · {formatBytes(d.size_bytes)} ·{" "}
                    {d.storage_provider}
                  </div>
                </div>
                {d.verified && <Badge tone="good">verified</Badge>}
                <Button variant="ghost" onClick={() => onDownload(d)}>
                  Download
                </Button>
                {canManage && (
                  <Button variant="ghost" onClick={() => onDelete(d)}>
                    Delete
                  </Button>
                )}
              </div>
            ))}
          </div>
        )}
      </Card>
    </div>
  );
}

// ---- Branding ----------------------------------------------------------------

function BrandingTab({ id, canManage }: { id: string; canManage: boolean }) {
  const [branding, setBranding] = useState<LlcBranding | null>(null);
  const [logos, setLogos] = useState<LlcDocument[]>([]);
  const [saving, setSaving] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);

  useEffect(() => {
    api.llcBranding(id).then(setBranding).catch(() => {});
    api
      .llcDocuments(id)
      .then((ds) => setLogos(ds.filter((d) => d.kind === "logo")))
      .catch(() => {});
  }, [id]);

  if (!branding) return <Card className="p-6 text-ink-3">Loading…</Card>;

  const set = (k: keyof LlcBranding, v: string) =>
    setBranding({ ...branding, [k]: v === "" ? null : v });

  const save = async () => {
    setSaving(true);
    setMsg(null);
    try {
      const { llc_id: _llc, ...rest } = branding;
      void _llc;
      const saved = await api.putLlcBranding(id, rest);
      setBranding(saved);
      setMsg("Saved.");
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : "Save failed");
    } finally {
      setSaving(false);
    }
  };

  const text = (label: string, k: keyof LlcBranding, placeholder = "") => (
    <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
      {label}
      <input
        className={FIELD}
        value={(branding[k] as string) ?? ""}
        placeholder={placeholder}
        disabled={!canManage}
        onChange={(e) => set(k, e.target.value)}
      />
    </label>
  );
  const area = (label: string, k: keyof LlcBranding, rows = 3) => (
    <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
      {label}
      <textarea
        className={`${FIELD} font-mono`}
        rows={rows}
        value={(branding[k] as string) ?? ""}
        disabled={!canManage}
        onChange={(e) => set(k, e.target.value)}
      />
    </label>
  );

  return (
    <Card className="space-y-5 p-6">
      <div className="grid gap-4 sm:grid-cols-2">
        <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
          Logo
          <select
            className={FIELD}
            value={branding.logo_document_id ?? ""}
            disabled={!canManage}
            onChange={(e) => set("logo_document_id", e.target.value)}
          >
            <option value="">— none —</option>
            {logos.map((l) => (
              <option key={l.id} value={l.id}>
                {l.title || l.original_filename}
              </option>
            ))}
          </select>
          {logos.length === 0 && (
            <span className="text-[11px] font-normal text-ink-3">
              Upload a document of type “logo” to choose one here.
            </span>
          )}
        </label>
        <div className="grid grid-cols-2 gap-4">
          {text("Primary color", "primary_color", "#F5451F")}
          {text("Accent color", "accent_color", "#1C7C53")}
        </div>
        {text("Signature name", "signature_name", "Jane Doe")}
        {text("Signature title", "signature_title", "Managing Member")}
      </div>
      {area("Signature block", "signature_block", 4)}
      {area("Letterhead (top of documents)", "letterhead", 2)}
      {area("Footer / disclaimer", "footer", 2)}

      {canManage && (
        <div className="flex items-center gap-3">
          <Button onClick={save} disabled={saving}>
            {saving ? "Saving…" : "Save branding"}
          </Button>
          {msg && <span className="text-sm text-ink-3">{msg}</span>}
        </div>
      )}
    </Card>
  );
}

// ---- Templates ---------------------------------------------------------------

const TEMPLATE_KINDS = ["lease", "tenant_letter", "welcome_email", "notice", "other"];

function TemplatesTab({ id, canManage }: { id: string; canManage: boolean }) {
  const [templates, setTemplates] = useState<LlcTemplate[]>([]);
  const [editing, setEditing] = useState<Partial<LlcTemplate> | null>(null);
  const [preview, setPreview] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    api.llcTemplates(id).then(setTemplates).catch((e) => setError(e.message));
  }, [id]);
  useEffect(load, [load]);

  const blank = (): Partial<LlcTemplate> => ({
    kind: "lease",
    name: "",
    subject: "",
    body: "",
    is_default: false,
  });

  const save = async () => {
    if (!editing) return;
    setError(null);
    try {
      if (editing.id) {
        await api.updateLlcTemplate(id, editing.id, {
          kind: editing.kind,
          name: editing.name,
          subject: editing.subject ?? undefined,
          body: editing.body,
          is_default: editing.is_default,
        });
      } else {
        await api.createLlcTemplate(id, {
          kind: editing.kind ?? "other",
          name: editing.name ?? "Untitled",
          subject: editing.subject ?? undefined,
          body: editing.body ?? "",
          is_default: editing.is_default,
        });
      }
      setEditing(null);
      setPreview(null);
      load();
    } catch (e) {
      setError(e instanceof ApiError ? e.message : "Save failed");
    }
  };

  const runPreview = async () => {
    if (!editing?.body) return;
    try {
      const r = await api.previewTemplate(id, {
        body: editing.body,
        context: {
          tenant_name: "Sam Tenant",
          property_address: "123 Main St, Portland OR",
          rent: "$1,950.00",
          deposit: "$1,950.00",
          start_date: "2026-07-01",
          end_date: "2027-06-30",
          unit: "2B",
        },
      });
      setPreview(r.rendered);
    } catch (e) {
      setError(e instanceof ApiError ? e.message : "Preview failed");
    }
  };

  const remove = async (t: LlcTemplate) => {
    if (!confirm(`Delete template “${t.name}”?`)) return;
    await api.deleteLlcTemplate(id, t.id).catch(() => {});
    load();
  };

  return (
    <div className="grid gap-4 lg:grid-cols-2">
      <div className="space-y-3">
        {canManage && (
          <Button variant="outline" onClick={() => setEditing(blank())}>
            + New template
          </Button>
        )}
        {error && <p className="text-bad">{error}</p>}
        {templates.length === 0 && (
          <Card className="p-6 text-ink-3">No templates yet.</Card>
        )}
        {templates.map((t) => (
          <Card key={t.id} className="flex items-center gap-3 p-4">
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <span className="truncate font-semibold">{t.name}</span>
                {t.is_default && <Badge tone="accent">default</Badge>}
              </div>
              <div className="text-xs text-ink-3">{humanize(t.kind)}</div>
            </div>
            {canManage && (
              <>
                <Button
                  variant="ghost"
                  onClick={() => {
                    setEditing(t);
                    setPreview(null);
                  }}
                >
                  Edit
                </Button>
                <Button variant="ghost" onClick={() => remove(t)}>
                  Delete
                </Button>
              </>
            )}
          </Card>
        ))}
      </div>

      {editing && (
        <Card className="space-y-3 p-5">
          <div className="grid grid-cols-2 gap-3">
            <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
              Kind
              <select
                className={FIELD}
                value={editing.kind}
                onChange={(e) => setEditing({ ...editing, kind: e.target.value })}
              >
                {TEMPLATE_KINDS.map((k) => (
                  <option key={k} value={k}>
                    {humanize(k)}
                  </option>
                ))}
              </select>
            </label>
            <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
              Name
              <input
                className={FIELD}
                value={editing.name ?? ""}
                onChange={(e) => setEditing({ ...editing, name: e.target.value })}
              />
            </label>
          </div>
          <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
            Email subject (optional)
            <input
              className={FIELD}
              value={editing.subject ?? ""}
              onChange={(e) =>
                setEditing({ ...editing, subject: e.target.value })
              }
            />
          </label>
          <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
            Body — use {"{{tenant_name}}"}, {"{{rent}}"}, {"{{llc_name}}"},{" "}
            {"{{property_address}}"}…
            <textarea
              className={`${FIELD} font-mono`}
              rows={14}
              value={editing.body ?? ""}
              onChange={(e) => setEditing({ ...editing, body: e.target.value })}
            />
          </label>
          <label className="flex items-center gap-2 text-sm text-ink-2">
            <input
              type="checkbox"
              checked={!!editing.is_default}
              onChange={(e) =>
                setEditing({ ...editing, is_default: e.target.checked })
              }
            />
            Default for its kind
          </label>
          <div className="flex flex-wrap gap-2">
            <Button onClick={save}>Save template</Button>
            <Button variant="outline" onClick={runPreview}>
              Preview
            </Button>
            <Button
              variant="ghost"
              onClick={() => {
                setEditing(null);
                setPreview(null);
              }}
            >
              Cancel
            </Button>
          </div>
          {preview !== null && (
            <pre className="max-h-72 overflow-auto whitespace-pre-wrap rounded-xl border border-line bg-surface-2 p-3 text-xs text-ink-2">
              {preview}
            </pre>
          )}
        </Card>
      )}
    </div>
  );
}

// ---- Generate ----------------------------------------------------------------

function GenerateTab({ id, canManage }: { id: string; canManage: boolean }) {
  const [templates, setTemplates] = useState<LlcTemplate[]>([]);
  const [generated, setGenerated] = useState<GeneratedDocument[]>([]);
  const [form, setForm] = useState({
    template_id: "",
    kind: "letter",
    title: "",
    recipient_name: "",
    recipient_email: "",
    property_address: "",
    lease_id: "",
    send_email: false,
  });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const loadGenerated = useCallback(() => {
    api.generatedDocuments(id).then(setGenerated).catch(() => {});
  }, [id]);
  useEffect(() => {
    api.llcTemplates(id).then(setTemplates).catch(() => {});
    loadGenerated();
  }, [id, loadGenerated]);

  const selectableTemplates = useMemo(
    () => templates.filter((t) => t.kind === form.kind || form.kind === "letter"),
    [templates, form.kind]
  );

  const generate = async () => {
    setBusy(true);
    setError(null);
    try {
      await api.generateDocument(id, {
        template_id: form.template_id || undefined,
        kind: form.kind,
        title: form.title || undefined,
        recipient_name: form.recipient_name || undefined,
        recipient_email: form.recipient_email || undefined,
        property_address: form.property_address || undefined,
        lease_id: form.lease_id || undefined,
        send_email: form.send_email,
      });
      loadGenerated();
    } catch (e) {
      setError(e instanceof ApiError ? e.message : "Generation failed");
    } finally {
      setBusy(false);
    }
  };

  const download = async (g: GeneratedDocument) => {
    try {
      const blob = await api.downloadGenerated(g.id);
      saveBlob(blob, `${g.title.replace(/[^\w.-]+/g, "_")}.pdf`);
    } catch (e) {
      setError(e instanceof ApiError ? e.message : "Download failed");
    }
  };

  return (
    <div className="grid gap-4 lg:grid-cols-2">
      {canManage && (
        <Card className="space-y-3 p-5">
          <div className="grid grid-cols-2 gap-3">
            <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
              Document type
              <select
                className={FIELD}
                value={form.kind}
                onChange={(e) =>
                  setForm({ ...form, kind: e.target.value, template_id: "" })
                }
              >
                <option value="lease">Lease contract</option>
                <option value="letter">Tenant letter</option>
              </select>
            </label>
            <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
              Template
              <select
                className={FIELD}
                value={form.template_id}
                onChange={(e) =>
                  setForm({ ...form, template_id: e.target.value })
                }
              >
                <option value="">Built-in default</option>
                {selectableTemplates.map((t) => (
                  <option key={t.id} value={t.id}>
                    {t.name}
                  </option>
                ))}
              </select>
            </label>
          </div>
          <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
            Title (optional)
            <input
              className={FIELD}
              value={form.title}
              onChange={(e) => setForm({ ...form, title: e.target.value })}
            />
          </label>
          <div className="grid grid-cols-2 gap-3">
            <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
              Recipient name
              <input
                className={FIELD}
                value={form.recipient_name}
                onChange={(e) =>
                  setForm({ ...form, recipient_name: e.target.value })
                }
              />
            </label>
            <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
              Recipient email
              <input
                className={FIELD}
                value={form.recipient_email}
                onChange={(e) =>
                  setForm({ ...form, recipient_email: e.target.value })
                }
              />
            </label>
          </div>
          <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
            Property address
            <input
              className={FIELD}
              value={form.property_address}
              onChange={(e) =>
                setForm({ ...form, property_address: e.target.value })
              }
            />
          </label>
          <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
            Lease ID (optional — pulls in rent & dates)
            <input
              className={`${FIELD} font-mono`}
              value={form.lease_id}
              placeholder="uuid"
              onChange={(e) => setForm({ ...form, lease_id: e.target.value })}
            />
          </label>
          <label className="flex items-center gap-2 text-sm text-ink-2">
            <input
              type="checkbox"
              checked={form.send_email}
              onChange={(e) =>
                setForm({ ...form, send_email: e.target.checked })
              }
            />
            Email it to the recipient
          </label>
          <Button onClick={generate} disabled={busy}>
            {busy ? "Generating…" : "Generate document"}
          </Button>
          {error && <p className="text-bad">{error}</p>}
        </Card>
      )}

      <Card className="overflow-hidden">
        <div className="border-b border-line px-5 py-3 text-xs font-bold uppercase tracking-wide text-ink-3">
          Generated documents
        </div>
        {generated.length === 0 ? (
          <div className="px-5 py-10 text-center text-ink-3">
            Nothing generated yet.
          </div>
        ) : (
          <div className="divide-y divide-line">
            {generated.map((g) => (
              <div key={g.id} className="flex items-center gap-3 px-5 py-3.5">
                <Icon name="file" size={18} className="text-ink-3" />
                <div className="min-w-0 flex-1">
                  <div className="truncate font-semibold">{g.title}</div>
                  <div className="text-xs text-ink-3">
                    {humanize(g.kind)} · {formatBytes(g.size_bytes)}
                  </div>
                </div>
                <Badge tone={statusTone(g.status)}>{g.status}</Badge>
                <Button variant="ghost" onClick={() => download(g)}>
                  Download
                </Button>
              </div>
            ))}
          </div>
        )}
      </Card>
    </div>
  );
}
