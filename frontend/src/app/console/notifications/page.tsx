"use client";

// Notifications console: the signed-in user's in-app inbox, browser push
// opt-in, and (for integrations managers) the delivery-provider setup —
// Resend/SendGrid/Postmark for email, Twilio for SMS, Slack/Discord for chat.

import { useCallback, useEffect, useState } from "react";
import { api, type InboxEntry, type NotificationProvider } from "@/lib/api";
import {
  currentSubscription,
  disablePush,
  enablePush,
  pushSupported,
} from "@/lib/push";
import { Badge, Card } from "@/components/ui";
import { TemplatesCard } from "@/components/TemplatesCard";
import { useAuth } from "@/lib/auth";

/** Channel → provider kinds + which config fields each needs. */
const PROVIDER_CATALOG: {
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
          { key: "from", label: "Sending number", placeholder: "+15551234567" },
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

function timeAgo(iso: string): string {
  const s = Math.max(
    1,
    Math.floor((Date.now() - new Date(iso).getTime()) / 1000)
  );
  if (s < 60) return `${s}s ago`;
  if (s < 3600) return `${Math.floor(s / 60)}m ago`;
  if (s < 86400) return `${Math.floor(s / 3600)}h ago`;
  return `${Math.floor(s / 86400)}d ago`;
}

export default function NotificationsPage() {
  const { can } = useAuth();
  const manage = can("integrations:manage");

  const [error, setError] = useState<string | null>(null);
  const [info, setInfo] = useState<string | null>(null);

  // ---- inbox ----
  const [inbox, setInbox] = useState<InboxEntry[] | null>(null);

  // ---- push ----
  const [pushOk] = useState(pushSupported);
  const [pushOn, setPushOn] = useState(false);
  const [pushBusy, setPushBusy] = useState(false);

  // ---- providers ----
  const [providers, setProviders] = useState<NotificationProvider[] | null>(
    null
  );
  const [channel, setChannel] = useState("email");
  const [kind, setKind] = useState("resend");
  const [config, setConfig] = useState<Record<string, string>>({});
  const [credential, setCredential] = useState("");
  const [saving, setSaving] = useState(false);

  const catalogChannel = PROVIDER_CATALOG.find((c) => c.channel === channel)!;
  const catalogKind =
    catalogChannel.kinds.find((k) => k.kind === kind) ??
    catalogChannel.kinds[0];

  const load = useCallback(() => {
    api
      .inbox()
      .then(setInbox)
      .catch((e) => setError(e.message));
    if (manage) {
      api
        .notificationProviders()
        .then(setProviders)
        .catch((e) => setError(e.message));
    }
  }, [manage]);

  useEffect(() => {
    load();
    currentSubscription().then((s) => setPushOn(!!s));
  }, [load]);

  async function markRead(id: string) {
    try {
      await api.markNotificationRead(id);
      load();
    } catch (e) {
      setError((e as Error).message);
    }
  }

  async function markAll() {
    try {
      await api.markAllNotificationsRead();
      load();
    } catch (e) {
      setError((e as Error).message);
    }
  }

  async function togglePush() {
    setPushBusy(true);
    setError(null);
    try {
      if (pushOn) {
        await disablePush();
        setPushOn(false);
      } else {
        await enablePush();
        setPushOn(true);
      }
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setPushBusy(false);
    }
  }

  async function sendTestPush() {
    try {
      await api.testPush();
      setInfo("Test push queued — it arrives via the background scheduler.");
    } catch (e) {
      setError((e as Error).message);
    }
  }

  async function addProvider(e: React.FormEvent) {
    e.preventDefault();
    setSaving(true);
    setError(null);
    try {
      await api.createNotificationProvider({
        channel,
        kind: catalogKind.kind,
        config,
        credential: credential.trim() || undefined,
      });
      setConfig({});
      setCredential("");
      load();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setSaving(false);
    }
  }

  async function testProvider(p: NotificationProvider) {
    setError(null);
    setInfo(null);
    try {
      let to: string | undefined;
      if (p.channel === "sms") {
        to =
          window.prompt("Send the test SMS to (E.164 phone number):") ??
          undefined;
        if (!to) return;
      }
      await api.testNotificationProvider(p.id, to);
      setInfo(
        `Test ${p.channel} queued through ${p.kind} — check the notification log for delivery status.`
      );
    } catch (e) {
      setError((e as Error).message);
    }
  }

  async function setDefault(p: NotificationProvider) {
    try {
      await api.updateNotificationProvider(p.id, { is_default: true });
      load();
    } catch (e) {
      setError((e as Error).message);
    }
  }

  async function toggleEnabled(p: NotificationProvider) {
    try {
      await api.updateNotificationProvider(p.id, { enabled: !p.enabled });
      load();
    } catch (e) {
      setError((e as Error).message);
    }
  }

  async function removeProvider(p: NotificationProvider) {
    try {
      await api.deleteNotificationProvider(p.id);
      load();
    } catch (e) {
      setError((e as Error).message);
    }
  }

  const unread = inbox?.filter((n) => !n.read_at).length ?? 0;

  return (
    <div className="space-y-6">
      <div>
        <h1 className="font-display text-3xl font-extrabold tracking-tight">
          Notifications
        </h1>
        <p className="text-ink-3">
          Your in-app inbox, browser push, and how this workspace delivers
          email, SMS, and chat notifications.
        </p>
      </div>

      {error && <p className="text-bad">{error}</p>}
      {info && <p className="text-good">{info}</p>}

      <Card className="p-5">
        <div className="mb-3 flex items-center gap-3">
          <h2 className="flex-1 font-display text-lg font-bold">Inbox</h2>
          {unread > 0 && <Badge tone="accent">{unread} unread</Badge>}
          {unread > 0 && (
            <button
              onClick={markAll}
              className="text-sm font-semibold text-accent"
            >
              Mark all read
            </button>
          )}
        </div>
        <div className="space-y-2">
          {inbox?.map((n) => (
            <div
              key={n.id}
              className={`flex flex-wrap items-center gap-3 rounded-lg border border-line px-4 py-2 ${
                n.read_at ? "opacity-60" : ""
              }`}
            >
              <div className="flex-1">
                <span className="font-semibold">
                  {n.subject ?? n.template_key}
                </span>
                {n.body && <p className="text-sm text-ink-3">{n.body}</p>}
              </div>
              <span className="text-xs text-ink-3">
                {timeAgo(n.created_at)}
              </span>
              {!n.read_at && (
                <button
                  onClick={() => markRead(n.id)}
                  className="text-sm font-semibold text-accent"
                >
                  Mark read
                </button>
              )}
            </div>
          ))}
          {inbox?.length === 0 && (
            <p className="text-sm text-ink-3">
              Nothing yet — you&apos;ll see new applications and other workspace
              events here.
            </p>
          )}
        </div>
      </Card>

      <Card className="p-5">
        <h2 className="mb-1 font-display text-lg font-bold">Browser push</h2>
        <p className="mb-3 text-sm text-ink-3">
          Get workspace events as system notifications on this device, even with
          the console closed. Standard Web Push — nothing to install.
        </p>
        {pushOk ? (
          <div className="flex flex-wrap items-center gap-3">
            <button
              onClick={togglePush}
              disabled={pushBusy}
              className="rounded-lg bg-accent px-4 py-2 font-semibold text-white disabled:opacity-50"
            >
              {pushOn
                ? "Disable push on this device"
                : "Enable push on this device"}
            </button>
            {pushOn && (
              <button
                onClick={sendTestPush}
                className="text-sm font-semibold text-accent"
              >
                Send a test push
              </button>
            )}
            <Badge tone={pushOn ? "good" : "neutral"}>
              {pushOn ? "enabled" : "off"}
            </Badge>
          </div>
        ) : (
          <p className="text-sm text-ink-3">
            This browser doesn&apos;t support Web Push.
          </p>
        )}
      </Card>

      {manage && (
        <Card className="p-5">
          <h2 className="mb-1 font-display text-lg font-bold">
            Delivery providers
          </h2>
          <p className="mb-3 text-sm text-ink-3">
            Connect your own services for outbound notifications. Credentials
            are stored encrypted in the vault and shown only as their last four
            characters. Without a provider, sends are simulated.
          </p>

          <form onSubmit={addProvider} className="mb-4 space-y-3">
            <div className="flex flex-wrap items-end gap-3">
              <label className="text-sm">
                <span className="mb-1 block text-ink-3">Channel</span>
                <select
                  value={channel}
                  onChange={(e) => {
                    const c = PROVIDER_CATALOG.find(
                      (x) => x.channel === e.target.value
                    )!;
                    setChannel(c.channel);
                    setKind(c.kinds[0].kind);
                    setConfig({});
                  }}
                  className="rounded-lg border border-line bg-surface px-3 py-2"
                >
                  {PROVIDER_CATALOG.map((c) => (
                    <option key={c.channel} value={c.channel}>
                      {c.label}
                    </option>
                  ))}
                </select>
              </label>
              <label className="text-sm">
                <span className="mb-1 block text-ink-3">Service</span>
                <select
                  value={catalogKind.kind}
                  onChange={(e) => {
                    setKind(e.target.value);
                    setConfig({});
                  }}
                  className="rounded-lg border border-line bg-surface px-3 py-2"
                >
                  {catalogChannel.kinds.map((k) => (
                    <option key={k.kind} value={k.kind}>
                      {k.label}
                    </option>
                  ))}
                </select>
              </label>
              {catalogKind.fields.map((f) => (
                <label key={f.key} className="text-sm">
                  <span className="mb-1 block text-ink-3">{f.label}</span>
                  <input
                    value={config[f.key] ?? ""}
                    onChange={(e) =>
                      setConfig((c) => ({ ...c, [f.key]: e.target.value }))
                    }
                    placeholder={f.placeholder}
                    className="w-52 rounded-lg border border-line bg-surface px-3 py-2 font-mono text-xs"
                  />
                </label>
              ))}
              <label className="flex-1 min-w-[220px] text-sm">
                <span className="mb-1 block text-ink-3">
                  {catalogKind.credentialLabel}
                </span>
                <input
                  value={credential}
                  onChange={(e) => setCredential(e.target.value)}
                  type="password"
                  className="w-full rounded-lg border border-line bg-surface px-3 py-2 font-mono text-xs"
                />
              </label>
              <button
                type="submit"
                disabled={saving}
                className="rounded-lg bg-accent px-4 py-2 font-semibold text-white disabled:opacity-50"
              >
                Add provider
              </button>
            </div>
          </form>

          <div className="space-y-2">
            {providers?.map((p) => (
              <div
                key={p.id}
                className="flex flex-wrap items-center gap-3 rounded-lg border border-line px-4 py-2"
              >
                <Badge tone="neutral">{p.channel}</Badge>
                <span className="font-semibold capitalize">{p.kind}</span>
                {p.credential_last4 && (
                  <span className="font-mono text-xs text-ink-3">
                    ••••{p.credential_last4}
                  </span>
                )}
                {p.is_default && <Badge tone="accent">default</Badge>}
                <Badge tone={p.enabled ? "good" : "neutral"}>
                  {p.enabled ? "enabled" : "disabled"}
                </Badge>
                <div className="ml-auto flex items-center gap-3">
                  <button
                    onClick={() => testProvider(p)}
                    className="text-sm font-semibold text-accent"
                  >
                    Test
                  </button>
                  {!p.is_default && (
                    <button
                      onClick={() => setDefault(p)}
                      className="text-sm text-ink-3"
                    >
                      Make default
                    </button>
                  )}
                  <button
                    onClick={() => toggleEnabled(p)}
                    className="text-sm text-ink-3"
                  >
                    {p.enabled ? "Disable" : "Enable"}
                  </button>
                  <button
                    onClick={() => removeProvider(p)}
                    className="text-sm text-ink-3"
                  >
                    Remove
                  </button>
                </div>
              </div>
            ))}
            {providers?.length === 0 && (
              <p className="text-sm text-ink-3">
                No providers yet — email and SMS sends are simulated until you
                connect one.
              </p>
            )}
          </div>
        </Card>
      )}

      {manage && <TemplatesCard />}
    </div>
  );
}
