import * as React from "react";
import { cn } from "@/lib/utils";

/**
 * Form field primitives. Work with plain inputs AND react-hook-form: the
 * styled controls spread `{...props}`, so `register("x")` slots straight in
 * (e.g. `<Input {...register("email")} />`). `Field` provides the label /
 * hint / error chrome; the `*Field` wrappers compose the two together.
 */

const controlBase =
  "w-full rounded-lg border border-line bg-surface px-3.5 py-2.5 text-sm text-ink outline-none transition focus:border-accent focus:ring-2 focus:ring-accent/20 placeholder:text-ink-3 disabled:opacity-50";

/** Label + control + optional hint + error chrome. Stacks with space-y-1.5. */
export function Field({
  label,
  htmlFor,
  error,
  hint,
  required,
  children,
  className,
}: {
  label?: React.ReactNode;
  htmlFor?: string;
  error?: string;
  hint?: React.ReactNode;
  required?: boolean;
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <div className={cn("space-y-1.5", className)}>
      {label && (
        <label
          htmlFor={htmlFor}
          className="block text-xs font-semibold text-ink-2"
        >
          {label}
          {required && <span className="ml-0.5 text-bad">*</span>}
        </label>
      )}
      {children}
      {hint && !error && <p className="text-xs text-ink-3">{hint}</p>}
      {error && <p className="text-xs text-bad">{error}</p>}
    </div>
  );
}

export interface InputProps
  extends React.InputHTMLAttributes<HTMLInputElement> {
  error?: boolean;
}

/** Styled `<input>`. Pass `error` to flag invalid state. RHF-ready. */
export const Input = React.forwardRef<HTMLInputElement, InputProps>(
  ({ className, error, ...props }, ref) => (
    <input
      ref={ref}
      className={cn(controlBase, error && "border-bad", className)}
      {...props}
    />
  )
);
Input.displayName = "Input";

export interface TextareaProps
  extends React.TextareaHTMLAttributes<HTMLTextAreaElement> {
  error?: boolean;
}

/** Styled `<textarea>`. Pass `error` to flag invalid state. RHF-ready. */
export const Textarea = React.forwardRef<HTMLTextAreaElement, TextareaProps>(
  ({ className, error, ...props }, ref) => (
    <textarea
      ref={ref}
      className={cn(controlBase, "min-h-[88px]", error && "border-bad", className)}
      {...props}
    />
  )
);
Textarea.displayName = "Textarea";

export interface TextFieldProps
  extends React.InputHTMLAttributes<HTMLInputElement> {
  label?: React.ReactNode;
  error?: string;
  hint?: React.ReactNode;
  required?: boolean;
}

/** `Field` + `Input` in one. Spreads input props (so `register` works). */
export const TextField = React.forwardRef<HTMLInputElement, TextFieldProps>(
  ({ label, error, hint, required, className, id, ...inputProps }, ref) => (
    <Field
      label={label}
      htmlFor={id}
      error={error}
      hint={hint}
      required={required}
      className={className}
    >
      <Input ref={ref} id={id} error={!!error} {...inputProps} />
    </Field>
  )
);
TextField.displayName = "TextField";

export interface TextareaFieldProps
  extends React.TextareaHTMLAttributes<HTMLTextAreaElement> {
  label?: React.ReactNode;
  error?: string;
  hint?: React.ReactNode;
  required?: boolean;
}

/** `Field` + `Textarea` in one. Spreads textarea props (so `register` works). */
export const TextareaField = React.forwardRef<
  HTMLTextAreaElement,
  TextareaFieldProps
>(({ label, error, hint, required, className, id, ...textareaProps }, ref) => (
  <Field
    label={label}
    htmlFor={id}
    error={error}
    hint={hint}
    required={required}
    className={className}
  >
    <Textarea ref={ref} id={id} error={!!error} {...textareaProps} />
  </Field>
));
TextareaField.displayName = "TextareaField";

export interface NativeSelectProps
  extends React.SelectHTMLAttributes<HTMLSelectElement> {
  error?: boolean;
}

/** Styled native `<select>` for simple option lists. RHF-ready. */
export const NativeSelect = React.forwardRef<
  HTMLSelectElement,
  NativeSelectProps
>(({ className, error, ...props }, ref) => (
  <select
    ref={ref}
    className={cn(controlBase, error && "border-bad", className)}
    {...props}
  />
));
NativeSelect.displayName = "NativeSelect";

export interface SelectFieldProps
  extends React.SelectHTMLAttributes<HTMLSelectElement> {
  label?: React.ReactNode;
  error?: string;
  hint?: React.ReactNode;
  required?: boolean;
}

/** `Field` + `NativeSelect` in one. Pass `<option>`s as children. RHF-ready. */
export const SelectField = React.forwardRef<
  HTMLSelectElement,
  SelectFieldProps
>(
  (
    { label, error, hint, required, className, id, children, ...selectProps },
    ref
  ) => (
    <Field
      label={label}
      htmlFor={id}
      error={error}
      hint={hint}
      required={required}
      className={className}
    >
      <NativeSelect ref={ref} id={id} error={!!error} {...selectProps}>
        {children}
      </NativeSelect>
    </Field>
  )
);
SelectField.displayName = "SelectField";
