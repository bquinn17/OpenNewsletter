import { describe, expect, it, vi } from "vitest";
import { primeAudioCache } from "./audioCache";

describe("primeAudioCache", () => {
  it("issues one unranged, no-credentials GET for a given url", async () => {
    const fetchMock = vi.fn().mockResolvedValue(new Response());
    await primeAudioCache("https://media.example.com/aud/g/c/q/u/a1/clip.webm", fetchMock);

    expect(fetchMock).toHaveBeenCalledWith("https://media.example.com/aud/g/c/q/u/a1/clip.webm", {
      mode: "cors",
      credentials: "omit",
    });
  });

  it("fetches the same url only once", async () => {
    const fetchMock = vi.fn().mockResolvedValue(new Response());
    const url = "https://media.example.com/aud/g/c/q/u/a2/clip.webm";

    await primeAudioCache(url, fetchMock);
    await primeAudioCache(url, fetchMock);

    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it("swallows a fetch failure", async () => {
    const fetchMock = vi.fn().mockRejectedValue(new Error("offline"));
    await expect(
      primeAudioCache("https://media.example.com/aud/g/c/q/u/a3/clip.webm", fetchMock),
    ).resolves.toBeUndefined();
  });
});
