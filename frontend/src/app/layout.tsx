import type { Metadata } from "next";
import { GeistSans } from "geist/font/sans";
import { GeistMono } from "geist/font/mono";
import "./globals.css";
import { Providers } from "./providers";

export const metadata: Metadata = {
  title: "Acre — Property Operations",
  description:
    "Acre is the operating system for property-management companies: portfolio operations, leasing, maintenance, entities, and a multi-tenant platform console.",
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    // suppressHydrationWarning is required by next-themes (it sets the theme
    // class on <html> from a pre-paint inline script). Geist is self-hosted via
    // next/font — no runtime font CDN, no layout shift.
    <html
      lang="en"
      suppressHydrationWarning
      className={`${GeistSans.variable} ${GeistMono.variable}`}
    >
      <body>
        <Providers>{children}</Providers>
      </body>
    </html>
  );
}
