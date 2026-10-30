import { render, screen, within } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { components } from "../../types/api";
import { GroupEditions } from "./GroupEditions";

type NewsletterSummary = components["schemas"]["NewsletterSummary"];

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

function renderEditions(items: NewsletterSummary[]) {
  render(
    <MemoryRouter>
      <GroupEditions groupId="g1" timezone="UTC" items={items} />
    </MemoryRouter>,
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

describe("GroupEditions", () => {
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

    render(
      <MemoryRouter>
        <GroupEditions groupId="g1" timezone="UTC" items={items} />
      </MemoryRouter>,
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

    render(
      <MemoryRouter>
        <GroupEditions groupId="g1" timezone="UTC" items={items} />
      </MemoryRouter>,
    );

    expect(screen.getByRole("link", { name: /October 2026/ })).toHaveTextContent("Voting");
    expect(screen.getByRole("link", { name: /September 2026/ })).toHaveTextContent("Open");
    expect(screen.getByRole("link", { name: /August 2026/ })).toHaveTextContent("Published");
  });
});
