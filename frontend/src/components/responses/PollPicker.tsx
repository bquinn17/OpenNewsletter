import type { components } from "../../types/api";

type S = components["schemas"];

type Props = {
  question: S["OpenQuestionResponse"];
  selected: string | null;
  disabled: boolean;
  onSelect: (optionId: string) => void;
};

export function PollPicker({ question, selected, disabled, onSelect }: Props) {
  if (!question.pollOptions) return null;
  return (
    <div className="mt-4 px-5">
      <div className="space-y-2 rounded-3xl border border-line bg-white p-5 shadow-soft">
        {question.pollOptions.map((opt) => {
          const mine = selected === opt.optionId;
          return (
            <button
              key={opt.optionId}
              type="button"
              disabled={disabled}
              onClick={() => onSelect(opt.optionId)}
              className={`flex w-full items-center gap-3 rounded-2xl border px-4 py-3 text-left transition disabled:opacity-60 ${
                mine
                  ? "border-grape bg-grape/5 ring-2 ring-grape/30"
                  : "border-line hover:border-ink"
              }`}
            >
              <span
                className={`grid h-5 w-5 place-items-center rounded-full border-2 ${
                  mine ? "border-grape" : "border-line"
                }`}
              >
                {mine && <span className="h-2.5 w-2.5 rounded-full bg-grape" />}
              </span>
              <span className="font-semibold">{opt.label}</span>
            </button>
          );
        })}
      </div>
    </div>
  );
}
