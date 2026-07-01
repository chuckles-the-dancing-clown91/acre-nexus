"use client";

// Root-layout error boundary. `error.tsx` doesn't catch failures in the root
// layout itself (providers, fonts, etc.) — only `global-error.tsx` does, and it
// must render its own <html>/<body> since the layout that would have is what
// failed. Kept deliberately plain (no theme/provider dependency) since we can't
// assume anything above this rendered.

import { useEffect } from "react";
import { logError } from "@/lib/log";

export default function GlobalError({
  error,
  reset,
}: {
  error: Error & { digest?: string };
  reset: () => void;
}) {
  useEffect(() => {
    logError("unhandled root layout error", error);
  }, [error]);

  return (
    <html lang="en">
      <body
        style={{
          display: "flex",
          minHeight: "100vh",
          alignItems: "center",
          justifyContent: "center",
          fontFamily: "system-ui, sans-serif",
          padding: "1.5rem",
        }}
      >
        <div style={{ maxWidth: 420, textAlign: "center" }}>
          <h1 style={{ fontSize: "1.25rem", fontWeight: 700 }}>
            Something went wrong
          </h1>
          <p style={{ color: "#666", margin: "0.75rem 0 1.25rem" }}>
            An unexpected error occurred. It&apos;s been logged — try reloading
            the page.
          </p>
          <button
            onClick={reset}
            style={{
              borderRadius: "0.75rem",
              padding: "0.5rem 1.25rem",
              fontWeight: 700,
              background: "#111",
              color: "#fff",
              border: "none",
              cursor: "pointer",
            }}
          >
            Try again
          </button>
        </div>
      </body>
    </html>
  );
}
