"use client";

// Address suggestions as you type: your own properties first, then the map
// (Photon, or Google Places when a key is set up). Picking one fills street,
// city, state and ZIP. Debounced, with a small in-memory cache per query.

import { useEffect, useRef, useState } from "react";
import { request } from "@/lib/api";

export interface Place {
  label: string;
  address: string;
  city: string;
  state: string;
  postal_code: string;
  latitude: number | null;
  longitude: number | null;
  precision: "address" | "street";
  source: "known" | "photon" | "google";
  property_id: string | null;
}

const cache = new Map<string, Place[]>();

export function suggestAddresses(q: string, limit = 6) {
  return request<Place[]>(
    `/geo/suggest?q=${encodeURIComponent(q)}&limit=${limit}`,
    { auth: true }
  );
}

export function AddressAutocomplete({
  value,
  onChange,
  onPick,
  label = "Address",
  required,
  placeholder = "Start typing the street address…",
  inputClassName,
}: {
  value: string;
  onChange: (v: string) => void;
  onPick: (place: Place) => void;
  label?: string;
  required?: boolean;
  placeholder?: string;
  inputClassName?: string;
}) {
  const [open, setOpen] = useState(false);
  const [places, setPlaces] = useState<Place[]>([]);
  const [active, setActive] = useState(-1);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const picked = useRef<string | null>(null);

  useEffect(() => {
    const q = value.trim();
    if (timer.current) clearTimeout(timer.current);
    if (q.length < 3 || picked.current === q) {
      setPlaces([]);
      return;
    }
    // Cached queries answer on the next tick too, so state changes stay in
    // callbacks rather than the effect body.
    const hit = cache.get(q);
    timer.current = setTimeout(
      () => {
        if (hit) {
          setPlaces(hit);
          setOpen(hit.length > 0);
          setActive(-1);
          return;
        }
        suggestAddresses(q)
          .then((res) => {
            cache.set(q, res);
            if (cache.size > 80) {
              const first = cache.keys().next().value;
              if (first !== undefined) cache.delete(first);
            }
            setPlaces(res);
            setOpen(res.length > 0);
            setActive(-1);
          })
          .catch(() => setPlaces([]));
      },
      hit ? 0 : 250
    );
    return () => {
      if (timer.current) clearTimeout(timer.current);
    };
  }, [value]);

  function pick(p: Place) {
    picked.current = p.address;
    setOpen(false);
    setPlaces([]);
    onChange(p.address);
    onPick(p);
  }

  const sourceLabel = (s: Place["source"]) =>
    s === "known" ? "yours" : s === "google" ? "Google" : "OpenStreetMap";

  return (
    <label className="relative flex flex-col gap-1 text-xs font-semibold text-ink-3">
      <span>
        {label}
        {required && <span className="ml-0.5 text-bad">*</span>}
      </span>
      <input
        type="text"
        value={value}
        placeholder={placeholder}
        autoComplete="off"
        onChange={(e) => {
          picked.current = null;
          onChange(e.target.value);
        }}
        onFocus={() => places.length > 0 && setOpen(true)}
        onBlur={() => setTimeout(() => setOpen(false), 150)}
        onKeyDown={(e) => {
          if (!open) return;
          if (e.key === "ArrowDown") {
            e.preventDefault();
            setActive((a) => Math.min(a + 1, places.length - 1));
          } else if (e.key === "ArrowUp") {
            e.preventDefault();
            setActive((a) => Math.max(a - 1, 0));
          } else if (e.key === "Enter" && active >= 0) {
            e.preventDefault();
            pick(places[active]);
          } else if (e.key === "Escape") {
            setOpen(false);
          }
        }}
        className={
          inputClassName ??
          "rounded-xl border border-line bg-surface px-3 py-2 text-sm font-normal text-ink outline-none focus:border-accent"
        }
      />
      {open && (
        <ul
          role="listbox"
          className="absolute left-0 right-0 top-full z-20 mt-1 overflow-hidden rounded-xl border border-line bg-raised shadow-lg"
        >
          {places.map((p, i) => (
            <li key={`${p.source}-${p.label}`}>
              <button
                type="button"
                role="option"
                aria-selected={i === active}
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => pick(p)}
                className={`flex w-full items-center justify-between gap-3 px-3 py-2 text-left text-sm font-normal ${
                  i === active
                    ? "bg-accent-soft text-accent-2"
                    : "text-ink hover:bg-surface-2"
                }`}
              >
                <span className="min-w-0 truncate">
                  {p.label}
                  {p.precision === "street" && (
                    <span className="ml-2 text-xs text-warn">
                      check the house number
                    </span>
                  )}
                </span>
                <span className="shrink-0 text-[10px] uppercase tracking-wide text-ink-3">
                  {sourceLabel(p.source)}
                </span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </label>
  );
}
