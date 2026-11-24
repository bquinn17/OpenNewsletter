import { Link, Navigate, useParams } from "react-router-dom";
import { ChevronsDownUp, ChevronsUpDown, Trophy } from "lucide-react";
import { useMemo, useState } from "react";
import { ApiError } from "../api/client";
import { useCycleCadence, useMembership, useNewsletter, useNewsletters } from "../api/queries";
import { PageHeader } from "../components/layout/PageHeader";
import { PageErrorState } from "../components/layout/PageErrorState";
import { Pill } from "../components/ui/Pill";
import { OpenQuestionRow } from "../components/newsletter/OpenQuestionRow";
import { PublishedQuestion } from "../components/newsletter/PublishedQuestion";
import { QuestionProgress } from "../components/newsletter/QuestionProgress";
import { useElementHeight } from "../components/newsletter/useElementHeight";
import { cadenceNoun, editionLabel, formatInZone, shortCountdown } from "../utils/dates";
import type { components } from "../types/api";

/**
 * How far below the pinned prompts the reading line sits, so a question's progress segment starts
 * filling as the question fills the screen rather than only once its prompt pins.
 */
const READING_LINE_OFFSET_PX = 120;

type S = components["schemas"];
type OpenDetail = Extract<S["NewsletterDetailResponse"], { status: "open" }>;
type PublishedDetail = Extract<S["NewsletterDetailResponse"], { status: "published" }>;

/** Answers on the page rise in one after another, capped at the first 8 (`docs/design/frontend.md` §18). */
const RISE_CAP = 8;

export function NewsletterPage() {
  const { groupId = "", cycleId = "" } = useParams();
  const { data, isLoading, isError, error, refetch } = useNewsletter(groupId, cycleId);
  const membership = useMembership(groupId);
  const timezone = membership?.timezone ?? "UTC";
  const cadence = useCycleCadence();

  if (isLoading || !cadence) return <LoadingShell />;
  const { frequency } = cadence;

  if (isError) {
    if (error instanceof ApiError && error.status === 410) {
      return <ArchivedState groupId={groupId} />;
    }
    if (error instanceof ApiError && error.status === 404) {
      return <NotFoundState groupId={groupId} />;
    }
    return (
      <PageErrorState
        title="Edition"
        back="/"
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
        frequency={frequency}
      />
    );
  }

  return (
    <PublishedLayout
      data={data}
      groupId={groupId}
      groupName={membership?.groupName ?? ""}
      timezone={timezone}
      frequency={frequency}
    />
  );
}

function LoadingShell() {
  return (
    <div className="pb-12">
      <PageHeader title="Loading…" eyebrow="Edition" back="/" />
      <div className="space-y-3 px-5 pt-6">
        {[0, 1, 2].map((i) => (
          <div key={i} className="skeleton h-32 rounded-3xl border border-line" />
        ))}
      </div>
    </div>
  );
}

function NotFoundState({ groupId }: { groupId: string }) {
  return (
    <div className="pb-12">
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
    <div className="pb-12">
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
  frequency,
}: {
  data: OpenDetail;
  groupId: string;
  groupName: string;
  timezone: string;
  frequency: S["CycleCadence"]["frequency"];
}) {
  const sorted = [...data.questions].sort((a, b) => a.displayOrder - b.displayOrder);
  const totalQs = sorted.length;
  const publishedCount = sorted.filter((q) => q.myResponse?.status === "published").length;
  const { title } = editionLabel(data, timezone, frequency);

  return (
    <div className="pb-12">
      <PageHeader
        eyebrow={`${groupName} · editions`}
        title={title}
        back="/"
        rightSlot={<Pill tone="coral">{shortCountdown(data.responseCloseAt)}</Pill>}
        phase="open"
      />

      <div className="px-5 pt-5">
        <div className="rounded-3xl border border-coral/40 bg-coral/10 p-4">
          <div className="text-xs font-bold uppercase tracking-widest text-coral-ink">
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
          <OpenQuestionRow
            key={q.questionId}
            question={q}
            groupId={groupId}
            cycleId={data.cycleId}
          />
        ))}
      </div>
    </div>
  );
}

function PublishedLayout({
  data,
  groupId,
  groupName,
  timezone,
  frequency,
}: {
  data: PublishedDetail;
  groupId: string;
  groupName: string;
  timezone: string;
  frequency: S["CycleCadence"]["frequency"];
}) {
  const { data: newsletters } = useNewsletters(groupId);
  const summary = newsletters?.items.find((n) => n.cycleId === data.cycleId);
  // The published payload carries no `responseOpenAt` (`docs/design/frontend.md`
  // §7.2), so the monthly label falls back to the cycle's `NewsletterSummary`,
  // then to `publishedAt`. Weekly ignores `responseOpenAt` entirely and labels
  // by `publishedAt` directly.
  const { title, short } = editionLabel(
    {
      responseOpenAt: summary?.responseOpenAt ?? data.publishedAt,
      responseCloseAt: data.responseCloseAt,
      publishedAt: data.publishedAt,
    },
    timezone,
    frequency,
  );
  const sorted = useMemo(
    () => [...data.questions].sort((a, b) => a.displayOrder - b.displayOrder),
    [data.questions],
  );
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>({});
  // The progress bar sticks under the header and the question prompts under both, at their
  // measured heights, so nothing overlaps or leaves a gap whatever the header's content.
  const [headerRef, headerHeight] = useElementHeight<HTMLDivElement>();
  const [progressRef, progressHeight] = useElementHeight<HTMLElement>();
  const promptTop = headerHeight + progressHeight;

  const riseIndexByResponseId = useMemo(() => {
    const map = new Map<string, number>();
    sorted
      .flatMap((q) => (q.kind === "text" ? (q.answers ?? []) : []))
      .slice(0, RISE_CAP)
      .forEach((a, i) => map.set(a.responseId, i));
    return map;
  }, [sorted]);

  const allCollapsed = sorted.length > 0 && sorted.every((q) => collapsed[q.questionId]);

  function toggleAll() {
    const next = !allCollapsed;
    setCollapsed(Object.fromEntries(sorted.map((q) => [q.questionId, next])));
  }

  return (
    <div className="pb-12">
      {/* The header carries the label, the publish date and the leaderboard link, and the
          question progress bar sticks under it (`docs/design/frontend.md` §7.2). */}
      <PageHeader
        ref={headerRef}
        eyebrow={
          frequency === "weekly"
            ? `${groupName} · published`
            : `${groupName} · published ${formatInZone(data.publishedAt, timezone, { month: "short", day: "numeric" })}`
        }
        title={
          // Shares a `view-transition-name` with the Home row's date tile it morphed from
          // (`docs/design/frontend.md` §18), keyed by cycle like `EditionRow`.
          <h1 style={{ viewTransitionName: `edition-${data.cycleId}` }} className="truncate">
            {title}
          </h1>
        }
        back="/"
        rightSlot={
          <Link
            to={`/g/${groupId}/leaderboard`}
            aria-label="Group leaderboard"
            title="Group leaderboard"
            className="grid h-9 w-9 place-items-center rounded-full border border-line bg-white text-grape-ink"
          >
            <Trophy aria-hidden className="h-4 w-4" />
          </Link>
        }
        phase="published"
      />

      <QuestionProgress
        questions={sorted.map((q) => ({ questionId: q.questionId, prompt: q.prompt }))}
        navRef={progressRef}
        stickyTopPx={headerHeight}
        readingLinePx={promptTop + READING_LINE_OFFSET_PX}
        trailing={
          <button
            type="button"
            onClick={toggleAll}
            className="flex shrink-0 items-center gap-1 whitespace-nowrap rounded-full px-2 py-1.5 text-xs font-semibold text-grape-ink"
          >
            {allCollapsed ? (
              <ChevronsUpDown aria-hidden className="h-4 w-4" />
            ) : (
              <ChevronsDownUp aria-hidden className="h-4 w-4" />
            )}
            {allCollapsed ? "Expand all" : "Collapse all"}
          </button>
        }
      />

      {sorted.map((q) => (
        <PublishedQuestion
          key={q.questionId}
          question={q}
          groupId={groupId}
          cycleId={data.cycleId}
          timezone={timezone}
          collapsed={!!collapsed[q.questionId]}
          onToggle={() =>
            setCollapsed((prev) => ({ ...prev, [q.questionId]: !prev[q.questionId] }))
          }
          cadenceNoun={cadenceNoun(frequency)}
          riseIndexByResponseId={riseIndexByResponseId}
          stickyTopPx={promptTop}
        />
      ))}

      <div className="mt-12 px-5 text-center text-sm text-inkmuted">
        <div className="mb-1 font-display text-2xl text-ink">
          {frequency === "weekly"
            ? `That's a wrap on the ${short} edition 🎈`
            : `That's a wrap on ${short} 🎈`}
        </div>
        <p>
          The next edition opens soon.{" "}
          <Link to={`/g/${groupId}/upcoming`} className="font-semibold text-grape-ink">
            Suggest a question
          </Link>{" "}
          anytime.
        </p>
      </div>
    </div>
  );
}
