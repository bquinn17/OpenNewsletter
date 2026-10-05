import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { MarkdownBody, referencedImageIds, stripImageTokens } from "./markdown";

vi.mock("./media", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./media")>();
  return { ...actual, ensureCookie: vi.fn().mockResolvedValue(undefined) };
});

describe("MarkdownBody sanitization", () => {
  it("strips script tags", () => {
    render(<MarkdownBody body={"before<script>alert(1)</script>after"} images={[]} groupId="g1" />);
    expect(document.querySelector("script")).not.toBeInTheDocument();
    expect(screen.getByText(/before/)).toBeInTheDocument();
  });

  it("drops a javascript: link", () => {
    render(<MarkdownBody body="[click me](javascript:alert(1))" images={[]} groupId="g1" />);
    const link = screen.queryByRole("link", { name: "click me" });
    expect(link).not.toBeInTheDocument();
  });

  it("opens an http link in a new tab with rel=noopener", () => {
    render(<MarkdownBody body="[site](https://example.com)" images={[]} groupId="g1" />);
    const link = screen.getByRole("link", { name: "site" });
    expect(link).toHaveAttribute("target", "_blank");
    expect(link).toHaveAttribute("rel", "noopener noreferrer");
  });

  it("drops raw HTML such as an onerror handler entirely (no rehype-raw wired up)", () => {
    render(
      <MarkdownBody
        body={'before<img src="https://example.com/x.png" onerror="alert(1)" alt="x" />after'}
        images={[]}
        groupId="g1"
      />,
    );
    expect(screen.queryByAltText("x")).not.toBeInTheDocument();
    expect(document.querySelector("[onerror]")).not.toBeInTheDocument();
  });

  it("renders a remote markdown image as a link, not an <img>", () => {
    render(<MarkdownBody body="![chart](https://example.com/x.png)" images={[]} groupId="g1" />);
    expect(screen.queryByRole("img")).not.toBeInTheDocument();
    expect(screen.getByRole("link", { name: "chart" })).toHaveAttribute(
      "href",
      "https://example.com/x.png",
    );
  });

  it("resolves an image: token to the CDN image", () => {
    render(
      <MarkdownBody
        body="![a photo](image:img1)"
        images={[
          {
            imageId: "img1",
            displayUrl: "https://cdn.test.invalid/img1/display.webp",
            thumbUrl: "https://cdn.test.invalid/img1/thumb.webp",
          },
        ]}
        groupId="g1"
      />,
    );
    const img = screen.getByAltText("a photo");
    expect(img).toHaveAttribute("src", expect.stringContaining("thumb.webp"));
  });

  it("drops an unknown image token", () => {
    render(<MarkdownBody body="![missing](image:unknown)" images={[]} groupId="g1" />);
    expect(screen.queryByAltText("missing")).not.toBeInTheDocument();
    expect(screen.queryByRole("img")).not.toBeInTheDocument();
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
