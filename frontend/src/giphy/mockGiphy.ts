import type { GiphyGif } from "./giphyClient";

/**
 * Fixed GIF fixtures for mock mode and the demo (`docs/design/frontend.md` §18.5/§17). Served from
 * `public/demo-media/` rather than the real GIPHY, so mock builds and the demo make no GIPHY
 * calls — the demo's off-site-request check (`frontend.md` §17) must still pass. IDs are
 * alphanumeric like GIPHY's, or the renderer drops them (`GIPHY_ID_RE`).
 */
const FIXTURES: (GiphyGif & { tags: string[] })[] = [
  {
    id: "demowave",
    title: "wave",
    rating: "g",
    width: 40,
    height: 40,
    videoUrl: "/demo-media/demo-gif-wave.gif",
    stillUrl: "/demo-media/demo-gif-wave-still.gif",
    tags: ["lol", "yes", "wave", "hi"],
  },
  {
    id: "democlap",
    title: "clap",
    rating: "g",
    width: 40,
    height: 40,
    videoUrl: "/demo-media/demo-gif-clap.gif",
    stillUrl: "/demo-media/demo-gif-clap-still.gif",
    tags: ["wow", "thank you", "clap", "nice"],
  },
  {
    id: "demoparty",
    title: "party",
    rating: "g",
    width: 40,
    height: 40,
    videoUrl: "/demo-media/demo-gif-party.gif",
    stillUrl: "/demo-media/demo-gif-party-still.gif",
    tags: ["celebrate", "party", "yes"],
  },
];

function stripTags(gif: GiphyGif & { tags: string[] }): GiphyGif {
  const { tags, ...rest } = gif;
  void tags;
  return rest;
}

/** A query matching no tag/title still returns every fixture, so the mock picker never shows "No GIFs". */
export function mockGiphySearch(
  query: string,
  offset: number,
): Promise<{ gifs: GiphyGif[]; hasMore: boolean }> {
  const q = query.trim().toLowerCase();
  const matched = q
    ? FIXTURES.filter((f) => f.title.includes(q) || f.tags.some((t) => t.includes(q)))
    : FIXTURES;
  const results = (matched.length > 0 ? matched : FIXTURES).map(stripTags);
  return Promise.resolve({ gifs: results.slice(offset), hasMore: false });
}

export function mockGiphyGetByIds(ids: string[]): Promise<GiphyGif[]> {
  const byId = new Map(FIXTURES.map((f) => [f.id, stripTags(f)]));
  return Promise.resolve(ids.flatMap((id) => (byId.has(id) ? [byId.get(id)!] : [])));
}
