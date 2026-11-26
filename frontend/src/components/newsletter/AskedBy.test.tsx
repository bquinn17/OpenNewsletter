import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { AskedBy } from "./AskedBy";

const sam = { userId: "u1", displayName: "Sam", avatarColor: "teal" as const, avatarUrl: null };

describe("AskedBy", () => {
  it("names the asker, or says the question was asked anonymously", () => {
    const { rerender } = render(<AskedBy askedBy={sam} isAnonymous={false} />);
    expect(screen.getByText("Sam asked")).toBeInTheDocument();
    rerender(<AskedBy askedBy={null} isAnonymous />);
    expect(screen.getByText("Asked anonymously")).toBeInTheDocument();
  });

  it("labels the standing question, which nobody asked", () => {
    render(<AskedBy askedBy={null} isAnonymous={false} isStanding />);
    expect(screen.getByText("Every edition")).toBeInTheDocument();
  });
});
