// Maps the module registry's string icon keys (and a few console-shell icons) to
// lucide-react components. The redesign standardizes on lucide as the single
// icon source; module authors keep using stable string keys in registry.ts.

import {
  Banknote,
  Building2,
  CircleDot,
  ClipboardCheck,
  Database,
  Globe2,
  KeyRound,
  LayoutDashboard,
  Palette,
  ScrollText,
  Settings2,
  Shield,
  ShieldCheck,
  Users,
  Wrench,
  type LucideIcon,
} from "lucide-react";

const NAV_ICONS: Record<string, LucideIcon> = {
  // module registry keys
  building: Building2,
  check: ClipboardCheck,
  shield: Shield,
  database: Database,
  globe: Globe2,
  user: Users,
  wrench: Wrench,
  key: KeyRound,
  dollar: Banknote,
  branding: Palette,
  // console-shell keys
  chart: LayoutDashboard,
  modules: Settings2,
  roles: ShieldCheck,
  audit: ScrollText,
  platform: Globe2,
};

export function navIcon(name: string): LucideIcon {
  return NAV_ICONS[name] ?? CircleDot;
}
