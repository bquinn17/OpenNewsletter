import { Link, useParams } from "react-router-dom";
import { useNewsletter } from "../mocks/legacyQueries";
import { PageHeader } from "../components/layout/PageHeader";
import { Pill } from "../components/ui/Pill";
import { AnswerCard } from "../components/newsletter/AnswerCard";
import { AskedBy } from "../components/newsletter/AskedBy";
import { PollWidget } from "../components/newsletter/PollWidget";
import { CandidatesPage } from "./CandidatesPage";
import { shortCountdown } from "../utils/dates";
import type { LockedQuestion, MyResponse, PublishedQuestion } from "../mocks/types";

type RecurringKind = "photo" | "mind" | "link";

function recurringKind(prompt: string): RecurringKind | null {
  if (prompt.includes("Photo Wall")) return "photo";
  if (prompt.includes("On Your Mind")) return "mind";
  if (prompt.includes("Check It Out")) return "link";
  return null;
}

function summaryIcon(q: PublishedQuestion): string {
  if (q.kind === "poll") return "\u{1F5F3}️"; // ballot box
  const r = recurringKind(q.prompt);
  if (r === "photo") return "\u{1F4F7}";
  if (r === "mind") return "\u{1F4AC}";
  if (r === "link") return "\u{1F440}";
  return "❓"; // user-submitted
}

function summaryTitle(q: PublishedQuestion): string {
  if (recurringKind(q.prompt)) {
    // Strip leading emoji + the " — tagline" so the row shows just the feature name.
    const sepIndex = q.prompt.indexOf(" — ");
    const head = sepIndex >= 0 ? q.prompt.slice(0, sepIndex) : q.prompt;
    return head.replace(/^\P{L}+/u, "").trim();
  }
  return q.prompt;
}

function summaryCount(q: PublishedQuestion): string {
  if (q.kind === "poll") {
    return `${q.totalVotes} ${q.totalVotes === 1 ? "vote" : "votes"}`;
  }
  if (recurringKind(q.prompt) === "photo") {
    const photos = q.answers.reduce((n, a) => n + a.images.length, 0);
    return `${photos} ${photos === 1 ? "photo" : "photos"}`;
  }
  const n = q.answers.length;
  return `${n} ${n === 1 ? "reply" : "replies"}`;
}

export function NewsletterPage() {
  const { groupId = "", cycleId = "" } = useParams();
  const { data, isLoading, isError, error, refetch } = useNewsletter(groupId, cycleId);

  if (isLoading) return <LoadingShell />;
  if (isError || !data)
    return (
      <ErrorState
        message={error instanceof Error ? error.message : "Couldn't load this edition."}
        onRetry={() => refetch()}
      />
    );
  if (data.status === "voting") return <CandidatesPage />;
  if (data.status === "open") return <OpenLayout data={data} />;
  if (data.status === "published")
    return <PublishedLayout data={data} groupId={groupId} cycleId={cycleId} />;
  return null;
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

function PublishedLayout({
  data,
  groupId,
  cycleId,
}: {
  data: Extract<ReturnType<typeof useNewsletter>["data"], { status: "published" }>;
  groupId: string;
  cycleId: string;
}) {
  if (!data) return null;
  return (
    <div className="bg-cream pb-12">
      <PageHeader
        eyebrow={`${data.groupName} · editions`}
        title={`${data.monthLabel} ${data.yearLabel}`}
        back="/"
        rightSlot={<Pill tone="ink">Published</Pill>}
      />

      <div className="px-5 pt-5">
        <div className="rounded-3xl border border-line bg-white p-4">
          <div className="mb-2 text-xs font-semibold uppercase tracking-widest text-inkmuted">
            In this edition
          </div>
          <ul className="text-sm">
            {data.questions.map((q) => {
              const asker = recurringKind(q.prompt) ? null : (q.askedBy?.displayName ?? null);
              return (
                <li key={q.questionId}>
                  <a
                    href={`#q-${q.questionId}`}
                    className="group -mx-2 flex items-start gap-3 rounded-xl px-2 py-2 transition-colors hover:bg-cream"
                  >
                    <span className="flex-shrink-0 select-none text-base leading-6" aria-hidden>
                      {summaryIcon(q)}
                    </span>
                    <span className="line-clamp-2 min-w-0 flex-1 text-ink group-hover:text-grape">
                      {summaryTitle(q)}
                    </span>
                    <span className="mt-0.5 flex-shrink-0 whitespace-nowrap text-xs text-inkmuted">
                      {asker && (
                        <>
                          <span>{asker}</span>
                          <span className="mx-1">·</span>
                        </>
                      )}
                      <span>{summaryCount(q)}</span>
                    </span>
                  </a>
                </li>
              );
            })}
          </ul>
          <div className="mt-3 text-xs text-inkmuted">
            Published {data.publishedAt ? new Date(data.publishedAt).toLocaleDateString() : ""} ·{" "}
            {data.questions.length} questions · {data.reactionTotal} reactions
          </div>
        </div>
      </div>

      {data.questions.map((q) => (
        <section key={q.questionId} id={`q-${q.questionId}`} className="mt-8 scroll-mt-20 px-5">
          <div className="mb-3 flex items-baseline gap-3">
            <span className="flex-shrink-0 text-2xl leading-none" aria-hidden>
              {summaryIcon(q)}
            </span>
            <div>
              {q.kind === "poll" && (
                <div className="text-xs font-bold uppercase tracking-widest text-grape">Poll</div>
              )}
              <AskedBy askedBy={q.askedBy} isAnonymous={q.isAnonymous} />
              <h2 className="font-display text-2xl font-bold leading-tight">{q.prompt}</h2>
            </div>
          </div>
          {q.kind === "text" ? (
            <>
              {q.answers.map((a) => (
                <AnswerCard
                  key={a.responseId}
                  answer={a}
                  groupId={groupId}
                  cycleId={cycleId}
                  questionId={q.questionId}
                />
              ))}
              {q.answers.length === 0 && (
                <div className="rounded-3xl border border-dashed border-line bg-white p-6 text-center text-sm text-inkmuted">
                  Nobody answered this one. Maybe next month.
                </div>
              )}
            </>
          ) : (
            <PollWidget question={q} groupId={groupId} cycleId={cycleId} />
          )}
        </section>
      ))}

      <div className="mt-12 px-5 text-center text-sm text-inkmuted">
        <div className="mb-1 font-display text-2xl text-ink">
          That&apos;s a wrap on {data.monthLabel} 🎈
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

function OpenLayout({
  data,
}: {
  data: Extract<ReturnType<typeof useNewsletter>["data"], { status: "open" }>;
}) {
  if (!data) return null;
  const responsesByQuestion = new Map<string, MyResponse>(
    data.myResponses.map((r) => [r.questionId, r]),
  );
  const totalQs = data.questions.length;
  const publishedCount = data.myResponses.filter((r) => r.status === "published").length;

  return (
    <div className="bg-cream pb-12">
      <PageHeader
        eyebrow={`${data.groupName} · editions`}
        title={`${data.monthLabel} ${data.yearLabel}`}
        back="/"
        rightSlot={<Pill tone="coral">{shortCountdown(data.responseCloseAt)}</Pill>}
      />

      <div className="px-5 pt-5">
        <div className="rounded-3xl border border-coral/40 bg-coral/10 p-4">
          <div className="text-xs font-bold uppercase tracking-widest text-coral">
            Open for responses
          </div>
          <div className="mt-1 font-semibold">
            {totalQs} questions · publishes in {shortCountdown(data.responseCloseAt)}
          </div>
          <div className="mb-1.5 mt-3 flex items-center justify-between text-xs text-inkmuted">
            <span>Your progress</span>
            <span>
              {publishedCount} of {totalQs} published
            </span>
          </div>
          <div className="flex gap-1.5">
            {data.questions.map((q) => {
              const r = responsesByQuestion.get(q.questionId);
              const tone =
                r?.status === "published"
                  ? "bg-coral"
                  : r?.status === "draft"
                    ? "bg-coral/50"
                    : "bg-coral/15";
              return <div key={q.questionId} className={`h-2 flex-1 rounded-full ${tone}`} />;
            })}
          </div>
        </div>
      </div>

      <div className="mt-5 space-y-3 px-5">
        {data.questions.map((q) => (
          <QuestionRow
            key={q.questionId}
            q={q}
            response={responsesByQuestion.get(q.questionId)}
            groupId={data.groupId}
            cycleId={data.cycleId}
          />
        ))}
      </div>

      {data.hypeMessage && (
        <div className="mt-6 px-5">
          <div className="rounded-3xl border border-grape/40 bg-grape/10 p-4">
            <div className="text-xs font-bold uppercase tracking-widest text-grape">Hype check</div>
            <div className="mt-1 font-semibold text-ink">{data.hypeMessage}</div>
          </div>
        </div>
      )}
    </div>
  );
}

function QuestionRow({
  q,
  response,
  groupId,
  cycleId,
}: {
  q: LockedQuestion;
  response: MyResponse | undefined;
  groupId: string;
  cycleId: string;
}) {
  const status = response?.status ?? "none";
  const meta =
    status === "published"
      ? { iconBg: "bg-mint/15 text-mint", icon: "✓", tone: "Published" }
      : status === "draft"
        ? { iconBg: "bg-sun/30 text-ink", icon: "…", tone: "Draft saved" }
        : { iconBg: "bg-cream text-inkmuted", icon: "○", tone: "Not started" };
  return (
    <Link
      to={`/g/${groupId}/n/${cycleId}/respond/${q.questionId}`}
      className="block flex w-full items-start gap-4 rounded-3xl border border-line bg-white p-5 text-left shadow-soft"
    >
      <div className={`h-9 w-9 rounded-2xl ${meta.iconBg} grid place-items-center font-bold`}>
        {meta.icon}
      </div>
      <div className="min-w-0 flex-1">
        <div className="text-xs font-bold uppercase tracking-widest text-coral">{meta.tone}</div>
        {(q.askedBy || q.isAnonymous) && (
          <div className="mt-0.5 text-[11px] font-bold uppercase tracking-widest text-inkmuted">
            {q.askedBy ? `${q.askedBy.displayName} asked` : "Asked anonymously"}
          </div>
        )}
        <div className="mt-0.5 font-semibold">
          {q.kind === "poll" && <span className="text-grape">Poll · </span>}
          {q.prompt}
        </div>
        {response && (
          <div className="mt-1 text-sm text-inkmuted">
            {response.wordCount ? `${response.wordCount} words · ` : ""}
            {response.imageMediaIds?.length ? `${response.imageMediaIds.length} photos · ` : ""}
            saved just now
          </div>
        )}
      </div>
      <span className="text-grape">→</span>
    </Link>
  );
}
