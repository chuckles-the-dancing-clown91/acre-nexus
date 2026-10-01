"use client";

// Website widgets: put listings, a tour form, reviews and site maps on the
// client's own website with a few lines of HTML.

import { useEffect, useState } from "react";
import Link from "next/link";
import { toast } from "sonner";
import { business, type Business } from "@/lib/business";
import { sso } from "@/lib/sso";
import { useAuth } from "@/lib/auth";
import { Badge, Button, Card } from "@/components/ui";

const WIDGETS: { key: string; label: string; note: string; extra?: string }[] =
  [
    {
      key: "listings",
      label: "Listings",
      note: "Search, filters and a card grid.",
    },
    {
      key: "tour",
      label: "Tour request form",
      note: "A form that lands in Tours. Add data-listing to tie it to one home.",
    },
    {
      key: "reviews",
      label: "Google reviews",
      note: "Your rating and the reviews you choose to show.",
    },
    {
      key: "map",
      label: "Site map",
      note: "A published map. Replace MAP_ID with the map's id from its address.",
      extra: ' data-map="MAP_ID"',
    },
  ];

export default function WebsiteSettingsPage() {
  const { can } = useAuth();
  const allowed = can("integrations:manage");
  const [biz, setBiz] = useState<Business | null>(null);
  const [slug, setSlug] = useState("");
  const [origins, setOrigins] = useState("");
  const [on, setOn] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [host, setHost] = useState("");

  useEffect(() => {
    setHost(window.location.origin);
    if (!allowed) return;
    Promise.all([business.get(), sso.status()])
      .then(([b, s]) => {
        setBiz(b);
        setSlug(s.tenant);
        setOn(b.embed_enabled);
        setOrigins(b.embed_origins ?? "");
      })
      .catch((e: Error) => setError(e.message));
  }, [allowed]);

  if (!allowed)
    return (
      <Card className="p-6 text-ink-2">
        You need the <span className="font-mono">integrations:manage</span>{" "}
        permission.
      </Card>
    );
  if (error) return <p className="text-sm text-bad">{error}</p>;
  if (!biz) return <p className="text-sm text-ink-3">Loading…</p>;

  const save = async () => {
    setBusy(true);
    try {
      const b = await business.save({
        embed_enabled: on,
        embed_origins: origins,
      });
      setBiz(b);
      setOrigins(b.embed_origins ?? "");
      toast.success("Saved");
    } catch (e) {
      toast.error((e as Error).message);
    } finally {
      setBusy(false);
    }
  };

  const snippet = (key: string, extra = "") =>
    `<div data-vantedge="${key}" data-tenant="${slug}"${extra}></div>\n<script async src="${host}/embed.js"></script>`;

  return (
    <div className="max-w-3xl space-y-5">
      <div>
        <Link href="/console/settings" className="text-xs text-ink-3">
          ← Settings
        </Link>
        <h1 className="font-display text-2xl font-bold">Website widgets</h1>
        <p className="text-sm text-ink-3">
          Show your homes, tour form, reviews and site maps on your own website.
          Each widget is a frame served from Vantedge in your colours, so it
          cannot touch your page.
        </p>
      </div>

      <Card className="space-y-3 p-5">
        <div className="flex items-center gap-2">
          <h2 className="font-display text-lg font-bold">Who can show them</h2>
          <Badge tone={on ? "good" : "neutral"}>{on ? "On" : "Off"}</Badge>
        </div>
        <label className="flex items-center gap-2 text-sm font-semibold">
          <input
            type="checkbox"
            checked={on}
            onChange={(e) => setOn(e.target.checked)}
          />
          Allow widgets on websites
        </label>
        <label className="block text-xs font-semibold text-ink-3">
          Allowed sites, one per line. Leave empty to allow any site.
          <textarea
            className="mt-1 w-full rounded-xl border border-line bg-surface px-3 py-2 text-sm text-ink"
            rows={3}
            placeholder={"https://www.yourcompany.com"}
            value={origins}
            onChange={(e) => setOrigins(e.target.value)}
          />
        </label>
        <p className="text-xs text-ink-3">
          Everything in a widget is already public, so this keeps other sites
          from showing your widgets as theirs. It is not a secret.
        </p>
        <Button onClick={save} disabled={busy}>
          Save
        </Button>
      </Card>

      {WIDGETS.map((w) => (
        <Card key={w.key} className="space-y-2 p-5">
          <div className="flex items-center gap-2">
            <h2 className="font-display text-lg font-bold">{w.label}</h2>
            <Button
              variant="ghost"
              onClick={() => {
                void navigator.clipboard.writeText(snippet(w.key, w.extra));
                toast.success("Copied");
              }}
            >
              Copy code
            </Button>
          </div>
          <p className="text-sm text-ink-3">{w.note}</p>
          <pre className="overflow-x-auto rounded-xl bg-surface-2 p-3 text-xs">
            {snippet(w.key, w.extra)}
          </pre>
          {host && (
            <a
              className="text-xs font-semibold text-accent-2"
              href={`${host}/embed/${w.key}?tenant=${slug}`}
              target="_blank"
              rel="noopener noreferrer"
            >
              Preview
            </a>
          )}
        </Card>
      ))}

      <Card className="space-y-1 p-5 text-sm text-ink-2">
        <h2 className="font-display text-lg font-bold">Matching your site</h2>
        <p>
          Widgets use your Vantedge brand colour. To match your site, add{" "}
          <code className="font-mono text-xs">
            data-accent=&quot;#0e7c86&quot;
          </code>{" "}
          or{" "}
          <code className="font-mono text-xs">data-mode=&quot;dark&quot;</code>{" "}
          to the widget&apos;s div.
        </p>
      </Card>
    </div>
  );
}
