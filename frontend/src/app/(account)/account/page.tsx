"use client";

// The account root sends each person to their own home: owners to the owner
// portal, residents to their repairs.

import { useEffect } from "react";
import { useRouter } from "next/navigation";
import { useAuth } from "@/lib/auth";
import { isOwnerOnly } from "@/lib/owner";
import { isVendorOnly } from "@/lib/resident";
import { Skeleton } from "@/components/ui/misc";

export default function AccountHome() {
  const { user, loading } = useAuth();
  const router = useRouter();
  useEffect(() => {
    if (loading) return;
    router.replace(
      isOwnerOnly(user)
        ? "/account/owner"
        : isVendorOnly(user)
          ? "/account/vendor"
          : "/account/maintenance"
    );
  }, [user, loading, router]);
  return <Skeleton className="h-32" />;
}
