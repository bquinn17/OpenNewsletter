/**
 * The files picked for upload in mock mode, kept in this page's memory so an uploaded image or
 * avatar shows the picture itself rather than a placeholder (`docs/design/frontend.md` §17). Each
 * is a `blob:` URL: nothing leaves the page or lands in browser storage, and it's gone with the
 * tab. Keyed by the image or avatar ID in the mock presign's upload URL.
 */

export type MockUploadedFile = { url: string; width: number; height: number };

const MOCK_UPLOAD_URL_PREFIX = "mock://upload/";
/** For a file the browser can't decode, which the real backend would turn into a JPEG. */
const FALLBACK_SIZE = { width: 800, height: 600 };

const files = new Map<string, MockUploadedFile>();

/** The presigned upload URL the mock hands out for image or avatar `id`. */
export function mockUploadUrl(id: string): string {
  return `${MOCK_UPLOAD_URL_PREFIX}${id}`;
}

/** Keeps `file` as the upload for the ID in `uploadUrl`. A no-op where `blob:` URLs don't exist (jsdom). */
export async function keepMockUpload(uploadUrl: string, file: File): Promise<void> {
  if (!uploadUrl.startsWith(MOCK_UPLOAD_URL_PREFIX) || typeof URL.createObjectURL !== "function") {
    return;
  }
  const id = uploadUrl.slice(MOCK_UPLOAD_URL_PREFIX.length);
  const size = await imageSize(file);
  files.set(id, { url: URL.createObjectURL(file), ...size });
}

async function imageSize(file: File): Promise<{ width: number; height: number }> {
  try {
    const bitmap = await createImageBitmap(file);
    const size = { width: bitmap.width, height: bitmap.height };
    bitmap.close();
    return size;
  } catch {
    return FALLBACK_SIZE;
  }
}

export function mockUploadedFile(id: string): MockUploadedFile | undefined {
  return files.get(id);
}

/** Frees the file kept for `id`, once its image or avatar is deleted. Idempotent. */
export function releaseMockUpload(id: string): void {
  const file = files.get(id);
  if (!file) return;
  URL.revokeObjectURL(file.url);
  files.delete(id);
}
