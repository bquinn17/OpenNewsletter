import { useEffect, useState } from "react";

/**
 * A callback ref and the rendered height in px of the element it's attached to, kept current as
 * it resizes. Sticky elements stacked under it offset by this instead of a guessed constant, since
 * a header's height depends on its content and font metrics. A callback ref rather than a ref
 * object, because the element can mount after the first render (pages render a loading state
 * first).
 */
export function useElementHeight<T extends HTMLElement>(): [(node: T | null) => void, number] {
  const [node, setNode] = useState<T | null>(null);
  const [height, setHeight] = useState(0);

  useEffect(() => {
    if (!node) return;
    setHeight(node.getBoundingClientRect().height);
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(() => setHeight(node.getBoundingClientRect().height));
    observer.observe(node);
    return () => observer.disconnect();
  }, [node]);

  return [setNode, height];
}
