import { createContext, useContext, useMemo, type ComponentProps } from "react";
import ReactMarkdown, { defaultUrlTransform, type Components } from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeSanitize, { defaultSchema, type Options as Schema } from "rehype-sanitize";
import clsx from "clsx";
import { MediaImage } from "../components/ui/MediaImage";
import { thumbSize } from "./media";

/**
 * Allows `image:{imageId}` tokens (`docs/design/media.md` §7) through the
 * sanitizer on `img[src]` in addition to the default `http`/`https`.
 * `<script>`/`<iframe>`/`<style>`, event handlers, and `javascript:` links
 * are rejected by `defaultSchema` already and untouched here.
 */
const markdownSanitizeSchema: Schema = {
  ...defaultSchema,
  protocols: {
    ...defaultSchema.protocols,
    src: [...(defaultSchema.protocols?.src ?? []), "image"],
  },
};

const IMAGE_TOKEN_RE = /!\[[^\]]*\]\(image:([^)\s]+)\)/g;

/**
 * `react-markdown`'s own URL sanitizer (independent of `rehype-sanitize`)
 * only allows `http(s)`/`irc(s)`/`mailto`/`xmpp` by default, so an
 * `image:{id}` token would be blanked before `rehypeSanitize` ever runs.
 * Pass this through unchanged for `image:` and defer to the default for
 * everything else (still rejecting `javascript:` etc.).
 */
function urlTransform(value: string): string {
  if (value.startsWith("image:")) return value;
  return defaultUrlTransform(value);
}

/** The `image:{id}` ids referenced inline in a response body. */
export function referencedImageIds(body: string): Set<string> {
  const ids = new Set<string>();
  for (const match of body.matchAll(IMAGE_TOKEN_RE)) {
    if (match[1]) ids.add(match[1]);
  }
  return ids;
}

/** `body` with every `![alt](image:{id})` token removed, whitespace collapsed. */
export function stripImageTokens(body: string): string {
  return body.replace(IMAGE_TOKEN_RE, "").replace(/\s+/g, " ").trim();
}

export type MarkdownImage = {
  imageId: string;
  displayUrl: string | null;
  thumbUrl: string | null;
  width?: number | null;
  height?: number | null;
};

type Props = {
  body: string;
  images: MarkdownImage[];
  className?: string;
  /**
   * When `false` (comments — `docs/design/engagement.md` §1.3/§5), `image:{id}` tokens
   * never render an `<img>`, regardless of `images`. A remote `http(s)` image
   * already degrades to a link either way, so this only changes the
   * `image:` token path.
   */
  allowImages?: boolean;
};

type ImageRendering = {
  byId: Map<string, MarkdownImage>;
  allowImages: boolean;
};

const ImageRenderingContext = createContext<ImageRendering>({ byId: new Map(), allowImages: true });

function MarkdownLink({ href, children }: ComponentProps<"a">) {
  return (
    <a href={href} target="_blank" rel="noopener noreferrer" className="text-grape underline">
      {children}
    </a>
  );
}

function MarkdownImg({ src, alt }: ComponentProps<"img">) {
  const { byId, allowImages } = useContext(ImageRenderingContext);
  if (!src) return null;
  // Only uploaded images render inline. A remote image would be blocked
  // by the CSP's `img-src` (`docs/design/frontend.md` §12.1) and would leak readers' IPs to
  // its host, so it degrades to a link.
  if (!src.startsWith("image:")) {
    return (
      <a href={src} target="_blank" rel="noopener noreferrer" className="text-grape underline">
        {alt || src}
      </a>
    );
  }
  if (!allowImages) return null;
  const image = byId.get(src.slice("image:".length));
  if (!image) return null;
  if (!image.displayUrl) {
    return (
      <span
        role="img"
        aria-label={alt || "Image uploading"}
        className="ph-img inline-block h-40 w-full rounded-xl align-top"
      />
    );
  }
  return (
    <MediaImage
      src={image.thumbUrl ?? image.displayUrl}
      alt={alt ?? ""}
      {...thumbSize(image.width, image.height)}
      className="rounded-xl"
    />
  );
}

/**
 * Module-level, so `ReactMarkdown` gets the same component types on every render. Rebuilt per
 * render, `img` would be a new type each time and React would remount every `MediaImage`,
 * wiping its retry state (`docs/design/media.md` §4.4). The per-render data reaches
 * `MarkdownImg` through `ImageRenderingContext` instead.
 */
const components: Components = { a: MarkdownLink, img: MarkdownImg };

/**
 * Renders a response/answer body as sanitized markdown, resolving
 * `image:{id}` tokens against `images` into `<MediaImage>`s. An id with no
 * match (not yet uploaded, or removed) renders nothing.
 */
export function MarkdownBody({ body, images, className, allowImages = true }: Props) {
  const rendering = useMemo(
    () => ({
      byId: new Map(images.map((image) => [image.imageId, image])),
      allowImages,
    }),
    [images, allowImages],
  );

  return (
    <div className={clsx("prose prose-sm max-w-none text-[15px] leading-relaxed", className)}>
      <ImageRenderingContext.Provider value={rendering}>
        <ReactMarkdown
          remarkPlugins={[remarkGfm]}
          rehypePlugins={[[rehypeSanitize, markdownSanitizeSchema]]}
          urlTransform={urlTransform}
          components={components}
        >
          {body}
        </ReactMarkdown>
      </ImageRenderingContext.Provider>
    </div>
  );
}
