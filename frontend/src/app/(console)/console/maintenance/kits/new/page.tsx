"use client";

// A new job kit, blank or copied from one in the catalog (?from=<kit id>).

import { Suspense } from "react";
import { useSearchParams } from "next/navigation";
import { useQuery } from "@tanstack/react-query";
import { desk, draftFrom, emptyDraft } from "@/lib/servicedesk";
import { KitEditor } from "@/components/desk/KitEditor";
import { Skeleton } from "@/components/ui/misc";

export default function NewKitPage() {
  return (
    <Suspense fallback={<Skeleton className="h-96 rounded-2xl" />}>
      <NewKit />
    </Suspense>
  );
}

function NewKit() {
  const from = useSearchParams().get("from");
  const kits = useQuery({
    queryKey: ["kits"],
    queryFn: desk.kits,
    enabled: !!from,
  });
  if (from && kits.isLoading) return <Skeleton className="h-96 rounded-2xl" />;
  const source = from ? kits.data?.find((k) => k.id === from) : undefined;
  const initial = source
    ? { ...draftFrom(source), name: `${source.name} (copy)` }
    : emptyDraft();
  return <KitEditor key={source?.id ?? "blank"} initial={initial} />;
}
