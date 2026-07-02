// Minimal stroke-icon set (Lucide-style paths) used across the app. Pluggable:
// add a path to `ICONS` and reference it by name.

export const ICONS: Record<string, string> = {
  globe:
    "M12 2a10 10 0 100 20 10 10 0 000-20zM2 12h20M12 2a15 15 0 010 20M12 2a15 15 0 000 20",
  user: "M20 21a8 8 0 10-16 0M12 11a4 4 0 100-8 4 4 0 000 8",
  building:
    "M3 21h18M5 21V5a2 2 0 012-2h10a2 2 0 012 2v16M9 7h0M9 11h0M9 15h0M15 7h0M15 11h0M15 15h0",
  wrench:
    "M14.7 6.3a4 4 0 00-5.4 5.4L3 18l3 3 6.3-6.3a4 4 0 005.4-5.4l-2.3 2.3-2.4-.6-.6-2.4 2.3-2.3z",
  dollar: "M12 1v22M17 5H9.5a3.5 3.5 0 000 7h5a3.5 3.5 0 010 7H6",
  chart: "M3 3v18h18M7 16l4-6 4 4 4-7",
  moon: "M21 12.8A8.5 8.5 0 1111.2 3a6.5 6.5 0 009.8 9.8z",
  sun: "M12 2v2M12 20v2M2 12h2M20 12h2M5 5l1.4 1.4M17.6 17.6L19 19M19 5l-1.4 1.4M6.4 17.6L5 19M16.5 12a4.5 4.5 0 11-9 0 4.5 4.5 0 019 0z",
  search: "M11 19a8 8 0 100-16 8 8 0 000 16zM21 21l-4-4",
  back: "M15 18l-6-6 6-6",
  check: "M20 6L9 17l-5-5",
  logout: "M9 21H5a2 2 0 01-2-2V5a2 2 0 012-2h4M16 17l5-5-5-5M21 12H9",
  shield: "M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z",
  key: "M21 2l-2 2m-7.6 7.6a5 5 0 11-7 7 5 5 0 017-7zm0 0L15 8l3 3 3-3-3-3",
  bell: "M18 8a6 6 0 00-12 0c0 7-3 9-3 9h18s-3-2-3-9M13.7 21a2 2 0 01-3.4 0",
};

export function Icon({
  name,
  size = 18,
  className,
  strokeWidth = 1.9,
}: {
  name: keyof typeof ICONS | string;
  size?: number;
  className?: string;
  strokeWidth?: number;
}) {
  const d = ICONS[name] ?? ICONS.building;
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={strokeWidth}
      strokeLinecap="round"
      strokeLinejoin="round"
      className={className}
      aria-hidden="true"
    >
      {d
        .split("M")
        .filter(Boolean)
        .map((seg, i) => (
          <path key={i} d={`M${seg}`} />
        ))}
    </svg>
  );
}
