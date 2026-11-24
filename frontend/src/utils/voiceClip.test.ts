import { describe, expect, it } from "vitest";
import {
  bucketLevels,
  bucketPeaks,
  canPlayOpusWebm,
  clipLimitMessage,
  durationWords,
  formatClipTime,
  isVoiceRecordingSupported,
  nextPlaybackRate,
  normalizeAudioMimeType,
  pickRecordingMimeType,
  voiceClipLabel,
  voiceClipValueText,
} from "./voiceClip";

describe("formatClipTime", () => {
  it("formats seconds and minutes as m:ss", () => {
    expect(formatClipTime(0)).toBe("0:00");
    expect(formatClipTime(12_000)).toBe("0:12");
    expect(formatClipTime(64_000)).toBe("1:04");
    expect(formatClipTime(120_000)).toBe("2:00");
  });
});

describe("durationWords", () => {
  it("spells out seconds alone", () => {
    expect(durationWords(12_000)).toBe("12 seconds");
    expect(durationWords(1_000)).toBe("1 second");
  });

  it("spells out minutes and seconds together", () => {
    expect(durationWords(64_000)).toBe("1 minute 4 seconds");
  });

  it("spells out an exact minute with no seconds", () => {
    expect(durationWords(60_000)).toBe("1 minute");
  });

  it("pluralizes minutes", () => {
    expect(durationWords(125_000)).toBe("2 minutes 5 seconds");
  });
});

describe("voiceClipLabel / voiceClipValueText", () => {
  it("builds the accessible label from the author and duration", () => {
    expect(voiceClipLabel("Alex", 64_000)).toBe("Voice clip from Alex, 1 minute 4 seconds");
  });

  it("builds the aria-valuetext as position of total", () => {
    expect(voiceClipValueText(12_000, 64_000)).toBe("12 seconds of 1 minute 4 seconds");
  });
});

describe("nextPlaybackRate", () => {
  it("cycles 1x -> 1.5x -> 2x -> 1x", () => {
    expect(nextPlaybackRate(1)).toBe(1.5);
    expect(nextPlaybackRate(1.5)).toBe(2);
    expect(nextPlaybackRate(2)).toBe(1);
  });
});

describe("canPlayOpusWebm", () => {
  it("reads true when the element reports support", () => {
    expect(canPlayOpusWebm({ canPlayType: () => "probably" })).toBe(true);
  });

  it("reads false for an empty string", () => {
    expect(canPlayOpusWebm({ canPlayType: () => "" })).toBe(false);
  });
});

describe("bucketPeaks", () => {
  it("scales the loudest bucket to 255", () => {
    const samples = new Float32Array([0.1, 0.2, 0.4, 0.8]);
    const peaks = bucketPeaks(samples, 2);
    expect(peaks).toEqual([64, 255]);
  });

  it("returns all zeros for a silent clip, with no division by zero", () => {
    expect(bucketPeaks(new Float32Array([0, 0, 0, 0]), 4)).toEqual([0, 0, 0, 0]);
  });

  it("returns all zeros for an empty buffer", () => {
    expect(bucketPeaks(new Float32Array([]), 4)).toEqual([0, 0, 0, 0]);
  });
});

describe("bucketLevels", () => {
  it("averages each bucket", () => {
    expect(bucketLevels(new Uint8Array([10, 20, 30, 40]), 2)).toEqual([15, 35]);
  });

  it("returns zeros for empty data", () => {
    expect(bucketLevels(new Uint8Array([]), 3)).toEqual([0, 0, 0]);
  });
});

describe("normalizeAudioMimeType", () => {
  it("strips parameters and lowercases", () => {
    expect(normalizeAudioMimeType("audio/webm;codecs=opus")).toBe("audio/webm");
    expect(normalizeAudioMimeType("AUDIO/MP4")).toBe("audio/mp4");
  });
});

describe("clipLimitMessage", () => {
  it("pluralizes correctly", () => {
    expect(clipLimitMessage(1)).toBe("You can attach up to 1 voice clip.");
    expect(clipLimitMessage(3)).toBe("You can attach up to 3 voice clips.");
  });
});

describe("pickRecordingMimeType", () => {
  it("prefers opus-in-webm when supported", () => {
    expect(pickRecordingMimeType((t) => t === "audio/webm;codecs=opus")).toBe(
      "audio/webm;codecs=opus",
    );
  });

  it("falls back to audio/mp4", () => {
    expect(pickRecordingMimeType((t) => t === "audio/mp4")).toBe("audio/mp4");
  });

  it("returns undefined when neither is supported", () => {
    expect(pickRecordingMimeType(() => false)).toBeUndefined();
  });
});

describe("isVoiceRecordingSupported", () => {
  it("is false when MediaRecorder isn't defined in this environment (jsdom)", () => {
    expect(isVoiceRecordingSupported()).toBe(false);
  });
});
