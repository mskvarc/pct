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
}

fn is_sub_delim(c: char) -> bool {
	matches!(
		c,
		'!' | '$' | '&' | '\'' | '(' | ')' | '*' | '+' | ',' | ';' | '='
	)
}

fn is_unreserved(c: char) -> bool {
	c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_' | '~')
}

#[cfg(test)]
mod tests {
	use crate::PctString;

	use super::UriReserved;

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
}
