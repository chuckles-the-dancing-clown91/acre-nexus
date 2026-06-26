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
