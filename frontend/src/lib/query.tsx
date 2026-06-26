"use client";

// TanStack Query setup. A single QueryClient is created per browser session
// (memoised in a ref so Fast Refresh / re-renders don't recreate it) with
// conservative defaults suited to an authenticated dashboard. The provider is
// mounted at the very top of app/providers.tsx so every client component —
// including AuthProvider-dependent ones — can use hooks from queries.ts.

import { useState } from "react";
import {
  QueryClient,
  QueryClientProvider,
  type QueryClientConfig,
} from "@tanstack/react-query";

export const queryClientConfig: QueryClientConfig = {
  defaultOptions: {
    queries: {
      staleTime: 30_000, // 30s — data is "fresh" briefly to avoid refetch storms
      retry: 1, // one retry, then surface the error
      refetchOnWindowFocus: false, // don't refetch every time the tab regains focus
    },
  },
};

export function makeQueryClient() {
  return new QueryClient(queryClientConfig);
}

export function QueryProvider({ children }: { children: React.ReactNode }) {
  // Lazy-init keeps one client for the component's lifetime.
  const [client] = useState(makeQueryClient);
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}
