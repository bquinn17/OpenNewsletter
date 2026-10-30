import { isMockMode } from "../demo";

export type UploadResult = { status: number };

const MOCK_PROGRESS_STEPS = [0.3, 0.7, 1];
const MOCK_PROGRESS_STEP_MS = 150;

/**
 * Stands in for the S3 PUT when the API is the in-memory mock, whose presigned URLs
 * (`mock://upload/…`) aren't reachable. Steps the progress bar so the uploader looks the same.
 */
async function mockUpload(onProgress?: (progress: number) => void): Promise<UploadResult> {
  for (const progress of MOCK_PROGRESS_STEPS) {
    await new Promise((resolve) => setTimeout(resolve, MOCK_PROGRESS_STEP_MS));
    onProgress?.(progress);
  }
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
  if (isMockMode) return mockUpload(onProgress);
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
