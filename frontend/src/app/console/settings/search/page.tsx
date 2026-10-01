"use client";

// How the public site looks in search results: the home page's title and
// description, a live preview, and Search Console verification.

import { useEffect, useState } from "react";
import Link from "next/link";
import { toast } from "sonner";
import { business, type Business } from "@/lib/business";
import { useAuth } from "@/lib/auth";
import { Button, Card } from "@/components/ui";

const field =
  "w-full rounded-xl border border-line bg-surface px-3 py-2 text-sm text-ink";

export default function SearchSettingsPage() {
  const { can } = useAuth();
  const allowed = can("integrations:manage");
  const [biz, setBiz] = useState<Business | null>(null);
  const [title, setTitle] = useState("");
  const [desc, setDesc] = useState("");
  const [token, setToken] = useState("");
  const [host, setHost] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    setHost(window.location.host);
    if (!allowed) return;
    business
      .get()
      .then((b) => {
        setBiz(b);
        setTitle(b.seo_title ?? "");
        setDesc(b.seo_description ?? "");
        setToken(b.google_site_verification ?? "");
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
      setBiz(
        await business.save({
          seo_title: title,
          seo_description: desc,
          google_site_verification: token,
        })
      );
      toast.success("Saved. Search engines pick it up within minutes.");
    } catch (e) {
      toast.error((e as Error).message);
    } finally {
      setBusy(false);
    }
  };

  const shownTitle =
    title.trim() || `Homes for rent | ${biz.business_name ?? "Your business"}`;
  const shownDesc =
    desc.trim() ||
    `Browse our verified rental homes from ${biz.business_name ?? "your business"}. Apply once, tour online, and move in faster.`;

  return (
    <div className="max-w-2xl space-y-5">
      <div>
        <Link href="/console/settings" className="text-xs text-ink-3">
          ← Settings
        </Link>
        <h1 className="font-display text-2xl font-bold">Search appearance</h1>
        <p className="text-sm text-ink-3">
          How your website reads in Google and in link previews. Every home page
          already gets its own title, description, preview image and structured
          data from its details.
        </p>
      </div>

      <Card className="space-y-3 p-5">
        <h2 className="font-display text-lg font-bold">Preview</h2>
        <div className="rounded-xl bg-surface-2 p-4">
          <div className="text-xs text-ink-3">{host}</div>
          <div className="text-lg text-info">
            {shownTitle.slice(0, 60)}
            {shownTitle.length > 60 ? "…" : ""}
          </div>
          <div className="text-sm text-ink-2">
            {shownDesc.slice(0, 160)}
            {shownDesc.length > 160 ? "…" : ""}
          </div>
        </div>
      </Card>

      <Card className="space-y-4 p-5">
        <label className="block text-xs font-semibold text-ink-3">
          Home page title ({title.length}/70, about 60 shows fully)
          <input
            className={`${field} mt-1`}
            maxLength={70}
            value={title}
            placeholder={`Homes for rent | ${biz.business_name ?? ""}`}
            onChange={(e) => setTitle(e.target.value)}
          />
        </label>
        <label className="block text-xs font-semibold text-ink-3">
          Home page description ({desc.length}/200, about 155 shows fully)
          <textarea
            className={`${field} mt-1`}
            rows={3}
            maxLength={200}
            value={desc}
            onChange={(e) => setDesc(e.target.value)}
          />
        </label>
        <label className="block text-xs font-semibold text-ink-3">
          Google Search Console verification token
          <input
            className={`${field} mt-1 font-mono`}
            value={token}
            placeholder="the content value of Google's meta tag"
            onChange={(e) => setToken(e.target.value)}
          />
        </label>
        <p className="text-xs text-ink-3">
          In Search Console choose HTML tag as the verification method and paste
          only the <span className="font-mono">content</span> value. Then submit{" "}
          <span className="font-mono">{host}/sitemap.xml</span>.
        </p>
        <Button onClick={save} disabled={busy}>
          Save
        </Button>
      </Card>

      <Card className="space-y-1 p-5 text-sm text-ink-2">
        <h2 className="font-display text-lg font-bold">What is already on</h2>
        <ul className="list-disc space-y-1 pl-5">
          <li>
            Each home&apos;s page is server-rendered with its own title,
            description, canonical link and share image.
          </li>
          <li>
            Structured data for the business and for each home as a rental
            offer.
          </li>
          <li>
            <span className="font-mono">/sitemap.xml</span> and{" "}
            <span className="font-mono">/robots.txt</span>, kept current as
            homes are published and leased.
          </li>
          <li>
            The console, sign-in and widget pages are kept out of search
            results.
          </li>
          <li>
            Business details and social profiles come from Business profile.
          </li>
        </ul>
      </Card>
    </div>
  );
}
