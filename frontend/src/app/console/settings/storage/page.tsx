"use client";

// Workspace document-storage settings. Choose the platform-managed default or
// bring your own bucket (Local / S3 / GCS). Gated by `storage:manage`.

import { useEffect } from "react";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import {
  Cloud,
  HardDrive,
  Lock,
  Server,
  ShieldCheck,
} from "lucide-react";

import { useAuth } from "@/lib/auth";
import { useStorageConfig, usePutStorageConfig } from "@/lib/queries";
import type { LucideIcon } from "lucide-react";

import { PageHeader, EmptyState } from "@/components/ui/page";
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Field, Input, Textarea } from "@/components/ui/form-field";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Badge } from "@/components/ui";

type Provider = "platform" | "local" | "s3" | "gcs";

const PROVIDERS: {
  value: Provider;
  label: string;
  hint: string;
  icon: LucideIcon;
}[] = [
  {
    value: "platform",
    label: "Platform-managed",
    hint: "Acre stores your documents — nothing to configure.",
    icon: ShieldCheck,
  },
  {
    value: "local",
    label: "Local filesystem",
    hint: "Store on the server's disk (single-node / dev).",
    icon: HardDrive,
  },
  {
    value: "s3",
    label: "Amazon S3 (or compatible)",
    hint: "Your own S3 / MinIO / Cloudflare R2 bucket.",
    icon: Cloud,
  },
  {
    value: "gcs",
    label: "Google Cloud Storage",
    hint: "Your own GCS bucket.",
    icon: Server,
  },
];

const schema = z.object({
  provider: z.enum(["platform", "local", "s3", "gcs"]),
  bucket: z.string().trim().optional(),
  region: z.string().trim().optional(),
  prefix: z.string().trim().optional(),
  endpoint: z.string().trim().optional(),
  secret: z.string().optional(),
});

type FormValues = z.infer<typeof schema>;

const DEFAULTS: FormValues = {
  provider: "platform",
  bucket: "",
  region: "",
  prefix: "",
  endpoint: "",
  secret: "",
};

export default function StorageSettingsPage() {
  const { can } = useAuth();
  const allowed = can("storage:manage");

  const cfgQuery = useStorageConfig({ enabled: allowed });
  const putConfig = usePutStorageConfig();

  const {
    register,
    handleSubmit,
    reset,
    setValue,
    watch,
    formState: { errors, isDirty },
  } = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: DEFAULTS,
  });

  const cfg = cfgQuery.data;

  // Hydrate the form once the saved config loads.
  useEffect(() => {
    if (!cfg) return;
    reset({
      provider: (cfg.provider as Provider) ?? "platform",
      bucket: cfg.bucket ?? "",
      region: cfg.region ?? "",
      prefix: cfg.prefix ?? "",
      endpoint: cfg.endpoint ?? "",
      secret: "",
    });
  }, [cfg, reset]);

  const provider = watch("provider");
  const active = PROVIDERS.find((p) => p.value === provider) ?? PROVIDERS[0];
  const isLocal = provider === "local";
  const isS3 = provider === "s3";
  const isGcs = provider === "gcs";
  const needsBucket = isS3 || isGcs;

  const onSubmit = handleSubmit((values) => {
    putConfig.mutate(
      {
        provider: values.provider,
        bucket: values.bucket || undefined,
        region: values.region || undefined,
        prefix: values.prefix || undefined,
        endpoint: values.endpoint || undefined,
        secret: values.secret || undefined,
      },
      {
        onSuccess: (saved) => {
          reset({
            provider: (saved.provider as Provider) ?? "platform",
            bucket: saved.bucket ?? "",
            region: saved.region ?? "",
            prefix: saved.prefix ?? "",
            endpoint: saved.endpoint ?? "",
            secret: "",
          });
        },
      }
    );
  });

  if (!allowed) {
    return (
      <div className="space-y-6">
        <PageHeader
          eyebrow="Settings"
          title="Document storage"
          description="Configure where uploaded documents and generated PDFs are stored."
        />
        <EmptyState
          icon={Lock}
          title="You don't have access"
          description={
            <>
              The <span className="font-mono text-ink-2">storage:manage</span>{" "}
              permission is required to view and change storage settings.
            </>
          }
        />
      </div>
    );
  }

  return (
    <div className="max-w-2xl space-y-6">
      <PageHeader
        eyebrow="Settings"
        title="Document storage"
        description="Where uploaded logos, LLC documents, and generated PDFs are stored."
      />

      {/* Current backend summary */}
      {cfgQuery.isLoading ? (
        <div className="skeleton h-16 rounded-xl" />
      ) : cfg ? (
        <Card>
          <CardContent className="flex flex-wrap items-center gap-3 py-4">
            <span className="text-sm text-ink-3">Current backend</span>
            <Badge tone="info">
              {PROVIDERS.find((p) => p.value === cfg.provider)?.label ??
                cfg.provider}
            </Badge>
            {cfg.is_default && <Badge tone="neutral">default</Badge>}
            {cfg.has_credentials && (
              <Badge tone="good">credentials set</Badge>
            )}
          </CardContent>
        </Card>
      ) : null}

      <form onSubmit={onSubmit}>
        <Card>
          <CardHeader>
            <div className="min-w-0">
              <CardTitle>Storage provider</CardTitle>
              <CardDescription>
                Acre manages storage by default — no setup needed. Switch to a
                bring-your-own backend to keep documents in your own bucket.
              </CardDescription>
            </div>
          </CardHeader>

          <CardContent className="space-y-5">
            <Field
              label="Provider"
              hint={active.hint}
              htmlFor="storage-provider"
            >
              <Select
                value={provider}
                onValueChange={(v) =>
                  setValue("provider", v as Provider, {
                    shouldDirty: true,
                    shouldValidate: true,
                  })
                }
              >
                <SelectTrigger id="storage-provider" aria-label="Provider">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {PROVIDERS.map((p) => {
                    const PIcon = p.icon;
                    return (
                      <SelectItem key={p.value} value={p.value}>
                        <span className="flex items-center gap-2">
                          <PIcon className="h-4 w-4 text-ink-3" />
                          {p.label}
                        </span>
                      </SelectItem>
                    );
                  })}
                </SelectContent>
              </Select>
            </Field>

            {provider === "platform" && (
              <div className="flex items-start gap-3 rounded-lg border border-line bg-surface-2 px-4 py-3">
                <ShieldCheck className="mt-0.5 h-4 w-4 shrink-0 text-accent-2" />
                <p className="text-sm text-ink-2">
                  Acre stores your documents on managed, encrypted
                  infrastructure. There&apos;s nothing to configure — this is
                  the recommended default for most workspaces.
                </p>
              </div>
            )}

            {isLocal && (
              <Field
                label="Base directory"
                hint="Absolute path on the server's disk."
                htmlFor="storage-prefix"
                error={errors.prefix?.message}
              >
                <Input
                  id="storage-prefix"
                  placeholder="/var/acre/storage"
                  error={!!errors.prefix}
                  {...register("prefix")}
                />
              </Field>
            )}

            {needsBucket && (
              <>
                <div className="grid gap-4 sm:grid-cols-2">
                  <Field
                    label="Bucket"
                    htmlFor="storage-bucket"
                    error={errors.bucket?.message}
                  >
                    <Input
                      id="storage-bucket"
                      placeholder={isGcs ? "my-gcs-bucket" : "my-s3-bucket"}
                      error={!!errors.bucket}
                      {...register("bucket")}
                    />
                  </Field>
                  <Field
                    label="Key prefix"
                    hint="Optional path within the bucket."
                    htmlFor="storage-key-prefix"
                    error={errors.prefix?.message}
                  >
                    <Input
                      id="storage-key-prefix"
                      placeholder="acre/documents"
                      error={!!errors.prefix}
                      {...register("prefix")}
                    />
                  </Field>
                </div>

                {isS3 && (
                  <div className="grid gap-4 sm:grid-cols-2">
                    <Field
                      label="Region"
                      htmlFor="storage-region"
                      error={errors.region?.message}
                    >
                      <Input
                        id="storage-region"
                        placeholder="us-east-1"
                        error={!!errors.region}
                        {...register("region")}
                      />
                    </Field>
                    <Field
                      label="Endpoint"
                      hint="For MinIO / R2. Leave blank for AWS S3."
                      htmlFor="storage-endpoint"
                      error={errors.endpoint?.message}
                    >
                      <Input
                        id="storage-endpoint"
                        placeholder="https://…"
                        error={!!errors.endpoint}
                        {...register("endpoint")}
                      />
                    </Field>
                  </div>
                )}

                <Field
                  label={
                    isGcs
                      ? "Service-account key JSON"
                      : "Credentials JSON"
                  }
                  hint="Encrypted at rest (AES-256-GCM) and never shown again."
                  htmlFor="storage-secret"
                  error={errors.secret?.message}
                >
                  <Textarea
                    id="storage-secret"
                    className="font-mono text-xs"
                    rows={isGcs ? 6 : 3}
                    placeholder={
                      cfg?.has_credentials
                        ? "•••••• (leave blank to keep existing)"
                        : isGcs
                          ? '{ "type": "service_account", … }'
                          : '{ "access_key_id": "…", "secret_access_key": "…" }'
                    }
                    error={!!errors.secret}
                    {...register("secret")}
                  />
                </Field>
              </>
            )}
          </CardContent>

          <CardFooter className="flex items-center justify-between gap-3">
            <p className="text-xs text-ink-3">
              {cfgQuery.error
                ? "Couldn't load the current configuration."
                : "Changes apply to new uploads immediately."}
            </p>
            <Button
              type="submit"
              disabled={putConfig.isPending || !isDirty}
            >
              {putConfig.isPending ? "Saving…" : "Save settings"}
            </Button>
          </CardFooter>
        </Card>
      </form>
    </div>
  );
}
