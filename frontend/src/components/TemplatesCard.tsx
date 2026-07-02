"use client";

// Message-templates editor: the platform notification catalog with the
// workspace's copies layered in. Import everything as editable DB copies, or
// edit/reset one template at a time — changes apply to the next send.

import { useCallback, useEffect, useState } from "react";
import { api, type NotificationTemplate } from "@/lib/api";
import { Badge, Card } from "@/components/ui";

export function TemplatesCard() {
  const [templates, setTemplates] = useState<NotificationTemplate[] | null>(
    null
  );
  const [error, setError] = useState<string | null>(null);
  const [info, setInfo] = useState<string | null>(null);
  const [openKey, setOpenKey] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  const load = useCallback(() => {
    api
      .notificationTemplates()
      .then((t) => {
        setTemplates(t);
        setError(null);
      })
      .catch((e) => setError(e.message));
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  async function importAll() {
    setBusy("import");
    setError(null);
    setInfo(null);
    try {
      const r = await api.importNotificationTemplates();
      setInfo(
        r.imported > 0
          ? `Imported ${r.imported} of ${r.total} platform templates into this workspace.`
          : "All platform templates already have workspace copies."
      );
      load();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(null);
    }
  }

  async function reset(key: string) {
    setBusy(`reset-${key}`);
    setError(null);
    try {
      await api.resetNotificationTemplate(key);
      setOpenKey(null);
      load();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(null);
    }
  }

  const customized = templates?.filter((t) => t.customized).length ?? 0;

  return (
    <Card className="p-5">
      <div className="mb-1 flex flex-wrap items-center gap-3">
        <h2 className="flex-1 font-display text-lg font-bold">
          Message templates
        </h2>
        {templates && customized > 0 && (
          <Badge tone="accent">{customized} customized</Badge>
        )}
        <button
          onClick={importAll}
          disabled={busy === "import"}
          className="rounded-lg border border-line px-3 py-1.5 text-sm font-semibold disabled:opacity-50"
        >
          {busy === "import" ? "Importing…" : "Import all to workspace"}
        </button>
      </div>
      <p className="mb-3 text-sm text-ink-3">
        What outbound email, SMS, push, chat, and in-app messages say —
        application updates, e-signature requests, and more. Placeholders like{" "}
        <code className="rounded bg-surface-2 px-1">{"{signer}"}</code> or{" "}
        <code className="rounded bg-surface-2 px-1">{"{sign_url}"}</code> fill
        in at send time. Import the platform catalog to hold editable copies, or
        edit individual templates below.
      </p>

      {error && <p className="mb-2 text-sm text-bad">{error}</p>}
      {info && <p className="mb-2 text-sm text-good">{info}</p>}

      <div className="space-y-2">
        {templates?.map((t) =>
          openKey === t.key ? (
            <TemplateEditor
              key={t.key}
              template={t}
              busy={busy === `save-${t.key}`}
              onCancel={() => setOpenKey(null)}
              onReset={t.customized ? () => reset(t.key) : undefined}
              onSave={async (fields) => {
                setBusy(`save-${t.key}`);
                setError(null);
                try {
                  await api.updateNotificationTemplate(t.key, fields);
                  setOpenKey(null);
                  load();
                } catch (e) {
                  setError((e as Error).message);
                } finally {
                  setBusy(null);
                }
              }}
            />
          ) : (
            <div
              key={t.key}
              className="flex flex-wrap items-center gap-3 rounded-lg border border-line px-4 py-2"
            >
              <div className="min-w-0 flex-1">
                <span className="font-mono text-sm font-semibold">{t.key}</span>
                <p className="truncate text-xs text-ink-3">{t.subject}</p>
              </div>
              <Badge tone={t.customized ? "accent" : "neutral"}>
                {t.customized
                  ? t.has_default
                    ? "customized"
                    : "custom"
                  : "platform default"}
              </Badge>
              <button
                onClick={() => setOpenKey(t.key)}
                className="text-sm font-semibold text-accent"
              >
                Edit
              </button>
            </div>
          )
        )}
        {templates === null && !error && (
          <p className="text-sm text-ink-3">Loading…</p>
        )}
      </div>
    </Card>
  );
}

function TemplateEditor({
  template,
  busy,
  onSave,
  onCancel,
  onReset,
}: {
  template: NotificationTemplate;
  busy: boolean;
  onSave: (fields: { subject: string; body: string; sms: string }) => void;
  onCancel: () => void;
  onReset?: () => void;
}) {
  const [subject, setSubject] = useState(template.subject);
  const [body, setBody] = useState(template.body);
  const [sms, setSms] = useState(template.sms);
  const valid = subject.trim() || body.trim() || sms.trim();

  return (
    <div className="space-y-3 rounded-lg border border-accent bg-surface-2 px-4 py-3">
      <div className="flex items-center gap-3">
        <span className="font-mono text-sm font-semibold">{template.key}</span>
        <Badge tone={template.customized ? "accent" : "neutral"}>
          {template.customized ? "workspace copy" : "editing platform default"}
        </Badge>
      </div>
      <label className="block text-sm">
        <span className="mb-1 block text-ink-3">
          Subject (email; push/in-app title)
        </span>
        <input
          value={subject}
          onChange={(e) => setSubject(e.target.value)}
          className="w-full rounded-lg border border-line bg-surface px-3 py-2"
        />
      </label>
      <label className="block text-sm">
        <span className="mb-1 block text-ink-3">Email body</span>
        <textarea
          value={body}
          onChange={(e) => setBody(e.target.value)}
          rows={5}
          className="w-full rounded-lg border border-line bg-surface px-3 py-2 font-mono text-xs leading-relaxed"
        />
      </label>
      <label className="block text-sm">
        <span className="mb-1 block text-ink-3">
          Short text (SMS, chat, push, in-app)
        </span>
        <textarea
          value={sms}
          onChange={(e) => setSms(e.target.value)}
          rows={2}
          className="w-full rounded-lg border border-line bg-surface px-3 py-2 font-mono text-xs leading-relaxed"
        />
      </label>
      <div className="flex flex-wrap items-center gap-3">
        <button
          onClick={() =>
            onSave({
              subject: subject.trim(),
              body: body.trim(),
              sms: sms.trim(),
            })
          }
          disabled={busy || !valid}
          className="rounded-lg bg-accent px-4 py-2 text-sm font-semibold text-on-accent disabled:opacity-50"
        >
          {busy ? "Saving…" : "Save to workspace"}
        </button>
        <button
          onClick={onCancel}
          className="rounded-lg border border-line px-4 py-2 text-sm font-semibold"
        >
          Cancel
        </button>
        {onReset && template.has_default && (
          <button
            onClick={onReset}
            className="ml-auto text-sm font-semibold text-bad"
          >
            Reset to platform default
          </button>
        )}
      </div>
    </div>
  );
}
