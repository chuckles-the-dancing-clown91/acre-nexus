"use client";

import dynamic from "next/dynamic";

/** The site map canvas, loaded in the browser only. */
export const SiteMapCanvas = dynamic(() => import("./SiteMapCanvas"), {
  ssr: false,
  loading: () => <div className="h-full w-full animate-pulse bg-surface-2" />,
});

export type { Tool } from "./SiteMapCanvas";
