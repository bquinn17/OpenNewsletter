export type UploadResult = { status: number };

/**
 * PUTs `file` to a presigned S3 `url` with every header from the presign response, reporting
 * progress via `onProgress` (0-1). Isolated from `ImageUploader` so tests can mock it.
 */
export function uploadToS3(
  url: string,
  file: File,
  headers: Record<string, string>,
  onProgress?: (progress: number) => void,
): Promise<UploadResult> {
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
