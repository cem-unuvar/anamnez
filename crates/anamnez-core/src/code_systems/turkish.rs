//! Turkish-locale casefold: `İ → i`, `I → ı`, NFC. Diacritics preserved
//! (`ş`, `ç` distinct from `s`, `c`).
//!
//! Used to pre-fold both write and query paths before they touch FTS5, since the
//! default `unicode61` tokenizer does not handle Turkish dotted/dotless I.

use unicode_normalization::UnicodeNormalization;

/// Apply Turkish-aware lowercase + NFC normalization. Idempotent on already-folded input.
///
/// Rules:
/// - `İ` (U+0130, capital dotted I) → `i` (U+0069)
/// - `I` (U+0049, capital dotless I) → `ı` (U+0131)
/// - other chars → Unicode default lowercase
/// - output normalized to NFC
/// - diacritics preserved (`ş`/`s`, `ç`/`c` are distinct letters)
#[must_use]
pub fn casefold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            'İ' => out.push('i'),
            'I' => out.push('ı'),
            _ => {
                for lower in ch.to_lowercase() {
                    out.push(lower);
                }
            }
        }
    }
    out.nfc().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dotted_i_folds_to_lowercase_i() {
        assert_eq!(casefold("İlaç"), "ilaç");
    }

    #[test]
    fn dotless_capital_i_folds_to_dotless_lower_i() {
        assert_eq!(casefold("Irmak"), "ırmak");
    }

    #[test]
    fn diacritics_preserved() {
        assert_eq!(casefold("Şeker"), "şeker");
        assert_eq!(casefold("Çocuk"), "çocuk");
        // ş is not folded to s
        assert_ne!(casefold("şeker"), "seker");
        // ç is not folded to c
        assert_ne!(casefold("çocuk"), "cocuk");
    }

    #[test]
    fn idempotent() {
        let folded = casefold("İlaç");
        assert_eq!(casefold(&folded), folded);
    }

    #[test]
    fn ascii_passthrough() {
        assert_eq!(casefold("Hello"), "hello");
    }
}
