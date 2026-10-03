"use client";

// Listings: every home on the market, whether it's on the website and the
// rental portals, and what's keeping any of them off. The portals each read
// a feed from their own secret URL.

import { useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { motion } from "motion/react";
import { CheckCircle2, ImageOff, Megaphone, Search } from "lucide-react";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { syndication, type ListingSyndication } from "@/lib/syndication";
import { ChannelCard } from "@/components/listings/ChannelCard";
import { ListingEditor } from "@/components/listings/ListingEditor";
import { Badge, statusTone } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

type View = "all" | "ready" | "work" | "off";

const VIEWS: { key: View; label: string }[] = [
  { key: "all", label: "All" },
  { key: "ready", label: "On the portals" },
  { key: "work", label: "Needs work" },
  { key: "off", label: "Not advertised" },
];

/** Leased, pending, unpublished or opted out: nothing to fix. */
function off(l: ListingSyndication): boolean {
  return (
    !l.is_public ||
    !l.syndicate ||
    l.status === "Leased" ||
    l.status === "Pending"
  );
}

export default function ListingsPage() {
  const { can } = useAuth();
  const manage = can("listing:write");
  const qc = useQueryClient();
  const data = useQuery({
    queryKey: ["syndication"],
    queryFn: syndication.overview,
  });
  const listings = useQuery({
    queryKey: ["console-listings"],
    queryFn: () => api.consoleListings(),
  });
  const [view, setView] = useState<View>("all");
  const [q, setQ] = useState("");
  const [editing, setEditing] = useState<string | null>(null);

  const refresh = () => {
    void qc.invalidateQueries({ queryKey: ["syndication"] });
    void qc.invalidateQueries({ queryKey: ["console-listings"] });
  };
  const all = useMemo(() => data.data?.listings ?? [], [data.data]);
  const counts = useMemo(
    () => ({
      ready: all.filter((l) => l.ready).length,
      work: all.filter((l) => !l.ready && !off(l)).length,
      off: all.filter(off).length,
    }),
    [all]
  );
  const rows = useMemo(() => {
    const needle = q.trim().toLowerCase();
    return all
      .filter((l) =>
        view === "all"
          ? true
          : view === "ready"
            ? l.ready
            : view === "work"
              ? !l.ready && !off(l)
              : off(l)
      )
      .filter(
        (l) =>
          !needle ||
          l.title.toLowerCase().includes(needle) ||
          l.address.toLowerCase().includes(needle)
      );
  }, [all, view, q]);
  const editingListing = listings.data?.find((l) => l.id === editing);
  const editingIssues =
    all.find((l) => l.id === editing)?.issues.map((i) => i.message) ?? [];

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Leasing"
        title="Listings"
        description="Your homes on the market, on your website and the rental portals."
      />

      <section className="grid gap-4 lg:grid-cols-2">
        {data.isLoading &&
          [0, 1].map((i) => <Skeleton key={i} className="h-80 rounded-2xl" />)}
        {data.data?.channels.map((c) => (
          <ChannelCard
            key={c.key}
            channel={c}
            manage={manage}
            onChange={refresh}
          />
        ))}
      </section>

      <Panel className="overflow-hidden">
        <PanelHeader
          title="Every listing"
          description={`${counts.ready} on the portals · ${counts.work} need work · ${counts.off} not advertised`}
        />
        <div className="flex flex-col gap-3 border-b border-line p-3 pt-4 sm:flex-row sm:items-center sm:justify-between">
          <div
            className="flex flex-wrap gap-1 rounded-xl bg-fill p-1"
            role="tablist"
          >
            {VIEWS.map((v) => (
              <button
                key={v.key}
                role="tab"
                aria-selected={view === v.key}
                onClick={() => setView(v.key)}
                className={cn(
                  "rounded-lg px-3 py-1.5 text-[13px] font-medium transition",
                  view === v.key
                    ? "bg-surface text-fg shadow-sm"
                    : "text-fg-3 hover:text-fg"
                )}
              >
                {v.label}
              </button>
            ))}
          </div>
          <div className="relative sm:w-72">
            <Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-fg-3" />
            <Input
              value={q}
              onChange={(e) => setQ(e.target.value)}
              placeholder="Search listings"
              aria-label="Search listings"
              className="pl-9"
            />
          </div>
        </div>
        {data.isLoading && <Skeleton className="m-3 h-48" />}
        {data.data && rows.length === 0 && (
          <EmptyState
            icon={<Megaphone />}
            title={all.length ? "Nothing here" : "No listings yet"}
            description={
              all.length ? undefined : "List a home from its property page."
            }
          />
        )}
        <ul className="divide-y divide-line">
          {rows.map((l, i) => {
            const blocking = l.issues.filter((x) => x.blocking);
            const advice = l.issues.filter((x) => !x.blocking);
            return (
              <motion.li
                key={l.id}
                initial={{ opacity: 0, y: 6 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: Math.min(i, 15) * 0.02 }}
              >
                <button
                  type="button"
                  disabled={!manage}
                  onClick={() => setEditing(l.id)}
                  className="flex w-full items-start gap-3 px-4 py-3 text-left transition enabled:hover:bg-fill-2"
                >
                  <span
                    className={cn(
                      "mt-0.5 flex size-8 shrink-0 items-center justify-center rounded-lg border",
                      l.ready
                        ? "border-good/30 bg-good/10 text-good"
                        : "border-line bg-fill text-fg-3"
                    )}
                  >
                    {l.ready ? (
                      <CheckCircle2 className="size-4" />
                    ) : l.photos === 0 ? (
                      <ImageOff className="size-4" />
                    ) : (
                      <Megaphone className="size-4" />
                    )}
                  </span>
                  <div className="min-w-0 flex-1">
                    <div className="flex flex-wrap items-center gap-2">
                      <span className="truncate text-[14px] font-medium text-fg">
                        {l.title}
                      </span>
                      <Badge tone={statusTone(l.status)}>{l.status}</Badge>
                    </div>
                    <div className="truncate text-xs text-fg-3">
                      {l.address}, {l.city}
                      {l.state ? `, ${l.state}` : ""} {l.postal_code} ·{" "}
                      {l.photos} photo{l.photos === 1 ? "" : "s"}
                    </div>
                    {l.ready ? (
                      <div className="mt-1 text-xs text-good">
                        On the portals
                        {advice.length > 0 && (
                          <span className="text-fg-3">
                            {" "}
                            · {advice[0].message}
                          </span>
                        )}
                      </div>
                    ) : (
                      <div className="mt-1 text-xs text-warn">
                        {blocking
                          .slice(0, 2)
                          .map((b) => b.message)
                          .join(" · ")}
                        {blocking.length > 2 &&
                          ` · ${blocking.length - 2} more`}
                      </div>
                    )}
                  </div>
                  <span className="figure shrink-0 text-[14px] font-semibold text-fg">
                    {l.rent_label}
                  </span>
                </button>
              </motion.li>
            );
          })}
        </ul>
      </Panel>

      {editingListing && (
        <ListingEditor
          listing={editingListing}
          issues={editingIssues}
          onClose={() => setEditing(null)}
          onPhotos={() =>
            void qc.invalidateQueries({ queryKey: ["syndication"] })
          }
          onSaved={() => {
            setEditing(null);
            refresh();
          }}
        />
      )}
    </div>
  );
}
