import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { audioUploadQuery } from "../../api/queries";
import type { components } from "../../types/api";

type S = components["schemas"];

/**
 * The voice-clip equivalent of `useHydratedImages`: loads the draft's `AudioMediaResponse` rows as
 * they were when the page opened, so `VoiceClipUploader`'s `initialClips` (read once, on mount)
 * doesn't autosave the draft's clips away. A clip that can't be fetched is dropped.
 */
export function useHydratedAudio(
  groupId: string,
  cycleId: string,
  audioMediaIds: string[],
): { clips: S["AudioMediaResponse"][]; ready: boolean } {
  const qc = useQueryClient();
  const [ids] = useState(audioMediaIds);
  const { data, isSuccess } = useQuery({
    queryKey: ["hydratedAudio", groupId, cycleId, ids.join(",")],
    queryFn: async () => {
      const results = await Promise.allSettled(
        ids.map((id) => qc.fetchQuery(audioUploadQuery(groupId, cycleId, id))),
      );
      return results.flatMap((r) => (r.status === "fulfilled" ? [r.value] : []));
    },
    enabled: ids.length > 0,
    staleTime: Infinity,
  });
  return { clips: data ?? [], ready: ids.length === 0 || isSuccess };
}
