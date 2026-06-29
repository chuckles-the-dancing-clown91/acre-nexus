"use client";

// LLC onboarding hub — the rich, tabbed profile for a single holding entity:
// editable details, uploaded documents, reusable templates (with live preview),
// lease/letter generation, and branding/signature blocks. Reads come from the
// typed hooks in queries.ts; mutating actions are gated by permission and
// confirmed via toasts. Gated overall by `llc:read`; writes need `llc:manage`
// (branding additionally honours `theme:write`).

import * as React from "react";
import { useMemo, useRef, useState } from "react";
import Link from "next/link";
import { useParams } from "next/navigation";
import { toast } from "sonner";
import {
  Building2,
  Download,
  FileText,
  Palette,
  Plus,
  Sparkles,
  Trash2,
  Upload,
  Wand2,
} from "lucide-react";

import { api, ApiError } from "@/lib/api";
import {
  useGeneratedDocuments,
  useGenerateDocument,
  useLlc,
  useLlcBranding,
  useLlcDocuments,
  useLlcTemplates,
  usePutLlcBranding,
  useUpdateLlc,
  useUploadLlcDocument,
  useDeleteLlcDocument,
  useCreateLlcTemplate,
  useUpdateLlcTemplate,
  useDeleteLlcTemplate,
} from "@/lib/queries";
import type {
  GeneratedDocument,
  Llc,
  LlcBranding,
  LlcDocument,
  LlcTemplate,
} from "@/lib/types";
import { useAuth } from "@/lib/auth";
import { Badge, statusTone } from "@/components/ui";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Breadcrumbs } from "@/components/ui/breadcrumbs";
import { PageHeader, EmptyState } from "@/components/ui/page";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Field,
  Input,
  NativeSelect,
  SelectField,
  TextareaField,
  TextField,
} from "@/components/ui/form-field";
import { Switch } from "@/components/ui/switch";
import { Skeleton } from "@/components/ui/skeleton";
import { formatDate } from "@/lib/format";

// ----------------------------------------------------------------------------
// constants + helpers
// ----------------------------------------------------------------------------

const ENTITY_TYPES = [
  "LLC",
  "C-Corp",
  "S-Corp",
  "LP",
  "Sole Proprietorship",
  "Trust",
];

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

const TEMPLATE_KINDS = [
  "lease",
  "tenant_letter",
  "welcome_email",
  "notice",
  "other",
];

/** snake/lower → human label, e.g. `operating_agreement` → `Operating agreement`. */
function humanize(s: string | null | undefined): string {
  if (!s) return "—";
  const t = s.replace(/_/g, " ");
  return t.charAt(0).toUpperCase() + t.slice(1);
}

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(0)} KB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}

function errMessage(e: unknown, fallback: string): string {
  return e instanceof ApiError || e instanceof Error ? e.message : fallback;
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

// ----------------------------------------------------------------------------
// page
// ----------------------------------------------------------------------------

export default function LlcDetailPage() {
  const { id } = useParams<{ id: string }>();
  const { can } = useAuth();
  const canManage = can("llc:manage");
  const canBrand = can("llc:manage") || can("theme:write");

  const llc = useLlc(id);
  const l = llc.data;

  if (!can("llc:read")) {
    return (
      <div className="space-y-6">
        <Breadcrumbs items={[{ label: "LLCs", href: "/console/llcs" }, { label: "Restricted" }]} />
        <EmptyState
          icon={Building2}
          title="No access to LLC onboarding"
          description={
            <>
              Ask an admin for the <span className="font-mono">llc:read</span>{" "}
              permission to view holding entities.
            </>
          }
        />
      </div>
    );
  }

  if (llc.isLoading) {
    return (
      <div className="space-y-6">
        <Skeleton className="h-5 w-48 rounded-lg" />
        <Skeleton className="h-16 w-full rounded-xl" />
        <Skeleton className="h-9 w-80 rounded-lg" />
        <Skeleton className="h-64 w-full rounded-xl" />
      </div>
    );
  }

  if (llc.isError || !l) {
    return (
      <div className="space-y-6">
        <Breadcrumbs items={[{ label: "LLCs", href: "/console/llcs" }, { label: "Not found" }]} />
        <EmptyState
          icon={Building2}
          title="Couldn't load this LLC"
          description={
            llc.error instanceof Error
              ? llc.error.message
              : "It may have been removed, or you don't have access to it."
          }
          action={
            <Button asChild variant="outline">
              <Link href="/console/llcs">Back to LLCs</Link>
            </Button>
          }
        />
      </div>
    );
  }

  const subtitle = [
    l.entity_type,
    `EIN ${l.ein || "—"}`,
    l.state || "—",
  ].join(" · ");

  return (
    <div className="space-y-6">
      <Breadcrumbs items={[{ label: "LLCs", href: "/console/llcs" }, { label: l.name }]} />

      <PageHeader
        eyebrow="Holding entity"
        title={l.name}
        description={subtitle}
        actions={
          <Badge tone={l.onboarded ? "good" : statusTone(l.status)}>
            {l.onboarded ? "Onboarded" : l.status}
          </Badge>
        }
      />

      <Tabs defaultValue="overview" className="space-y-0">
        <TabsList className="w-full overflow-x-auto">
          <TabsTrigger value="overview">Overview</TabsTrigger>
          <TabsTrigger value="documents">Documents</TabsTrigger>
          <TabsTrigger value="templates">Templates</TabsTrigger>
          <TabsTrigger value="generate">Generate</TabsTrigger>
          <TabsTrigger value="branding">Branding</TabsTrigger>
        </TabsList>

        <TabsContent value="overview">
          <OverviewTab llc={l} canManage={canManage} />
        </TabsContent>
        <TabsContent value="documents">
          <DocumentsTab id={id} canManage={canManage} />
        </TabsContent>
        <TabsContent value="templates">
          <TemplatesTab id={id} canManage={canManage} />
        </TabsContent>
        <TabsContent value="generate">
          <GenerateTab id={id} canManage={canManage} />
        </TabsContent>
        <TabsContent value="branding">
          <BrandingTab id={id} canManage={canBrand} />
        </TabsContent>
      </Tabs>
    </div>
  );
}

// ----------------------------------------------------------------------------
// Overview tab — editable details form
// ----------------------------------------------------------------------------

type LlcFormKey =
  | "name"
  | "ein"
  | "state"
  | "formation_date"
  | "registered_agent"
  | "principal_address"
  | "mailing_address"
  | "contact_name"
  | "contact_email"
  | "contact_phone"
  | "website";

function OverviewTab({ llc, canManage }: { llc: Llc; canManage: boolean }) {
  const update = useUpdateLlc(llc.id);
  const [form, setForm] = useState<Llc>(llc);

  // Re-sync local form when the underlying record changes (e.g. after save).
  React.useEffect(() => setForm(llc), [llc]);

  const set = (k: keyof Llc, v: string) => setForm((f) => ({ ...f, [k]: v }));

  const submit = (markOnboarded: boolean) => {
    update.mutate(
      {
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
        status: markOnboarded ? "active" : form.status,
      },
      markOnboarded
        ? { onSuccess: () => toast.success("Onboarding marked complete") }
        : undefined
    );
  };

  const field = (label: string, k: LlcFormKey, placeholder?: string) => (
    <TextField
      label={label}
      value={(form[k] as string) ?? ""}
      placeholder={placeholder}
      disabled={!canManage}
      onChange={(e) => set(k, e.target.value)}
    />
  );

  return (
    <form
      className="space-y-6"
      onSubmit={(e) => {
        e.preventDefault();
        submit(false);
      }}
    >
      <Card>
        <CardHeader>
          <CardTitle>Entity details</CardTitle>
          {!canManage && (
            <CardDescription>Read-only — requires llc:manage.</CardDescription>
          )}
        </CardHeader>
        <CardContent className="grid gap-4 sm:grid-cols-2">
          {field("Legal name", "name")}
          <SelectField
            label="Entity type"
            value={form.entity_type}
            disabled={!canManage}
            onChange={(e) => set("entity_type", e.target.value)}
          >
            {ENTITY_TYPES.map((t) => (
              <option key={t} value={t}>
                {t}
              </option>
            ))}
          </SelectField>
          {field("EIN / Tax ID", "ein", "12-3456789")}
          {field("State of registration", "state", "OR")}
          {field("Formation date", "formation_date", "YYYY-MM-DD")}
          {field("Registered agent", "registered_agent")}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Addresses &amp; contact</CardTitle>
        </CardHeader>
        <CardContent className="grid gap-4 sm:grid-cols-2">
          {field("Principal address", "principal_address")}
          {field("Mailing address", "mailing_address")}
          {field("Contact name", "contact_name")}
          {field("Contact email", "contact_email")}
          {field("Contact phone", "contact_phone")}
          {field("Website", "website")}
        </CardContent>
      </Card>

      {canManage && (
        <div className="flex flex-wrap items-center gap-3">
          <Button type="submit" disabled={update.isPending}>
            {update.isPending ? "Saving…" : "Save profile"}
          </Button>
          {!llc.onboarded && (
            <Button
              type="button"
              variant="outline"
              disabled={update.isPending}
              onClick={() => submit(true)}
            >
              <Sparkles className="h-4 w-4" />
              Mark onboarding complete
            </Button>
          )}
        </div>
      )}
    </form>
  );
}

// ----------------------------------------------------------------------------
// Documents tab
// ----------------------------------------------------------------------------

function DocumentsTab({ id, canManage }: { id: string; canManage: boolean }) {
  const docs = useLlcDocuments(id);
  const upload = useUploadLlcDocument(id);
  const remove = useDeleteLlcDocument(id);
  const [kind, setKind] = useState("articles_of_organization");
  const fileRef = useRef<HTMLInputElement>(null);

  const onPick = (file: File) => {
    upload.mutate(
      { file, kind, title: file.name },
      {
        onSettled: () => {
          if (fileRef.current) fileRef.current.value = "";
        },
      }
    );
  };

  const download = async (d: LlcDocument) => {
    try {
      const blob = await api.downloadLlcDocument(id, d.id);
      saveBlob(blob, d.original_filename);
    } catch (e) {
      toast.error("Download failed", { description: errMessage(e, "") });
    }
  };

  const list = docs.data ?? [];

  return (
    <div className="space-y-6">
      {canManage && (
        <Card>
          <CardHeader>
            <CardTitle>Upload a document</CardTitle>
            <CardDescription>PDF, PNG, or JPG up to 25 MB.</CardDescription>
          </CardHeader>
          <CardContent className="flex flex-wrap items-end gap-4">
            <Field label="Document type" className="min-w-[14rem]">
              <NativeSelect
                value={kind}
                onChange={(e) => setKind(e.target.value)}
              >
                {DOC_KINDS.map((k) => (
                  <option key={k} value={k}>
                    {humanize(k)}
                  </option>
                ))}
              </NativeSelect>
            </Field>
            <Button
              type="button"
              variant="outline"
              disabled={upload.isPending}
              onClick={() => fileRef.current?.click()}
            >
              <Upload className="h-4 w-4" />
              {upload.isPending ? "Uploading…" : "Choose file"}
            </Button>
            <input
              ref={fileRef}
              type="file"
              className="hidden"
              disabled={upload.isPending}
              onChange={(e) => {
                const f = e.target.files?.[0];
                if (f) onPick(f);
              }}
            />
          </CardContent>
        </Card>
      )}

      <Card>
        <CardHeader>
          <CardTitle>Documents</CardTitle>
        </CardHeader>
        <CardContent className="p-0">
          {docs.isLoading ? (
            <div className="space-y-2 p-4">
              {Array.from({ length: 3 }).map((_, i) => (
                <Skeleton key={i} className="h-14 rounded-lg" />
              ))}
            </div>
          ) : list.length === 0 ? (
            <EmptyState
              className="m-4 border-0"
              icon={FileText}
              title="No documents yet"
              description="Upload formation paperwork, the EIN letter, insurance, and more."
            />
          ) : (
            <ul className="divide-y divide-line">
              {list.map((d) => (
                <li
                  key={d.id}
                  className="flex items-center gap-3 px-5 py-3.5"
                >
                  <FileText className="h-5 w-5 shrink-0 text-ink-3" />
                  <div className="min-w-0 flex-1">
                    <div className="truncate font-medium text-ink">
                      {d.title || d.original_filename}
                    </div>
                    <div className="text-xs text-ink-3">
                      {humanize(d.kind)} · {formatBytes(d.size_bytes)} ·{" "}
                      {d.storage_provider} · {formatDate(d.created_at)}
                    </div>
                  </div>
                  {d.verified && <Badge tone="good">Verified</Badge>}
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => download(d)}
                  >
                    <Download className="h-4 w-4" />
                    Download
                  </Button>
                  {canManage && (
                    <Button
                      variant="ghost"
                      size="icon"
                      aria-label="Delete document"
                      disabled={remove.isPending}
                      onClick={() => remove.mutate(d.id)}
                    >
                      <Trash2 className="h-4 w-4 text-bad" />
                    </Button>
                  )}
                </li>
              ))}
            </ul>
          )}
        </CardContent>
      </Card>
    </div>
  );
}

// ----------------------------------------------------------------------------
// Templates tab
// ----------------------------------------------------------------------------

type TemplateDraft = {
  id?: string;
  kind: string;
  name: string;
  subject: string;
  body: string;
  is_default: boolean;
};

function blankDraft(): TemplateDraft {
  return { kind: "lease", name: "", subject: "", body: "", is_default: false };
}

function toDraft(t: LlcTemplate): TemplateDraft {
  return {
    id: t.id,
    kind: t.kind,
    name: t.name,
    subject: t.subject ?? "",
    body: t.body,
    is_default: t.is_default,
  };
}

function TemplatesTab({ id, canManage }: { id: string; canManage: boolean }) {
  const templates = useLlcTemplates(id);
  const create = useCreateLlcTemplate(id);
  const update = useUpdateLlcTemplate(id);
  const remove = useDeleteLlcTemplate(id);

  const [draft, setDraft] = useState<TemplateDraft | null>(null);
  const [open, setOpen] = useState(false);
  const [preview, setPreview] = useState<string | null>(null);
  const [previewing, setPreviewing] = useState(false);

  const list = templates.data ?? [];

  const openNew = () => {
    setDraft(blankDraft());
    setPreview(null);
    setOpen(true);
  };
  const openEdit = (t: LlcTemplate) => {
    setDraft(toDraft(t));
    setPreview(null);
    setOpen(true);
  };

  const runPreview = async () => {
    if (!draft?.body) return;
    setPreviewing(true);
    try {
      const r = await api.previewTemplate(id, {
        body: draft.body,
        context: {
          tenant_name: "Sam Tenant",
          llc_name: "Your LLC",
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
      toast.error("Preview failed", { description: errMessage(e, "") });
    } finally {
      setPreviewing(false);
    }
  };

  const save = (e: React.FormEvent) => {
    e.preventDefault();
    if (!draft) return;
    const onSuccess = () => {
      setOpen(false);
      setDraft(null);
      setPreview(null);
    };
    if (draft.id) {
      update.mutate(
        {
          templateId: draft.id,
          body: {
            kind: draft.kind,
            name: draft.name,
            subject: draft.subject || undefined,
            body: draft.body,
            is_default: draft.is_default,
          },
        },
        { onSuccess }
      );
    } else {
      create.mutate(
        {
          kind: draft.kind,
          name: draft.name || "Untitled",
          subject: draft.subject || undefined,
          body: draft.body,
          is_default: draft.is_default,
        },
        { onSuccess }
      );
    }
  };

  const saving = create.isPending || update.isPending;

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between gap-3">
        <p className="text-sm text-ink-2">
          Reusable Handlebars templates for leases, letters, and emails. Use{" "}
          <span className="font-mono text-xs">{"{{tenant_name}}"}</span>,{" "}
          <span className="font-mono text-xs">{"{{rent}}"}</span>,{" "}
          <span className="font-mono text-xs">{"{{property_address}}"}</span>…
        </p>
        {canManage && (
          <Button onClick={openNew}>
            <Plus className="h-4 w-4" />
            New template
          </Button>
        )}
      </div>

      <Card>
        <CardContent className="p-0">
          {templates.isLoading ? (
            <div className="space-y-2 p-4">
              {Array.from({ length: 3 }).map((_, i) => (
                <Skeleton key={i} className="h-14 rounded-lg" />
              ))}
            </div>
          ) : list.length === 0 ? (
            <EmptyState
              className="m-4 border-0"
              icon={FileText}
              title="No templates yet"
              description="Create a lease or letter template to generate documents from."
              action={
                canManage ? (
                  <Button onClick={openNew}>
                    <Plus className="h-4 w-4" />
                    New template
                  </Button>
                ) : undefined
              }
            />
          ) : (
            <ul className="divide-y divide-line">
              {list.map((t) => (
                <li key={t.id} className="flex items-center gap-3 px-5 py-3.5">
                  <FileText className="h-5 w-5 shrink-0 text-ink-3" />
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <span className="truncate font-medium text-ink">
                        {t.name}
                      </span>
                      {t.is_default && <Badge tone="accent">Default</Badge>}
                    </div>
                    <div className="text-xs text-ink-3">{humanize(t.kind)}</div>
                  </div>
                  {canManage && (
                    <>
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => openEdit(t)}
                      >
                        Edit
                      </Button>
                      <Button
                        variant="ghost"
                        size="icon"
                        aria-label="Delete template"
                        disabled={remove.isPending}
                        onClick={() => remove.mutate(t.id)}
                      >
                        <Trash2 className="h-4 w-4 text-bad" />
                      </Button>
                    </>
                  )}
                </li>
              ))}
            </ul>
          )}
        </CardContent>
      </Card>

      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent className="max-w-2xl">
          <DialogHeader>
            <DialogTitle>
              {draft?.id ? "Edit template" : "New template"}
            </DialogTitle>
          </DialogHeader>
          {draft && (
            <form onSubmit={save} className="space-y-4">
              <div className="grid gap-4 sm:grid-cols-2">
                <SelectField
                  label="Kind"
                  value={draft.kind}
                  onChange={(e) =>
                    setDraft({ ...draft, kind: e.target.value })
                  }
                >
                  {TEMPLATE_KINDS.map((k) => (
                    <option key={k} value={k}>
                      {humanize(k)}
                    </option>
                  ))}
                </SelectField>
                <TextField
                  label="Name"
                  value={draft.name}
                  onChange={(e) =>
                    setDraft({ ...draft, name: e.target.value })
                  }
                  required
                />
              </div>
              <TextField
                label="Email subject (optional)"
                value={draft.subject}
                onChange={(e) =>
                  setDraft({ ...draft, subject: e.target.value })
                }
              />
              <TextareaField
                label="Body"
                hint="Handlebars syntax — {{tenant_name}}, {{rent}}, {{llc_name}}, {{property_address}}…"
                className="[&_textarea]:min-h-[200px] [&_textarea]:font-mono"
                value={draft.body}
                onChange={(e) => setDraft({ ...draft, body: e.target.value })}
              />
              <label className="flex items-center gap-2.5 text-sm text-ink-2">
                <Switch
                  checked={draft.is_default}
                  onCheckedChange={(v) =>
                    setDraft({ ...draft, is_default: v })
                  }
                />
                Default for its kind
              </label>

              {preview !== null && (
                <pre className="max-h-60 overflow-auto whitespace-pre-wrap rounded-lg border border-line bg-surface-2 p-3 text-xs text-ink-2">
                  {preview}
                </pre>
              )}

              <DialogFooter>
                <Button
                  type="button"
                  variant="ghost"
                  onClick={runPreview}
                  disabled={previewing || !draft.body}
                >
                  {previewing ? "Rendering…" : "Preview"}
                </Button>
                <Button type="submit" disabled={saving}>
                  {saving ? "Saving…" : "Save template"}
                </Button>
              </DialogFooter>
            </form>
          )}
        </DialogContent>
      </Dialog>
    </div>
  );
}

// ----------------------------------------------------------------------------
// Generate tab
// ----------------------------------------------------------------------------

function generatedTone(status: string): "good" | "warn" | "bad" | "neutral" {
  const s = status.toLowerCase();
  if (s === "rendered" || s === "sent" || s === "ready") return "good";
  if (s === "pending" || s === "queued") return "warn";
  if (s === "failed" || s === "error") return "bad";
  return "neutral";
}

function GenerateTab({ id, canManage }: { id: string; canManage: boolean }) {
  const templates = useLlcTemplates(id);
  const generated = useGeneratedDocuments(id);
  const generate = useGenerateDocument(id);

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

  const selectable = useMemo(
    () =>
      (templates.data ?? []).filter(
        (t) => t.kind === form.kind || form.kind === "letter"
      ),
    [templates.data, form.kind]
  );

  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    generate.mutate({
      template_id: form.template_id || undefined,
      kind: form.kind,
      title: form.title || undefined,
      recipient_name: form.recipient_name || undefined,
      recipient_email: form.recipient_email || undefined,
      property_address: form.property_address || undefined,
      lease_id: form.lease_id || undefined,
      send_email: form.send_email,
    });
  };

  const download = async (g: GeneratedDocument) => {
    try {
      const blob = await api.downloadGenerated(g.id);
      saveBlob(blob, `${g.title.replace(/[^\w.-]+/g, "_")}.pdf`);
    } catch (e) {
      toast.error("Download failed", { description: errMessage(e, "") });
    }
  };

  const docs = generated.data ?? [];

  return (
    <div className="grid gap-6 lg:grid-cols-[1fr_1fr]">
      {canManage && (
        <Card>
          <CardHeader>
            <CardTitle>Generate a document</CardTitle>
            <CardDescription>
              Render a lease or letter from a template, optionally emailing it.
            </CardDescription>
          </CardHeader>
          <CardContent>
            <form onSubmit={submit} className="space-y-4">
              <div className="grid gap-4 sm:grid-cols-2">
                <SelectField
                  label="Document type"
                  value={form.kind}
                  onChange={(e) =>
                    setForm({ ...form, kind: e.target.value, template_id: "" })
                  }
                >
                  <option value="lease">Lease contract</option>
                  <option value="letter">Tenant letter</option>
                </SelectField>
                <SelectField
                  label="Template"
                  value={form.template_id}
                  onChange={(e) =>
                    setForm({ ...form, template_id: e.target.value })
                  }
                >
                  <option value="">Built-in default</option>
                  {selectable.map((t) => (
                    <option key={t.id} value={t.id}>
                      {t.name}
                    </option>
                  ))}
                </SelectField>
              </div>
              <TextField
                label="Title (optional)"
                value={form.title}
                onChange={(e) => setForm({ ...form, title: e.target.value })}
              />
              <div className="grid gap-4 sm:grid-cols-2">
                <TextField
                  label="Recipient name"
                  value={form.recipient_name}
                  onChange={(e) =>
                    setForm({ ...form, recipient_name: e.target.value })
                  }
                />
                <TextField
                  label="Recipient email"
                  type="email"
                  value={form.recipient_email}
                  onChange={(e) =>
                    setForm({ ...form, recipient_email: e.target.value })
                  }
                />
              </div>
              <TextField
                label="Property address"
                value={form.property_address}
                onChange={(e) =>
                  setForm({ ...form, property_address: e.target.value })
                }
              />
              <TextField
                label="Lease ID (optional)"
                hint="Pulls in rent & dates from an existing lease."
                className="[&_input]:font-mono"
                placeholder="uuid"
                value={form.lease_id}
                onChange={(e) =>
                  setForm({ ...form, lease_id: e.target.value })
                }
              />
              <label className="flex items-center gap-2.5 text-sm text-ink-2">
                <Switch
                  checked={form.send_email}
                  onCheckedChange={(v) => setForm({ ...form, send_email: v })}
                />
                Email it to the recipient
              </label>
              <Button type="submit" disabled={generate.isPending}>
                <Wand2 className="h-4 w-4" />
                {generate.isPending ? "Generating…" : "Generate document"}
              </Button>
            </form>
          </CardContent>
        </Card>
      )}

      <Card>
        <CardHeader>
          <CardTitle>Generated documents</CardTitle>
        </CardHeader>
        <CardContent className="p-0">
          {generated.isLoading ? (
            <div className="space-y-2 p-4">
              {Array.from({ length: 3 }).map((_, i) => (
                <Skeleton key={i} className="h-14 rounded-lg" />
              ))}
            </div>
          ) : docs.length === 0 ? (
            <EmptyState
              className="m-4 border-0"
              icon={FileText}
              title="Nothing generated yet"
              description="Generate a lease or letter to see it listed here."
            />
          ) : (
            <ul className="divide-y divide-line">
              {docs.map((g) => (
                <li key={g.id} className="flex items-center gap-3 px-5 py-3.5">
                  <FileText className="h-5 w-5 shrink-0 text-ink-3" />
                  <div className="min-w-0 flex-1">
                    <div className="truncate font-medium text-ink">
                      {g.title}
                    </div>
                    <div className="text-xs text-ink-3">
                      {humanize(g.kind)} · {formatBytes(g.size_bytes)} ·{" "}
                      {formatDate(g.created_at)}
                    </div>
                  </div>
                  <Badge tone={generatedTone(g.status)}>{g.status}</Badge>
                  <Button variant="ghost" size="sm" onClick={() => download(g)}>
                    <Download className="h-4 w-4" />
                    Download
                  </Button>
                </li>
              ))}
            </ul>
          )}
        </CardContent>
      </Card>
    </div>
  );
}

// ----------------------------------------------------------------------------
// Branding tab
// ----------------------------------------------------------------------------

type BrandingForm = Omit<LlcBranding, "llc_id">;

function BrandingTab({ id, canManage }: { id: string; canManage: boolean }) {
  const branding = useLlcBranding(id);
  const docs = useLlcDocuments(id);
  const put = usePutLlcBranding(id);

  const [form, setForm] = useState<BrandingForm | null>(null);

  React.useEffect(() => {
    if (branding.data) {
      const { llc_id: _llc, ...rest } = branding.data;
      void _llc;
      setForm(rest);
    }
  }, [branding.data]);

  const logos = (docs.data ?? []).filter((d) => d.kind === "logo");

  if (branding.isLoading || !form) {
    return (
      <Card>
        <CardContent className="space-y-3">
          {Array.from({ length: 4 }).map((_, i) => (
            <Skeleton key={i} className="h-10 rounded-lg" />
          ))}
        </CardContent>
      </Card>
    );
  }

  const set = (k: keyof BrandingForm, v: string) =>
    setForm((f) => (f ? { ...f, [k]: v === "" ? null : v } : f));

  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    if (form) put.mutate(form);
  };

  return (
    <form onSubmit={submit} className="space-y-6">
      <Card>
        <CardHeader>
          <CardTitle>Logo &amp; colours</CardTitle>
          {!canManage && (
            <CardDescription>
              Read-only — requires llc:manage or theme:write.
            </CardDescription>
          )}
        </CardHeader>
        <CardContent className="grid gap-4 sm:grid-cols-2">
          <Field
            label="Logo"
            hint={
              logos.length === 0
                ? 'Upload a document of type "logo" to choose one here.'
                : undefined
            }
          >
            <NativeSelect
              value={form.logo_document_id ?? ""}
              disabled={!canManage}
              onChange={(e) => set("logo_document_id", e.target.value)}
            >
              <option value="">— none —</option>
              {logos.map((l) => (
                <option key={l.id} value={l.id}>
                  {l.title || l.original_filename}
                </option>
              ))}
            </NativeSelect>
          </Field>
          <div className="grid grid-cols-2 gap-4">
            <Field label="Primary color">
              <div className="flex items-center gap-2">
                <input
                  type="color"
                  aria-label="Primary color picker"
                  className="h-10 w-10 shrink-0 cursor-pointer rounded-lg border border-line bg-surface disabled:opacity-50"
                  value={form.primary_color || "#F5451F"}
                  disabled={!canManage}
                  onChange={(e) => set("primary_color", e.target.value)}
                />
                <Input
                  value={form.primary_color ?? ""}
                  placeholder="#F5451F"
                  disabled={!canManage}
                  className="font-mono"
                  onChange={(e) => set("primary_color", e.target.value)}
                />
              </div>
            </Field>
            <Field label="Accent color">
              <div className="flex items-center gap-2">
                <input
                  type="color"
                  aria-label="Accent color picker"
                  className="h-10 w-10 shrink-0 cursor-pointer rounded-lg border border-line bg-surface disabled:opacity-50"
                  value={form.accent_color || "#1C7C53"}
                  disabled={!canManage}
                  onChange={(e) => set("accent_color", e.target.value)}
                />
                <Input
                  value={form.accent_color ?? ""}
                  placeholder="#1C7C53"
                  disabled={!canManage}
                  className="font-mono"
                  onChange={(e) => set("accent_color", e.target.value)}
                />
              </div>
            </Field>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Signature block</CardTitle>
        </CardHeader>
        <CardContent className="grid gap-4 sm:grid-cols-2">
          <TextField
            label="Signature name"
            placeholder="Jane Doe"
            disabled={!canManage}
            value={form.signature_name ?? ""}
            onChange={(e) => set("signature_name", e.target.value)}
          />
          <TextField
            label="Signature title"
            placeholder="Managing Member"
            disabled={!canManage}
            value={form.signature_title ?? ""}
            onChange={(e) => set("signature_title", e.target.value)}
          />
          <TextareaField
            label="Signature block"
            className="sm:col-span-2 [&_textarea]:font-mono"
            disabled={!canManage}
            value={form.signature_block ?? ""}
            onChange={(e) => set("signature_block", e.target.value)}
          />
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Letterhead &amp; footer</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <TextareaField
            label="Letterhead (top of documents)"
            className="[&_textarea]:font-mono"
            disabled={!canManage}
            value={form.letterhead ?? ""}
            onChange={(e) => set("letterhead", e.target.value)}
          />
          <TextareaField
            label="Footer / disclaimer"
            className="[&_textarea]:font-mono"
            disabled={!canManage}
            value={form.footer ?? ""}
            onChange={(e) => set("footer", e.target.value)}
          />
        </CardContent>
      </Card>

      {canManage && (
        <Button type="submit" disabled={put.isPending}>
          <Palette className="h-4 w-4" />
          {put.isPending ? "Saving…" : "Save branding"}
        </Button>
      )}
    </form>
  );
}
