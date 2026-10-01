"use client";

// Federated-login landing (issue #63). The provider (or the sandbox) redirects
// the browser here with ?provider&code&state; we complete the exchange and
// either apply the session, run the MFA step-up, or confirm a link. Alpha SSO
// arrives as ?sso=alpha&token=….

import { Suspense, useEffect, useRef, useState } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import { CircleAlert, LoaderCircle } from "lucide-react";
import { toast } from "sonner";
import { useAuth } from "@/lib/auth";
import { api, type MfaChallenge } from "@/lib/api";
import { sso } from "@/lib/sso";
import { AuthFrame } from "@/components/gate/AuthFrame";
import { MfaForm } from "@/components/gate/MfaForm";
import { Button } from "@/components/ui/button";

function Callback() {
  const params = useSearchParams();
  const router = useRouter();
  const { establishSession } = useAuth();
  const ran = useRef(false);
  const [error, setError] = useState<string | null>(null);
  const [challenge, setChallenge] = useState<MfaChallenge | null>(null);

  useEffect(() => {
    if (ran.current) return;
    ran.current = true;

    const ssoToken = params.get("sso") === "alpha" ? params.get("token") : null;
    if (ssoToken) {
      sso
        .signIn(ssoToken)
        .then((res) => {
          if (res.outcome === "session" && res.session) {
            establishSession(res.session);
            router.replace("/console");
          } else if (res.outcome === "mfa" && res.mfa) {
            setChallenge(res.mfa);
          } else {
            throw new Error("Unexpected response from sign-in.");
          }
        })
        .catch(() =>
          setError(
            "This sign-in link is not valid or has expired. Open it again from Alpha, or sign in here."
          )
        );
      return;
    }

    const provider = params.get("provider") ?? "";
    const code = params.get("code") ?? "";
    const state = params.get("state") ?? "";
    (async () => {
      if (!provider || !code || !state)
        throw new Error("Missing callback parameters.");
      const res = await api.oauthCallback(provider, code, state);
      if (res.outcome === "session" && res.session) {
        establishSession(res.session);
        router.replace("/console");
      } else if (res.outcome === "mfa" && res.mfa) {
        setChallenge(res.mfa);
      } else if (res.outcome === "linked") {
        toast.success(`Linked your ${res.provider ?? "account"}`);
        router.replace("/console/security");
      } else {
        throw new Error("Unexpected response from the sign-in provider.");
      }
    })().catch((e) =>
      setError(e instanceof Error ? e.message : "Sign-in failed")
    );
  }, [params, router, establishSession]);

  if (challenge) {
    return (
      <MfaForm
        challenge={challenge}
        onVerified={(tokens) => {
          establishSession(tokens);
          router.replace("/console");
        }}
      />
    );
  }

  if (error) {
    return (
      <>
        <div className="mb-5 flex size-11 items-center justify-center rounded-2xl border border-bad/30 bg-bad/10 text-bad">
          <CircleAlert className="size-5" />
        </div>
        <h1 className="text-2xl font-semibold text-fg">
          Sign-in didn&apos;t finish
        </h1>
        <p className="mt-2 mb-6 text-[13px] text-fg-2">{error}</p>
        <Button
          size="lg"
          className="w-full"
          onClick={() => router.replace("/login")}
        >
          Back to sign in
        </Button>
      </>
    );
  }

  return <Completing />;
}

function Completing() {
  return (
    <div className="flex items-center gap-3 py-6 text-[13px] text-fg-2">
      <LoaderCircle className="size-4 animate-spin text-accent" />
      Completing sign-in…
    </div>
  );
}

export default function OauthCallbackPage() {
  return (
    <AuthFrame>
      <Suspense fallback={<Completing />}>
        <Callback />
      </Suspense>
    </AuthFrame>
  );
}
