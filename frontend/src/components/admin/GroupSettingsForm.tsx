import { useEffect, type ReactNode } from "react";
import { useFieldArray, useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import clsx from "clsx";
import { usePatchGroup } from "../../api/mutations";
import { ApiError } from "../../api/client";
import { useToasts } from "../../state/toast";
import { Button } from "../ui/Button";
import { GRADIENT_SLUGS, gradientClasses } from "../../utils/gradient";
import type { components } from "../../types/api";

type S = components["schemas"];
type GroupResponse = S["GroupResponse"];
type PatchGroupRequest = S["PatchGroupRequest"];

// Mirrors `backend/crates/shared/src/config.rs` and
// `backend/crates/lambda-groups/src/validation.rs` (`03-api-contract.md` §4.3).
const MAX_NAME_CHARS = 40;
const MIN_QUESTIONS_PER_CYCLE = 1;
const MAX_QUESTIONS_PER_CYCLE = 20;
const MIN_RESPONSE_WINDOW_DAYS = 1;
const MAX_RESPONSE_WINDOW_DAYS = 28;
const MAX_REMINDER_OFFSET_HOURS = 168;

const baseSchema = z.object({
  name: z
    .string()
    .trim()
    .min(1, "Name can't be empty.")
    .max(MAX_NAME_CHARS, `${MAX_NAME_CHARS} characters max.`),
  gradient: z.enum(GRADIENT_SLUGS),
  timezone: z.string().min(1, "Pick a timezone."),
  questionsPerCycle: z.coerce
    .number({ invalid_type_error: "Enter a number." })
    .int("Whole numbers only.")
    .min(MIN_QUESTIONS_PER_CYCLE, `At least ${MIN_QUESTIONS_PER_CYCLE}.`)
    .max(MAX_QUESTIONS_PER_CYCLE, `${MAX_QUESTIONS_PER_CYCLE} max.`),
  votesPerUserPerCycle: z.coerce
    .number({ invalid_type_error: "Enter a number." })
    .int("Whole numbers only.")
    .min(1, "At least 1."),
  responseWindowDays: z.coerce
    .number({ invalid_type_error: "Enter a number." })
    .int("Whole numbers only.")
    .min(MIN_RESPONSE_WINDOW_DAYS, `At least ${MIN_RESPONSE_WINDOW_DAYS} day.`)
    .max(MAX_RESPONSE_WINDOW_DAYS, `${MAX_RESPONSE_WINDOW_DAYS} days max.`),
  memberSoftCap: z.coerce
    .number({ invalid_type_error: "Enter a number." })
    .int("Whole numbers only.")
    .min(0, "Can't be negative."),
  onCycleOpen: z.boolean(),
  reminderOffsets: z
    .array(
      z.object({
        hours: z.coerce
          .number({ invalid_type_error: "Enter a number." })
          .int("Whole hours only.")
          .min(1, "At least 1 hour.")
          .max(MAX_REMINDER_OFFSET_HOURS, `${MAX_REMINDER_OFFSET_HOURS} hours max.`),
      }),
    )
    .min(1, "Add at least one reminder."),
});

/** `memberCount` is the group's current member count, not a fixed bound — only known per-group at render time. */
function buildSchema(memberCount: number) {
  return baseSchema.superRefine((val, ctx) => {
    if (val.votesPerUserPerCycle > val.questionsPerCycle) {
      ctx.addIssue({
        code: "custom",
        path: ["votesPerUserPerCycle"],
        message: `Can't exceed questions per cycle (${val.questionsPerCycle}).`,
      });
    }
    if (val.memberSoftCap < memberCount) {
      ctx.addIssue({
        code: "custom",
        path: ["memberSoftCap"],
        message: `Can't be below the current ${memberCount} members.`,
      });
    }
    const seen = new Set<number>();
    val.reminderOffsets.forEach((offset, i) => {
      if (seen.has(offset.hours)) {
        ctx.addIssue({
          code: "custom",
          path: ["reminderOffsets", i, "hours"],
          message: "Duplicate reminder.",
        });
      }
      seen.add(offset.hours);
    });
  });
}

type FormValues = z.infer<typeof baseSchema>;

/** The fields that compare and patch as a single `cycleSettings` sub-object. */
type CycleFields = Pick<
  FormValues,
  "questionsPerCycle" | "votesPerUserPerCycle" | "responseWindowDays"
>;

function toFormValues(group: GroupResponse): FormValues {
  return {
    name: group.name,
    gradient: group.gradient,
    timezone: group.timezone,
    questionsPerCycle: group.cycleSettings.questionsPerCycle,
    votesPerUserPerCycle: group.cycleSettings.votesPerUserPerCycle,
    responseWindowDays: group.cycleSettings.responseWindowDays,
    memberSoftCap: group.memberSoftCap,
    onCycleOpen: group.notificationSettings.onCycleOpen,
    reminderOffsets: group.notificationSettings.offsetsHoursBeforeClose.map((hours) => ({
      hours,
    })),
  };
}

function cycleFieldsEqual(a: CycleFields, b: CycleFields): boolean {
  return (
    a.questionsPerCycle === b.questionsPerCycle &&
    a.votesPerUserPerCycle === b.votesPerUserPerCycle &&
    a.responseWindowDays === b.responseWindowDays
  );
}

function sortedOffsets(values: FormValues): number[] {
  return [...new Set(values.reminderOffsets.map((o) => Number(o.hours)))].sort((a, b) => b - a);
}

/**
 * `watch()` reflects the DOM's raw (string) input value for a number field
 * the moment it's edited, even back to its original value — so comparing it
 * straight against the numeric baseline from `toFormValues` would call it
 * "changed" forever after one keystroke. Coercing both sides the same way
 * `buildPatch` already does for `reminderOffsets` keeps "unsaved changes"
 * accurate for the Save button.
 */
function normalizeForComparison(values: FormValues): FormValues {
  return {
    ...values,
    questionsPerCycle: Number(values.questionsPerCycle),
    votesPerUserPerCycle: Number(values.votesPerUserPerCycle),
    responseWindowDays: Number(values.responseWindowDays),
    memberSoftCap: Number(values.memberSoftCap),
    reminderOffsets: values.reminderOffsets.map((o) => ({ hours: Number(o.hours) })),
  };
}

/**
 * Diffs the submitted form against its loaded baseline into a
 * `PatchGroupRequest` carrying only what changed — `cycleSettings` and
 * `notificationSettings` are sent whole when any one of their fields changed
 * (`PatchGroupRequest`'s sub-objects replace, not merge, server-side).
 */
function buildPatch(baseline: FormValues, next: FormValues): PatchGroupRequest {
  const patch: PatchGroupRequest = {};
  const trimmedName = next.name.trim();
  if (trimmedName !== baseline.name) patch.name = trimmedName;
  if (next.gradient !== baseline.gradient) patch.gradient = next.gradient;
  if (next.timezone !== baseline.timezone) patch.timezone = next.timezone;
  if (next.memberSoftCap !== baseline.memberSoftCap) patch.memberSoftCap = next.memberSoftCap;

  if (!cycleFieldsEqual(baseline, next)) {
    patch.cycleSettings = {
      questionsPerCycle: next.questionsPerCycle,
      votesPerUserPerCycle: next.votesPerUserPerCycle,
      responseWindowDays: next.responseWindowDays,
    };
  }

  const nextOffsets = sortedOffsets(next);
  const baselineOffsets = sortedOffsets(baseline);
  const offsetsChanged =
    nextOffsets.length !== baselineOffsets.length ||
    nextOffsets.some((h, i) => h !== baselineOffsets[i]);
  if (offsetsChanged || next.onCycleOpen !== baseline.onCycleOpen) {
    patch.notificationSettings = {
      offsetsHoursBeforeClose: nextOffsets,
      onCycleOpen: next.onCycleOpen,
    };
  }

  return patch;
}

function timezoneOptions(current: string): string[] {
  if (typeof Intl.supportedValuesOf !== "function") return [current];
  const zones = Intl.supportedValuesOf("timeZone");
  return zones.includes(current) ? zones : [current, ...zones];
}

// Server field names that map onto this form's fields, so a 422's
// `fieldErrors` can be shown inline.
const SERVER_FIELD_TO_FORM_FIELD: Record<string, keyof FormValues> = {
  name: "name",
  gradient: "gradient",
  timezone: "timezone",
  questionsPerCycle: "questionsPerCycle",
  votesPerUserPerCycle: "votesPerUserPerCycle",
  responseWindowDays: "responseWindowDays",
  offsetsHoursBeforeClose: "reminderOffsets",
  memberSoftCap: "memberSoftCap",
};

type Props = {
  group: GroupResponse;
};

export function GroupSettingsForm({ group }: Props) {
  const patchGroup = usePatchGroup(group.groupId);
  const pushToast = useToasts((s) => s.push);

  const {
    register,
    handleSubmit,
    watch,
    setValue,
    setError,
    reset,
    control,
    formState: { errors },
  } = useForm<FormValues>({
    resolver: zodResolver(buildSchema(group.memberCount)),
    defaultValues: toFormValues(group),
  });
  const { fields, append, remove } = useFieldArray({ control, name: "reminderOffsets" });

  // Re-baseline whenever the server's copy changes — including right after
  // this form's own save (`usePatchGroup` writes the response into the
  // cache), so the "unsaved changes" comparison below starts fresh.
  useEffect(() => {
    reset(toFormValues(group));
  }, [group, reset]);

  const values = watch();
  const patchPreview = buildPatch(toFormValues(group), normalizeForComparison(values));
  const hasChanges = Object.keys(patchPreview).length > 0;

  const onSubmit = handleSubmit(async (parsed) => {
    const patch = buildPatch(toFormValues(group), parsed);
    if (Object.keys(patch).length === 0) return;
    try {
      await patchGroup.mutateAsync(patch);
      pushToast("Group settings saved.", "success");
    } catch (error) {
      if (error instanceof ApiError && error.code === "VALIDATION_FAILED") {
        const fieldErrors = error.problem?.fieldErrors ?? [];
        for (const fieldError of fieldErrors) {
          const formField = SERVER_FIELD_TO_FORM_FIELD[fieldError.field];
          if (formField === "reminderOffsets") {
            setError("reminderOffsets", { message: fieldError.message });
          } else if (formField) {
            setError(formField, { message: fieldError.message });
          } else {
            setError("root", { message: fieldError.message });
          }
        }
        if (fieldErrors.length === 0) {
          setError("root", { message: error.problem?.detail ?? "Couldn't save your changes." });
        }
        return;
      }
      setError("root", {
        message:
          error instanceof ApiError
            ? (error.problem?.detail ?? error.message)
            : "Couldn't save your changes.",
      });
    }
  });

  return (
    <form onSubmit={onSubmit} className="space-y-4 px-5 pt-4">
      {errors.root?.message && (
        <p
          role="alert"
          aria-live="polite"
          className="rounded-2xl bg-coral/10 p-3 text-sm text-coral"
        >
          {errors.root.message}
        </p>
      )}

      <Field id="group-name" label="Group name" error={errors.name?.message}>
        <input
          id="group-name"
          {...register("name")}
          maxLength={MAX_NAME_CHARS}
          aria-invalid={!!errors.name}
          className="w-full rounded-2xl border border-line bg-cream px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-coral/30"
        />
      </Field>

      <fieldset>
        <legend className="mb-2 text-xs font-bold uppercase tracking-widest text-inkmuted">
          Gradient
        </legend>
        <div role="radiogroup" aria-label="Group gradient" className="flex flex-wrap gap-2">
          {GRADIENT_SLUGS.map((slug) => (
            <label key={slug} className="cursor-pointer">
              <input
                type="radio"
                value={slug}
                checked={values.gradient === slug}
                onChange={() => setValue("gradient", slug, { shouldValidate: true })}
                className="peer sr-only"
                aria-label={slug}
              />
              <span
                className={clsx(
                  "block h-9 w-9 rounded-full ring-offset-2 ring-offset-white transition",
                  gradientClasses(slug),
                  values.gradient === slug && "ring-2 ring-ink",
                )}
              />
            </label>
          ))}
        </div>
      </fieldset>

      <Field
        id="group-timezone"
        label="Timezone"
        error={errors.timezone?.message}
        hint="Changing this reschedules the current voting cycle's dates."
      >
        {typeof Intl.supportedValuesOf === "function" ? (
          <select
            id="group-timezone"
            {...register("timezone")}
            className="w-full rounded-2xl border border-line bg-cream px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-coral/30"
          >
            {timezoneOptions(group.timezone).map((tz) => (
              <option key={tz} value={tz}>
                {tz}
              </option>
            ))}
          </select>
        ) : (
          <input
            id="group-timezone"
            {...register("timezone")}
            className="w-full rounded-2xl border border-line bg-cream px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-coral/30"
          />
        )}
      </Field>

      <Field
        id="group-questions-per-cycle"
        label="Questions per cycle"
        error={errors.questionsPerCycle?.message}
      >
        <input
          id="group-questions-per-cycle"
          type="number"
          {...register("questionsPerCycle")}
          className="w-24 rounded-2xl border border-line bg-cream px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-coral/30"
        />
      </Field>

      <Field
        id="group-votes-per-cycle"
        label="Votes per member per cycle"
        error={errors.votesPerUserPerCycle?.message}
      >
        <input
          id="group-votes-per-cycle"
          type="number"
          {...register("votesPerUserPerCycle")}
          className="w-24 rounded-2xl border border-line bg-cream px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-coral/30"
        />
      </Field>

      <Field
        id="group-response-window"
        label="Response window (days)"
        error={errors.responseWindowDays?.message}
        hint="Changing this reschedules the current voting cycle's dates."
      >
        <input
          id="group-response-window"
          type="number"
          {...register("responseWindowDays")}
          className="w-24 rounded-2xl border border-line bg-cream px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-coral/30"
        />
      </Field>

      <fieldset>
        <legend className="mb-2 text-xs font-bold uppercase tracking-widest text-inkmuted">
          Reminders before close (hours)
        </legend>
        <div className="space-y-2">
          {fields.map((field, i) => (
            <div key={field.id} className="flex items-center gap-2">
              <label htmlFor={`reminder-${field.id}`} className="sr-only">
                Reminder {i + 1} (hours before close)
              </label>
              <input
                id={`reminder-${field.id}`}
                type="number"
                {...register(`reminderOffsets.${i}.hours` as const)}
                className="w-24 rounded-2xl border border-line bg-cream px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-coral/30"
              />
              {fields.length > 1 && (
                <button
                  type="button"
                  onClick={() => remove(i)}
                  aria-label={`Remove reminder ${i + 1}`}
                  className="h-9 w-9 rounded-full border border-line text-inkmuted hover:border-coral hover:text-coral"
                >
                  ×
                </button>
              )}
            </div>
          ))}
        </div>
        <button
          type="button"
          onClick={() => append({ hours: 24 })}
          className="mt-2 text-sm font-semibold text-grape"
        >
          ＋ Add reminder
        </button>
        {errors.reminderOffsets?.message && (
          <p aria-live="polite" className="mt-2 text-xs font-semibold text-coral">
            {errors.reminderOffsets.message}
          </p>
        )}
        {fields.map((field, i) => {
          const message = errors.reminderOffsets?.[i]?.hours?.message;
          return message ? (
            <p key={field.id} aria-live="polite" className="mt-1 text-xs font-semibold text-coral">
              {message}
            </p>
          ) : null;
        })}
      </fieldset>

      <div className="flex items-center gap-3 rounded-2xl border border-line bg-cream px-3 py-3">
        <div className="flex-1 text-sm">Notify members when a cycle opens</div>
        <button
          type="button"
          onClick={() => setValue("onCycleOpen", !values.onCycleOpen, { shouldValidate: true })}
          className={`toggle ${values.onCycleOpen ? "on" : ""}`}
          aria-label="Toggle cycle-open notifications"
        />
      </div>

      <Field
        id="group-member-soft-cap"
        label="Member soft cap"
        error={errors.memberSoftCap?.message}
      >
        <input
          id="group-member-soft-cap"
          type="number"
          {...register("memberSoftCap")}
          className="w-24 rounded-2xl border border-line bg-cream px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-coral/30"
        />
      </Field>

      <div className="flex justify-end">
        <Button type="submit" disabled={!hasChanges || patchGroup.isPending}>
          {patchGroup.isPending ? "Saving…" : "Save"}
        </Button>
      </div>
    </form>
  );
}

function Field({
  id,
  label,
  error,
  hint,
  children,
}: {
  id: string;
  label: string;
  error?: string;
  hint?: string;
  children: ReactNode;
}) {
  return (
    <div>
      <label
        htmlFor={id}
        className="mb-1 block text-xs font-bold uppercase tracking-widest text-inkmuted"
      >
        {label}
      </label>
      {children}
      {hint && !error && <p className="mt-1 text-xs text-inkmuted">{hint}</p>}
      {error && (
        <p role="alert" aria-live="polite" className="mt-1 text-xs font-semibold text-coral">
          {error}
        </p>
      )}
    </div>
  );
}
