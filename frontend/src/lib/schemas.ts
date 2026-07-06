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

// ---- IAM forms ---------------------------------------------------------------

const optionalText = z.string().trim().optional().or(z.literal(""));

/** "New user" form (console/platform/users). */
export const createUserSchema = z.object({
  email: z.string().trim().email("Enter a valid email address."),
  name: z.string().trim().min(2, "Enter the user's full name."),
  username: optionalText,
  password: optionalText,
  scope: z.enum(["platform", "tenant"]),
  tenant_id: optionalText,
  profile_type: optionalText,
  title: optionalText,
  legal_first_name: optionalText,
  legal_last_name: optionalText,
  phone: optionalText,
});

export type CreateUserInputForm = z.infer<typeof createUserSchema>;

/** "Edit profile" form (console/platform/users/[id]). */
export const profileFormSchema = z.object({
  legal_first_name: optionalText,
  legal_middle_name: optionalText,
  legal_last_name: optionalText,
  preferred_name: optionalText,
  date_of_birth: optionalText,
  phone: optionalText,
  address_line1: optionalText,
  address_line2: optionalText,
  city: optionalText,
  region: optionalText,
  postal_code: optionalText,
  country: optionalText,
  ssn: optionalText,
  gov_id_type: optionalText,
  gov_id_number: optionalText,
  // Renter attributes (application auto-fill).
  has_pet: z.boolean().optional(),
  pet_details: optionalText,
  is_military: z.boolean().optional(),
  annual_income: optionalText,
});

export type ProfileFormInput = z.infer<typeof profileFormSchema>;

/** "New role" form (console/platform/roles). */
export const createRoleSchema = z.object({
  scope: z.enum(["platform", "tenant"]),
  tenant_id: optionalText,
  key: z
    .string()
    .trim()
    .min(2, "Give the role a key (2+ characters).")
    .regex(/^[a-z0-9_]+$/, "Use lowercase letters, digits and underscores."),
  name: z.string().trim().min(2, "Give the role a name."),
  description: optionalText,
});

export type CreateRoleInputForm = z.infer<typeof createRoleSchema>;

/** "Invite member" form (console/members). */
export const inviteMemberSchema = z.object({
  email: z.string().trim().email("Enter a valid email address."),
  name: z.string().trim().min(2, "Enter the member's full name."),
  profile_type: z.string().trim().min(1, "Pick a persona."),
  title: optionalText,
});

export type InviteMemberInputForm = z.infer<typeof inviteMemberSchema>;

// ---- Accounts payable (#58) ----------------------------------------------------

/** "New vendor bill" form (console/payables). Amount is entered in dollars. */
export const createPayableSchema = z.object({
  counterparty_id: z.string().trim().min(1, "Pick the vendor being paid."),
  property_id: optionalText,
  entity_id: optionalText,
  maintenance_ticket_id: optionalText,
  memo: z.string().trim().min(2, "Describe what the bill is for."),
  amount: z.coerce
    .number()
    .positive("Enter the bill amount (must be positive)."),
  due_date: optionalText,
});

export type CreatePayableInputForm = z.infer<typeof createPayableSchema>;

// ---- Calendar / reminders (#54) -------------------------------------------------

/** Subject types a reminder can carry. Keep in sync with the backend. */
export const REMINDER_SUBJECTS = [
  "lease",
  "license",
  "insurance",
  "tour",
  "inspection",
  "custom",
] as const;

/** "New reminder" form (console/calendar). Lead days are comma-separated. */
export const createReminderSchema = z.object({
  subject_type: z.enum(REMINDER_SUBJECTS),
  title: z.string().trim().min(2, "Give the reminder a title."),
  description: optionalText,
  due_date: z
    .string()
    .trim()
    .regex(/^\d{4}-\d{2}-\d{2}$/, "Pick a due date."),
  lead_days: optionalText,
  recipients: optionalText,
});

export type CreateReminderInputForm = z.infer<typeof createReminderSchema>;
