// Zod validation schemas. Co-located so forms (react-hook-form via
// @hookform/resolvers/zod) and unit tests share the exact same validation.

import { z } from "zod";

/** Scopes a vendor API token may be granted. Keep in sync with the backend. */
export const TOKEN_SCOPES = [
  "listing:read",
  "property:read",
  "application:read",
] as const;

/** "Create API token" form (console/tokens). */
export const createTokenSchema = z.object({
  name: z
    .string()
    .trim()
    .min(2, "Give the token a recognisable name (2+ characters).")
    .max(60, "Keep the name under 60 characters."),
  scopes: z.array(z.enum(TOKEN_SCOPES)).min(1, "Select at least one scope."),
});

export type CreateTokenInput = z.infer<typeof createTokenSchema>;

/** Public rental application form (listings/[id]). */
export const applicationSchema = z.object({
  applicant_name: z.string().trim().min(2, "Enter your full name."),
  email: z.string().trim().email("Enter a valid email address."),
  phone: z.string().trim().optional().or(z.literal("")),
  income: z
    .union([
      z.coerce.number().nonnegative("Income can't be negative."),
      z.literal(""),
    ])
    .optional(),
  move_in: z.string().trim().optional().or(z.literal("")),
});

export type ApplicationInput = z.infer<typeof applicationSchema>;
