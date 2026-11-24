import type { ReactElement } from "react";
import { act, fireEvent, render as rtlRender, screen } from "@testing-library/react";
import { QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "../../api/client";
import { queryClient } from "../../api/queryClient";
import { uploadToS3 } from "../../utils/uploadToS3";
import { VoiceClipUploader } from "./VoiceClipUploader";

vi.mock("../../utils/uploadToS3", () => ({ uploadToS3: vi.fn() }));

// Keeps the real `pollMediaStatus` but stubs out the SHA-256 digest and the local waveform
// decode, neither of which is under test here (mirrors `ImageUploader.test.tsx`).
vi.mock("../../utils/media", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../utils/media")>();
  return { ...actual, sha256Base64: vi.fn().mockResolvedValue(undefined) };
});
vi.mock("../../utils/audioDecode", () => ({
  decodeLocalClip: vi.fn().mockRejectedValue(new Error("no AudioContext in jsdom")),
}));

vi.mock("../../api/client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../api/client")>();
  return {
    ...actual,
    api: {
      media: {
        createAudioUpload: vi.fn(),
        getAudioUpload: vi.fn(),
        patchAudioUpload: vi.fn(),
        deleteAudioUpload: vi.fn(),
      },
    },
  };
});

const createAudioUpload = api.media.createAudioUpload as ReturnType<typeof vi.fn>;
const getAudioUpload = api.media.getAudioUpload as ReturnType<typeof vi.fn>;
const deleteAudioUpload = api.media.deleteAudioUpload as ReturnType<typeof vi.fn>;
const mockedUploadToS3 = uploadToS3 as ReturnType<typeof vi.fn>;

function makeFile(name: string, type: string, size: number): File {
  return new File([new Uint8Array(size)], name, { type });
}

function getFileInput(): HTMLInputElement {
  return screen.getByLabelText("Upload audio") as HTMLInputElement;
}

function render(ui: ReactElement) {
  return rtlRender(<QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>);
}

async function flush(times = 8) {
  for (let i = 0; i < times; i++) await Promise.resolve();
}

describe("VoiceClipUploader", () => {
  beforeEach(() => {
    createAudioUpload.mockReset();
    getAudioUpload.mockReset();
    deleteAudioUpload.mockReset();
    mockedUploadToS3.mockReset();
  });

  afterEach(() => {
    vi.useRealTimers();
    queryClient.clear();
  });

  it("rejects a file that fails client-side validation without calling the API", async () => {
    render(<VoiceClipUploader groupId="g1" cycleId="202606" questionId="q1" maxClips={3} />);
    const badType = makeFile("doc.txt", "text/plain", 100);

    await act(async () => {
      fireEvent.change(getFileInput(), { target: { files: [badType] } });
      await flush();
    });

    expect(createAudioUpload).not.toHaveBeenCalled();
    expect(screen.getByText("doc.txt: That file type isn't supported.")).toBeInTheDocument();
  });

  it("respects the remaining-slot cap", async () => {
    createAudioUpload.mockResolvedValue({
      audioId: "aud1",
      uploadUrl: "https://s3.test/up",
      headers: { "content-type": "audio/webm" },
      expiresInSeconds: 600,
    });
    mockedUploadToS3.mockReturnValue(new Promise(() => {}));

    render(<VoiceClipUploader groupId="g1" cycleId="202606" questionId="q1" maxClips={1} />);
    const first = makeFile("a.webm", "audio/webm", 100);
    const second = makeFile("b.webm", "audio/webm", 100);

    await act(async () => {
      fireEvent.change(getFileInput(), { target: { files: [first, second] } });
      await flush();
    });

    expect(createAudioUpload).toHaveBeenCalledTimes(1);
    expect(screen.getByText("b.webm: You can attach up to 1 voice clip.")).toBeInTheDocument();
  });

  it("uploads, PUTs, polls, and reports the clip ready", async () => {
    vi.useFakeTimers();
    createAudioUpload.mockResolvedValue({
      audioId: "aud1",
      uploadUrl: "https://s3.test/up",
      headers: { "content-type": "audio/webm" },
      expiresInSeconds: 600,
    });
    mockedUploadToS3.mockResolvedValue({ status: 200 });
    getAudioUpload.mockResolvedValue({
      audioId: "aud1",
      status: "ready",
      durationMs: 4000,
      peaks: new Array(48).fill(10),
      caption: null,
      url: "https://media.test.invalid/aud/g1/202606/q1/u1/aud1/clip.webm",
      errorMessage: null,
    });

    const onChange = vi.fn();
    render(
      <VoiceClipUploader
        groupId="g1"
        cycleId="202606"
        questionId="q1"
        maxClips={3}
        onChange={onChange}
      />,
    );
    const file = makeFile("clip.webm", "audio/webm", 1000);

    await act(async () => {
      fireEvent.change(getFileInput(), { target: { files: [file] } });
      await flush();
    });

    expect(createAudioUpload).toHaveBeenCalledWith(
      expect.objectContaining({
        groupId: "g1",
        cycleId: "202606",
        questionId: "q1",
        purpose: "response",
        mimeType: "audio/webm",
        byteSize: 1000,
      }),
    );
    expect(mockedUploadToS3).toHaveBeenCalledWith(
      "https://s3.test/up",
      file,
      { "content-type": "audio/webm" },
      expect.any(Function),
    );

    // A clip joins the list as soon as the upload starts (`uploading`), before `ready`.
    expect(onChange).toHaveBeenCalledWith([expect.objectContaining({ status: "uploading" })]);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1500);
      await flush();
    });

    expect(onChange).toHaveBeenLastCalledWith([
      expect.objectContaining({ audioId: "aud1", status: "ready", durationMs: 4000 }),
    ]);
  });

  it("shows a friendly error and a Retry button when the server marks the clip failed", async () => {
    vi.useFakeTimers();
    createAudioUpload.mockResolvedValue({
      audioId: "aud1",
      uploadUrl: "https://s3.test/up",
      headers: {},
      expiresInSeconds: 600,
    });
    mockedUploadToS3.mockResolvedValue({ status: 200 });
    getAudioUpload.mockResolvedValue({
      audioId: "aud1",
      status: "failed",
      durationMs: null,
      peaks: null,
      caption: null,
      url: null,
      errorMessage: "AUDIO_TOO_LONG",
    });

    render(<VoiceClipUploader groupId="g1" cycleId="202606" questionId="q1" maxClips={3} />);
    const file = makeFile("clip.webm", "audio/webm", 1000);

    await act(async () => {
      fireEvent.change(getFileInput(), { target: { files: [file] } });
      await flush();
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1500);
      await flush();
    });

    expect(screen.getByText("That voice clip is longer than 2 minutes.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Retry" })).toBeInTheDocument();

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Retry" }));
      await flush();
    });

    expect(createAudioUpload).toHaveBeenCalledTimes(2);
  });

  it("calls DELETE /audio-uploads/{id} on remove", async () => {
    createAudioUpload.mockResolvedValue({
      audioId: "aud1",
      uploadUrl: "https://s3.test/up",
      headers: {},
      expiresInSeconds: 600,
    });
    mockedUploadToS3.mockResolvedValue({ status: 200 });
    getAudioUpload.mockResolvedValue({
      audioId: "aud1",
      status: "ready",
      durationMs: 2000,
      peaks: new Array(48).fill(5),
      caption: null,
      url: "https://media.test.invalid/aud/x/clip.webm",
      errorMessage: null,
    });
    deleteAudioUpload.mockResolvedValue(undefined);

    render(<VoiceClipUploader groupId="g1" cycleId="202606" questionId="q1" maxClips={3} />);
    const file = makeFile("clip.webm", "audio/webm", 1000);

    await act(async () => {
      fireEvent.change(getFileInput(), { target: { files: [file] } });
      await flush(12);
    });

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Remove voice clip" }));
      await flush();
    });

    expect(deleteAudioUpload).toHaveBeenCalledWith("aud1", "g1", "202606");
  });
});
