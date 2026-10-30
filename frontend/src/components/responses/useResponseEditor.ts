import { useCallback, useEffect, useRef, useState } from "react";
import { useQueryClient, type QueryClient } from "@tanstack/react-query";
import { api, ApiError, apiErrorMessage } from "../../api/client";
import { queryKeys } from "../../api/queries";
import { env } from "../../env";
import { appStorage } from "../../utils/appStorage";
import { backoffDelayMs } from "../../utils/retry";
import type { components } from "../../types/api";

type S = components["schemas"];

export type EditorStatus =
  | { kind: "saved"; at: string }
  | { kind: "saving" }
  | { kind: "retrying" }
  | { kind: "dirty" }
  | { kind: "closed"; at: string | null };

type Draft = {
  body: string;
  imageMediaIds: string[];
};

type DetachWaiter = {
  imageId: string;
  resolve: () => void;
  reject: (reason: unknown) => void;
};

/** Resolves every `detachImage` waiter whose image the server's draft no longer lists. */
function settleDetachWaiters(waiters: { current: DetachWaiter[] }, server: Draft): void {
  waiters.current = waiters.current.filter((waiter) => {
    if (server.imageMediaIds.includes(waiter.imageId)) return true;
    waiter.resolve();
    return false;
  });
}

function rejectDetachWaiters(waiters: { current: DetachWaiter[] }, reason: unknown): void {
  const pending = waiters.current;
  waiters.current = [];
  for (const waiter of pending) waiter.reject(reason);
}

type Params = {
  groupId: string;
  cycleId: string;
  questionId: string;
  userId: string | undefined;
  /** `null` when the caller has no draft yet for this question. */
  initial: S["MyResponseSummary"] | null;
  /**
   * False for a poll question. The hook is still called (hooks can't be
   * conditional), but every side effect — autosave, flush, localStorage — is
   * a no-op, so mounting it never overwrites a poll answer with an empty
   * text save.
   */
  enabled?: boolean;
};

function draftsEqual(a: Draft, b: Draft): boolean {
  return (
    a.body === b.body &&
    a.imageMediaIds.length === b.imageMediaIds.length &&
    a.imageMediaIds.every((id, i) => id === b.imageMediaIds[i])
  );
}

function storageKey(groupId: string, cycleId: string, questionId: string, userId: string): string {
  return `draft:${groupId}/${cycleId}/${questionId}/${userId}`;
}

function readStoredDraft(key: string): Draft | null {
  try {
    const raw = appStorage().getItem(key);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as Partial<Draft>;
    if (typeof parsed.body !== "string") return null;
    return {
      body: parsed.body,
      imageMediaIds: Array.isArray(parsed.imageMediaIds) ? parsed.imageMediaIds : [],
    };
  } catch {
    return null;
  }
}

function writeStoredDraft(key: string, draft: Draft): void {
  try {
    appStorage().setItem(key, JSON.stringify(draft));
  } catch {
    // Best effort — a crashed write just means tab-crash recovery doesn't work this time.
  }
}

function clearStoredDraft(key: string): void {
  try {
    appStorage().removeItem(key);
  } catch {
    // Best effort.
  }
}

/**
 * Writes a just-saved response into the cached newsletter detail (so leaving
 * and reopening the question shows it without waiting out `staleTime`) and
 * invalidates the newsletter list's draft/published counts. Shared by the
 * text autosave and `RespondPage`'s poll saves.
 */
export function cacheSavedResponse(
  qc: QueryClient,
  groupId: string,
  cycleId: string,
  questionId: string,
  response: S["ResponseDto"],
): void {
  qc.setQueryData<S["NewsletterDetailResponse"]>(queryKeys.newsletter(groupId, cycleId), (old) => {
    if (!old || old.status !== "open") return old;
    return {
      ...old,
      questions: old.questions.map((q) =>
        q.questionId === questionId ? { ...q, myResponse: toMyResponseSummary(response) } : q,
      ),
    };
  });
  void qc.invalidateQueries({ queryKey: queryKeys.newsletters(groupId) });
}

function toMyResponseSummary(response: S["ResponseDto"]): S["MyResponseSummary"] {
  return {
    responseId: response.responseId,
    status: response.status,
    body: response.body,
    imageMediaIds: response.imageMediaIds,
    pollOptionId: response.pollOptionId,
    updatedAt: response.updatedAt,
    publishedAt: response.publishedAt,
  };
}

function isRetryable(error: unknown): boolean {
  if (!(error instanceof ApiError)) return true;
  return error.status === 0 || error.status >= 500;
}

/**
 * Drives the text-response autosave loop (`docs/design/frontend.md` §8):
 * a debounced PUT after `env.autosaveDebounceMs` of no local changes, at
 * most one save in flight (later edits coalesce into the next save),
 * exponential-backoff retry on network/5xx failures, a hard stop on
 * `CYCLE_NOT_OPEN`, and a localStorage mirror for tab-crash recovery.
 */
export function useResponseEditor({
  groupId,
  cycleId,
  questionId,
  userId,
  initial,
  enabled = true,
}: Params) {
  const qc = useQueryClient();
  const key = userId ? storageKey(groupId, cycleId, questionId, userId) : null;

  const initialDraft: Draft = {
    body: initial?.body ?? "",
    imageMediaIds: initial?.imageMediaIds ?? [],
  };
  const restored = key ? readStoredDraft(key) : null;
  const startingDraft = restored && !draftsEqual(restored, initialDraft) ? restored : initialDraft;

  const [local, setLocal] = useState<Draft>(startingDraft);
  const [status, setStatus] = useState<EditorStatus>(
    draftsEqual(startingDraft, initialDraft)
      ? { kind: "saved", at: initial?.updatedAt ?? new Date().toISOString() }
      : { kind: "dirty" },
  );
  const [problemDetail, setProblemDetail] = useState<string | null>(null);
  const [lastResponse, setLastResponse] = useState<S["ResponseDto"] | null>(null);

  const localRef = useRef(local);
  localRef.current = local;
  const serverDraftRef = useRef<Draft>(initialDraft);
  const inFlightRef = useRef(false);
  const queuedRef = useRef<{ publish: boolean } | null>(null);
  const attemptRef = useRef(0);
  const debounceTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const retryTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const closedRef = useRef(false);
  // `detachImage` callers waiting for a save that no longer lists their image.
  const detachWaitersRef = useRef<DetachWaiter[]>([]);

  const performSaveRef = useRef<(publish: boolean) => Promise<void>>(() => Promise.resolve());

  performSaveRef.current = async (publish: boolean) => {
    if (!enabled || closedRef.current) return;
    if (inFlightRef.current) {
      queuedRef.current = { publish: (queuedRef.current?.publish ?? false) || publish };
      return;
    }
    inFlightRef.current = true;
    setStatus({ kind: "saving" });

    const draft = localRef.current;
    try {
      const response = await api.saveMyResponse(groupId, cycleId, questionId, {
        kind: "text",
        body: draft.body,
        imageMediaIds: draft.imageMediaIds,
        publish,
      });
      attemptRef.current = 0;
      serverDraftRef.current = { body: response.body ?? "", imageMediaIds: response.imageMediaIds };
      settleDetachWaiters(detachWaitersRef, serverDraftRef.current);
      setLastResponse(response);
      setProblemDetail(null);
      if (key) clearStoredDraft(key);

      cacheSavedResponse(qc, groupId, cycleId, questionId, response);

      inFlightRef.current = false;
      const queued = queuedRef.current;
      queuedRef.current = null;
      if (queued) {
        void performSaveRef.current(queued.publish);
      } else if (draftsEqual(localRef.current, serverDraftRef.current)) {
        setStatus({ kind: "saved", at: response.updatedAt });
      } else {
        setStatus({ kind: "dirty" });
      }
    } catch (error) {
      inFlightRef.current = false;

      if (error instanceof ApiError && error.code === "CYCLE_NOT_OPEN") {
        closedRef.current = true;
        queuedRef.current = null;
        rejectDetachWaiters(detachWaitersRef, error);
        setStatus({ kind: "closed", at: lastResponse?.updatedAt ?? null });
        void qc.invalidateQueries({ queryKey: queryKeys.newsletter(groupId, cycleId) });
        return;
      }

      if (!isRetryable(error)) {
        queuedRef.current = null;
        rejectDetachWaiters(detachWaitersRef, error);
        attemptRef.current = 0;
        setProblemDetail(apiErrorMessage(error, "Couldn't save."));
        setStatus({ kind: "dirty" });
        return;
      }

      setStatus({ kind: "retrying" });
      const delay = backoffDelayMs(attemptRef.current);
      attemptRef.current += 1;
      retryTimerRef.current = setTimeout(() => {
        const queued = queuedRef.current;
        queuedRef.current = null;
        void performSaveRef.current(queued?.publish ?? publish);
      }, delay);
    }
  };

  const flush = useCallback(
    async (publish = false): Promise<void> => {
      if (debounceTimerRef.current) {
        clearTimeout(debounceTimerRef.current);
        debounceTimerRef.current = null;
      }
      if (!enabled || closedRef.current) return;
      // Blur, tab-hide and unmount all flush; with nothing changed that must
      // not PUT, or merely visiting a question would create an empty draft.
      if (
        !publish &&
        !inFlightRef.current &&
        draftsEqual(localRef.current, serverDraftRef.current)
      ) {
        return;
      }
      await performSaveRef.current(publish);
    },
    [enabled],
  );

  // Debounced autosave: fires `env.autosaveDebounceMs` after the local draft
  // last changed from what the server has.
  useEffect(() => {
    if (!enabled || closedRef.current) return;
    if (draftsEqual(local, serverDraftRef.current)) return;
    if (debounceTimerRef.current) clearTimeout(debounceTimerRef.current);
    debounceTimerRef.current = setTimeout(() => {
      void performSaveRef.current(false);
    }, env.autosaveDebounceMs);
    return () => {
      if (debounceTimerRef.current) clearTimeout(debounceTimerRef.current);
    };
  }, [local, enabled]);

  // Mirror `local` into `appStorage` (`docs/design/frontend.md` §8.3) — best-effort, wrapped in try/catch inside the helpers.
  useEffect(() => {
    if (!enabled || !key) return;
    if (draftsEqual(local, serverDraftRef.current)) {
      clearStoredDraft(key);
    } else {
      writeStoredDraft(key, local);
    }
  }, [key, local, enabled]);

  // Flush on blur-equivalent triggers: tab hidden, or unmount.
  useEffect(() => {
    function onVisibilityChange() {
      if (document.visibilityState === "hidden") void flush();
    }
    document.addEventListener("visibilitychange", onVisibilityChange);
    return () => {
      document.removeEventListener("visibilitychange", onVisibilityChange);
      void flush();
    };
  }, [flush]);

  useEffect(() => {
    return () => {
      if (retryTimerRef.current) clearTimeout(retryTimerRef.current);
    };
  }, []);

  const setBody = useCallback((body: string) => {
    setLocal((prev) => ({ ...prev, body }));
  }, []);

  const setImageMediaIds = useCallback((imageMediaIds: string[]) => {
    setLocal((prev) => ({ ...prev, imageMediaIds }));
  }, []);

  const publish = useCallback(() => flush(true), [flush]);

  /**
   * Drops `imageId` from the draft (its ID, and its tokens via
   * `transformBody`) and resolves once a save without it has landed — the
   * first two steps of the editor's "remove image" (`docs/design/api.md`
   * §9.4): the draft must stop referencing an image before
   * `DELETE /uploads/{id}` removes it, or its next save would fail image
   * validation. Rejects if saving stops (closed cycle, non-retryable error).
   */
  const detachImage = useCallback(
    (imageId: string, transformBody: (body: string) => string): Promise<void> => {
      if (!enabled || closedRef.current) {
        return Promise.reject(new Error("The editor is no longer saving."));
      }
      const next: Draft = {
        body: transformBody(localRef.current.body),
        imageMediaIds: localRef.current.imageMediaIds.filter((id) => id !== imageId),
      };
      localRef.current = next;
      setLocal(next);
      // Nothing on the server (or on its way there) references it.
      if (!inFlightRef.current && !serverDraftRef.current.imageMediaIds.includes(imageId)) {
        return Promise.resolve();
      }
      if (debounceTimerRef.current) {
        clearTimeout(debounceTimerRef.current);
        debounceTimerRef.current = null;
      }
      return new Promise<void>((resolve, reject) => {
        detachWaitersRef.current.push({ imageId, resolve, reject });
        void performSaveRef.current(false);
      });
    },
    [enabled],
  );

  return {
    body: local.body,
    setBody,
    imageMediaIds: local.imageMediaIds,
    setImageMediaIds,
    status,
    problemDetail,
    readOnly: status.kind === "closed",
    lastResponse,
    flush: () => flush(false),
    publish,
    detachImage,
    onBlur: () => void flush(false),
  };
}
