import type { Metadata } from "next";
import "./globals.css";
import { Providers } from "./providers";
import { currentTenant, loadSite, siteOrigin } from "@/lib/seo";
import { homeDescription, homeTitle } from "@/lib/seo-schema";

// Per-workspace defaults: the site's own name and origin, so every page
// inherits a title template, the canonical base and the Search Console tag.
export async function generateMetadata(): Promise<Metadata> {
  const tenant = await currentTenant();
  const [site, origin] = await Promise.all([loadSite(tenant), siteOrigin()]);
  return {
    metadataBase: new URL(origin),
    title: {
      default: homeTitle(site),
      template: `%s | ${site.company_name}`,
    },
    description: homeDescription(site, 0),
    applicationName: site.company_name,
    verification: site.google_site_verification
      ? { google: site.google_site_verification }
      : undefined,
    openGraph: { siteName: site.company_name, type: "website" },
  };
}

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en" suppressHydrationWarning>
      <head>
        <link rel="preconnect" href="https://fonts.googleapis.com" />
        <link
          rel="preconnect"
          href="https://fonts.gstatic.com"
          crossOrigin="anonymous"
        />
        <link
          href="https://fonts.googleapis.com/css2?family=Bricolage+Grotesque:opsz,wght@12..96,500;12..96,600;12..96,700;12..96,800&family=Hanken+Grotesk:wght@400;500;600;700&family=Space+Mono:wght@400;700&display=swap"
          rel="stylesheet"
        />
      </head>
      <body>
        <Providers>{children}</Providers>
      </body>
    </html>
  );
}
