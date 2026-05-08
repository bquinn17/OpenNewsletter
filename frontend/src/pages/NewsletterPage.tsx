import { Link, useParams } from "react-router-dom";
import { useNewsletter } from "../api/queries";
import { PageHeader } from "../components/layout/PageHeader";
import { Pill } from "../components/ui/Pill";
import { AnswerCard } from "../components/newsletter/AnswerCard";
import { PollWidget } from "../components/newsletter/PollWidget";
import { CandidatesPage } from "./CandidatesPage";
import { shortCountdown } from "../utils/dates";
import type { LockedQuestion, MyResponse } from "../api/types";

export function NewsletterPage() {
  const { groupId = "", cycleId = "" } = useParams();
  const { data, isLoading, isError, error, refetch } = useNewsletter(groupId, cycleId);

  if (isLoading) return <LoadingShell />;
  if (isError || !data) return <ErrorState message={error instanceof Error ? error.message : "Couldn't load this edition."} onRetry={() => refetch()} />;
  if (data.status === "voting") return <CandidatesPage />;
  if (data.status === "open") return <OpenLayout data={data} />;
  if (data.status === "published") return <PublishedLayout data={data} groupId={groupId} cycleId={cycleId} />;
  return null;
}

function ErrorState({ message, onRetry }: { message: string; onRetry: () => void }) {
  return (
    <div className="bg-cream pb-12">
      <PageHeader title="Edition" eyebrow="Couldn't load" back="/" />
      <div className="px-5 pt-8 text-center">
        <div className="text-5xl mb-3">😬</div>
        <p className="text-ink font-semibold">{message}</p>
        <button onClick={onRetry} className="mt-5 px-5 py-3 rounded-full bg-ink text-cream font-semibold">
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
      <div className="px-5 pt-6 space-y-3">
        {[0, 1, 2].map((i) => (
          <div key={i} className="h-32 rounded-3xl bg-white border border-line animate-pulse" />
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
        <div className="bg-white rounded-3xl border border-line p-4">
          <div className="text-xs uppercase tracking-widest text-inkmuted font-semibold mb-2">In this edition</div>
          <ol className="space-y-2 text-sm">
            {data.questions.map((q, i) => (
              <li key={q.questionId} className="flex gap-3">
                <span className="font-bold text-grape">{String(i + 1).padStart(2, "0")}</span>
                <span>
                  {q.kind === "poll" && <span className="text-grape font-semibold">Poll · </span>}
                  {q.prompt}
                </span>
              </li>
            ))}
          </ol>
          <div className="text-xs text-inkmuted mt-3">
            Published {data.publishedAt ? new Date(data.publishedAt).toLocaleDateString() : ""} ·{" "}
            {data.questions.length} questions · {data.reactionTotal} reactions
          </div>
        </div>
      </div>

      {data.questions.map((q, i) => (
        <section key={q.questionId} className="px-5 mt-8">
          <div className="flex items-baseline gap-3 mb-3">
            <span className="font-display font-bold text-coral text-2xl">{String(i + 1).padStart(2, "0")}</span>
            <div>
              {q.kind === "poll" && (
                <div className="text-xs uppercase tracking-widest text-grape font-bold">Poll</div>
              )}
              <h2 className="font-display text-2xl font-bold leading-tight">{q.prompt}</h2>
            </div>
          </div>
          {q.kind === "text" ? (
            <>
              {q.answers.map((a) => (
                <AnswerCard key={a.responseId} answer={a} groupId={groupId} cycleId={cycleId} questionId={q.questionId} />
              ))}
              {q.answers.length === 0 && (
                <div className="bg-white border border-dashed border-line rounded-3xl p-6 text-center text-inkmuted text-sm">
                  Nobody answered this one. Maybe next month.
                </div>
              )}
            </>
          ) : (
            <PollWidget question={q} groupId={groupId} cycleId={cycleId} />
          )}
        </section>
      ))}

      <div className="px-5 mt-12 text-center text-inkmuted text-sm">
        <div className="font-display text-2xl text-ink mb-1">That's a wrap on {data.monthLabel} 🎈</div>
        <p>
          The next edition opens soon.{" "}
          <Link to={`/g/${groupId}/upcoming`} className="text-grape font-semibold">
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
  const responsesByQuestion = new Map<string, MyResponse>(data.myResponses.map((r) => [r.questionId, r]));
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
        <div className="bg-coral/10 border border-coral/40 rounded-3xl p-4">
          <div className="text-xs uppercase tracking-widest font-bold text-coral">Open for responses</div>
          <div className="font-semibold mt-1">
            {totalQs} questions · publishes in {shortCountdown(data.responseCloseAt)}
          </div>
          <div className="flex items-center justify-between text-xs text-inkmuted mt-3 mb-1.5">
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
              return <div key={q.questionId} className={`flex-1 h-2 rounded-full ${tone}`} />;
            })}
          </div>
        </div>
      </div>

      <div className="px-5 mt-5 space-y-3">
        {data.questions.map((q) => (
          <QuestionRow key={q.questionId} q={q} response={responsesByQuestion.get(q.questionId)} groupId={data.groupId} cycleId={data.cycleId} />
        ))}
      </div>

      {data.hypeMessage && (
        <div className="px-5 mt-6">
          <div className="rounded-3xl bg-grape/10 border border-grape/40 p-4">
            <div className="text-xs uppercase tracking-widest text-grape font-bold">Hype check</div>
            <div className="font-semibold mt-1 text-ink">{data.hypeMessage}</div>
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
      className="w-full text-left bg-white rounded-3xl border border-line shadow-soft p-5 flex items-start gap-4 block"
    >
      <div className={`w-9 h-9 rounded-2xl ${meta.iconBg} grid place-items-center font-bold`}>{meta.icon}</div>
      <div className="flex-1 min-w-0">
        <div className="text-xs uppercase tracking-widest font-bold text-coral">{meta.tone}</div>
        <div className="font-semibold mt-0.5">
          {q.kind === "poll" && <span className="text-grape">Poll · </span>}
          {q.prompt}
        </div>
        {response && (
          <div className="text-sm text-inkmuted mt-1">
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
