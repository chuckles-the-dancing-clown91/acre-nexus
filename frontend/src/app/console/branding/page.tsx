"use client";

import { useEffect, useState } from "react";
import { api, type ThemeConfig } from "@/lib/api";
import { Card } from "@/components/ui";

export default function BrandingPage() {
  const [theme, setTheme] = useState<ThemeConfig | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const [busy, setBusy] = useState(false);

  // Editable fields.
  const [companyName, setCompanyName] = useState("");
  const [logoUrl, setLogoUrl] = useState("");
  const [primary, setPrimary] = useState("#f5451f");
  const [accent, setAccent] = useState("#f5451f");
  const [mode, setMode] = useState("light");

  useEffect(() => {
    api
      .theme()
      .then((t) => {
        setTheme(t);
        setCompanyName(t.company_name);
        setLogoUrl(t.logo_url ?? "");
        setPrimary(t.primary_color || "#f5451f");
        setAccent(t.accent_color || "#f5451f");
        setMode(t.default_mode || "light");
      })
      .catch((e) => setError(e.message));
  }, []);

  async function save(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    setSaved(false);
    try {
      const updated = await api.updateTheme({
        company_name: companyName,
        logo_url: logoUrl,
        primary_color: primary,
        accent_color: accent,
        default_mode: mode,
      });
      setTheme(updated);
      setSaved(true);
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="space-y-6">
      <div>
        <h1 className="font-display text-3xl font-extrabold tracking-tight">
          Branding
        </h1>
        <p className="text-ink-3">
          Your white-label identity — shown on the public site, owner and renter
          portals, and emails.
        </p>
      </div>

      {error && <p className="text-bad">{error}</p>}

      {theme && (
        <div className="grid gap-6 lg:grid-cols-[1fr_320px]">
          <Card className="p-5">
            <form onSubmit={save} className="space-y-4">
              <label className="block text-sm">
                <span className="mb-1 block text-ink-3">Company name</span>
                <input
                  value={companyName}
                  onChange={(e) => setCompanyName(e.target.value)}
                  className="w-full rounded-lg border border-line bg-surface px-3 py-2"
                />
              </label>

              <label className="block text-sm">
                <span className="mb-1 block text-ink-3">Logo URL</span>
                <input
                  value={logoUrl}
                  onChange={(e) => setLogoUrl(e.target.value)}
                  placeholder="https://…/logo.svg"
                  className="w-full rounded-lg border border-line bg-surface px-3 py-2"
                />
              </label>

              <div className="flex flex-wrap gap-4">
                <label className="text-sm">
                  <span className="mb-1 block text-ink-3">Primary color</span>
                  <div className="flex items-center gap-2">
                    <input
                      type="color"
                      value={primary}
                      onChange={(e) => setPrimary(e.target.value)}
                      className="h-10 w-12 rounded border border-line bg-surface"
                    />
                    <input
                      value={primary}
                      onChange={(e) => setPrimary(e.target.value)}
                      className="w-28 rounded-lg border border-line bg-surface px-2 py-2 font-mono text-xs"
                    />
                  </div>
                </label>
                <label className="text-sm">
                  <span className="mb-1 block text-ink-3">Accent color</span>
                  <div className="flex items-center gap-2">
                    <input
                      type="color"
                      value={accent}
                      onChange={(e) => setAccent(e.target.value)}
                      className="h-10 w-12 rounded border border-line bg-surface"
                    />
                    <input
                      value={accent}
                      onChange={(e) => setAccent(e.target.value)}
                      className="w-28 rounded-lg border border-line bg-surface px-2 py-2 font-mono text-xs"
                    />
                  </div>
                </label>
              </div>

              <label className="block text-sm">
                <span className="mb-1 block text-ink-3">Default mode</span>
                <select
                  value={mode}
                  onChange={(e) => setMode(e.target.value)}
                  className="rounded-lg border border-line bg-surface px-3 py-2 capitalize"
                >
                  <option value="light">light</option>
                  <option value="dark">dark</option>
                </select>
              </label>

              <div className="flex items-center gap-3">
                <button
                  type="submit"
                  disabled={busy}
                  className="rounded-lg bg-accent px-4 py-2 font-semibold text-white disabled:opacity-50"
                >
                  Save branding
                </button>
                {saved && <span className="text-sm text-good">Saved.</span>}
              </div>
            </form>
          </Card>

          {/* Live preview */}
          <Card className="space-y-4 p-5">
            <div className="text-xs font-semibold uppercase tracking-wide text-ink-3">
              Preview
            </div>
            <div
              className="rounded-xl p-5 text-white"
              style={{ backgroundColor: primary }}
            >
              <div className="flex items-center gap-3">
                {logoUrl ? (
                  // eslint-disable-next-line @next/next/no-img-element
                  <img
                    src={logoUrl}
                    alt=""
                    className="h-8 w-8 rounded bg-white/20 object-contain"
                  />
                ) : (
                  <div className="flex h-8 w-8 items-center justify-center rounded bg-white/20 font-bold">
                    {companyName.charAt(0) || "A"}
                  </div>
                )}
                <span className="font-display text-lg font-bold">
                  {companyName || "Your firm"}
                </span>
              </div>
            </div>
            <button
              type="button"
              className="w-full rounded-lg px-4 py-2 font-semibold text-white"
              style={{ backgroundColor: accent }}
            >
              Accent button
            </button>
            <p className="text-xs text-ink-3">
              Mode default: <span className="capitalize">{mode}</span>
            </p>
          </Card>
        </div>
      )}
    </div>
  );
}
