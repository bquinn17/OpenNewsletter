import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";
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

describe("GroupEditions", () => {
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
});
