"use client";

// The resident's mobile ID card: who they are, where they live, and a code
// the office can scan. Looks like a card on a phone and in the console.

import { useEffect, useState } from "react";
import QRCode from "qrcode";
import { BadgeCheck, PawPrint, Phone } from "lucide-react";
import type { IdCard } from "@/lib/api";
import { cn } from "@/lib/utils";

function initials(name: string) {
  return name
    .split(" ")
    .map((w) => w[0])
    .filter(Boolean)
    .slice(0, 2)
    .join("")
    .toUpperCase();
}

function month(d: string | null | undefined) {
  if (!d) return null;
  const t = new Date(`${d}T00:00:00`);
  return Number.isNaN(t.getTime())
    ? d
    : t.toLocaleDateString(undefined, { month: "short", year: "numeric" });
}

export function IdCardView({
  card,
  className,
}: {
  card: IdCard;
  className?: string;
}) {
  const [qr, setQr] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    QRCode.toDataURL(`RES:${card.code}`, {
      margin: 1,
      width: 240,
      color: { dark: "#0f172a", light: "#ffffff" },
    })
      .then((u) => live && setQr(u))
      .catch(() => live && setQr(null));
    return () => {
      live = false;
    };
  }, [card.code]);

  const home = card.residence;
  return (
    <div
      className={cn(
        "relative mx-auto w-full max-w-[380px] overflow-hidden rounded-3xl bg-gradient-to-br from-accent to-accent/70 p-5 text-white shadow-xl",
        className
      )}
    >
      <div
        aria-hidden
        className="pointer-events-none absolute -top-16 -right-12 size-52 rounded-full bg-white/10"
      />
      <div className="relative flex items-center justify-between text-[11px] font-semibold tracking-[0.14em] uppercase">
        <span>{card.company || "Resident"}</span>
        <span className="rounded-full bg-white/20 px-2.5 py-0.5 tracking-normal normal-case">
          {card.standing === "active" ? "Current resident" : "Resident"}
        </span>
      </div>

      <div className="relative mt-5 flex items-center gap-4">
        {card.photo_url ? (
          // eslint-disable-next-line @next/next/no-img-element
          <img
            src={card.photo_url}
            alt=""
            className="size-20 rounded-2xl border-2 border-white/60 object-cover"
          />
        ) : (
          <span className="flex size-20 shrink-0 items-center justify-center rounded-2xl border-2 border-white/40 bg-white/15 text-2xl font-semibold">
            {initials(card.name)}
          </span>
        )}
        <div className="min-w-0">
          <div className="truncate text-[20px] leading-tight font-semibold">
            {card.name}
          </div>
          <div className="mt-0.5 flex items-center gap-1 text-[12px] text-white/80">
            {card.screening_cleared && (
              <>
                <BadgeCheck className="size-3.5" /> Screening cleared
              </>
            )}
          </div>
          {card.phone && (
            <div className="mt-1 flex items-center gap-1 text-[12px] text-white/80">
              <Phone className="size-3.5" /> {card.phone}
            </div>
          )}
        </div>
      </div>

      <div className="relative mt-5 rounded-2xl bg-white/15 p-3 text-[13px]">
        {home ? (
          <>
            <div className="text-[11px] tracking-wide text-white/70 uppercase">
              Residence
            </div>
            {home.address
              .toLowerCase()
              .startsWith(home.property.toLowerCase()) ? (
              <div className="font-medium">
                {home.address}
                {home.unit ? `, Unit ${home.unit}` : ""}
              </div>
            ) : (
              <>
                <div className="font-medium">
                  {home.property}
                  {home.unit ? `, Unit ${home.unit}` : ""}
                </div>
                <div className="text-white/80">{home.address}</div>
              </>
            )}
            <div className="mt-1 text-[12px] text-white/70">
              Lease {month(home.lease_start)} to{" "}
              {home.lease_end ? month(home.lease_end) : "month to month"}
            </div>
          </>
        ) : (
          <div className="text-white/80">No active lease.</div>
        )}
      </div>

      <div className="relative mt-4 flex items-end justify-between gap-4">
        <div className="min-w-0 space-y-1 text-[12px] text-white/85">
          {card.resident_since && (
            <div>Resident since {month(card.resident_since)}</div>
          )}
          {card.pets.length > 0 && (
            <div className="flex items-center gap-1">
              <PawPrint className="size-3.5 shrink-0" />
              <span className="truncate">{card.pets.length} on file</span>
            </div>
          )}
          <div className="font-mono text-[13px] tracking-wider text-white">
            {card.code}
          </div>
        </div>
        <div className="shrink-0 rounded-xl bg-white p-1.5">
          {qr ? (
            // eslint-disable-next-line @next/next/no-img-element
            <img
              src={qr}
              alt={`QR code for ${card.code}`}
              className="size-24"
            />
          ) : (
            <div className="size-24" />
          )}
        </div>
      </div>
    </div>
  );
}
