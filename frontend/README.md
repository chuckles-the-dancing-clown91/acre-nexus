# Acre frontend (Next.js)

Next.js 15 (App Router) + React 19 + TypeScript + Tailwind. Talks to the Rust API.

## Run

```bash
cp .env.local.example .env.local   # NEXT_PUBLIC_API_URL, NEXT_PUBLIC_DEFAULT_TENANT
npm install
npm run dev                        # http://localhost:3000
```

Start the backend first (see `../backend/README.md`).

## Structure

```
src/
  app/
    page.tsx                 public website (listings)
    listings/[id]/page.tsx   listing detail + application form
    login/page.tsx           console sign-in
    console/                 authenticated landlord/PM console
      page.tsx               portfolio dashboard
      properties/[id]        full property profile (computed economics)
      llcs/ applications/ tokens/ platform/
  components/                modular, pluggable UI (ui/, ListingCard, Icon, …)
  lib/                       api client, auth + theme providers, types
```

## Theming

Design tokens live as CSS variables in `app/globals.css` (ported from the
prototype). Tailwind colours reference them, so the palette re-themes for:

- **Dark mode** — `.dark` class on `<html>` (toggle in the header).
- **White-label** — `ThemeProvider` overrides `--accent` from each tenant's theme
  (`GET /public/theme`), so a client's brand colour flows through the whole UI.

## Build

```bash
npm run build      # production build
npm run typecheck  # tsc --noEmit
```

## Framework & tooling

The frontend is hardened into a complete app framework. Each layer is wired and
compiling; the reference patterns below show the intended way to add more.

### Data layer — TanStack Query (v5)

- `src/lib/query.tsx` — `QueryProvider` + a `QueryClient` (staleTime 30s, retry 1,
  `refetchOnWindowFocus: false`). Mounted outermost in `app/providers.tsx`.
- `src/lib/queries.ts` — typed hooks wrapping the existing `api` client with
  centralised `queryKeys`: `useProperties`, `useModules`, `usePortfolioSummary`,
  `useApplications`, `useApiTokens`, plus the `useCreateApiToken` /
  `useRevokeApiToken` mutations (which invalidate the token list on success).
- **Reference page:** `app/console/properties/page.tsx` fetches via
  `useProperties()` instead of `useEffect` + `useState`. Prefer this pattern for
  new pages.

### Forms — react-hook-form + Zod

- `src/lib/schemas.ts` — Zod schemas shared by forms and unit tests
  (`createTokenSchema`, `applicationSchema`).
- **Canonical form:** `app/console/tokens/page.tsx` — the "create API token" form
  uses `useForm` + `zodResolver(createTokenSchema)`, renders inline field errors,
  submits inside a shadcn `Dialog`, and fires a sonner success toast.

### State — Zustand (v5)

- `src/lib/store.ts` — `useUiStore`, a small global UI store with `persist`
  (key `acre.ui`). Holds the persisted **sidebar collapsed** state (wired into the
  console sidebar collapse toggle in `app/console/layout.tsx`) and the staff
  **acting tenant** slug, kept in sync with the api client's `actingTenant`
  localStorage key.

### UI library — shadcn/ui

shadcn is layered **on top of the existing design tokens** — `globals.css` is not
discarded. shadcn's CSS variables (`--background`, `--foreground`, `--primary`,
`--card`, …) are defined in `globals.css` as aliases of the Acre brand tokens
(`--bg`, `--ink`, `--accent`, `--surface`, …), so shadcn components inherit the
brand, dark mode, and white-label accent automatically. `tailwind.config.ts` maps
these to colour utilities. shadcn's hover "accent" is exposed as the
`accent-shadcn` utility because `accent` already belongs to the Acre brand token.

- `src/lib/utils.ts` — `cn()` (clsx + tailwind-merge).
- `components.json`, plus components in their own files under `components/ui/`:
  `button.tsx`, `input.tsx`, `label.tsx`, `dialog.tsx`, `dropdown-menu.tsx`,
  `sonner.tsx` (toast system). The legacy primitives in `components/ui/index.tsx`
  are kept; import shadcn pieces explicitly (e.g.
  `import { Button } from "@/components/ui/button"`) to avoid the name collision
  with the legacy `Button`.
- `<Toaster/>` is mounted in `app/providers.tsx`; trigger toasts with
  `import { toast } from "sonner"`.

### Testing — Vitest + Playwright

```bash
npm run test         # vitest run (unit/component)
npm run test:watch   # vitest watch
npm run test:e2e     # playwright (needs npm run dev + backend running)
```

- `vitest.config.ts` (jsdom, `@`→`src` alias, excludes `e2e/`) + `vitest.setup.ts`
  (jest-dom matchers). Unit/component tests live next to source as `*.test.ts(x)`:
  module registry, `hasPermission` auth logic, the Zod schemas, and a shadcn
  `Button` render test.
- `playwright.config.ts` runs the **preinstalled** Chromium via
  `launchOptions.executablePath = process.env.PLAYWRIGHT_CHROMIUM ?? "/opt/pw-browsers/chromium"`
  — do **not** run `playwright install`. The smoke spec `e2e/home.spec.ts` loads
  the public homepage and requires `npm run dev` + the backend to be running.

### Tooling — Prettier / ESLint / Husky / lint-staged

- Prettier: `.prettierrc` + `.prettierignore`; `npm run format` / `format:check`.
- ESLint: `eslint.config.mjs` (flat config bridging `next/core-web-vitals` +
  `next/typescript` via `FlatCompat`); `npm run lint`.
- Husky + lint-staged: `prepare` runs `husky`; `.husky/pre-commit` runs
  `lint-staged`, scoped to frontend globs (prettier --write + eslint --fix on
  staged files). Run `npm run prepare` once to install the git hook.

### Conversion status (reference patterns vs. TODO)

Converted as canonical examples:

- `app/console/properties/page.tsx` → TanStack Query (`useProperties`).
- `app/console/tokens/page.tsx` → RHF + Zod + shadcn Dialog + sonner toast.

Still using `useEffect` + `useState` (TODO: migrate to Query hooks / RHF):

- `app/console/page.tsx`, `app/console/applications/page.tsx`,
  `app/console/llcs/page.tsx`, `app/console/modules/page.tsx`,
  `app/console/platform/page.tsx`, `app/console/properties/[id]/page.tsx`,
  `app/console/flips/page.tsx`, `app/listings/[id]/page.tsx` (public application
  form — convert to RHF using the existing `applicationSchema`),
  `app/login/page.tsx`.
