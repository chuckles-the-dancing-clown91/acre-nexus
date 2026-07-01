// Minimal client-side error logging. The app has no telemetry SDK wired up
// (no Sentry/PostHog/etc.) — this is the single place to add one later, so
// "best-effort, non-fatal" data loads (secondary panels, background refreshes)
// stop failing completely silently and instead leave a trace in the console,
// mirroring the backend's `tracing::error!` best-effort logging.

/** Log a non-fatal error with context. Never throws. */
export function logError(context: string, err: unknown): void {
  console.error(`[acre] ${context}:`, err);
}
