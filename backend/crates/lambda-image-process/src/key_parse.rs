//! S3 key decoding and parsing (`plans/08-media-uploads.md` §2, §11.1).
//!
//! S3 event records URL-encode the object key (and use `+` for a literal
//! space, the old `application/x-www-form-urlencoded` convention, not plain
//! percent-encoding) — decode before parsing. A hand-rolled decoder avoids a
//! dependency for what is a well-known, small routine.

use domain::{AvatarId, CycleId, GroupId, ImageId, QuestionId, UserId};

/// Decodes an S3 event object key: `+` -> space, then `%XX` percent-escapes.
pub fn decode_s3_key(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => match (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                (Some(hi), Some(lo)) => {
                    out.push(hi * 16 + lo);
                    i += 3;
                }
                _ => {
                    out.push(bytes[i]);
                    i += 1;
                }
            },
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// `uploads/{groupId}/{cycleId}/{questionId}/{userId}/{imageId}.{ext}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedImageKey {
    pub group_id: GroupId,
    pub cycle_id: CycleId,
    pub question_id: QuestionId,
    pub user_id: UserId,
    pub image_id: ImageId,
}

/// `uploads/{userId}/{avatarId}.{ext}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedAvatarKey {
    pub user_id: UserId,
    pub avatar_id: AvatarId,
}

/// Parses an already-decoded originals-bucket key for the per-question image
/// pipeline. `None` on anything malformed — the caller logs and skips the
/// record rather than failing the batch.
pub fn parse_image_key(key: &str) -> Option<ParsedImageKey> {
    let rest = key.strip_prefix("uploads/")?;
    let parts: Vec<&str> = rest.split('/').collect();
    let [group_id, cycle_id, question_id, user_id, filename] = parts[..] else {
        return None;
    };
    let image_id = strip_extension(filename)?;
    if [group_id, cycle_id, question_id, user_id, image_id]
        .iter()
        .any(|p| p.is_empty())
    {
        return None;
    }
    Some(ParsedImageKey {
        group_id: GroupId::new(group_id),
        cycle_id: CycleId::new(cycle_id),
        question_id: QuestionId::new(question_id),
        user_id: UserId::new(user_id),
        image_id: ImageId::new(image_id),
    })
}

/// Parses an already-decoded originals-bucket key for the avatar pipeline.
pub fn parse_avatar_key(key: &str) -> Option<ParsedAvatarKey> {
    let rest = key.strip_prefix("uploads/")?;
    let parts: Vec<&str> = rest.split('/').collect();
    let [user_id, filename] = parts[..] else {
        return None;
    };
    let avatar_id = strip_extension(filename)?;
    if user_id.is_empty() || avatar_id.is_empty() {
        return None;
    }
    Some(ParsedAvatarKey {
        user_id: UserId::new(user_id),
        avatar_id: AvatarId::new(avatar_id),
    })
}

fn strip_extension(filename: &str) -> Option<&str> {
    let id = filename.split('.').next()?;
    if id.is_empty() {
        None
    } else {
        Some(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_percent_and_plus_escapes() {
        assert_eq!(
            decode_s3_key("uploads/a+b/c%2Dd.jpg"),
            "uploads/a b/c-d.jpg"
        );
    }

    #[test]
    fn leaves_a_trailing_percent_without_two_hex_digits_untouched() {
        assert_eq!(decode_s3_key("uploads/weird%2"), "uploads/weird%2");
    }

    #[test]
    fn parses_a_well_formed_image_key() {
        let parsed = parse_image_key("uploads/g1/202606/q1/u1/img-01.jpg").expect("should parse");
        assert_eq!(parsed.group_id, GroupId::new("g1"));
        assert_eq!(parsed.cycle_id, CycleId::new("202606"));
        assert_eq!(parsed.question_id, QuestionId::new("q1"));
        assert_eq!(parsed.user_id, UserId::new("u1"));
        assert_eq!(parsed.image_id, ImageId::new("img-01"));
    }

    #[test]
    fn rejects_an_image_key_with_too_few_segments() {
        assert_eq!(parse_image_key("uploads/g1/202606/q1/img-01.jpg"), None);
    }

    #[test]
    fn rejects_an_image_key_missing_the_uploads_prefix() {
        assert_eq!(parse_image_key("other/g1/202606/q1/u1/img-01.jpg"), None);
    }

    #[test]
    fn parses_a_well_formed_avatar_key() {
        let parsed = parse_avatar_key("uploads/u1/av-01.png").expect("should parse");
        assert_eq!(parsed.user_id, UserId::new("u1"));
        assert_eq!(parsed.avatar_id, AvatarId::new("av-01"));
    }

    #[test]
    fn rejects_an_avatar_key_with_too_many_segments() {
        assert_eq!(parse_avatar_key("uploads/u1/sub/av-01.png"), None);
    }
}
