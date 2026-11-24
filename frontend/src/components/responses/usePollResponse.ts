import { useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { api, ApiError } from "../../api/client";
import { useToasts } from "../../state/toast";
import { cacheSavedResponse } from "./useResponseEditor";
import type { components } from "../../types/api";

type S = components["schemas"];

type Params = {
  groupId: string;
  cycleId: string;
  questionId: string;
  initialResponse: S["MyResponseSummary"] | null | undefined;
};

/**
 * Drives a poll question's answer: every pick saves immediately as a draft
 * (`docs/design/frontend.md` §7.3, D2). A non-empty draft auto-publishes
 * server-side at cycle close. Mirrors `useResponseEditor`'s save shape for
 * the text path, minus autosave debouncing — a poll pick has no typing to
 * wait out.
 */
export function usePollResponse({ groupId, cycleId, questionId, initialResponse }: Params) {
  const queryClient = useQueryClient();
  const pushToast = useToasts((s) => s.push);

  const [pollOptionId, setPollOptionId] = useState<string | null>(
    initialResponse?.pollOptionId ?? null,
  );
  // The pick the server holds, so a failed save can put the UI back on it.
  const savedPollOptionIdRef = useRef<string | null>(initialResponse?.pollOptionId ?? null);
  const [saving, setSaving] = useState(false);
  const [response, setResponse] = useState<S["ResponseDto"] | null>(null);
  const [closed, setClosed] = useState(false);
  // The most recent `save()` call, so `settle()` can await whatever pick is in flight.
  const savePromiseRef = useRef<Promise<void>>(Promise.resolve());

  const isPublished = (response?.status ?? initialResponse?.status) === "published";
  const isUnpublishedDraft = !isPublished && pollOptionId !== null;

  async function save(optionId: string) {
    setSaving(true);
    try {
      const saved = await api.saveMyResponse(groupId, cycleId, questionId, {
        kind: "poll",
        pollOptionId: optionId,
        publish: false,
      });
      setResponse(saved);
      savedPollOptionIdRef.current = saved.pollOptionId ?? optionId;
      cacheSavedResponse(queryClient, groupId, cycleId, questionId, saved);
    } catch (err) {
      // The server still holds the last saved pick, and that's what would
      // auto-publish, so show that rather than the unsaved one.
      setPollOptionId(savedPollOptionIdRef.current);
      if (err instanceof ApiError && err.status === 409) {
        setClosed(true);
      }
      const message =
        err instanceof ApiError && err.status === 409
          ? "This edition has closed — your last saved version stands."
          : "Couldn't save your vote — try again.";
      pushToast(message, "error");
    } finally {
      setSaving(false);
    }
  }

  function select(optionId: string) {
    setPollOptionId(optionId);
    // Every pick saves right away as a draft, the first one included, the
    // same as a text autosave (`docs/design/frontend.md` §7.3, D2). An unpublished pick is then
    // auto-published at the deadline; once published, a change stays
    // published (publishing is sticky server-side, `docs/design/api.md` §7.3).
    savePromiseRef.current = save(optionId);
  }

  /** Resolves once the pick's save (if any is in flight) has settled — for the "Done" CTA. */
  async function settle(): Promise<void> {
    await savePromiseRef.current;
  }

  return {
    pollOptionId,
    saving,
    response,
    closed,
    isUnpublishedDraft,
    select,
    settle,
  };
}
