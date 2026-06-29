"use client";

import { useEffect } from "react";
import { AlertTriangle } from "lucide-react";
import { Button } from "@/components/ui/button";

export default function ConsoleError({
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
    <div className="rounded-2xl border border-line bg-surface p-8 text-center shadow-acre">
      <div className="mx-auto mb-4 flex h-12 w-12 items-center justify-center rounded-xl bg-bad-soft text-bad">
        <AlertTriangle className="h-6 w-6" />
      </div>
      <h2 className="font-display text-lg font-bold text-ink">
        Couldn’t load this view
      </h2>
      <p className="mx-auto mt-1.5 max-w-md text-sm text-ink-2">
        {error.message || "An unexpected error occurred while loading data."}
      </p>
      <div className="mt-6">
        <Button onClick={reset}>Try again</Button>
      </div>
    </div>
  );
}
