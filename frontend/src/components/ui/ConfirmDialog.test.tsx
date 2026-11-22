import { render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ConfirmDialog } from "./ConfirmDialog";

describe("ConfirmDialog", () => {
  it("focuses Confirm by default", async () => {
    render(
      <ConfirmDialog
        open
        title="Leave the group?"
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
        confirmLabel="Leave"
      />,
    );

    await waitFor(() => expect(screen.getByRole("button", { name: "Leave" })).toHaveFocus());
  });

  it("focuses Cancel instead of Confirm when tone is danger", async () => {
    render(
      <ConfirmDialog
        open
        title="Delete this comment?"
        tone="danger"
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
        confirmLabel="Delete"
      />,
    );

    await waitFor(() => expect(screen.getByRole("button", { name: "Cancel" })).toHaveFocus());
    expect(screen.getByRole("button", { name: "Delete" })).not.toHaveFocus();
  });
});
