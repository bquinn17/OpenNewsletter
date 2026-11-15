import { act, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useCelebration } from "../../state/celebration";
import { ConfettiBurst } from "./ConfettiBurst";

afterEach(() => {
  vi.useRealTimers();
  useCelebration.setState({ burst: 0 });
});

describe("ConfettiBurst", () => {
  it("shows nothing until something celebrates, then clears itself", () => {
    vi.useFakeTimers();
    const { container } = render(<ConfettiBurst />);
    expect(container).toBeEmptyDOMElement();

    act(() => useCelebration.getState().celebrate());
    expect(container.querySelectorAll(".animate-confetti").length).toBeGreaterThan(0);

    act(() => vi.advanceTimersByTime(5000));
    expect(container).toBeEmptyDOMElement();
  });
});
