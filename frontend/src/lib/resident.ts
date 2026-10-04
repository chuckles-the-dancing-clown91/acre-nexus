// Who is a resident: someone whose only footing in the workspace is a
// lease. They get the resident pages (/account), not the staff console.

import type { User } from "./types";
import { isOwnerOnly } from "@/lib/owner";

/** True when every membership the user holds is a renter's. Staff, owners
 * and anyone with a second role keep the console. */
export function isResidentOnly(
  user: Pick<User, "is_platform_staff" | "memberships"> | null | undefined
): boolean {
  if (!user || user.is_platform_staff) return false;
  const active = user.memberships.filter((m) => m.status !== "revoked");
  return active.length > 0 && active.every((m) => m.profile_type === "renter");
}

/** Signed in only as a vendor of a workspace (the vendor portal). */
export function isVendorOnly(
  user: Pick<User, "is_platform_staff" | "memberships"> | null | undefined
): boolean {
  if (!user || user.is_platform_staff) return false;
  const active = user.memberships.filter((m) => m.status !== "revoked");
  return active.length > 0 && active.every((m) => m.profile_type === "vendor");
}

/** Status words a resident reads. */
export function residentStatus(status: string, waitingOn?: string | null) {
  switch (status) {
    case "open":
    case "triage":
      return "Received";
    case "scheduled":
      return "Scheduled";
    case "in_progress":
      return "In progress";
    case "on_hold":
      return waitingOn === "resident"
        ? "Waiting on you"
        : waitingOn === "parts"
          ? "Waiting on parts"
          : "On hold";
    case "resolved":
      return "Done";
    case "closed":
      return "Closed";
    case "cancelled":
      return "Cancelled";
    default:
      return status.replace(/_/g, " ");
  }
}

/** The request a text or email link opens with: `?new=1&title=..`. */
export function prefillFrom(params: URLSearchParams): {
  title: string;
  category: string;
  description: string;
} | null {
  if (params.get("new") !== "1") return null;
  return {
    title: (params.get("title") ?? "").slice(0, 200),
    category: params.get("category") ?? "",
    description: (params.get("description") ?? "").slice(0, 4000),
  };
}

/** Where to land after sign-in: a same-site `?next=` path if one was
 * given, else the resident pages for a resident and the console for staff. */
export function landingFor(
  user: Pick<User, "is_platform_staff" | "memberships"> | null | undefined,
  search: string
): string {
  const next = new URLSearchParams(search).get("next");
  if (
    next &&
    next.startsWith("/") &&
    !next.startsWith("//") &&
    !next.startsWith("/\\")
  )
    return next;
  if (isOwnerOnly(user)) return "/account/owner";
  if (isVendorOnly(user)) return "/account/vendor";
  return isResidentOnly(user) ? "/account/maintenance" : "/console";
}
