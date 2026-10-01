// Theme vocabulary shared by server (root layouts) and client (provider).
//
// The theme is decided at the gate: the host → audience mapping from
// `GET /public/resolve` (admin app, owner portal, renter portal, or the public
// site) picks the surface, and each surface has a theme. The staff console is
// always Obsidian; the gate pages (login etc.) follow the host's audience.

export type ThemeName = "obsidian" | "daylight";

export type Audience = "admin" | "owner" | "renter" | "public";

export const AUDIENCE_THEME: Record<Audience, ThemeName> = {
  admin: "obsidian",
  owner: "daylight",
  renter: "daylight",
  public: "daylight",
};

export function normalizeAudience(raw: string | null | undefined): Audience {
  return raw === "admin" || raw === "owner" || raw === "renter"
    ? raw
    : "public";
}

export interface Brand {
  company_name: string;
  logo_url: string | null;
  accent_color: string;
}

/** Everything the gate decides about a request before anyone signs in. */
export interface Gate {
  tenant: string;
  audience: Audience;
  brand: Brand;
}

/** Cookie holding the console's HUD-intensity preference (`1` = on). */
export const HUD_COOKIE = "acre.hud";
