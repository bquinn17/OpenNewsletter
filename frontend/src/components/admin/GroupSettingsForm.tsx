import type { ReactNode } from "react";
import clsx from "clsx";
import { Button } from "../ui/Button";
import { GRADIENT_SLUGS, gradientClasses } from "../../utils/gradient";
import type { components } from "../../types/api";
import { MAX_NAME_CHARS, timezoneOptions, useGroupSettingsForm } from "./useGroupSettingsForm";

type S = components["schemas"];
type GroupResponse = S["GroupResponse"];

type Props = {
  group: GroupResponse;
};

export function GroupSettingsForm({ group }: Props) {
  const {
    register,
    errors,
    values,
    setValue,
    fields,
    append,
    remove,
    hasChanges,
    isSaving,
    onSubmit,
  } = useGroupSettingsForm(group);

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
        hint="Timezone changes affect the current voting cycle's open time."
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
        hint="Moves the current voting cycle's close, and an open cycle's close if it's still in the future."
      >
        <input
          id="group-response-window"
          type="number"
          {...register("responseWindowDays")}
          className="w-24 rounded-2xl border border-line bg-cream px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-coral/30"
        />
      </Field>

      <p className="text-xs text-inkmuted">Changes apply to the current cycle where possible.</p>

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
          className={clsx("toggle", values.onCycleOpen && "on")}
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
        <Button type="submit" disabled={!hasChanges || isSaving}>
          {isSaving ? "Saving…" : "Save"}
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
