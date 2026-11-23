import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { AllInOverlay } from "./AllInOverlay";

afterEach(() => {
  vi.useRealTimers();
});

describe("AllInOverlay", () => {
  it("renders nothing when closed", () => {
    const { container } = render(
      <AllInOverlay open={false} editionLabel="October 2026" onDismiss={vi.fn()} />,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("shows the edition label when loaded, and skips it rather than guess when it isn't", () => {
    const { rerender } = render(
      <AllInOverlay open editionLabel="October 2026" onDismiss={vi.fn()} />,
    );
    expect(screen.getByRole("status")).toHaveTextContent("You're all in for October 2026 🎉");

    rerender(<AllInOverlay open editionLabel={undefined} onDismiss={vi.fn()} />);
    expect(screen.getByRole("status")).toHaveTextContent("You're all in 🎉");
  });

  it("dismisses on its own button and automatically after ~3s", () => {
    vi.useFakeTimers();
    const onDismiss = vi.fn();
    render(<AllInOverlay open editionLabel="October 2026" onDismiss={onDismiss} />);

    fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(onDismiss).toHaveBeenCalledTimes(1);

    act(() => vi.advanceTimersByTime(3000));
    expect(onDismiss).toHaveBeenCalledTimes(2);
  });
});
