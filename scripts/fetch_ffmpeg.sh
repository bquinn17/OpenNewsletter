#!/usr/bin/env bash
# Put a static arm64 ffmpeg in the audio-process Lambda layer.
#
# MediaPipelineStack builds the layer from backend/target/ffmpeg-layer, so the
# binary lands at /opt/bin/ffmpeg. That directory holds only bin/ffmpeg: CDK
# zips all of it, so the download cache lives next to it, not inside it.
# The build is John Van Sickle's, pinned by version and SHA-256
# (docs/decisions.md, "Voice clips are transcoded by a static ffmpeg").
#
# Usage: scripts/fetch_ffmpeg.sh   (from anywhere in the repo; idempotent)
set -euo pipefail

FFMPEG_VERSION="7.0.2"
FFMPEG_SHA256="f4149bb2b0784e30e99bdda85471c9b5930d3402014e934a5098b41d0f7201b1"

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
layer_bin="$repo_root/backend/target/ffmpeg-layer/bin"
cache_dir="$repo_root/backend/target/ffmpeg-cache"
name="ffmpeg-${FFMPEG_VERSION}-arm64-static"
tarball="$cache_dir/$name.tar.xz"
stamp="$cache_dir/$name.installed"

if [ -x "$layer_bin/ffmpeg" ] && [ "$(cat "$stamp" 2>/dev/null)" = "$FFMPEG_SHA256" ]; then
    echo "ffmpeg $FFMPEG_VERSION already in the layer."
    exit 0
fi

mkdir -p "$cache_dir" "$layer_bin"

sha_of() { shasum -a 256 "$1" | awk '{print $1}'; }

if [ ! -f "$tarball" ] || [ "$(sha_of "$tarball")" != "$FFMPEG_SHA256" ]; then
    # The current release lives under releases/; once superseded it moves to
    # old-releases/, which keeps every version.
    downloaded=0
    for url in \
        "https://johnvansickle.com/ffmpeg/releases/$name.tar.xz" \
        "https://www.johnvansickle.com/ffmpeg/old-releases/$name.tar.xz"; do
        echo "Downloading $url"
        if curl -fsSL "$url" -o "$tarball.tmp"; then
            mv "$tarball.tmp" "$tarball"
            downloaded=1
            break
        fi
    done
    if [ "$downloaded" -eq 0 ]; then
        rm -f "$tarball.tmp"
        echo "Couldn't download $name from either URL." >&2
        exit 1
    fi
fi

actual="$(sha_of "$tarball")"
if [ "$actual" != "$FFMPEG_SHA256" ]; then
    rm -f "$tarball"
    echo "SHA-256 mismatch for $name: expected $FFMPEG_SHA256, got $actual." >&2
    exit 1
fi

tar -xJf "$tarball" -C "$cache_dir" "$name/ffmpeg"
install -m 0755 "$cache_dir/$name/ffmpeg" "$layer_bin/ffmpeg"
echo "$FFMPEG_SHA256" > "$stamp"
echo "ffmpeg $FFMPEG_VERSION is in the layer at backend/target/ffmpeg-layer/bin/ffmpeg."
