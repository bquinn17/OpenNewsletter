import { useEffect, useMemo } from "react";
import ReactMarkdown, { defaultUrlTransform, type Components } from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeSanitize, { defaultSchema, type Options as Schema } from "rehype-sanitize";
import clsx from "clsx";
import { CdnImage } from "../components/ui/CdnImage";
import { ensureCookie } from "./media";

/**
 * Allows `image:{imageId}` tokens (`08-media-uploads.md` §7) through the
 * sanitizer on `img[src]` in addition to the default `http`/`https`.
 * `<script>`/`<iframe>`/`<style>`, event handlers, and `javascript:` links
 * are rejected by `defaultSchema` already and untouched here.
 */
export const markdownSanitizeSchema: Schema = {
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
};

type Props = {
  body: string;
  images: MarkdownImage[];
  groupId: string;
  className?: string;
};

/**
 * Renders a response/answer body as sanitized markdown, resolving
 * `image:{id}` tokens against `images` into `<CdnImage>`s. An id with no
 * match (not yet uploaded, or removed) renders nothing.
 */
export function MarkdownBody({ body, images, groupId, className }: Props) {
  const byId = useMemo(() => new Map(images.map((image) => [image.imageId, image])), [images]);
  const hasReadyImage = images.some((image) => image.displayUrl);

  useEffect(() => {
    if (hasReadyImage) void ensureCookie(groupId);
  }, [groupId, hasReadyImage]);

  const components: Components = {
    a: ({ href, children }) => (
      <a href={href} target="_blank" rel="noopener noreferrer" className="text-grape underline">
        {children}
      </a>
    ),
    img: ({ src, alt }) => {
      if (!src) return null;
      // Only uploaded images render inline. A remote image would be blocked
      // by the CSP's `img-src` (`04` §12.1) and would leak readers' IPs to
      // its host, so it degrades to a link.
      if (!src.startsWith("image:")) {
        return (
          <a href={src} target="_blank" rel="noopener noreferrer" className="text-grape underline">
            {alt || src}
          </a>
        );
      }
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
        <CdnImage
          src={image.thumbUrl ?? image.displayUrl}
          groupId={groupId}
          alt={alt ?? ""}
          className="rounded-xl"
        />
      );
    },
  };

  return (
    <div className={clsx("prose prose-sm max-w-none text-[15px] leading-relaxed", className)}>
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        rehypePlugins={[[rehypeSanitize, markdownSanitizeSchema]]}
        urlTransform={urlTransform}
        components={components}
      >
        {body}
      </ReactMarkdown>
    </div>
  );
}
