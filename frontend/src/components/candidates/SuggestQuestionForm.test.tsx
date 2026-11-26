import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { api } from "../../api/client";
import { SuggestQuestionForm } from "./SuggestQuestionForm";

vi.mock("../../api/client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../api/client")>();
  return { ...actual, api: { createCandidate: vi.fn() } };
});

function renderForm(onSuccess = vi.fn()) {
  const qc = new QueryClient({ defaultOptions: { mutations: { retry: false } } });
  render(
    <QueryClientProvider client={qc}>
      <SuggestQuestionForm groupId="g1" onSuccess={onSuccess} />
    </QueryClientProvider>,
  );
  return onSuccess;
}

describe("SuggestQuestionForm", () => {
  it("puts the name switch last, before Submit, and on by default", () => {
    renderForm();
    const nameSwitch = screen.getByRole("switch", { name: "Show my name" });
    expect(nameSwitch).toBeChecked();
    expect(screen.getByText("Asked by you")).toBeInTheDocument();
    expect(screen.getByText("Enabled")).toBeInTheDocument();

    const prompt = screen.getByLabelText("Your question");
    const submit = screen.getByRole("button", { name: "Submit" });
    // DOCUMENT_POSITION_FOLLOWING: the switch comes after the prompt and before Submit.
    expect(
      prompt.compareDocumentPosition(nameSwitch) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(
      nameSwitch.compareDocumentPosition(submit) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
  });

  it("submits with the name attached by default", async () => {
    const user = userEvent.setup();
    vi.mocked(api.createCandidate).mockResolvedValue({} as never);
    const onSuccess = renderForm();

    await user.type(screen.getByLabelText("Your question"), "Best hike this year?");
    await user.click(screen.getByRole("button", { name: "Submit" }));

    await waitFor(() => expect(onSuccess).toHaveBeenCalledWith(false));
    expect(api.createCandidate).toHaveBeenCalledWith(
      "g1",
      expect.objectContaining({ isAnonymous: false }),
    );
  });

  it("asks anonymously once the switch is off", async () => {
    const user = userEvent.setup();
    vi.mocked(api.createCandidate).mockResolvedValue({} as never);
    const onSuccess = renderForm();

    await user.click(screen.getByRole("switch", { name: "Show my name" }));
    expect(screen.getByText("Anonymous to the group")).toBeInTheDocument();
    expect(screen.getByText("Disabled")).toBeInTheDocument();

    await user.type(screen.getByLabelText("Your question"), "Best hike this year?");
    await user.click(screen.getByRole("button", { name: "Submit anonymously" }));

    await waitFor(() => expect(onSuccess).toHaveBeenCalledWith(true));
    expect(api.createCandidate).toHaveBeenCalledWith(
      "g1",
      expect.objectContaining({ isAnonymous: true }),
    );
  });
});
