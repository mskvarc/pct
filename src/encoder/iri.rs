use super::Encoder;

/// IRI-reserved characters encoder.
///
/// This [`Encoder`] encodes characters that are reserved in the syntax of IRI
/// according to [RFC 3987](https://tools.ietf.org/html/rfc3987).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IriReserved {
    /// Encode characters reserved in any IRI component.
    Any,

    /// Encode characters reserved in a path segment.
    Path,

    /// Encode characters reserved in a query.
    Query,

    /// Encode characters reserved in a fragment.
    Fragment,
}

impl IriReserved {
    fn is_reserved_but_safe(&self, c: char) -> bool {
        match self {
            Self::Any => false,
            Self::Path => is_sub_delim(c) || c == '@',
            Self::Query => is_sub_delim(c) || is_iprivate(c) || matches!(c, ':' | '@' | '/' | '?'),
            Self::Fragment => is_sub_delim(c) || matches!(c, ':' | '@' | '/' | '?'),
        }
    }
}

impl Encoder for IriReserved {
    fn encode(&self, c: char) -> bool {
        !is_iunreserved(c) && !self.is_reserved_but_safe(c)
    }
}

fn is_sub_delim(c: char) -> bool {
    matches!(c, '!' | '$' | '&' | '\'' | '(' | ')' | '*' | '+' | ',' | ';' | '=')
}

fn is_iprivate(c: char) -> bool {
    matches!(c, '\u{E000}'..='\u{F8FF}' | '\u{F0000}'..='\u{FFFFD}' | '\u{100000}'..='\u{10FFFD}')
}

fn is_iunreserved(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_' | '~') || is_ucschar(c)
}

fn is_ucschar(c: char) -> bool {
    matches!(c,
          '\u{00A0}'..='\u{0D7FF}'
        | '\u{F900}'..='\u{FDCF}'
        | '\u{FDF0}'..='\u{FFEF}'
        | '\u{10000}'..='\u{1FFFD}'
        | '\u{20000}'..='\u{2FFFD}'
        | '\u{30000}'..='\u{3FFFD}'
        | '\u{40000}'..='\u{4FFFD}'
        | '\u{50000}'..='\u{5FFFD}'
        | '\u{60000}'..='\u{6FFFD}'
        | '\u{70000}'..='\u{7FFFD}'
        | '\u{80000}'..='\u{8FFFD}'
        | '\u{90000}'..='\u{9FFFD}'
        | '\u{A0000}'..='\u{AFFFD}'
        | '\u{B0000}'..='\u{BFFFD}'
        | '\u{C0000}'..='\u{CFFFD}'
        | '\u{D0000}'..='\u{DFFFD}'
        | '\u{E1000}'..='\u{EFFFD}'
    )
}

#[cfg(test)]
mod tests {
    use crate::PctString;

    use super::IriReserved;

    #[test]
    fn iri_encode_cyrillic() {
        let encoder = IriReserved::Path;
        let pct_string = PctString::encode("традиционное польское блюдо".chars(), encoder);
        assert_eq!(&pct_string, &"традиционное польское блюдо");
        assert_eq!(&pct_string.as_str(), &"традиционное%20польское%20блюдо");
    }

    #[test]
    fn iri_encode_segment() {
        let encoder = IriReserved::Path;
        let pct_string = PctString::encode("?test=традиционное польское блюдо&cjk=真正&private=\u{10FFFD}".chars(), encoder);

        assert_eq!(&pct_string, &"?test=традиционное польское блюдо&cjk=真正&private=\u{10FFFD}");
        assert_eq!(&pct_string.as_str(), &"%3Ftest=традиционное%20польское%20блюдо&cjk=真正&private=%F4%8F%BF%BD");
    }

    #[test]
    fn iri_encode_segment_nocolon() {
        let encoder = IriReserved::Path;
        let pct_string = PctString::encode("?test=традиционное польское блюдо&cjk=真正&private=\u{10FFFD}".chars(), encoder);
        assert_eq!(&pct_string, &"?test=традиционное польское блюдо&cjk=真正&private=\u{10FFFD}");
        assert_eq!(&pct_string.as_str(), &"%3Ftest=традиционное%20польское%20блюдо&cjk=真正&private=%F4%8F%BF%BD");
    }

    #[test]
    fn iri_encode_fragment() {
        let encoder = IriReserved::Fragment;
        let pct_string = PctString::encode("?test=традиционное польское блюдо&cjk=真正&private=\u{10FFFD}".chars(), encoder);
        assert_eq!(&pct_string, &"?test=традиционное польское блюдо&cjk=真正&private=\u{10FFFD}");
        assert_eq!(&pct_string.as_str(), &"?test=традиционное%20польское%20блюдо&cjk=真正&private=%F4%8F%BF%BD");
    }

    #[test]
    fn iri_encode_query() {
        let encoder = IriReserved::Query;
        let pct_string = PctString::encode("?test=традиционное польское блюдо&cjk=真正&private=\u{10FFFD}".chars(), encoder);
        assert_eq!(&pct_string, &"?test=традиционное польское блюдо&cjk=真正&private=\u{10FFFD}");
        assert_eq!(&pct_string.as_str(), &"?test=традиционное%20польское%20блюдо&cjk=真正&private=\u{10FFFD}");
    }
}
