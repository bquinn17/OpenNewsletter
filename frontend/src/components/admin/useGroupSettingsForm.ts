import { useEffect } from "react";
import { useFieldArray, useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { usePatchGroup } from "../../api/mutations";
import { ApiError, apiErrorMessage } from "../../api/client";
import { useToasts } from "../../state/toast";
import { GRADIENT_SLUGS } from "../../utils/gradient";
import type { components } from "../../types/api";

type S = components["schemas"];
type GroupResponse = S["GroupResponse"];
type PatchGroupRequest = S["PatchGroupRequest"];

// Mirrors `backend/crates/shared/src/config.rs` and
// `backend/crates/lambda-groups/src/validation.rs` (`docs/design/api.md` §4.3).
export const MAX_NAME_CHARS = 40;
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
        message: `Can't be below the current ${memberCount} member${memberCount === 1 ? "" : "s"}.`,
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

export type FormValues = z.infer<typeof baseSchema>;

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

export function timezoneOptions(current: string): string[] {
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

/**
 * Copy for the `CLOSES_IN_PAST` field error: a `responseWindowDays` change
 * that would close the already-open cycle in the past
 * (`docs/design/lifecycle.md` §6, D10). The form shows its own copy
 * (`docs/design/frontend.md` §7.7) rather than the server's detail.
 */
const CLOSES_IN_PAST_MESSAGE = "Would close the open cycle in the past.";

/** Form state, validation and submit wiring for the group settings form (`docs/design/frontend.md` §7.7). */
export function useGroupSettingsForm(group: GroupResponse) {
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
          if (fieldError.code === "CLOSES_IN_PAST") {
            setError("responseWindowDays", { message: CLOSES_IN_PAST_MESSAGE });
          } else if (formField === "reminderOffsets") {
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
      setError("root", { message: apiErrorMessage(error, "Couldn't save your changes.") });
    }
  });

  return {
    register,
    errors,
    values,
    setValue,
    fields,
    append,
    remove,
    hasChanges,
    isSaving: patchGroup.isPending,
    onSubmit,
  };
}
