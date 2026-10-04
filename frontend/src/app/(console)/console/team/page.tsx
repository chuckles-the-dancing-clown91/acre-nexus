"use client";

// Team: the office view of staff. Profiles and pay, the weekly schedule, time
// off, and the members who can sign in (with their personas and invites).
// Staff tabs need `team:read` and changes `team:manage`; pay and bill rates
// only come back with `payroll:read`. Members need `member:read`, inviting
// and sending sign-in links `member:manage`.

import { Suspense, useMemo } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import { useQuery } from "@tanstack/react-query";
import { CalendarOff, Clock, IdCard, Timer, Users } from "lucide-react";
import { hm, team } from "@/lib/backoffice";
import { useAuth } from "@/lib/auth";
import { useHasTenantScope } from "@/components/shell/tenant-scope";
import { Tabs } from "@/components/ui/data-table";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";
import { cn } from "@/lib/utils";
import { MembersDirectory } from "./members";
import { People } from "./people";
import { Schedule } from "./schedule";
import { TimeOffList } from "./time-off";

type Tab = "people" | "schedule" | "timeoff" | "members";

export default function TeamPage() {
  return (
    <Suspense fallback={<Skeleton className="h-96 rounded-2xl" />}>
      <Team />
    </Suspense>
  );
}

function Team() {
  const { can } = useAuth();
  const scoped = useHasTenantScope();
  const read = can("team:read");
  const manage = can("team:manage");
  const seePay = can("payroll:read");
  const seeMembers = can("member:read");
  const params = useSearchParams();
  const router = useRouter();

  const tabs = useMemo(
    () =>
      [
        ...(read
          ? ([
              ["people", "People"],
              ["schedule", "Schedule"],
              ["timeoff", "Time off"],
            ] as const)
          : []),
        ...(seeMembers ? ([["members", "Members"]] as const) : []),
      ] as (readonly [Tab, string])[],
    [read, seeMembers]
  );
  const asked = params.get("tab") as Tab | null;
  const tab: Tab | undefined =
    tabs.find(([k]) => k === asked)?.[0] ?? tabs[0]?.[0];
  const choose = (k: Tab) =>
    router.replace(`/console/team?tab=${k}`, { scroll: false });

  const roster = useQuery({
    queryKey: ["team", "roster"],
    queryFn: team.roster,
    enabled: scoped && read,
  });
  const timeOff = useQuery({
    queryKey: ["team", "time-off"],
    queryFn: () => team.timeOff(),
    enabled: scoped && read,
  });

  if (!tab) {
    return (
      <div className="space-y-6">
        <PageHeader eyebrow="Team" title="Team" />
        <Panel>
          <EmptyState
            icon={<Users />}
            title="You don't have access to the team"
            description="Ask an admin for the team:read or member:read permission."
          />
        </Panel>
      </div>
    );
  }

  const staff = (roster.data ?? []).filter((p) => p.profile?.current);
  const clocked = staff.filter((p) => p.clocked_in).length;
  const weekMinutes = staff.reduce((s, p) => s + p.week_minutes, 0);
  const pending = (timeOff.data ?? []).filter(
    (t) => t.status === "pending"
  ).length;

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Team"
        title="Team"
        description="Your staff, who's working when, time off, and who can sign in."
      />

      {read && (
        <section className="grid grid-cols-2 gap-3 xl:grid-cols-4">
          <Figure icon={<IdCard />} label="On staff" value={staff.length} />
          <Figure
            icon={<Clock />}
            label="On the clock now"
            value={clocked}
            tone={clocked ? "good" : undefined}
          />
          <Figure
            icon={<Timer />}
            label="Hours this week"
            value={hm(weekMinutes)}
          />
          <Figure
            icon={<CalendarOff />}
            label="Time off to review"
            value={pending}
            tone={pending ? "warn" : undefined}
          />
        </section>
      )}

      <Tabs tabs={tabs} value={tab} onChange={choose} />

      {tab === "people" && (
        <People
          roster={roster.data}
          loading={roster.isLoading}
          error={roster.error}
          manage={manage}
          seePay={seePay}
        />
      )}
      {tab === "schedule" && (
        <Schedule roster={roster.data ?? []} manage={manage} />
      )}
      {tab === "timeoff" && (
        <TimeOffList
          list={timeOff.data}
          loading={timeOff.isLoading}
          error={timeOff.error}
          manage={manage}
        />
      )}
      {tab === "members" && <MembersDirectory />}
    </div>
  );
}

function Figure({
  icon,
  label,
  value,
  tone,
}: {
  icon: React.ReactNode;
  label: string;
  value: React.ReactNode;
  tone?: "good" | "warn";
}) {
  return (
    <Panel className="flex items-center gap-4 p-4">
      <span
        className={cn(
          "flex size-10 items-center justify-center rounded-xl border [&_svg]:size-[18px]",
          tone === "good"
            ? "border-good/30 bg-good/10 text-good"
            : tone === "warn"
              ? "border-warn/30 bg-warn/10 text-warn"
              : "border-line bg-fill text-fg-2"
        )}
      >
        {icon}
      </span>
      <div>
        <div className="figure text-[24px] leading-none font-semibold text-fg">
          {value}
        </div>
        <div className="mt-1 text-xs text-fg-3">{label}</div>
      </div>
    </Panel>
  );
}
