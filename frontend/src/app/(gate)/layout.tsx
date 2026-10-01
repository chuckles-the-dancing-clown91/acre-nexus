import type { Metadata } from "next";
import { RootDocument } from "@/theme/RootDocument";
import { hudPreference, resolveGate } from "@/theme/gate";
import { AUDIENCE_THEME } from "@/theme/themes";

export async function generateMetadata(): Promise<Metadata> {
  const { brand } = await resolveGate();
  return {
    title: {
      default: brand.company_name,
      template: `%s · ${brand.company_name}`,
    },
    robots: { index: false, follow: false },
  };
}

/**
 * Sign-in, password, and auth-callback pages. Their theme follows the host's
 * audience: an admin domain gets Obsidian, owner/renter portals get Daylight.
 */
export default async function GateRootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const [gate, hud] = await Promise.all([resolveGate(), hudPreference()]);
  return (
    <RootDocument theme={AUDIENCE_THEME[gate.audience]} gate={gate} hud={hud}>
      {children}
    </RootDocument>
  );
}
