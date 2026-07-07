import { useNavigate, useParams } from "react-router-dom";
import { useFieldArray, useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { useSuggestCandidate } from "../api/queries";
import { useToasts } from "../state/toast";
import { PageHeader } from "../components/layout/PageHeader";
import { Button } from "../components/ui/Button";

const sparks = [
  "A small win you had…",
  "Last thing that surprised you",
  "Show us a photo of…",
  "A meal worth remembering",
];

const schema = z
  .object({
    kind: z.enum(["text", "poll"]),
    prompt: z.string().min(5, "Add a bit more").max(500, "500 characters max"),
    isAnonymous: z.boolean(),
    options: z
      .array(z.object({ label: z.string().min(1).max(80) }))
      .max(6, "6 options max"),
  })
  .superRefine((val, ctx) => {
    if (val.kind === "poll") {
      if (val.options.length < 2) {
        ctx.addIssue({ code: "custom", path: ["options"], message: "At least 2 options" });
      }
      const labels = val.options.map((o) => o.label.trim().toLowerCase());
      if (new Set(labels).size !== labels.length) {
        ctx.addIssue({ code: "custom", path: ["options"], message: "Options must be unique" });
      }
    }
  });

type FormValues = z.infer<typeof schema>;

export function SuggestPage() {
  const { groupId = "" } = useParams();
  const navigate = useNavigate();
  const suggest = useSuggestCandidate(groupId);
  const pushToast = useToasts((s) => s.push);

  const {
    register,
    handleSubmit,
    watch,
    setValue,
    control,
    formState: { errors, isValid },
  } = useForm<FormValues>({
    resolver: zodResolver(schema),
    mode: "onChange",
    defaultValues: {
      kind: "text",
      prompt: "",
      isAnonymous: false,
      options: [{ label: "" }, { label: "" }],
    },
  });
  const { fields, append, remove } = useFieldArray({ control, name: "options" });

  const kind = watch("kind");
  const prompt = watch("prompt") ?? "";
  const isAnonymous = watch("isAnonymous") ?? false;

  const onSubmit = handleSubmit(async (values) => {
    try {
      await suggest.mutateAsync({
        kind: values.kind,
        prompt: values.prompt,
        pollOptions: values.kind === "poll" ? values.options.map((o) => o.label) : [],
        isAnonymous: values.isAnonymous,
      });
      pushToast(
        values.isAnonymous ? "Question submitted anonymously." : "Question submitted.",
        "success",
      );
      navigate(`/g/${groupId}/upcoming`);
    } catch (e) {
      pushToast((e as Error).message ?? "Couldn't submit", "error");
    }
  });

  return (
    <div className="bg-cream pb-12">
      <PageHeader eyebrow="For next month" title="Suggest a question" back={`/g/${groupId}/upcoming`} />

      <form onSubmit={onSubmit}>
        <div className="px-5 pt-5">
          <label
            className={`block rounded-3xl p-4 text-sm flex gap-3 cursor-pointer transition border ${
              isAnonymous
                ? "bg-grape/5 border-grape/40 text-grape"
                : "bg-white border-line text-ink"
            }`}
          >
            <span className="text-lg">{isAnonymous ? "🎭" : "💬"}</span>
            <div className="flex-1">
              <div className="flex items-center justify-between gap-3">
                <strong>
                  {isAnonymous ? "Anonymous to the group" : "Asked by you"}
                </strong>
                <input
                  type="checkbox"
                  className="sr-only peer"
                  {...register("isAnonymous")}
                />
                <span
                  aria-hidden
                  className={`relative w-10 h-6 rounded-full transition-colors shrink-0 ${
                    isAnonymous ? "bg-grape" : "bg-line"
                  }`}
                >
                  <span
                    className={`absolute top-0.5 left-0.5 w-5 h-5 rounded-full bg-white shadow transition-transform ${
                      isAnonymous ? "translate-x-4" : ""
                    }`}
                  />
                </span>
              </div>
              <div className="text-xs mt-1 opacity-90">
                {isAnonymous
                  ? "Your friends see the question, but not who suggested it."
                  : "Your friends see “You asked: …” next to this question."}
              </div>
            </div>
          </label>
        </div>

        <div className="px-5 mt-5">
          <div className="text-xs uppercase tracking-widest text-inkmuted font-bold mb-2">Type</div>
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

        <div className="px-5 mt-5">
          <label className="block text-xs uppercase tracking-widest text-inkmuted font-bold mb-2">Your question</label>
          <textarea
            {...register("prompt")}
            className="w-full bg-white border border-line rounded-2xl px-4 py-3 text-[15px] min-h-[100px] focus:outline-none focus:ring-2 focus:ring-coral/30"
            placeholder="Try something specific. The best questions invite stories."
          />
          <div className="flex items-center justify-between text-xs mt-1.5">
            <span className={errors.prompt ? "text-coral font-semibold" : "text-inkmuted"}>
              {errors.prompt?.message ?? "5–500 characters"}
            </span>
            <span className="text-inkmuted">{prompt.length} / 500</span>
          </div>
        </div>

        {kind === "poll" && (
          <div className="px-5 mt-5">
            <label className="block text-xs uppercase tracking-widest text-inkmuted font-bold mb-2">Options</label>
            <div className="space-y-2">
              {fields.map((field, i) => (
                <div key={field.id} className="flex items-center gap-2">
                  <input
                    {...register(`options.${i}.label` as const)}
                    placeholder={`Option ${i + 1}`}
                    className="flex-1 bg-white border border-line rounded-2xl px-4 py-2.5 text-sm focus:outline-none focus:ring-2 focus:ring-coral/30"
                  />
                  {fields.length > 2 && (
                    <button
                      type="button"
                      onClick={() => remove(i)}
                      className="w-9 h-9 rounded-full border border-line text-inkmuted hover:border-coral hover:text-coral"
                      aria-label={`Remove option ${i + 1}`}
                    >
                      ×
                    </button>
                  )}
                </div>
              ))}
            </div>
            {fields.length < 6 && (
              <button
                type="button"
                onClick={() => append({ label: "" })}
                className="mt-2 text-grape font-semibold text-sm"
              >
                ＋ Add option
              </button>
            )}
            {errors.options && (
              <div className="text-coral text-xs font-semibold mt-2">{errors.options.message ?? "Check options"}</div>
            )}
          </div>
        )}

        <div className="px-5 mt-5">
          <div className="text-xs uppercase tracking-widest text-inkmuted font-bold mb-2">A few sparks if you're stuck</div>
          <div className="flex flex-wrap gap-2">
            {sparks.map((s) => (
              <button
                key={s}
                type="button"
                onClick={() => setValue("prompt", s, { shouldValidate: true })}
                className="px-3 py-1.5 rounded-full bg-white border border-line text-sm hover:border-ink"
              >
                {s}
              </button>
            ))}
          </div>
        </div>

        <div className="absolute bottom-3 left-3 right-3 z-40">
          <div className="bg-ink text-cream rounded-3xl shadow-pop p-3 flex items-center gap-3">
            <span className="text-sm opacity-70">Voting opens immediately</span>
            <span className="flex-1" />
            <Button type="submit" disabled={!isValid || suggest.isPending}>
              {suggest.isPending
                ? "Submitting…"
                : isAnonymous
                  ? "Submit anonymously"
                  : "Submit"}
            </Button>
          </div>
        </div>
      </form>
    </div>
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
      className={`p-4 rounded-2xl bg-white text-left transition ${
        active ? "border-2 border-coral ring-2 ring-coral/20" : "border border-line"
      }`}
    >
      <div className="text-2xl mb-1">{icon}</div>
      <div className="font-semibold">{label}</div>
      <div className="text-xs text-inkmuted">{hint}</div>
    </button>
  );
}
