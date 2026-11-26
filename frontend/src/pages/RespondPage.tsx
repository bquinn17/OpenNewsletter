import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { Navigate, useNavigate, useParams } from "react-router-dom";
import { queryKeys, useAudioUploads, useConfig, useNewsletter, useUploads } from "../api/queries";
import { PageHeader } from "../components/layout/PageHeader";
import { PageErrorState } from "../components/layout/PageErrorState";
import { Pill } from "../components/ui/Pill";
import { Button } from "../components/ui/Button";
import { Spinner } from "../components/ui/Spinner";
import { QuestionPrompt } from "../components/newsletter/QuestionPrompt";
import { ResponseEditor } from "../components/responses/ResponseEditor";
import { AUTO_PUBLISH_BADGE, DraftStatusBadge } from "../components/responses/DraftStatusBadge";
import { useResponseEditor } from "../components/responses/useResponseEditor";
import { useHydratedImages } from "../components/responses/useHydratedImages";
import { useHydratedAudio } from "../components/responses/useHydratedAudio";
import { VoiceClip } from "../components/newsletter/VoiceClip";
import { usePollResponse } from "../components/responses/usePollResponse";
import { PollPicker } from "../components/responses/PollPicker";
import { GiphyLookupProvider } from "../giphy/GiphyLookupProvider";
import { MarkdownBody, referencedGiphyIds } from "../utils/markdown";
import { shortCountdown } from "../utils/dates";
import { nextUnansweredQuestionId } from "../utils/responseProgress";
import type { components } from "../types/api";

type S = components["schemas"];

export function RespondPage() {
  const { groupId = "", cycleId = "", questionId = "" } = useParams();
  const { data, isLoading, isError, error, refetch } = useNewsletter(groupId, cycleId);

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
  responseCloseAt,
}: {
  question: S["OpenQuestionResponse"];
  groupId: string;
  cycleId: string;
  responseCloseAt: string;
}) {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { data: config } = useConfig();
  const isText = question.kind === "text";

  const { images: hydratedImages, ready: imagesReady } = useHydratedImages(
    groupId,
    cycleId,
    isText ? (question.myResponse?.imageMediaIds ?? []) : [],
  );
  const { clips: hydratedClips, ready: clipsReady } = useHydratedAudio(
    groupId,
    cycleId,
    isText ? (question.myResponse?.audioMediaIds ?? []) : [],
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

  // The read-only view follows the current draft, including images added on this visit. Their
  // rows are the uploader's own `uploadQuery` entries, already cached.
  const viewImages = useUploads(groupId, cycleId, editor.imageMediaIds);
  const viewClips = useAudioUploads(groupId, cycleId, editor.audioMediaIds);

  const isPublished = (editor.lastResponse?.status ?? question.myResponse?.status) === "published";
  const canPublishText =
    editor.body.trim().length > 0 ||
    editor.imageMediaIds.length > 0 ||
    editor.audioMediaIds.length > 0;
  const textIsUnpublishedDraft = !isPublished && canPublishText;

  const poll = usePollResponse({
    groupId,
    cycleId,
    questionId: question.questionId,
    initialResponse: question.myResponse,
  });
  // "Done" (`docs/design/frontend.md` §7.3): flushes any pending edit, then moves on to the
  // next unanswered question of this edition, wrapping around, or back to the edition page
  // once every question has a non-empty draft or answer. The frontend no longer publishes
  // manually — a non-empty answer auto-publishes at close.
  async function handleDone() {
    if (isText) {
      await editor.flush();
    } else {
      await poll.settle();
    }

    const cached = queryClient.getQueryData<S["NewsletterDetailResponse"]>(
      queryKeys.newsletter(groupId, cycleId),
    );
    const questions = cached && cached.status === "open" ? cached.questions : [question];
    const nextQuestionId = nextUnansweredQuestionId(questions, question.questionId);
    navigate(
      nextQuestionId
        ? `/g/${groupId}/n/${cycleId}/respond/${nextQuestionId}`
        : `/g/${groupId}/n/${cycleId}`,
    );
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
              <GiphyLookupProvider ids={[...referencedGiphyIds(editor.body)]}>
                <MarkdownBody
                  body={editor.body}
                  images={viewImages}
                  className="mt-1"
                  maxGiphy={5}
                  giphyVariant="answer"
                />
              </GiphyLookupProvider>
              {viewClips.map(
                (clip) =>
                  clip.url && (
                    <VoiceClip
                      key={clip.audioId}
                      audioId={clip.audioId}
                      url={clip.url}
                      durationMs={clip.durationMs ?? 0}
                      peaks={clip.peaks ?? []}
                      caption={clip.caption}
                      authorName="you"
                    />
                  ),
              )}
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
            {!imagesReady || !clipsReady ? (
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
                onAudioMediaIdsChange={editor.setAudioMediaIds}
                onUploadingChange={setUploading}
                initialImages={hydratedImages}
                initialClips={hydratedClips}
                readOnly={editor.readOnly}
                onDetachImage={editor.detachImage}
                onDetachAudio={editor.detachAudio}
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
          {isText && !editor.readOnly && (
            <Button
              onClick={() => void handleDone()}
              disabled={uploading || !!editor.problemDetail}
            >
              Done
            </Button>
          )}
          {!isText && !poll.closed && (
            <Button onClick={() => void handleDone()} disabled={poll.saving}>
              Done
            </Button>
          )}
        </div>
      </div>
    </div>
  );
}
