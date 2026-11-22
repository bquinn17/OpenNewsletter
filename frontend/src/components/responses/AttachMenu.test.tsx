import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { AttachMenu, type AttachMenuItem } from "./AttachMenu";

function items(overrides: Partial<AttachMenuItem>[] = []): AttachMenuItem[] {
  const base: AttachMenuItem[] = [
    {
      key: "photo",
      icon: "📷",
      label: "Photo",
      subtitle: "3/10 · up to 15 MB each",
      onSelect: vi.fn(),
    },
    { key: "gif", icon: "GIF", label: "GIF", subtitle: "Search GIPHY", onSelect: vi.fn() },
  ];
  return base.map((item, i) => ({ ...item, ...overrides[i] }));
}

describe("AttachMenu", () => {
  it("renders nothing when there are no items", () => {
    render(<AttachMenu items={[]} ariaLabel="Add" />);
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });

  it("shows only the items it's given", () => {
    render(
      <AttachMenu
        items={[{ key: "photo", icon: "📷", label: "Photo", onSelect: vi.fn() }]}
        ariaLabel="Add a photo"
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Add a photo" }));

    expect(screen.getByRole("menuitem", { name: /Photo/ })).toBeInTheDocument();
    expect(screen.queryByRole("menuitem", { name: /GIF/ })).not.toBeInTheDocument();
  });

  it("opens the menu, with the correct subtitle, on the + button", () => {
    render(<AttachMenu items={items()} ariaLabel="Add a photo or GIF" />);

    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Add a photo or GIF" }));

    expect(screen.getByRole("menu")).toBeInTheDocument();
    expect(screen.getByText("3/10 · up to 15 MB each")).toBeInTheDocument();
    expect(screen.getByText("Search GIPHY")).toBeInTheDocument();
  });

  it("calls the item's onSelect and closes the menu when picked", () => {
    const onSelect = vi.fn();
    render(<AttachMenu items={items([{}, { onSelect }])} ariaLabel="Add a photo or GIF" />);
    fireEvent.click(screen.getByRole("button", { name: "Add a photo or GIF" }));
    fireEvent.click(screen.getByRole("menuitem", { name: /GIF/ }));

    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });

  it("shows a disabled item at its limit with the limit as its subtitle, and never calls onSelect", () => {
    const onSelect = vi.fn();
    render(
      <AttachMenu
        items={items([{ disabled: true, subtitle: "10/10 photos", onSelect }])}
        ariaLabel="Add a photo or GIF"
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Add a photo or GIF" }));

    const photoItem = screen.getByRole("menuitem", { name: /Photo/ });
    expect(photoItem).toBeDisabled();
    expect(screen.getByText("10/10 photos")).toBeInTheDocument();

    fireEvent.click(photoItem);
    expect(onSelect).not.toHaveBeenCalled();
  });

  it("moves focus to the first item on open, then down/up/Home/End across items, skipping disabled ones", () => {
    render(<AttachMenu items={items([{ disabled: true }])} ariaLabel="Add a photo or GIF" />);
    fireEvent.click(screen.getByRole("button", { name: "Add a photo or GIF" }));

    const gif = screen.getByRole("menuitem", { name: /GIF/ });
    // Photo is disabled, so it's skipped — GIF is the only enabled item and gets focus.
    expect(gif).toHaveFocus();

    fireEvent.keyDown(screen.getByRole("menu"), { key: "ArrowDown" });
    expect(gif).toHaveFocus();
    fireEvent.keyDown(screen.getByRole("menu"), { key: "ArrowUp" });
    expect(gif).toHaveFocus();
    fireEvent.keyDown(screen.getByRole("menu"), { key: "Home" });
    expect(gif).toHaveFocus();
    fireEvent.keyDown(screen.getByRole("menu"), { key: "End" });
    expect(gif).toHaveFocus();
  });

  it("navigates with the arrow keys across two enabled items", () => {
    render(<AttachMenu items={items()} ariaLabel="Add a photo or GIF" />);
    fireEvent.click(screen.getByRole("button", { name: "Add a photo or GIF" }));

    const photo = screen.getByRole("menuitem", { name: /Photo/ });
    const gif = screen.getByRole("menuitem", { name: /GIF/ });
    expect(photo).toHaveFocus();

    fireEvent.keyDown(screen.getByRole("menu"), { key: "ArrowDown" });
    expect(gif).toHaveFocus();
    fireEvent.keyDown(screen.getByRole("menu"), { key: "ArrowDown" });
    expect(photo).toHaveFocus();
    fireEvent.keyDown(screen.getByRole("menu"), { key: "ArrowUp" });
    expect(gif).toHaveFocus();
  });

  it("closes on Escape and returns focus to the + button", () => {
    render(<AttachMenu items={items()} ariaLabel="Add a photo or GIF" />);
    const trigger = screen.getByRole("button", { name: "Add a photo or GIF" });
    fireEvent.click(trigger);
    expect(screen.getByRole("menu")).toBeInTheDocument();

    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
    expect(trigger).toHaveFocus();
  });
});
