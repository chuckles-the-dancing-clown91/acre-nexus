import type { OauthProvider } from "./api";

/** Display names for the social sign-in providers the API knows. */
const PROVIDER_LABELS: Record<string, string> = {
  google: "Google",
  microsoft: "Microsoft",
  apple: "Apple",
};

/**
 * The social sign-in buttons to show: only the providers the server reports
 * as available (live, or the sandbox on a non-production server), in the
 * server's order. Unknown keys are dropped; nothing is shown until it answers.
 */
export function socialButtons(
  providers: OauthProvider[] | undefined
): { key: string; label: string }[] {
  return (providers ?? [])
    .filter((p) => p.key in PROVIDER_LABELS)
    .map((p) => ({ key: p.key, label: PROVIDER_LABELS[p.key] }));
}

/**
 * Whether the login page lists the seeded demo accounts. A build-time flag
 * (`NEXT_PUBLIC_SHOW_DEMO_ACCOUNTS=1`), so production builds — which never set
 * it — don't advertise them. Default: hidden.
 */
export function demoAccountsEnabled(
  flag: string | undefined = process.env.NEXT_PUBLIC_SHOW_DEMO_ACCOUNTS
): boolean {
  const v = (flag ?? "").trim().toLowerCase();
  return v === "1" || v === "true";
}
