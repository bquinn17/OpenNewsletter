import { Link } from "react-router-dom";
import { useState, useRef, useEffect, type ReactNode } from "react";
import clsx from "clsx";
import { useConfig, useNewsletters } from "../api/queries";
import { useCurrentGroup } from "../state/currentGroup";
import { Avatar } from "../components/ui/Avatar";
import { Pill } from "../components/ui/Pill";
import { shortCountdown } from "../utils/dates";
import type { NewsletterSummary } from "../api/types";

export function HomePage() {
  const { data: config } = useConfig();
  const { currentGroupId, setCurrentGroup } = useCurrentGroup();
  const groupId = currentGroupId ?? "g_trail";
  const { data: newsletters = [] } = useNewsletters(groupId);

  const membership = config?.memberships.find((m) => m.groupId === groupId);
  const open = newsletters.find((n) => n.status === "open");
  const voting = newsletters.find((n) => n.status === "voting");
  const published = newsletters.filter((n) => n.status === "published");

  return (
    <div className="bg-cream pb-12">
      <div className="px-5 pt-6 pb-3 flex items-center justify-between">
        <GroupSwitcher
          memberships={config?.memberships ?? []}
          currentGroupId={groupId}
          onPick={setCurrentGroup}
          fallbackClass={membership?.gradient.className ?? "bg-coral"}
          fallbackName={membership?.groupName ?? "—"}
        />
        <Link to="/settings" aria-label="Profile">
          <Avatar name={config?.user.displayName ?? "?"} color={config?.user.avatarColor} size="sm" />
        </Link>
      </div>

      <div className="px-5 pt-2 pb-2">
        <h1 className="font-display text-2xl leading-tight">
          Hey {config?.user.displayName} 👋
        </h1>
        <p className="text-inkmuted text-sm mt-1">{membership?.groupName} editions</p>
      </div>

      {open && (
        <Section title="Open for responses" tone="coral">
          <EditionRow
            newsletter={open}
            groupId={groupId}
            tone="coral"
            description={
              <>
                {shortCountdown(open.responseCloseAt)} left · you've published {open.myPublishedCount} of{" "}
                {open.questionCount}
              </>
            }
            adornment={<Pill tone="coral">{shortCountdown(open.responseCloseAt)}</Pill>}
          />
        </Section>
      )}

      {voting && (
        <Section title="Coming up — voting on questions" tone="grape">
          <EditionRow
            newsletter={voting}
            groupId={groupId}
            tone="grape"
            description="Suggest and vote on questions for next month"
            adornment={<Pill tone="grape">vote</Pill>}
          />
        </Section>
      )}

      <Section title="Published" tone="ink">
        {published.length === 0 ? (
          <div className="rounded-3xl border border-dashed border-line p-6 text-center text-inkmuted text-sm">
            No published editions yet.
          </div>
        ) : (
          <div className="space-y-2">
            {published.map((nl) => (
              <EditionRow
                key={nl.cycleId}
                newsletter={nl}
                groupId={groupId}
                tone="ink"
                description={
                  <>
                    {nl.questionCount} questions · published{" "}
                    {nl.publishedAt ? new Date(nl.publishedAt).toLocaleDateString() : ""}
                  </>
                }
                adornment={<span className="text-inkmuted">→</span>}
              />
            ))}
          </div>
        )}
      </Section>
    </div>
  );
}

function Section({
  title,
  tone,
  children,
}: {
  title: string;
  tone: "coral" | "grape" | "ink";
  children: ReactNode;
}) {
  const dotClass = tone === "coral" ? "bg-coral" : tone === "grape" ? "bg-grape" : "bg-ink";
  return (
    <section className="px-5 mt-5">
      <div className="flex items-center gap-2 mb-2">
        <span className={clsx("w-2 h-2 rounded-full", dotClass)} />
        <h2 className="font-display text-base font-semibold uppercase tracking-wide text-inkmuted">{title}</h2>
      </div>
      {children}
    </section>
  );
}

function EditionRow({
  newsletter,
  groupId,
  tone,
  description,
  adornment,
}: {
  newsletter: NewsletterSummary;
  groupId: string;
  tone: "coral" | "grape" | "ink";
  description: ReactNode;
  adornment?: ReactNode;
}) {
  const wrap = {
    coral: "bg-coral/10 border-coral/40 hover:border-coral",
    grape: "bg-grape/10 border-grape/40 hover:border-grape",
    ink: "bg-white border-line hover:border-ink",
  }[tone];
  const accent = {
    coral: "bg-coral text-white",
    grape: "bg-grape text-white",
    ink: "bg-cream text-ink",
  }[tone];
  return (
    <Link
      to={`/g/${groupId}/n/${newsletter.cycleId}`}
      className={clsx(
        "block rounded-3xl border p-4 transition flex items-center gap-3",
        wrap,
      )}
    >
      <div className={clsx("w-12 h-12 rounded-2xl grid place-items-center font-display font-bold", accent)}>
        <div className="text-[10px] uppercase tracking-widest opacity-80 leading-none">{newsletter.monthLabel}</div>
        <div className="text-sm leading-none mt-0.5">{newsletter.yearLabel}</div>
      </div>
      <div className="flex-1 min-w-0">
        <div className="font-semibold">
          {newsletter.monthLabel} {newsletter.yearLabel}
        </div>
        <div className="text-sm text-inkmuted">{description}</div>
      </div>
      {adornment}
    </Link>
  );
}

function GroupSwitcher({
  memberships,
  currentGroupId,
  onPick,
  fallbackClass,
  fallbackName,
}: {
  memberships: { groupId: string; groupName: string; gradient: { className: string } }[];
  currentGroupId: string;
  onPick: (id: string) => void;
  fallbackClass: string;
  fallbackName: string;
}) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    function onDoc(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    }
    document.addEventListener("mousedown", onDoc);
    return () => document.removeEventListener("mousedown", onDoc);
  }, []);
  const others = memberships.filter((m) => m.groupId !== currentGroupId);
  return (
    <div ref={ref} className="relative">
      <button
        onClick={() => setOpen((v) => !v)}
        className="flex items-center gap-2 px-3 py-1.5 rounded-full bg-white shadow-soft border border-line"
      >
        <div className={clsx("w-6 h-6 rounded-full", fallbackClass)} />
        <span className="font-semibold text-sm">{fallbackName}</span>
        <svg className="w-4 h-4 text-inkmuted" viewBox="0 0 20 20" fill="currentColor">
          <path d="M5.23 7.21a.75.75 0 011.06.02L10 11.06l3.71-3.83a.75.75 0 111.08 1.04l-4.25 4.39a.75.75 0 01-1.08 0L5.21 8.27a.75.75 0 01.02-1.06z" />
        </svg>
      </button>
      {open && others.length > 0 && (
        <div className="absolute left-0 mt-2 w-60 bg-white rounded-2xl shadow-pop border border-line p-1 z-40">
          {others.map((m) => (
            <button
              key={m.groupId}
              onClick={() => {
                onPick(m.groupId);
                setOpen(false);
              }}
              className="w-full flex items-center gap-2 px-3 py-2 rounded-xl hover:bg-cream text-left"
            >
              <div className={clsx("w-6 h-6 rounded-full", m.gradient.className)} />
              <span className="font-semibold text-sm">{m.groupName}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
