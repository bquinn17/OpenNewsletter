import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AuthContext, type AuthValue } from "../auth/useAuth";
import { api } from "../api/client";
import { uploadToS3 } from "../utils/uploadToS3";
import { SettingsPage } from "./SettingsPage";

vi.mock("../utils/uploadToS3", () => ({ uploadToS3: vi.fn() }));

vi.mock("../utils/media", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../utils/media")>();
  return { ...actual, sha256Base64: vi.fn().mockResolvedValue(undefined) };
});

vi.mock("../api/client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../api/client")>();
  return {
    ...actual,
    api: {
      getMe: vi.fn(),
      getConfig: vi.fn(),
      patchMe: vi.fn(),
      removeMember: vi.fn(),
      media: {
        createAvatar: vi.fn(),
        getAvatar: vi.fn(),
        deleteAvatar: vi.fn(),
      },
      push: {
        listSubscriptions: vi.fn().mockResolvedValue({ items: [] }),
        listPreferences: vi.fn().mockResolvedValue({ items: [] }),
      },
    },
  };
});

const mockedUploadToS3 = uploadToS3 as ReturnType<typeof vi.fn>;

const signedIn: AuthValue = {
  status: "authenticated",
  user: { sub: "sub-1", email: "sam@example.com", name: "Sam" },
  login: vi.fn(),
  logout: vi.fn(),
};

function baseUser() {
  return {
    userId: "u1",
    email: "sam@example.com",
    displayName: "Sam",
    avatarColor: "teal" as const,
    avatarMediaId: null,
    avatarUrl: null,
    createdAt: "2026-01-01T00:00:00Z",
  };
}

function baseConfig() {
  return {
    userId: "u1",
    email: "sam@example.com",
    displayName: "Sam",
    vapidPublicKey: "",
    groupDefaults: {},
    memberships: [],
  };
}

function renderSettings() {
  const queryClient = new QueryClient();
  return render(
    <QueryClientProvider client={queryClient}>
      <AuthContext.Provider value={signedIn}>
        <MemoryRouter>
          <SettingsPage />
        </MemoryRouter>
      </AuthContext.Provider>
    </QueryClientProvider>,
  );
}

async function flush(times = 8) {
  for (let i = 0; i < times; i++) await Promise.resolve();
}

describe("SettingsPage avatar upload", () => {
  beforeEach(() => {
    vi.mocked(api.getMe).mockResolvedValue(baseUser());
    vi.mocked(api.getConfig).mockResolvedValue(baseConfig());
    mockedUploadToS3.mockReset();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("uploads a photo and ends with PATCH /me carrying the new avatarMediaId", async () => {
    vi.mocked(api.media.createAvatar).mockResolvedValue({
      avatarId: "av1",
      uploadUrl: "https://s3.test/up",
      headers: { "content-type": "image/png" },
      expiresInSeconds: 600,
    });
    mockedUploadToS3.mockResolvedValue({ status: 200 });
    vi.mocked(api.media.getAvatar).mockResolvedValue({
      avatarId: "av1",
      mimeType: "image/png",
      status: "ready",
      bytes: 100,
      avatarUrl: "https://media.test.invalid/avatar/av1/display.webp",
      errorMessage: null,
      uploadedAt: "2026-01-01T00:00:00Z",
      processedAt: "2026-01-01T00:00:01Z",
    });
    vi.mocked(api.patchMe).mockResolvedValue({
      ...baseUser(),
      avatarMediaId: "av1",
      avatarUrl: "https://media.test.invalid/avatar/av1/display.webp",
    });

    renderSettings();
    const input = await screen.findByLabelText("Upload a profile photo");

    vi.useFakeTimers();
    const file = new File([new Uint8Array(100)], "me.png", { type: "image/png" });

    await act(async () => {
      fireEvent.change(input, { target: { files: [file] } });
      await flush();
    });

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1500);
      await flush();
    });

    expect(api.media.createAvatar).toHaveBeenCalledWith(
      expect.objectContaining({ mimeType: "image/png", byteSize: 100 }),
    );
    expect(mockedUploadToS3).toHaveBeenCalledWith(
      "https://s3.test/up",
      file,
      { "content-type": "image/png" },
      expect.any(Function),
    );
    expect(api.patchMe).toHaveBeenCalledWith({ avatarMediaId: "av1" });
  });
});

describe("SettingsPage avatar delete confirms (D8)", () => {
  const withAvatar = () => ({
    ...baseUser(),
    avatarMediaId: "av-old",
    avatarUrl: "https://media.test.invalid/avatar/av-old/display.webp",
  });

  beforeEach(() => {
    vi.mocked(api.getMe).mockResolvedValue(withAvatar());
    vi.mocked(api.getConfig).mockResolvedValue(baseConfig());
    vi.mocked(api.patchMe).mockReset();
    vi.mocked(api.media.deleteAvatar).mockReset();
  });

  it("deletes the avatar only after the permanent-delete confirm", async () => {
    vi.mocked(api.patchMe).mockResolvedValue(baseUser());
    vi.mocked(api.media.deleteAvatar).mockResolvedValue(undefined);
    renderSettings();

    fireEvent.click(await screen.findByRole("button", { name: "Remove photo" }));
    const dialog = await screen.findByRole("dialog", { name: "Remove your photo?" });
    expect(dialog).toHaveTextContent("This permanently deletes the image. It can't be recovered.");
    expect(api.patchMe).not.toHaveBeenCalled();
    expect(api.media.deleteAvatar).not.toHaveBeenCalled();

    fireEvent.click(within(dialog).getByRole("button", { name: "Remove photo" }));

    await waitFor(() => expect(api.media.deleteAvatar).toHaveBeenCalledWith("av-old"));
    expect(api.patchMe).toHaveBeenCalledWith({ avatarMediaId: null });
  });

  it("keeps the avatar when the remove confirm is cancelled", async () => {
    renderSettings();

    fireEvent.click(await screen.findByRole("button", { name: "Remove photo" }));
    const dialog = await screen.findByRole("dialog", { name: "Remove your photo?" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));

    await act(() => flush());
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(api.patchMe).not.toHaveBeenCalled();
    expect(api.media.deleteAvatar).not.toHaveBeenCalled();
  });

  it("confirms before replacing an existing avatar, and only then opens the file picker", async () => {
    renderSettings();
    const input = await screen.findByLabelText<HTMLInputElement>("Upload a profile photo");
    const pick = vi.spyOn(input, "click").mockImplementation(() => undefined);

    fireEvent.click(screen.getByRole("button", { name: "Upload photo" }));
    const dialog = await screen.findByRole("dialog", { name: "Replace your photo?" });
    expect(dialog).toHaveTextContent("This permanently deletes the image. It can't be recovered.");
    expect(pick).not.toHaveBeenCalled();

    fireEvent.click(within(dialog).getByRole("button", { name: "Choose a new photo" }));
    expect(pick).toHaveBeenCalledTimes(1);
  });

  it("opens the file picker straight away when there's no avatar to replace", async () => {
    vi.mocked(api.getMe).mockResolvedValue(baseUser());
    renderSettings();
    const input = await screen.findByLabelText<HTMLInputElement>("Upload a profile photo");
    const pick = vi.spyOn(input, "click").mockImplementation(() => undefined);

    fireEvent.click(screen.getByRole("button", { name: "Upload photo" }));

    expect(pick).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });
});
