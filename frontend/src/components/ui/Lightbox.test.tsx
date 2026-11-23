import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { Lightbox } from "./Lightbox";

const images = [
  { src: "https://media.test.invalid/a.webp", alt: "A ridge walk" },
  { src: "https://media.test.invalid/b.webp", alt: "A summit" },
];

describe("Lightbox", () => {
  it("opens focused on the close button, labelled by position, showing the alt text", () => {
    render(<Lightbox images={images} index={0} onClose={vi.fn()} onIndexChange={vi.fn()} />);

    expect(screen.getByRole("dialog", { name: "Image 1 of 2" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Close" })).toHaveFocus();
    expect(screen.getByText("A ridge walk")).toBeInTheDocument();
  });

  it("closes on Escape and restores focus to the element that opened it", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    const opener = document.createElement("button");
    opener.textContent = "open";
    document.body.appendChild(opener);
    opener.focus();

    render(<Lightbox images={images} index={0} onClose={onClose} onIndexChange={vi.fn()} />);
    await user.keyboard("{Escape}");

    expect(onClose).toHaveBeenCalledTimes(1);
    opener.remove();
  });

  it("restores focus on unmount", () => {
    const opener = document.createElement("button");
    document.body.appendChild(opener);
    opener.focus();

    const { unmount } = render(
      <Lightbox images={images} index={0} onClose={vi.fn()} onIndexChange={vi.fn()} />,
    );
    unmount();

    expect(opener).toHaveFocus();
    opener.remove();
  });

  it("steps between images with the arrow keys, stopping at the ends", async () => {
    const user = userEvent.setup();
    const onIndexChange = vi.fn();
    render(<Lightbox images={images} index={0} onClose={vi.fn()} onIndexChange={onIndexChange} />);

    await user.keyboard("{ArrowLeft}");
    expect(onIndexChange).not.toHaveBeenCalled();

    await user.keyboard("{ArrowRight}");
    expect(onIndexChange).toHaveBeenCalledWith(1);
  });

  it("hides the previous/next buttons at the ends of the set", () => {
    render(<Lightbox images={images} index={0} onClose={vi.fn()} onIndexChange={vi.fn()} />);
    expect(screen.queryByRole("button", { name: "Previous image" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Next image" })).toBeInTheDocument();
  });
});
