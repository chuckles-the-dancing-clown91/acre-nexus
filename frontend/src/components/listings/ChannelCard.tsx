"use client";

// One rental portal: on or off, the feed URL to give them, who renters
// reach, and when the portal last came for the feed.

import { useState } from "react";
import { Check, Copy, ExternalLink, RefreshCw, Rss } from "lucide-react";
import { toast } from "sonner";
import { since, syndication, type Channel } from "@/lib/syndication";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/menu";
import { Panel } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

const field =
  "w-full rounded-xl border border-line bg-surface px-3 py-2 text-[13px] text-fg outline-none focus:border-accent";

export function ChannelCard({
  channel: c,
  manage,
  onChange,
}: {
  channel: Channel;
  manage: boolean;
  onChange: () => void;
}) {
  const [copied, setCopied] = useState(false);
  const [editing, setEditing] = useState(false);
  const [rotating, setRotating] = useState(false);
  const [busy, setBusy] = useState(false);
  const [contact, setContact] = useState({
    name: c.contact_name ?? "",
    email: c.contact_email ?? "",
    phone: c.contact_phone ?? "",
  });
  const blocking = c.issues.filter((i) => i.blocking);

  async function toggle() {
    if (!c.enabled && blocking.length) {
      setEditing(true);
      return;
    }
    setBusy(true);
    try {
      await syndication.update(c.key, { enabled: !c.enabled });
      toast.success(c.enabled ? `${c.label} feed off` : `${c.label} feed on`);
      onChange();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't change it");
    } finally {
      setBusy(false);
    }
  }

  async function saveContact(enable: boolean) {
    setBusy(true);
    try {
      await syndication.update(c.key, {
        contact_name: contact.name,
        contact_email: contact.email,
        contact_phone: contact.phone,
        ...(enable ? { enabled: true } : {}),
      });
      setEditing(false);
      toast.success(enable ? `${c.label} feed on` : "Contact saved");
      onChange();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't save it");
    } finally {
      setBusy(false);
    }
  }

  async function copy() {
    await navigator.clipboard.writeText(c.feed_url).catch(() => undefined);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  }

  async function preview() {
    try {
      const xml = await syndication.preview(c.key);
      const url = URL.createObjectURL(new Blob([xml], { type: "text/xml" }));
      window.open(url, "_blank", "noopener");
      setTimeout(() => URL.revokeObjectURL(url), 60_000);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't build the feed");
    }
  }

  async function rotate() {
    setBusy(true);
    try {
      await syndication.rotate(c.key);
      setRotating(false);
      toast.success(
        "New feed URL. Send it to the portal; the old one has stopped working."
      );
      onChange();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't change it");
    } finally {
      setBusy(false);
    }
  }

  return (
    <Panel className="flex min-w-0 flex-col p-5">
      <div className="flex items-start gap-3">
        <span
          className={cn(
            "flex size-10 shrink-0 items-center justify-center rounded-xl border",
            c.enabled
              ? "border-good/30 bg-good/10 text-good"
              : "border-line bg-fill text-fg-3"
          )}
        >
          <Rss className="size-[18px]" />
        </span>
        <div className="min-w-0 flex-1">
          <div className="text-[15px] font-semibold text-fg">{c.label}</div>
          <div className="text-xs text-fg-3">{c.reaches}</div>
        </div>
        {manage ? (
          <button
            type="button"
            role="switch"
            aria-checked={c.enabled}
            aria-label={`${c.label} feed`}
            disabled={busy}
            onClick={toggle}
            className={cn(
              "relative h-6 w-11 shrink-0 rounded-full transition",
              c.enabled ? "bg-accent" : "bg-fill-2"
            )}
          >
            <span
              className={cn(
                "absolute top-0.5 size-5 rounded-full bg-surface shadow transition-all",
                c.enabled ? "left-[22px]" : "left-0.5"
              )}
            />
          </button>
        ) : (
          <Badge tone={c.enabled ? "good" : "neutral"}>
            {c.enabled ? "on" : "off"}
          </Badge>
        )}
      </div>

      <div className="mt-4 grid grid-cols-2 gap-2 text-center">
        <Stat
          label="In the feed"
          value={`${c.listings} listing${c.listings === 1 ? "" : "s"}`}
        />
        <Stat
          label="Last pulled"
          value={c.last_pulled_at ? since(c.last_pulled_at) : "not yet"}
        />
      </div>

      <div className="mt-4">
        <div className="eyebrow mb-1.5">Feed URL</div>
        <div className="flex gap-1.5">
          <code
            className={cn(
              "min-w-0 flex-1 truncate rounded-lg border border-line bg-fill px-2.5 py-2 text-xs",
              c.enabled ? "text-fg-2" : "text-fg-4"
            )}
            title={c.feed_url}
          >
            {c.feed_url}
          </code>
          <Button
            size="sm"
            variant="secondary"
            onClick={copy}
            aria-label="Copy feed URL"
          >
            {copied ? <Check /> : <Copy />}
          </Button>
        </div>
        <p className="mt-2 text-xs text-fg-3">
          {c.format}. {c.how_to}
        </p>
        {c.last_pull_agent && (
          <p className="mt-1 text-xs text-fg-4">
            Last read by {c.last_pull_agent} · {c.pull_count} pulls in all
          </p>
        )}
      </div>

      <div className="mt-4 rounded-xl border border-line bg-fill/40 px-3 py-2.5 text-[13px]">
        <div className="flex items-center justify-between gap-2">
          <span className="text-fg-3">Renters reach</span>
          {manage && (
            <button
              type="button"
              onClick={() => setEditing(true)}
              className="text-xs font-medium text-accent hover:underline"
            >
              Change
            </button>
          )}
        </div>
        <div className="mt-1 text-fg">
          {c.shown_name}
          {c.shown_email && ` · ${c.shown_email}`}
          {c.shown_phone && ` · ${c.shown_phone}`}
        </div>
        {c.issues.map((i) => (
          <div
            key={i.message}
            className={cn(
              "mt-1 text-xs",
              i.blocking ? "text-warn" : "text-fg-3"
            )}
          >
            {i.message}
          </div>
        ))}
      </div>

      <div className="mt-4 flex flex-wrap gap-2">
        <Button size="sm" variant="ghost" onClick={preview}>
          <ExternalLink />
          See the feed
        </Button>
        {manage && (
          <Button size="sm" variant="ghost" onClick={() => setRotating(true)}>
            <RefreshCw />
            New URL
          </Button>
        )}
      </div>

      <Dialog open={editing} onOpenChange={setEditing}>
        <DialogContent>
          <DialogTitle className="text-[17px] font-semibold">
            Who renters reach on {c.label}
          </DialogTitle>
          <DialogDescription className="mt-1 text-[13px] text-fg-3">
            Shown on every listing. Leave blank to use your business profile.
          </DialogDescription>
          <div className="mt-4 space-y-3">
            <input
              className={field}
              placeholder="Leasing office"
              aria-label="Contact name"
              value={contact.name}
              onChange={(e) => setContact({ ...contact, name: e.target.value })}
            />
            <input
              className={field}
              type="email"
              placeholder="leasing@yourcompany.com"
              aria-label="Contact email"
              value={contact.email}
              onChange={(e) =>
                setContact({ ...contact, email: e.target.value })
              }
            />
            <input
              className={field}
              placeholder="(555) 555-0100"
              aria-label="Contact phone"
              value={contact.phone}
              onChange={(e) =>
                setContact({ ...contact, phone: e.target.value })
              }
            />
          </div>
          <div className="mt-5 flex justify-end gap-2">
            <Button variant="ghost" onClick={() => setEditing(false)}>
              Cancel
            </Button>
            {c.enabled ? (
              <Button onClick={() => saveContact(false)} disabled={busy}>
                Save
              </Button>
            ) : (
              <>
                <Button
                  variant="secondary"
                  onClick={() => saveContact(false)}
                  disabled={busy}
                >
                  Save
                </Button>
                <Button onClick={() => saveContact(true)} disabled={busy}>
                  Save and turn on
                </Button>
              </>
            )}
          </div>
        </DialogContent>
      </Dialog>

      <Dialog open={rotating} onOpenChange={setRotating}>
        <DialogContent>
          <DialogTitle className="text-[17px] font-semibold">
            Give {c.label} a new URL?
          </DialogTitle>
          <DialogDescription className="mt-2 text-[13px] text-fg-3">
            The current URL stops working right away, so your listings drop off
            until the portal has the new one. Do this if the URL got out.
          </DialogDescription>
          <div className="mt-5 flex justify-end gap-2">
            <Button variant="ghost" onClick={() => setRotating(false)}>
              Keep this one
            </Button>
            <Button variant="danger" onClick={rotate} disabled={busy}>
              New URL
            </Button>
          </div>
        </DialogContent>
      </Dialog>
    </Panel>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-xl border border-line bg-fill/50 px-2 py-2">
      <div className="text-[11px] text-fg-3">{label}</div>
      <div className="figure text-[14px] font-medium text-fg">{value}</div>
    </div>
  );
}
