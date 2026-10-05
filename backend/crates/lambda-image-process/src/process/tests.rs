use super::*;
use image::{Delay, ImageBuffer, Rgba, RgbaImage};
use pretty_assertions::assert_eq;
use std::io::Cursor as StdCursor;

fn solid_image(width: u32, height: u32) -> DynamicImage {
    let buf: RgbaImage = ImageBuffer::from_fn(width, height, |x, y| {
        Rgba([(x % 256) as u8, (y % 256) as u8, 128, 255])
    });
    DynamicImage::ImageRgba8(buf)
}

fn encode(img: &DynamicImage, format: ImageFormat) -> Vec<u8> {
    let mut buf = StdCursor::new(Vec::new());
    img.write_to(&mut buf, format).expect("fixture encodes");
    buf.into_inner()
}

fn encode_webp_fixture(img: &DynamicImage) -> Vec<u8> {
    let rgba = img.to_rgba8();
    webp::Encoder::from_rgba(rgba.as_raw(), img.width(), img.height())
        .encode(90.0)
        .to_vec()
}

fn encode_gif_fixture(frames: &[DynamicImage], delay_ms: u64) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut encoder = GifEncoder::new(&mut out);
        encoder
            .set_repeat(Repeat::Infinite)
            .expect("gif fixture repeat sets");
        for frame_img in frames {
            let buf = frame_img.to_rgba8();
            let delay = Delay::from_saturating_duration(std::time::Duration::from_millis(delay_ms));
            encoder
                .encode_frame(Frame::from_parts(buf, 0, 0, delay))
                .expect("gif fixture frame encodes");
        }
    }
    out
}

/// Inserts a minimal Exif APP1 segment (TIFF header + one `Orientation` IFD
/// entry) right after SOI — the shape a real camera JPEG carries, trimmed to
/// the one tag `image`'s orientation reader needs.
fn jpeg_with_orientation(img: &DynamicImage, orientation: u16) -> Vec<u8> {
    let raw = encode(img, ImageFormat::Jpeg);

    let mut tiff = Vec::new();
    tiff.extend_from_slice(b"II"); // little-endian byte order mark
    tiff.extend_from_slice(&42u16.to_le_bytes()); // TIFF magic number
    tiff.extend_from_slice(&8u32.to_le_bytes()); // offset to IFD0
    tiff.extend_from_slice(&1u16.to_le_bytes()); // one IFD entry
    tiff.extend_from_slice(&0x0112u16.to_le_bytes()); // tag: Orientation
    tiff.extend_from_slice(&3u16.to_le_bytes()); // type: SHORT
    tiff.extend_from_slice(&1u32.to_le_bytes()); // count: 1
    let mut value_field = [0u8; 4];
    value_field[0..2].copy_from_slice(&orientation.to_le_bytes());
    tiff.extend_from_slice(&value_field);
    tiff.extend_from_slice(&0u32.to_le_bytes()); // no next IFD

    let mut payload = Vec::new();
    payload.extend_from_slice(b"Exif\0\0");
    payload.extend_from_slice(&tiff);

    let mut segment = vec![0xFF, 0xE1];
    let length = (payload.len() + 2) as u16;
    segment.extend_from_slice(&length.to_be_bytes());
    segment.extend_from_slice(&payload);

    let mut out = Vec::new();
    out.extend_from_slice(&raw[0..2]); // SOI
    out.extend_from_slice(&segment);
    out.extend_from_slice(&raw[2..]);
    out
}

fn is_webp(bytes: &[u8]) -> bool {
    bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP"
}

fn is_gif(bytes: &[u8]) -> bool {
    bytes.len() >= 6 && (&bytes[0..6] == b"GIF87a" || &bytes[0..6] == b"GIF89a")
}

fn decoded_dimensions(bytes: &[u8]) -> (u32, u32) {
    let img = image::load_from_memory(bytes).expect("variant bytes decode");
    (img.width(), img.height())
}

#[test]
fn jpeg_input_produces_both_variants_within_their_long_edge() {
    let img = solid_image(2000, 1000);
    let bytes = encode(&img, ImageFormat::Jpeg);

    let variants = process_image(&bytes).expect("jpeg processes");
    assert_eq!((variants.width, variants.height), (2000, 1000));
    assert_eq!(variants.display.format, VariantFormat::Webp);
    assert_eq!(variants.thumb.format, VariantFormat::Webp);
    assert!(is_webp(&variants.display.bytes));
    assert!(is_webp(&variants.thumb.bytes));

    let (dw, dh) = decoded_dimensions(&variants.display.bytes);
    assert!(dw.max(dh) <= shared::config::DISPLAY_LONG_EDGE_PX);
    let (tw, th) = decoded_dimensions(&variants.thumb.bytes);
    assert!(tw.max(th) <= shared::config::THUMB_LONG_EDGE_PX);
}

#[test]
fn png_input_produces_both_variants_within_their_long_edge() {
    let img = solid_image(1600, 900);
    let bytes = encode(&img, ImageFormat::Png);

    let variants = process_image(&bytes).expect("png processes");
    assert!(is_webp(&variants.display.bytes));
    assert!(is_webp(&variants.thumb.bytes));
    let (dw, dh) = decoded_dimensions(&variants.display.bytes);
    assert!(dw.max(dh) <= shared::config::DISPLAY_LONG_EDGE_PX);
}

#[test]
fn webp_input_produces_both_variants_within_their_long_edge() {
    let img = solid_image(1500, 1500);
    let bytes = encode_webp_fixture(&img);

    let variants = process_image(&bytes).expect("webp processes");
    assert!(is_webp(&variants.display.bytes));
    assert!(is_webp(&variants.thumb.bytes));
    let (tw, th) = decoded_dimensions(&variants.thumb.bytes);
    assert!(tw.max(th) <= shared::config::THUMB_LONG_EDGE_PX);
}

#[test]
fn gif_input_produces_both_variants_as_gif() {
    let img = solid_image(900, 500);
    let bytes = encode_gif_fixture(&[img], 10);

    let variants = process_image(&bytes).expect("gif processes");
    assert_eq!(variants.display.format, VariantFormat::Gif);
    assert_eq!(variants.thumb.format, VariantFormat::Gif);
    assert!(is_gif(&variants.display.bytes));
    assert!(is_gif(&variants.thumb.bytes));
    let (tw, th) = decoded_dimensions(&variants.thumb.bytes);
    assert!(tw.max(th) <= shared::config::THUMB_LONG_EDGE_PX);
}

#[test]
fn a_small_image_is_not_upscaled() {
    let img = solid_image(100, 50);
    let bytes = encode(&img, ImageFormat::Jpeg);

    let variants = process_image(&bytes).expect("jpeg processes");
    let (dw, dh) = decoded_dimensions(&variants.display.bytes);
    assert_eq!((dw, dh), (100, 50));
    let (tw, th) = decoded_dimensions(&variants.thumb.bytes);
    assert_eq!((tw, th), (100, 50));
}

#[test]
fn an_animated_gif_round_trips_with_all_its_frames() {
    let frames = vec![
        solid_image(20, 10),
        DynamicImage::ImageRgba8(ImageBuffer::from_pixel(20, 10, Rgba([255, 0, 0, 255]))),
        DynamicImage::ImageRgba8(ImageBuffer::from_pixel(20, 10, Rgba([0, 255, 0, 255]))),
    ];
    let bytes = encode_gif_fixture(&frames, 50);

    let variants = process_image(&bytes).expect("animated gif processes");

    let decoded = GifDecoder::new(StdCursor::new(variants.display.bytes))
        .expect("display gif decodes")
        .into_frames()
        .collect_frames()
        .expect("display frames collect");
    assert_eq!(decoded.len(), 3);
}

#[test]
fn exif_orientation_6_swaps_recorded_width_and_height() {
    let img = solid_image(4, 6);
    let bytes = jpeg_with_orientation(&img, 6);

    let variants = process_image(&bytes).expect("oriented jpeg processes");
    assert_eq!((variants.width, variants.height), (6, 4));
}

#[test]
fn an_undecodable_image_fails_terminally() {
    let err = process_image(b"not an image").unwrap_err();
    assert_eq!(err, ProcessError::DecodeFailed);
}

#[test]
fn avatar_output_is_exactly_256_by_256_webp() {
    let img = solid_image(500, 300);
    let bytes = encode(&img, ImageFormat::Jpeg);

    let variant = process_avatar(&bytes).expect("avatar processes");
    assert!(is_webp(&variant.display.bytes));
    let (w, h) = decoded_dimensions(&variant.display.bytes);
    assert_eq!((w, h), (256, 256));
}

#[test]
fn a_gif_avatar_fails_terminally() {
    let img = solid_image(300, 300);
    let bytes = encode_gif_fixture(&[img], 10);
    let err = process_avatar(&bytes).unwrap_err();
    assert_eq!(err, ProcessError::DecodeFailed);
}
