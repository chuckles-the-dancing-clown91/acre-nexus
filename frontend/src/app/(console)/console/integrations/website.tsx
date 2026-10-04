"use client";

// Your own website: widgets that show your homes, tour form, reviews and site
// maps with two lines of HTML, and how the Vantedge-hosted site reads in
// search results.

import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Check, ExternalLink } from "lucide-react";
import type { Business } from "@/lib/business";
import { sso } from "@/lib/sso";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Field, Input, Label } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { cn } from "@/lib/utils";
import { fieldClass } from "@/components/ui/input";
import { CopyButton, Switch, useOrigin } from "./bits";
import { useSaveBusiness, WithBusiness } from "./business";

const WIDGETS: { key: string; label: string; note: string; extra?: string }[] =
  [
    {
      key: "listings",
      label: "Listings",
      note: "Search, filters and a grid of your available homes.",
    },
    {
      key: "tour",
      label: "Tour request form",
      note: "Requests land in Tours. Add data-listing to tie it to one home.",
    },
    {
      key: "reviews",
      label: "Google reviews",
      note: "Your rating and the reviews you choose to show.",
    },
    {
      key: "map",
      label: "Site map",
      note: "A published map. Replace MAP_ID with the id from the map's address.",
      extra: ' data-map="MAP_ID"',
    },
  ];

export function WidgetsTab() {
  return (
    <WithBusiness>
      {(b) => (
        <div className="space-y-6">
          <EmbedAccess key={b.updated_at ?? "new"} biz={b} />
          <Snippets />
        </div>
      )}
    </WithBusiness>
  );
}

function EmbedAccess({ biz }: { biz: Business }) {
  const save = useSaveBusiness();
  const [on, setOn] = useState(biz.embed_enabled);
  const [origins, setOrigins] = useState(biz.embed_origins ?? "");
  const bad = origins
    .split(/\s+/)
    .filter(Boolean)
    .filter((o) => !/^https?:\/\/[^/\s]+$/i.test(o));

  return (
    <Panel>
      <PanelHeader
        title="Who can show them"
        description="Everything in a widget is already public. The site list stops other sites showing your widgets as theirs; it isn't a secret."
        action={
          <Badge tone={biz.embed_enabled ? "good" : "neutral"}>
            {biz.embed_enabled ? "On" : "Off"}
          </Badge>
        }
      />
      <form
        className="space-y-5 p-5"
        onSubmit={(e) => {
          e.preventDefault();
          save.mutate({
            body: { embed_enabled: on, embed_origins: origins },
            ok: "Widget settings saved",
          });
        }}
      >
        <div className="flex items-center justify-between gap-4">
          <div className="text-[13px] font-medium text-fg">
            Allow widgets on websites
          </div>
          <Switch
            checked={on}
            onChange={setOn}
            label="Allow widgets on websites"
          />
        </div>
        <div className="space-y-1.5">
          <Label htmlFor="embed-origins">Allowed sites</Label>
          <textarea
            id="embed-origins"
            className={cn(fieldClass, "block w-full py-2.5 font-mono text-xs")}
            rows={3}
            placeholder="https://www.yourcompany.com"
            value={origins}
            onChange={(e) => setOrigins(e.target.value)}
          />
          <p className={cn("text-xs", bad.length ? "text-warn" : "text-fg-3")}>
            {bad.length
              ? `Use the site's address only, like https://www.yourcompany.com. Check: ${bad.join(", ")}`
              : "One per line. Leave empty to allow any site."}
          </p>
        </div>
        <div className="flex justify-end">
          <Button type="submit" loading={save.isPending}>
            {!save.isPending && <Check />}
            Save
          </Button>
        </div>
      </form>
    </Panel>
  );
}

function Snippets() {
  const host = useOrigin();
  const status = useQuery({ queryKey: ["sso-alpha"], queryFn: sso.status });
  const slug = status.data?.tenant ?? "";
  if (status.isLoading || !host)
    return <Skeleton className="h-48 rounded-2xl" />;
  if (status.error)
    return (
      <Panel className="border-bad/30 p-4 text-[13px] text-bad">
        Couldn&apos;t load the workspace address: {status.error.message}
      </Panel>
    );

  const snippet = (key: string, extra = "") =>
    `<div data-vantedge="${key}" data-tenant="${slug}"${extra}></div>\n<script async src="${host}/embed.js"></script>`;

  return (
    <>
      <div className="grid gap-4 lg:grid-cols-2">
        {WIDGETS.map((w) => (
          <Panel key={w.key} className="flex flex-col">
            <PanelHeader
              title={w.label}
              description={w.note}
              action={
                <CopyButton value={snippet(w.key, w.extra)} label="Copy code" />
              }
            />
            <pre className="mx-5 mt-3 overflow-x-auto rounded-xl border border-line bg-fill/60 p-3 font-mono text-[11px] leading-relaxed text-fg-2">
              {snippet(w.key, w.extra)}
            </pre>
            <div className="mt-auto px-5 py-4">
              <Button asChild size="sm" variant="ghost">
                <a
                  href={`${host}/embed/${w.key}?tenant=${encodeURIComponent(slug)}`}
                  target="_blank"
                  rel="noopener noreferrer"
                >
                  <ExternalLink />
                  Preview
                </a>
              </Button>
            </div>
          </Panel>
        ))}
      </div>
      <Panel className="p-5 text-[13px] text-fg-2">
        <div className="text-[15px] font-semibold text-fg">
          Matching your site
        </div>
        <p className="mt-1">
          Widgets use your brand colour. To match your site, add{" "}
          <code className="font-mono text-xs text-fg">
            data-accent=&quot;#0e7c86&quot;
          </code>{" "}
          or{" "}
          <code className="font-mono text-xs text-fg">
            data-mode=&quot;dark&quot;
          </code>{" "}
          to the widget&apos;s div. Each widget is a frame from Vantedge, so it
          can&apos;t touch your page.
        </p>
      </Panel>
    </>
  );
}

// ---- Search appearance ----

export function SearchTab() {
  return (
    <WithBusiness>
      {(b) => <SearchForm key={b.updated_at ?? "new"} biz={b} />}
    </WithBusiness>
  );
}

function SearchForm({ biz }: { biz: Business }) {
  const save = useSaveBusiness();
  const origin = useOrigin();
  const host = origin.replace(/^https?:\/\//, "");
  const [title, setTitle] = useState(biz.seo_title ?? "");
  const [desc, setDesc] = useState(biz.seo_description ?? "");
  const [token, setToken] = useState(biz.google_site_verification ?? "");
  const name = biz.business_name ?? "Your business";
  const shownTitle = title.trim() || `Homes for rent | ${name}`;
  const shownDesc =
    desc.trim() ||
    `Browse our verified rental homes from ${name}. Apply once, tour online, and move in faster.`;

  return (
    <div className="grid gap-6 lg:grid-cols-[minmax(0,1fr)_360px]">
      <Panel>
        <PanelHeader
          title="Search appearance"
          description="How your home page reads in Google and link previews. Each home's page already gets its own."
        />
        <form
          className="space-y-5 p-5"
          onSubmit={(e) => {
            e.preventDefault();
            save.mutate({
              body: {
                seo_title: title,
                seo_description: desc,
                google_site_verification: token.trim(),
              },
              ok: "Saved. Search engines pick it up within minutes.",
            });
          }}
        >
          <Field
            label="Home page title"
            hint={`${title.length}/70. About 60 characters show in full.`}
          >
            {(p) => (
              <Input
                {...p}
                maxLength={70}
                placeholder={`Homes for rent | ${name}`}
                value={title}
                onChange={(e) => setTitle(e.target.value)}
              />
            )}
          </Field>
          <div className="space-y-1.5">
            <Label htmlFor="seo-desc">Home page description</Label>
            <textarea
              id="seo-desc"
              className={cn(fieldClass, "block w-full py-2.5 text-sm")}
              rows={3}
              maxLength={200}
              value={desc}
              onChange={(e) => setDesc(e.target.value)}
            />
            <p className="text-xs text-fg-3">
              {desc.length}/200. About 155 characters show in full.
            </p>
          </div>
          <Field
            label="Google Search Console verification"
            hint="Choose HTML tag in Search Console and paste only its content value."
          >
            {(p) => (
              <Input
                {...p}
                className="font-mono"
                placeholder="The content value of Google's meta tag"
                value={token}
                onChange={(e) => setToken(e.target.value)}
              />
            )}
          </Field>
          {host && (
            <p className="text-[13px] text-fg-3">
              Then submit{" "}
              <code className="font-mono text-xs text-fg">
                {host}/sitemap.xml
              </code>{" "}
              in Search Console.
            </p>
          )}
          <div className="flex justify-end">
            <Button type="submit" loading={save.isPending}>
              {!save.isPending && <Check />}
              Save
            </Button>
          </div>
        </form>
      </Panel>
      <div className="space-y-4">
        <Panel className="p-5">
          <div className="eyebrow">Preview</div>
          <div className="mt-3 rounded-xl border border-line bg-fill/50 p-4">
            <div className="text-xs text-fg-3">{host || "your site"}</div>
            <div className="mt-0.5 text-[16px] text-info">
              {shownTitle.length > 60
                ? `${shownTitle.slice(0, 60)}…`
                : shownTitle}
            </div>
            <div className="mt-0.5 text-[13px] text-fg-2">
              {shownDesc.length > 160
                ? `${shownDesc.slice(0, 160)}…`
                : shownDesc}
            </div>
          </div>
        </Panel>
        <Panel className="p-5 text-[13px] text-fg-2">
          <div className="text-[15px] font-semibold text-fg">Already on</div>
          <ul className="mt-2 list-disc space-y-1 pl-5">
            <li>
              Each home&apos;s page has its own title, description, canonical
              link and share image.
            </li>
            <li>Structured data for your business and each home for rent.</li>
            <li>
              <code className="font-mono text-xs">/sitemap.xml</code> and{" "}
              <code className="font-mono text-xs">/robots.txt</code>, kept
              current as homes are published and leased.
            </li>
            <li>The console, sign-in and widget pages stay out of search.</li>
            <li>Business details and social links come from the profile.</li>
          </ul>
        </Panel>
      </div>
    </div>
  );
}
