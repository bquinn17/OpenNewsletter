import { render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { GiphyEmbed } from "./GiphyEmbed";
import { useGiphyLimit } from "../../state/giphyLimit";

const { giphyLookupEntry } = vi.hoisted(() => ({
  giphyLookupEntry: vi.fn<() => { status: string; gif?: unknown }>(),
}));

vi.mock("../../giphy/GiphyLookupProvider", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../giphy/GiphyLookupProvider")>()),
  useGiphyGif: () => giphyLookupEntry(),
}));

describe("GiphyEmbed", () => {
  afterEach(() => {
    useGiphyLimit.getState().clear();
    giphyLookupEntry.mockReset();
  });

  it("shows a 4:3 placeholder card with the token's title and a disabled Load GIF button when rate-limited", () => {
    giphyLookupEntry.mockReturnValue({ status: "rate-limited" });
    useGiphyLimit.getState().markRateLimited();

    render(<GiphyEmbed id="abc123" alt="tent fail" variant="answer" />);

    expect(screen.getByText("tent fail")).toBeInTheDocument();
    expect(screen.getByText(/it'll load later/i)).toBeInTheDocument();
    const button = screen.getByRole("button", { name: /Load GIF/ });
    expect(button).toBeDisabled();
  });

  it("shows 'GIF unavailable' for an id GIPHY no longer returns", () => {
    giphyLookupEntry.mockReturnValue({ status: "ready", gif: null });

    render(<GiphyEmbed id="gone" alt="old gif" variant="answer" />);

    expect(screen.getByText("GIF unavailable")).toBeInTheDocument();
  });

  it("renders nothing for a GIF rated above pg-13", () => {
    giphyLookupEntry.mockReturnValue({
      status: "ready",
      gif: {
        id: "r1",
        title: "nope",
        rating: "r",
        width: 10,
        height: 10,
        videoUrl: "/x.mp4",
        stillUrl: "/x-still.gif",
      },
    });

    const { container } = render(<GiphyEmbed id="r1" alt="nope" variant="answer" />);

    expect(container).toBeEmptyDOMElement();
  });
});
