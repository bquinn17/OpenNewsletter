import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeSanitize from "rehype-sanitize";
import { mockApi as api } from "../mocks/api";
import { useNewsletter, useSaveResponse } from "../mocks/legacyQueries";
import { useToasts } from "../state/toast";
import { PageHeader } from "../components/layout/PageHeader";
import { Button } from "../components/ui/Button";
import { Pill } from "../components/ui/Pill";
import { formatRelative } from "../utils/dates";
import type { LockedQuestion, MyResponse } from "../mocks/types";

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
  const [imageCaptions, setImageCaptions] = useState<Record<string, string>>({});
  const [savedAt, setSavedAt] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const initialized = useRef(false);

  useEffect(() => {
    if (initialized.current) return;
    // Wait until the newsletter has loaded and the target question resolves —
    // only then do we know whether there's a pre-existing draft to hydrate.
    if (!question) return;
    if (existing) {
      setBody(existing.body ?? "");
      setPollOptionId(existing.pollOptionId ?? null);
      setImageIds(existing.imageMediaIds ?? []);
      setImageCaptions(existing.imageCaptions ?? {});
      setSavedAt(existing.updatedAt ?? null);
    }
    // Mark initialized either way so the autosave effect can start firing on
    // the user's first keystroke for brand-new responses.
    initialized.current = true;
  }, [question, existing]);

  // Debounced autosave
  useEffect(() => {
    if (!question || !initialized.current) return;
    const id = setTimeout(() => {
      void doSave(false);
    }, AUTOSAVE_MS);
    return () => clearTimeout(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [body, pollOptionId, imageIds, imageCaptions]);

  async function doSave(publish: boolean): Promise<boolean> {
    if (!question) return false;
    setSaving(true);
    try {
      // Drop captions for any image that's been removed.
      const cleanedCaptions: Record<string, string> = {};
      for (const id of imageIds) {
        const c = imageCaptions[id]?.trim();
        if (c) cleanedCaptions[id] = c;
      }
      const result = await save.mutateAsync({
        questionId: question.questionId,
        body:
          question.kind === "text"
            ? {
                kind: "text",
                body,
                imageMediaIds: imageIds,
                imageCaptions: cleanedCaptions,
                publish,
              }
            : { kind: "poll", pollOptionId: pollOptionId ?? "", publish },
      });
      setSavedAt(result.updatedAt);
      if (publish)
        pushToast("Published — friends can see this when the edition publishes.", "success");
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
        <div className="px-5 pt-6 text-inkmuted">
          This question isn&apos;t accepting responses right now.
        </div>
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
        rightSlot={<Pill tone={saving ? "grape" : "mint"}>● {saving ? "saving" : "saved"}</Pill>}
      />

      <div className="px-5 pt-5">
        <div className="rounded-3xl border border-line bg-white p-5 shadow-soft">
          <div className="text-xs font-bold uppercase tracking-widest text-grape">
            {question.askedBy
              ? `${question.askedBy.displayName} asked`
              : question.isAnonymous
                ? "Asked anonymously"
                : "Question"}
          </div>
          <h1 className="mt-1 font-display text-2xl font-bold leading-tight">{question.prompt}</h1>
          {question.helperText && (
            <p className="mt-2 text-sm text-inkmuted">{question.helperText}</p>
          )}
        </div>
      </div>

      {question.kind === "text" ? (
        <TextEditor
          groupId={groupId}
          cycleId={cycleId}
          questionId={question.questionId}
          body={body}
          onBodyChange={setBody}
          imageIds={imageIds}
          setImageIds={setImageIds}
          imageCaptions={imageCaptions}
          setImageCaptions={setImageCaptions}
        />
      ) : (
        <PollPicker question={question} selected={pollOptionId} onSelect={setPollOptionId} />
      )}

      <div className="mt-4 px-5">
        <div className="flex gap-3 rounded-3xl border border-grape/20 bg-grape/5 p-4 text-sm text-grape">
          <span className="text-lg">💡</span>
          <div>
            <strong className="font-semibold">Drafts autosave every couple seconds.</strong> Your
            friends won&apos;t see this until the edition publishes, and you can edit anytime before
            then.
          </div>
        </div>
      </div>

      <div className="absolute bottom-3 left-3 right-3 z-40">
        <div className="flex items-center gap-3 rounded-3xl bg-ink p-3 text-cream shadow-pop">
          <div className="text-xs leading-tight">
            <div className="font-semibold">
              {isPublished ? "Published" : "Draft"} · {wordCount} words
            </div>
            <div className="opacity-70">
              {savedAt ? `Last saved ${formatRelative(savedAt)}` : "Saving…"}
            </div>
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

interface PendingUpload {
  imageId: string;
  fileName: string;
  status: "uploading" | "processing" | "ready" | "failed";
  progress: number;
  dataUrl: string;
  error?: string | null;
}

function TextEditor({
  groupId,
  cycleId,
  questionId,
  body,
  onBodyChange,
  imageIds,
  setImageIds,
  imageCaptions,
  setImageCaptions,
}: {
  groupId: string;
  cycleId: string;
  questionId: string;
  body: string;
  onBodyChange: (s: string) => void;
  imageIds: string[];
  setImageIds: (ids: string[]) => void;
  imageCaptions: Record<string, string>;
  setImageCaptions: (next: Record<string, string>) => void;
}) {
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [preview, setPreview] = useState(false);
  const [pending, setPending] = useState<Record<string, PendingUpload>>({});
  const [previews, setPreviews] = useState<Record<string, string>>({});
  const pushToast = useToasts((s) => s.push);
  const pollers = useRef<Set<string>>(new Set());

  // Cancel any in-flight uploads on unmount.
  useEffect(() => {
    // The Set itself never changes identity, so capturing it here still sees
    // every ID added later.
    const active = pollers.current;
    return () => {
      for (const id of active) {
        void api.cancelImageUpload(id).catch(() => undefined);
      }
    };
  }, []);

  const totalCount = imageIds.length + Object.keys(pending).length;

  const readAsDataUrl = (f: File) =>
    new Promise<string>((resolve, reject) => {
      const r = new FileReader();
      r.onload = () => resolve(r.result as string);
      r.onerror = () => reject(r.error ?? new Error("read failed"));
      r.readAsDataURL(f);
    });

  async function pollUploadStatus(imageId: string) {
    pollers.current.add(imageId);
    const start = Date.now();
    while (pollers.current.has(imageId)) {
      try {
        const s = await api.getImageUploadStatus(imageId);
        setPending((prev) => {
          const cur = prev[imageId];
          if (!cur) return prev;
          return { ...prev, [imageId]: { ...cur, status: s.status, progress: s.progress } };
        });
        if (s.status === "ready") {
          pollers.current.delete(imageId);
          setPreviews((p) => ({ ...p, [imageId]: s.displayUrl ?? s.thumbUrl ?? "" }));
          setImageIds([...imageIds, imageId]);
          setPending((prev) => {
            const next = { ...prev };
            delete next[imageId];
            return next;
          });
          return;
        }
        if (s.status === "failed") {
          pollers.current.delete(imageId);
          setPending((prev) => {
            const cur = prev[imageId];
            if (!cur) return prev;
            return {
              ...prev,
              [imageId]: { ...cur, status: "failed", error: s.error ?? "Upload failed" },
            };
          });
          return;
        }
      } catch {
        pollers.current.delete(imageId);
        return;
      }
      // Bail out after 30s — something's wrong.
      if (Date.now() - start > 30_000) {
        pollers.current.delete(imageId);
        setPending((prev) => {
          const cur = prev[imageId];
          if (!cur) return prev;
          return { ...prev, [imageId]: { ...cur, status: "failed", error: "Timed out" } };
        });
        return;
      }
      await new Promise((r) => setTimeout(r, 250));
    }
  }

  async function handleFiles(files: FileList | null) {
    if (!files) return;
    const remaining = 10 - totalCount;
    const accepted = Array.from(files).slice(0, Math.max(0, remaining));
    if (files.length > accepted.length) {
      pushToast(`Only ${remaining} more photo${remaining === 1 ? "" : "s"} allowed.`, "error");
    }
    for (const file of accepted) {
      if (!file.type.startsWith("image/")) {
        pushToast(`${file.name} isn't an image.`, "error");
        continue;
      }
      if (file.size > 15 * 1024 * 1024) {
        pushToast(`${file.name} is over 15 MB.`, "error");
        continue;
      }
      try {
        const dataUrl = await readAsDataUrl(file);
        const { imageId } = await api.startImageUpload({
          groupId,
          cycleId,
          questionId,
          fileName: file.name,
          fileSize: file.size,
          mimeType: file.type,
          dataUrl,
        });
        setPending((prev) => ({
          ...prev,
          [imageId]: {
            imageId,
            fileName: file.name,
            status: "uploading",
            progress: 0,
            dataUrl,
          },
        }));
        void pollUploadStatus(imageId);
      } catch (e) {
        pushToast((e as Error).message ?? "Couldn't start upload.", "error");
      }
    }
    if (fileInputRef.current) fileInputRef.current.value = "";
  }

  function removeCommitted(id: string) {
    setImageIds(imageIds.filter((x) => x !== id));
    const next = { ...imageCaptions };
    delete next[id];
    setImageCaptions(next);
  }

  function cancelPending(id: string) {
    pollers.current.delete(id);
    void api.cancelImageUpload(id).catch(() => undefined);
    setPending((prev) => {
      const next = { ...prev };
      delete next[id];
      return next;
    });
  }

  function applyMarkdown(kind: "bold" | "italic" | "list" | "quote") {
    const ta = textareaRef.current;
    if (!ta) return;
    const start = ta.selectionStart;
    const end = ta.selectionEnd;
    const before = body.slice(0, start);
    const selected = body.slice(start, end);
    const after = body.slice(end);

    let next: string;
    let cursorStart: number;
    let cursorEnd: number;

    if (kind === "bold" || kind === "italic") {
      const wrap = kind === "bold" ? "**" : "_";
      // CommonMark requires the emphasis delimiters to be flush against
      // non-whitespace — `** foo **` does NOT render as bold. Pull any
      // leading/trailing whitespace out of the wrap so the rendered output
      // matches what the user expects.
      const raw = selected || (kind === "bold" ? "bold text" : "italic text");
      const leadMatch = raw.match(/^\s*/);
      const trailMatch = raw.match(/\s*$/);
      const lead = leadMatch ? leadMatch[0] : "";
      const trail = trailMatch ? trailMatch[0] : "";
      const inner =
        raw.slice(lead.length, raw.length - trail.length) ||
        raw.trim() ||
        (kind === "bold" ? "bold text" : "italic text");
      // If trimming consumed everything (selection was pure whitespace), fall
      // back to wrapping a placeholder and drop the would-be empty padding.
      const padLead = inner === raw.trim() && raw.trim() === "" ? "" : lead;
      const padTrail = inner === raw.trim() && raw.trim() === "" ? "" : trail;
      next = `${before}${padLead}${wrap}${inner}${wrap}${padTrail}${after}`;
      cursorStart = before.length + padLead.length + wrap.length;
      cursorEnd = cursorStart + inner.length;
    } else {
      const prefix = kind === "list" ? "- " : "> ";
      const target = selected || (kind === "list" ? "item" : "quote");
      const lines = target
        .split("\n")
        .map((l) => `${prefix}${l}`)
        .join("\n");
      // Ensure leading newline if not at start of a line
      const leading = before.length === 0 || before.endsWith("\n") ? "" : "\n";
      next = `${before}${leading}${lines}${after}`;
      cursorStart = before.length + leading.length + prefix.length;
      cursorEnd = cursorStart + target.length;
    }

    onBodyChange(next);
    requestAnimationFrame(() => {
      ta.focus();
      ta.setSelectionRange(cursorStart, cursorEnd);
    });
  }

  return (
    <div className="mt-4 px-5">
      <div className="overflow-hidden rounded-3xl border border-line bg-white shadow-soft">
        <div className="flex items-center gap-1 border-b border-line px-3 py-2 text-sm text-inkmuted">
          <button
            type="button"
            onClick={() => applyMarkdown("bold")}
            disabled={preview}
            className="grid h-8 w-8 place-items-center rounded-lg font-bold hover:bg-cream disabled:opacity-40"
            aria-label="Bold"
          >
            B
          </button>
          <button
            type="button"
            onClick={() => applyMarkdown("italic")}
            disabled={preview}
            className="grid h-8 w-8 place-items-center rounded-lg italic hover:bg-cream disabled:opacity-40"
            aria-label="Italic"
          >
            I
          </button>
          <button
            type="button"
            onClick={() => applyMarkdown("list")}
            disabled={preview}
            className="grid h-8 w-8 place-items-center rounded-lg hover:bg-cream disabled:opacity-40"
            aria-label="Bulleted list"
          >
            •
          </button>
          <button
            type="button"
            onClick={() => applyMarkdown("quote")}
            disabled={preview}
            className="grid h-8 w-8 place-items-center rounded-lg hover:bg-cream disabled:opacity-40"
            aria-label="Quote"
          >
            &quot;
          </button>
          <span className="flex-1" />
          <button
            type="button"
            onClick={() => setPreview((p) => !p)}
            className={`rounded-md px-2 py-1 text-xs font-medium ${
              preview ? "bg-ink text-cream" : "hover:bg-cream"
            }`}
          >
            {preview ? "Edit" : "Preview"}
          </button>
        </div>
        {preview ? (
          <div className="markdown-preview min-h-[200px] px-4 py-4 text-[15px] leading-relaxed">
            {body.trim() ? (
              <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeSanitize]}>
                {body}
              </ReactMarkdown>
            ) : (
              <p className="italic text-inkmuted">Nothing to preview yet.</p>
            )}
          </div>
        ) : (
          <textarea
            ref={textareaRef}
            value={body}
            onChange={(e) => onBodyChange(e.target.value)}
            className="min-h-[200px] w-full resize-none bg-transparent px-4 py-4 text-[15px] leading-relaxed focus:outline-none"
            placeholder="Tell us about it…"
          />
        )}
        <div className="border-t border-line p-3">
          <div className="space-y-3">
            {imageIds.map((id) => (
              <div key={id} className="flex items-start gap-3">
                <div className="ph-img relative h-20 w-20 flex-shrink-0 overflow-hidden rounded-2xl">
                  {previews[id] && (
                    <img src={previews[id]} alt="" className="h-full w-full object-cover" />
                  )}
                  <button
                    type="button"
                    onClick={() => removeCommitted(id)}
                    className="absolute right-1.5 top-1.5 grid h-5 w-5 place-items-center rounded-full bg-ink/80 text-[10px] text-white"
                    aria-label="Remove"
                  >
                    ×
                  </button>
                </div>
                <input
                  value={imageCaptions[id] ?? ""}
                  onChange={(e) => setImageCaptions({ ...imageCaptions, [id]: e.target.value })}
                  maxLength={140}
                  placeholder="Caption (optional)"
                  className="mt-1 flex-1 rounded-xl border border-line bg-cream px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-coral/30"
                />
              </div>
            ))}
            {Object.values(pending).map((p) => (
              <div key={p.imageId} className="flex items-start gap-3">
                <div className="relative h-20 w-20 flex-shrink-0 overflow-hidden rounded-2xl border border-line bg-cream">
                  <img src={p.dataUrl} alt="" className="h-full w-full object-cover opacity-70" />
                  {p.status !== "failed" && (
                    <div className="absolute inset-x-0 bottom-0 h-1.5 bg-black/30">
                      <div
                        className={`h-full ${p.status === "processing" ? "animate-pulse bg-grape" : "bg-coral"}`}
                        style={{ width: `${Math.max(5, Math.round(p.progress * 100))}%` }}
                      />
                    </div>
                  )}
                  <button
                    type="button"
                    onClick={() => cancelPending(p.imageId)}
                    className="absolute right-1.5 top-1.5 grid h-5 w-5 place-items-center rounded-full bg-ink/80 text-[10px] text-white"
                    aria-label={p.status === "failed" ? "Dismiss" : "Cancel upload"}
                  >
                    ×
                  </button>
                </div>
                <div className="mt-1 flex-1 text-xs">
                  <div className="truncate font-semibold text-inkmuted">{p.fileName}</div>
                  <div className="mt-0.5 text-inkmuted">
                    {p.status === "uploading" && `Uploading… ${Math.round(p.progress * 100)}%`}
                    {p.status === "processing" && "Processing image…"}
                    {p.status === "failed" && (
                      <span className="text-coral">
                        Upload failed{p.error ? ` — ${p.error}` : ""}
                      </span>
                    )}
                  </div>
                </div>
              </div>
            ))}
            {totalCount < 10 && (
              <>
                <input
                  ref={fileInputRef}
                  type="file"
                  accept="image/*"
                  multiple
                  className="hidden"
                  onChange={(e) => handleFiles(e.target.files)}
                />
                <button
                  type="button"
                  onClick={() => fileInputRef.current?.click()}
                  className="flex w-full items-center justify-center gap-2 rounded-2xl border-2 border-dashed border-line py-3 text-sm text-inkmuted transition hover:border-grape hover:text-grape"
                >
                  <span className="text-xl leading-none">＋</span>
                  <span>Add photo</span>
                </button>
              </>
            )}
          </div>
          <div className="mt-2 text-xs text-inkmuted">{totalCount} of 10 photos</div>
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
    <div className="mt-4 px-5">
      <div className="space-y-2 rounded-3xl border border-line bg-white p-5 shadow-soft">
        {question.pollOptions.map((opt) => {
          const mine = selected === opt.optionId;
          return (
            <button
              key={opt.optionId}
              onClick={() => onSelect(opt.optionId)}
              className={`flex w-full items-center gap-3 rounded-2xl border px-4 py-3 text-left transition ${
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
