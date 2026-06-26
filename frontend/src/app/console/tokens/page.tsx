"use client";

import { useEffect, useState } from "react";
import { api, API_BASE, type CreateTokenResponse, type TokenSummary } from "@/lib/api";
import { Badge, Button, Card } from "@/components/ui";

const SCOPE_OPTIONS = [
  "listing:read",
  "property:read",
  "application:read",
];

export default function TokensPage() {
  const [tokens, setTokens] = useState<TokenSummary[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [name, setName] = useState("");
  const [scopes, setScopes] = useState<string[]>(["listing:read"]);
  const [created, setCreated] = useState<CreateTokenResponse | null>(null);

  function load() {
    api.apiTokens().then(setTokens).catch((e) => setError(e.message));
  }
  useEffect(load, []);

  async function create() {
    if (!name) return;
    try {
      const res = await api.createApiToken(name, scopes);
      setCreated(res);
      setName("");
      load();
    } catch (e: any) {
      setError(e.message);
    }
  }

  async function revoke(id: string) {
    await api.revokeApiToken(id);
    load();
  }

  return (
    <div className="space-y-6">
      <div>
        <h1 className="font-display text-3xl font-extrabold tracking-tight">
          API tokens
        </h1>
        <p className="text-ink-3">
          Scoped, revocable keys for the vendor API (<code>{API_BASE}/api/v1</code>).
        </p>
      </div>

      {error && <p className="text-bad">{error}</p>}

      {created && (
        <Card className="border-good-soft bg-good-soft p-5">
          <p className="mb-2 font-bold text-good">
            Token created — copy it now, it won&apos;t be shown again:
          </p>
          <code className="block break-all rounded-lg bg-surface px-3 py-2 font-mono text-sm">
            {created.token}
          </code>
        </Card>
      )}

      <Card className="p-5">
        <h2 className="mb-3 font-display text-lg font-bold">Create token</h2>
        <input
          placeholder="Token name (e.g. Zillow sync)"
          value={name}
          onChange={(e) => setName(e.target.value)}
          className="mb-3 w-full rounded-xl border border-line bg-surface-2 px-3 py-2.5 text-sm outline-none focus:border-accent"
        />
        <div className="mb-4 flex flex-wrap gap-2">
          {SCOPE_OPTIONS.map((s) => {
            const on = scopes.includes(s);
            return (
              <button
                key={s}
                onClick={() =>
                  setScopes((cur) =>
                    on ? cur.filter((x) => x !== s) : [...cur, s]
                  )
                }
                className={`rounded-full border px-3 py-1.5 font-mono text-xs font-bold ${
                  on
                    ? "border-accent bg-accent-soft text-accent-2"
                    : "border-line-2 text-ink-3"
                }`}
              >
                {s}
              </button>
            );
          })}
        </div>
        <Button onClick={create}>Create token</Button>
      </Card>

      <Card className="overflow-hidden">
        <div className="border-b border-line px-5 py-4 font-display text-lg font-bold">
          Active tokens
        </div>
        <div className="divide-y divide-line">
          {tokens?.map((t) => (
            <div key={t.id} className="flex items-center gap-4 px-5 py-3.5">
              <div className="min-w-0 flex-1">
                <div className="font-semibold">{t.name}</div>
                <code className="font-mono text-xs text-ink-3">{t.prefix}…</code>
              </div>
              <div className="hidden gap-1 sm:flex">
                {t.scopes.map((s) => (
                  <Badge key={s} tone="neutral">
                    {s}
                  </Badge>
                ))}
              </div>
              {t.revoked ? (
                <Badge tone="bad">Revoked</Badge>
              ) : (
                <button
                  onClick={() => revoke(t.id)}
                  className="text-sm font-semibold text-bad hover:underline"
                >
                  Revoke
                </button>
              )}
            </div>
          ))}
          {tokens && tokens.length === 0 && (
            <div className="px-5 py-10 text-center text-ink-3">
              No tokens yet.
            </div>
          )}
        </div>
      </Card>
    </div>
  );
}
