import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useCelebration } from "../../state/celebration";
import { CompletionCelebration } from "./CompletionCelebration";

afterEach(() => {
  localStorage.clear();
  useCelebration.setState({ burst: 0 });
  vi.restoreAllMocks();
});

const closeAt = "2026-06-08T03:59:00Z";

describe("CompletionCelebration", () => {
  it("shows the dialog once every question of the edition is answered", () => {
    render(
      <CompletionCelebration
        groupId="g1"
        cycleId="c1"
        allAnswered={true}
        responseCloseAt={closeAt}
        timezone="UTC"
      />,
    );

    const dialog = screen.getByRole("dialog", { name: "You completed the newsletter!" });
    expect(dialog).toBeInTheDocument();
    expect(
      screen.getByText(/You can still edit any answer until the deadline/),
    ).toBeInTheDocument();
  });

  it("moves focus to the dialog when it opens", async () => {
    render(
      <CompletionCelebration
        groupId="g1"
        cycleId="c1"
        allAnswered={true}
        responseCloseAt={closeAt}
        timezone="UTC"
      />,
    );

    await act(() => new Promise((resolve) => setTimeout(resolve, 0)));
    expect(screen.getByRole("dialog")).toHaveFocus();
  });

  it("does not show when something is still unanswered", () => {
    render(
      <CompletionCelebration
        groupId="g1"
        cycleId="c1"
        allAnswered={false}
        responseCloseAt={closeAt}
        timezone="UTC"
      />,
    );

    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("dismisses on Escape", () => {
    render(
      <CompletionCelebration
        groupId="g1"
        cycleId="c1"
        allAnswered={true}
        responseCloseAt={closeAt}
        timezone="UTC"
      />,
    );

    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("dismisses on the button click", () => {
    render(
      <CompletionCelebration
        groupId="g1"
        cycleId="c1"
        allAnswered={true}
        responseCloseAt={closeAt}
        timezone="UTC"
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Nice!" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("does not show a second time once the flag is set, even across remounts", () => {
    const { unmount } = render(
      <CompletionCelebration
        groupId="g1"
        cycleId="c1"
        allAnswered={true}
        responseCloseAt={closeAt}
        timezone="UTC"
      />,
    );
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    unmount();

    render(
      <CompletionCelebration
        groupId="g1"
        cycleId="c1"
        allAnswered={true}
        responseCloseAt={closeAt}
        timezone="UTC"
      />,
    );
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("triggers the shared confetti burst when it shows", () => {
    render(
      <CompletionCelebration
        groupId="g1"
        cycleId="c1"
        allAnswered={true}
        responseCloseAt={closeAt}
        timezone="UTC"
      />,
    );

    expect(useCelebration.getState().burst).toBe(1);
  });

  it("still shows, without crashing, when localStorage throws", () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("blocked");
    });
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("blocked");
    });

    expect(() =>
      render(
        <CompletionCelebration
          groupId="g1"
          cycleId="c1"
          allAnswered={true}
          responseCloseAt={closeAt}
          timezone="UTC"
        />,
      ),
    ).not.toThrow();

    expect(screen.getByRole("dialog")).toBeInTheDocument();
  });
});
