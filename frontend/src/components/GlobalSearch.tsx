"use client";

// Global search (roadmap Phase 8, issue #55) — a console-wide command palette.
// Type a property address, tenant name, contractor, ticket, or LLC and jump
// straight to it. ⌘K / Ctrl-K focuses the box; results self-gate by permission
// on the server.

import { useCallback, useEffect, useRef, useState } from "react";
import { useRouter } from "next/navigation";
import { api, type SearchHit } from "@/lib/api";
import { Icon } from "@/components/Icon";
import { logError } from "@/lib/log";

const KIND_ORDER = ["property", "lease", "entity", "ticket", "llc"] as const;
const KIND_LABEL: Record<string, string> = {
  property: "Properties",
  lease: "Tenants",
  entity: "Entities",
  ticket: "Maintenance",
  llc: "Legal entities",
};
const KIND_ICON: Record<string, string> = {
  property: "building",
  lease: "user",
  entity: "bank",
  ticket: "wrench",
  llc: "ledger",
};

export function GlobalSearch() {
  const router = useRouter();
  const inputRef = useRef<HTMLInputElement>(null);
  const [q, setQ] = useState("");
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);

  // Debounced query.
  useEffect(() => {
    const term = q.trim();
    if (term.length < 2) {
      setHits([]);
      setOpen(false);
      return;
    }
    const t = setTimeout(() => {
      api
        .search(term)
        .then((r) => {
          setHits(r.hits);
          setActive(0);
          setOpen(true);
        })
        .catch((e) => logError("search failed", e));
    }, 250);
    return () => clearTimeout(t);
  }, [q]);

  // ⌘K / Ctrl-K to focus.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        inputRef.current?.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const select = useCallback(
    (hit: SearchHit | undefined) => {
      if (!hit) return;
      setOpen(false);
      setQ("");
      setHits([]);
      inputRef.current?.blur();
      router.push(hit.href);
    },
    [router]
  );

  function onKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
    if (!open || hits.length === 0) return;
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setActive((a) => Math.min(a + 1, hits.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActive((a) => Math.max(a - 1, 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      select(hits[active]);
    } else if (e.key === "Escape") {
      setOpen(false);
    }
  }

  // Group hits by kind, preserving the server order within each group.
  const groups = KIND_ORDER.map((kind) => ({
    kind,
    items: hits.filter((h) => h.kind === kind),
  })).filter((g) => g.items.length > 0);

  return (
    <div className="relative hidden w-full max-w-md md:block">
      <div className="flex items-center gap-2 rounded-xl border border-line bg-surface-2 px-3">
        <Icon name="search" size={15} />
        <input
          ref={inputRef}
          value={q}
          onChange={(e) => setQ(e.target.value)}
          onKeyDown={onKeyDown}
          onFocus={() => hits.length > 0 && setOpen(true)}
          onBlur={() => setTimeout(() => setOpen(false), 150)}
          placeholder="Search properties, tenants, entities…  (⌘K)"
          className="w-full bg-transparent py-2 text-sm outline-none placeholder:text-ink-3"
        />
      </div>

      {open && (
        <div className="absolute left-0 right-0 top-11 z-50 max-h-[70vh] overflow-y-auto rounded-xl border border-line bg-surface p-1 shadow-acre">
          {groups.length === 0 ? (
            <div className="px-3 py-4 text-sm text-ink-3">No matches.</div>
          ) : (
            groups.map((g) => (
              <div key={g.kind} className="py-1">
                <div className="px-3 py-1 text-[11px] font-bold uppercase tracking-wide text-ink-3">
                  {KIND_LABEL[g.kind]}
                </div>
                {g.items.map((hit) => {
                  const idx = hits.indexOf(hit);
                  return (
                    <button
                      key={`${hit.kind}-${hit.id}`}
                      onMouseDown={(e) => e.preventDefault()}
                      onClick={() => select(hit)}
                      onMouseEnter={() => setActive(idx)}
                      className={`flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left ${
                        idx === active ? "bg-surface-2" : ""
                      }`}
                    >
                      <Icon name={KIND_ICON[hit.kind] ?? "search"} size={16} />
                      <span className="min-w-0">
                        <span className="block truncate text-sm font-semibold">
                          {hit.title}
                        </span>
                        <span className="block truncate text-xs text-ink-3">
                          {hit.subtitle}
                        </span>
                      </span>
                    </button>
                  );
                })}
              </div>
            ))
          )}
        </div>
      )}
    </div>
  );
}
