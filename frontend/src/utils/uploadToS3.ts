import { keepMockUpload } from "../api/mockUploads";
import { isMockMode } from "../demo";

export type UploadResult = { status: number };

/** A believable upload speed, so the demo's progress bar fills the way a real one would. */
const MOCK_UPLOAD_BYTES_PER_SECOND = 1_500_000;
const MOCK_UPLOAD_MIN_MS = 1200;
const MOCK_UPLOAD_MAX_MS = 4000;
const MOCK_PROGRESS_TICK_MS = 100;

/**
 * Stands in for the S3 PUT when the API is the in-memory mock, whose presigned URLs
 * (`mock://upload/…`) aren't reachable. Keeps `file` in memory, so the mock serves the picture
 * itself once it's "processed", and fills the progress bar over a time that grows with its size.
 */
async function mockUpload(
  url: string,
  file: File,
  onProgress?: (progress: number) => void,
): Promise<UploadResult> {
  const kept = keepMockUpload(url, file);
  const durationMs = Math.min(
    Math.max((file.size / MOCK_UPLOAD_BYTES_PER_SECOND) * 1000, MOCK_UPLOAD_MIN_MS),
    MOCK_UPLOAD_MAX_MS,
  );
  const ticks = Math.ceil(durationMs / MOCK_PROGRESS_TICK_MS);
  for (let tick = 1; tick <= ticks; tick++) {
    await new Promise((resolve) => setTimeout(resolve, MOCK_PROGRESS_TICK_MS));
    onProgress?.(tick / ticks);
  }
  await kept;
  return { status: 200 };
}

/**
 * PUTs `file` to a presigned S3 `url` with every header from the presign response, reporting
 * progress via `onProgress` (0-1). Isolated from `ImageUploader` so tests can mock it. Never
 * touches the network in mock mode, so the demo writes nothing to S3 (`docs/design/frontend.md` §17).
 */
export function uploadToS3(
  url: string,
  file: File,
  headers: Record<string, string>,
  onProgress?: (progress: number) => void,
): Promise<UploadResult> {
  if (isMockMode) return mockUpload(url, file, onProgress);
  return new Promise((resolve, reject) => {
    const xhr = new XMLHttpRequest();
    xhr.open("PUT", url, true);
    for (const [name, value] of Object.entries(headers)) {
      xhr.setRequestHeader(name, value);
    }
    xhr.upload.onprogress = (event) => {
      if (onProgress && event.lengthComputable) onProgress(event.loaded / event.total);
    };
    xhr.onload = () => resolve({ status: xhr.status });
    xhr.onerror = () => reject(new Error("Network error while uploading the image."));
    xhr.send(file);
  });
}
