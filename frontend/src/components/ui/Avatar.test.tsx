import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { Avatar } from "./Avatar";

const { refreshMediaUrls } = vi.hoisted(() => ({ refreshMediaUrls: vi.fn() }));

vi.mock("../../utils/media", () => ({ refreshMediaUrls }));

/** Flushes the microtasks `handleError`'s refresh-then-remount chain schedules, inside `act`. */
async function flush() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
}

describe("Avatar", () => {
  beforeEach(() => {
    refreshMediaUrls.mockReset().mockResolvedValue(undefined);
  });

  it("renders initials with no url", () => {
    render(<Avatar name="Quinn Baker" url={null} />);

    expect(screen.queryByRole("img")).not.toBeInTheDocument();
    expect(screen.getByText("Q")).toBeInTheDocument();
  });

  it("renders the image when a url is given", () => {
    render(<Avatar name="Quinn Baker" url="https://media.test.invalid/avatar/a1/display.webp" />);

    const img = screen.getByRole("img", { name: "Quinn Baker" });
    expect(img).toHaveAttribute("src", "https://media.test.invalid/avatar/a1/display.webp");
    expect(img).toHaveAttribute("crossorigin", "anonymous");
  });

  it("retries once via refreshMediaUrls on error, then falls back to initials on a second error", async () => {
    render(<Avatar name="Quinn Baker" url="https://media.test.invalid/avatar/a1/display.webp" />);

    fireEvent.error(screen.getByRole("img", { name: "Quinn Baker" }));
    await flush();

    expect(refreshMediaUrls).toHaveBeenCalledTimes(1);
    // Still an <img> after the first error — one retry happens before giving up.
    expect(screen.getByRole("img", { name: "Quinn Baker" })).toBeInTheDocument();

    fireEvent.error(screen.getByRole("img", { name: "Quinn Baker" }));
    await flush();

    expect(screen.queryByRole("img")).not.toBeInTheDocument();
    expect(screen.getByText("Q")).toBeInTheDocument();
    expect(refreshMediaUrls).toHaveBeenCalledTimes(1);
  });
});
