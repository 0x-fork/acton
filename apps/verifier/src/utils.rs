/// Shortens a code hash for display, keeping its first and last six characters.
#[must_use]
pub fn shorten_code_hash(code_hash: &str) -> String {
    let code_hash = code_hash.trim();
    match (
        code_hash.get(..6),
        code_hash.get(code_hash.len().saturating_sub(6)..),
    ) {
        (Some(prefix), Some(suffix)) if code_hash.len() > 12 => {
            format!("{prefix}..{suffix}")
        }
        _ => code_hash.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::shorten_code_hash;

    #[test]
    fn shorten_code_hash_keeps_six_characters_at_each_end() {
        for (code_hash, expected) in [
            (
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                "012345..abcdef",
            ),
            (
                "f4SmFiL+Y0Yxu0Uw1zIfTDQ/JT6Qje03EmHlaQQWDK8=",
                "f4SmFi..QWDK8=",
            ),
        ] {
            assert_eq!(shorten_code_hash(code_hash), expected);
        }
    }

    #[test]
    fn shorten_code_hash_preserves_strings_up_to_twelve_characters() {
        for code_hash in ["", "a", "abcde", "abcdef", "abcdef12345", "abcdef123456"] {
            assert_eq!(shorten_code_hash(code_hash), code_hash);
        }
    }

    #[test]
    fn shorten_code_hash_shortens_thirteen_characters() {
        assert_eq!(shorten_code_hash("abcdef0123456"), "abcdef..123456");
    }

    #[test]
    fn shorten_code_hash_trims_surrounding_whitespace() {
        for (code_hash, expected) in [
            (" \tabcdef0123456\r\n", "abcdef..123456"),
            (" \tabcdef123456\r\n", "abcdef123456"),
            (" \tabc\r\n", "abc"),
            (" \t\r\n", ""),
        ] {
            assert_eq!(shorten_code_hash(code_hash), expected);
        }
    }

    #[test]
    fn shorten_code_hash_does_not_split_utf8_characters() {
        for code_hash in ["abcde🦀0123456789", "0123456789🦀abcde", "🦀🦀🦀🦀"] {
            assert_eq!(shorten_code_hash(code_hash), code_hash);
        }
    }
}
