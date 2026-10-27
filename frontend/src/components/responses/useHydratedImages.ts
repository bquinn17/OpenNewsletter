import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { uploadQuery } from "../../api/queries";
import type { components } from "../../types/api";

type S = components["schemas"];

/**
 * Loads the `ImageMediaResponse` rows for the draft's images as they were when
 * the page opened. `ImageUploader` and `ResponseEditor` read `initialImages`
 * once, on mount, so the editor must not render until this has settled — an
 * editor mounted with no images would report `imageMediaIds: []` and autosave
 * the draft's images away. An image that can't be fetched (e.g. deleted) is
 * dropped rather than blocking the editor.
 *
 * Each row is fetched into its own `uploadQuery` entry, which is what the uploader, the preview
 * and the read-only answer read afterwards. This one-shot snapshot carries no `MEDIA_URLS_META`,
 * so a URL refresh refetches those entries and never this, and `ready` can't flip back.
 */
export function useHydratedImages(
  groupId: string,
  cycleId: string,
  imageMediaIds: string[],
): { images: S["ImageMediaResponse"][]; ready: boolean } {
  const qc = useQueryClient();
  const [ids] = useState(imageMediaIds);
  const { data, isSuccess } = useQuery({
    queryKey: ["hydratedImages", groupId, cycleId, ids.join(",")],
    queryFn: async () => {
      const results = await Promise.allSettled(
        ids.map((id) => qc.fetchQuery(uploadQuery(groupId, cycleId, id))),
      );
      return results.flatMap((r) => (r.status === "fulfilled" ? [r.value] : []));
    },
    enabled: ids.length > 0,
    staleTime: Infinity,
  });
  return { images: data ?? [], ready: ids.length === 0 || isSuccess };
}
