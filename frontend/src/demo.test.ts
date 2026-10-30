import { afterEach, describe, expect, it, vi } from "vitest";
import { isDemoPath } from "./demo";

/**
 * `isDemo` is read once, when `demo.ts` loads, from the URL the page loaded at. These tests set
 * that URL, then load fresh copies of the modules under test, as a real page load would.
 */
async function loadAt(pathname: string) {
  window.history.replaceState(null, "", pathname);
  vi.resetModules();
  const [demo, client, upload, storage, push, invite] = await Promise.all([
    import("./demo"),
    import("./api/client"),
    import("./utils/uploadToS3"),
    import("./utils/appStorage"),
    import("./pwa/pushSetup"),
    import("./utils/inviteUrl"),
  ]);
  return { ...demo, ...client, ...upload, ...storage, ...push, ...invite };
}

afterEach(() => {
  window.history.replaceState(null, "", "/");
  vi.unstubAllGlobals();
});

describe("isDemoPath", () => {
  it.each(["/demo", "/demo/", "/demo/g/g_trail/upcoming"])("treats %s as the demo", (path) => {
    expect(isDemoPath(path)).toBe(true);
  });

  it.each(["/", "/demonstration", "/g/demo", "/settings"])("treats %s as the real app", (path) => {
    expect(isDemoPath(path)).toBe(false);
  });
});

describe("a page loaded under /demo", () => {
  it("is the demo, and one loaded anywhere else isn't", async () => {
    expect((await loadAt("/demo/g/g_trail")).isDemo).toBe(true);
    expect((await loadAt("/g/g_trail")).isDemo).toBe(false);
  });

  it("answers the API and uploads images without any network request", async () => {
    const fetchSpy = vi.fn();
    const xhrSpy = vi.fn();
    vi.stubGlobal("fetch", fetchSpy);
    vi.stubGlobal("XMLHttpRequest", xhrSpy);
    const { api, uploadToS3 } = await loadAt("/demo");

    await api.createCandidate("g_trail", {
      kind: "text",
      prompt: "Best trail snack?",
      isAnonymous: false,
    });
    const presign = await api.media.createUpload({
      groupId: "g_trail",
      cycleId: "c_2026_10",
      questionId: "q_1",
      mimeType: "image/png",
      byteSize: 1,
    });
    const progress: number[] = [];
    const result = await uploadToS3(
      presign.uploadUrl,
      new File(["x"], "x.png"),
      presign.headers,
      (p) => progress.push(p),
    );

    expect(result.status).toBe(200);
    expect(progress.at(-1)).toBe(1);
    expect(fetchSpy).not.toHaveBeenCalled();
    expect(xhrSpy).not.toHaveBeenCalled();
  });

  it("keeps stored client state in memory, never in localStorage", async () => {
    const { appStorage } = await loadAt("/demo");

    appStorage().setItem("on:currentGroup", "g_trail");

    expect(appStorage().getItem("on:currentGroup")).toBe("g_trail");
    expect(localStorage.length).toBe(0);
  });

  it("reports push as unsupported, so nothing subscribes this browser", async () => {
    const { isPushSupported } = await loadAt("/demo");

    expect(isPushSupported()).toBe(false);
  });

  it("builds invite links that stay in the demo", async () => {
    const { inviteUrl } = await loadAt("/demo/g/g_trail/admin");

    expect(inviteUrl("DEMO-JOIN-CODE")).toBe(
      `${window.location.origin}/demo/join?code=DEMO-JOIN-CODE`,
    );
  });
});
