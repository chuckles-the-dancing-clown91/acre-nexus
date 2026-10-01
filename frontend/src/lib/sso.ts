// Single sign-on with Alpha.

import { request } from "@/lib/api";
import type { OauthCallbackResult } from "@/lib/api";

export interface SsoStatus {
  enabled: boolean;
  audience: string;
  issuer: string;
  tenant: string;
  login_url: string;
}

export const sso = {
  status: () => request<SsoStatus>("/sso/alpha", { auth: true }),
  enable: (rotate = false) =>
    request<SsoStatus & { secret: string }>("/sso/alpha/enable", {
      method: "POST",
      auth: true,
      body: { rotate },
    }),
  disable: () =>
    request<SsoStatus>("/sso/alpha", { method: "DELETE", auth: true }),
  launch: (body: {
    counterparty_id: string;
    web_url?: string;
    next?: string;
  }) =>
    request<{ url: string }>("/sso/alpha/launch", {
      method: "POST",
      auth: true,
      body,
    }),
  signIn: (token: string) =>
    request<OauthCallbackResult>("/auth/sso/alpha", {
      method: "POST",
      body: { token },
    }),
};
