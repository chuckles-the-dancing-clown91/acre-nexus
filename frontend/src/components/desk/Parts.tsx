"use client";

// Parts on a work order, each with where to buy it: its own product link
// when someone saved one, otherwise a search at the usual stores.

import { useState } from "react";
import { ExternalLink, Link2, Package } from "lucide-react";
import { toast } from "sonner";
import { desk, dollars, looksLikeUrl, partLinks } from "@/lib/servicedesk";
import type { TicketPart } from "@/lib/types";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

const field =
  "min-w-0 flex-1 rounded-lg border border-line bg-surface px-2.5 py-1.5 text-xs text-fg outline-none focus:border-accent";

export function Parts({
  parts,
  manage,
  onChange,
}: {
  parts: TicketPart[];
  manage: boolean;
  onChange: () => void;
}) {
  const [editing, setEditing] = useState<string | null>(null);
  const [url, setUrl] = useState("");
  const [busy, setBusy] = useState(false);

  async function save(p: TicketPart) {
    if (!looksLikeUrl(url)) {
      toast.error("The link must be a web address (https://…).");
      return;
    }
    setBusy(true);
    try {
      await desk.updatePart(p.id, { url: url.trim() });
      setEditing(null);
      onChange();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't save the link");
    } finally {
      setBusy(false);
    }
  }

  return (
    <Panel>
      <PanelHeader
        title="Parts"
        description="From stock, to buy, or maybe. Tap a store to buy it."
      />
      <div className="p-2 pt-3">
        {parts.length === 0 && (
          <EmptyState
            icon={<Package />}
            title="No parts listed"
            className="py-6"
          />
        )}
        <ul className="divide-y divide-line">
          {parts.map((p) => {
            const buy = p.status !== "used" && p.status !== "from_stock";
            return (
              <li key={p.id} className="px-3 py-2">
                <div className="flex items-center gap-3">
                  <span className="figure w-8 text-right text-[13px] text-fg-3">
                    {p.quantity}×
                  </span>
                  <span className="min-w-0 flex-1 truncate text-[13px] text-fg">
                    {p.name}
                  </span>
                  {p.unit_cost_cents != null && (
                    <span className="figure text-xs text-fg-3">
                      {dollars(p.unit_cost_cents * p.quantity)}
                    </span>
                  )}
                  <Badge
                    tone={
                      p.status === "to_order"
                        ? "warn"
                        : p.status === "used" || p.status === "from_stock"
                          ? "good"
                          : "neutral"
                    }
                  >
                    {p.status.replace("_", " ")}
                  </Badge>
                </div>
                {buy && (
                  <div className="mt-1.5 flex flex-wrap items-center gap-x-3 gap-y-1 pl-11 text-xs">
                    {partLinks(p).map((l) => (
                      <a
                        key={l.href}
                        href={l.href}
                        target="_blank"
                        rel="noopener noreferrer"
                        className={cn(
                          "inline-flex items-center gap-1 hover:underline",
                          p.url ? "font-medium text-accent" : "text-fg-3"
                        )}
                      >
                        {l.label}
                        <ExternalLink className="size-3" />
                      </a>
                    ))}
                    {manage && editing !== p.id && (
                      <button
                        type="button"
                        className="inline-flex items-center gap-1 text-fg-3 hover:text-fg"
                        onClick={() => {
                          setEditing(p.id);
                          setUrl(p.url ?? "");
                        }}
                      >
                        <Link2 className="size-3" />
                        {p.url ? "Change link" : "Add link"}
                      </button>
                    )}
                  </div>
                )}
                {editing === p.id && (
                  <div className="mt-2 flex gap-1.5 pl-11">
                    <input
                      className={cn(field, !looksLikeUrl(url) && "border-bad")}
                      inputMode="url"
                      autoFocus
                      placeholder="https://www.homedepot.com/p/…"
                      aria-label={`${p.name} product link`}
                      value={url}
                      onChange={(e) => setUrl(e.target.value)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") void save(p);
                        if (e.key === "Escape") setEditing(null);
                      }}
                    />
                    <Button size="sm" onClick={() => save(p)} disabled={busy}>
                      Save
                    </Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      onClick={() => setEditing(null)}
                    >
                      Cancel
                    </Button>
                  </div>
                )}
              </li>
            );
          })}
        </ul>
      </div>
    </Panel>
  );
}
