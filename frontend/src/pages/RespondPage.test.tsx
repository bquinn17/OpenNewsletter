import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "../api/client";
import { AuthContext, type AuthValue } from "../auth/useAuth";
import { uploadToS3 } from "../utils/uploadToS3";
import type { components } from "../types/api";
import { RespondPage } from "./RespondPage";

type S = components["schemas"];

vi.mock("../utils/uploadToS3", () => ({ uploadToS3: vi.fn() }));

vi.mock("../utils/media", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../utils/media")>();
  return {
    ...actual,
    ensureCookie: vi.fn().mockResolvedValue(undefined),
    sha256Base64: vi.fn().mockResolvedValue(undefined),
  };
});

vi.mock("../api/client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../api/client")>();
  return {
    ...actual,
    api: {
      getNewsletter: vi.fn(),
      getConfig: vi.fn(),
      saveMyResponse: vi.fn(),
      media: {
        createUpload: vi.fn(),
        getUpload: vi.fn(),
        completeUpload: vi.fn(),
        deleteUpload: vi.fn(),
      },
    },
  };
});

const saveMyResponse = api.saveMyResponse as ReturnType<typeof vi.fn>;
const mockedUploadToS3 = uploadToS3 as ReturnType<typeof vi.fn>;

const signedIn: AuthValue = {
  status: "authenticated",
  user: { sub: "sub-1", email: "quinn@example.com", name: "Quinn" },
  login: vi.fn(),
  logout: vi.fn(),
};

function baseConfig(): S["ConfigResponse"] {
  return {
    userId: "u_quinn",
    email: "quinn@example.com",
    displayName: "Quinn",
    vapidPublicKey: "",
    groupDefaults: {},
    memberships: [],
  };
}

function openDetail(questions: S["OpenQuestionResponse"][]): S["NewsletterDetailResponse"] {
  return {
    status: "open",
    cycleId: "c1",
    responseOpenAt: "2026-06-01T00:00:00Z",
    responseCloseAt: new Date(Date.now() + 2 * 24 * 60 * 60_000).toISOString(),
    questions,
  };
}

function renderAt(questionId: string) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={queryClient}>
      <AuthContext.Provider value={signedIn}>
        <MemoryRouter initialEntries={[`/g/g1/n/c1/respond/${questionId}`]}>
          <Routes>
            <Route path="/g/:groupId/n/:cycleId/respond/:questionId" element={<RespondPage />} />
          </Routes>
        </MemoryRouter>
      </AuthContext.Provider>
    </QueryClientProvider>,
  );
}

async function flush(times = 8) {
  for (let i = 0; i < times; i++) await Promise.resolve();
}

describe("RespondPage", () => {
  beforeEach(() => {
    saveMyResponse.mockReset();
    vi.mocked(api.media.getUpload).mockReset();
    vi.mocked(api.getConfig).mockResolvedValue(baseConfig());
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("publishes a text response with the typed body and an empty imageMediaIds", async () => {
    vi.mocked(api.getNewsletter).mockResolvedValue(
      openDetail([
        {
          questionId: "q1",
          kind: "text",
          prompt: "What's new?",
          displayOrder: 0,
          askedBy: null,
          isAnonymous: false,
          myResponse: null,
        },
      ]),
    );
    saveMyResponse.mockResolvedValue({
      responseId: "r1",
      userId: "u_quinn",
      questionId: "q1",
      groupId: "g1",
      cycleId: "c1",
      kind: "text",
      status: "published",
      body: "Hello world",
      imageMediaIds: [],
      updatedAt: "2026-06-02T00:00:00Z",
      publishedAt: "2026-06-02T00:00:00Z",
    });

    renderAt("q1");

    const textarea = await screen.findByPlaceholderText("Tell us about it…");
    fireEvent.change(textarea, { target: { value: "Hello world" } });

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Publish" }));
      await flush();
    });

    expect(saveMyResponse).toHaveBeenCalledWith("g1", "c1", "q1", {
      kind: "text",
      body: "Hello world",
      imageMediaIds: [],
      publish: true,
    });
  });

  it("keeps a draft's existing images when the editor opens and the body is edited", async () => {
    vi.mocked(api.getNewsletter).mockResolvedValue(
      openDetail([
        {
          questionId: "q1",
          kind: "text",
          prompt: "What's new?",
          displayOrder: 0,
          askedBy: null,
          isAnonymous: false,
          myResponse: {
            responseId: "r1",
            status: "draft",
            body: "Look ![](image:img1)",
            imageMediaIds: ["img1"],
            updatedAt: "2026-06-02T00:00:00Z",
            publishedAt: null,
          },
        },
      ]),
    );
    vi.mocked(api.media.getUpload).mockResolvedValue({
      imageId: "img1",
      status: "ready",
      purpose: "response",
      mimeType: "image/jpeg",
      caption: null,
      thumbUrl: "https://cdn.example/img1-thumb.webp",
      displayUrl: "https://cdn.example/img1.webp",
      width: 100,
      height: 100,
      errorMessage: null,
    } as S["ImageMediaResponse"]);
    saveMyResponse.mockImplementation(
      (_g: string, _c: string, _q: string, body: S["SaveResponseRequest"]) =>
        Promise.resolve({
          responseId: "r1",
          userId: "u_quinn",
          questionId: "q1",
          groupId: "g1",
          cycleId: "c1",
          kind: "text",
          status: "draft",
          body: body.kind === "text" ? body.body : "",
          imageMediaIds: body.kind === "text" ? body.imageMediaIds : [],
          updatedAt: "2026-06-03T00:00:00Z",
          publishedAt: null,
        }),
    );

    renderAt("q1");

    const textarea = await screen.findByPlaceholderText("Tell us about it…");
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 1700));
    });
    // Opening the editor alone must not save (and so must not drop the image).
    expect(saveMyResponse).not.toHaveBeenCalled();

    fireEvent.change(textarea, { target: { value: "Look ![](image:img1) again" } });
    await act(async () => {
      fireEvent.blur(textarea);
      await flush();
    });

    expect(saveMyResponse).toHaveBeenCalledWith("g1", "c1", "q1", {
      kind: "text",
      body: "Look ![](image:img1) again",
      imageMediaIds: ["img1"],
      publish: false,
    });
  });

  it("inserts an image token on upload-ready and removes it when the image is removed", async () => {
    vi.mocked(api.getNewsletter).mockResolvedValue(
      openDetail([
        {
          questionId: "q1",
          kind: "text",
          prompt: "Share a photo",
          displayOrder: 0,
          askedBy: null,
          isAnonymous: false,
          myResponse: null,
        },
      ]),
    );
    vi.mocked(api.media.createUpload).mockResolvedValue({
      imageId: "img1",
      uploadUrl: "https://s3.test/up",
      headers: { "content-type": "image/jpeg" },
      expiresInSeconds: 600,
    });
    mockedUploadToS3.mockResolvedValue({ status: 200 });
    vi.mocked(api.media.completeUpload).mockResolvedValue({} as S["ImageMediaResponse"]);
    vi.mocked(api.media.getUpload).mockResolvedValue({
      imageId: "img1",
      userId: "u_quinn",
      groupId: "g1",
      cycleId: "c1",
      questionId: "q1",
      purpose: "response",
      mimeType: "image/jpeg",
      status: "ready",
      bytes: 1000,
      width: 800,
      height: 600,
      caption: null,
      displayUrl: "https://cdn.test.invalid/img1/display.webp",
      thumbUrl: "https://cdn.test.invalid/img1/thumb.webp",
      errorMessage: null,
      uploadedAt: "2026-06-01T00:00:00Z",
      processedAt: "2026-06-01T00:00:01Z",
    });
    vi.mocked(api.media.deleteUpload).mockResolvedValue(undefined);

    renderAt("q1");

    const textarea = await screen.findByPlaceholderText("Tell us about it…");
    const fileInput = document.querySelector("input[type=file]") as HTMLInputElement;
    const file = new File([new Uint8Array(100)], "photo.jpg", { type: "image/jpeg" });

    vi.useFakeTimers();
    await act(async () => {
      fireEvent.change(fileInput, { target: { files: [file] } });
      await flush();
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1500);
      await flush();
    });

    expect(textarea).toHaveValue("![](image:img1)\n");

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Remove image photo.jpg" }));
      await flush();
    });

    expect(textarea).toHaveValue("");
  });

  it("publishes a poll response with the selected option", async () => {
    vi.mocked(api.getNewsletter).mockResolvedValue(
      openDetail([
        {
          questionId: "q2",
          kind: "poll",
          prompt: "Pick one",
          displayOrder: 0,
          askedBy: null,
          isAnonymous: true,
          pollOptions: [
            { optionId: "optA", label: "Option A" },
            { optionId: "optB", label: "Option B" },
          ],
          myResponse: null,
        },
      ]),
    );
    saveMyResponse.mockResolvedValue({
      responseId: "r2",
      userId: "u_quinn",
      questionId: "q2",
      groupId: "g1",
      cycleId: "c1",
      kind: "poll",
      status: "published",
      pollOptionId: "optB",
      imageMediaIds: [],
      updatedAt: "2026-06-02T00:00:00Z",
      publishedAt: "2026-06-02T00:00:00Z",
    });

    renderAt("q2");

    const optionButton = await screen.findByRole("button", { name: "Option B" });
    fireEvent.click(optionButton);

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Publish" }));
      await flush();
    });

    expect(saveMyResponse).toHaveBeenCalledWith("g1", "c1", "q2", {
      kind: "poll",
      pollOptionId: "optB",
      publish: true,
    });
  });
});
