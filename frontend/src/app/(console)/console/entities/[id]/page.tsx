"use client";

// One counterparty: contact details, a running log of notes, and for
// contractors their Alpha link and their tax and insurance paperwork.

import { useState } from "react";
import Link from "next/link";
import { useParams } from "next/navigation";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  ArrowLeft,
  Briefcase,
  Globe,
  Mail,
  MessageSquareText,
  Phone,
  Send,
} from "lucide-react";
import { toast } from "sonner";
import { api, ApiError } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { useHasTenantScope } from "@/components/shell/tenant-scope";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { Fact } from "@/components/property/bits";
import { humanize } from "../kinds";
import { Compliance } from "./Compliance";
import { PartnerLink } from "./PartnerLink";

function when(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}

export default function EntityPage() {
  const { id } = useParams<{ id: string }>();
  const { can } = useAuth();
  const scoped = useHasTenantScope();
  const manage = can("entity:manage");
  const qc = useQueryClient();
  const detail = useQuery({
    queryKey: ["entities", "one", id],
    queryFn: () => api.entity(id),
    enabled: scoped && can("entity:read"),
  });
  const [note, setNote] = useState("");
  const [saving, setSaving] = useState(false);

  async function addNote() {
    if (!note.trim()) return;
    setSaving(true);
    try {
      await api.addEntityNote(id, note.trim());
      setNote("");
      await qc.invalidateQueries({ queryKey: ["entities", "one", id] });
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't add the note");
    } finally {
      setSaving(false);
    }
  }

  const back = (
    <Link
      href="/console/entities"
      className="inline-flex items-center gap-1.5 text-[13px] text-fg-3 transition hover:text-fg"
    >
      <ArrowLeft className="size-4" />
      Entities
    </Link>
  );

  if (detail.error) {
    const missing =
      detail.error instanceof ApiError && detail.error.status === 404;
    return (
      <div className="space-y-6">
        {back}
        <Panel className="mx-auto max-w-lg">
          <EmptyState
            icon={<Briefcase />}
            title={missing ? "This entity isn't here" : "Couldn't load it"}
            description={missing ? undefined : detail.error.message}
          />
        </Panel>
      </div>
    );
  }
  const d = detail.data;
  if (!d)
    return (
      <div className="space-y-6">
        {back}
        <Skeleton className="h-20 rounded-2xl" />
        <Skeleton className="h-64 rounded-2xl" />
      </div>
    );

  const log = [...d.notes_log].sort((a, b) =>
    b.created_at.localeCompare(a.created_at)
  );
  const vendor = d.kind === "contractor";
  const linkable = vendor || d.kind === "property_manager" || !!d.partner_kind;

  return (
    <div className="space-y-6">
      {back}
      <PageHeader
        eyebrow={
          <span className="inline-flex items-center gap-2">
            Entity <Badge tone="info">{humanize(d.kind)}</Badge>
          </span>
        }
        title={d.name}
        description={d.contact_name ?? undefined}
        actions={
          <>
            {d.email && (
              <Button variant="secondary" size="sm" asChild>
                <a href={`mailto:${d.email}`}>
                  <Mail />
                  Email
                </a>
              </Button>
            )}
            {d.phone && (
              <Button variant="secondary" size="sm" asChild>
                <a href={`tel:${d.phone}`}>
                  <Phone />
                  Call
                </a>
              </Button>
            )}
          </>
        }
      />

      <div className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_minmax(0,1.3fr)]">
        <div className="space-y-4">
          <Panel>
            <PanelHeader title="Details" />
            <dl className="divide-y divide-line/60 px-5 pt-2 pb-4">
              <Fact label="Contact">{d.contact_name ?? "—"}</Fact>
              <Fact label="Email">{d.email ?? "—"}</Fact>
              <Fact label="Phone">{d.phone ?? "—"}</Fact>
              <Fact label="Website">
                {d.website ? (
                  <a
                    href={
                      /^https?:\/\//.test(d.website)
                        ? d.website
                        : `https://${d.website}`
                    }
                    target="_blank"
                    rel="noreferrer"
                    className="inline-flex items-center gap-1 text-accent hover:underline"
                  >
                    <Globe className="size-3.5" />
                    {d.website}
                  </a>
                ) : (
                  "—"
                )}
              </Fact>
              <Fact label="Address">{d.address ?? "—"}</Fact>
              <Fact label="Notes">
                <span className="whitespace-pre-wrap">{d.notes ?? "—"}</span>
              </Fact>
            </dl>
          </Panel>
          {linkable && <PartnerLink counterpartyId={d.id} manage={manage} />}
        </div>

        <div className="space-y-4">
          {vendor && <Compliance counterpartyId={d.id} manage={manage} />}

          <Panel>
            <PanelHeader
              title="Notes log"
              description="What was said and agreed, newest first."
            />
            <div className="space-y-4 p-5 pt-4">
              {manage && (
                <form
                  className="space-y-2"
                  onSubmit={(e) => {
                    e.preventDefault();
                    void addNote();
                  }}
                >
                  <textarea
                    value={note}
                    onChange={(e) => setNote(e.target.value)}
                    rows={3}
                    placeholder="Add a note about this entity"
                    aria-label="New note"
                    className="w-full rounded-xl border border-line-strong bg-fill px-3.5 py-2.5 text-sm text-fg outline-none placeholder:text-fg-4 focus:border-accent"
                  />
                  <div className="flex justify-end">
                    <Button
                      type="submit"
                      size="sm"
                      disabled={!note.trim()}
                      loading={saving}
                    >
                      <Send />
                      Add note
                    </Button>
                  </div>
                </form>
              )}
              {log.length === 0 ? (
                <EmptyState
                  icon={<MessageSquareText />}
                  title="No notes yet"
                  className="py-6"
                />
              ) : (
                <ul className="divide-y divide-line">
                  {log.map((n) => (
                    <li key={n.id} className="py-3 first:pt-0">
                      <p className="text-[13px] whitespace-pre-wrap text-fg">
                        {n.body}
                      </p>
                      <p className="mt-1 text-xs text-fg-3">
                        {when(n.created_at)}
                      </p>
                    </li>
                  ))}
                </ul>
              )}
            </div>
          </Panel>
        </div>
      </div>
    </div>
  );
}
