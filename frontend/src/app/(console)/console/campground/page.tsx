"use client";

// Campgrounds: the campground and RV park site maps, each one's front desk.

import Link from "next/link";
import { useQuery } from "@tanstack/react-query";
import { ChevronRight, Tent } from "lucide-react";
import { campground } from "@/lib/campground";
import { useAuth } from "@/lib/auth";
import { useHasTenantScope } from "@/components/shell/tenant-scope";
import { Badge } from "@/components/ui/badge";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";

export default function CampgroundsPage() {
  const { can } = useAuth();
  const scoped = useHasTenantScope();
  const q = useQuery({
    queryKey: ["campgrounds"],
    queryFn: campground.list,
    enabled: scoped && can("property:read"),
  });
  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Property"
        title="Campgrounds"
        description="Sites drawn on a campground or RV park map, booked by the night, week or month."
      />
      {q.isLoading && <Skeleton className="h-40 rounded-2xl" />}
      {q.data?.length === 0 && (
        <EmptyState
          icon={<Tent />}
          title="No campgrounds yet"
          description="Draw a campground or RV park on a site map, with its sites, and it shows up here."
        />
      )}
      <div className="grid gap-4 md:grid-cols-2">
        {q.data?.map((c) => (
          <Link key={c.map_id} href={`/console/campground/${c.map_id}`}>
            <Panel className="flex items-center gap-4 p-5 transition hover:border-fg-4">
              <div className="flex size-11 items-center justify-center rounded-xl bg-accent/10 text-accent">
                <Tent className="size-5" />
              </div>
              <div className="min-w-0 flex-1">
                <div className="flex flex-wrap items-center gap-2">
                  <span className="truncate text-[15px] font-semibold text-fg">
                    {c.name}
                  </span>
                  {c.booking_open && c.published ? (
                    <Badge tone="good">Taking bookings</Badge>
                  ) : (
                    <Badge>Not taking bookings</Badge>
                  )}
                </div>
                <div className="text-[13px] text-fg-3">
                  {c.property_name} · {c.sites} sites · {c.in_house} in house ·{" "}
                  {c.arriving_today} arriving today
                </div>
              </div>
              <ChevronRight className="size-4 text-fg-4" />
            </Panel>
          </Link>
        ))}
      </div>
    </div>
  );
}
