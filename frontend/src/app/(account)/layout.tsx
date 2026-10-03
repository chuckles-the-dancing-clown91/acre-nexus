import type { Metadata } from "next";
import { RootDocument } from "@/theme/RootDocument";
import { hudPreference, resolveGate } from "@/theme/gate";

export async function generateMetadata(): Promise<Metadata> {
  const { brand } = await resolveGate();
  return {
    title: {
      default: `Your home · ${brand.company_name}`,
      template: `%s · ${brand.company_name}`,
    },
    robots: { index: false, follow: false },
  };
}

/** Resident pages: always Daylight, in the landlord's brand. */
export default async function AccountRootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const [gate, hud] = await Promise.all([resolveGate(), hudPreference()]);
  return (
    <RootDocument theme="daylight" gate={gate} hud={hud}>
      {children}
    </RootDocument>
  );
}
