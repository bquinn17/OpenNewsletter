#!/usr/bin/env node
// Generates the PWA icon/badge PNGs in `public/` with no image-editing tool
// involved — a hand-rolled PNG encoder (IHDR/IDAT/IEND chunks, deflate via
// Node's built-in `zlib`) drawing a simple ring glyph in the app's brand
// colors (see `frontend/index.html` / `public/favicon.svg`). Stand-in
// artwork until a real design lands; re-run with `node scripts/generate-icons.mjs`
// after editing the colors/sizes below. Per `plans/04-frontend-architecture.md`
// §14.1 and `plans/07-notifications.md` §12 (monochrome Android badge).
import { deflateSync } from "node:zlib";
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const publicDir = path.join(__dirname, "..", "public");

// Brand coral from `tailwind.config.ts` (`colors.coral`).
const CORAL = [0xff, 0x5a, 0x6b];
const WHITE = [0xff, 0xff, 0xff];

const SIGNATURE = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);

const CRC_TABLE = (() => {
  const table = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) {
      c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    }
    table[n] = c;
  }
  return table;
})();

function crc32(buf) {
  let crc = 0xffffffff;
  for (const byte of buf) {
    crc = CRC_TABLE[(crc ^ byte) & 0xff] ^ (crc >>> 8);
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const typeBuf = Buffer.from(type, "ascii");
  const lenBuf = Buffer.alloc(4);
  lenBuf.writeUInt32BE(data.length, 0);
  const crcBuf = Buffer.alloc(4);
  crcBuf.writeUInt32BE(crc32(Buffer.concat([typeBuf, data])), 0);
  return Buffer.concat([lenBuf, typeBuf, data, crcBuf]);
}

/** Encodes an 8-bit RGBA pixel grid (row-major, 4 bytes/pixel) as a PNG buffer. */
function encodePng(width, height, pixels) {
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 6; // color type: RGBA
  ihdr[10] = 0; // compression method
  ihdr[11] = 0; // filter method
  ihdr[12] = 0; // interlace method

  const stride = width * 4;
  const raw = Buffer.alloc((stride + 1) * height);
  for (let y = 0; y < height; y++) {
    const rowStart = y * (stride + 1);
    raw[rowStart] = 0; // per-scanline filter: none
    pixels.copy(raw, rowStart + 1, y * stride, y * stride + stride);
  }
  const idat = deflateSync(raw);

  return Buffer.concat([
    SIGNATURE,
    chunk("IHDR", ihdr),
    chunk("IDAT", idat),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

/**
 * A centered ring ("O", echoing `favicon.svg`'s wordmark) on a solid
 * background. `transparentBg` drops the background fill, leaving only the
 * opaque white ring on a transparent canvas — the shape the Android
 * notification tray tints itself for the monochrome badge.
 */
function drawRingIcon(size, { transparentBg = false } = {}) {
  const pixels = Buffer.alloc(size * size * 4);
  const cx = size / 2;
  const cy = size / 2;
  // Ring sits well within the ~80%-diameter maskable safe zone.
  const outerR = size * 0.34;
  const innerR = size * 0.21;

  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const dx = x + 0.5 - cx;
      const dy = y + 0.5 - cy;
      const dist = Math.sqrt(dx * dx + dy * dy);
      const onRing = dist <= outerR && dist >= innerR;
      const offset = (y * size + x) * 4;

      const rgba = onRing ? [...WHITE, 0xff] : transparentBg ? [0, 0, 0, 0] : [...CORAL, 0xff];
      pixels[offset] = rgba[0];
      pixels[offset + 1] = rgba[1];
      pixels[offset + 2] = rgba[2];
      pixels[offset + 3] = rgba[3];
    }
  }
  return pixels;
}

function writeIcon(name, size, opts) {
  const png = encodePng(size, size, drawRingIcon(size, opts));
  writeFileSync(path.join(publicDir, name), png);
  console.log(`wrote public/${name} (${size}x${size}, ${png.length} bytes)`);
}

writeIcon("icon-192.png", 192, {});
writeIcon("icon-512.png", 512, {});
// Maskable: the background already fills edge-to-edge and the ring sits
// inside the safe zone, so the same artwork satisfies both purposes.
writeIcon("icon-maskable-512.png", 512, {});
// iOS ignores alpha on this one and matters about it being fully opaque.
writeIcon("apple-touch-icon.png", 180, {});
// Android notification badge — monochrome glyph, transparent elsewhere.
writeIcon("badge-96.png", 96, { transparentBg: true });
