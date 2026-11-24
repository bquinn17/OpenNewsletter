import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useAudioPlayback } from "../../state/audioPlayback";
import { VoiceClip } from "./VoiceClip";

const { refreshMediaUrls } = vi.hoisted(() => ({ refreshMediaUrls: vi.fn() }));
vi.mock("../../utils/media", () => ({ refreshMediaUrls }));
vi.mock("../../utils/audioCache", () => ({ primeAudioCache: vi.fn() }));

const CLIP = {
  audioId: "aud1",
  url: "https://media.test.invalid/aud/g1/c/q/u/aud1/clip.webm",
  durationMs: 64_000,
  peaks: Array.from({ length: 48 }, (_, i) => (i % 2 === 0 ? 200 : 50)),
  caption: "A short story",
  authorName: "Alex",
};

beforeEach(() => {
  refreshMediaUrls.mockReset().mockResolvedValue(undefined);
  useAudioPlayback.setState({ playingId: null });
  vi.spyOn(HTMLMediaElement.prototype, "play").mockResolvedValue(undefined);
  vi.spyOn(HTMLMediaElement.prototype, "pause").mockImplementation(() => {});
  // jsdom's real `canPlayType` always returns "" (unimplemented); stub it to report support by
  // default, same as a real browser that can play Opus-in-WebM — the dedicated test below
  // overrides this to "" to exercise the unsupported-codec branch.
  vi.spyOn(HTMLMediaElement.prototype, "canPlayType").mockReturnValue("probably");
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("VoiceClip", () => {
  it("shows the accessible label and caption", () => {
    render(<VoiceClip {...CLIP} />);
    expect(
      screen.getByRole("slider", { name: "Voice clip from Alex, 1 minute 4 seconds" }),
    ).toBeInTheDocument();
    expect(screen.getByText("A short story")).toBeInTheDocument();
    expect(screen.getByText("0:00 / 1:04")).toBeInTheDocument();
  });

  it("plays on click of the round button and sets the shared playing id", () => {
    render(<VoiceClip {...CLIP} />);
    fireEvent.click(screen.getByRole("button", { name: "Play voice clip" }));

    expect(useAudioPlayback.getState().playingId).toBe("aud1");
    expect(screen.getByRole("button", { name: "Pause voice clip" })).toBeInTheDocument();
  });

  it("only one clip plays at a time — starting a second pauses the first", () => {
    render(
      <>
        <VoiceClip {...CLIP} />
        <VoiceClip {...CLIP} audioId="aud2" authorName="Sam" />
      </>,
    );

    fireEvent.click(screen.getAllByRole("button", { name: "Play voice clip" })[0]!);
    expect(useAudioPlayback.getState().playingId).toBe("aud1");

    const secondPlay = screen.getByRole("button", { name: "Play voice clip" }); // only aud2's remains unplayed-labeled
    fireEvent.click(secondPlay);

    expect(useAudioPlayback.getState().playingId).toBe("aud2");
    // The first clip's button reverts to "Play" once it's no longer the active one.
    expect(screen.getAllByRole("button", { name: "Play voice clip" })).toHaveLength(1);
  });

  it("seeks 5 seconds with ArrowRight/ArrowLeft on the waveform slider", () => {
    render(<VoiceClip {...CLIP} />);
    const slider = screen.getByRole("slider");

    fireEvent.keyDown(slider, { key: "ArrowRight" });
    expect(slider).toHaveAttribute("aria-valuenow", "5000");
    expect(slider).toHaveAttribute("aria-valuetext", "5 seconds of 1 minute 4 seconds");

    fireEvent.keyDown(slider, { key: "ArrowLeft" });
    expect(slider).toHaveAttribute("aria-valuenow", "0");
  });

  it("toggles play/pause with Space on the slider", () => {
    render(<VoiceClip {...CLIP} />);
    const slider = screen.getByRole("slider");

    fireEvent.keyDown(slider, { key: " " });
    expect(useAudioPlayback.getState().playingId).toBe("aud1");
  });

  it("cycles the speed toggle 1x -> 1.5x -> 2x", () => {
    render(<VoiceClip {...CLIP} />);
    const speedButton = screen.getByRole("button", { name: "1×" });

    fireEvent.click(speedButton);
    expect(screen.getByRole("button", { name: "1.5×" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "1.5×" }));
    expect(screen.getByRole("button", { name: "2×" })).toBeInTheDocument();
  });

  it("shows an offline, disabled play button when there's no url", () => {
    render(<VoiceClip {...CLIP} url={null} />);
    const button = screen.getByRole("button", { name: "Voice clip unavailable offline" });
    expect(button).toBeDisabled();
    expect(screen.getByText(/Available when you.?re back online\./)).toBeInTheDocument();
  });

  it("shows an unsupported-codec message instead of a player when canPlayType is empty", () => {
    vi.spyOn(window.HTMLMediaElement.prototype, "canPlayType").mockReturnValue("");
    render(<VoiceClip {...CLIP} />);

    expect(screen.getByText(/Update your browser to play voice clips/)).toBeInTheDocument();
    expect(screen.queryByRole("slider")).not.toBeInTheDocument();
  });

  it("goes through useMediaRetry on an <audio> error", async () => {
    render(<VoiceClip {...CLIP} />);
    const audio = document.querySelector("audio")!;

    await act(async () => {
      fireEvent.error(audio);
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(refreshMediaUrls).toHaveBeenCalledTimes(1);
  });
});
