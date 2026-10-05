import { Link, Navigate, useParams } from "react-router-dom";
import { useState } from "react";
import { ApiError } from "../api/client";
import { useMembership, useNewsletter, useNewsletters } from "../api/queries";
import { PageHeader } from "../components/layout/PageHeader";
import { Pill } from "../components/ui/Pill";
import { AnswerCard } from "../components/newsletter/AnswerCard";
import { PollWidget } from "../components/newsletter/PollWidget";
import { QuestionPrompt } from "../components/newsletter/QuestionPrompt";
import { cycleLabels, formatInZone, shortCountdown } from "../utils/dates";
import { stripImageTokens } from "../utils/markdown";
import type { components } from "../types/api";

type S = components["schemas"];
type OpenDetail = Extract<S["NewsletterDetailResponse"], { status: "open" }>;
type PublishedDetail = Extract<S["NewsletterDetailResponse"], { status: "published" }>;

const PREVIEW_MAX_CHARS = 140;

function draftPreview(question: S["OpenQuestionResponse"]): string | null {
  const myResponse = question.myResponse;
  if (!myResponse) return null;
  if (question.kind === "poll") {
    const option = question.pollOptions?.find((o) => o.optionId === myResponse.pollOptionId);
    return option ? option.label : null;
  }
  if (!myResponse.body) return null;
  const stripped = stripImageTokens(myResponse.body);
  if (!stripped) return null;
  return stripped.length > PREVIEW_MAX_CHARS
    ? `${stripped.slice(0, PREVIEW_MAX_CHARS).trimEnd()}…`
    : stripped;
}

function statusLabel(
  myResponse: S["MyResponseSummary"] | null,
): "Not started" | "Draft" | "Published" {
  if (!myResponse) return "Not started";
  return myResponse.status === "published" ? "Published" : "Draft";
}

export function NewsletterPage() {
  const { groupId = "", cycleId = "" } = useParams();
  const { data, isLoading, isError, error, refetch } = useNewsletter(groupId, cycleId);
  const membership = useMembership(groupId);
  const timezone = membership?.timezone ?? "UTC";

  if (isLoading) return <LoadingShell />;

  if (isError) {
    if (error instanceof ApiError && error.status === 410) {
      return <ArchivedState groupId={groupId} />;
    }
    if (error instanceof ApiError && error.status === 404) {
      return <NotFoundState groupId={groupId} />;
    }
    return (
      <ErrorState
        message={error instanceof Error ? error.message : "Couldn't load this edition."}
        onRetry={() => refetch()}
      />
    );
  }

  if (!data) return null;

  if (data.status === "voting") return <Navigate to={`/g/${groupId}/upcoming`} replace />;

  if (data.status === "open") {
    return (
      <OpenLayout
        data={data}
        groupId={groupId}
        groupName={membership?.groupName ?? ""}
        timezone={timezone}
      />
    );
  }

  return (
    <PublishedLayout
      data={data}
      groupId={groupId}
      groupName={membership?.groupName ?? ""}
      timezone={timezone}
    />
  );
}

function LoadingShell() {
  return (
    <div className="bg-cream pb-12">
      <PageHeader title="Loading…" eyebrow="Edition" back="/" />
      <div className="space-y-3 px-5 pt-6">
        {[0, 1, 2].map((i) => (
          <div key={i} className="h-32 animate-pulse rounded-3xl border border-line bg-white" />
        ))}
      </div>
    </div>
  );
}

function ErrorState({ message, onRetry }: { message: string; onRetry: () => void }) {
  return (
    <div className="bg-cream pb-12">
      <PageHeader title="Edition" eyebrow="Couldn't load" back="/" />
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

function NotFoundState({ groupId }: { groupId: string }) {
  return (
    <div className="bg-cream pb-12">
      <PageHeader title="Edition" eyebrow="Not found" back={`/g/${groupId}`} />
      <div className="px-5 pt-8 text-center">
        <div className="mb-3 text-5xl">🔍</div>
        <p className="font-semibold text-ink">This edition doesn&apos;t exist.</p>
      </div>
    </div>
  );
}

function ArchivedState({ groupId }: { groupId: string }) {
  return (
    <div className="bg-cream pb-12">
      <PageHeader title="Edition" eyebrow="Archived" back={`/g/${groupId}`} />
      <div className="px-5 pt-8 text-center">
        <div className="mb-3 text-5xl">🗄️</div>
        <p className="font-semibold text-ink">This edition has been archived.</p>
      </div>
    </div>
  );
}

function OpenLayout({
  data,
  groupId,
  groupName,
  timezone,
}: {
  data: OpenDetail;
  groupId: string;
  groupName: string;
  timezone: string;
}) {
  const sorted = [...data.questions].sort((a, b) => a.displayOrder - b.displayOrder);
  const totalQs = sorted.length;
  const publishedCount = sorted.filter((q) => q.myResponse?.status === "published").length;
  const { monthLabel, yearLabel } = cycleLabels(data.responseOpenAt, timezone);

  return (
    <div className="bg-cream pb-12">
      <PageHeader
        eyebrow={`${groupName} · editions`}
        title={`${monthLabel} ${yearLabel}`}
        back="/"
        rightSlot={<Pill tone="coral">{shortCountdown(data.responseCloseAt)}</Pill>}
      />

      <div className="px-5 pt-5">
        <div className="rounded-3xl border border-coral/40 bg-coral/10 p-4">
          <div className="text-xs font-bold uppercase tracking-widest text-coral">
            Open for responses
          </div>
          <div className="mt-1 font-semibold">
            Closes {formatInZone(data.responseCloseAt, timezone)}
          </div>
          <div className="mb-1.5 mt-3 flex items-center justify-between text-xs text-inkmuted">
            <span>Your progress</span>
            <span>
              You&apos;ve answered {publishedCount} of {totalQs}
            </span>
          </div>
          <div className="flex gap-1.5">
            {sorted.map((q) => {
              const tone =
                q.myResponse?.status === "published"
                  ? "bg-coral"
                  : q.myResponse
                    ? "bg-coral/50"
                    : "bg-coral/15";
              return <div key={q.questionId} className={`h-2 flex-1 rounded-full ${tone}`} />;
            })}
          </div>
        </div>
      </div>

      <div className="mt-5 space-y-3 px-5">
        {sorted.map((q) => (
          <QuestionRow key={q.questionId} q={q} groupId={groupId} cycleId={data.cycleId} />
        ))}
      </div>
    </div>
  );
}

function QuestionRow({
  q,
  groupId,
  cycleId,
}: {
  q: S["OpenQuestionResponse"];
  groupId: string;
  cycleId: string;
}) {
  const status = statusLabel(q.myResponse);
  const preview = draftPreview(q);
  const meta =
    status === "Published"
      ? { iconBg: "bg-mint/15 text-mint", icon: "✓" }
      : status === "Draft"
        ? { iconBg: "bg-sun/30 text-ink", icon: "…" }
        : { iconBg: "bg-cream text-inkmuted", icon: "○" };

  return (
    <Link
      to={`/g/${groupId}/n/${cycleId}/respond/${q.questionId}`}
      className="flex w-full items-start gap-4 rounded-3xl border border-line bg-white p-5 text-left shadow-soft"
    >
      <div
        className={`h-9 w-9 rounded-2xl ${meta.iconBg} grid flex-shrink-0 place-items-center font-bold`}
      >
        {meta.icon}
      </div>
      <div className="min-w-0 flex-1">
        <div className="text-xs font-bold uppercase tracking-widest text-coral">{status}</div>
        <QuestionPrompt
          prompt={q.prompt}
          kind={q.kind}
          askedBy={q.askedBy}
          isAnonymous={q.isAnonymous}
          className="mt-0.5"
        />
        {preview && <div className="mt-1 text-sm text-inkmuted">{preview}</div>}
      </div>
      <span className="text-grape">→</span>
    </Link>
  );
}

function PublishedLayout({
  data,
  groupId,
  groupName,
  timezone,
}: {
  data: PublishedDetail;
  groupId: string;
  groupName: string;
  timezone: string;
}) {
  const { data: newsletters } = useNewsletters(groupId);
  const summary = newsletters?.items.find((n) => n.cycleId === data.cycleId);
  const { monthLabel, yearLabel } = cycleLabels(
    summary?.responseOpenAt ?? data.publishedAt,
    timezone,
  );
  const sorted = [...data.questions].sort((a, b) => a.displayOrder - b.displayOrder);
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>({});

  return (
    <div className="bg-cream pb-12">
      <PageHeader
        eyebrow={`${groupName} · editions`}
        title={`${monthLabel} ${yearLabel}`}
        back="/"
        rightSlot={<Pill tone="ink">Published</Pill>}
      />

      <div className="px-5 pt-5 text-sm text-inkmuted">
        Published {formatInZone(data.publishedAt, timezone)}
      </div>

      {sorted.map((q) => {
        const isExpanded = !collapsed[q.questionId];
        return (
          <section key={q.questionId} className="mt-6 px-5">
            <button
              type="button"
              aria-expanded={isExpanded}
              onClick={() =>
                setCollapsed((prev) => ({ ...prev, [q.questionId]: !prev[q.questionId] }))
              }
              className="flex w-full items-start justify-between gap-3 text-left"
            >
              <QuestionPrompt
                prompt={q.prompt}
                kind={q.kind}
                askedBy={q.askedBy}
                isAnonymous={q.isAnonymous}
              />
              <span className="mt-1 flex-shrink-0 text-inkmuted" aria-hidden>
                {isExpanded ? "−" : "+"}
              </span>
            </button>

            {isExpanded &&
              (q.kind === "text" ? (
                <div className="mt-3">
                  {(q.answers ?? []).map((a) => (
                    <AnswerCard
                      key={a.responseId}
                      answer={a}
                      groupId={groupId}
                      timezone={timezone}
                    />
                  ))}
                  {(q.answers ?? []).length === 0 && (
                    <div className="rounded-3xl border border-dashed border-line bg-white p-6 text-center text-sm text-inkmuted">
                      Nobody answered this one. Maybe next month.
                    </div>
                  )}
                </div>
              ) : (
                <div className="mt-3">
                  <PollWidget question={q} />
                </div>
              ))}
          </section>
        );
      })}

      <div className="mt-12 px-5 text-center text-sm text-inkmuted">
        <div className="mb-1 font-display text-2xl text-ink">
          That&apos;s a wrap on {monthLabel} 🎈
        </div>
        <p>
          The next edition opens soon.{" "}
          <Link to={`/g/${groupId}/upcoming`} className="font-semibold text-grape">
            Suggest a question
          </Link>{" "}
          anytime.
        </p>
      </div>
    </div>
  );
}
