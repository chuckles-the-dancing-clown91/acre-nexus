// robots.txt: crawl the public site, keep the console and sign-in pages out.

import type { MetadataRoute } from "next";
import { siteOrigin } from "@/lib/seo";

export default async function robots(): Promise<MetadataRoute.Robots> {
  const origin = await siteOrigin();
  return {
    rules: [
      {
        userAgent: "*",
        allow: ["/", "/listings/"],
        disallow: [
          "/console",
          "/embed",
          "/login",
          "/auth",
          "/account",
          "/set-password",
          "/forgot-password",
          "/sign",
        ],
      },
    ],
    sitemap: `${origin}/sitemap.xml`,
    host: origin,
  };
}
