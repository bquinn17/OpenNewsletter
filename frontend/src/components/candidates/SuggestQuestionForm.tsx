import { useFieldArray, useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { useCreateCandidate } from "../../api/mutations";
import { ApiError } from "../../api/client";
import { Button } from "../ui/Button";

// Mirrors `backend/crates/shared/src/config.rs` and
// `backend/crates/lambda-questions/src/validation.rs` (`03-api-contract.md` §6.2).
const MIN_PROMPT_CHARS = 5;
const MAX_PROMPT_CHARS = 500;
const MIN_POLL_OPTIONS = 2;
const MAX_POLL_OPTION_LABEL_CHARS = 80;
const MAX_POLL_OPTIONS = 6;

const sparks = [
  "A small win you had…",
  "Last thing that surprised you",
  "Show us a photo of…",
  "A meal worth remembering",
];

const schema = z
  .object({
    kind: z.enum(["text", "poll"]),
    prompt: z
      .string()
      .trim()
      .min(MIN_PROMPT_CHARS, `Add a bit more (at least ${MIN_PROMPT_CHARS} characters)`)
      .max(MAX_PROMPT_CHARS, `${MAX_PROMPT_CHARS} characters max`),
    isAnonymous: z.boolean(),
    pollOptions: z.array(
      z.object({ label: z.string().max(MAX_POLL_OPTION_LABEL_CHARS, "80 characters max") }),
    ),
  })
  .superRefine((val, ctx) => {
    if (val.kind !== "poll") return;
    if (val.pollOptions.length < MIN_POLL_OPTIONS) {
      ctx.addIssue({
        code: "custom",
        path: ["pollOptions"],
        message: `At least ${MIN_POLL_OPTIONS} options`,
      });
    }
    if (val.pollOptions.length > MAX_POLL_OPTIONS) {
      ctx.addIssue({
        code: "custom",
        path: ["pollOptions"],
        message: `${MAX_POLL_OPTIONS} options max`,
      });
    }
    const seen = new Set<string>();
    val.pollOptions.forEach((opt, i) => {
      const label = opt.label.trim();
      if (label.length === 0) {
        ctx.addIssue({ code: "custom", path: ["pollOptions", i, "label"], message: "Required" });
        return;
      }
      if (seen.has(label)) {
        ctx.addIssue({
          code: "custom",
          path: ["pollOptions", i, "label"],
          message: `"${label}" is already an option`,
        });
        return;
      }
      seen.add(label);
    });
  });

type FormValues = z.infer<typeof schema>;

type Props = {
  groupId: string;
  onSuccess: (isAnonymous: boolean) => void;
};

export function SuggestQuestionForm({ groupId, onSuccess }: Props) {
  const createCandidate = useCreateCandidate(groupId);

  const {
    register,
    handleSubmit,
    watch,
    setValue,
    setError,
    control,
    formState: { errors, isValid },
  } = useForm<FormValues>({
    resolver: zodResolver(schema),
    mode: "onChange",
    defaultValues: {
      kind: "text",
      prompt: "",
      isAnonymous: false,
      pollOptions: [{ label: "" }, { label: "" }],
    },
  });
  const { fields, append, remove } = useFieldArray({ control, name: "pollOptions" });

  const kind = watch("kind");
  const prompt = watch("prompt") ?? "";
  const isAnonymous = watch("isAnonymous") ?? false;

  const onSubmit = handleSubmit(async (values) => {
    try {
      await createCandidate.mutateAsync({
        kind: values.kind,
        prompt: values.prompt,
        isAnonymous: values.isAnonymous,
        ...(values.kind === "poll"
          ? { pollOptions: values.pollOptions.map((o) => ({ label: o.label.trim() })) }
          : {}),
      });
      onSuccess(values.isAnonymous);
    } catch (error) {
      if (error instanceof ApiError && error.code === "VALIDATION_FAILED") {
        const fieldErrors = error.problem?.fieldErrors ?? [];
        if (fieldErrors.length === 0) {
          setError("root", { message: error.problem?.detail ?? "Couldn't submit your question." });
        }
        for (const fieldError of fieldErrors) {
          if (fieldError.field === "prompt") {
            setError("prompt", { message: fieldError.message });
          } else if (fieldError.field === "pollOptions") {
            setError("pollOptions", { message: fieldError.message });
          } else {
            setError("root", { message: fieldError.message });
          }
        }
        return;
      }
      if (error instanceof ApiError && error.code === "CYCLE_NOT_VOTING") {
        setError("root", {
          message: "Voting for the next edition isn't open right now — check back soon.",
        });
        return;
      }
      setError("root", { message: "Couldn't submit your question. Try again." });
    }
  });

  return (
    <form onSubmit={onSubmit}>
      {errors.root?.message && (
        <div className="px-5 pt-4">
          <p
            role="alert"
            aria-live="polite"
            className="rounded-2xl bg-coral/10 p-3 text-sm text-coral"
          >
            {errors.root.message}
          </p>
        </div>
      )}

      <div className="px-5 pt-5">
        <label
          className={`block flex cursor-pointer gap-3 rounded-3xl border p-4 text-sm transition ${
            isAnonymous ? "border-grape/40 bg-grape/5 text-grape" : "border-line bg-white text-ink"
          }`}
        >
          <span className="text-lg">{isAnonymous ? "🎭" : "💬"}</span>
          <div className="flex-1">
            <div className="flex items-center justify-between gap-3">
              <strong>{isAnonymous ? "Anonymous to the group" : "Asked by you"}</strong>
              <input type="checkbox" className="peer sr-only" {...register("isAnonymous")} />
              <span
                aria-hidden
                className={`relative h-6 w-10 shrink-0 rounded-full transition-colors ${
                  isAnonymous ? "bg-grape" : "bg-line"
                }`}
              >
                <span
                  className={`absolute left-0.5 top-0.5 h-5 w-5 rounded-full bg-white shadow transition-transform ${
                    isAnonymous ? "translate-x-4" : ""
                  }`}
                />
              </span>
            </div>
            <div className="mt-1 text-xs opacity-90">
              {isAnonymous
                ? "Admins can still see who asked — your friends won't."
                : "Your friends see “You asked: …” next to this question."}
            </div>
          </div>
        </label>
      </div>

      <div className="mt-5 px-5">
        <div className="mb-2 text-xs font-bold uppercase tracking-widest text-inkmuted">Type</div>
        <div className="grid grid-cols-2 gap-3">
          <KindButton
            active={kind === "text"}
            onClick={() => setValue("kind", "text", { shouldValidate: true })}
            icon="📝"
            label="Open"
            hint="Friends write a free-form answer."
          />
          <KindButton
            active={kind === "poll"}
            onClick={() => setValue("kind", "poll", { shouldValidate: true })}
            icon="📊"
            label="Poll"
            hint="Pick one of your options."
          />
        </div>
      </div>

      <div className="mt-5 px-5">
        <label
          htmlFor="suggest-prompt"
          className="mb-2 block text-xs font-bold uppercase tracking-widest text-inkmuted"
        >
          Your question
        </label>
        <textarea
          id="suggest-prompt"
          {...register("prompt")}
          aria-invalid={!!errors.prompt}
          aria-describedby="suggest-prompt-hint"
          className="min-h-[100px] w-full rounded-2xl border border-line bg-white px-4 py-3 text-[15px] focus:outline-none focus:ring-2 focus:ring-coral/30"
          placeholder="Try something specific. The best questions invite stories."
        />
        <div
          id="suggest-prompt-hint"
          aria-live="polite"
          className="mt-1.5 flex items-center justify-between text-xs"
        >
          <span className={errors.prompt ? "font-semibold text-coral" : "text-inkmuted"}>
            {errors.prompt?.message ?? `${MIN_PROMPT_CHARS}–${MAX_PROMPT_CHARS} characters`}
          </span>
          <span className="text-inkmuted">
            {prompt.length} / {MAX_PROMPT_CHARS}
          </span>
        </div>
      </div>

      {kind === "poll" && (
        <div className="mt-5 px-5">
          <p className="mb-2 block text-xs font-bold uppercase tracking-widest text-inkmuted">
            Options
          </p>
          <div className="space-y-2">
            {fields.map((field, i) => (
              <div key={field.id} className="flex items-center gap-2">
                <label htmlFor={`option-${field.id}`} className="sr-only">
                  Option {i + 1}
                </label>
                <input
                  id={`option-${field.id}`}
                  {...register(`pollOptions.${i}.label` as const)}
                  placeholder={`Option ${i + 1}`}
                  className="flex-1 rounded-2xl border border-line bg-white px-4 py-2.5 text-sm focus:outline-none focus:ring-2 focus:ring-coral/30"
                />
                {fields.length > MIN_POLL_OPTIONS && (
                  <button
                    type="button"
                    onClick={() => remove(i)}
                    className="h-9 w-9 rounded-full border border-line text-inkmuted hover:border-coral hover:text-coral"
                    aria-label={`Remove option ${i + 1}`}
                  >
                    ×
                  </button>
                )}
              </div>
            ))}
          </div>
          {fields.length < MAX_POLL_OPTIONS && (
            <button
              type="button"
              onClick={() => append({ label: "" })}
              className="mt-2 text-sm font-semibold text-grape"
            >
              ＋ Add option
            </button>
          )}
          {errors.pollOptions?.message && (
            <div aria-live="polite" className="mt-2 text-xs font-semibold text-coral">
              {errors.pollOptions.message}
            </div>
          )}
          {!errors.pollOptions?.message &&
            fields.map((field, i) => {
              const message = errors.pollOptions?.[i]?.label?.message;
              return message ? (
                <div
                  key={field.id}
                  aria-live="polite"
                  className="mt-2 text-xs font-semibold text-coral"
                >
                  {message}
                </div>
              ) : null;
            })}
        </div>
      )}

      <div className="mt-5 px-5">
        <div className="mb-2 text-xs font-bold uppercase tracking-widest text-inkmuted">
          A few sparks if you&apos;re stuck
        </div>
        <div className="flex flex-wrap gap-2">
          {sparks.map((s) => (
            <button
              key={s}
              type="button"
              onClick={() => setValue("prompt", s, { shouldValidate: true })}
              className="rounded-full border border-line bg-white px-3 py-1.5 text-sm hover:border-ink"
            >
              {s}
            </button>
          ))}
        </div>
      </div>

      <div className="mt-6 px-5">
        <Button type="submit" disabled={!isValid || createCandidate.isPending} className="w-full">
          {createCandidate.isPending
            ? "Submitting…"
            : isAnonymous
              ? "Submit anonymously"
              : "Submit"}
        </Button>
      </div>
    </form>
  );
}

function KindButton({
  active,
  onClick,
  icon,
  label,
  hint,
}: {
  active: boolean;
  onClick: () => void;
  icon: string;
  label: string;
  hint: string;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-pressed={active}
      className={`rounded-2xl bg-white p-4 text-left transition ${
        active ? "border-2 border-coral ring-2 ring-coral/20" : "border border-line"
      }`}
    >
      <div className="mb-1 text-2xl">{icon}</div>
      <div className="font-semibold">{label}</div>
      <div className="text-xs text-inkmuted">{hint}</div>
    </button>
  );
}
