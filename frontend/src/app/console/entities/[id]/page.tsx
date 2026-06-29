"use client";

// Entity detail: a single counterparty's contact details plus a chronological
// notes timeline with an inline composer (gated by "entity:manage").

import { useParams } from "next/navigation";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import {
  AtSign,
  Building2,
  Globe,
  MapPin,
  MessageSquarePlus,
  Phone,
  StickyNote,
  User,
} from "lucide-react";

import { useEntity, useAddEntityNote } from "@/lib/queries";
import { useAuth } from "@/lib/auth";
import { relativeDate, formatDateTime, titleCase, initials } from "@/lib/format";
import { Badge } from "@/components/ui";
import { Breadcrumbs } from "@/components/ui/breadcrumbs";
import { PageHeader, EmptyState } from "@/components/ui/page";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/form-field";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import type { LucideIcon } from "lucide-react";
import type { CounterpartyDetail, CounterpartyNote } from "@/lib/types";

const noteSchema = z.object({
  body: z.string().trim().min(1, "Note can't be empty"),
});
type NoteForm = z.infer<typeof noteSchema>;

export default function EntityDetailPage() {
  const { id } = useParams<{ id: string }>();
  const { can } = useAuth();
  const canManage = can("entity:manage");

  const entity = useEntity(id);

  return (
    <div className="space-y-6">
      <Breadcrumbs
        items={[
          { label: "Entities", href: "/console/entities" },
          { label: entity.data?.name ?? "Entity" },
        ]}
      />

      {entity.isLoading ? (
        <LoadingState />
      ) : entity.isError || !entity.data ? (
        <EmptyState
          icon={Building2}
          title="Couldn't load this entity"
          description={
            entity.error?.message ??
            "The counterparty may have been removed, or you may not have access."
          }
        />
      ) : (
        <EntityDetail
          detail={entity.data}
          entityId={id}
          canManage={canManage}
        />
      )}
    </div>
  );
}

function EntityDetail({
  detail,
  entityId,
  canManage,
}: {
  detail: CounterpartyDetail;
  entityId: string;
  canManage: boolean;
}) {
  return (
    <>
      <PageHeader
        eyebrow="Entity"
        title={detail.name}
        description={
          detail.contact_name
            ? `Primary contact · ${detail.contact_name}`
            : "Counterparty registry record"
        }
        actions={<Badge tone="info">{titleCase(detail.kind)}</Badge>}
      />

      <div className="grid gap-6 lg:grid-cols-3">
        <DetailsCard detail={detail} />
        <div className="lg:col-span-2">
          <NotesCard
            entityId={entityId}
            notes={detail.notes_log}
            canManage={canManage}
          />
        </div>
      </div>
    </>
  );
}

function DetailsCard({ detail }: { detail: CounterpartyDetail }) {
  return (
    <Card className="h-fit lg:col-span-1">
      <CardHeader>
        <CardTitle>Details</CardTitle>
        <Badge tone="info">{titleCase(detail.kind)}</Badge>
      </CardHeader>
      <CardContent className="space-y-1">
        <DetailRow icon={User} label="Contact" value={detail.contact_name} />
        <DetailRow
          icon={AtSign}
          label="Email"
          value={detail.email}
          href={detail.email ? `mailto:${detail.email}` : undefined}
        />
        <DetailRow
          icon={Phone}
          label="Phone"
          value={detail.phone}
          href={detail.phone ? `tel:${detail.phone}` : undefined}
          numeric
        />
        <DetailRow
          icon={Globe}
          label="Website"
          value={detail.website}
          href={detail.website ?? undefined}
          external
        />
        <DetailRow icon={MapPin} label="Address" value={detail.address} />
        {detail.notes ? (
          <div className="border-t border-line pt-3">
            <div className="mb-1 flex items-center gap-2 text-xs font-semibold uppercase tracking-wide text-ink-3">
              <StickyNote className="h-3.5 w-3.5" />
              Summary
            </div>
            <p className="whitespace-pre-wrap text-sm text-ink-2">
              {detail.notes}
            </p>
          </div>
        ) : null}
      </CardContent>
    </Card>
  );
}

function DetailRow({
  icon: Icon,
  label,
  value,
  href,
  external,
  numeric,
}: {
  icon: LucideIcon;
  label: string;
  value: string | null;
  href?: string;
  external?: boolean;
  numeric?: boolean;
}) {
  return (
    <div className="flex items-start justify-between gap-3 border-b border-line py-2.5 last:border-0">
      <div className="flex items-center gap-2 text-ink-3">
        <Icon className="h-4 w-4 shrink-0" />
        <span className="text-sm">{label}</span>
      </div>
      {value ? (
        href ? (
          <a
            href={href}
            target={external ? "_blank" : undefined}
            rel={external ? "noopener noreferrer" : undefined}
            className="truncate text-right text-sm font-medium text-accent-2 hover:underline"
            data-numeric={numeric || undefined}
          >
            {value}
          </a>
        ) : (
          <span
            className="truncate text-right text-sm font-medium text-ink"
            data-numeric={numeric || undefined}
          >
            {value}
          </span>
        )
      ) : (
        <span className="text-sm text-ink-3">—</span>
      )}
    </div>
  );
}

function NotesCard({
  entityId,
  notes,
  canManage,
}: {
  entityId: string;
  notes: CounterpartyNote[];
  canManage: boolean;
}) {
  const addNote = useAddEntityNote(entityId);
  const {
    register,
    handleSubmit,
    reset,
    formState: { errors, isValid },
  } = useForm<NoteForm>({
    resolver: zodResolver(noteSchema),
    mode: "onChange",
    defaultValues: { body: "" },
  });

  const onSubmit = handleSubmit((values) => {
    addNote.mutate(values.body.trim(), { onSuccess: () => reset() });
  });

  const sorted = [...notes].sort((a, b) =>
    b.created_at.localeCompare(a.created_at)
  );

  return (
    <Card>
      <CardHeader>
        <CardTitle>Notes</CardTitle>
        <span data-numeric className="text-xs text-ink-3">
          {notes.length} {notes.length === 1 ? "entry" : "entries"}
        </span>
      </CardHeader>
      <CardContent className="space-y-5">
        {canManage && (
          <form onSubmit={onSubmit} className="space-y-2">
            <Textarea
              {...register("body")}
              error={!!errors.body}
              placeholder="Add a note about this entity…"
              rows={3}
              aria-label="New note"
              disabled={addNote.isPending}
            />
            <div className="flex items-center justify-between">
              <p className="text-xs text-bad">{errors.body?.message ?? ""}</p>
              <Button
                type="submit"
                size="sm"
                disabled={!isValid || addNote.isPending}
              >
                <MessageSquarePlus className="h-4 w-4" />
                {addNote.isPending ? "Adding…" : "Add note"}
              </Button>
            </div>
          </form>
        )}

        {sorted.length === 0 ? (
          <EmptyState
            className="border-0 bg-transparent py-10"
            icon={StickyNote}
            title="No notes yet"
            description={
              canManage
                ? "Add the first note to start a record for this entity."
                : "Notes added by your team will appear here."
            }
          />
        ) : (
          <ol className="relative space-y-5 pl-6">
            <span
              aria-hidden
              className="absolute left-[7px] top-1.5 bottom-1.5 w-px bg-line"
            />
            {sorted.map((note) => (
              <NoteItem key={note.id} note={note} />
            ))}
          </ol>
        )}
      </CardContent>
    </Card>
  );
}

function NoteItem({ note }: { note: CounterpartyNote }) {
  return (
    <li className="relative">
      <span
        aria-hidden
        className="absolute -left-6 top-1 flex h-4 w-4 items-center justify-center rounded-full border-2 border-surface bg-accent"
      />
      <div className="rounded-lg border border-line bg-surface-2/40 px-4 py-3">
        <div className="mb-1.5 flex items-center gap-2">
          <span className="flex h-5 w-5 items-center justify-center rounded-full bg-accent-soft text-[10px] font-bold text-accent-2">
            {note.author_user_id ? initials("U") : "•"}
          </span>
          <TooltipProvider>
            <Tooltip>
              <TooltipTrigger asChild>
                <span className="cursor-default text-xs font-medium text-ink-3">
                  {relativeDate(note.created_at)}
                </span>
              </TooltipTrigger>
              <TooltipContent>{formatDateTime(note.created_at)}</TooltipContent>
            </Tooltip>
          </TooltipProvider>
        </div>
        <p className="whitespace-pre-wrap text-sm text-ink">{note.body}</p>
      </div>
    </li>
  );
}

function LoadingState() {
  return (
    <div className="space-y-6">
      <div className="space-y-2">
        <div className="skeleton h-8 w-64 rounded-lg" />
        <div className="skeleton h-4 w-48 rounded-lg" />
      </div>
      <div className="grid gap-6 lg:grid-cols-3">
        <div className="skeleton h-64 rounded-xl lg:col-span-1" />
        <div className="skeleton h-64 rounded-xl lg:col-span-2" />
      </div>
    </div>
  );
}
