import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
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

describe("GroupSwitcher", () => {
  it("is fully keyboard operable: open, pick, and Escape back to the trigger", async () => {
    const user = userEvent.setup();
    const onPick = vi.fn();
    render(<GroupSwitcher memberships={memberships} currentGroupId="g1" onPick={onPick} />);

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
});
