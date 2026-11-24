//! S3 key decoding and parsing (`docs/design/media.md` §12).
//!
//! Mirrors `lambda-image-process::key_parse`: S3 event records URL-encode
//! the object key (and use `+` for a literal space), so this decodes before
//! parsing.

use domain::{AudioId, CycleId, GroupId, QuestionId, UserId};

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

/// `audio-uploads/{groupId}/{cycleId}/{questionId}/{userId}/{audioId}.{ext}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedAudioKey {
    pub group_id: GroupId,
    pub cycle_id: CycleId,
    pub question_id: QuestionId,
    pub user_id: UserId,
    pub audio_id: AudioId,
}

/// Parses an already-decoded originals-bucket key for the voice-clip
/// pipeline. `None` on anything malformed — the caller logs and skips the
/// record rather than failing the batch.
pub fn parse_audio_key(key: &str) -> Option<ParsedAudioKey> {
    let rest = key.strip_prefix("audio-uploads/")?;
    let parts: Vec<&str> = rest.split('/').collect();
    let [group_id, cycle_id, question_id, user_id, filename] = parts[..] else {
        return None;
    };
    let audio_id = strip_extension(filename)?;
    if [group_id, cycle_id, question_id, user_id, audio_id]
        .iter()
        .any(|p| p.is_empty())
    {
        return None;
    }
    Some(ParsedAudioKey {
        group_id: GroupId::new(group_id),
        cycle_id: CycleId::new(cycle_id),
        question_id: QuestionId::new(question_id),
        user_id: UserId::new(user_id),
        audio_id: AudioId::new(audio_id),
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
            decode_s3_key("audio-uploads/a+b/c%2Dd.webm"),
            "audio-uploads/a b/c-d.webm"
        );
    }

    #[test]
    fn parses_a_well_formed_audio_key() {
        let parsed =
            parse_audio_key("audio-uploads/g1/202606/q1/u1/aud-01.webm").expect("should parse");
        assert_eq!(parsed.group_id, GroupId::new("g1"));
        assert_eq!(parsed.cycle_id, CycleId::new("202606"));
        assert_eq!(parsed.question_id, QuestionId::new("q1"));
        assert_eq!(parsed.user_id, UserId::new("u1"));
        assert_eq!(parsed.audio_id, AudioId::new("aud-01"));
    }

    #[test]
    fn rejects_a_key_with_too_few_segments() {
        assert_eq!(
            parse_audio_key("audio-uploads/g1/202606/q1/aud-01.webm"),
            None
        );
    }

    #[test]
    fn rejects_a_key_missing_the_audio_uploads_prefix() {
        assert_eq!(parse_audio_key("uploads/g1/202606/q1/u1/aud-01.webm"), None);
    }
}
