"use client";

import { useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { AnimatePresence, motion } from "motion/react";
import { ArrowLeft, ArrowRight, CircleAlert, Eye, EyeOff } from "lucide-react";
import { useAuth } from "@/lib/auth";
import { api, isMfaChallenge, type MfaChallenge } from "@/lib/api";
import { AuthFrame } from "@/components/gate/AuthFrame";
import { MfaForm } from "@/components/gate/MfaForm";
import { Button } from "@/components/ui/button";
import { Field, Input } from "@/components/ui/input";
import { useTheme } from "@/theme/ThemeProvider";

const DEMO_ACCOUNTS = [
  { name: "Avery Stone", role: "Platform staff", email: "avery@acrehq.com" },
  {
    name: "Jordan Mills",
    role: "Northwind admin",
    email: "jordan@northwind.com",
  },
  { name: "Priya Rao", role: "Cascade admin", email: "priya@cascade.com" },
];

const PROVIDERS = [
  { key: "google", label: "Google" },
  { key: "microsoft", label: "Microsoft" },
  { key: "apple", label: "Apple" },
];

type Step =
  | { kind: "password" }
  | { kind: "mfa"; challenge: MfaChallenge }
  | { kind: "sandbox"; provider: string; url: string };

const slide = {
  initial: { opacity: 0, x: 12 },
  animate: { opacity: 1, x: 0 },
  exit: { opacity: 0, x: -12 },
  transition: { duration: 0.22, ease: [0.22, 1, 0.36, 1] as const },
};

export default function LoginPage() {
  const { login, establishSession } = useAuth();
  const { brand, gate } = useTheme();
  const router = useRouter();
  const [step, setStep] = useState<Step>({ kind: "password" });
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [sandboxEmail, setSandboxEmail] = useState("new.renter@example.com");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const res = await login(email.trim(), password);
      if (isMfaChallenge(res)) {
        setStep({ kind: "mfa", challenge: res });
        setBusy(false);
      } else {
        router.push("/console");
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Sign-in failed");
      setBusy(false);
    }
  }

  async function social(provider: string) {
    setError(null);
    try {
      const res = await api.oauthStart(provider, {
        intent: "login",
        tenant: gate.tenant,
      });
      if (res.sandbox)
        setStep({ kind: "sandbox", provider, url: res.authorize_url });
      else window.location.assign(res.authorize_url);
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Couldn't start social sign-in"
      );
    }
  }

  return (
    <AuthFrame>
      <AnimatePresence mode="wait" initial={false}>
        {step.kind === "mfa" ? (
          <motion.div key="mfa" {...slide}>
            <MfaForm
              challenge={step.challenge}
              onVerified={(tokens) => {
                establishSession(tokens);
                router.push("/console");
              }}
              onBack={() => setStep({ kind: "password" })}
            />
          </motion.div>
        ) : step.kind === "sandbox" ? (
          <motion.form
            key="sandbox"
            {...slide}
            onSubmit={(e) => {
              e.preventDefault();
              // eslint-disable-next-line @next/next/no-location-assign-relative-destination -- the API's sandbox authorize URL, not a page
              window.location.assign(
                `${step.url}&email=${encodeURIComponent(sandboxEmail)}`
              );
            }}
          >
            <h1 className="text-2xl font-semibold text-fg capitalize">
              {step.provider}
            </h1>
            <p className="mt-1.5 mb-6 text-[13px] text-fg-3">
              No live {step.provider} credentials are configured, so this is a
              simulated sign-in. Choose the email to sign in (or sign up) with.
            </p>
            <Field label="Email">
              {(p) => (
                <Input
                  {...p}
                  type="email"
                  value={sandboxEmail}
                  onChange={(e) => setSandboxEmail(e.target.value)}
                  autoFocus
                />
              )}
            </Field>
            <Button type="submit" size="lg" className="mt-5 w-full">
              Continue
            </Button>
            <Button
              type="button"
              variant="ghost"
              className="mt-2 w-full"
              onClick={() => setStep({ kind: "password" })}
            >
              <ArrowLeft />
              Back
            </Button>
          </motion.form>
        ) : (
          <motion.div key="password" {...slide}>
            <h1 className="text-2xl font-semibold text-fg">Sign in</h1>
            <p className="mt-1.5 text-[13px] text-fg-3">
              Welcome back to {brand.company_name}.
            </p>

            <form onSubmit={submit} className="mt-7 space-y-4">
              <Field label="Email">
                {(p) => (
                  <Input
                    {...p}
                    type="email"
                    autoComplete="email"
                    placeholder="you@company.com"
                    value={email}
                    onChange={(e) => setEmail(e.target.value)}
                    required
                    autoFocus
                  />
                )}
              </Field>
              <Field
                label="Password"
                trailing={
                  <Link
                    href={`/forgot-password${email ? `?email=${encodeURIComponent(email)}` : ""}`}
                    className="text-xs font-medium text-fg-3 transition hover:text-accent"
                  >
                    Forgot?
                  </Link>
                }
              >
                {(p) => (
                  <div className="relative">
                    <Input
                      {...p}
                      type={showPassword ? "text" : "password"}
                      autoComplete="current-password"
                      value={password}
                      onChange={(e) => setPassword(e.target.value)}
                      className="pr-11"
                      required
                    />
                    <button
                      type="button"
                      onClick={() => setShowPassword((s) => !s)}
                      aria-label={
                        showPassword ? "Hide password" : "Show password"
                      }
                      className="absolute top-1/2 right-2 -translate-y-1/2 rounded-lg p-1.5 text-fg-3 transition hover:text-fg"
                    >
                      {showPassword ? (
                        <EyeOff className="size-4" />
                      ) : (
                        <Eye className="size-4" />
                      )}
                    </button>
                  </div>
                )}
              </Field>

              {error && (
                <div
                  role="alert"
                  className="flex items-start gap-2 rounded-xl border border-bad/25 bg-bad/10 px-3 py-2.5 text-[13px] text-bad"
                >
                  <CircleAlert className="mt-0.5 size-4 shrink-0" />
                  {error}
                </div>
              )}

              <Button type="submit" size="lg" className="w-full" loading={busy}>
                Sign in
                {!busy && <ArrowRight />}
              </Button>
            </form>

            <div className="my-6 flex items-center gap-3 text-xs text-fg-4">
              <span className="h-px flex-1 bg-line" />
              or
              <span className="h-px flex-1 bg-line" />
            </div>

            <div className="grid grid-cols-3 gap-2">
              {PROVIDERS.map((p) => (
                <Button
                  key={p.key}
                  variant="secondary"
                  size="sm"
                  onClick={() => social(p.key)}
                >
                  {p.label}
                </Button>
              ))}
            </div>

            <details className="group mt-6 rounded-xl border border-line bg-fill/50 open:bg-fill">
              <summary className="flex cursor-pointer list-none items-center justify-between px-3.5 py-2.5 text-xs font-medium text-fg-3 select-none">
                Demo accounts
                <span className="text-fg-4 transition group-open:rotate-90">
                  ›
                </span>
              </summary>
              <div className="space-y-1 px-1.5 pb-1.5">
                {DEMO_ACCOUNTS.map((a) => (
                  <button
                    key={a.email}
                    type="button"
                    onClick={() => {
                      setEmail(a.email);
                      setPassword("password");
                    }}
                    className="flex w-full items-center justify-between rounded-lg px-2 py-2 text-left transition hover:bg-fill-2"
                  >
                    <span>
                      <span className="block text-[13px] font-medium text-fg">
                        {a.name}
                      </span>
                      <span className="block text-xs text-fg-3">{a.role}</span>
                    </span>
                    <span className="font-mono text-[11px] text-fg-4">
                      {a.email}
                    </span>
                  </button>
                ))}
              </div>
            </details>
          </motion.div>
        )}
      </AnimatePresence>
    </AuthFrame>
  );
}
