import type { Metadata } from "next";
import { RootDocument } from "@/theme/RootDocument";
import { hudPreference, resolveGate } from "@/theme/gate";

export async function generateMetadata(): Promise<Metadata> {
  const { brand } = await resolveGate();
  return {
    title: {
      default: `Console · ${brand.company_name}`,
      template: `%s · ${brand.company_name}`,
    },
    robots: { index: false, follow: false },
  };
}

/** Staff console root: always Obsidian, whatever the host's audience. */
export default async function ConsoleRootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const [gate, hud] = await Promise.all([resolveGate(), hudPreference()]);
  return (
    <RootDocument theme="obsidian" gate={gate} hud={hud}>
      {children}
    </RootDocument>
  );
}
