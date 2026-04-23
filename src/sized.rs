use std::{
    borrow::Borrow,
    cmp::Ordering,
    fmt::{self, Display, Formatter},
    hash::{Hash, Hasher},
    str::FromStr,
};

use crate::{Encoder, InvalidPctString, PctStr, scan::scan_keep_run, util::find_percent};

/// Owned, mutable percent-encoded string.
///
/// This is the equivalent of [`String`] for percent-encoded strings.
/// It implements [`Deref`](`std::ops::Deref`) to [`PctStr`] meaning that all methods on [`PctStr`] slices are
/// available on `PctString` values as well.
pub struct PctString(Vec<u8>);

impl PctString {
    /// Create a new owned percent-encoded string.
    ///
    /// The input string is checked for correct percent-encoding.
    /// If the test fails, a [`InvalidPctString`] error is returned.
    pub fn new<B: Into<Vec<u8>>>(bytes: B) -> Result<Self, InvalidPctString<Vec<u8>>> {
        let bytes = bytes.into();
        if find_percent(&bytes).is_none() {
            return match core::str::from_utf8(&bytes) {
                Ok(_) => Ok(Self(bytes)),
                Err(_) => Err(InvalidPctString(bytes)),
            };
        }
        if PctStr::validate(bytes.iter().copied()) {
            Ok(Self(bytes))
        } else {
            Err(InvalidPctString(bytes))
        }
    }

    pub fn from_string(string: String) -> Result<Self, InvalidPctString<String>> {
        Self::new(string).map_err(|e| {
            e.map(|bytes| unsafe {
                // SAFETY: the bytes come from the UTF-8 encoded input `string`.
                String::from_utf8_unchecked(bytes)
            })
        })
    }

    /// Creates a new owned percent-encoded string without validation.
    ///
    /// # Safety
    ///
    /// The input string must be correctly percent-encoded.
    pub unsafe fn new_unchecked<B: Into<Vec<u8>>>(bytes: B) -> Self {
        Self(bytes.into())
    }

    /// Encode a string into a percent-encoded string.
    ///
    /// This function takes an [`Encoder`] instance to decide which character of the string must
    /// be encoded.
    ///
    /// Note that the character `%` will always be encoded regardless of the provided [`Encoder`].
    ///
    /// # Example
    ///
    /// ```
    /// use pct::{PctString, UriReserved};
    ///
    /// let pct_string = PctString::encode("Hello World!".chars(), UriReserved::Any);
    /// println!("{}", pct_string.as_str()); // => Hello World%21
    /// ```
    pub fn encode<E: Encoder>(src: impl IntoIterator<Item = char>, encoder: E) -> PctString {
        static HEX: &[u8; 16] = b"0123456789ABCDEF";

        let iter = src.into_iter();
        let mut out = Vec::with_capacity(iter.size_hint().0);
        let mut ubuf = [0u8; 4];
        for c in iter {
            if encoder.encode(c) || c == '%' {
                let s = c.encode_utf8(&mut ubuf);
                for &b in s.as_bytes() {
                    out.push(b'%');
                    out.push(HEX[(b >> 4) as usize]);
                    out.push(HEX[(b & 0x0F) as usize]);
                }
            } else if c.is_ascii() {
                out.push(c as u8);
            } else {
                out.extend_from_slice(c.encode_utf8(&mut ubuf).as_bytes());
            }
        }

        PctString(out)
    }

    /// Byte-oriented encode path.
    ///
    /// Walks the input as bytes, skipping 8-byte plain runs via SWAR when the
    /// encoder exposes an ASCII keep table (`UriReserved`, `IriReserved`),
    /// and falls back to the per-char loop otherwise. Multi-byte UTF-8
    /// sequences are handled in one batch.
    pub fn encode_bytes<E: Encoder>(src: &str, encoder: E) -> PctString {
        static HEX: &[u8; 16] = b"0123456789ABCDEF";

        let bytes = src.as_bytes();
        let mut out: Vec<u8> = Vec::with_capacity(bytes.len());

        if let Some(table) = encoder.ascii_keep_table() {
            let mut i = 0usize;
            while i < bytes.len() {
                let run_end = scan_keep_run(bytes, i, table);
                if run_end > i {
                    out.extend_from_slice(&bytes[i..run_end]);
                    i = run_end;
                    if i >= bytes.len() {
                        break;
                    }
                }
                let b = bytes[i];
                if b < 0x80 {
                    // Break byte: `%` or a table-declined ASCII byte.
                    out.push(b'%');
                    out.push(HEX[(b >> 4) as usize]);
                    out.push(HEX[(b & 0x0F) as usize]);
                    i += 1;
                } else {
                    // Multi-byte UTF-8 sequence starts here.
                    let start = i;
                    i += 1;
                    while i < bytes.len() && (bytes[i] & 0xC0) == 0x80 {
                        i += 1;
                    }
                    let seq = &bytes[start..i];
                    // SAFETY: `src` is `&str`, so `seq` is a valid UTF-8 sequence.
                    let c = unsafe { core::str::from_utf8_unchecked(seq) }
                        .chars()
                        .next()
                        .unwrap();
                    if encoder.encode(c) {
                        for &byte in seq {
                            out.push(b'%');
                            out.push(HEX[(byte >> 4) as usize]);
                            out.push(HEX[(byte & 0x0F) as usize]);
                        }
                    } else {
                        out.extend_from_slice(seq);
                    }
                }
            }
        } else {
            // Custom encoder without a keep table → per-char fallback.
            let mut ubuf = [0u8; 4];
            for c in src.chars() {
                if encoder.encode(c) || c == '%' {
                    let s = c.encode_utf8(&mut ubuf);
                    for &b in s.as_bytes() {
                        out.push(b'%');
                        out.push(HEX[(b >> 4) as usize]);
                        out.push(HEX[(b & 0x0F) as usize]);
                    }
                } else if c.is_ascii() {
                    out.push(c as u8);
                } else {
                    out.extend_from_slice(c.encode_utf8(&mut ubuf).as_bytes());
                }
            }
        }

        PctString(out)
    }

    /// Return this string as a borrowed percent-encoded string slice.
    #[inline]
    pub fn as_pct_str(&self) -> &PctStr {
        unsafe {
            // SAFETY: the bytes have been validated.
            PctStr::new_unchecked(&self.0)
        }
    }

    /// Return the internal string of the [`PctString`], consuming it
    #[inline]
    pub fn into_string(self) -> String {
        unsafe {
            // SAFETY: the bytes have been validated, and a percent-encoded
            //         string is a valid UTF-8 string.
            String::from_utf8_unchecked(self.0)
        }
    }

    #[inline]
    pub fn into_bytes(self) -> Vec<u8> {
        self.0
    }
}

impl std::ops::Deref for PctString {
    type Target = PctStr;

    #[inline]
    fn deref(&self) -> &PctStr {
        self.as_pct_str()
    }
}

impl Borrow<PctStr> for PctString {
    fn borrow(&self) -> &PctStr {
        self.as_pct_str()
    }
}

impl AsRef<PctStr> for PctString {
    fn as_ref(&self) -> &PctStr {
        self.as_pct_str()
    }
}

impl AsRef<str> for PctString {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<[u8]> for PctString {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl PartialEq for PctString {
    #[inline]
    fn eq(&self, other: &PctString) -> bool {
        **self == **other
    }
}

impl Eq for PctString {}

impl PartialEq<PctStr> for PctString {
    #[inline]
    fn eq(&self, other: &PctStr) -> bool {
        **self == *other
    }
}

impl PartialEq<&str> for PctString {
    #[inline]
    fn eq(&self, other: &&str) -> bool {
        **self == **other
    }
}

impl PartialEq<str> for PctString {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        **self == *other
    }
}

impl PartialOrd for PctString {
    fn partial_cmp(&self, other: &PctString) -> Option<Ordering> {
        self.as_pct_str().partial_cmp(other.as_pct_str())
    }
}

impl PartialOrd<PctStr> for PctString {
    fn partial_cmp(&self, other: &PctStr) -> Option<Ordering> {
        self.as_pct_str().partial_cmp(other)
    }
}

impl Hash for PctString {
    #[inline]
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        (**self).hash(hasher)
    }
}

impl Display for PctString {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Display::fmt(self.as_str(), f)
    }
}

impl fmt::Debug for PctString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}

impl FromStr for PctString {
    type Err = InvalidPctString<String>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_string(s.to_string())
    }
}

impl TryFrom<String> for PctString {
    type Error = InvalidPctString<String>;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_string(value)
    }
}

impl<'a> TryFrom<&'a str> for PctString {
    type Error = InvalidPctString<String>;

    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        Self::from_string(value.to_owned())
    }
}

impl<'a> TryFrom<&'a str> for &'a PctStr {
    type Error = InvalidPctString<&'a str>;

    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        PctStr::new(value)
    }
}

#[cfg(test)]
mod encode_bytes_tests {
    use super::*;
    use crate::{IriReserved, UriReserved};

    fn check<E: Encoder + Copy>(src: &str, enc: E) {
        let per_char = PctString::encode(src.chars(), enc);
        let byte_path = PctString::encode_bytes(src, enc);
        assert_eq!(per_char.as_str(), byte_path.as_str(), "src={:?}", src);
    }

    #[test]
    fn parity_ascii_mixed() {
        check("Hello, world!", UriReserved::Any);
        check("foo/bar?baz=qux&x=y", UriReserved::Path);
        check("abc 123 !@#$%^&*()", UriReserved::Query);
        check("", UriReserved::Any);
        check("%already%encoded%20", UriReserved::Any);
    }

    #[test]
    fn parity_unicode() {
        check("традиционное польское блюдо", UriReserved::Any);
        check("中文 日本語 한국어", UriReserved::Any);
        check("😀🚀🌍", UriReserved::Any);
        check("традиционное польское блюдо", IriReserved::Path);
        check("?test=中文&private=\u{10FFFD}", IriReserved::Query);
        check("?test=中文&private=\u{10FFFD}", IriReserved::Fragment);
    }

    #[test]
    fn parity_custom_encoder() {
        struct UppercaseToo;
        impl Encoder for UppercaseToo {
            fn encode(&self, c: char) -> bool {
                UriReserved::Any.encode(c) || c.is_uppercase()
            }
        }
        let src = "Hello World!";
        let per_char = PctString::encode(src.chars(), UppercaseToo);
        let byte_path = PctString::encode_bytes(src, UppercaseToo);
        assert_eq!(per_char.as_str(), byte_path.as_str());
    }
}
