import Link from "next/link";
import { Compass } from "lucide-react";
import { Button } from "@/components/ui/button";

export default function NotFound() {
  return (
    <div className="flex min-h-screen items-center justify-center bg-bg p-6">
      <div className="w-full max-w-md text-center">
        <div className="mx-auto mb-4 flex h-12 w-12 items-center justify-center rounded-xl bg-accent-soft text-accent-2">
          <Compass className="h-6 w-6" />
        </div>
        <div className="font-mono text-sm font-semibold text-ink-3">404</div>
        <h1 className="mt-1 font-display text-2xl font-bold tracking-tight text-ink">
          Page not found
        </h1>
        <p className="mt-1.5 text-sm text-ink-2">
          The page you’re looking for doesn’t exist or has moved.
        </p>
        <div className="mt-6 flex justify-center gap-2">
          <Button asChild variant="outline">
            <Link href="/">Home</Link>
          </Button>
          <Button asChild>
            <Link href="/console">Go to console</Link>
          </Button>
        </div>
      </div>
    </div>
  );
}
