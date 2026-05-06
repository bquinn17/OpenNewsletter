import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { useNewsletter, useSaveResponse } from "../api/queries";
import { useToasts } from "../state/toast";
import { PageHeader } from "../components/layout/PageHeader";
import { Button } from "../components/ui/Button";
import { Pill } from "../components/ui/Pill";
import { formatRelative } from "../utils/dates";
import type { LockedQuestion, MyResponse } from "../api/types";

const AUTOSAVE_MS = 1500;

export function RespondPage() {
  const { groupId = "", cycleId = "", questionId = "" } = useParams();
  const navigate = useNavigate();
  const { data, isLoading } = useNewsletter(groupId, cycleId);
  const save = useSaveResponse(groupId, cycleId);
  const pushToast = useToasts((s) => s.push);

  const question: LockedQuestion | undefined = useMemo(() => {
    if (!data) return undefined;
    if (data.status === "open") return data.questions.find((q) => q.questionId === questionId);
    return undefined;
  }, [data, questionId]);

  const existing: MyResponse | undefined = useMemo(() => {
    if (!data || data.status !== "open") return undefined;
    return data.myResponses.find((r) => r.questionId === questionId);
  }, [data, questionId]);

  const [body, setBody] = useState("");
  const [pollOptionId, setPollOptionId] = useState<string | null>(null);
  const [imageIds, setImageIds] = useState<string[]>([]);
  const [savedAt, setSavedAt] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const initialized = useRef(false);

  useEffect(() => {
    if (!existing || initialized.current) return;
    setBody(existing.body ?? "");
    setPollOptionId(existing.pollOptionId ?? null);
    setImageIds(existing.imageMediaIds ?? []);
    setSavedAt(existing.updatedAt ?? null);
    initialized.current = true;
  }, [existing]);

  // Debounced autosave
  useEffect(() => {
    if (!question || !initialized.current) return;
    const id = setTimeout(() => {
      void doSave(false);
    }, AUTOSAVE_MS);
    return () => clearTimeout(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [body, pollOptionId, imageIds]);

  async function doSave(publish: boolean): Promise<boolean> {
    if (!question) return false;
    setSaving(true);
    try {
      const result = await save.mutateAsync({
        questionId: question.questionId,
        body:
          question.kind === "text"
            ? { kind: "text", body, imageMediaIds: imageIds, publish }
            : { kind: "poll", pollOptionId: pollOptionId ?? "", publish },
      });
      setSavedAt(result.updatedAt);
      if (publish) pushToast("Published — friends can see this when the edition publishes.", "success");
      return true;
    } catch (e) {
      pushToast((e as Error).message ?? "Save failed", "error");
      return false;
    } finally {
      setSaving(false);
    }
  }

  if (isLoading || !data) return null;
  if (data.status !== "open" || !question) {
    return (
      <div className="bg-cream pb-32">
        <PageHeader title="Not open for responses" back="/" />
        <div className="px-5 pt-6 text-inkmuted">This question isn't accepting responses right now.</div>
      </div>
    );
  }

  const wordCount = body.trim() ? body.trim().split(/\s+/).length : 0;
  const isPublished = existing?.status === "published";

  return (
    <div className="bg-cream pb-32">
      <PageHeader
        eyebrow={`Question · ${data.monthLabel} ${data.yearLabel}`}
        title={question.prompt}
        back={`/g/${groupId}/n/${cycleId}`}
        rightSlot={
          <Pill tone={saving ? "grape" : "mint"}>● {saving ? "saving" : "saved"}</Pill>
        }
      />

      <div className="px-5 pt-5">
        <div className="bg-white rounded-3xl border border-line shadow-soft p-5">
          <div className="text-xs uppercase tracking-widest text-grape font-bold">Question</div>
          <h1 className="font-display text-2xl font-bold leading-tight mt-1">{question.prompt}</h1>
          {question.helperText && <p className="text-sm text-inkmuted mt-2">{question.helperText}</p>}
        </div>
      </div>

      {question.kind === "text" ? (
        <TextEditor body={body} onBodyChange={setBody} imageIds={imageIds} setImageIds={setImageIds} />
      ) : (
        <PollPicker
          question={question}
          selected={pollOptionId}
          onSelect={setPollOptionId}
        />
      )}

      <div className="px-5 mt-4">
        <div className="bg-grape/5 border border-grape/20 rounded-3xl p-4 text-sm text-grape flex gap-3">
          <span className="text-lg">💡</span>
          <div>
            <strong className="font-semibold">Drafts autosave every couple seconds.</strong> Your friends won't see this
            until the edition publishes, and you can edit anytime before then.
          </div>
        </div>
      </div>

      <div className="absolute bottom-3 left-3 right-3 z-40">
        <div className="bg-ink text-cream rounded-3xl shadow-pop p-3 flex items-center gap-3">
          <div className="text-xs leading-tight">
            <div className="font-semibold">
              {isPublished ? "Published" : "Draft"} · {wordCount} words
            </div>
            <div className="opacity-70">{savedAt ? `Last saved ${formatRelative(savedAt)}` : "Saving…"}</div>
          </div>
          <span className="flex-1" />
          <Button variant="soft" onClick={() => navigate(`/g/${groupId}/n/${cycleId}`)}>
            Save & exit
          </Button>
          <Button onClick={() => doSave(true)}>{isPublished ? "Save changes" : "Publish"}</Button>
        </div>
      </div>
    </div>
  );
}

function TextEditor({
  body,
  onBodyChange,
  imageIds,
  setImageIds,
}: {
  body: string;
  onBodyChange: (s: string) => void;
  imageIds: string[];
  setImageIds: (ids: string[]) => void;
}) {
  return (
    <div className="px-5 mt-4">
      <div className="bg-white rounded-3xl border border-line shadow-soft overflow-hidden">
        <div className="flex items-center gap-1 px-3 py-2 border-b border-line text-inkmuted text-sm">
          <button className="w-8 h-8 grid place-items-center rounded-lg hover:bg-cream font-bold">B</button>
          <button className="w-8 h-8 grid place-items-center rounded-lg hover:bg-cream italic">I</button>
          <button className="w-8 h-8 grid place-items-center rounded-lg hover:bg-cream">•</button>
          <button className="w-8 h-8 grid place-items-center rounded-lg hover:bg-cream">"</button>
          <span className="flex-1" />
          <button className="text-xs px-2 py-1 rounded-md hover:bg-cream font-medium">Preview</button>
        </div>
        <textarea
          value={body}
          onChange={(e) => onBodyChange(e.target.value)}
          className="w-full px-4 py-4 min-h-[200px] resize-none focus:outline-none text-[15px] leading-relaxed bg-transparent"
          placeholder="Tell us about it…"
        />
        <div className="border-t border-line p-3">
          <div className="grid grid-cols-3 gap-2">
            {imageIds.map((id) => (
              <div key={id} className="ph-img aspect-square rounded-2xl relative">
                <button
                  onClick={() => setImageIds(imageIds.filter((x) => x !== id))}
                  className="absolute top-1.5 right-1.5 w-5 h-5 rounded-full bg-ink/80 text-white text-[10px] grid place-items-center"
                  aria-label="Remove"
                >
                  ×
                </button>
              </div>
            ))}
            {imageIds.length < 10 && (
              <button
                onClick={() => setImageIds([...imageIds, `i_${Date.now()}`])}
                className="aspect-square rounded-2xl border-2 border-dashed border-line text-inkmuted text-sm hover:border-grape hover:text-grape transition flex flex-col items-center justify-center gap-1"
              >
                <span className="text-2xl">＋</span>
                <span>Add photo</span>
              </button>
            )}
          </div>
          <div className="text-xs text-inkmuted mt-2">{imageIds.length} of 10 photos</div>
        </div>
      </div>
    </div>
  );
}

function PollPicker({
  question,
  selected,
  onSelect,
}: {
  question: LockedQuestion;
  selected: string | null;
  onSelect: (id: string) => void;
}) {
  if (!question.pollOptions) return null;
  return (
    <div className="px-5 mt-4">
      <div className="bg-white rounded-3xl border border-line shadow-soft p-5 space-y-2">
        {question.pollOptions.map((opt) => {
          const mine = selected === opt.optionId;
          return (
            <button
              key={opt.optionId}
              onClick={() => onSelect(opt.optionId)}
              className={`w-full text-left rounded-2xl px-4 py-3 border transition flex items-center gap-3 ${
                mine ? "bg-grape/5 border-grape ring-2 ring-grape/30" : "border-line hover:border-ink"
              }`}
            >
              <span
                className={`w-5 h-5 rounded-full border-2 grid place-items-center ${
                  mine ? "border-grape" : "border-line"
                }`}
              >
                {mine && <span className="w-2.5 h-2.5 rounded-full bg-grape" />}
              </span>
              <span className="font-semibold">{opt.label}</span>
            </button>
          );
        })}
      </div>
    </div>
  );
}
