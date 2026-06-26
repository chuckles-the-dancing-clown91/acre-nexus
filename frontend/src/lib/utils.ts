import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

/**
 * shadcn/ui's class-merging helper: combines conditional classes (clsx) and
 * resolves conflicting Tailwind utilities (tailwind-merge). Used by every
 * shadcn component under `components/ui/*`.
 */
export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
