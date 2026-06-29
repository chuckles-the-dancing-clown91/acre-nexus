import { NextResponse } from "next/server";
import type { NextRequest } from "next/server";

/**
 * Edge route guard. Reads the `acre.session` presence cookie (set by the API
 * client on login/refresh) to gate the console BEFORE render — eliminating the
 * unauthenticated content flash the old client-only `useEffect` redirect caused.
 *
 * This is a presence check, not authorization: every API call is still verified
 * by the JWT, and the backend is authoritative. Fine-grained gating (platform
 * vs tenant, per-permission) happens in the console shell + the backend.
 */
const SESSION_COOKIE = "acre.session";

export function middleware(req: NextRequest) {
  const hasSession = req.cookies.has(SESSION_COOKIE);
  const { pathname } = req.nextUrl;

  if (pathname.startsWith("/console") && !hasSession) {
    const url = req.nextUrl.clone();
    url.pathname = "/login";
    url.searchParams.set("next", pathname);
    return NextResponse.redirect(url);
  }

  // Signed in already → keep them out of the login screen.
  if (pathname === "/login" && hasSession) {
    const url = req.nextUrl.clone();
    url.pathname = "/console";
    url.search = "";
    return NextResponse.redirect(url);
  }

  return NextResponse.next();
}

export const config = {
  matcher: ["/console/:path*", "/login"],
};
