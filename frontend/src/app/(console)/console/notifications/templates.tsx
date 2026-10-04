"use client";

// Message templates: the platform catalog with this workspace's copies laid
// over it. Import everything as editable copies, or edit and reset one at a
// time. Changes apply to the next send.

import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Download, FileText, Pencil, RotateCcw } from "lucide-react";
import { toast } from "sonner";
import { api, type NotificationTemplate } from "@/lib/api";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Field, Input, fieldClass } from "@/components/ui/input";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/menu";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

function errMsg(e: unknown, fallback = "That didn't work") {
  return e instanceof Error ? e.message : fallback;
}

export function Templates() {
  const qc = useQueryClient();
  const templates = useQuery({
    queryKey: ["notification-templates"],
    queryFn: api.notificationTemplates,
  });
  const [editing, setEditing] = useState<NotificationTemplate | null>(null);
  const [importing, setImporting] = useState(false);
  const customized = templates.data?.filter((t) => t.customized).length ?? 0;

  async function importAll() {
    setImporting(true);
    try {
      const r = await api.importNotificationTemplates();
      toast.success(
        r.imported > 0
          ? `Imported ${r.imported} of ${r.total} platform templates`
          : "Every platform template already has a workspace copy"
      );
      await qc.invalidateQueries({ queryKey: ["notification-templates"] });
    } catch (e) {
      toast.error(errMsg(e, "Couldn't import them"));
    } finally {
      setImporting(false);
    }
  }

  return (
    <Panel className="overflow-hidden">
      <PanelHeader
        title={
          <span className="flex items-center gap-2">
            <FileText className="size-4" />
            Message templates
            {customized > 0 && (
              <Badge tone="accent">{customized} customized</Badge>
            )}
          </span>
        }
        description={
          <>
            What outbound email, texts, push, chat and in-app messages say.
            Placeholders like{" "}
            <code className="rounded bg-fill px-1 font-mono text-[12px]">
              {"{signer}"}
            </code>{" "}
            or{" "}
            <code className="rounded bg-fill px-1 font-mono text-[12px]">
              {"{sign_url}"}
            </code>{" "}
            fill in when it sends.
          </>
        }
        action={
          <Button
            size="sm"
            variant="secondary"
            loading={importing}
            onClick={importAll}
          >
            {!importing && <Download />}
            Import all
          </Button>
        }
      />
      <div className="mt-4 border-t border-line">
        {templates.isLoading && (
          <div className="space-y-2 p-5">
            {Array.from({ length: 4 }, (_, i) => (
              <Skeleton key={i} className="h-12" />
            ))}
          </div>
        )}
        {templates.error && (
          <p className="p-5 text-[13px] text-bad">
            Couldn&apos;t load templates: {templates.error.message}
          </p>
        )}
        {templates.data?.length === 0 && (
          <EmptyState icon={<FileText />} title="No templates" />
        )}
        <ul className="divide-y divide-line">
          {templates.data?.map((t) => (
            <li key={t.key}>
              <button
                type="button"
                onClick={() => setEditing(t)}
                className="flex w-full items-center gap-3 px-5 py-3 text-left transition hover:bg-fill-2"
              >
                <div className="min-w-0 flex-1">
                  <div className="truncate font-mono text-[13px] font-medium text-fg">
                    {t.key}
                  </div>
                  <div className="truncate text-xs text-fg-3">
                    {t.subject || t.sms}
                  </div>
                </div>
                <Badge tone={t.customized ? "accent" : "neutral"}>
                  {t.customized
                    ? t.has_default
                      ? "customized"
                      : "custom"
                    : "platform default"}
                </Badge>
                <Pencil className="size-4 shrink-0 text-fg-4" />
              </button>
            </li>
          ))}
        </ul>
      </div>
      {editing && (
        <TemplateDialog
          key={editing.key}
          template={editing}
          onClose={() => setEditing(null)}
        />
      )}
    </Panel>
  );
}

function TemplateDialog({
  template,
  onClose,
}: {
  template: NotificationTemplate;
  onClose: () => void;
}) {
  const qc = useQueryClient();
  const [subject, setSubject] = useState(template.subject);
  const [body, setBody] = useState(template.body);
  const [sms, setSms] = useState(template.sms);
  const [busy, setBusy] = useState<"save" | "reset" | null>(null);
  const valid = !!(subject.trim() || body.trim() || sms.trim());

  async function run(
    which: "save" | "reset",
    fn: () => Promise<unknown>,
    ok: string
  ) {
    setBusy(which);
    try {
      await fn();
      toast.success(ok);
      await qc.invalidateQueries({ queryKey: ["notification-templates"] });
      onClose();
    } catch (e) {
      toast.error(errMsg(e));
      setBusy(null);
    }
  }

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-h-[90dvh] max-w-2xl overflow-y-auto">
        <DialogTitle className="font-mono text-[16px] font-semibold">
          {template.key}
        </DialogTitle>
        <DialogDescription className="mt-1 flex items-center gap-2 text-[13px] text-fg-3">
          <Badge tone={template.customized ? "accent" : "neutral"}>
            {template.customized ? "workspace copy" : "platform default"}
          </Badge>
          Saving keeps a copy for this workspace.
        </DialogDescription>
        <form
          className="mt-4 space-y-3"
          onSubmit={(e) => {
            e.preventDefault();
            void run(
              "save",
              () =>
                api.updateNotificationTemplate(template.key, {
                  subject: subject.trim(),
                  body: body.trim(),
                  sms: sms.trim(),
                }),
              `Saved ${template.key}`
            );
          }}
        >
          <Field
            label="Subject"
            hint="Email subject, and the push and in-app title."
          >
            {(p) => (
              <Input
                {...p}
                value={subject}
                onChange={(e) => setSubject(e.target.value)}
              />
            )}
          </Field>
          <Field label="Email body">
            {(p) => (
              <textarea
                {...p}
                rows={7}
                className={cn(
                  fieldClass,
                  "w-full font-mono text-[12px] leading-relaxed"
                )}
                value={body}
                onChange={(e) => setBody(e.target.value)}
              />
            )}
          </Field>
          <Field
            label="Short text"
            hint="Used for texts, chat, push and in-app."
          >
            {(p) => (
              <textarea
                {...p}
                rows={3}
                className={cn(
                  fieldClass,
                  "w-full font-mono text-[12px] leading-relaxed"
                )}
                value={sms}
                onChange={(e) => setSms(e.target.value)}
              />
            )}
          </Field>
          <div className="flex flex-wrap items-center gap-2 pt-1">
            {template.customized && template.has_default && (
              <Button
                type="button"
                variant="danger"
                size="sm"
                loading={busy === "reset"}
                disabled={!!busy}
                onClick={() =>
                  run(
                    "reset",
                    () => api.resetNotificationTemplate(template.key),
                    `${template.key} is back to the platform default`
                  )
                }
              >
                {busy !== "reset" && <RotateCcw />}
                Reset to default
              </Button>
            )}
            <div className="ml-auto flex gap-2">
              <Button type="button" variant="ghost" onClick={onClose}>
                Cancel
              </Button>
              <Button
                type="submit"
                loading={busy === "save"}
                disabled={!!busy || !valid}
              >
                Save
              </Button>
            </div>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}
