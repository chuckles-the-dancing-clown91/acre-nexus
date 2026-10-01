"use client";

// The home page's search bar: text, price ceiling, bedrooms, bathrooms,
// available now, and sort.

import type { ListingSearch } from "@/lib/tours";

const field =
  "rounded-xl border border-line bg-surface px-3 py-2 text-sm text-ink";

export function ListingFilters({
  value,
  onChange,
}: {
  value: ListingSearch;
  onChange: (v: ListingSearch) => void;
}) {
  const set = (p: Partial<ListingSearch>) => onChange({ ...value, ...p });
  return (
    <div className="mb-5 flex flex-wrap items-end gap-3">
      <input
        className={`${field} w-56`}
        placeholder="City, street or name"
        value={value.q ?? ""}
        onChange={(e) => set({ q: e.target.value })}
        aria-label="Search"
      />
      <label className="text-xs font-semibold text-ink-3">
        Max rent
        <select
          className={`${field} mt-1 block`}
          value={value.max_rent ?? ""}
          onChange={(e) =>
            set({
              max_rent: e.target.value ? Number(e.target.value) : undefined,
            })
          }
        >
          <option value="">Any</option>
          {[1000, 1500, 2000, 2500, 3000, 4000].map((n) => (
            <option key={n} value={n}>
              ${n.toLocaleString()}
            </option>
          ))}
        </select>
      </label>
      <label className="text-xs font-semibold text-ink-3">
        Beds
        <select
          className={`${field} mt-1 block`}
          value={value.beds ?? ""}
          onChange={(e) =>
            set({ beds: e.target.value ? Number(e.target.value) : undefined })
          }
        >
          <option value="">Any</option>
          {[1, 2, 3, 4].map((n) => (
            <option key={n} value={n}>
              {n}+
            </option>
          ))}
        </select>
      </label>
      <label className="text-xs font-semibold text-ink-3">
        Baths
        <select
          className={`${field} mt-1 block`}
          value={value.baths ?? ""}
          onChange={(e) =>
            set({ baths: e.target.value ? Number(e.target.value) : undefined })
          }
        >
          <option value="">Any</option>
          {[1, 2, 3].map((n) => (
            <option key={n} value={n}>
              {n}+
            </option>
          ))}
        </select>
      </label>
      <label className="flex items-center gap-2 pb-2 text-sm font-semibold">
        <input
          type="checkbox"
          checked={value.available_now ?? false}
          onChange={(e) => set({ available_now: e.target.checked })}
        />
        Available now
      </label>
      <label className="ml-auto text-xs font-semibold text-ink-3">
        Sort
        <select
          className={`${field} mt-1 block`}
          value={value.sort ?? "newest"}
          onChange={(e) => set({ sort: e.target.value })}
        >
          <option value="newest">Newest</option>
          <option value="price_asc">Price, low to high</option>
          <option value="price_desc">Price, high to low</option>
          <option value="beds">Most bedrooms</option>
          <option value="sqft">Largest</option>
        </select>
      </label>
    </div>
  );
}
