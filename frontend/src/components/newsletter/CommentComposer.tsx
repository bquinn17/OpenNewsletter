import { useRef, useState, type ClipboardEvent, type DragEvent } from "react";
import { apiErrorMessage } from "../../api/client";
import { useAddComment } from "../../api/mutations";
import { env } from "../../env";
import type { GiphyGif } from "../../giphy/giphyClient";
import { giphyLimitRemainingMs, useGiphyLimit } from "../../state/giphyLimit";
import { isVoiceRecordingSupported, MAX_AUDIO_CLIPS_PER_COMMENT } from "../../utils/voiceClip";
import { AttachMenu, type AttachMenuHandle, type AttachMenuItem } from "../responses/AttachMenu";
import { GiphyPicker } from "../responses/GiphyPicker";
import {
  ImageUploader,
  type ImageUploaderHandle,
  type UploaderImage,
} from "../responses/ImageUploader";
import {
  VoiceClipUploader,
  type UploaderClip,
  type VoiceClipUploaderHandle,
} from "../responses/VoiceClipUploader";
import { COUNTER_THRESHOLD, MAX_BODY_CHARS } from "./commentFormatting";

type Props = {
  groupId: string;
  cycleId: string;
  questionId: string;
  responseId: string;
};

function minutesLeft(ms: number): number {
  return Math.max(1, Math.ceil(ms / 60_000));
}

export function CommentComposer({ groupId, cycleId, questionId, responseId }: Props) {
  const [body, setBody] = useState("");
  const [images, setImages] = useState<UploaderImage[]>([]);
  const [clips, setClips] = useState<UploaderClip[]>([]);
  const [gif, setGif] = useState<GiphyGif | null>(null);
  const [gifPickerOpen, setGifPickerOpen] = useState(false);
  const [dragOver, setDragOver] = useState(false);
  // Bumped after a post to remount the uploaders: they own their own attachment lists, and the
  // posted image/clip would otherwise stay in its single slot.
  const [uploaderKey, setUploaderKey] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const addComment = useAddComment(groupId, cycleId, questionId);
  const uploaderRef = useRef<ImageUploaderHandle>(null);
  const clipUploaderRef = useRef<VoiceClipUploaderHandle>(null);
  const attachMenuRef = useRef<AttachMenuHandle>(null);
  const giphyUntil = useGiphyLimit((s) => s.until);
  const giphyRateLimited = giphyUntil !== null && giphyUntil > Date.now();

  const readyImage = images.find((img) => img.status === "ready");
  const clip = clips[0];
  const uploading =
    images.some((img) => img.status === "uploading" || img.status === "processing") ||
    clips.some((c) => c.status === "uploading" || c.status === "processing");
  const trimmed = body.trim();
  const canPost =
    (!!trimmed || !!readyImage || !!clip || !!gif) && !uploading && !addComment.isPending;

  async function submit() {
    if (!canPost) return;
    setError(null);
    const withGif = gif
      ? `${trimmed}${trimmed ? "\n\n" : ""}![${gif.title}](giphy:${gif.id})`
      : trimmed;
    try {
      await addComment.mutateAsync({
        responseId,
        body: {
          body: withGif,
          ...(readyImage ? { imageMediaId: readyImage.imageId } : {}),
          ...(clip ? { audioMediaId: clip.audioId } : {}),
        },
      });
      setBody("");
      setImages([]);
      setClips([]);
      setGif(null);
      setUploaderKey((k) => k + 1);
    } catch (err) {
      setError(apiErrorMessage(err, "Couldn't post your comment — try again."));
    }
  }

  function handleDrop(e: DragEvent<HTMLDivElement>) {
    e.preventDefault();
    setDragOver(false);
    const files = Array.from(e.dataTransfer.files);
    uploaderRef.current?.acceptFiles(files.filter((f) => f.type.startsWith("image/")));
    clipUploaderRef.current?.acceptFiles(files.filter((f) => f.type.startsWith("audio/")));
  }

  function handlePaste(e: ClipboardEvent<HTMLTextAreaElement>) {
    const files = Array.from(e.clipboardData?.files ?? []);
    const imageFiles = files.filter((f) => f.type.startsWith("image/"));
    const audioFiles = files.filter((f) => f.type.startsWith("audio/"));
    if (imageFiles.length > 0) uploaderRef.current?.acceptFiles(imageFiles);
    if (audioFiles.length > 0) clipUploaderRef.current?.acceptFiles(audioFiles);
  }

  const imageAtLimit = images.length >= 1;
  const clipAtLimit = clips.length >= MAX_AUDIO_CLIPS_PER_COMMENT;
  const attachItems: AttachMenuItem[] = [
    {
      key: "photo",
      icon: "📷",
      label: "Photo",
      subtitle: imageAtLimit ? "Already attached" : "Attach a photo",
      disabled: imageAtLimit,
      onSelect: () => uploaderRef.current?.openPicker(),
    },
  ];
  if (isVoiceRecordingSupported()) {
    attachItems.push({
      key: "record-voice-clip",
      icon: "🎙️",
      label: "Record a voice clip",
      subtitle: clipAtLimit ? "Already attached" : "Tap to start recording",
      disabled: clipAtLimit,
      onSelect: () => clipUploaderRef.current?.startRecording(),
    });
  }
  attachItems.push({
    key: "upload-audio",
    icon: "🎵",
    label: "Upload audio",
    subtitle: clipAtLimit ? "Already attached" : "Pick an audio file",
    disabled: clipAtLimit,
    onSelect: () => clipUploaderRef.current?.openPicker(),
  });
  if (env.giphyApiKey) {
    attachItems.push({
      key: "gif",
      icon: "GIF",
      label: "GIF",
      subtitle: giphyRateLimited
        ? `GIF limit reached · try again in ${minutesLeft(giphyLimitRemainingMs())} min`
        : gif
          ? "Already attached"
          : "Search GIPHY",
      disabled: !!gif || giphyRateLimited,
      onSelect: () => setGifPickerOpen(true),
    });
  }

  return (
    <div
      className="relative mt-2 space-y-2"
      onDragOver={(e) => {
        e.preventDefault();
        setDragOver(true);
      }}
      onDragLeave={() => setDragOver(false)}
      onDrop={handleDrop}
    >
      {dragOver && (
        <div className="absolute inset-0 z-10 flex items-center justify-center rounded-2xl bg-grape/10 text-sm font-semibold text-grape">
          Drop to attach
        </div>
      )}
      <label htmlFor={`compose-comment-${responseId}`} className="sr-only">
        Add a comment
      </label>
      <textarea
        id={`compose-comment-${responseId}`}
        value={body}
        onChange={(e) => setBody(e.target.value.slice(0, MAX_BODY_CHARS))}
        onPaste={handlePaste}
        placeholder="Add a comment…"
        maxLength={MAX_BODY_CHARS}
        className="min-h-[44px] w-full rounded-2xl border border-line bg-cream px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-coral/30"
      />
      {body.length > COUNTER_THRESHOLD && (
        <div className="text-right text-xs text-inkmuted">
          {body.length}/{MAX_BODY_CHARS}
        </div>
      )}
      <ImageUploader
        key={`image-${uploaderKey}`}
        ref={uploaderRef}
        groupId={groupId}
        cycleId={cycleId}
        questionId={questionId}
        purpose="comment"
        maxImages={1}
        onChange={setImages}
        inputLabel="Add a photo"
      />
      <VoiceClipUploader
        key={`audio-${uploaderKey}`}
        ref={clipUploaderRef}
        groupId={groupId}
        cycleId={cycleId}
        questionId={questionId}
        purpose="comment"
        maxClips={MAX_AUDIO_CLIPS_PER_COMMENT}
        onChange={setClips}
        inputLabel="Add a voice clip"
      />
      {gif && (
        <div className="flex items-center gap-2 rounded-2xl border border-line bg-cream p-2 text-xs">
          <img
            src={gif.stillUrl || gif.videoUrl}
            alt={gif.title}
            referrerPolicy="no-referrer"
            crossOrigin="anonymous"
            className="h-10 w-10 rounded-lg object-cover"
          />
          <span className="flex-1 truncate font-semibold text-ink">{gif.title || "GIF"}</span>
          <button
            type="button"
            onClick={() => setGif(null)}
            aria-label="Remove GIF"
            className="grid h-6 w-6 place-items-center rounded-full bg-ink text-xs font-bold text-cream"
          >
            ×
          </button>
        </div>
      )}
      {error && (
        <p role="alert" className="text-xs font-semibold text-coral">
          {error}
        </p>
      )}
      <div className="flex items-center justify-between gap-2">
        <AttachMenu
          ref={attachMenuRef}
          items={attachItems}
          ariaLabel="Add a photo, voice clip, or GIF"
        />
        <button
          type="button"
          onClick={() => void submit()}
          disabled={!canPost}
          className="rounded-full bg-grape px-4 py-2 text-xs font-bold text-white disabled:cursor-not-allowed disabled:opacity-40"
        >
          {addComment.isPending ? "Posting…" : "Post"}
        </button>
      </div>

      {gifPickerOpen && (
        <GiphyPicker
          anchorRef={{ current: attachMenuRef.current?.getAnchorEl() ?? null }}
          onPick={(picked) => {
            setGif(picked);
            setGifPickerOpen(false);
          }}
          onClose={() => setGifPickerOpen(false)}
        />
      )}
    </div>
  );
}
