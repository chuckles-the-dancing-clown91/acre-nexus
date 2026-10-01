"use client";

// Change a job kit in the catalog.

import Link from "next/link";
import { useParams } from "next/navigation";
import { useQuery } from "@tanstack/react-query";
import { ArrowLeft, ClipboardList } from "lucide-react";
import { desk, draftFrom } from "@/lib/servicedesk";
import { KitEditor } from "@/components/desk/KitEditor";
import { Button } from "@/components/ui/button";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";

export default function EditKitPage() {
  const { id } = useParams<{ id: string }>();
  const kits = useQuery({ queryKey: ["kits"], queryFn: desk.kits });
  if (kits.isLoading) return <Skeleton className="h-96 rounded-2xl" />;
  const kit = kits.data?.find((k) => k.id === id);
  if (!kit) {
    return (
      <Panel className="mx-auto mt-10 max-w-lg">
        <EmptyState
          icon={<ClipboardList />}
          title="This kit isn't in the catalog"
          description={
            kits.error ? kits.error.message : "It may have been retired."
          }
          action={
            <Button variant="secondary" asChild>
              <Link href="/console/maintenance/kits">
                <ArrowLeft />
                Job kits
              </Link>
            </Button>
          }
        />
      </Panel>
    );
  }
  return <KitEditor key={kit.id} kitId={kit.id} initial={draftFrom(kit)} />;
}
