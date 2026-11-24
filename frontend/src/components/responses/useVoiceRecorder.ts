import { useCallback, useEffect, useRef, useState } from "react";
import { bucketLevels, MAX_AUDIO_SECONDS, pickRecordingMimeType } from "../../utils/voiceClip";

const LEVEL_BAR_COUNT = 12;
const TICK_MS = 100;

export type RecorderPhase = "idle" | "requesting-permission" | "denied" | "recording" | "stopped";

export type RecorderState = {
  phase: RecorderPhase;
  /** Milliseconds recorded so far; meaningful while `phase` is `recording` or `stopped`. */
  elapsedMs: number;
  /** 12 bars, 0-255, live while `phase === "recording"` (`docs/design/frontend.md` §19.1). */
  levels: number[];
  /** Set once `phase === "stopped"`. */
  blob: Blob | null;
};

type WakeLockLike = { release(): Promise<void> };

function idleState(): RecorderState {
  return { phase: "idle", elapsedMs: 0, levels: [], blob: null };
}

/**
 * Drives the voice-clip recorder bar's state machine (`docs/design/frontend.md` §19.1): asks for
 * mic permission on the first `start()`, records with `MediaRecorder` (preferring Opus-in-WebM,
 * `audioBitsPerSecond: 32000`), reports a live ~12-bar level meter and elapsed time from a
 * `Web Audio` `AnalyserNode`, holds a Wake Lock where supported, and auto-stops at
 * `MAX_AUDIO_SECONDS`. Tap-to-start/tap-to-stop, not hold-to-talk.
 */
export function useVoiceRecorder(onAutoStop?: () => void) {
  const [state, setState] = useState<RecorderState>(idleState);
  const stateRef = useRef(state);
  stateRef.current = state;

  const streamRef = useRef<MediaStream | null>(null);
  const audioContextRef = useRef<AudioContext | null>(null);
  const analyserRef = useRef<AnalyserNode | null>(null);
  const recorderRef = useRef<MediaRecorder | null>(null);
  const chunksRef = useRef<BlobPart[]>([]);
  const mimeTypeRef = useRef<string | undefined>(undefined);
  const tickTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const startedAtRef = useRef(0);
  const wakeLockRef = useRef<WakeLockLike | null>(null);
  const cancelledRef = useRef(false);

  function cleanupTracks() {
    streamRef.current?.getTracks().forEach((track) => track.stop());
    streamRef.current = null;
    void audioContextRef.current?.close().catch(() => {});
    audioContextRef.current = null;
    analyserRef.current = null;
  }

  function stopTicking() {
    if (tickTimerRef.current) {
      clearInterval(tickTimerRef.current);
      tickTimerRef.current = null;
    }
  }

  async function releaseWakeLock() {
    try {
      await wakeLockRef.current?.release();
    } catch {
      // Best effort — nothing the member needs to see.
    }
    wakeLockRef.current = null;
  }

  const start = useCallback(async () => {
    if (
      stateRef.current.phase === "recording" ||
      stateRef.current.phase === "requesting-permission"
    ) {
      return;
    }
    setState({ ...idleState(), phase: "requesting-permission" });

    let stream: MediaStream;
    try {
      stream = await navigator.mediaDevices.getUserMedia({ audio: true });
    } catch {
      setState({ ...idleState(), phase: "denied" });
      return;
    }
    streamRef.current = stream;

    const AudioContextCtor =
      window.AudioContext ??
      (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
    const ctx = new AudioContextCtor();
    const analyser = ctx.createAnalyser();
    analyser.fftSize = 64;
    ctx.createMediaStreamSource(stream).connect(analyser);
    audioContextRef.current = ctx;
    analyserRef.current = analyser;

    const mimeType = pickRecordingMimeType();
    mimeTypeRef.current = mimeType;
    const recorder = new MediaRecorder(stream, {
      ...(mimeType ? { mimeType } : {}),
      audioBitsPerSecond: 32_000,
    });
    chunksRef.current = [];
    cancelledRef.current = false;
    recorder.addEventListener("dataavailable", (event) => {
      if (event.data.size > 0) chunksRef.current.push(event.data);
    });
    recorder.addEventListener("stop", () => {
      cleanupTracks();
      if (cancelledRef.current) {
        cancelledRef.current = false;
        setState(idleState());
        return;
      }
      const blob = new Blob(chunksRef.current, { type: mimeTypeRef.current ?? "audio/webm" });
      setState((prev) => ({ phase: "stopped", elapsedMs: prev.elapsedMs, levels: [], blob }));
    });
    recorderRef.current = recorder;

    try {
      wakeLockRef.current = (await navigator.wakeLock?.request("screen")) ?? null;
    } catch {
      wakeLockRef.current = null;
    }

    recorder.start();
    startedAtRef.current = Date.now();
    setState({
      phase: "recording",
      elapsedMs: 0,
      levels: new Array(LEVEL_BAR_COUNT).fill(0),
      blob: null,
    });

    tickTimerRef.current = setInterval(() => {
      const elapsedMs = Date.now() - startedAtRef.current;
      const data = new Uint8Array(analyser.frequencyBinCount);
      analyser.getByteFrequencyData(data);
      const levels = bucketLevels(data, LEVEL_BAR_COUNT);
      if (elapsedMs >= MAX_AUDIO_SECONDS * 1000) {
        stopTicking();
        void releaseWakeLock();
        recorderRef.current?.stop();
        onAutoStop?.();
        return;
      }
      setState((prev) => (prev.phase === "recording" ? { ...prev, elapsedMs, levels } : prev));
    }, TICK_MS);
  }, [onAutoStop]);

  const stop = useCallback(() => {
    if (stateRef.current.phase !== "recording") return;
    stopTicking();
    void releaseWakeLock();
    recorderRef.current?.stop();
  }, []);

  const cancel = useCallback(() => {
    stopTicking();
    void releaseWakeLock();
    if (recorderRef.current && recorderRef.current.state !== "inactive") {
      cancelledRef.current = true;
      recorderRef.current.stop();
    } else {
      cleanupTracks();
      setState(idleState());
    }
  }, []);

  /** Clears a `stopped` clip back to idle, e.g. after it's handed off to the uploader. */
  const reset = useCallback(() => {
    setState(idleState());
  }, []);

  useEffect(() => {
    return () => {
      stopTicking();
      void releaseWakeLock();
      cleanupTracks();
    };
  }, []);

  return { state, start, stop, cancel, reset };
}
