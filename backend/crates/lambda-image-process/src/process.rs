//! Pure image processing — no AWS, no I/O. `handler.rs` wires this to S3 and
//! the DynamoDB row updates; this module is what the unit tests in
//! `plans/08-media-uploads.md` §10 exercise directly.

use image::codecs::gif::{GifDecoder, GifEncoder, Repeat};
use image::codecs::jpeg::JpegDecoder;
use image::codecs::png::PngDecoder;
use image::codecs::webp::WebPDecoder;
use image::{AnimationDecoder, DynamicImage, Frame, ImageDecoder, ImageFormat};
use std::io::Cursor;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ProcessError {
    #[error("could not decode image data")]
    DecodeFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariantFormat {
    Webp,
    Gif,
}

impl VariantFormat {
    pub fn extension(self) -> &'static str {
        match self {
            VariantFormat::Webp => "webp",
            VariantFormat::Gif => "gif",
        }
    }

    pub fn content_type(self) -> &'static str {
        match self {
            VariantFormat::Webp => "image/webp",
            VariantFormat::Gif => "image/gif",
        }
    }
}

#[derive(Debug, Clone)]
pub struct EncodedVariant {
    pub bytes: Vec<u8>,
    pub format: VariantFormat,
}

#[derive(Debug, Clone)]
pub struct ImageVariants {
    /// Oriented original's dimensions — what gets recorded on the row.
    pub width: u32,
    pub height: u32,
    pub display: EncodedVariant,
    pub thumb: EncodedVariant,
}

/// Decodes `bytes` (sniffing the format — never trusting a filename
/// extension), applies EXIF orientation, and produces the display (1200px
/// long edge, q82) and thumb (400px long edge, q75) variants. GIFs are
/// resized frame-by-frame and stay GIF so animation keeps working; every
/// other supported format becomes lossy WebP. Never upscales.
pub fn process_image(bytes: &[u8]) -> Result<ImageVariants, ProcessError> {
    let format = image::guess_format(bytes).map_err(|_| ProcessError::DecodeFailed)?;

    if format == ImageFormat::Gif {
        return process_animated_gif(bytes);
    }

    let (orientation, mut img) = decode_oriented(format, bytes)?;
    img.apply_orientation(orientation);
    let (width, height) = (img.width(), img.height());

    let display = encode_webp_variant(
        &resize_long_edge(&img, shared::config::DISPLAY_LONG_EDGE_PX),
        shared::config::DISPLAY_QUALITY,
    );
    let thumb = encode_webp_variant(
        &resize_long_edge(&img, shared::config::THUMB_LONG_EDGE_PX),
        shared::config::THUMB_QUALITY,
    );

    Ok(ImageVariants {
        width,
        height,
        display,
        thumb,
    })
}

#[derive(Debug, Clone)]
pub struct AvatarVariant {
    pub display: EncodedVariant,
}

/// Center-crops to a square, resizes to 256x256, and encodes lossy WebP q82.
/// Only `jpeg`/`png`/`webp` are accepted; a GIF (or anything else) is a
/// decode failure — animated avatars aren't supported
/// (`plans/08-media-uploads.md` §11.1).
pub fn process_avatar(bytes: &[u8]) -> Result<AvatarVariant, ProcessError> {
    let format = image::guess_format(bytes).map_err(|_| ProcessError::DecodeFailed)?;
    if format == ImageFormat::Gif {
        return Err(ProcessError::DecodeFailed);
    }

    let (orientation, mut img) = decode_oriented(format, bytes)?;
    img.apply_orientation(orientation);

    let cropped = center_crop_to_square(&img);
    let resized = cropped.resize_exact(
        shared::config::AVATAR_SIZE_PX,
        shared::config::AVATAR_SIZE_PX,
        image::imageops::FilterType::Lanczos3,
    );
    let display = encode_webp_variant(&resized, shared::config::AVATAR_QUALITY);

    Ok(AvatarVariant { display })
}

fn decode_oriented(
    format: ImageFormat,
    bytes: &[u8],
) -> Result<(image::metadata::Orientation, DynamicImage), ProcessError> {
    match format {
        ImageFormat::Jpeg => {
            let mut decoder =
                JpegDecoder::new(Cursor::new(bytes)).map_err(|_| ProcessError::DecodeFailed)?;
            let orientation = decoder
                .orientation()
                .unwrap_or(image::metadata::Orientation::NoTransforms);
            let img =
                DynamicImage::from_decoder(decoder).map_err(|_| ProcessError::DecodeFailed)?;
            Ok((orientation, img))
        }
        ImageFormat::Png => {
            let mut decoder =
                PngDecoder::new(Cursor::new(bytes)).map_err(|_| ProcessError::DecodeFailed)?;
            let orientation = decoder
                .orientation()
                .unwrap_or(image::metadata::Orientation::NoTransforms);
            let img =
                DynamicImage::from_decoder(decoder).map_err(|_| ProcessError::DecodeFailed)?;
            Ok((orientation, img))
        }
        ImageFormat::WebP => {
            let mut decoder =
                WebPDecoder::new(Cursor::new(bytes)).map_err(|_| ProcessError::DecodeFailed)?;
            let orientation = decoder
                .orientation()
                .unwrap_or(image::metadata::Orientation::NoTransforms);
            let img =
                DynamicImage::from_decoder(decoder).map_err(|_| ProcessError::DecodeFailed)?;
            Ok((orientation, img))
        }
        _ => Err(ProcessError::DecodeFailed),
    }
}

/// Resizes so the long edge is at most `max_long_edge`, preserving aspect
/// ratio. Never upscales — an image already at or under the target keeps its
/// size (`plans/08-media-uploads.md` §1).
fn resize_long_edge(img: &DynamicImage, max_long_edge: u32) -> DynamicImage {
    let (width, height) = (img.width(), img.height());
    let long_edge = width.max(height);
    if long_edge <= max_long_edge {
        return img.clone();
    }
    let scale = f64::from(max_long_edge) / f64::from(long_edge);
    let new_width = ((f64::from(width) * scale).round() as u32).max(1);
    let new_height = ((f64::from(height) * scale).round() as u32).max(1);
    img.resize(new_width, new_height, image::imageops::FilterType::Lanczos3)
}

fn center_crop_to_square(img: &DynamicImage) -> DynamicImage {
    let (width, height) = (img.width(), img.height());
    let side = width.min(height);
    let x = (width - side) / 2;
    let y = (height - side) / 2;
    img.crop_imm(x, y, side, side)
}

/// Lossy WebP via `libwebp` (through the `webp` crate) — `image`'s own WebP
/// encoder only writes lossless, which can't honor a target quality.
fn encode_webp_variant(img: &DynamicImage, quality: f32) -> EncodedVariant {
    let rgba = img.to_rgba8();
    let encoder = webp::Encoder::from_rgba(rgba.as_raw(), img.width(), img.height());
    let bytes = encoder.encode(quality).to_vec();
    EncodedVariant {
        bytes,
        format: VariantFormat::Webp,
    }
}

fn process_animated_gif(bytes: &[u8]) -> Result<ImageVariants, ProcessError> {
    let (width, height) = gif_dimensions(bytes)?;
    let display = resize_gif(bytes, shared::config::DISPLAY_LONG_EDGE_PX)?;
    let thumb = resize_gif(bytes, shared::config::THUMB_LONG_EDGE_PX)?;
    Ok(ImageVariants {
        width,
        height,
        display,
        thumb,
    })
}

fn gif_dimensions(bytes: &[u8]) -> Result<(u32, u32), ProcessError> {
    let decoder = GifDecoder::new(Cursor::new(bytes)).map_err(|_| ProcessError::DecodeFailed)?;
    Ok(decoder.dimensions())
}

fn resize_gif(bytes: &[u8], max_long_edge: u32) -> Result<EncodedVariant, ProcessError> {
    let decoder = GifDecoder::new(Cursor::new(bytes)).map_err(|_| ProcessError::DecodeFailed)?;
    let frames: Vec<Frame> = decoder
        .into_frames()
        .collect_frames()
        .map_err(|_| ProcessError::DecodeFailed)?;

    let mut out = Vec::new();
    {
        let mut encoder = GifEncoder::new(&mut out);
        encoder
            .set_repeat(Repeat::Infinite)
            .map_err(|_| ProcessError::DecodeFailed)?;
        for frame in &frames {
            let delay = frame.delay();
            let resized = resize_long_edge(
                &DynamicImage::ImageRgba8(frame.buffer().clone()),
                max_long_edge,
            )
            .to_rgba8();
            let resized_frame = Frame::from_parts(resized, 0, 0, delay);
            encoder
                .encode_frame(resized_frame)
                .map_err(|_| ProcessError::DecodeFailed)?;
        }
    }

    Ok(EncodedVariant {
        bytes: out,
        format: VariantFormat::Gif,
    })
}

#[cfg(test)]
mod tests;
