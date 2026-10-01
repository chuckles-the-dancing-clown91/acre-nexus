import { ModulesProvider } from "@/lib/modules";
import { ConsoleShell } from "@/components/shell/ConsoleShell";

export default function ConsoleLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <ModulesProvider>
      <ConsoleShell>{children}</ConsoleShell>
    </ModulesProvider>
  );
}
