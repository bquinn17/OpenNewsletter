import { useEffect, useRef, useState } from "react";
import { Navigate, useNavigate, useParams } from "react-router-dom";
import { useConfig, useMembership, useNewsletter, useUploads } from "../api/queries";
import { PageHeader } from "../components/layout/PageHeader";
import { PageErrorState } from "../components/layout/PageErrorState";
import { Pill } from "../components/ui/Pill";
import { Button } from "../components/ui/Button";
import { Spinner } from "../components/ui/Spinner";
import { QuestionPrompt } from "../components/newsletter/QuestionPrompt";
import { ResponseEditor } from "../components/responses/ResponseEditor";
import { AUTO_PUBLISH_BADGE, DraftStatusBadge } from "../components/responses/DraftStatusBadge";
import { PublishButton } from "../components/responses/PublishButton";
import { useResponseEditor } from "../components/responses/useResponseEditor";
import { useHydratedImages } from "../components/responses/useHydratedImages";
import { usePollResponse } from "../components/responses/usePollResponse";
import { PollPicker } from "../components/responses/PollPicker";
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
      <PageErrorState
        title="Question"
        back={`/g/${groupId}/n/${cycleId}`}
        message={error instanceof Error ? error.message : "Couldn't load this question."}
        onRetry={() => refetch()}
        className="pb-32"
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
    <div className="pb-32">
      <PageHeader title="Loading…" back={`/g/${groupId}/n/${cycleId}`} />
      <div className="space-y-3 px-5 pt-6">
        <div className="h-32 animate-pulse rounded-3xl border border-line bg-white" />
      </div>
    </div>
  );
}

function NotFoundState({ groupId, cycleId }: { groupId: string; cycleId: string }) {
  return (
    <div className="pb-32">
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
  // `docs/design/frontend.md` §7.3 (D2): a non-empty draft auto-publishes when responses close.
  const showAutoPublishNote = isText
    ? textIsUnpublishedDraft && !editor.readOnly
    : poll.isUnpublishedDraft && !poll.closed;

  async function handlePublish() {
    if (isText) {
      await editor.publish();
    } else {
      await poll.publish();
    }
  }

  return (
    <div className="pb-32">
      <PageHeader
        eyebrow={`Question · ${question.kind === "poll" ? "poll" : "question"}`}
        title={question.prompt}
        back={`/g/${groupId}/n/${cycleId}`}
        rightSlot={<Pill tone="coral">{shortCountdown(responseCloseAt)}</Pill>}
        phase="open"
      />

      <div className="px-5 pt-5">
        <div className="rounded-3xl border border-coral/40 bg-coral/10 p-3 text-sm text-coral">
          Closes {formatInZone(responseCloseAt, timezone)}
        </div>
      </div>

      {showAutoPublishNote && (
        <div className="px-5 pt-3">
          <p
            role="note"
            className="rounded-3xl border border-sun/60 bg-sun/20 p-3 text-sm text-ink"
          >
            Drafts are published automatically when responses close on{" "}
            {formatInZone(responseCloseAt, timezone)}.
            {/* A poll pick can be changed but not cleared (`pollOptionId` is required). */}
            {isText && " Clear your answer if you don't want it included."}
          </p>
        </div>
      )}

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

      <div className="fixed bottom-3 left-3 right-3 z-40">
        <div className="flex items-center gap-3 rounded-3xl bg-ink p-3 text-cream shadow-pop">
          <div className="text-xs leading-tight">
            {isText ? (
              <DraftStatusBadge status={editor.status} unpublishedDraft={textIsUnpublishedDraft} />
            ) : poll.saving ? (
              <Pill tone="grape">● Saving…</Pill>
            ) : (poll.response || question.myResponse) && poll.isUnpublishedDraft ? (
              <Pill tone="mint">{AUTO_PUBLISH_BADGE}</Pill>
            ) : poll.response || question.myResponse ? (
              <Pill tone="mint">Saved</Pill>
            ) : (
              <Pill tone="sun">Pick one</Pill>
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
          {!isText && !poll.closed && (
            <PublishButton
              isPublished={poll.isPublished}
              disabled={!poll.pollOptionId || poll.saving}
              onPublish={() => void handlePublish()}
            />
          )}
        </div>
      </div>
    </div>
  );
}
