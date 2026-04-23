use super::Encoder;

/// URI-reserved characters encoder.
///
/// This [`Encoder`] encodes characters that are reserved in the syntax of URI
/// according to [RFC 3986](https://tools.ietf.org/html/rfc3986).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UriReserved {
    Any,
    Path,
    Query,
    Fragment,
}

impl UriReserved {
    fn is_reserved_but_safe(&self, c: char) -> bool {
        match self {
            Self::Any => false,
            Self::Path => is_sub_delim(c) || c == '@',
            Self::Query | Self::Fragment => is_sub_delim(c) || matches!(c, ':' | '@' | '/' | '?'),
        }
    }
}

impl Encoder for UriReserved {
    fn encode(&self, c: char) -> bool {
        !is_unreserved(c) && !self.is_reserved_but_safe(c)
    }

    #[inline]
    fn encode_ascii(&self, b: u8) -> Option<bool> {
        // b < 0x80 guaranteed by caller.
        Some(self.keep_table()[b as usize] == 0)
    }

    #[inline]
    fn ascii_keep_table(&self) -> Option<&'static [u8; 128]> {
        Some(self.keep_table())
    }
}

impl UriReserved {
    #[inline]
    fn keep_table(&self) -> &'static [u8; 128] {
        match self {
            Self::Any => &URI_KEEP_ANY,
            Self::Path => &URI_KEEP_PATH,
            Self::Query => &URI_KEEP_QUERY,
            Self::Fragment => &URI_KEEP_FRAGMENT,
        }
    }
}

fn is_sub_delim(c: char) -> bool {
    matches!(c, '!' | '$' | '&' | '\'' | '(' | ')' | '*' | '+' | ',' | ';' | '=')
}

fn is_unreserved(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_' | '~')
}

// --- Keep tables -----------------------------------------------------------
//
// A 1 means "byte is ASCII and must NOT be percent-encoded"; 0 means "must be
// encoded". `%` itself is always 0 so the scanner always treats it as a break.

const fn build_keep_table(variant: u8) -> [u8; 128] {
    let mut t = [0u8; 128];
    let mut i = 0u16;
    while i < 128 {
        let b = i as u8;
        let unreserved = matches!(
            b,
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'.'
            | b'_'
            | b'~'
        );
        let sub_delim = matches!(b, b'!' | b'$' | b'&' | b'\'' | b'(' | b')' | b'*' | b'+' | b',' | b';' | b'=');
        let safe = match variant {
            0 => false,                                                   // Any
            1 => sub_delim || b == b'@',                                  // Path
            2 | 3 => sub_delim || matches!(b, b':' | b'@' | b'/' | b'?'), // Query/Fragment
            _ => false,
        };
        let keep = unreserved || safe;
        // Percent must always be encoded.
        t[i as usize] = if keep && b != b'%' { 1 } else { 0 };
        i += 1;
    }
    t
}

pub(crate) static URI_KEEP_ANY: [u8; 128] = build_keep_table(0);
pub(crate) static URI_KEEP_PATH: [u8; 128] = build_keep_table(1);
pub(crate) static URI_KEEP_QUERY: [u8; 128] = build_keep_table(2);
pub(crate) static URI_KEEP_FRAGMENT: [u8; 128] = build_keep_table(3);

#[cfg(test)]
mod tests {
    use crate::PctString;

    use super::*;

    #[test]
    fn uri_encode_cyrillic() {
        let encoder = UriReserved::Any;
        let pct_string = PctString::encode("традиционное польское блюдо\0".chars(), encoder);
        assert_eq!(&pct_string, &"традиционное польское блюдо\0");
        assert_eq!(
            &pct_string.as_str(),
            &"%D1%82%D1%80%D0%B0%D0%B4%D0%B8%D1%86%D0%B8%D0%BE%D0%BD%D0%BD%D0%BE%D0%B5%20%D0%BF%D0%BE%D0%BB%D1%8C%D1%81%D0%BA%D0%BE%D0%B5%20%D0%B1%D0%BB%D1%8E%D0%B4%D0%BE%00"
        );
    }

    #[test]
    fn keep_table_matches_encode() {
        for variant in [UriReserved::Any, UriReserved::Path, UriReserved::Query, UriReserved::Fragment] {
            for b in 0u8..128 {
                let from_table = variant.ascii_keep_table().unwrap()[b as usize] != 0;
                let from_encode = !variant.encode(b as char);
                // `%` must always be encoded regardless of variant rules.
                let expected = from_encode && b != b'%';
                assert_eq!(from_table, expected, "variant {:?} byte {:#x}", variant, b);
            }
        }
    }
}
