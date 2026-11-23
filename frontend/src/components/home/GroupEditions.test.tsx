import type { ReactNode } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, within } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "../../api/client";
import type { components } from "../../types/api";
import { GroupEditions } from "./GroupEditions";

vi.mock("../../api/client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../api/client")>();
  return { ...actual, api: { listCandidates: vi.fn() } };
});

type NewsletterSummary = components["schemas"]["NewsletterSummary"];
type CandidateListResponse = components["schemas"]["CandidateListResponse"];

function cycle(
  cycleId: string,
  status: NewsletterSummary["status"],
  month: string,
): NewsletterSummary {
  return {
    cycleId,
    status,
    responseOpenAt: `2026-${month}-01T12:00:00Z`,
    responseCloseAt: `2026-${month}-05T12:00:00Z`,
    publishedAt: status === "published" ? `2026-${month}-05T12:00:00Z` : null,
    questionCount: 4,
    myDraftCount: 0,
    myPublishedCount: 1,
  };
}

function candidateList(overrides: Partial<CandidateListResponse> = {}): CandidateListResponse {
  return {
    nextCycleId: "next",
    votesPerUserPerCycle: 3,
    myVoteCount: 0,
    items: [],
    nextCursor: null,
    ...overrides,
  };
}

function renderWithProviders(children: ReactNode) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter>{children}</MemoryRouter>
    </QueryClientProvider>,
  );
}

function renderEditions(
  items: NewsletterSummary[],
  frequency: components["schemas"]["CycleCadence"]["frequency"] = "monthly",
) {
  renderWithProviders(
    <GroupEditions
      groupId="g1"
      timezone="UTC"
      frequency={frequency}
      gradient="grape-sky"
      items={items}
    />,
  );
}

function sectionTitles() {
  return screen.getAllByRole("heading", { level: 2 }).map((h) => h.textContent);
}

function rowLabels(section: string) {
  return within(screen.getByRole("region", { name: section }))
    .getAllByRole("link")
    .map((a) => a.textContent?.match(/[A-Z][a-z]+ 2026/)?.[0]);
}

/** Weekly row titles are a publication date, e.g. "Nov 15, 2026". */
function weeklyRowLabels(section: string) {
  return within(screen.getByRole("region", { name: section }))
    .getAllByRole("link")
    .map((a) => a.textContent?.match(/[A-Z][a-z]+ \d{1,2}, 2026/)?.[0]);
}

describe("GroupEditions", () => {
  beforeEach(() => {
    vi.mocked(api.listCandidates).mockResolvedValue(candidateList());
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("orders newest first and groups voting, this month, and past after the window closes", () => {
    vi.useFakeTimers({ toFake: ["Date"] });
    vi.setSystemTime(new Date("2026-10-20T12:00:00Z"));
    renderEditions([
      cycle("202611", "voting", "11"),
      cycle("202610", "published", "10"),
      cycle("202609", "published", "09"),
      cycle("202608", "published", "08"),
      cycle("202607", "published", "07"),
      cycle("202606", "published", "06"),
    ]);

    expect(sectionTitles()).toEqual([
      "Taking suggestions",
      "This month's edition",
      "Past editions",
    ]);
    // Only the three most recent past editions, newest first.
    expect(rowLabels("Past editions")).toEqual(["September 2026", "August 2026", "July 2026"]);
    expect(rowLabels("This month's edition")).toEqual(["October 2026"]);
    expect(rowLabels("Taking suggestions")).toEqual(["November 2026"]);
  });

  it("puts last month's edition in the past while this month's is awaiting responses", () => {
    vi.useFakeTimers({ toFake: ["Date"] });
    vi.setSystemTime(new Date("2026-10-03T12:00:00Z"));
    renderEditions([
      cycle("202611", "voting", "11"),
      cycle("202610", "open", "10"),
      cycle("202609", "published", "09"),
    ]);

    expect(sectionTitles()).toEqual(["Taking suggestions", "Awaiting responses", "Past editions"]);
    expect(rowLabels("Past editions")).toEqual(["September 2026"]);
    expect(rowLabels("Awaiting responses")).toEqual(["October 2026"]);
  });

  it("shows both the open cycle and next month's voting cycle", () => {
    // Newest first, as the API returns them.
    const items = [
      cycle("202610", "voting", "10"),
      cycle("202609", "open", "09"),
      cycle("202608", "published", "08"),
    ];

    renderWithProviders(
      <GroupEditions
        groupId="g1"
        timezone="UTC"
        frequency="monthly"
        gradient="grape-sky"
        items={items}
      />,
    );

    expect(screen.getByRole("link", { name: /September 2026/ })).toHaveAttribute(
      "href",
      "/g/g1/n/202609",
    );
    expect(screen.getByRole("link", { name: /October 2026/ })).toHaveAttribute(
      "href",
      "/g/g1/upcoming",
    );
    expect(screen.getByRole("link", { name: /August 2026/ })).toBeInTheDocument();
  });

  it("counts a published edition's questions in the singular for one", () => {
    renderEditions([
      { ...cycle("202609", "published", "09"), questionCount: 1 },
      cycle("202608", "published", "08"),
    ]);

    expect(screen.getByRole("link", { name: /September 2026/ })).toHaveTextContent(
      /1 question · published/,
    );
    expect(screen.getByRole("link", { name: /August 2026/ })).toHaveTextContent(
      /4 questions · published/,
    );
  });

  it("labels each row with its phase", () => {
    const items = [
      cycle("202610", "voting", "10"),
      cycle("202609", "open", "09"),
      cycle("202608", "published", "08"),
    ];

    renderWithProviders(
      <GroupEditions
        groupId="g1"
        timezone="UTC"
        frequency="monthly"
        gradient="grape-sky"
        items={items}
      />,
    );

    expect(screen.getByRole("link", { name: /October 2026/ })).toHaveTextContent("Voting");
    expect(screen.getByRole("link", { name: /August 2026/ })).toHaveTextContent("Published");
  });

  it("shows the open edition as a hero card with progress and an 'answer more' CTA", () => {
    renderWithProviders(
      <GroupEditions
        groupId="g1"
        timezone="UTC"
        frequency="monthly"
        gradient="grape-sky"
        items={[{ ...cycle("202609", "open", "09"), questionCount: 4, myPublishedCount: 1 }]}
      />,
    );

    const hero = screen.getByRole("link", { name: /September 2026/ });
    expect(hero).toHaveTextContent("1 of 4 answered");
    expect(hero).toHaveTextContent("Answer 3 more");
    expect(hero).toHaveAttribute("href", "/g/g1/n/202609");
  });

  it("shows 'Open the edition' on the hero once every question is answered", () => {
    renderWithProviders(
      <GroupEditions
        groupId="g1"
        timezone="UTC"
        frequency="monthly"
        gradient="grape-sky"
        items={[{ ...cycle("202609", "open", "09"), questionCount: 4, myPublishedCount: 4 }]}
      />,
    );

    expect(screen.getByRole("link", { name: /September 2026/ })).toHaveTextContent(
      "Open the edition",
    );
  });

  it("shows votes left on the voting row once the shared candidates query loads", async () => {
    vi.mocked(api.listCandidates).mockResolvedValue(
      candidateList({ votesPerUserPerCycle: 3, myVoteCount: 1 }),
    );
    renderEditions([cycle("202610", "voting", "10")]);

    expect(
      await screen.findByText(/Suggest and vote on questions for next month · 2 votes left/),
    ).toBeInTheDocument();
  });

  it("weekly: labels editions by publication date and titles the current section 'This week's edition'", () => {
    vi.useFakeTimers({ toFake: ["Date"] });
    vi.setSystemTime(new Date("2026-11-20T12:00:00Z"));
    const items: NewsletterSummary[] = [
      {
        ...cycle("202611w3", "voting", "11"),
        responseOpenAt: "2026-11-22T12:00:00Z",
        responseCloseAt: "2026-11-29T12:00:00Z",
        publishedAt: null,
      },
      {
        ...cycle("202611w2", "published", "11"),
        responseOpenAt: "2026-11-15T12:00:00Z",
        responseCloseAt: "2026-11-15T12:00:00Z",
        publishedAt: "2026-11-15T12:00:00Z",
      },
      {
        ...cycle("202611w1", "published", "11"),
        responseOpenAt: "2026-11-08T12:00:00Z",
        responseCloseAt: "2026-11-08T12:00:00Z",
        publishedAt: "2026-11-08T12:00:00Z",
      },
    ];
    renderEditions(items, "weekly");

    expect(sectionTitles()).toEqual(["Taking suggestions", "This week's edition", "Past editions"]);
    expect(weeklyRowLabels("This week's edition")).toEqual(["Nov 15, 2026"]);
    expect(weeklyRowLabels("Past editions")).toEqual(["Nov 8, 2026"]);
  });

  it("weekly: the voting row invites next week's questions, and a published row drops the date suffix", () => {
    const items: NewsletterSummary[] = [
      {
        ...cycle("202611w1", "voting", "11"),
        responseOpenAt: "2026-11-22T12:00:00Z",
        responseCloseAt: "2026-11-29T12:00:00Z",
        publishedAt: null,
      },
      {
        ...cycle("202610w1", "published", "10"),
        responseOpenAt: "2026-10-01T12:00:00Z",
        responseCloseAt: "2026-10-08T12:00:00Z",
        publishedAt: "2026-10-08T12:00:00Z",
      },
    ];
    renderEditions(items, "weekly");

    expect(screen.getByRole("link", { name: /Suggest and vote/ })).toHaveTextContent(
      "Suggest and vote on questions for next week",
    );
    const publishedLink = screen.getByRole("link", { name: /Oct 8, 2026/ });
    expect(publishedLink).toHaveTextContent(/4 questions$/);
    expect(publishedLink).not.toHaveTextContent("published");
  });
});
