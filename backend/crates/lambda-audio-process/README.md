# lambda-audio-process

S3 `ObjectCreated` trigger for voice clips. Reads an uploaded original from the `audio-uploads/` prefix, decodes it with `ffmpeg` to get its duration and waveform peaks, transcodes it to mono Opus in a WebM container, writes the result to the processed bucket under `aud/*`, and marks the `AudioMedia` row `ready`. See [`docs/design/media.md` §12](../../../docs/design/media.md#12-voice-clips).

## Environment

`TABLE_NAME`, `MEDIA_ORIGINALS_BUCKET`, `PROCESSED_BUCKET`, `FFMPEG_PATH` (default `/opt/bin/ffmpeg` — the infra stack ships a static `ffmpeg` binary in a Lambda layer at that path).

Unlike `lambda-image-process`, there is only one originals bucket (voice clips share the media-originals bucket images use, under a different prefix) and no `ffprobe` call: the decode pass that produces the waveform also yields the duration.

## Tests

`cargo test -p lambda-audio-process` runs the pure-logic unit tests (`src/process.rs`, `src/key_parse.rs`, `src/transcode.rs`) and the DynamoDB Local handler tests (`tests/handler.rs`, against a fake transcoder — no `ffmpeg` needed) without Docker beyond what `persistence::test_factories` already requires.

`tests/ffmpeg_real.rs` exercises the real `ffmpeg` binary end to end: it generates a sine-tone WAV in Rust (no binary fixtures) and runs the real decode/transcode passes. It locates `ffmpeg` via `FFMPEG_PATH` or `PATH`; if neither resolves it skips with a note, unless `REQUIRE_FFMPEG=1` is set, in which case it panics (CI sets this after installing `ffmpeg`).
