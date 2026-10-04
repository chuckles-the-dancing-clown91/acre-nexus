"use client";

// Owners and CRM: the people whose properties you manage (owners) and the
// people who might hire you (owner leads). Follow-ups that are due, an owner
// directory with each owner's timeline and portal invite, and a pipeline
// board from first contact to a signed management agreement. Reading needs
// `entity:read`; changes need `entity:manage`.

import { Suspense, useMemo, useState } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import { useQuery } from "@tanstack/react-query";
import { Handshake } from "lucide-react";
import { crm } from "@/lib/backoffice";
import { useAuth } from "@/lib/auth";
import { useMembers } from "@/lib/queries";
import { useHasTenantScope } from "@/components/shell/tenant-scope";
import { Tabs } from "@/components/ui/data-table";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/menu";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";
import { FollowUps } from "./follow-ups";
import { OwnerDetails, OwnersTab } from "./owners";
import { LeadDetails, PipelineTab, useLeadMover } from "./pipeline";
import { SUBJECT_LABEL, type Opened } from "./shared";
import { CrmTimeline } from "./timeline";

type Tab = "followups" | "owners" | "pipeline";
const TAB_KEYS: Tab[] = ["followups", "owners", "pipeline"];

export default function CrmPage() {
  return (
    <Suspense fallback={<Skeleton className="h-96 rounded-2xl" />}>
      <Crm />
    </Suspense>
  );
}

function Crm() {
  const { can } = useAuth();
  const scoped = useHasTenantScope();
  const read = can("entity:read");
  const manage = can("entity:manage");
  const ready = scoped && read;
  const params = useSearchParams();
  const router = useRouter();
  const [opened, setOpened] = useState<Opened | null>(null);
  const { move, dialog } = useLeadMover();

  const due = useQuery({
    queryKey: ["crm", "follow-ups", false],
    queryFn: () => crm.followUps(),
    enabled: ready,
  });
  const owners = useQuery({
    queryKey: ["crm", "owners"],
    queryFn: crm.owners,
    enabled: ready,
  });
  const leads = useQuery({
    queryKey: ["crm", "leads"],
    queryFn: () => crm.leads(),
    enabled: ready,
  });
  const summary = useQuery({
    queryKey: ["crm", "pipeline"],
    queryFn: crm.pipeline,
    enabled: ready,
  });
  const members = useMembers({ enabled: ready && manage });
  const active = useMemo(
    () => (members.data ?? []).filter((m) => m.status === "active"),
    [members.data]
  );

  const asked = params.get("tab") as Tab | null;
  // Land on follow-ups when something is due, otherwise on owners.
  const tab: Tab | null =
    asked && TAB_KEYS.includes(asked)
      ? asked
      : due.data
        ? due.data.length
          ? "followups"
          : "owners"
        : due.error
          ? "owners"
          : null;
  const choose = (k: Tab) =>
    router.replace(`/console/crm?tab=${k}`, { scroll: false });

  if (!read) {
    return (
      <div className="space-y-6">
        <PageHeader eyebrow="Owners" title="Owners and CRM" />
        <Panel>
          <EmptyState
            icon={<Handshake />}
            title="You don't have access to the owner CRM"
            description="Ask an admin for the entity:read permission."
          />
        </Panel>
      </div>
    );
  }

  const count = (n: number | undefined) => (n ? ` · ${n}` : "");
  const tabs = [
    ["followups", `Follow-ups${count(due.data?.length)}`],
    ["owners", `Owners${count(owners.data?.length)}`],
    ["pipeline", `Pipeline${count(leads.data?.length)}`],
  ] as const;

  const openOwner =
    opened?.type === "owner"
      ? owners.data?.find((o) => o.id === opened.id)
      : undefined;
  const openLead =
    opened?.type === "owner_lead"
      ? leads.data?.find((l) => l.id === opened.id)
      : undefined;

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Owners"
        title="Owners and CRM"
        description="Keep up with the owners you manage for, and win new ones."
      />

      {tab === null ? (
        <Skeleton className="h-64 rounded-2xl" />
      ) : (
        <>
          <Tabs tabs={tabs} value={tab} onChange={choose} />
          {tab === "followups" && (
            <FollowUps enabled={ready} manage={manage} onOpen={setOpened} />
          )}
          {tab === "owners" && (
            <OwnersTab
              owners={owners.data}
              loading={owners.isLoading}
              error={owners.error}
              manage={manage}
              onOpen={setOpened}
            />
          )}
          {tab === "pipeline" && (
            <PipelineTab
              leads={leads.data}
              summary={summary.data}
              loading={leads.isLoading}
              error={leads.error}
              manage={manage}
              onMove={move}
              onOpen={setOpened}
            />
          )}
        </>
      )}

      <Dialog
        open={opened !== null}
        onOpenChange={(o) => !o && setOpened(null)}
      >
        <DialogContent className="top-0 right-0 left-auto h-dvh w-full max-w-2xl translate-x-0 translate-y-0 overflow-y-auto rounded-none p-5 sm:rounded-l-2xl sm:p-6">
          <div className="eyebrow">
            {opened ? SUBJECT_LABEL[opened.type] : ""}
          </div>
          <DialogTitle className="mt-1 pr-8 text-[22px] leading-tight font-semibold">
            {openOwner?.name ?? openLead?.name ?? opened?.name ?? "Timeline"}
          </DialogTitle>
          <DialogDescription className="sr-only">
            Details and timeline
          </DialogDescription>
          <div className="mt-5 space-y-6">
            {openOwner && (
              <OwnerDetails
                key={openOwner.id}
                owner={openOwner}
                manage={manage}
              />
            )}
            {openLead && (
              <LeadDetails
                key={openLead.id}
                lead={openLead}
                manage={manage}
                members={active}
                onMove={(s) => move(openLead, s)}
                onConverted={() => {
                  setOpened(null);
                  choose("owners");
                }}
              />
            )}
            {opened && (
              <section className="space-y-3">
                <h3 className="text-[15px] font-semibold text-fg">Timeline</h3>
                <CrmTimeline
                  key={`${opened.type}:${opened.id}`}
                  subjectType={opened.type}
                  subjectId={opened.id}
                  canManage={manage}
                />
              </section>
            )}
          </div>
        </DialogContent>
      </Dialog>

      {dialog}
    </div>
  );
}
