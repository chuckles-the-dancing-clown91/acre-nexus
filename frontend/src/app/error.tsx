"use client";

import { useEffect } from "react";
import { AlertTriangle } from "lucide-react";
import { Button } from "@/components/ui/button";

export default function GlobalError({
  error,
  reset,
}: {
  error: Error & { digest?: string };
  reset: () => void;
}) {
  useEffect(() => {
    console.error(error);
  }, [error]);

  return (
    <div className="flex min-h-screen items-center justify-center bg-bg p-6">
      <div className="w-full max-w-md rounded-2xl border border-line bg-surface p-8 text-center shadow-acre">
        <div className="mx-auto mb-4 flex h-12 w-12 items-center justify-center rounded-xl bg-bad-soft text-bad">
          <AlertTriangle className="h-6 w-6" />
        </div>
        <h1 className="font-display text-lg font-bold text-ink">
          Something went wrong
        </h1>
        <p className="mt-1.5 text-sm text-ink-2">
          {error.message || "An unexpected error occurred."}
        </p>
        <div className="mt-6 flex justify-center gap-2">
          <Button variant="outline" onClick={() => (window.location.href = "/")}>
            Go home
          </Button>
          <Button onClick={reset}>Try again</Button>
        </div>
      </div>
    </div>
  );
}
