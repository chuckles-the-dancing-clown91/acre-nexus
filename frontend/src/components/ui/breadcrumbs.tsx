import * as React from "react";
import Link from "next/link";
import { ChevronRight } from "lucide-react";

import { cn } from "@/lib/utils";

export interface BreadcrumbItem {
  label: string;
  href?: string;
}

/**
 * Breadcrumb trail for detail pages. Items are separated by a chevron; linked
 * items navigate via next/link, while the last/current item is rendered as
 * non-interactive emphasised text.
 */
export function Breadcrumbs({
  items,
  className,
}: {
  items: BreadcrumbItem[];
  className?: string;
}) {
  return (
    <nav
      aria-label="Breadcrumb"
      className={cn("flex items-center gap-1.5 text-sm", className)}
    >
      {items.map((item, index) => {
        const isLast = index === items.length - 1;
        return (
          <React.Fragment key={`${item.label}-${index}`}>
            {index > 0 && (
              <ChevronRight className="h-3.5 w-3.5 shrink-0 text-ink-3" />
            )}
            {item.href && !isLast ? (
              <Link
                href={item.href}
                className="text-ink-3 transition-colors hover:text-ink"
              >
                {item.label}
              </Link>
            ) : (
              <span
                className={cn(
                  isLast ? "font-medium text-ink" : "text-ink-3"
                )}
                aria-current={isLast ? "page" : undefined}
              >
                {item.label}
              </span>
            )}
          </React.Fragment>
        );
      })}
    </nav>
  );
}
