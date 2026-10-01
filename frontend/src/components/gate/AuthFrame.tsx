"use client";

// The frame every gate page (sign in, password links, auth callback) shares:
// a hero with the workspace brand over a glass city, and the form card.

import { Ambient } from "@/components/ambient";
import { BrandLogo, PoweredBy, PRODUCT_SLOGAN } from "@/components/brand";
import { IsoScene } from "@/components/iso/IsoScene";
import type { IsoBlock } from "@/components/iso/geometry";
import { Panel } from "@/components/ui/panel";
import { useTheme } from "@/theme/ThemeProvider";

// Decorative only: no numbers, nothing that reads as real portfolio data.
// Heights fall toward the viewer so roofs never stack confusingly.
const HERO_CITY: IsoBlock[] = [
  {
    id: "a",
    x: 0,
    y: 0,
    w: 1.8,
    d: 1.8,
    h: 7.5,
    floors: 11,
    lit: 0.55,
    tone: "accent",
  },
  {
    id: "b",
    x: 2.7,
    y: 0,
    w: 2,
    d: 2,
    h: 5,
    floors: 7,
    lit: 0.5,
    tone: "accent",
  },
  {
    id: "c",
    x: 0,
    y: 2.7,
    w: 2,
    d: 1.8,
    h: 5.5,
    floors: 8,
    lit: 0.62,
    tone: "info",
  },
  {
    id: "d",
    x: 5.4,
    y: 0,
    w: 2.2,
    d: 2,
    h: 3.4,
    floors: 5,
    lit: 0.45,
    tone: "accent",
  },
  {
    id: "e",
    x: 2.7,
    y: 2.7,
    w: 2,
    d: 2,
    h: 4,
    floors: 6,
    lit: 0.6,
    tone: "accent",
  },
  {
    id: "f",
    x: 0,
    y: 5.4,
    w: 2,
    d: 2.2,
    h: 3,
    floors: 4,
    lit: 0.7,
    tone: "accent",
  },
  {
    id: "g",
    x: 5.4,
    y: 2.7,
    w: 2,
    d: 2,
    h: 2.4,
    floors: 3,
    lit: 0.4,
    tone: "plasma",
  },
  {
    id: "h",
    x: 2.7,
    y: 5.4,
    w: 2,
    d: 2,
    h: 2,
    floors: 3,
    lit: 0.8,
    tone: "accent",
  },
  {
    id: "i",
    x: 5.4,
    y: 5.4,
    w: 2.2,
    d: 1.8,
    h: 1.3,
    floors: 2,
    lit: 0.85,
    tone: "accent",
  },
];

export function AuthFrame({ children }: { children: React.ReactNode }) {
  const { brand } = useTheme();
  return (
    <main className="relative grid min-h-dvh lg:grid-cols-[minmax(0,1.1fr)_minmax(0,1fr)]">
      <Ambient />

      <section className="relative hidden flex-col justify-between overflow-hidden px-12 py-10 lg:flex">
        <div className="flex items-center gap-3">
          <BrandLogo brand={brand} size={36} />
          <div className="leading-tight">
            <div className="font-display text-[15px] font-semibold text-fg">
              {brand.company_name}
            </div>
            <div className="text-xs text-fg-3">Operations console</div>
          </div>
        </div>

        <div className="flex flex-1 items-center justify-center py-8">
          <div className="w-full max-w-[520px] animate-[float_10s_ease-in-out_infinite]">
            <IsoScene blocks={HERO_CITY} scale={30} />
          </div>
        </div>

        <div className="max-w-md">
          <h2 className="text-[34px] leading-[1.1] font-semibold text-fg">
            {PRODUCT_SLOGAN.split(". ")[0]}.
            <br />
            <span className="text-fg-3">{PRODUCT_SLOGAN.split(". ")[1]}</span>
          </h2>
          <p className="mt-3 text-[15px] text-fg-2">
            Properties, residents, maintenance and money in one live view.
          </p>
          <PoweredBy className="mt-8" />
        </div>
      </section>

      <section className="flex items-center justify-center px-5 py-10 sm:px-8">
        <div className="w-full max-w-[400px]">
          <div className="mb-8 flex items-center gap-3 lg:hidden">
            <BrandLogo brand={brand} size={34} />
            <div className="font-display text-[15px] font-semibold text-fg">
              {brand.company_name}
            </div>
          </div>
          <Panel className="p-7 sm:p-8">{children}</Panel>
          <PoweredBy className="mt-6 flex justify-center lg:hidden" />
        </div>
      </section>
    </main>
  );
}
