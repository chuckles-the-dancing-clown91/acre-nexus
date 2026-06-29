"use client";

// Tenant white-label branding editor. A workspace admin sets the company name,
// logo, brand colours, and default colour mode that re-skin the whole product
// (and the public site). The accent colour previews live — as it changes we push
// it into the running theme via `setBrand`, so the entire app re-skins instantly
// before anything is persisted. Gated by `theme:write`.

import { useEffect, useRef } from "react";
import { useForm } from "react-hook-form";
import { Building2, Check, Lock, Sparkles } from "lucide-react";

import { useAuth } from "@/lib/auth";
import { useTheme } from "@/lib/theme";
import { useTenantTheme, useUpdateTenantTheme } from "@/lib/queries";
import type { PublicTheme } from "@/lib/types";

import { PageHeader, EmptyState } from "@/components/ui/page";
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Field, TextField, SelectField } from "@/components/ui/form-field";
import { Badge } from "@/components/ui";

interface FormValues {
  company_name: string;
  logo_url: string;
  primary_color: string;
  accent_color: string;
  default_mode: string;
}

const DEFAULTS: FormValues = {
  company_name: "",
  logo_url: "",
  primary_color: "#1f6f47",
  accent_color: "#1f6f47",
  default_mode: "light",
};

/** A labelled native colour swatch + its hex string, kept in sync. */
function ColorField({
  label,
  hint,
  value,
  onChange,
}: {
  label: string;
  hint?: string;
  value: string;
  onChange: (hex: string) => void;
}) {
  return (
    <Field label={label} hint={hint}>
      <div className="flex items-center gap-3">
        <input
          type="color"
          aria-label={label}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          className="h-10 w-12 shrink-0 cursor-pointer rounded-lg border border-line bg-surface p-1 outline-none transition focus-visible:border-accent"
        />
        <input
          type="text"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          spellCheck={false}
          className="w-full rounded-lg border border-line bg-surface px-3.5 py-2.5 font-mono text-sm uppercase text-ink outline-none transition focus:border-accent focus:ring-2 focus:ring-accent/20 placeholder:text-ink-3"
        />
      </div>
    </Field>
  );
}

export default function BrandingSettingsPage() {
  const { can } = useAuth();
  const allowed = can("theme:write");

  const themeQuery = useTenantTheme({ enabled: allowed });
  const updateTheme = useUpdateTenantTheme();
  const { setBrand } = useTheme();

  const {
    register,
    handleSubmit,
    reset,
    setValue,
    watch,
    formState: { isDirty },
  } = useForm<FormValues>({ defaultValues: DEFAULTS });

  const theme = themeQuery.data;
  const legalCount = theme
    ? Object.keys(theme.legal_templates ?? {}).length
    : 0;

  // Snapshot the brand active on mount so we can restore it if the editor is
  // left without saving (the live preview only mutates an in-memory CSS var).
  const savedBrandRef = useRef<PublicTheme | null>(null);

  // Hydrate the form once the saved theme loads, and remember the baseline brand.
  useEffect(() => {
    if (!theme) return;
    const next: FormValues = {
      company_name: theme.company_name ?? "",
      logo_url: theme.logo_url ?? "",
      primary_color: theme.primary_color || DEFAULTS.primary_color,
      accent_color: theme.accent_color || DEFAULTS.accent_color,
      default_mode: theme.default_mode || "light",
    };
    reset(next);
    savedBrandRef.current = {
      company_name: next.company_name,
      logo_url: next.logo_url || null,
      primary_color: next.primary_color,
      accent_color: next.accent_color,
      default_mode: next.default_mode,
    };
  }, [theme, reset]);

  const values = watch();

  // Live preview: push the in-progress brand into the running theme so the whole
  // app (nav, buttons, this page) re-skins to the chosen accent instantly.
  useEffect(() => {
    if (!allowed) return;
    setBrand({
      company_name: values.company_name,
      logo_url: values.logo_url || null,
      primary_color: values.primary_color,
      accent_color: values.accent_color,
      default_mode: values.default_mode,
    });
  }, [
    allowed,
    setBrand,
    values.company_name,
    values.logo_url,
    values.primary_color,
    values.accent_color,
    values.default_mode,
  ]);

  // On unmount, drop any unsaved preview back to the brand we loaded with.
  useEffect(() => {
    return () => {
      if (savedBrandRef.current) setBrand(savedBrandRef.current);
    };
  }, [setBrand]);

  const onSubmit = handleSubmit((v) => {
    updateTheme.mutate(
      {
        company_name: v.company_name,
        logo_url: v.logo_url || null,
        primary_color: v.primary_color,
        accent_color: v.accent_color,
        default_mode: v.default_mode,
      },
      {
        onSuccess: (saved) => {
          reset({
            company_name: saved.company_name ?? "",
            logo_url: saved.logo_url ?? "",
            primary_color: saved.primary_color || DEFAULTS.primary_color,
            accent_color: saved.accent_color || DEFAULTS.accent_color,
            default_mode: saved.default_mode || "light",
          });
          // The saved brand is now the baseline to restore to on unmount.
          savedBrandRef.current = {
            company_name: saved.company_name ?? "",
            logo_url: saved.logo_url ?? null,
            primary_color: saved.primary_color || DEFAULTS.primary_color,
            accent_color: saved.accent_color || DEFAULTS.accent_color,
            default_mode: saved.default_mode || "light",
          };
        },
      }
    );
  });

  if (!allowed) {
    return (
      <div className="space-y-6">
        <PageHeader
          eyebrow="Settings"
          title="Branding"
          description="White-label the workspace with your company name, logo, and brand colours."
        />
        <EmptyState
          icon={Lock}
          title="You don't have access"
          description={
            <>
              The <span className="font-mono text-ink-2">theme:write</span>{" "}
              permission is required to view and change branding. Ask a workspace
              admin if you need to update it.
            </>
          }
        />
      </div>
    );
  }

  if (themeQuery.isLoading) {
    return (
      <div className="space-y-6">
        <PageHeader
          eyebrow="Settings"
          title="Branding"
          description="White-label the workspace with your company name, logo, and brand colours."
        />
        <div className="grid gap-6 lg:grid-cols-2">
          <div className="skeleton h-96 rounded-xl" />
          <div className="skeleton h-96 rounded-xl" />
        </div>
      </div>
    );
  }

  const companyName = values.company_name?.trim() || "Your company";
  const accent = values.accent_color || DEFAULTS.accent_color;

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Settings"
        title="Branding"
        description="White-label the workspace with your company name, logo, and brand colours. Changes preview live and apply across the app once saved."
      />

      <form onSubmit={onSubmit} className="grid gap-6 lg:grid-cols-2">
        {/* ── Editor ─────────────────────────────────────────────────────── */}
        <Card>
          <CardHeader>
            <div className="min-w-0">
              <CardTitle>Identity &amp; colours</CardTitle>
              <CardDescription>
                These re-skin the console and your public site.
              </CardDescription>
            </div>
          </CardHeader>

          <CardContent className="space-y-5">
            <TextField
              label="Company name"
              placeholder="Northwind Properties"
              {...register("company_name")}
            />

            <TextField
              label="Logo URL"
              hint="A square logo works best. Leave blank to show the company initial."
              placeholder="https://…/logo.png"
              {...register("logo_url")}
            />

            <div className="grid gap-4 sm:grid-cols-2">
              <ColorField
                label="Primary color"
                hint="Used for primary surfaces."
                value={values.primary_color || DEFAULTS.primary_color}
                onChange={(hex) =>
                  setValue("primary_color", hex, { shouldDirty: true })
                }
              />
              <ColorField
                label="Accent color"
                hint="Buttons, links, highlights."
                value={accent}
                onChange={(hex) =>
                  setValue("accent_color", hex, { shouldDirty: true })
                }
              />
            </div>

            <SelectField
              label="Default color mode"
              hint="The mode new visitors see first."
              {...register("default_mode")}
            >
              <option value="light">Light</option>
              <option value="dark">Dark</option>
            </SelectField>

            <div className="flex items-start gap-3 rounded-lg border border-line bg-surface-2 px-4 py-3">
              <Sparkles className="mt-0.5 h-4 w-4 shrink-0 text-accent-2" />
              <p className="text-sm text-ink-2">
                {legalCount > 0 ? (
                  <>
                    <span className="font-semibold text-ink">
                      {legalCount} legal{" "}
                      {legalCount === 1 ? "template" : "templates"}
                    </span>{" "}
                    are configured for this workspace. They&apos;re managed
                    separately and aren&apos;t edited here.
                  </>
                ) : (
                  <>
                    Legal templates (lease boilerplate, disclosures) are managed
                    separately and aren&apos;t edited here.
                  </>
                )}
              </p>
            </div>
          </CardContent>

          <CardFooter className="flex items-center justify-between gap-3">
            <p className="text-xs text-ink-3">
              {themeQuery.error
                ? "Couldn't load the current branding."
                : "Preview is live — save to apply for everyone."}
            </p>
            <Button type="submit" disabled={updateTheme.isPending || !isDirty}>
              {updateTheme.isPending ? "Saving…" : "Save branding"}
            </Button>
          </CardFooter>
        </Card>

        {/* ── Live preview ───────────────────────────────────────────────── */}
        <Card>
          <CardHeader>
            <div className="min-w-0">
              <CardTitle>Preview</CardTitle>
              <CardDescription>
                How your brand looks across the app.
              </CardDescription>
            </div>
            <Badge tone="accent">Live</Badge>
          </CardHeader>

          <CardContent className="space-y-5">
            {/* Sample app header */}
            <div className="overflow-hidden rounded-xl border border-line">
              <div
                className="flex items-center gap-3 px-4 py-3 text-on-accent"
                style={{
                  background: `linear-gradient(135deg, ${accent} 0%, ${
                    values.primary_color || accent
                  } 100%)`,
                }}
              >
                {values.logo_url ? (
                  // eslint-disable-next-line @next/next/no-img-element
                  <img
                    src={values.logo_url}
                    alt=""
                    className="h-9 w-9 shrink-0 rounded-lg bg-white/10 object-contain"
                  />
                ) : (
                  <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-white/15 font-display text-lg font-bold">
                    {companyName.charAt(0).toUpperCase()}
                  </span>
                )}
                <div className="min-w-0">
                  <div className="truncate font-display text-base font-bold tracking-tight">
                    {companyName}
                  </div>
                  <div className="truncate text-xs text-on-accent/70">
                    Property operations
                  </div>
                </div>
              </div>

              {/* Sample body using the running accent token (preview-driven) */}
              <div className="space-y-4 bg-surface px-4 py-4">
                <div className="flex flex-wrap items-center gap-2">
                  <Button size="sm" type="button">
                    Primary action
                  </Button>
                  <Button size="sm" variant="outline" type="button">
                    Secondary
                  </Button>
                  <Button size="sm" variant="ghost" type="button">
                    Ghost
                  </Button>
                </div>

                <div className="flex flex-wrap items-center gap-2">
                  <Badge tone="accent">
                    <Check className="h-3 w-3" />
                    Brand accent
                  </Badge>
                  <Badge tone="good">Active</Badge>
                  <Badge tone="neutral">Draft</Badge>
                </div>

                <div className="flex items-center gap-3 rounded-lg border border-line bg-surface-2 px-3 py-2.5">
                  <Building2 className="h-4 w-4 text-accent-2" />
                  <span className="text-sm text-ink-2">
                    Accent links &amp; icons pick up{" "}
                    <span className="font-mono text-xs uppercase text-accent-2">
                      {accent}
                    </span>
                  </span>
                </div>
              </div>
            </div>

            {/* Swatch row */}
            <div className="grid grid-cols-2 gap-3">
              <div className="flex items-center gap-2.5 rounded-lg border border-line px-3 py-2.5">
                <span
                  className="h-7 w-7 shrink-0 rounded-md border border-line-2"
                  style={{ background: values.primary_color }}
                />
                <div className="min-w-0">
                  <div className="text-xs font-semibold text-ink">Primary</div>
                  <div className="truncate font-mono text-xs uppercase text-ink-3">
                    {values.primary_color}
                  </div>
                </div>
              </div>
              <div className="flex items-center gap-2.5 rounded-lg border border-line px-3 py-2.5">
                <span
                  className="h-7 w-7 shrink-0 rounded-md border border-line-2"
                  style={{ background: accent }}
                />
                <div className="min-w-0">
                  <div className="text-xs font-semibold text-ink">Accent</div>
                  <div className="truncate font-mono text-xs uppercase text-ink-3">
                    {accent}
                  </div>
                </div>
              </div>
            </div>
          </CardContent>
        </Card>
      </form>
    </div>
  );
}
