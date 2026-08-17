//! Invite-code generation.
//!
//! Crockford base32 without `I`, `L`, `O` or `U`, so codes survive being read
//! aloud or retyped. 16 characters over a 32-symbol alphabet is 80 bits of
//! entropy, which is what makes guessing codes infeasible
//! (`plans/05-auth-flow.md` §12).

use domain::InviteCode;
use rand::Rng;
use shared::config::INVITE_CODE_LEN;

const ALPHABET: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const GROUP_LEN: usize = 4;

pub fn generate() -> InviteCode {
    let mut rng = rand::thread_rng();
    let raw: String = (0..INVITE_CODE_LEN)
        .map(|_| ALPHABET[rng.gen_range(0..ALPHABET.len())] as char)
        .collect();
    InviteCode::new(raw)
}

/// Group the code into hyphenated quads for display: `ABCD-EFGH-JKMN-PQRS`.
pub fn format_for_display(code: &InviteCode) -> String {
    code.as_str()
        .as_bytes()
        .chunks(GROUP_LEN)
        .map(|chunk| String::from_utf8_lossy(chunk).into_owned())
        .collect::<Vec<_>>()
        .join("-")
}

/// Accept a code with or without display hyphens, in any case.
pub fn normalize(raw: &str) -> InviteCode {
    InviteCode::new(
        raw.chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .map(|c| c.to_ascii_uppercase())
            .collect::<String>(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_codes_use_only_the_unambiguous_alphabet() {
        let code = generate();
        assert_eq!(code.as_str().len(), INVITE_CODE_LEN);
        assert!(code.as_str().bytes().all(|byte| ALPHABET.contains(&byte)));
    }

    #[test]
    fn generated_codes_differ() {
        assert_ne!(generate(), generate());
    }

    #[test]
    fn display_form_is_hyphenated_quads() {
        let code = InviteCode::new("ABCDEFGHJKMNPQRS");
        assert_eq!(format_for_display(&code), "ABCD-EFGH-JKMN-PQRS");
    }

    #[test]
    fn normalize_strips_hyphens_and_upcases() {
        assert_eq!(
            normalize("abcd-efgh-jkmn-pqrs"),
            InviteCode::new("ABCDEFGHJKMNPQRS")
        );
    }
}
