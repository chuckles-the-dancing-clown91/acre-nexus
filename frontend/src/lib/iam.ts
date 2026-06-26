// Pure IAM helpers shared by the admin UI and unit tests.

import type { PermissionDef } from "./api";

/** A category of permissions plus the permissions that belong to it. */
export interface PermissionGroup {
  category: string;
  permissions: PermissionDef[];
}

/**
 * Group a flat permission catalog into ordered category buckets for the role
 * permission matrix. Categories appear in first-seen order; permissions within
 * a category preserve their original order.
 */
export function groupPermissions(
  permissions: PermissionDef[]
): PermissionGroup[] {
  const order: string[] = [];
  const byCategory = new Map<string, PermissionDef[]>();
  for (const p of permissions) {
    let bucket = byCategory.get(p.category);
    if (!bucket) {
      bucket = [];
      byCategory.set(p.category, bucket);
      order.push(p.category);
    }
    bucket.push(p);
  }
  return order.map((category) => ({
    category,
    permissions: byCategory.get(category) ?? [],
  }));
}

/**
 * Render a human-friendly "First Last" name from a status string or persona
 * key (e.g. "platform_admin" -> "Platform admin"). Used for badges/labels.
 */
export function humanizeKey(key: string): string {
  const spaced = key.replace(/[_-]+/g, " ").trim();
  return spaced.charAt(0).toUpperCase() + spaced.slice(1);
}
