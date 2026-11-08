# Frontend design review: color, motion and reading

Reviewed 2026-10-20 on branch `bquinn/dev-stack-deploy`; line references re-checked 2026-10-25. The goal: a frontend that is clean but fun and modern, feels effortless, has more striking animation, and reads well when an edition is long. Nothing in this review has been implemented yet.

**How it was reviewed.** I read the theme ([`tailwind.config.ts`](../frontend/tailwind.config.ts), [`tailwind.css`](../frontend/src/styles/tailwind.css)) and every page and component. I also rendered the app in mock mode (`VITE_USE_MOCKS=true`) with Playwright at 390px and 1280px wide, and computed WCAG 2.x contrast ratios for each color pair the components use.

**Summary.** The foundation suits the goal: Fraunces with Plus Jakarta Sans, the lavender-to-blush canvas, and color-coding by edition phase. Three things hold it back:
- a handful of real defects (§1)
- a palette that fails contrast and gives each hue several meanings (§2)
- published editions where the comment and reaction controls take more space than the answers (§3)

§4 covers motion, §5 smaller flow fixes, §6 places where the design docs and the code disagree, and §7 a suggested order of work.

---

## 1. Defects to fix first

1. **Formatting in answers is broken.** [`markdown.tsx:145`](../frontend/src/utils/markdown.tsx#L145) styles the body with `prose prose-sm`, but `@tailwindcss/typography` isn't installed, so those classes do nothing. The hand-written `.markdown-preview` rules in [`tailwind.css:111-189`](../frontend/src/styles/tailwind.css#L111-L189) are never applied anywhere. In the rendered app, headings, bullets, numbered lists and blockquotes all collapse into plain lines. Applying `markdown-preview` in `MarkdownBody` fixes it, and §3.2 then extends those rules for reading.
2. **The palette fails WCAG AA throughout** (§2.1). [`frontend.md` §11](design/frontend.md#11-accessibility--ux) sets AA (4.5:1) as the target.
3. **The Respond page's bottom bar** ([`RespondPage.tsx:260`](../frontend/src/pages/RespondPage.tsx#L260)) is `fixed bottom-3 left-3 right-3`:
   - On desktop it spans the whole browser window, outside the app card.
   - At 390px the draft badge wraps to three lines and "Save & exit" wraps to two.
4. **No safe-area insets.** [`index.html:7`](../frontend/index.html#L7) sets `viewport-fit=cover`, so in the installed iOS app:
   - the sticky header sits under the status bar
   - the bottom bar sits under the home indicator

   Neither uses `env(safe-area-inset-*)`.
5. **No `prefers-reduced-motion` handling anywhere**, although [`frontend.md` §11](design/frontend.md#11-accessibility--ux) requires it. This has to land before any of §4.
6. **Destructive dialogs focus the wrong button.** `ConfirmDialog` autofocuses the confirm button ([`ConfirmDialog.tsx:40`](../frontend/src/components/ui/ConfirmDialog.tsx#L40)), so pressing Enter deletes. When `tone="danger"`, focus Cancel instead.
7. **Avatar fallback is a bare letter.** With no `colorClassName`, `Avatar` renders an initial with no background ([`Avatar.tsx:53`](../frontend/src/components/ui/Avatar.tsx#L53)). The published edition in mock mode shows "S", "Q" and "A" floating with no circle. Fall back to one of the eight avatar colors, picked by hashing the name.

---

## 2. Color

### 2.1 Contrast

Requirements: 4.5:1 for body text, 3:1 for large text and UI boundaries. Almost every failing pair below is used for `text-xs` or `text-sm`, so 4.5:1 applies.

| Pair | Where | Ratio |
|---|---|---|
| mint text on mint/15 tint | `Pill tone="mint"`, the Published icon tile | 1.63 |
| sky text on sky/15 tint | `Pill tone="sky"` | 1.72 |
| white on mint | success toast ([`Toast.tsx:14`](../frontend/src/components/ui/Toast.tsx#L14)) | 1.80 |
| white on sky | Candidates banner end ([`CandidatesPage.tsx:109`](../frontend/src/pages/CandidatesPage.tsx#L109)) | 1.89 |
| white on sky-400 / emerald-300 / purple-300 | ends of the `grape-sky`, `forest-mint` and `slate-lilac` group gradients ([`gradient.ts`](../frontend/src/utils/gradient.ts)) | 2.14 / 1.52 / 1.77 |
| white on green-500 / teal-500 / orange-500 | avatar colors ([`avatarColor.ts`](../frontend/src/utils/avatarColor.ts)) | 2.28 / 2.49 / 2.80 |
| coral on coral/10 | "Closes…" banner, inline alerts | 2.71 |
| white on coral | primary `Button` ([`Button.tsx:20`](../frontend/src/components/ui/Button.tsx#L20)), voted tile, error toast | 3.03 |
| coral on white | status labels, errors, Sign out | 3.03 |
| grape on phase-voting chip | Home "VOTING" chip (10px) | 3.62 |
| grape on grape/10 | `Pill tone="grape"` | 3.88 |
| rose-600 on phase-open chip | Home "OPEN" chip (10px, [`phase.ts:18`](../frontend/src/utils/phase.ts#L18)) | 3.89 |
| `inkmuted` on the canvas gradient | Home section labels | 4.05–4.17 |
| grape on white | links, Edit, the "Poll" label | 4.39 |

Pairs that pass: `inkmuted` on white (5.00) and on cream (4.65), emerald-700 on the phase-published chip (4.74), and cream on ink.

### 2.2 Each hue has too many jobs

- **Coral** is the primary button, errors, Delete, the open phase, the countdown, and every status label. That includes the "PUBLISHED" label on the open edition ([`NewsletterPage.tsx:222`](../frontend/src/pages/NewsletterPage.tsx#L222)), while "published" is mint everywhere else.
- **Delete and Publish look the same.** Both are coral.
- **Primary buttons disagree.** The comment Post button is grape ([`CommentComposer.tsx:84`](../frontend/src/components/newsletter/CommentComposer.tsx#L84)); every other primary button is coral.
- **Three color systems sit side by side:** the custom hues in `tailwind.config.ts`, the pastel phase tints, and the raw Tailwind palette used in `gradient.ts`, `avatarColor.ts` and `phase.ts`.

### 2.3 Proposed palette

**One meaning per hue:**

| Role | Hue |
|---|---|
| Primary action, the open phase, urgency | coral |
| Links, secondary controls, the voting phase, "your pick" | grape |
| Success, published | mint |
| Draft, warning | sun |
| Destructive actions | new red, `#DC2626` (4.83:1 on white) |

**Keep the vivid colors as fills, and add darker shades for text.** Each of these passes on white and on its own 12% tint:

| Token | Hex | On white | On own 12% tint |
|---|---|---|---|
| `coral-ink` | `#BD434F` | 5.15 | 4.50 |
| `grape-ink` | `#6C50E0` | 5.44 | 4.66 |
| `mint-ink` | `#237E5F` | 4.98 | 4.60 |
| `sky-ink` | `#367899` | 4.88 | 4.51 |

**Other changes:**
- **Primary button:** ink text on coral (5.81:1) is the bolder, more modern option. To keep white text, darken the coral fill to `#CC4856` (4.54:1).
- **Toasts:** ink text on mint (9.79:1).
- **Muted text:** change `inkmuted` from `#6F6E7C` to `#5E5D6B`. That passes on white (6.45), lavender (5.38) and blush (5.23).
- **Phase chips:** use the `-ink` shades for their text.
- **Avatar and group-gradient colors:** where white text fails, switch to ink text or a darker stop. These class strings are presentation only. The slugs are the API contract ([`frontend.md` §14a](design/frontend.md#14a-client-derived-display-data-not-api-fields)), so the classes can change freely.

### 2.4 Tokens and dark mode

Move colors to CSS variables on `:root`, with Tailwind reading them as `rgb(var(--coral) / <alpha-value>)` so opacity modifiers keep working. That is what [`frontend.md` §4.2](design/frontend.md#42-theming) already describes, and it makes dark mode mostly a matter of swapping variables under `prefers-color-scheme: dark`. Dark mode is worth doing for reading an edition at night.

### 2.5 Make the group gradient the newsletter's identity

Today a group's gradient only shows as a 28px dot in the `GroupSwitcher`. Use it as the masthead of the edition page (§3.3), so each group's newsletter feels like its own publication. Three gradient ends fail with white text (§2.1), so title text over them needs a dark overlay or ink color.

### 2.6 Visual hierarchy

Nearly every surface is the same white `rounded-3xl` card with a border and shadow, so nothing stands out. Use three tiers:
- **Hero:** gradient fill, for the thing to act on now.
- **Primary card:** white with a shadow.
- **Secondary:** a flat tint with no border.

---

## 3. Reading long editions

In mock mode, the September edition (5 questions: 5 short text answers and a poll) is about 4,000px tall at 390px wide. Most of each answer card is comment and reaction controls, not the answer.

### 3.1 Trim the controls on each answer

These changes should make each answer card roughly 40% shorter (estimated from the screenshots):

- **Comment composer.** Today every answer shows a full textarea, an image dropzone and a Post button, even with nothing typed ([`CommentList.tsx:72`](../frontend/src/components/newsletter/CommentList.tsx#L72), [`CommentComposer.tsx`](../frontend/src/components/newsletter/CommentComposer.tsx)).
  - Collapse it to a one-line "Add a comment…" button.
  - Tapping it expands a textarea plus a 📷 button.
  - The dropzone appears only after tapping 📷.
- **Reactions.** Each answer shows six dashed quick-pick emoji, and the "+" wraps onto its own line at phone width ([`ReactionBar.tsx:59`](../frontend/src/components/newsletter/ReactionBar.tsx#L59)). Show only existing reactions plus one add button, and move the quick picks into the picker's top row.
- **Collapsed comments.** Show a summary row ("💬 3" plus small avatars) instead of always rendering the last three comments.

### 3.2 Typography

- **Answer text:** 17px with 1.65 line height (it is 15px today), cap the measure near 65 characters, and add clear paragraph spacing.
- **Lists:** markers in coral (`li::marker`).
- **Blockquotes:** italic Fraunces pull quotes with a grape rule.
- **Answer grouping:** put each question's answers on one surface separated by dividers, or use an avatar rail down the side, instead of stacking N heavy bordered cards ([`AnswerCard.tsx:25`](../frontend/src/components/newsletter/AnswerCard.tsx#L25)).

### 3.3 Navigation within an edition

- **Masthead.** Show:
  - the group gradient (§2.5)
  - the month in large Fraunces
  - a stack of contributor avatars
  - a line like "5 answers · 4 min read"

  Reading time is computed in the frontend, so it belongs in [`frontend.md` §14a](design/frontend.md#14a-client-derived-display-data-not-api-fields).
- **Sticky question chips.** A horizontally scrolling row of chips under the page header, one per question. It highlights the question being read (IntersectionObserver) and jumps there on tap.
- **Sticky prompts.** Each question's prompt stays pinned under the header while you read its answers, so you always know which question an answer belongs to.
- **Reading progress bar.** A 3px bar in the page header, in pure CSS with `animation-timeline: scroll()`. Browsers that don't support it simply show no bar.
- **Collapse controls.** Replace the tiny "−/+" glyph ([`NewsletterPage.tsx:306`](../frontend/src/pages/NewsletterPage.tsx#L306)) with:
  - a rotating chevron
  - an "N answers" count when collapsed
  - a "Collapse all" toggle in the header
- **Header height.** The app bar ([`AppShell.tsx:46`](../frontend/src/components/layout/AppShell.tsx#L46)) plus the `PageHeader` take about 120px of an 844px screen. On reading pages, hide the app bar when scrolling down and show it when scrolling up.

### 3.4 Images

- **Inline images are blurry.** They use the 400px thumbnail (`THUMB_LONG_EDGE_PX`, [`markdown.tsx:114`](../frontend/src/utils/markdown.tsx#L114)), which is soft on 2–3× phone screens. Show the 1200px `displayUrl` and use the thumbnail as a blur-up placeholder, as [`frontend.md` §9](design/frontend.md#9-renderers) already describes.
- **Gallery images leave the app.** They open `displayUrl` in a new tab ([`ImageGallery.tsx:22`](../frontend/src/components/newsletter/ImageGallery.tsx#L22)), which throws the installed app out to the browser. Add an in-app lightbox with swipe between an answer's images. The same applies to comment images in `CommentRow`.

---

## 4. Motion

### 4.1 Principle

Be loud at moments of accomplishment and quiet for routine actions:
- **Quiet:** navigation and typing stay fast.
- **Loud:** votes, reactions, publishing and finishing the edition get the flourish.

Everything here goes behind `motion-safe:` plus a global reduced-motion rule (§1.5).

### 4.2 Foundation

- **Motion tokens** in `tailwind.config.ts`, which today defines a single `pop` keyframe ([line 42](../frontend/tailwind.config.ts#L42)):
  - durations: 120 / 220 / 400ms
  - easings: a springy `cubic-bezier(.34,1.56,.64,1)` and a smooth `cubic-bezier(.2,.8,.2,1)`
  - keyframes: pop-in, rise (fade up), shimmer, float-up
- **A global reduced-motion block** in `tailwind.css` that neutralizes animations and transitions.
- **Press feedback:** `active:scale-[.97]` on `Button`, `EditionRow`, `CandidateCard` and pills. This is the cheapest and biggest "feels good" improvement.

### 4.3 Moments

Roughly ranked by delight per effort:

| Moment | Effect | How |
|---|---|---|
| Publish an answer | The button turns into a ✓, a confetti burst plays, and the progress segment on the edition page fills | Hand-rolled confetti, about 40 lines on the Web Animations API. `canvas-confetti`'s default instance builds a `blob:` worker, which the CSP's `worker-src 'self'` blocks and the Playwright no-CSP-violations test would catch. If you use it, call `confetti.create(canvas, { useWorker: false })`. |
| All questions answered | A full-screen "You're all in for October 🎉" | The same confetti, bigger |
| Upvote | The vote tile springs, the count rolls up, and coral sparks burst. The Top-sorted list reorders smoothly instead of jumping. | Web Animations API for the burst; `document.startViewTransition` around the cache update for the reorder |
| Reaction | The emoji pops, floats up and fades; the count bumps | A cloned emoji animated with the Web Animations API |
| Home → edition | The month tile on the `EditionRow` morphs into the masthead | React Router 6.30's `<Link viewTransition>` plus `view-transition-name` |
| Edition loads | The masthead gradient drifts slowly; answers fade up one after another, capped at about 8 | CSS `animation-delay: calc(var(--i) * 40ms)` |
| Poll results | Bars grow from zero as they scroll into view, and the winner gets ✨ | IntersectionObserver |
| Saving a draft | The "Saving…" dot pulses, then a check draws in | SVG `stroke-dashoffset` |
| Under 24 hours left | The countdown pill gently pulses | `motion-safe:` animation |
| Loading | Shimmering skeletons shaped like the real content, instead of `animate-pulse` blocks | CSS |
| Toasts | Spring up, swipe to dismiss | CSS plus pointer events |

View transitions and scroll-driven animations are progressive enhancement: browsers without support just skip the animation. Because the app ships no lazy chunks ([`frontend.md` §14.2](design/frontend.md#142-service-worker)), every animation helper must be a static import.

---

## 5. Smaller fixes for a smoother flow

- **Respond page: move between questions.** Replace the header title (currently the full prompt, [`RespondPage.tsx:164`](../frontend/src/pages/RespondPage.tsx#L164)) with "Question 2 of 5" and add previous/next arrows, so you can answer every question without returning to the list.
- **Respond page: less repetition.** The prompt appears twice (header and card), and the deadline appears three times:
  - the countdown pill
  - the "Closes…" banner ([line 172](../frontend/src/pages/RespondPage.tsx#L172))
  - the auto-publish note ([line 179](../frontend/src/pages/RespondPage.tsx#L179))

  Merge the banner and the note into one.
- **Respond page: bottom bar.**
  - Constrain it to the app width: `fixed inset-x-0 bottom-0` with an inner `mx-auto max-w-2xl`.
  - Pad it with `env(safe-area-inset-bottom)`.
  - Shorten "Save & exit" to "Done".
  - Show the save state as a dot and a short label above the buttons.
- **Open edition list.** Rows stack up to three uppercase labels in three colors ("NOT STARTED / POLL / ASKED ANONYMOUSLY"). Use one meta line instead ("Poll · asked anonymously"); the status already shows in the icon tile.
- **Home.** The open edition has a deadline and needs your answers, yet it looks the same as past editions. Make it a hero card with progress and an "Answer 4 more →" button, and show votes left on the voting card.
- **Candidates.** The "The October edition is open — answer now" link has no arrow or color, so it doesn't look tappable ([`CandidatesPage.tsx:125`](../frontend/src/pages/CandidatesPage.tsx#L125)).
- **Focus styles.**
  - Only `CandidateCard` has a `focus-visible` style.
  - 17 inputs use `focus:outline-none` with a faint `ring-coral/30`.

  Add a global `:focus-visible` outline: 2px, `grape-ink`, with an offset.
- **Icons.** "←", "＋", "×" and "−/+" are text characters that render differently across fonts. Use one SVG icon set for controls. `lucide-react` imports statically, so it fits the no-lazy-chunks rule. Keep emoji for personality.
- **Safe area at the top.** In standalone mode the app bar needs `padding-top: env(safe-area-inset-top)`.

---

## 6. Docs that disagree with the code

Per [`CLAUDE.md`](../CLAUDE.md), these are flagged for a decision rather than silently fixed. For each, either build what the doc says or change the doc.

| Doc | Says | Code |
|---|---|---|
| [`frontend.md` §4.2](design/frontend.md#42-theming) | CSS variables on `:root`; dark mode via `prefers-color-scheme` | Hex values in `tailwind.config.ts`; no variables, no `dark:` classes |
| [`frontend.md` §4.3](design/frontend.md#43-typography) | Markdown is styled by the prose ruleset in `tailwind.css` | The ruleset (`.markdown-preview`) is never applied (§1.1) |
| [`frontend.md` §9](design/frontend.md#9-renderers) | `<picture>` with `srcset` from `displayUrl` and a `thumbUrl` blur-up | A plain `<img>` of the thumbnail (§3.4) |
| [`frontend.md` §11](design/frontend.md#11-accessibility--ux) | WCAG AA contrast, `prefers-reduced-motion` respected, `focus-visible` rings | None of the three is met (§1.2, §1.5, §5) |

Dead CSS: the `.hero-*` classes in [`tailwind.css:57-74`](../frontend/src/styles/tailwind.css#L57-L74) are unused.

---

## 7. Suggested order

| Step | Contents | Size |
|---|---|---|
| 1 | The defects in §1; the color tokens and contrast fixes in §2.1–§2.4; focus and safe-area fixes from §5 | S–M |
| 2 | Trim the answer controls (§3.1) and the reading typography (§3.2). The biggest improvement for long editions. | M |
| 3 | Masthead, question chips, sticky prompts, progress bar (§3.3), and the lightbox and full-size inline images (§3.4) | M–L |
| 4 | Motion foundation (§4.2), then the moments in §4.3, starting with the publish celebration | M |
| 5 | Remaining flow fixes (§5), group-gradient identity (§2.5), dark mode (§2.4) | M |

Each step that changes behavior described in [`frontend.md`](design/frontend.md) updates that page in the same change, including the drift in §6.
