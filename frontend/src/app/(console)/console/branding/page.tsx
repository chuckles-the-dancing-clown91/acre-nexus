"use client";

// Branding: the name, logo and colours your people see. The host decides
// whose brand shows: your verified domains wear yours, Vantedge's own hosts
// wear Vantedge. The preview fits your colour to both themes the same way the
// real theme does (src/theme/accent.ts), so what you see here is what ships.

import { useEffect, useMemo, useState } from "react";
import Link from "next/link";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Check,
  Eye,
  Globe,
  Palette,
  RotateCcw,
  TriangleAlert,
} from "lucide-react";
import { toast } from "sonner";
import { api, type ThemeConfig } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { useDomains } from "@/lib/queries";
import { useTheme } from "@/theme/ThemeProvider";
import {
  accentPalette,
  accentStyle,
  contrast,
  DEFAULT_ACCENT,
  hexToRgb,
} from "@/theme/accent";
import type { Brand, ThemeName } from "@/theme/themes";
import { BrandLogo, PoweredBy } from "@/components/brand";
import { useHasTenantScope } from "@/components/shell/tenant-scope";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Field, fieldClass, Input, Label } from "@/components/ui/input";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

const KEY = ["theme-config"];
const HEX = /^#[0-9a-f]{6}$/i;

function normalizeHex(v: string): string {
  const s = v.trim();
  return s.startsWith("#") ? s : `#${s}`;
}

export default function BrandingPage() {
  const { can } = useAuth();
  const scoped = useHasTenantScope();
  const allowed = can("theme:write");
  const q = useQuery({
    queryKey: KEY,
    queryFn: api.theme,
    enabled: scoped && allowed,
  });

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Workspace"
        title="Branding"
        description="Your name, logo and colours on the website, the owner and resident portals, and emails."
      />
      {!allowed && (
        <Panel>
          <EmptyState
            icon={<Palette />}
            title="You can't change branding"
            description="Ask a company owner for the theme:write permission."
          />
        </Panel>
      )}
      {q.isLoading && <Skeleton className="h-96 rounded-2xl" />}
      {q.error && (
        <Panel className="border-bad/30 p-4 text-[13px] text-bad">
          Couldn&apos;t load branding: {q.error.message}
        </Panel>
      )}
      {q.data && <BrandEditor initial={q.data} />}
    </div>
  );
}

function BrandEditor({ initial }: { initial: ThemeConfig }) {
  const qc = useQueryClient();
  const { gate, brand: liveBrand, setBrand } = useTheme();
  const [companyName, setCompanyName] = useState(initial.company_name);
  const [logoUrl, setLogoUrl] = useState(initial.logo_url ?? "");
  const [primary, setPrimary] = useState(
    initial.primary_color || DEFAULT_ACCENT
  );
  const [accent, setAccent] = useState(
    initial.accent_color || initial.primary_color || DEFAULT_ACCENT
  );
  const [mode, setMode] = useState(initial.default_mode || "light");
  const [trying, setTrying] = useState(false);

  // The brand to put back when trying-on stops or the page closes.
  const [restore, setRestore] = useState<Brand>(liveBrand);

  const primaryOk = HEX.test(primary);
  const accentOk = HEX.test(accent);
  const logoOk = !logoUrl.trim() || /^https?:\/\/\S+$/i.test(logoUrl.trim());
  const valid =
    companyName.trim().length > 0 && primaryOk && accentOk && logoOk;
  const dirty =
    companyName !== initial.company_name ||
    logoUrl !== (initial.logo_url ?? "") ||
    primary !== (initial.primary_color || DEFAULT_ACCENT) ||
    accent !==
      (initial.accent_color || initial.primary_color || DEFAULT_ACCENT) ||
    mode !== (initial.default_mode || "light");

  const draftName = companyName.trim() || "Your company";
  const draftLogo = logoOk && logoUrl.trim() ? logoUrl.trim() : null;
  const draftAccent = accentOk ? accent : primaryOk ? primary : DEFAULT_ACCENT;
  const draft = useMemo<Brand>(
    () => ({
      company_name: draftName,
      logo_url: draftLogo,
      accent_color: draftAccent,
    }),
    [draftName, draftLogo, draftAccent]
  );

  // While trying it on, the whole console follows the draft.
  useEffect(() => {
    if (!trying) return;
    setBrand(draft);
    return () => setBrand(restore);
  }, [trying, draft, restore, setBrand]);

  const save = useMutation({
    mutationFn: () =>
      api.updateTheme({
        company_name: companyName.trim(),
        logo_url: logoUrl.trim(),
        primary_color: primary,
        accent_color: accent,
        default_mode: mode,
      }),
    onSuccess: (t) => {
      qc.setQueryData(KEY, t);
      // On your own domain the console wears your brand, so show it now.
      if (gate.branded) {
        const saved: Brand = {
          company_name: t.company_name,
          logo_url: t.logo_url,
          accent_color: t.accent_color || t.primary_color,
        };
        setRestore(saved);
        setBrand(saved);
      }
      toast.success("Branding saved");
    },
    onError: (e) => toast.error(e.message || "Couldn't save branding"),
  });

  function reset() {
    setCompanyName(initial.company_name);
    setLogoUrl(initial.logo_url ?? "");
    setPrimary(initial.primary_color || DEFAULT_ACCENT);
    setAccent(initial.accent_color || initial.primary_color || DEFAULT_ACCENT);
    setMode(initial.default_mode || "light");
  }

  return (
    <div className="grid gap-6 lg:grid-cols-[minmax(0,1fr)_380px]">
      <div className="space-y-6">
        <Panel>
          <PanelHeader
            title="Identity"
            description="Shown in the header of every portal and at the foot of emails."
          />
          <form
            className="space-y-5 p-5"
            onSubmit={(e) => {
              e.preventDefault();
              if (valid) save.mutate();
            }}
          >
            <Field
              label="Company name"
              error={companyName.trim() ? null : "Enter your company name"}
            >
              {(p) => (
                <Input
                  {...p}
                  value={companyName}
                  onChange={(e) => setCompanyName(e.target.value)}
                />
              )}
            </Field>
            <Field
              label="Logo address"
              hint="A square image works best: SVG or PNG, at least 128 pixels. Leave empty to use a monogram."
              error={logoOk ? null : "Use a full https:// address"}
            >
              {(p) => (
                <Input
                  {...p}
                  placeholder="https://yourcompany.com/logo.svg"
                  value={logoUrl}
                  onChange={(e) => setLogoUrl(e.target.value)}
                />
              )}
            </Field>
            <div className="grid gap-5 sm:grid-cols-2">
              <ColorField
                label="Primary colour"
                hint="Headers and bands on your website."
                value={primary}
                onChange={setPrimary}
              />
              <ColorField
                label="Accent colour"
                hint="Buttons, links and highlights everywhere."
                value={accent}
                onChange={setAccent}
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="brand-mode">Website default</Label>
              <select
                id="brand-mode"
                className={cn(fieldClass, "block h-11 w-full sm:w-56")}
                value={mode}
                onChange={(e) => setMode(e.target.value)}
              >
                <option value="light">Light</option>
                <option value="dark">Dark</option>
              </select>
              <p className="text-xs text-fg-3">
                The staff app is always dark; portals and the website start in
                this mode.
              </p>
            </div>
            <div className="flex flex-wrap items-center gap-2 border-t border-line pt-5">
              <Button
                type="submit"
                disabled={!valid || !dirty}
                loading={save.isPending}
              >
                {!save.isPending && <Check />}
                Save branding
              </Button>
              <Button
                type="button"
                variant="ghost"
                disabled={!dirty}
                onClick={reset}
              >
                <RotateCcw />
                Undo changes
              </Button>
              <Button
                type="button"
                variant="secondary"
                className="sm:ml-auto"
                onClick={() => setTrying((t) => !t)}
                disabled={!accentOk && !trying}
              >
                <Eye />
                {trying ? "Stop trying it on" : "Try it on this console"}
              </Button>
            </div>
          </form>
        </Panel>

        <WhereItShows />
      </div>

      <div className="space-y-4 lg:sticky lg:top-6 lg:self-start">
        <Preview theme="daylight" brand={draft} primary={primary} />
        <Preview theme="obsidian" brand={draft} primary={primary} />
        <FitNote accent={draft.accent_color} />
      </div>
    </div>
  );
}

function ColorField({
  label,
  hint,
  value,
  onChange,
}: {
  label: string;
  hint: string;
  value: string;
  onChange: (v: string) => void;
}) {
  const ok = HEX.test(value);
  return (
    <Field
      label={label}
      hint={hint}
      error={ok ? null : "Use a hex colour like #0e7c86"}
    >
      {(p) => (
        <div className="flex items-center gap-2">
          <input
            type="color"
            aria-label={`${label} picker`}
            value={ok ? value : DEFAULT_ACCENT}
            onChange={(e) => onChange(e.target.value)}
            className="h-11 w-12 shrink-0 cursor-pointer rounded-xl border border-line-strong bg-fill p-1"
          />
          <Input
            {...p}
            className="font-mono"
            maxLength={7}
            value={value}
            onChange={(e) => onChange(normalizeHex(e.target.value))}
          />
        </div>
      )}
    </Field>
  );
}

/** A small mock of a page in one theme, with the accent fitted to it. */
function Preview({
  theme,
  brand,
  primary,
}: {
  theme: ThemeName;
  brand: Brand;
  primary: string;
}) {
  const label =
    theme === "daylight" ? "Portals and website" : "Staff app on your domain";
  const band = HEX.test(primary) ? primary : DEFAULT_ACCENT;
  const bandInk =
    hexToRgb(band) && contrast(band, "#ffffff") >= contrast(band, "#071019")
      ? "#ffffff"
      : "#071019";
  return (
    <Panel className="overflow-hidden">
      <div className="flex items-center justify-between px-4 pt-4">
        <span className="eyebrow">{label}</span>
        <Badge tone="neutral">
          {theme === "daylight" ? "Daylight" : "Obsidian"}
        </Badge>
      </div>
      <div
        data-theme={theme}
        style={accentStyle(brand.accent_color, theme) as React.CSSProperties}
        className="m-4 overflow-hidden rounded-xl border border-line bg-bg text-fg"
      >
        {theme === "daylight" && (
          <div
            className="flex items-center gap-2.5 px-4 py-3"
            style={{ backgroundColor: band, color: bandInk }}
          >
            <BrandLogo brand={brand} size={28} />
            <span className="truncate font-display text-[15px] font-semibold">
              {brand.company_name}
            </span>
          </div>
        )}
        <div className="space-y-3 p-4">
          {theme === "obsidian" && (
            <div className="flex items-center gap-2.5">
              <BrandLogo brand={brand} size={28} />
              <span className="truncate text-[14px] font-semibold text-fg">
                {brand.company_name}
              </span>
            </div>
          )}
          <div className="glass rounded-xl p-3">
            <div className="text-[13px] font-medium text-fg">
              2 bed, 1 bath on Alder St
            </div>
            <div className="mt-0.5 text-xs text-fg-3">
              Available now · <span className="text-accent">See photos</span>
            </div>
          </div>
          <div className="flex items-center gap-2">
            <Button size="sm" type="button" tabIndex={-1}>
              Request a tour
            </Button>
            <Badge tone="accent">New</Badge>
          </div>
          <PoweredBy />
        </div>
      </div>
    </Panel>
  );
}

/** Why the colour on screen can differ a little from the one picked. */
function FitNote({ accent }: { accent: string }) {
  const dark = accentPalette(accent, "obsidian");
  const light = accentPalette(accent, "daylight");
  const moved =
    dark.accent.toLowerCase() !== accent.toLowerCase() ||
    light.accent.toLowerCase() !== accent.toLowerCase();
  return (
    <Panel className="p-4">
      <div className="flex items-start gap-2 text-[13px] text-fg-2">
        {moved ? (
          <TriangleAlert className="mt-0.5 size-4 shrink-0 text-warn" />
        ) : (
          <Check className="mt-0.5 size-4 shrink-0 text-good" />
        )}
        <p>
          {moved
            ? "Your accent is adjusted a little in each theme so text on it stays readable."
            : "Your accent works as is in both themes."}
        </p>
      </div>
      <dl className="mt-3 grid grid-cols-3 gap-2 text-xs">
        {[
          ["Picked", accent],
          ["Light", light.accent],
          ["Dark", dark.accent],
        ].map(([k, v]) => (
          <div key={k} className="rounded-lg border border-line p-2">
            <span
              className="mb-1.5 block h-5 rounded"
              style={{ backgroundColor: v }}
            />
            <dt className="text-fg-3">{k}</dt>
            <dd className="font-mono text-fg">{v}</dd>
          </div>
        ))}
      </dl>
    </Panel>
  );
}

/** Which hosts wear the brand, from the gate's rules and your domains. */
function WhereItShows() {
  const { can } = useAuth();
  const scoped = useHasTenantScope();
  const { gate } = useTheme();
  const domains = useDomains({ enabled: scoped && can("domain:read") });
  const verified = (domains.data ?? []).filter((d) => d.verified);
  const pending = (domains.data ?? []).filter((d) => !d.verified);
  return (
    <Panel>
      <PanelHeader
        title="Where your brand shows"
        description="The address decides. Your verified domains show your brand with “Powered by Vantedge”; Vantedge's own addresses show Vantedge."
        action={
          <Badge tone={gate.branded ? "good" : "neutral"}>
            {gate.branded ? "This address: yours" : "This address: Vantedge"}
          </Badge>
        }
      />
      <div className="p-5">
        {domains.isLoading && <Skeleton className="h-16" />}
        {domains.data && (
          <ul className="divide-y divide-line rounded-xl border border-line">
            {verified.map((d) => (
              <li
                key={d.id}
                className="flex items-center gap-3 px-4 py-2.5 text-[13px]"
              >
                <Globe className="size-4 text-fg-3" />
                <span className="min-w-0 flex-1 truncate font-mono text-fg">
                  {d.hostname}
                </span>
                <Badge tone="good">Shows your brand</Badge>
              </li>
            ))}
            {pending.map((d) => (
              <li
                key={d.id}
                className="flex items-center gap-3 px-4 py-2.5 text-[13px]"
              >
                <Globe className="size-4 text-fg-3" />
                <span className="min-w-0 flex-1 truncate font-mono text-fg">
                  {d.hostname}
                </span>
                <Badge tone="warn">Waiting on DNS</Badge>
              </li>
            ))}
            {domains.data.length === 0 && (
              <li className="px-4 py-3 text-[13px] text-fg-3">
                No domains of your own yet, so only your reserved subdomain and
                the public website show your brand.
              </li>
            )}
          </ul>
        )}
        {can("domain:read") && (
          <Button asChild size="sm" variant="secondary" className="mt-3">
            <Link href="/console/domains">
              <Globe />
              Manage domains
            </Link>
          </Button>
        )}
      </div>
    </Panel>
  );
}
