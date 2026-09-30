// The centred card the sign-in pages share: brand, theme toggle, content.

import Link from "next/link";
import { Card } from "@/components/ui";
import { ThemeToggle } from "@/components/ThemeToggle";
import { Brand } from "@/components/Brand";

export function AuthShell({ children }: { children: React.ReactNode }) {
  return (
    <main className="flex min-h-screen items-center justify-center px-6">
      <div className="absolute right-5 top-5">
        <ThemeToggle />
      </div>
      <Card className="w-full max-w-md p-8">
        <Brand slogan className="mb-6" />
        {children}
        <Link
          href="/login"
          className="mt-6 block text-center text-sm font-semibold text-ink-3 hover:text-ink"
        >
          ← Back to sign in
        </Link>
      </Card>
    </main>
  );
}

export const authField =
  "w-full rounded-xl border border-line bg-surface-2 px-3.5 py-3 text-sm outline-none focus:border-accent";
