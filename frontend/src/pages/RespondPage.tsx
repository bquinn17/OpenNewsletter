import { useEffect, useRef, useState } from "react";
import { Link, Navigate, useNavigate, useParams } from "react-router-dom";
import { ChevronLeft, ChevronRight } from "lucide-react";
import clsx from "clsx";
import {
  useConfig,
  useCycleCadence,
  useMembership,
  useNewsletter,
  useUploads,
} from "../api/queries";
import { PageHeader } from "../components/layout/PageHeader";
import { PageErrorState } from "../components/layout/PageErrorState";
import { Pill } from "../components/ui/Pill";
import { Button } from "../components/ui/Button";
import { Spinner } from "../components/ui/Spinner";
import { AllInOverlay } from "../components/ui/AllInOverlay";
import { QuestionPrompt } from "../components/newsletter/QuestionPrompt";
import { ResponseEditor } from "../components/responses/ResponseEditor";
import {
  AUTO_PUBLISH_BADGE,
  DraftStatusBadge,
  SaveStateDot,
} from "../components/responses/DraftStatusBadge";
import { PublishButton } from "../components/responses/PublishButton";
import { useResponseEditor } from "../components/responses/useResponseEditor";
import { useHydratedImages } from "../components/responses/useHydratedImages";
import { usePollResponse } from "../components/responses/usePollResponse";
import { PollPicker } from "../components/responses/PollPicker";
import { MarkdownBody } from "../utils/markdown";
import { useToasts } from "../state/toast";
import { useCelebration } from "../state/celebration";
import { countdownTo, editionLabel, formatInZone, shortCountdown } from "../utils/dates";
import type { components } from "../types/api";

type S = components["schemas"];

export function RespondPage() {
  const { groupId = "", cycleId = "", questionId = "" } = useParams();
  const { data, isLoading, isError, error, refetch } = useNewsletter(groupId, cycleId);
  const membership = useMembership(groupId);
  const timezone = membership?.timezone ?? "UTC";
  const cadence = useCycleCadence();

  if (isLoading) return <LoadingShell groupId={groupId} cycleId={cycleId} />;

  if (isError) {
    return (
      <PageErrorState
        title="Question"
        back={`/g/${groupId}/n/${cycleId}`}
        message={error instanceof Error ? error.message : "Couldn't load this question."}
        onRetry={() => refetch()}
        className="pb-40"
      />
    );
  }

  if (!data) return null;
  if (data.status !== "open") return <Navigate to={`/g/${groupId}/n/${cycleId}`} replace />;

  // Display order is the edition's question order (`docs/design/frontend.md` §7.3): "Question
  // 2 of 5" and the previous/next arrows walk this order, not the array's raw order.
  const ordered = [...data.questions].sort((a, b) => a.displayOrder - b.displayOrder);
  const index = ordered.findIndex((q) => q.questionId === questionId);
  if (index === -1) return <NotFoundState groupId={groupId} cycleId={cycleId} />;

  const editionLabelText = cadence
    ? editionLabel(data, timezone, cadence.frequency).short
    : undefined;

  return (
    <QuestionBody
      // A fresh key per question forces a full remount when the arrows navigate, so the old
      // question's `useResponseEditor` unmounts (flushing its pending autosave, the same as
      // leaving the page) before the new one mounts fresh.
      key={questionId}
      question={ordered[index]!}
      questions={ordered}
      questionNumber={index + 1}
      questionCount={ordered.length}
      prevQuestionId={index > 0 ? ordered[index - 1]!.questionId : null}
      nextQuestionId={index < ordered.length - 1 ? ordered[index + 1]!.questionId : null}
      groupId={groupId}
      cycleId={cycleId}
      timezone={timezone}
      responseCloseAt={data.responseCloseAt}
      editionLabelText={editionLabelText}
    />
  );
}

function LoadingShell({ groupId, cycleId }: { groupId: string; cycleId: string }) {
  return (
    <div className="pb-40">
      <PageHeader title="Loading…" back={`/g/${groupId}/n/${cycleId}`} />
      <div className="space-y-3 px-5 pt-6">
        <div className="skeleton h-32 rounded-3xl border border-line" />
      </div>
    </div>
  );
}

function NotFoundState({ groupId, cycleId }: { groupId: string; cycleId: string }) {
  return (
    <div className="pb-40">
      <PageHeader title="Question" eyebrow="Not found" back={`/g/${groupId}/n/${cycleId}`} />
      <div className="px-5 pt-8 text-center">
        <div className="mb-3 text-5xl">🔍</div>
        <p className="font-semibold text-ink">This question doesn&apos;t exist in this edition.</p>
      </div>
    </div>
  );
}

function QuestionBody({
  question,
  questions,
  questionNumber,
  questionCount,
  prevQuestionId,
  nextQuestionId,
  groupId,
  cycleId,
  timezone,
  responseCloseAt,
  editionLabelText,
}: {
  question: S["OpenQuestionResponse"];
  /** Every question in the edition, in display order — used to tell whether a fresh publish answers the last one. */
  questions: S["OpenQuestionResponse"][];
  questionNumber: number;
  questionCount: number;
  prevQuestionId: string | null;
  nextQuestionId: string | null;
  groupId: string;
  cycleId: string;
  timezone: string;
  responseCloseAt: string;
  editionLabelText: string | undefined;
}) {
  const navigate = useNavigate();
  const pushToast = useToasts((s) => s.push);
  const celebrate = useCelebration((s) => s.celebrate);
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
  const [justPublished, setJustPublished] = useState(false);
  const [showAllIn, setShowAllIn] = useState(false);
  const prevStatusRef = useRef<S["ResponseStatus"] | undefined>(question.myResponse?.status);

  // Whether answering `question` just now would leave every other question in the edition
  // published too — the trigger for the full-screen "all in" overlay and a bigger confetti burst.
  function isLastUnanswered(): boolean {
    return questions.every(
      (q) => q.questionId === question.questionId || q.myResponse?.status === "published",
    );
  }

  // Shared by the text and poll publish paths below: the UI reaction to a fresh publish (the
  // toast itself differs — the poll hook already pushes its own). `celebrate({ big })` fires the
  // bigger burst, and the full-screen overlay opens too, exactly when this was the last
  // unanswered question in the edition (`docs/design/frontend.md` §7.3, §18).
  function onFreshPublish() {
    setShowEditor(false);
    setJustPublished(true);
    const allIn = isLastUnanswered();
    celebrate({ big: allIn });
    if (allIn) setShowAllIn(true);
  }

  useEffect(() => {
    const newStatus = editor.lastResponse?.status;
    if (newStatus && newStatus !== prevStatusRef.current) {
      if (newStatus === "published") {
        pushToast("Published — friends can see this when the edition publishes.", "success");
        onFreshPublish();
      }
      prevStatusRef.current = newStatus;
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- `pushToast`/`celebrate` are stable zustand setters.
  }, [editor.lastResponse]);

  // Editing again after a fresh publish drops the button back to its normal "Save changes" state.
  useEffect(() => {
    if (justPublished && (editor.status.kind === "dirty" || editor.status.kind === "saving")) {
      setJustPublished(false);
    }
  }, [justPublished, editor.status.kind]);

  // The read-only view follows the current draft, including images added on this visit. Their
  // rows are the uploader's own `uploadQuery` entries, already cached.
  const viewImages = useUploads(groupId, cycleId, editor.imageMediaIds);

  const isPublished = (editor.lastResponse?.status ?? question.myResponse?.status) === "published";
  const canPublishText = editor.body.trim().length > 0 || editor.imageMediaIds.length > 0;
  const textIsUnpublishedDraft = !isPublished && canPublishText;

  const poll = usePollResponse({
    groupId,
    cycleId,
    questionId: question.questionId,
    initialResponse: question.myResponse,
  });
  // `usePollResponse` already pushes its own "Published —…" toast; this only drives the
  // celebration + overlay, the same as the text path's effect above.
  const prevPollPublishedRef = useRef(poll.isPublished);
  useEffect(() => {
    if (poll.isPublished && !prevPollPublishedRef.current) onFreshPublish();
    prevPollPublishedRef.current = poll.isPublished;
    // eslint-disable-next-line react-hooks/exhaustive-deps -- `onFreshPublish` closes over fresh state each render; only the transition matters.
  }, [poll.isPublished]);
  useEffect(() => {
    if (justPublished && poll.saving) setJustPublished(false);
  }, [justPublished, poll.saving]);

  // `docs/design/frontend.md` §7.3 (D2): a non-empty draft auto-publishes when responses close.
  const showAutoPublishNote = isText
    ? textIsUnpublishedDraft && !editor.readOnly
    : poll.isUnpublishedDraft && !poll.closed;

  const closeCountdown = countdownTo(responseCloseAt);
  const pulseCountdown = !closeCountdown.expired && closeCountdown.days === 0;

  async function handlePublish() {
    if (isText) {
      await editor.publish();
    } else {
      await poll.publish();
    }
  }

  return (
    <div className="pb-40">
      <PageHeader
        eyebrow={[editionLabelText, question.kind === "poll" ? "Poll" : null]
          .filter(Boolean)
          .join(" · ")}
        title={`Question ${questionNumber} of ${questionCount}`}
        back={`/g/${groupId}/n/${cycleId}`}
        rightSlot={
          <Pill tone="coral" className={clsx(pulseCountdown && "motion-safe:animate-pulse-soft")}>
            {shortCountdown(responseCloseAt)}
          </Pill>
        }
        phase="open"
      />

      <div className="flex items-center justify-between px-5 pt-3">
        {prevQuestionId ? (
          <Link
            to={`/g/${groupId}/n/${cycleId}/respond/${prevQuestionId}`}
            aria-label="Previous question"
            className="grid h-9 w-9 place-items-center rounded-full border border-line bg-white transition hover:border-ink motion-safe:active:scale-[.97]"
          >
            <ChevronLeft aria-hidden className="h-4 w-4" />
          </Link>
        ) : (
          <span aria-hidden="true" className="h-9 w-9" />
        )}
        {nextQuestionId ? (
          <Link
            to={`/g/${groupId}/n/${cycleId}/respond/${nextQuestionId}`}
            aria-label="Next question"
            className="grid h-9 w-9 place-items-center rounded-full border border-line bg-white transition hover:border-ink motion-safe:active:scale-[.97]"
          >
            <ChevronRight aria-hidden className="h-4 w-4" />
          </Link>
        ) : (
          <span aria-hidden="true" className="h-9 w-9" />
        )}
      </div>

      <div className="px-5 pt-3">
        <div className="rounded-3xl border border-coral/40 bg-coral/10 p-3 text-sm text-coral-ink">
          <p>Closes {formatInZone(responseCloseAt, timezone)}</p>
          {showAutoPublishNote && (
            <p role="note" className="mt-1 text-ink">
              Drafts are published automatically when responses close on{" "}
              {formatInZone(responseCloseAt, timezone)}.
              {/* A poll pick can be changed but not cleared (`pollOptionId` is required). */}
              {isText && " Clear your answer if you don't want it included."}
            </p>
          )}
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
              <MarkdownBody body={editor.body} images={viewImages} className="mt-1" />
              <button
                type="button"
                onClick={() => setShowEditor(true)}
                className="mt-3 text-sm font-semibold text-grape-ink"
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
                onDetachImage={editor.detachImage}
              />
            )}
          </>
        )
      ) : (
        <PollPicker
          question={question}
          selected={poll.pollOptionId}
          disabled={poll.saving}
          onSelect={poll.select}
        />
      )}

      {(isText ? editor.readOnly : poll.closed) && (
        <div className="mt-4 px-5">
          <div className="rounded-3xl border border-line bg-white p-4 text-sm text-inkmuted">
            This edition has closed — your last saved version stands.
          </div>
        </div>
      )}

      <div className="fixed inset-x-0 bottom-0 z-40">
        <div className="mx-auto w-full px-3 pb-[max(0.75rem,env(safe-area-inset-bottom))] md:max-w-3xl lg:max-w-2xl">
          <div className="rounded-3xl bg-ink p-3 text-cream shadow-pop">
            <div className="px-1 pb-2">
              {isText ? (
                <DraftStatusBadge
                  status={editor.status}
                  unpublishedDraft={textIsUnpublishedDraft}
                />
              ) : poll.saving ? (
                <SaveStateDot tone="grape" label="Saving…" pulsing />
              ) : (poll.response || question.myResponse) && poll.isUnpublishedDraft ? (
                <SaveStateDot tone="mint" label={AUTO_PUBLISH_BADGE} justSaved />
              ) : poll.response || question.myResponse ? (
                <SaveStateDot tone="mint" label="Saved" justSaved />
              ) : (
                <SaveStateDot tone="sun" label="Pick one" />
              )}
            </div>
            <div className="flex items-center gap-3">
              <Button variant="soft" onClick={() => navigate(`/g/${groupId}/n/${cycleId}`)}>
                Done
              </Button>
              <span className="flex-1" />
              {isText && !editor.readOnly && (
                <PublishButton
                  isPublished={isPublished}
                  disabled={!canPublishText || uploading}
                  onPublish={() => void handlePublish()}
                  justPublished={justPublished}
                />
              )}
              {!isText && !poll.closed && (
                <PublishButton
                  isPublished={poll.isPublished}
                  disabled={!poll.pollOptionId || poll.saving}
                  onPublish={() => void handlePublish()}
                  justPublished={justPublished}
                />
              )}
            </div>
          </div>
        </div>
      </div>

      <AllInOverlay
        open={showAllIn}
        editionLabel={editionLabelText}
        onDismiss={() => setShowAllIn(false)}
      />
    </div>
  );
}
