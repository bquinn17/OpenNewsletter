import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";
import { GroupSwitcher } from "./GroupSwitcher";

const memberships = [
  {
    groupId: "g1",
    role: "admin" as const,
    groupName: "Trail Crew",
    timezone: "UTC",
    gradient: "grape-sky" as const,
  },
  {
    groupId: "g2",
    role: "member" as const,
    groupName: "Game Night",
    timezone: "UTC",
    gradient: "ocean-dusk" as const,
  },
];

function renderSwitcher(list: typeof memberships, onPick = vi.fn()) {
  render(
    <MemoryRouter>
      <GroupSwitcher memberships={list} currentGroupId="g1" onPick={onPick} />
    </MemoryRouter>,
  );
}

describe("GroupSwitcher", () => {
  it("is fully keyboard operable: open, pick, and Escape back to the trigger", async () => {
    const user = userEvent.setup();
    const onPick = vi.fn();
    renderSwitcher(memberships, onPick);

    const trigger = screen.getByRole("button", { name: /Trail Crew/ });
    await user.click(trigger);
    expect(screen.getAllByRole("menuitem")[0]).toHaveFocus();

    await user.keyboard("{Escape}");
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
    expect(trigger).toHaveFocus();

    await user.click(trigger);
    await user.click(screen.getByRole("menuitem", { name: /Game Night/ }));
    expect(onPick).toHaveBeenCalledWith("g2");
  });

  it("ends the menu with a link to join another group, even with only one group", async () => {
    const user = userEvent.setup();
    renderSwitcher(memberships.slice(0, 1));

    await user.click(screen.getByRole("button", { name: /Trail Crew/ }));
    const items = screen.getAllByRole("menuitem");
    expect(items).toHaveLength(2);
    expect(items.at(-1)).toHaveAccessibleName("Join with an invite code");
    expect(items.at(-1)).toHaveAttribute("href", "/join");
  });
});
