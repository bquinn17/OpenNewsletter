import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { MediaImage } from "./MediaImage";

const { refreshMediaUrls } = vi.hoisted(() => ({ refreshMediaUrls: vi.fn() }));

vi.mock("../../utils/media", () => ({ refreshMediaUrls }));

/** Flushes the microtasks `handleError`'s refresh-then-remount chain schedules, inside `act`. */
async function flush() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
}

describe("MediaImage", () => {
  beforeEach(() => {
    refreshMediaUrls.mockReset().mockResolvedValue(undefined);
  });

  it("refreshes the media queries on the first error, then remounts", async () => {
    render(<MediaImage src="https://media.test.invalid/img/g1/thumb.webp" alt="a photo" />);

    fireEvent.error(screen.getByAltText("a photo"));
    await flush();

    expect(refreshMediaUrls).toHaveBeenCalledTimes(1);
    expect(screen.getByAltText("a photo")).toBeInTheDocument();
  });

  it("shows a placeholder after a second error for the same path", async () => {
    render(<MediaImage src="https://media.test.invalid/img/g1/thumb.webp" alt="a photo" />);

    fireEvent.error(screen.getByAltText("a photo"));
    await flush();
    fireEvent.error(screen.getByAltText("a photo"));
    await flush();

    expect(screen.queryByAltText("a photo")).not.toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Couldn't load image: a photo" })).toBeInTheDocument();
    // Only one retry ever happens for this path.
    expect(refreshMediaUrls).toHaveBeenCalledTimes(1);
  });

  it("gives the placeholder the image's width and height, so it keeps the same box", async () => {
    render(
      <MediaImage
        src="https://media.test.invalid/img/g1/thumb.webp"
        alt="a photo"
        width={400}
        height={300}
      />,
    );

    fireEvent.error(screen.getByAltText("a photo"));
    await flush();
    fireEvent.error(screen.getByAltText("a photo"));
    await flush();

    const placeholder = screen.getByRole("img", { name: "Couldn't load image: a photo" });
    expect(placeholder).toHaveAttribute("width", "400");
    expect(placeholder).toHaveAttribute("height", "300");
  });

  it("resets retry state when the image path changes", async () => {
    const { rerender } = render(
      <MediaImage src="https://media.test.invalid/img/g1/thumb.webp" alt="a photo" />,
    );
    fireEvent.error(screen.getByAltText("a photo"));
    await flush();
    fireEvent.error(screen.getByAltText("a photo"));
    await flush();
    expect(screen.queryByAltText("a photo")).not.toBeInTheDocument();

    rerender(<MediaImage src="https://media.test.invalid/img/g2/thumb.webp" alt="a photo" />);

    expect(screen.getByAltText("a photo")).toBeInTheDocument();
  });

  it("loads a re-signed URL for the same path once more after giving up, without a refresh", async () => {
    const { rerender } = render(
      <MediaImage src="https://media.test.invalid/img/g1/thumb.webp?X-Amz-Date=1" alt="a photo" />,
    );
    fireEvent.error(screen.getByAltText("a photo"));
    await flush();
    fireEvent.error(screen.getByAltText("a photo"));
    await flush();
    expect(screen.queryByAltText("a photo")).not.toBeInTheDocument();

    rerender(
      <MediaImage src="https://media.test.invalid/img/g1/thumb.webp?X-Amz-Date=2" alt="a photo" />,
    );
    expect(screen.getByAltText("a photo")).toBeInTheDocument();

    fireEvent.error(screen.getByAltText("a photo"));
    await flush();
    expect(screen.queryByAltText("a photo")).not.toBeInTheDocument();
    expect(refreshMediaUrls).toHaveBeenCalledTimes(1);
  });

  it("starts over when the browser comes back online", async () => {
    render(<MediaImage src="https://media.test.invalid/img/g1/thumb.webp" alt="a photo" />);
    fireEvent.error(screen.getByAltText("a photo"));
    await flush();
    fireEvent.error(screen.getByAltText("a photo"));
    await flush();
    expect(screen.queryByAltText("a photo")).not.toBeInTheDocument();

    act(() => {
      window.dispatchEvent(new Event("online"));
    });
    expect(screen.getByAltText("a photo")).toBeInTheDocument();

    // A full retry again: the first error refreshes.
    fireEvent.error(screen.getByAltText("a photo"));
    await flush();
    expect(refreshMediaUrls).toHaveBeenCalledTimes(2);
    expect(screen.getByAltText("a photo")).toBeInTheDocument();
  });

  it("handles a rejected refresh without an unhandled rejection", async () => {
    refreshMediaUrls.mockRejectedValue(new Error("offline"));
    render(<MediaImage src="https://media.test.invalid/img/g1/thumb.webp" alt="a photo" />);

    fireEvent.error(screen.getByAltText("a photo"));
    await flush();

    expect(refreshMediaUrls).toHaveBeenCalledTimes(1);
    expect(screen.getByAltText("a photo")).toBeInTheDocument();
  });

  it("passes width, height and the given className through to the <img>", () => {
    render(
      <MediaImage
        src="https://media.test.invalid/img/g1/thumb.webp"
        alt="a photo"
        width={800}
        height={600}
        className="rounded-xl"
      />,
    );

    const img = screen.getByAltText("a photo");
    expect(img).toHaveAttribute("width", "800");
    expect(img).toHaveAttribute("height", "600");
    expect(img).toHaveAttribute("class", "rounded-xl");
    expect(img).toHaveAttribute("crossorigin", "anonymous");
    expect(img).toHaveAttribute("loading", "lazy");
    expect(img).toHaveAttribute("decoding", "async");
  });
});
