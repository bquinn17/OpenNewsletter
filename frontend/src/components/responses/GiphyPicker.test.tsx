import { createRef } from "react";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { GiphyPicker } from "./GiphyPicker";
import { GiphyRateLimitedError } from "../../giphy/giphyClient";
import { useGiphyLimit } from "../../state/giphyLimit";

const { searchGifs } = vi.hoisted(() => ({ searchGifs: vi.fn() }));

vi.mock("../../giphy/giphyClient", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../giphy/giphyClient")>();
  return { ...actual, searchGifs };
});

function renderPicker(onPick = vi.fn(), onClose = vi.fn()) {
  const anchorRef = createRef<HTMLElement>();
  return {
    ...render(<GiphyPicker anchorRef={anchorRef} onPick={onPick} onClose={onClose} />),
    onPick,
    onClose,
  };
}

describe("GiphyPicker rate limiting (`docs/design/frontend.md` §18.4)", () => {
  afterEach(() => {
    useGiphyLimit.getState().clear();
    searchGifs.mockReset();
    vi.useRealTimers();
  });

  it("shows the banner and disables the controls after a 429 from search", async () => {
    // `giphyClient`'s real `searchGifs` marks the shared store before throwing; the mock mirrors
    // that contract rather than re-testing `giphyClient` itself (covered in `giphyLimit.test.ts`).
    searchGifs.mockImplementation(() => {
      useGiphyLimit.getState().markRateLimited();
      return Promise.reject(new GiphyRateLimitedError());
    });
    renderPicker();

    fireEvent.change(screen.getByPlaceholderText("Search GIPHY"), { target: { value: "lol" } });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Search GIPHY" }));
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(screen.getByText("GIF limit reached.")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("Search GIPHY")).toBeDisabled();
    expect(screen.getByRole("button", { name: "Search GIPHY" })).toBeDisabled();
  });

  it("a 429 from the display side (the shared store) blocks search too", () => {
    useGiphyLimit.getState().markRateLimited();
    renderPicker();

    expect(screen.getByText("GIF limit reached.")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("Search GIPHY")).toBeDisabled();
    expect(searchGifs).not.toHaveBeenCalled();
  });

  it("the countdown re-enables the controls on its own, without sending a request", () => {
    vi.useFakeTimers();
    useGiphyLimit.getState().markRateLimited();
    renderPicker();
    expect(screen.getByText("GIF limit reached.")).toBeInTheDocument();

    act(() => {
      vi.advanceTimersByTime(10 * 60_000 + 1000);
    });

    expect(screen.queryByText("GIF limit reached.")).not.toBeInTheDocument();
    expect(screen.getByPlaceholderText("Search GIPHY")).not.toBeDisabled();
    expect(searchGifs).not.toHaveBeenCalled();
  });
});
