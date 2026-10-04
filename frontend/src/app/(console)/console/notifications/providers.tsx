"use client";

// Delivery providers: the workspace's own email, SMS and chat services.
// Credentials go into the vault and only their last four characters come
// back. A channel with no provider is simulated.

import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  KeyRound,
  Plus,
  Power,
  PowerOff,
  Send,
  Star,
  Trash2,
  Unplug,
} from "lucide-react";
import { toast } from "sonner";
import { api, type NotificationProvider } from "@/lib/api";
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

/** Channel, the services for it, and the settings each one needs. */
const CATALOG: {
  channel: string;
  label: string;
  kinds: {
    kind: string;
    label: string;
    fields: { key: string; label: string; placeholder: string }[];
    credentialLabel: string;
  }[];
}[] = [
  {
    channel: "email",
    label: "Email",
    kinds: [
      {
        kind: "resend",
        label: "Resend",
        fields: [
          {
            key: "from",
            label: "From address",
            placeholder: "notify@yourdomain.com",
          },
        ],
        credentialLabel: "API key (re_…)",
      },
      {
        kind: "sendgrid",
        label: "SendGrid",
        fields: [
          {
            key: "from",
            label: "From address",
            placeholder: "notify@yourdomain.com",
          },
        ],
        credentialLabel: "API key (SG.…)",
      },
      {
        kind: "postmark",
        label: "Postmark",
        fields: [
          {
            key: "from",
            label: "From address",
            placeholder: "notify@yourdomain.com",
          },
        ],
        credentialLabel: "Server token",
      },
    ],
  },
  {
    channel: "sms",
    label: "SMS",
    kinds: [
      {
        kind: "twilio",
        label: "Twilio",
        fields: [
          { key: "account_sid", label: "Account SID", placeholder: "AC…" },
          {
            key: "from",
            label: "Sending number",
            placeholder: "+15551234567",
          },
        ],
        credentialLabel: "Auth token",
      },
    ],
  },
  {
    channel: "chat",
    label: "Chat",
    kinds: [
      {
        kind: "slack",
        label: "Slack",
        fields: [],
        credentialLabel: "Incoming webhook URL",
      },
      {
        kind: "discord",
        label: "Discord",
        fields: [],
        credentialLabel: "Webhook URL",
      },
    ],
  },
];

function errMsg(e: unknown, fallback = "That didn't work") {
  return e instanceof Error ? e.message : fallback;
}

export function Providers() {
  const qc = useQueryClient();
  const providers = useQuery({
    queryKey: ["notification-providers"],
    queryFn: api.notificationProviders,
  });
  const [adding, setAdding] = useState(false);
  const [rotating, setRotating] = useState<NotificationProvider | null>(null);
  const [testing, setTesting] = useState<NotificationProvider | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const refresh = () =>
    qc.invalidateQueries({ queryKey: ["notification-providers"] });

  async function run(id: string, fn: () => Promise<unknown>, ok?: string) {
    setBusy(id);
    try {
      await fn();
      if (ok) toast.success(ok);
      await refresh();
    } catch (e) {
      toast.error(errMsg(e));
    } finally {
      setBusy(null);
    }
  }

  async function test(p: NotificationProvider) {
    if (p.channel === "sms") {
      setTesting(p);
      return;
    }
    await run(
      p.id,
      () => api.testNotificationProvider(p.id),
      `Test ${p.channel} queued through ${p.kind}. The notification log shows how it went.`
    );
  }

  return (
    <Panel className="overflow-hidden">
      <PanelHeader
        title={
          <span className="flex items-center gap-2">
            <Unplug className="size-4" />
            Delivery providers
          </span>
        }
        description="Your own services for outbound messages. Keys are sealed in the vault; only the last four characters show again. Without a provider, sends are simulated."
        action={
          <Button size="sm" onClick={() => setAdding(true)}>
            <Plus />
            Add provider
          </Button>
        }
      />
      <div className="mt-4 border-t border-line">
        {providers.isLoading && (
          <div className="space-y-2 p-5">
            <Skeleton className="h-12" />
            <Skeleton className="h-12" />
          </div>
        )}
        {providers.error && (
          <p className="p-5 text-[13px] text-bad">
            Couldn&apos;t load providers: {providers.error.message}
          </p>
        )}
        {providers.data?.length === 0 && (
          <EmptyState
            icon={<Unplug />}
            title="No providers yet"
            description="Email and texts are simulated until you connect one."
          />
        )}
        <ul className="divide-y divide-line">
          {providers.data?.map((p) => (
            <li
              key={p.id}
              className="flex flex-col gap-2 px-5 py-3 sm:flex-row sm:items-center sm:gap-4"
            >
              <div className="flex min-w-0 flex-1 flex-wrap items-center gap-2">
                <Badge>{p.channel}</Badge>
                <span className="text-[14px] font-medium text-fg capitalize">
                  {p.kind}
                </span>
                {typeof p.config.from === "string" && (
                  <span className="truncate text-xs text-fg-3">
                    {p.config.from}
                  </span>
                )}
                {p.credential_last4 && (
                  <span className="font-mono text-xs text-fg-3">
                    ····{p.credential_last4}
                  </span>
                )}
                {p.is_default && <Badge tone="accent">default</Badge>}
                <Badge tone={p.enabled ? "good" : "neutral"}>
                  {p.enabled ? "on" : "off"}
                </Badge>
              </div>
              <div className="flex flex-wrap items-center gap-1">
                <Button
                  size="sm"
                  variant="secondary"
                  disabled={busy === p.id}
                  onClick={() => test(p)}
                >
                  <Send />
                  Test
                </Button>
                {!p.is_default && (
                  <Button
                    size="sm"
                    variant="ghost"
                    disabled={busy === p.id}
                    onClick={() =>
                      run(
                        p.id,
                        () =>
                          api.updateNotificationProvider(p.id, {
                            is_default: true,
                          }),
                        `${p.kind} is the default for ${p.channel}`
                      )
                    }
                  >
                    <Star />
                    Make default
                  </Button>
                )}
                <Button
                  size="sm"
                  variant="ghost"
                  disabled={busy === p.id}
                  onClick={() => setRotating(p)}
                >
                  <KeyRound />
                  Rotate key
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  disabled={busy === p.id}
                  onClick={() =>
                    run(p.id, () =>
                      api.updateNotificationProvider(p.id, {
                        enabled: !p.enabled,
                      })
                    )
                  }
                >
                  {p.enabled ? <PowerOff /> : <Power />}
                  {p.enabled ? "Turn off" : "Turn on"}
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  disabled={busy === p.id}
                  aria-label={`Remove ${p.kind}`}
                  onClick={() => {
                    if (
                      !window.confirm(
                        `Remove ${p.kind}? ${p.channel} goes back to simulated sends unless another provider covers it.`
                      )
                    )
                      return;
                    void run(
                      p.id,
                      () => api.deleteNotificationProvider(p.id),
                      `Removed ${p.kind}`
                    );
                  }}
                >
                  <Trash2 />
                </Button>
              </div>
            </li>
          ))}
        </ul>
      </div>

      {adding && (
        <AddProviderDialog
          onClose={() => setAdding(false)}
          onAdded={() => {
            setAdding(false);
            void refresh();
          }}
        />
      )}
      {rotating && (
        <RotateDialog
          provider={rotating}
          onClose={() => setRotating(null)}
          onDone={() => {
            setRotating(null);
            void refresh();
          }}
        />
      )}
      {testing && (
        <TestSmsDialog provider={testing} onClose={() => setTesting(null)} />
      )}
    </Panel>
  );
}

function AddProviderDialog({
  onClose,
  onAdded,
}: {
  onClose: () => void;
  onAdded: () => void;
}) {
  const [channel, setChannel] = useState("email");
  const [kind, setKind] = useState("resend");
  const [config, setConfig] = useState<Record<string, string>>({});
  const [credential, setCredential] = useState("");
  const [makeDefault, setMakeDefault] = useState(false);
  const [busy, setBusy] = useState(false);
  const ch = CATALOG.find((c) => c.channel === channel) ?? CATALOG[0];
  const k = ch.kinds.find((x) => x.kind === kind) ?? ch.kinds[0];

  async function submit() {
    setBusy(true);
    try {
      await api.createNotificationProvider({
        channel,
        kind: k.kind,
        config,
        credential: credential.trim() || undefined,
        is_default: makeDefault || undefined,
      });
      toast.success(`${k.label} added`);
      onAdded();
    } catch (e) {
      toast.error(errMsg(e, "Couldn't add the provider"));
      setBusy(false);
    }
  }

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-md">
        <DialogTitle className="text-[17px] font-semibold">
          Add a provider
        </DialogTitle>
        <DialogDescription className="mt-1 text-[13px] text-fg-3">
          The key is sealed in the vault as soon as you save.
        </DialogDescription>
        <form
          className="mt-4 space-y-3"
          onSubmit={(e) => {
            e.preventDefault();
            void submit();
          }}
        >
          <div className="grid grid-cols-2 gap-3">
            <Field label="Channel">
              {(p) => (
                <select
                  {...p}
                  className={cn(fieldClass, "h-11 w-full")}
                  value={channel}
                  onChange={(e) => {
                    const c =
                      CATALOG.find((x) => x.channel === e.target.value) ??
                      CATALOG[0];
                    setChannel(c.channel);
                    setKind(c.kinds[0].kind);
                    setConfig({});
                  }}
                >
                  {CATALOG.map((c) => (
                    <option key={c.channel} value={c.channel}>
                      {c.label}
                    </option>
                  ))}
                </select>
              )}
            </Field>
            <Field label="Service">
              {(p) => (
                <select
                  {...p}
                  className={cn(fieldClass, "h-11 w-full")}
                  value={k.kind}
                  onChange={(e) => {
                    setKind(e.target.value);
                    setConfig({});
                  }}
                >
                  {ch.kinds.map((x) => (
                    <option key={x.kind} value={x.kind}>
                      {x.label}
                    </option>
                  ))}
                </select>
              )}
            </Field>
          </div>
          {k.fields.map((f) => (
            <Field key={f.key} label={f.label}>
              {(p) => (
                <Input
                  {...p}
                  className="font-mono text-[13px]"
                  placeholder={f.placeholder}
                  value={config[f.key] ?? ""}
                  onChange={(e) =>
                    setConfig((c) => ({ ...c, [f.key]: e.target.value }))
                  }
                />
              )}
            </Field>
          ))}
          <Field label={k.credentialLabel}>
            {(p) => (
              <Input
                {...p}
                type="password"
                autoComplete="off"
                className="font-mono text-[13px]"
                value={credential}
                onChange={(e) => setCredential(e.target.value)}
              />
            )}
          </Field>
          <label className="flex items-center gap-2 text-[13px] text-fg-2">
            <input
              type="checkbox"
              checked={makeDefault}
              onChange={(e) => setMakeDefault(e.target.checked)}
              className="size-4 accent-[var(--accent)]"
            />
            Make it the default for {ch.label.toLowerCase()}
          </label>
          <div className="flex justify-end gap-2">
            <Button type="button" variant="ghost" onClick={onClose}>
              Cancel
            </Button>
            <Button type="submit" loading={busy}>
              {!busy && <Plus />}
              Add provider
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function RotateDialog({
  provider,
  onClose,
  onDone,
}: {
  provider: NotificationProvider;
  onClose: () => void;
  onDone: () => void;
}) {
  const [credential, setCredential] = useState("");
  const [busy, setBusy] = useState(false);
  const label =
    CATALOG.flatMap((c) => c.kinds).find((k) => k.kind === provider.kind)
      ?.credentialLabel ?? "New key";

  async function save() {
    if (!credential.trim()) return;
    setBusy(true);
    try {
      await api.updateNotificationProvider(provider.id, {
        credential: credential.trim(),
      });
      toast.success(`New key saved for ${provider.kind}`);
      onDone();
    } catch (e) {
      toast.error(errMsg(e, "Couldn't save the key"));
      setBusy(false);
    }
  }

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-sm">
        <DialogTitle className="text-[17px] font-semibold capitalize">
          Rotate {provider.kind} key
        </DialogTitle>
        <DialogDescription className="mt-1 text-[13px] text-fg-3">
          The old key stops being used as soon as you save.
        </DialogDescription>
        <form
          className="mt-4 space-y-3"
          onSubmit={(e) => {
            e.preventDefault();
            void save();
          }}
        >
          <Field label={label}>
            {(p) => (
              <Input
                {...p}
                autoFocus
                type="password"
                autoComplete="off"
                className="font-mono text-[13px]"
                value={credential}
                onChange={(e) => setCredential(e.target.value)}
              />
            )}
          </Field>
          <div className="flex justify-end gap-2">
            <Button type="button" variant="ghost" onClick={onClose}>
              Cancel
            </Button>
            <Button type="submit" loading={busy} disabled={!credential.trim()}>
              Save key
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function TestSmsDialog({
  provider,
  onClose,
}: {
  provider: NotificationProvider;
  onClose: () => void;
}) {
  const [to, setTo] = useState("");
  const [busy, setBusy] = useState(false);

  async function send() {
    if (!to.trim()) return;
    setBusy(true);
    try {
      await api.testNotificationProvider(provider.id, to.trim());
      toast.success(
        `Test text queued through ${provider.kind}. The notification log shows how it went.`
      );
      onClose();
    } catch (e) {
      toast.error(errMsg(e, "Couldn't send the test"));
      setBusy(false);
    }
  }

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-sm">
        <DialogTitle className="text-[17px] font-semibold">
          Send a test text
        </DialogTitle>
        <DialogDescription className="mt-1 text-[13px] text-fg-3">
          Through {provider.kind}, to a phone you can check.
        </DialogDescription>
        <form
          className="mt-4 space-y-3"
          onSubmit={(e) => {
            e.preventDefault();
            void send();
          }}
        >
          <Field label="Phone number" hint="With the country code, e.g. +1">
            {(p) => (
              <Input
                {...p}
                autoFocus
                type="tel"
                inputMode="tel"
                placeholder="+15551234567"
                value={to}
                onChange={(e) => setTo(e.target.value)}
              />
            )}
          </Field>
          <div className="flex justify-end gap-2">
            <Button type="button" variant="ghost" onClick={onClose}>
              Cancel
            </Button>
            <Button type="submit" loading={busy} disabled={!to.trim()}>
              {!busy && <Send />}
              Send test
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}
