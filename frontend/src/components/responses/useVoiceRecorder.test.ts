import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useVoiceRecorder } from "./useVoiceRecorder";

type Listener = (event: unknown) => void;

class FakeTrack {
  stopped = false;
  stop() {
    this.stopped = true;
  }
}

class FakeStream {
  tracks = [new FakeTrack(), new FakeTrack()];
  getTracks() {
    return this.tracks;
  }
}

class FakeAnalyser {
  fftSize = 0;
  frequencyBinCount = 32;
  getByteFrequencyData(array: Uint8Array) {
    array.fill(100);
  }
}

class FakeAudioContext {
  createMediaStreamSource() {
    return { connect: vi.fn() };
  }
  createAnalyser() {
    return new FakeAnalyser();
  }
  close() {
    return Promise.resolve();
  }
}

class FakeMediaRecorder {
  static isTypeSupported = vi.fn().mockReturnValue(true);
  state: "inactive" | "recording" = "inactive";
  mimeType: string;
  listeners = new Map<string, Listener>();
  constructor(
    public stream: FakeStream,
    public options: { mimeType?: string },
  ) {
    this.mimeType = options.mimeType ?? "audio/webm";
  }
  addEventListener(type: string, listener: Listener) {
    this.listeners.set(type, listener);
  }
  start() {
    this.state = "recording";
  }
  stop() {
    this.state = "inactive";
    this.listeners.get("dataavailable")?.({ data: { size: 10 } });
    this.listeners.get("stop")?.({});
  }
}

let getUserMedia: ReturnType<typeof vi.fn>;

beforeEach(() => {
  vi.useFakeTimers();
  getUserMedia = vi.fn().mockResolvedValue(new FakeStream());
  vi.stubGlobal("navigator", {
    ...navigator,
    mediaDevices: { getUserMedia },
    wakeLock: undefined,
  });
  vi.stubGlobal("MediaRecorder", FakeMediaRecorder);
  vi.stubGlobal("AudioContext", FakeAudioContext);
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

describe("useVoiceRecorder", () => {
  it("starts idle", () => {
    const { result } = renderHook(() => useVoiceRecorder());
    expect(result.current.state.phase).toBe("idle");
  });

  it("asks for mic permission and moves to recording on success", async () => {
    const { result } = renderHook(() => useVoiceRecorder());

    await act(async () => {
      await result.current.start();
    });

    expect(getUserMedia).toHaveBeenCalledWith({ audio: true });
    expect(result.current.state.phase).toBe("recording");
  });

  it("moves to denied when getUserMedia rejects, with an upload fallback available", async () => {
    getUserMedia.mockRejectedValue(new DOMException("denied"));
    const { result } = renderHook(() => useVoiceRecorder());

    await act(async () => {
      await result.current.start();
    });

    expect(result.current.state.phase).toBe("denied");
  });

  it("reports elapsed time and level bars while recording", async () => {
    const { result } = renderHook(() => useVoiceRecorder());
    await act(async () => {
      await result.current.start();
    });

    await act(async () => {
      vi.advanceTimersByTime(300);
    });

    expect(result.current.state.elapsedMs).toBeGreaterThan(0);
    expect(result.current.state.levels.length).toBe(12);
    expect(result.current.state.levels[0]).toBe(100);
  });

  it("produces a stopped blob on stop()", async () => {
    const { result } = renderHook(() => useVoiceRecorder());
    await act(async () => {
      await result.current.start();
    });

    await act(async () => {
      result.current.stop();
    });

    expect(result.current.state.phase).toBe("stopped");
    expect(result.current.state.blob).toBeInstanceOf(Blob);
  });

  it("discards the clip and returns to idle on cancel(), without a stopped blob", async () => {
    const { result } = renderHook(() => useVoiceRecorder());
    await act(async () => {
      await result.current.start();
    });

    await act(async () => {
      result.current.cancel();
    });

    expect(result.current.state).toEqual({ phase: "idle", elapsedMs: 0, levels: [], blob: null });
  });

  it("auto-stops at the 2-minute cap and calls the onAutoStop callback", async () => {
    const onAutoStop = vi.fn();
    const { result } = renderHook(() => useVoiceRecorder(onAutoStop));
    await act(async () => {
      await result.current.start();
    });

    await act(async () => {
      vi.advanceTimersByTime(121_000);
    });

    expect(onAutoStop).toHaveBeenCalledTimes(1);
    expect(result.current.state.phase).toBe("stopped");
  });

  it("stops the media tracks once recording ends", async () => {
    const { result } = renderHook(() => useVoiceRecorder());
    let stream: FakeStream | undefined;
    getUserMedia.mockImplementation(() => {
      stream = new FakeStream();
      return Promise.resolve(stream);
    });

    await act(async () => {
      await result.current.start();
    });
    await act(async () => {
      result.current.stop();
    });

    expect(stream!.tracks.every((t) => t.stopped)).toBe(true);
  });
});
