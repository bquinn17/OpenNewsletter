import { Link } from "react-router-dom";
import clsx from "clsx";
import { DEMO_BASENAME } from "../../demo";
import { InviteOnlyNote } from "../auth/InviteOnlyNote";
import { buttonClassName } from "../ui/Button";

const STEPS = [
  {
    emoji: "💡",
    tint: "bg-phase-voting",
    title: "Suggest & vote",
    body: "Anyone can suggest a question. The group votes the best ones into the next edition.",
  },
  {
    emoji: "✍️",
    tint: "bg-phase-open",
    title: "Everyone answers",
    body: "While the edition is open, everyone replies with stories, photos and poll picks.",
  },
  {
    emoji: "📬",
    tint: "bg-phase-published",
    title: "Read it together",
    body: "The edition lands for the whole group, then the comments and reactions roll in.",
  },
] as const;

/** Three tilted "editions", the top one bobbing. Decorative. */
function EditionStack() {
  return (
    <div aria-hidden="true" className="relative mx-auto h-44 w-60">
      <div className="absolute inset-x-6 top-6 h-36 -rotate-6 rounded-3xl bg-white shadow-soft">
        <div className="hero-june h-12 rounded-t-3xl" />
      </div>
      <div className="absolute inset-x-3 top-3 h-36 rotate-3 rounded-3xl bg-white shadow-soft">
        <div className="hero-may h-12 rounded-t-3xl" />
      </div>
      <div className="absolute inset-x-0 top-0 h-36 animate-float rounded-3xl bg-white shadow-card motion-reduce:animate-none">
        <div className="hero-april flex h-14 items-end rounded-t-3xl px-4 pb-2">
          <span className="font-display text-sm font-bold text-white">The Latest Edition</span>
        </div>
        <div className="space-y-2 px-4 py-3">
          <p className="text-[11px] font-semibold leading-snug text-ink">
            What made you laugh lately?
          </p>
          <div className="flex -space-x-1.5">
            {["bg-grape", "bg-mint", "bg-sky", "bg-peach"].map((tone) => (
              <span key={tone} className={clsx("h-5 w-5 rounded-full ring-2 ring-white", tone)} />
            ))}
            <span className="pl-3 text-[10px] font-semibold text-inkmuted">+4 answered</span>
          </div>
        </div>
      </div>
    </div>
  );
}

/** Home for a signed-out visitor (`docs/design/frontend.md` §7.1). */
export function Landing() {
  return (
    <div className="px-5 pb-14 pt-8">
      <EditionStack />

      <div className="mt-8 text-center">
        <p className="text-xs font-bold uppercase tracking-widest text-grape">
          By your people, for your people
        </p>
        <h1 className="mt-2 font-display text-4xl font-bold leading-tight">
          Your group&apos;s newsletter, together.
        </h1>
        <p className="mx-auto mt-3 max-w-md text-inkmuted">
          Ask good questions, share real answers, and get a little newsletter full of the people you
          care about.
        </p>

        <div className="mt-6 flex flex-wrap justify-center gap-3">
          {/* A plain link, not a router `Link`: the demo is decided when a page loads, so
              entering it has to be a full page load (`docs/design/frontend.md` §17). */}
          <a
            href={DEMO_BASENAME}
            className={buttonClassName({ size: "lg", className: "min-w-[8rem]" })}
          >
            See a demo
          </a>
          <Link
            to="/signin"
            className={buttonClassName({ variant: "ghost", size: "lg", className: "min-w-[8rem]" })}
          >
            Sign in
          </Link>
        </div>
        <InviteOnlyNote className="mx-auto mt-5 max-w-sm">
          <p className="font-semibold text-ink">Received an invite?</p>
          <Link
            to="/join"
            className={buttonClassName({
              variant: "soft",
              size: "lg",
              className: "mt-3 min-w-[8rem]",
            })}
          >
            Sign up
          </Link>
        </InviteOnlyNote>
      </div>

      <section aria-labelledby="how-it-works" className="mt-12">
        <h2
          id="how-it-works"
          className="text-center text-xs font-bold uppercase tracking-widest text-inkmuted"
        >
          How it works
        </h2>
        <ol className="mt-4 grid gap-3 sm:grid-cols-3">
          {STEPS.map((step, index) => (
            <li key={step.title} className="rounded-3xl bg-white/80 p-4 shadow-soft">
              <span
                aria-hidden="true"
                className={clsx("grid h-10 w-10 place-items-center rounded-2xl text-xl", step.tint)}
              >
                {step.emoji}
              </span>
              <h3 className="mt-3 font-display text-lg font-bold">
                <span className="sr-only">Step {index + 1}: </span>
                {step.title}
              </h3>
              <p className="mt-1 text-sm text-inkmuted">{step.body}</p>
            </li>
          ))}
        </ol>
      </section>
    </div>
  );
}
