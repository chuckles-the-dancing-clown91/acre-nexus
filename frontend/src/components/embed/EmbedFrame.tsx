"use client";

// The shell around every embeddable widget: applies the client's brand,
// checks the embedding site against the allowed list, and tells the host page
// how tall it is. Content is public, so the site check is a courtesy to the
// client (their widgets, their sites), not a secret.

import { Suspense, useEffect, useRef, useState } from "react";
import { useSearchParams } from "next/navigation";
import { api, DEFAULT_TENANT } from "@/lib/api";
import { business } from "@/lib/business";

const HEX = /^#[0-9a-fA-F]{6}$/;

/** The site framing us: Chrome's ancestor list, else the referrer's origin. */
function parentOrigin(): string | null {
  try {
    const a = window.location.ancestorOrigins?.[0];
    if (a) return a;
    if (document.referrer) return new URL(document.referrer).origin;
  } catch {
    /* ignore */
  }
  return null;
}

function Frame({
  children,
}: {
  children: (p: { tenant: string }) => React.ReactNode;
}) {
  const params = useSearchParams();
  const tenant = params.get("tenant") || DEFAULT_TENANT;
  const id = params.get("id") ?? "";
  const accent = params.get("accent") ?? "";
  const dark = params.get("mode") === "dark";
  const [state, setState] = useState<"checking" | "ok" | "off" | "blocked">(
    "checking"
  );
  const box = useRef<HTMLDivElement>(null);

  useEffect(() => {
    document.documentElement.classList.toggle("dark", dark);
    document.body.style.background = "transparent";
    api
      .publicTheme(tenant)
      .then((t) => {
        const c = HEX.test(accent) ? accent : t.accent_color || t.primary_color;
        if (c) {
          document.documentElement.style.setProperty("--accent", c);
          document.documentElement.style.setProperty("--accent-2", c);
        }
      })
      .catch(() => {});
    business
      .embedConfig(tenant)
      .then((cfg) => {
        if (!cfg.enabled) return setState("off");
        const framed = window.parent !== window;
        const where = parentOrigin();
        if (
          framed &&
          cfg.allowed_origins.length > 0 &&
          where &&
          !cfg.allowed_origins.includes(where)
        )
          return setState("blocked");
        setState("ok");
      })
      .catch(() => setState("ok"));
  }, [tenant, accent, dark]);

  // Tell the host page how tall we are, now and whenever it changes.
  useEffect(() => {
    const el = box.current;
    if (!el || window.parent === window) return;
    const send = () =>
      window.parent.postMessage(
        { type: "vantedge:height", id, height: el.scrollHeight },
        "*"
      );
    const ro = new ResizeObserver(send);
    ro.observe(el);
    send();
    return () => ro.disconnect();
  }, [id, state]);

  return (
    <div ref={box} className="p-2">
      {state === "ok" && children({ tenant })}
      {state === "off" && (
        <p className="text-sm text-ink-3">This widget is turned off.</p>
      )}
      {state === "blocked" && (
        <p className="text-sm text-ink-3">
          This site is not set up to show this widget. Ask the owner to add it
          in Vantedge settings.
        </p>
      )}
    </div>
  );
}

export function EmbedFrame({
  children,
}: {
  children: (p: { tenant: string }) => React.ReactNode;
}) {
  return (
    <Suspense fallback={null}>
      <Frame>{children}</Frame>
    </Suspense>
  );
}
