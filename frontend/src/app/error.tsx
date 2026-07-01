"use client";

// App Router error boundary: catches render/render-effect exceptions anywhere
// under this segment (i.e. the whole app, since this file lives at the root)
// that a component didn't handle itself. Without this, an uncaught error blew
// away the entire page with Next's generic error screen and left no trace
// anywhere — now it's logged and the user gets a way back in.

import { useEffect } from "react";
import { logError } from "@/lib/log";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui";

export default function ErrorBoundary({
  error,
  reset,
}: {
  error: Error & { digest?: string };
  reset: () => void;
}) {
  useEffect(() => {
    logError("unhandled render error", error);
  }, [error]);

  return (
    <div className="flex min-h-screen items-center justify-center p-6">
      <Card className="max-w-md space-y-4 p-6 text-center">
        <h1 className="font-display text-xl font-bold">Something went wrong</h1>
        <p className="text-sm text-ink-3">
          An unexpected error occurred. It&apos;s been logged — try again, or
          reload the page if it keeps happening.
        </p>
        <Button onClick={reset}>Try again</Button>
      </Card>
    </div>
  );
}
