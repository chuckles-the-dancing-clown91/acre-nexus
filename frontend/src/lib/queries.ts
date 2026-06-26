"use client";

// Typed TanStack Query hooks wrapping the existing `api` client. These are the
// canonical way to read server state from client components — prefer them over
// ad-hoc useEffect + useState. Query keys are centralised in `queryKeys` so
// they can be invalidated consistently after mutations.

import {
  useMutation,
  useQuery,
  useQueryClient,
  type UseQueryOptions,
} from "@tanstack/react-query";
import {
  api,
  type CreateTokenResponse,
  type ModuleInfo,
  type TokenSummary,
} from "./api";
import { tokenStore } from "./api";
import type { Application, PortfolioSummary, Property } from "./types";

/** Centralised, hierarchical query keys. */
export const queryKeys = {
  modules: ["modules"] as const,
  properties: ["properties"] as const,
  property: (id: string) => ["properties", id] as const,
  portfolioSummary: ["portfolio", "summary"] as const,
  applications: ["applications"] as const,
  apiTokens: ["api-tokens"] as const,
};

/** True when there's an access token to authenticate console requests. */
function isAuthed() {
  return !!tokenStore.access;
}

type QueryOpts<T> = Omit<
  UseQueryOptions<T, Error, T, readonly unknown[]>,
  "queryKey" | "queryFn"
>;

export function useModules(opts?: QueryOpts<ModuleInfo[]>) {
  return useQuery({
    queryKey: queryKeys.modules,
    queryFn: () => api.modules(),
    enabled: isAuthed(),
    ...opts,
  });
}

export function useProperties(opts?: QueryOpts<Property[]>) {
  return useQuery({
    queryKey: queryKeys.properties,
    queryFn: () => api.properties(),
    enabled: isAuthed(),
    ...opts,
  });
}

export function usePortfolioSummary(opts?: QueryOpts<PortfolioSummary>) {
  return useQuery({
    queryKey: queryKeys.portfolioSummary,
    queryFn: () => api.portfolioSummary(),
    enabled: isAuthed(),
    ...opts,
  });
}

export function useApplications(opts?: QueryOpts<Application[]>) {
  return useQuery({
    queryKey: queryKeys.applications,
    queryFn: () => api.applications(),
    enabled: isAuthed(),
    ...opts,
  });
}

export function useApiTokens(opts?: QueryOpts<TokenSummary[]>) {
  return useQuery({
    queryKey: queryKeys.apiTokens,
    queryFn: () => api.apiTokens(),
    enabled: isAuthed(),
    ...opts,
  });
}

/** Create an API token, then invalidate the token list. */
export function useCreateApiToken() {
  const qc = useQueryClient();
  return useMutation<
    CreateTokenResponse,
    Error,
    { name: string; scopes: string[] }
  >({
    mutationFn: ({ name, scopes }) => api.createApiToken(name, scopes),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.apiTokens });
    },
  });
}

/** Revoke an API token, then invalidate the token list. */
export function useRevokeApiToken() {
  const qc = useQueryClient();
  return useMutation<void, Error, string>({
    mutationFn: (id) => api.revokeApiToken(id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.apiTokens });
    },
  });
}
