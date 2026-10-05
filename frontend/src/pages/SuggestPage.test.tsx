import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import { api, ApiError } from "../api/client";
import { SuggestPage } from "./SuggestPage";

vi.mock("../api/client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../api/client")>();
  return { ...actual, api: { createCandidate: vi.fn() } };
});

function renderPage() {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={["/g/g_test/upcoming/suggest"]}>
        <Routes>
          <Route path="/g/:groupId/upcoming/suggest" element={<SuggestPage />} />
          <Route path="/g/:groupId/upcoming" element={<div>Upcoming page</div>} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

afterEach(() => vi.clearAllMocks());

describe("SuggestPage", () => {
  it("disables submit for a too-short prompt and enables it once valid", async () => {
    const user = userEvent.setup();
    renderPage();

    const submit = screen.getByRole("button", { name: "Submit" });
    expect(submit).toBeDisabled();

    await user.type(screen.getByLabelText("Your question"), "hi");
    expect(screen.getByText(/at least 5 characters/)).toBeInTheDocument();
    expect(submit).toBeDisabled();

    await user.clear(screen.getByLabelText("Your question"));
    await user.type(screen.getByLabelText("Your question"), "What's your favorite hike?");
    await waitFor(() => expect(submit).toBeEnabled());
  });

  it("submits a text question with the expected payload", async () => {
    const user = userEvent.setup();
    vi.mocked(api.createCandidate).mockResolvedValue({
      questionId: "cq_new",
      kind: "text",
      prompt: "What's your favorite hike?",
      askedBy: { userId: "u1", displayName: "Quinn", avatarColor: "teal", avatarUrl: null },
      isAnonymous: false,
      voteCount: 0,
      votedByMe: false,
      submittedAt: "2026-10-01T00:00:00Z",
    });

    renderPage();
    await user.type(screen.getByLabelText("Your question"), "What's your favorite hike?");
    await user.click(screen.getByRole("button", { name: "Submit" }));

    await waitFor(() =>
      expect(api.createCandidate).toHaveBeenCalledWith("g_test", {
        kind: "text",
        prompt: "What's your favorite hike?",
        isAnonymous: false,
      }),
    );
    expect(await screen.findByText("Upcoming page")).toBeInTheDocument();
  });

  it("submits a poll with pollOptions and omits them for text", async () => {
    const user = userEvent.setup();
    vi.mocked(api.createCandidate).mockResolvedValue({
      questionId: "cq_poll",
      kind: "poll",
      prompt: "Which trail next?",
      pollOptions: [],
      askedBy: null,
      isAnonymous: false,
      voteCount: 0,
      votedByMe: false,
      submittedAt: "2026-10-01T00:00:00Z",
    });

    renderPage();
    await user.type(screen.getByLabelText("Your question"), "Which trail next?");
    await user.click(screen.getByRole("button", { name: /Poll/ }));
    await user.type(screen.getByPlaceholderText("Option 1"), "Mt Si");
    await user.type(screen.getByPlaceholderText("Option 2"), "Lake 22");
    await user.click(screen.getByRole("button", { name: "Submit" }));

    await waitFor(() =>
      expect(api.createCandidate).toHaveBeenCalledWith("g_test", {
        kind: "poll",
        prompt: "Which trail next?",
        isAnonymous: false,
        pollOptions: [{ label: "Mt Si" }, { label: "Lake 22" }],
      }),
    );
  });

  it("rejects duplicate poll option labels before submitting", async () => {
    const user = userEvent.setup();
    renderPage();

    await user.type(screen.getByLabelText("Your question"), "Which trail next?");
    await user.click(screen.getByRole("button", { name: /Poll/ }));
    await user.type(screen.getByPlaceholderText("Option 1"), "Mt Si");
    await user.type(screen.getByPlaceholderText("Option 2"), "Mt Si");

    expect(await screen.findByText(/already an option/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Submit" })).toBeDisabled();
  });

  it("enforces the 2-6 poll option bounds via add/remove", async () => {
    const user = userEvent.setup();
    renderPage();

    await user.click(screen.getByRole("button", { name: /Poll/ }));
    expect(screen.queryByLabelText("Remove option 1")).not.toBeInTheDocument();

    for (let i = 0; i < 4; i++) {
      await user.click(screen.getByRole("button", { name: "＋ Add option" }));
    }
    expect(screen.getAllByPlaceholderText(/Option \d/)).toHaveLength(6);
    expect(screen.queryByRole("button", { name: "＋ Add option" })).not.toBeInTheDocument();

    for (let i = 0; i < 4; i++) {
      await user.click(screen.getAllByLabelText(/Remove option/)[0]!);
    }
    expect(screen.getAllByPlaceholderText(/Option \d/)).toHaveLength(2);
    expect(screen.queryByLabelText("Remove option 1")).not.toBeInTheDocument();
  });

  it("maps a server VALIDATION_FAILED field error onto the prompt field", async () => {
    const user = userEvent.setup();
    vi.mocked(api.createCandidate).mockRejectedValue(
      new ApiError(422, "VALIDATION_FAILED", {
        type: "https://api.opennewsletter.example.com/errors/validation-failed",
        title: "Validation failed",
        status: 422,
        detail: "prompt must be between 5 and 500 characters",
        code: "VALIDATION_FAILED",
        correlationId: "corr-1",
        fieldErrors: [
          {
            field: "prompt",
            code: "INVALID",
            message: "prompt must be between 5 and 500 characters",
          },
        ],
      }),
    );

    renderPage();
    await user.type(screen.getByLabelText("Your question"), "What's your favorite hike?");
    await user.click(screen.getByRole("button", { name: "Submit" }));

    expect(
      await screen.findByText("prompt must be between 5 and 500 characters"),
    ).toBeInTheDocument();
  });

  it("shows a form-level message for CYCLE_NOT_VOTING", async () => {
    const user = userEvent.setup();
    vi.mocked(api.createCandidate).mockRejectedValue(
      new ApiError(409, "CYCLE_NOT_VOTING", {
        type: "https://api.opennewsletter.example.com/errors/cycle-not-voting",
        title: "Conflict",
        status: 409,
        detail: "this group has no cycle currently accepting candidate questions",
        code: "CYCLE_NOT_VOTING",
        correlationId: "corr-2",
      }),
    );

    renderPage();
    await user.type(screen.getByLabelText("Your question"), "What's your favorite hike?");
    await user.click(screen.getByRole("button", { name: "Submit" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(/check back soon/i);
  });
});
