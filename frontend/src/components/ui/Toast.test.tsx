import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { useToasts } from "../../state/toast";
import { ToastStack } from "./Toast";

afterEach(() => {
  useToasts.setState({ toasts: [] });
});

describe("ToastStack", () => {
  it("announces toasts via an aria-live region and dismisses on click", () => {
    render(<ToastStack />);
    act(() => useToasts.getState().push("Saved", "success"));

    const region = screen.getByRole("status");
    expect(region).toHaveAttribute("aria-live", "polite");
    const toast = screen.getByText("Saved");
    expect(region).toContainElement(toast);

    fireEvent.click(toast);
    expect(screen.queryByText("Saved")).not.toBeInTheDocument();
  });

  it("dismisses on a swipe past the threshold", () => {
    render(<ToastStack />);
    act(() => useToasts.getState().push("Swipe me"));

    const toast = screen.getByText("Swipe me");
    fireEvent.pointerDown(toast, { clientX: 0 });
    fireEvent.pointerMove(toast, { clientX: -120 });
    fireEvent.pointerUp(toast);

    expect(screen.queryByText("Swipe me")).not.toBeInTheDocument();
  });

  it("snaps back on a short drag instead of dismissing", () => {
    render(<ToastStack />);
    act(() => useToasts.getState().push("Stay"));

    const toast = screen.getByText("Stay");
    fireEvent.pointerDown(toast, { clientX: 0 });
    fireEvent.pointerMove(toast, { clientX: -20 });
    fireEvent.pointerUp(toast);

    expect(screen.getByText("Stay")).toBeInTheDocument();
  });
});
