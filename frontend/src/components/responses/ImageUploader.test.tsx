import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "../../api/client";
import { uploadToS3 } from "../../utils/uploadToS3";
import { ImageUploader } from "./ImageUploader";

vi.mock("../../utils/uploadToS3", () => ({ uploadToS3: vi.fn() }));

vi.mock("../../api/client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../api/client")>();
  return {
    ...actual,
    api: {
      media: {
        createUpload: vi.fn(),
        getUpload: vi.fn(),
        completeUpload: vi.fn(),
        deleteUpload: vi.fn(),
        patchUpload: vi.fn(),
      },
    },
  };
});

// Keeps the real `withMediaAuth`/`refreshMediaAuth`/`pollMediaStatus` (exercised via real timers
// below) but stubs out the network-backed cookie fetch and the SHA-256 digest.
vi.mock("../../utils/media", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../utils/media")>();
  return {
    ...actual,
    ensureCookie: vi.fn().mockResolvedValue(undefined),
    sha256Base64: vi.fn().mockResolvedValue(undefined),
  };
});

const createUpload = api.media.createUpload as ReturnType<typeof vi.fn>;
const getUpload = api.media.getUpload as ReturnType<typeof vi.fn>;
const completeUpload = api.media.completeUpload as ReturnType<typeof vi.fn>;
const deleteUpload = api.media.deleteUpload as ReturnType<typeof vi.fn>;
const mockedUploadToS3 = uploadToS3 as ReturnType<typeof vi.fn>;

function makeFile(name: string, type: string, size: number): File {
  return new File([new Uint8Array(size)], name, { type });
}

function getFileInput(): HTMLInputElement {
  return document.querySelector("input[type=file]") as HTMLInputElement;
}

async function flush(times = 8) {
  for (let i = 0; i < times; i++) await Promise.resolve();
}

describe("ImageUploader", () => {
  beforeEach(() => {
    createUpload.mockReset();
    getUpload.mockReset();
    completeUpload.mockReset().mockResolvedValue({});
    deleteUpload.mockReset();
    mockedUploadToS3.mockReset();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("rejects files that fail client-side validation without calling the API", async () => {
    render(<ImageUploader groupId="g1" cycleId="202606" questionId="q1" />);
    const tooBig = makeFile("big.png", "image/png", 16_000_000);
    const badType = makeFile("doc.txt", "text/plain", 100);

    await act(async () => {
      fireEvent.change(getFileInput(), { target: { files: [tooBig, badType] } });
      await flush();
    });

    expect(createUpload).not.toHaveBeenCalled();
    expect(screen.getByText("big.png: That image is larger than 15 MB.")).toBeInTheDocument();
    expect(screen.getByText("doc.txt: That file type isn't supported.")).toBeInTheDocument();
  });

  it("respects the remaining-slot cap", async () => {
    createUpload.mockResolvedValue({
      imageId: "img1",
      uploadUrl: "https://s3.test/up",
      headers: { "content-type": "image/jpeg" },
      expiresInSeconds: 600,
    });
    mockedUploadToS3.mockReturnValue(new Promise(() => {}));

    render(<ImageUploader groupId="g1" cycleId="202606" questionId="q1" maxImages={1} />);
    const first = makeFile("a.jpg", "image/jpeg", 100);
    const second = makeFile("b.jpg", "image/jpeg", 100);

    await act(async () => {
      fireEvent.change(getFileInput(), { target: { files: [first, second] } });
      await flush();
    });

    expect(createUpload).toHaveBeenCalledTimes(1);
    expect(screen.getByText("b.jpg: You can attach up to 1 images.")).toBeInTheDocument();
  });

  it("uploads, PUTs with every presign header, polls, and shows the ready thumbnail", async () => {
    vi.useFakeTimers();
    createUpload.mockResolvedValue({
      imageId: "img1",
      uploadUrl: "https://s3.test/up",
      headers: { "content-type": "image/jpeg", "x-amz-checksum-sha256": "abc" },
      expiresInSeconds: 600,
    });
    mockedUploadToS3.mockResolvedValue({ status: 200 });
    getUpload.mockResolvedValue({
      imageId: "img1",
      status: "ready",
      thumbUrl: "https://cdn.test.invalid/img/g1/202606/q1/u1/img1/thumb.webp",
      displayUrl: "https://cdn.test.invalid/img/g1/202606/q1/u1/img1/display.webp",
      errorMessage: null,
    });

    const onChange = vi.fn();
    render(<ImageUploader groupId="g1" cycleId="202606" questionId="q1" onChange={onChange} />);
    const file = makeFile("photo.jpg", "image/jpeg", 1000);

    await act(async () => {
      fireEvent.change(getFileInput(), { target: { files: [file] } });
      await flush();
    });

    expect(createUpload).toHaveBeenCalledWith(
      expect.objectContaining({
        groupId: "g1",
        cycleId: "202606",
        questionId: "q1",
        purpose: "response",
        mimeType: "image/jpeg",
        byteSize: 1000,
      }),
    );
    expect(mockedUploadToS3).toHaveBeenCalledWith(
      "https://s3.test/up",
      file,
      { "content-type": "image/jpeg", "x-amz-checksum-sha256": "abc" },
      expect.any(Function),
    );

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1500);
      await flush();
    });

    expect(screen.getByAltText("Uploaded image")).toBeInTheDocument();
    expect(onChange).toHaveBeenLastCalledWith([
      expect.objectContaining({ imageId: "img1", status: "ready" }),
    ]);
  });

  it("retries once with a fresh presign after a PUT 403", async () => {
    vi.useFakeTimers();
    createUpload
      .mockResolvedValueOnce({
        imageId: "img1",
        uploadUrl: "https://s3.test/up1",
        headers: { "content-type": "image/jpeg" },
        expiresInSeconds: 600,
      })
      .mockResolvedValueOnce({
        imageId: "img2",
        uploadUrl: "https://s3.test/up2",
        headers: { "content-type": "image/jpeg" },
        expiresInSeconds: 600,
      });
    mockedUploadToS3.mockResolvedValueOnce({ status: 403 }).mockResolvedValueOnce({ status: 200 });
    getUpload.mockResolvedValue({
      imageId: "img2",
      status: "pending",
      thumbUrl: null,
      displayUrl: null,
      errorMessage: null,
    });

    render(<ImageUploader groupId="g1" cycleId="202606" questionId="q1" />);
    const file = makeFile("photo.jpg", "image/jpeg", 1000);

    await act(async () => {
      fireEvent.change(getFileInput(), { target: { files: [file] } });
      await flush(12);
    });

    expect(createUpload).toHaveBeenCalledTimes(2);
    expect(mockedUploadToS3).toHaveBeenCalledTimes(2);
    expect(mockedUploadToS3).toHaveBeenNthCalledWith(
      2,
      "https://s3.test/up2",
      file,
      { "content-type": "image/jpeg" },
      expect.any(Function),
    );
  });

  it("shows a friendly error when the server marks the upload failed", async () => {
    vi.useFakeTimers();
    createUpload.mockResolvedValue({
      imageId: "img1",
      uploadUrl: "https://s3.test/up",
      headers: {},
      expiresInSeconds: 600,
    });
    mockedUploadToS3.mockResolvedValue({ status: 200 });
    getUpload.mockResolvedValue({
      imageId: "img1",
      status: "failed",
      thumbUrl: null,
      displayUrl: null,
      errorMessage: "IMAGE_DECODE_FAILED",
    });

    render(<ImageUploader groupId="g1" cycleId="202606" questionId="q1" />);
    const file = makeFile("photo.jpg", "image/jpeg", 1000);

    await act(async () => {
      fireEvent.change(getFileInput(), { target: { files: [file] } });
      await flush();
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1500);
      await flush();
    });

    expect(
      screen.getByText("Couldn't process that image — try a different file."),
    ).toBeInTheDocument();
  });

  it("removes a ready image via DELETE and drops it from the list", async () => {
    vi.useFakeTimers();
    createUpload.mockResolvedValue({
      imageId: "img1",
      uploadUrl: "https://s3.test/up",
      headers: {},
      expiresInSeconds: 600,
    });
    mockedUploadToS3.mockResolvedValue({ status: 200 });
    getUpload.mockResolvedValue({
      imageId: "img1",
      status: "ready",
      thumbUrl: "https://cdn.test.invalid/img/g1/202606/q1/u1/img1/thumb.webp",
      displayUrl: "https://cdn.test.invalid/img/g1/202606/q1/u1/img1/display.webp",
      errorMessage: null,
    });
    deleteUpload.mockResolvedValue(undefined);

    render(<ImageUploader groupId="g1" cycleId="202606" questionId="q1" />);
    const file = makeFile("photo.jpg", "image/jpeg", 1000);

    await act(async () => {
      fireEvent.change(getFileInput(), { target: { files: [file] } });
      await flush();
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1500);
      await flush();
    });

    expect(screen.getByAltText("Uploaded image")).toBeInTheDocument();

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /remove image/i }));
      await flush();
    });

    expect(deleteUpload).toHaveBeenCalledWith("img1", "g1", "202606");
    expect(screen.queryByAltText("Uploaded image")).not.toBeInTheDocument();
  });

  it("resumes polling an initial image that was still pending on mount", async () => {
    vi.useFakeTimers();
    getUpload.mockResolvedValue({
      imageId: "img1",
      status: "ready",
      thumbUrl: "https://cdn.test.invalid/img/g1/202606/q1/u1/img1/thumb.webp",
      displayUrl: "https://cdn.test.invalid/img/g1/202606/q1/u1/img1/display.webp",
      errorMessage: null,
    });

    render(
      <ImageUploader
        groupId="g1"
        cycleId="202606"
        questionId="q1"
        initialImages={[
          {
            imageId: "img1",
            userId: "u1",
            groupId: "g1",
            cycleId: "202606",
            questionId: "q1",
            purpose: "response",
            mimeType: "image/jpeg",
            status: "pending",
            bytes: 1000,
            width: null,
            height: null,
            caption: null,
            displayUrl: null,
            thumbUrl: null,
            errorMessage: null,
            uploadedAt: "2026-01-01T00:00:00Z",
            processedAt: null,
          },
        ]}
      />,
    );

    expect(getUpload).not.toHaveBeenCalled();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1500);
      await flush();
    });

    expect(getUpload).toHaveBeenCalledWith("img1", "g1", "202606");
    expect(screen.getByAltText("Uploaded image")).toBeInTheDocument();
  });
});
