# Acre OS — UI Build Guide

The single source of truth for the frontend overhaul. **Read this + the golden-sample
files before writing any component or page.** Match their style exactly.

Golden samples (read these first):

- `src/app/globals.css` — design tokens
- `src/app/console/layout.tsx` — the console shell
- `src/app/console/page.tsx` — the dashboard (page pattern, table, stats)
- `src/app/login/page.tsx` — form + split layout
- `src/components/ui/card.tsx`, `src/components/ui/page.tsx` — primitives
- `src/lib/api.ts` — the typed API client (`api`, `iam`) — the contract
- `src/lib/queries.ts` — TanStack Query hooks (the data layer)
- `src/lib/types.ts` — domain types

## Stack

Next.js 15 (App Router) · React 19 · TypeScript · Tailwind 3 · Radix · shadcn-style
primitives · **TanStack Query v5** (server state) · Zustand (UI state) ·
react-hook-form + zod (forms) · lucide-react (icons) · sonner (toasts) ·
@tanstack/react-table (DataTable) · recharts (charts) · date-fns.

## Design tokens → Tailwind classes

Never hardcode hex. Use these utilities (all re-theme for dark + white-label):

- Surfaces: `bg-bg` (app canvas), `bg-surface` (cards), `bg-surface-2` (subtle fill),
  `bg-raised` (popovers/menus)
- Text: `text-ink` (primary), `text-ink-2` (secondary), `text-ink-3` (tertiary/muted)
- Borders: `border-line` (default hairline), `border-line-2` (stronger)
- Brand: `bg-accent` / `text-accent-2` / `bg-accent-soft` / `text-on-accent`
- Semantic: `good`/`warn`/`bad`/`info` each with a `-soft` bg variant
  (e.g. `bg-good-soft text-good`)
- Elevation: `shadow-acre`, `shadow-acre-lg`
- Type: `font-display` (headings), `font-sans` (body, default), `font-mono` (numbers/IDs)
- Radius: rounded-lg/md/sm derive from `--radius`; cards use `rounded-xl`

Numbers: put `data-numeric` on cells/values with figures (enables tabular-nums).
Animate page mounts with the `acre-fade` class on the page root (already applied by the
shell, so usually not needed per page).

## Components (import paths)

- `@/components/ui/button` → `Button` (variants: default, outline, ghost, secondary,
  destructive, link; sizes: default, sm, lg, icon; supports `asChild` for `<Link>`)
- `@/components/ui/card` → `Card, CardHeader, CardTitle, CardDescription, CardContent, CardFooter`
- `@/components/ui/page` → `PageHeader, StatCard, EmptyState`
- `@/components/ui` → legacy `Badge` + `statusTone(status)` helper (keep using these for status pills)
- `@/components/ui/data-table` → `DataTable` (sorting/filter/pagination; built in Phase 2)
- `@/components/ui/form-field` → `Field, TextField, SelectField, TextareaField` (RHF-aware; Phase 2)
- `@/components/ui/tabs`, `tooltip`, `select`, `switch`, `dialog`, `breadcrumbs` (Phase 2)
- `@/lib/utils` → `cn(...)` for class merging
- Icons: import from `lucide-react`. For module nav keys use `navIcon()` from
  `@/components/console/nav-icons`.

## Data layer — ALWAYS TanStack Query, never useEffect+useState

Read with hooks from `@/lib/queries`. If a hook is missing, add it there following the
existing pattern (query keys in `queryKeys`, `enabled: isAuthed()`). Examples:

```tsx
const props = useProperties();           // props.data, props.isLoading, props.error
const summary = usePortfolioSummary();
```

Mutations: `useMutation` wrapping `api.*` / `iam.*`, invalidate the relevant `queryKeys`
on success, and toast via `sonner` (`toast.success` / `toast.error`). See the IAM
mutations in `queries.ts`.

Polling async jobs (enrichment, background jobs): pass `refetchInterval` to the query
while a run is `pending`/`running` — do NOT use setTimeout.

## Page scaffold

```tsx
"use client";
export default function FooPage() {
  const { can } = useAuth();
  const q = useFoo();
  return (
    <div className="space-y-6">
      <PageHeader title="Foo" description="…" actions={<Button>…</Button>} />
      {q.isLoading ? <Skeletons/> : q.data?.length ? <DataTable .../> : <EmptyState .../>}
    </div>
  );
}
```

- Gate actions/links with `useAuth().can("perm:key")`. Permissions are listed in the
  user object; the backend is authoritative.
- Loading → skeletons (`<div className="skeleton h-N rounded-lg" />`) or DataTable's own.
- Empty → `EmptyState`. Errors from mutations → toast. Page-level query errors are caught
  by `console/error.tsx`.
- Detail pages (`[id]`): use `Breadcrumbs` + tabs where there are sub-sections.

## Personas (route + permission map)

- **Tenant admin / employee** → `/console/*` (module-gated by `isEnabled` + `can`)
- **Acre platform staff** (`is_platform_staff`) → `/console/platform/*`
- The shell (`console/layout.tsx`) already builds nav from the module registry +
  permissions. Pages just render content + gate their own actions.

## Conventions

- `"use client"` on any interactive/stateful page or component.
- Currency: the API usually returns a preformatted `*_label` (e.g. `monthly_rent_label`)
  alongside `*_cents`. Prefer the label; if formatting yourself use
  `(cents/100).toLocaleString(undefined,{style:"currency",currency:"USD"})`.
- Dates: `date-fns` `format(new Date(iso), "MMM d, yyyy")`.
- Accessibility: real `<button>`/`<a>`, `aria-label` on icon-only controls, `<th>` headers
  on tables, focus-visible rings come from globals.
- Keep files focused; colocate small page-only subcomponents in the page file.
