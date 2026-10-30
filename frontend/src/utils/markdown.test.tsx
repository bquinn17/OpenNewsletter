import { act, fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { MarkdownBody, referencedImageIds, stripImageTokens } from "./markdown";

const { refreshMediaUrls } = vi.hoisted(() => ({ refreshMediaUrls: vi.fn() }));

vi.mock("./media", async (importOriginal) => ({
  ...(await importOriginal<typeof import("./media")>()),
  refreshMediaUrls,
}));

describe("MarkdownBody sanitization", () => {
  it("strips script tags", () => {
    render(<MarkdownBody body={"before<script>alert(1)</script>after"} images={[]} />);
    expect(document.querySelector("script")).not.toBeInTheDocument();
    expect(screen.getByText(/before/)).toBeInTheDocument();
  });

  it("drops a javascript: link", () => {
    render(<MarkdownBody body="[click me](javascript:alert(1))" images={[]} />);
    const link = screen.queryByRole("link", { name: "click me" });
    expect(link).not.toBeInTheDocument();
  });

  it("opens an http link in a new tab with rel=noopener", () => {
    render(<MarkdownBody body="[site](https://example.com)" images={[]} />);
    const link = screen.getByRole("link", { name: "site" });
    expect(link).toHaveAttribute("target", "_blank");
    expect(link).toHaveAttribute("rel", "noopener noreferrer");
  });

  it("drops raw HTML such as an onerror handler entirely (no rehype-raw wired up)", () => {
    render(
      <MarkdownBody
        body={'before<img src="https://example.com/x.png" onerror="alert(1)" alt="x" />after'}
        images={[]}
      />,
    );
    expect(screen.queryByAltText("x")).not.toBeInTheDocument();
    expect(document.querySelector("[onerror]")).not.toBeInTheDocument();
  });

  it("renders a remote markdown image as a link, not an <img>", () => {
    render(<MarkdownBody body="![chart](https://example.com/x.png)" images={[]} />);
    expect(screen.queryByRole("img")).not.toBeInTheDocument();
    expect(screen.getByRole("link", { name: "chart" })).toHaveAttribute(
      "href",
      "https://example.com/x.png",
    );
  });

  it("resolves an image: token to the media image", () => {
    render(
      <MarkdownBody
        body="![a photo](image:img1)"
        images={[
          {
            imageId: "img1",
            displayUrl: "https://media.test.invalid/img1/display.webp",
            thumbUrl: "https://media.test.invalid/img1/thumb.webp",
          },
        ]}
      />,
    );
    const img = screen.getByAltText("a photo");
    expect(img).toHaveAttribute("src", expect.stringContaining("thumb.webp"));
  });

  it("keeps an inline image's retry state when re-rendered with fresh URLs", async () => {
    refreshMediaUrls.mockResolvedValue(undefined);
    const images = (signature: string) => [
      {
        imageId: "img1",
        displayUrl: `https://media.test.invalid/img1/display.webp?sig=${signature}`,
        thumbUrl: `https://media.test.invalid/img1/thumb.webp?sig=${signature}`,
      },
    ];
    const { rerender } = render(
      <MarkdownBody body="![a photo](image:img1)" images={images("a")} />,
    );

    fireEvent.error(screen.getByAltText("a photo"));
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    rerender(<MarkdownBody body="![a photo](image:img1)" images={images("b")} />);
    fireEvent.error(screen.getByAltText("a photo"));

    expect(screen.getByRole("img", { name: "Couldn't load image: a photo" })).toBeInTheDocument();
    expect(refreshMediaUrls).toHaveBeenCalledTimes(1);
  });

  it("drops an unknown image token", () => {
    render(<MarkdownBody body="![missing](image:unknown)" images={[]} />);
    expect(screen.queryByAltText("missing")).not.toBeInTheDocument();
    expect(screen.queryByRole("img")).not.toBeInTheDocument();
  });

  it("renders markdown headers rather than stripping them", () => {
    render(<MarkdownBody body={"# Big heading\n\n## Smaller heading"} images={[]} />);
    expect(screen.getByRole("heading", { level: 1, name: "Big heading" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 2, name: "Smaller heading" })).toBeInTheDocument();
  });
});

describe("MarkdownBody with allowImages=false (comments, `docs/design/engagement.md` §5)", () => {
  it("still strips script tags", () => {
    render(
      <MarkdownBody
        body={"before<script>alert(1)</script>after"}
        images={[]}
        allowImages={false}
      />,
    );
    expect(document.querySelector("script")).not.toBeInTheDocument();
    expect(screen.getByText(/before/)).toBeInTheDocument();
  });

  it("still neutralizes a javascript: link", () => {
    render(<MarkdownBody body="[click me](javascript:alert(1))" images={[]} allowImages={false} />);
    expect(screen.queryByRole("link", { name: "click me" })).not.toBeInTheDocument();
  });

  it("does not render an <img> for an image: token even with a matching image", () => {
    render(
      <MarkdownBody
        body="![a photo](image:img1)"
        images={[
          {
            imageId: "img1",
            displayUrl: "https://media.test.invalid/img1/display.webp",
            thumbUrl: "https://media.test.invalid/img1/thumb.webp",
          },
        ]}
        allowImages={false}
      />,
    );
    expect(screen.queryByRole("img")).not.toBeInTheDocument();
    expect(screen.queryByAltText("a photo")).not.toBeInTheDocument();
  });

  it("still renders a remote image as a link, not an <img>", () => {
    render(
      <MarkdownBody body="![chart](https://example.com/x.png)" images={[]} allowImages={false} />,
    );
    expect(screen.queryByRole("img")).not.toBeInTheDocument();
    expect(screen.getByRole("link", { name: "chart" })).toBeInTheDocument();
  });
});

describe("stripImageTokens", () => {
  it("removes image tokens and collapses whitespace", () => {
    expect(stripImageTokens("hello ![alt](image:abc)   world")).toBe("hello world");
  });

  it("leaves plain text untouched", () => {
    expect(stripImageTokens("just text")).toBe("just text");
  });
});

describe("referencedImageIds", () => {
  it("extracts every referenced id", () => {
    const ids = referencedImageIds("![a](image:one) text ![b](image:two)");
    expect(ids).toEqual(new Set(["one", "two"]));
  });

  it("returns an empty set when there are none", () => {
    expect(referencedImageIds("no images here").size).toBe(0);
  });
});
