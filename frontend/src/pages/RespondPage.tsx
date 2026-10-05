import { useEffect, useRef, useState } from "react";
import { Navigate, useNavigate, useParams } from "react-router-dom";
import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { api, ApiError } from "../api/client";
import { useConfig, useMembership, useNewsletter } from "../api/queries";
import { PageHeader } from "../components/layout/PageHeader";
import { Pill } from "../components/ui/Pill";
import { Button } from "../components/ui/Button";
import { Spinner } from "../components/ui/Spinner";
import { QuestionPrompt } from "../components/newsletter/QuestionPrompt";
import { ResponseEditor } from "../components/responses/ResponseEditor";
import { DraftStatusBadge } from "../components/responses/DraftStatusBadge";
import { PublishButton } from "../components/responses/PublishButton";
import { useResponseEditor } from "../components/responses/useResponseEditor";
import { MarkdownBody } from "../utils/markdown";
import { useToasts } from "../state/toast";
import { formatInZone, shortCountdown } from "../utils/dates";
import type { components } from "../types/api";

type S = components["schemas"];

export function RespondPage() {
  const { groupId = "", cycleId = "", questionId = "" } = useParams();
  const { data, isLoading, isError, error, refetch } = useNewsletter(groupId, cycleId);
  const membership = useMembership(groupId);
  const timezone = membership?.timezone ?? "UTC";

  if (isLoading) return <LoadingShell groupId={groupId} cycleId={cycleId} />;

  if (isError) {
    return (
      <ErrorState
        groupId={groupId}
        cycleId={cycleId}
        message={error instanceof Error ? error.message : "Couldn't load this question."}
        onRetry={() => refetch()}
      />
    );
  }

  if (!data) return null;
  if (data.status !== "open") return <Navigate to={`/g/${groupId}/n/${cycleId}`} replace />;

  const question = data.questions.find((q) => q.questionId === questionId);
  if (!question) return <NotFoundState groupId={groupId} cycleId={cycleId} />;

  return (
    <QuestionBody
      question={question}
      groupId={groupId}
      cycleId={cycleId}
      timezone={timezone}
      responseCloseAt={data.responseCloseAt}
    />
  );
}

function LoadingShell({ groupId, cycleId }: { groupId: string; cycleId: string }) {
  return (
    <div className="bg-cream pb-32">
      <PageHeader title="Loading…" back={`/g/${groupId}/n/${cycleId}`} />
      <div className="space-y-3 px-5 pt-6">
        <div className="h-32 animate-pulse rounded-3xl border border-line bg-white" />
      </div>
    </div>
  );
}

function ErrorState({
  groupId,
  cycleId,
  message,
  onRetry,
}: {
  groupId: string;
  cycleId: string;
  message: string;
  onRetry: () => void;
}) {
  return (
    <div className="bg-cream pb-32">
      <PageHeader title="Question" eyebrow="Couldn't load" back={`/g/${groupId}/n/${cycleId}`} />
      <div className="px-5 pt-8 text-center">
        <div className="mb-3 text-5xl">😬</div>
        <p className="font-semibold text-ink">{message}</p>
        <button
          onClick={onRetry}
          className="mt-5 rounded-full bg-ink px-5 py-3 font-semibold text-cream"
        >
          Try again
        </button>
      </div>
    </div>
  );
}

function NotFoundState({ groupId, cycleId }: { groupId: string; cycleId: string }) {
  return (
    <div className="bg-cream pb-32">
      <PageHeader title="Question" eyebrow="Not found" back={`/g/${groupId}/n/${cycleId}`} />
      <div className="px-5 pt-8 text-center">
        <div className="mb-3 text-5xl">🔍</div>
        <p className="font-semibold text-ink">This question doesn&apos;t exist in this edition.</p>
      </div>
    </div>
  );
}

/**
 * Loads the `ImageMediaResponse` rows for the draft's images as they were when
 * the page opened. `ImageUploader` and `ResponseEditor` read `initialImages`
 * once, on mount, so the editor must not render until this has settled — an
 * editor mounted with no images would report `imageMediaIds: []` and autosave
 * the draft's images away. An image that can't be fetched (e.g. deleted) is
 * dropped rather than blocking the editor.
 */
function useHydratedImages(
  groupId: string,
  cycleId: string,
  imageMediaIds: string[],
): { images: S["ImageMediaResponse"][]; ready: boolean } {
  const [ids] = useState(imageMediaIds);
  const { data, isSuccess } = useQuery(imagesQuery(groupId, cycleId, ids));
  return { images: data ?? [], ready: ids.length === 0 || isSuccess };
}

function imagesQuery(groupId: string, cycleId: string, ids: string[]) {
  return {
    queryKey: ["myResponseImages", groupId, cycleId, ids.join(",")],
    queryFn: async () => {
      const results = await Promise.allSettled(
        ids.map((id) => api.media.getUpload(id, groupId, cycleId)),
      );
      return results.flatMap((r) => (r.status === "fulfilled" ? [r.value] : []));
    },
    enabled: ids.length > 0,
    staleTime: Infinity,
  };
}

function QuestionBody({
  question,
  groupId,
  cycleId,
  timezone,
  responseCloseAt,
}: {
  question: S["OpenQuestionResponse"];
  groupId: string;
  cycleId: string;
  timezone: string;
  responseCloseAt: string;
}) {
  const navigate = useNavigate();
  const pushToast = useToasts((s) => s.push);
  const { data: config } = useConfig();
  const isText = question.kind === "text";

  const { images: hydratedImages, ready: imagesReady } = useHydratedImages(
    groupId,
    cycleId,
    isText ? (question.myResponse?.imageMediaIds ?? []) : [],
  );

  const editor = useResponseEditor({
    groupId,
    cycleId,
    questionId: question.questionId,
    userId: config?.userId ?? undefined,
    initial: question.myResponse,
    enabled: isText,
  });

  const [showEditor, setShowEditor] = useState(() => question.myResponse?.status !== "published");
  const [uploading, setUploading] = useState(false);
  const prevStatusRef = useRef<S["ResponseStatus"] | undefined>(question.myResponse?.status);

  useEffect(() => {
    const newStatus = editor.lastResponse?.status;
    if (newStatus && newStatus !== prevStatusRef.current) {
      if (newStatus === "published") {
        pushToast("Published — friends can see this when the edition publishes.", "success");
        setShowEditor(false);
      }
      prevStatusRef.current = newStatus;
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- `pushToast` is a stable zustand setter.
  }, [editor.lastResponse]);

  // The read-only view follows the current draft, including images added on this visit.
  const { data: viewImages = hydratedImages } = useQuery({
    ...imagesQuery(groupId, cycleId, editor.imageMediaIds),
    placeholderData: keepPreviousData,
  });

  const isPublished = (editor.lastResponse?.status ?? question.myResponse?.status) === "published";
  const canPublishText = editor.body.trim().length > 0 || editor.imageMediaIds.length > 0;

  const [pollOptionId, setPollOptionId] = useState<string | null>(
    question.myResponse?.pollOptionId ?? null,
  );
  const [pollSaving, setPollSaving] = useState(false);
  const [pollResponse, setPollResponse] = useState<S["ResponseDto"] | null>(null);
  const [pollClosed, setPollClosed] = useState(false);
  const pollIsPublished = (pollResponse?.status ?? question.myResponse?.status) === "published";

  async function savePoll(publish: boolean, optionId: string) {
    setPollSaving(true);
    try {
      const response = await api.saveMyResponse(groupId, cycleId, question.questionId, {
        kind: "poll",
        pollOptionId: optionId,
        publish,
      });
      setPollResponse(response);
      if (response.status === "published" && prevStatusRef.current !== "published") {
        pushToast("Published — friends can see this when the edition publishes.", "success");
      }
      prevStatusRef.current = response.status;
    } catch (err) {
      if (err instanceof ApiError && err.status === 409) {
        setPollClosed(true);
      }
      const message =
        err instanceof ApiError && err.status === 409
          ? "This edition has closed — your last saved version stands."
          : "Couldn't save your vote — try again.";
      pushToast(message, "error");
    } finally {
      setPollSaving(false);
    }
  }

  function handlePollSelect(optionId: string) {
    setPollOptionId(optionId);
    // Before the first publish, a pick is just local state — it's only
    // persisted when Publish is pressed. Once published, every change is
    // sticky and saves immediately (`03-api-contract.md` §7.3).
    if (pollIsPublished) void savePoll(false, optionId);
  }

  async function handlePublish() {
    if (isText) {
      await editor.publish();
    } else if (pollOptionId) {
      await savePoll(true, pollOptionId);
    }
  }

  return (
    <div className="bg-cream pb-32">
      <PageHeader
        eyebrow={`Question · ${question.kind === "poll" ? "poll" : "question"}`}
        title={question.prompt}
        back={`/g/${groupId}/n/${cycleId}`}
        rightSlot={<Pill tone="coral">{shortCountdown(responseCloseAt)}</Pill>}
      />

      <div className="px-5 pt-5">
        <div className="rounded-3xl border border-coral/40 bg-coral/10 p-3 text-sm text-coral">
          Closes {formatInZone(responseCloseAt, timezone)}
        </div>
      </div>

      <div className="px-5 pt-5">
        <div className="rounded-3xl border border-line bg-white p-5 shadow-soft">
          <QuestionPrompt
            prompt={question.prompt}
            kind={question.kind}
            askedBy={question.askedBy}
            isAnonymous={question.isAnonymous}
          />
        </div>
      </div>

      {isText ? (
        isPublished && !showEditor ? (
          <div className="mt-4 px-5">
            <div className="rounded-3xl border border-line bg-white p-5 shadow-soft">
              <MarkdownBody
                body={editor.body}
                images={viewImages}
                groupId={groupId}
                className="mt-1"
              />
              <button
                type="button"
                onClick={() => setShowEditor(true)}
                className="mt-3 text-sm font-semibold text-grape"
              >
                Edit
              </button>
            </div>
          </div>
        ) : (
          <>
            {isPublished && (
              <div className="mt-4 px-5 text-xs text-inkmuted">
                Changes save automatically and update your published answer.
              </div>
            )}
            {!imagesReady ? (
              <div className="flex justify-center py-10">
                <Spinner />
              </div>
            ) : (
              <ResponseEditor
                groupId={groupId}
                cycleId={cycleId}
                questionId={question.questionId}
                body={editor.body}
                onBodyChange={editor.setBody}
                onBlur={editor.onBlur}
                onImageMediaIdsChange={editor.setImageMediaIds}
                onUploadingChange={setUploading}
                initialImages={hydratedImages}
                readOnly={editor.readOnly}
              />
            )}
          </>
        )
      ) : (
        <PollPicker
          question={question}
          selected={pollOptionId}
          disabled={pollSaving}
          onSelect={handlePollSelect}
        />
      )}

      {(isText ? editor.readOnly : pollClosed) && (
        <div className="mt-4 px-5">
          <div className="rounded-3xl border border-line bg-white p-4 text-sm text-inkmuted">
            This edition has closed — your last saved version stands.
          </div>
        </div>
      )}

      <div className="fixed bottom-3 left-3 right-3 z-40">
        <div className="flex items-center gap-3 rounded-3xl bg-ink p-3 text-cream shadow-pop">
          <div className="text-xs leading-tight">
            {isText ? (
              <DraftStatusBadge status={editor.status} />
            ) : pollSaving ? (
              <Pill tone="grape">● Saving…</Pill>
            ) : pollResponse || question.myResponse ? (
              <Pill tone="mint">Saved</Pill>
            ) : (
              <Pill tone="sun">Pick one, then publish</Pill>
            )}
          </div>
          <span className="flex-1" />
          <Button variant="soft" onClick={() => navigate(`/g/${groupId}/n/${cycleId}`)}>
            Save &amp; exit
          </Button>
          {isText && !editor.readOnly && (
            <PublishButton
              isPublished={isPublished}
              disabled={!canPublishText || uploading}
              onPublish={() => void handlePublish()}
            />
          )}
          {!isText && !pollClosed && (
            <PublishButton
              isPublished={pollIsPublished}
              disabled={!pollOptionId || pollSaving}
              onPublish={() => void handlePublish()}
            />
          )}
        </div>
      </div>
    </div>
  );
}

function PollPicker({
  question,
  selected,
  disabled,
  onSelect,
}: {
  question: S["OpenQuestionResponse"];
  selected: string | null;
  disabled: boolean;
  onSelect: (optionId: string) => void;
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
