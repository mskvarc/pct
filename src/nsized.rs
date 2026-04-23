use core::{
    cmp::Ordering,
    fmt::{self, Debug, Display, Formatter},
    hash::{Hash, Hasher},
};

use crate::{
    InvalidPctString,
    util::{HEX_VAL, TryEncodedBytes, find_percent},
};

#[cfg(feature = "std")]
use crate::PctString;

/// Percent-Encoded string slice.
///
/// This is the equivalent of [`str`] for percent-encoded strings.
/// This is an *unsized* type, meaning that it must always be used behind a
/// pointer like `&` or [`Box`]. For an owned version of this type,
/// see [`PctString`].
///
/// # Examples
///
/// ```
/// use pct::PctStr;
///
/// let buffer = "Hello%20World%21";
/// let pct_str = PctStr::new(buffer).unwrap();
///
/// // You can compare percent-encoded strings with a regular string.
/// assert!(pct_str == "Hello World!");
///
/// // The underlying string is unchanged.
/// assert!(pct_str.as_str() == "Hello%20World%21");
///
/// // Just as a regular string, you can iterate over the
/// // encoded characters of `pct_str` with [`PctStr::chars`].
/// for c in pct_str.chars() {
///   print!("{}", c);
/// }
///
/// // You can decode the string and every remove percent-encoded characters
/// // with the [`PctStr::decode`] method.
/// let decoded_string: String = pct_str.decode();
/// println!("{}", decoded_string);
/// ```
pub struct PctStr([u8]);

impl PctStr {
    /// Create a new percent-encoded string slice.
    ///
    /// The input slice is checked for correct percent-encoding.
    /// If the test fails, a [`InvalidPctString`] error is returned.
    pub fn new<S: AsRef<[u8]> + ?Sized>(input: &S) -> Result<&PctStr, InvalidPctString<&S>> {
        let input_bytes = input.as_ref();
        if find_percent(input_bytes).is_none() {
            return match core::str::from_utf8(input_bytes) {
                Ok(_) => Ok(unsafe { Self::new_unchecked(input_bytes) }),
                Err(_) => Err(InvalidPctString(input)),
            };
        }
        match validate_fast(input_bytes) {
            FastCheck::Valid => Ok(unsafe { Self::new_unchecked(input_bytes) }),
            FastCheck::Invalid => Err(InvalidPctString(input)),
            FastCheck::NeedsFullCheck => {
                if Self::validate(input_bytes.iter().copied()) {
                    Ok(unsafe { Self::new_unchecked(input_bytes) })
                } else {
                    Err(InvalidPctString(input))
                }
            }
        }
    }

    /// Create a new percent-encoded string slice without checking for correct encoding.
    ///
    /// This is an unsafe function. The resulting string slice will have an undefined behaviour
    /// if the input slice is not percent-encoded.
    ///
    /// # Safety
    ///
    /// The input `str` must be a valid percent-encoded string.
    pub unsafe fn new_unchecked<S: AsRef<[u8]> + ?Sized>(input: &S) -> &PctStr {
        unsafe { core::mem::transmute::<&[u8], &PctStr>(input.as_ref()) }
    }

    /// Checks that the given iterator produces a valid percent-encoded string.
    pub fn validate(input: impl Iterator<Item = u8>) -> bool {
        let chars = TryEncodedBytes::new(input);
        utf8_decode::TryDecoder::new(chars).all(|r| r.is_ok())
    }

    /// Length of the decoded string (character count).
    ///
    /// Computed in linear time.
    /// This is different from the byte length, which can be retrieved using
    /// `value.as_bytes().len()`.
    #[inline]
    pub fn len(&self) -> usize {
        self.bytes().filter(|b| (b & 0xC0) != 0x80).count()
    }

    /// Checks if the string is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns the underlying percent-encoding bytes.
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Get the underlying percent-encoded string slice.
    #[inline]
    pub fn as_str(&self) -> &str {
        unsafe {
            // SAFETY: the data has be validated, and all percent-encoded
            //         strings are valid UTF-8 strings.
            core::str::from_utf8_unchecked(&self.0)
        }
    }

    /// Iterate over the encoded characters of the string.
    #[inline]
    pub fn chars(&self) -> Chars<'_> {
        Chars::new(self.bytes())
    }

    /// Iterate over the encoded bytes of the string.
    #[inline]
    pub fn bytes(&self) -> Bytes<'_> {
        Bytes(self.0.iter())
    }

    /// Iterate over bytes in RFC 3986 §6.2.2.2 normalized form:
    /// percent-encoded *unreserved* octets are decoded in place, all other
    /// `%XX` triples stay encoded with uppercase hex digits.
    #[inline]
    pub fn bytes_rfc3986(&self) -> Rfc3986Bytes<'_> {
        Rfc3986Bytes::new(&self.0)
    }

    /// RFC 3986 §6.2.2.2 equality: two strings compare equal iff their
    /// `bytes_rfc3986` streams are byte-identical. Unlike [`PartialEq`],
    /// pct-encoded *reserved* octets stay encoded (so `"%2F"` ≠ `"/"`).
    pub fn eq_rfc3986(&self, other: &PctStr) -> bool {
        if self.0 == other.0 {
            return true;
        }
        self.bytes_rfc3986().eq(other.bytes_rfc3986())
    }

    /// RFC 3986 §6.2.2.2 ordering. See [`PctStr::eq_rfc3986`].
    pub fn cmp_rfc3986(&self, other: &PctStr) -> Ordering {
        self.bytes_rfc3986().cmp(other.bytes_rfc3986())
    }

    /// RFC 3986 §6.2.2.2 hash. Bulk-writes plain runs between `%`s and, for
    /// each triplet, writes either the decoded unreserved byte or the three
    /// bytes of the (uppercased) `%XX`.
    pub fn hash_rfc3986<H: Hasher>(&self, hasher: &mut H) {
        let bytes: &[u8] = &self.0;
        #[cfg(feature = "memchr")]
        {
            if find_percent(bytes).is_none() {
                hasher.write(bytes);
                return;
            }
            let mut prev = 0usize;
            for pct in memchr::memchr_iter(b'%', bytes) {
                hasher.write(&bytes[prev..pct]);
                let h1 = bytes[pct + 1];
                let h2 = bytes[pct + 2];
                let a = HEX_VAL[h1 as usize];
                let c = HEX_VAL[h2 as usize];
                debug_assert!(a != 0xFF && c != 0xFF);
                let decoded = (a << 4) | c;
                if is_unreserved_byte(decoded) {
                    hasher.write(&[decoded]);
                } else {
                    hasher.write(&[b'%', to_upper_hex_byte(h1), to_upper_hex_byte(h2)]);
                }
                prev = pct + 3;
            }
            hasher.write(&bytes[prev..]);
        }
        #[cfg(not(feature = "memchr"))]
        {
            for b in self.bytes_rfc3986() {
                b.hash(hasher);
            }
        }
    }

    /// Decoding.
    ///
    /// Return the string with the percent-encoded characters decoded.
    #[cfg(feature = "std")]
    pub fn decode(&self) -> String {
        let bytes: &[u8] = &self.0;
        if find_percent(bytes).is_none() {
            return self.as_str().to_owned();
        }
        let mut out = Vec::with_capacity(bytes.len());

        #[cfg(feature = "memchr")]
        {
            let mut prev = 0usize;
            for pct in memchr::memchr_iter(b'%', bytes) {
                // SAFETY-ish: PctStr invariant guarantees %XX with valid hex.
                out.extend_from_slice(&bytes[prev..pct]);
                let a = crate::util::HEX_VAL[bytes[pct + 1] as usize];
                let b = crate::util::HEX_VAL[bytes[pct + 2] as usize];
                debug_assert!(a != 0xFF && b != 0xFF);
                out.push((a << 4) | b);
                prev = pct + 3;
            }
            out.extend_from_slice(&bytes[prev..]);
        }
        #[cfg(not(feature = "memchr"))]
        {
            for b in self.bytes() {
                out.push(b);
            }
        }

        unsafe {
            // SAFETY: decoded bytes form valid UTF-8 because `validate` passed.
            String::from_utf8_unchecked(out)
        }
    }
}

/// Outcome of [`validate_fast`].
pub(crate) enum FastCheck {
    /// Triplets valid and decoded output is pure ASCII → UTF-8 valid by construction.
    Valid,
    /// Triplet was malformed (incomplete or bad hex).
    Invalid,
    /// Triplets well-formed but some non-ASCII byte is present in input or
    /// decoded output — caller must run the full UTF-8 validator.
    NeedsFullCheck,
}

/// Walk a byte slice, verify every `%XX` triplet has valid hex, and track
/// whether any non-ASCII byte appears in the input or in decoded triplet
/// values. When everything stays ASCII, the decoded output is trivially
/// UTF-8; otherwise the caller runs the full UTF-8 validator.
#[inline]
pub(crate) fn validate_fast(bytes: &[u8]) -> FastCheck {
    use crate::util::HEX_VAL;
    let n = bytes.len();
    let mut had_non_ascii: u8 = 0;

    #[cfg(feature = "memchr")]
    {
        let mut last = 0usize;
        for pct in memchr::memchr_iter(b'%', bytes) {
            // Scan the plain run before this '%' for non-ASCII.
            for &b in &bytes[last..pct] {
                had_non_ascii |= b & 0x80;
            }
            if pct + 2 >= n {
                return FastCheck::Invalid;
            }
            let a = HEX_VAL[bytes[pct + 1] as usize];
            let c = HEX_VAL[bytes[pct + 2] as usize];
            if a | c == 0xFF {
                return FastCheck::Invalid;
            }
            had_non_ascii |= ((a << 4) | c) & 0x80;
            last = pct + 3;
        }
        for &b in &bytes[last..] {
            had_non_ascii |= b & 0x80;
        }
    }
    #[cfg(not(feature = "memchr"))]
    {
        let mut i = 0usize;
        while i < n {
            let b = bytes[i];
            if b == b'%' {
                if i + 2 >= n {
                    return FastCheck::Invalid;
                }
                let a = HEX_VAL[bytes[i + 1] as usize];
                let c = HEX_VAL[bytes[i + 2] as usize];
                if a | c == 0xFF {
                    return FastCheck::Invalid;
                }
                had_non_ascii |= ((a << 4) | c) & 0x80;
                i += 3;
            } else {
                had_non_ascii |= b & 0x80;
                i += 1;
            }
        }
    }

    if had_non_ascii == 0 { FastCheck::Valid } else { FastCheck::NeedsFullCheck }
}

impl PartialEq for PctStr {
    #[inline]
    fn eq(&self, other: &PctStr) -> bool {
        if self.0 == other.0 {
            return true;
        }
        self.bytes().eq(other.bytes())
    }
}

impl Eq for PctStr {}

impl PartialEq<str> for PctStr {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        self.bytes().eq(other.as_bytes().iter().copied())
    }
}

#[cfg(feature = "std")]
impl PartialEq<PctString> for PctStr {
    #[inline]
    fn eq(&self, other: &PctString) -> bool {
        self == other.as_pct_str()
    }
}

impl PartialOrd for PctStr {
    fn partial_cmp(&self, other: &PctStr) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PctStr {
    fn cmp(&self, other: &PctStr) -> Ordering {
        self.bytes().cmp(other.bytes())
    }
}

#[cfg(feature = "std")]
impl PartialOrd<PctString> for PctStr {
    fn partial_cmp(&self, other: &PctString) -> Option<Ordering> {
        self.partial_cmp(other.as_pct_str())
    }
}

impl Hash for PctStr {
    #[inline]
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        let bytes: &[u8] = &self.0;
        #[cfg(feature = "memchr")]
        {
            if find_percent(bytes).is_none() {
                hasher.write(bytes);
                return;
            }
            let mut prev = 0usize;
            for pct in memchr::memchr_iter(b'%', bytes) {
                hasher.write(&bytes[prev..pct]);
                // SAFETY-ish: PctStr invariant guarantees %XX with valid hex.
                let a = crate::util::HEX_VAL[bytes[pct + 1] as usize];
                let b = crate::util::HEX_VAL[bytes[pct + 2] as usize];
                debug_assert!(a != 0xFF && b != 0xFF);
                hasher.write(&[(a << 4) | b]);
                prev = pct + 3;
            }
            hasher.write(&bytes[prev..]);
        }
        #[cfg(not(feature = "memchr"))]
        {
            for b in self.bytes() {
                b.hash(hasher)
            }
        }
    }
}

impl Display for PctStr {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), f)
    }
}

impl Debug for PctStr {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Debug::fmt(self.as_str(), f)
    }
}

#[cfg(feature = "std")]
impl ToOwned for PctStr {
    type Owned = PctString;

    fn to_owned(&self) -> Self::Owned {
        unsafe { PctString::new_unchecked(self.0.to_owned()) }
    }
}

impl AsRef<str> for PctStr {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<[u8]> for PctStr {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

/// Bytes iterator.
///
/// Iterates over the decoded bytes of a percent-encoded string.
pub struct Bytes<'a>(core::slice::Iter<'a, u8>);

impl<'a> Iterator for Bytes<'a> {
    type Item = u8;

    fn next(&mut self) -> Option<u8> {
        if let Some(next) = self.0.next().copied() {
            match next {
                b'%' => {
                    let a = self.0.next().copied().unwrap();
                    let b = self.0.next().copied().unwrap();
                    let ah = HEX_VAL[a as usize];
                    let bh = HEX_VAL[b as usize];
                    debug_assert!(ah != 0xFF && bh != 0xFF, "PctStr invariant: valid hex");
                    Some((ah << 4) | bh)
                }
                _ => Some(next),
            }
        } else {
            None
        }
    }
}

impl<'a> core::iter::FusedIterator for Bytes<'a> {}

/// Characters iterator.
///
/// Iterates over the decoded characters of a percent-encoded string.
pub struct Chars<'a> {
    inner: utf8_decode::Decoder<Bytes<'a>>,
}

impl<'a> Chars<'a> {
    fn new(bytes: Bytes<'a>) -> Self {
        Self {
            inner: utf8_decode::Decoder::new(bytes),
        }
    }
}

impl<'a> Iterator for Chars<'a> {
    type Item = char;

    fn next(&mut self) -> Option<char> {
        // Safe as PctStr guarantees a valid byte sequence
        self.inner.next().map(|x| x.unwrap())
    }
}

impl<'a> core::iter::FusedIterator for Chars<'a> {}

#[inline]
fn is_unreserved_byte(b: u8) -> bool {
    matches!(b,
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9'
        | b'-' | b'.' | b'_' | b'~'
    )
}

#[inline]
fn to_upper_hex_byte(b: u8) -> u8 {
    // ASCII hex digit — PctStr invariant — uppercase if lowercase.
    if b >= b'a' { b - b'a' + b'A' } else { b }
}

/// RFC 3986 §6.2.2.2 normalized byte iterator.
///
/// Yields the bytes of the underlying string after decoding only those
/// `%XX` triples whose value is an *unreserved* octet. Reserved triples
/// stay as `%` + two uppercase hex digits.
pub struct Rfc3986Bytes<'a> {
    iter: core::slice::Iter<'a, u8>,
    pending: [u8; 2],
    pending_len: u8,
    pending_pos: u8,
}

impl<'a> Rfc3986Bytes<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            iter: bytes.iter(),
            pending: [0; 2],
            pending_len: 0,
            pending_pos: 0,
        }
    }
}

impl<'a> Iterator for Rfc3986Bytes<'a> {
    type Item = u8;

    fn next(&mut self) -> Option<u8> {
        if self.pending_pos < self.pending_len {
            let b = self.pending[self.pending_pos as usize];
            self.pending_pos += 1;
            return Some(b);
        }
        let b = *self.iter.next()?;
        if b == b'%' {
            // SAFETY: PctStr invariant guarantees two hex digits follow.
            let h1 = *self.iter.next().unwrap();
            let h2 = *self.iter.next().unwrap();
            let a = HEX_VAL[h1 as usize];
            let c = HEX_VAL[h2 as usize];
            debug_assert!(a != 0xFF && c != 0xFF);
            let decoded = (a << 4) | c;
            if is_unreserved_byte(decoded) {
                return Some(decoded);
            }
            self.pending = [to_upper_hex_byte(h1), to_upper_hex_byte(h2)];
            self.pending_len = 2;
            self.pending_pos = 0;
            return Some(b'%');
        }
        Some(b)
    }
}

impl<'a> core::iter::FusedIterator for Rfc3986Bytes<'a> {}

#[cfg(test)]
mod rfc3986_tests {
    use super::*;
    use std::collections::hash_map::DefaultHasher;

    fn h_rfc(s: &str) -> u64 {
        let ps = PctStr::new(s).unwrap();
        let mut h = DefaultHasher::new();
        ps.hash_rfc3986(&mut h);
        h.finish()
    }

    #[test]
    fn unreserved_decoded_eq() {
        let a = PctStr::new("%7Eb").unwrap();
        let b = PctStr::new("~b").unwrap();
        assert!(a.eq_rfc3986(b));
    }

    #[test]
    fn reserved_stays_encoded_ne() {
        let a = PctStr::new("%2Fb").unwrap();
        let b = PctStr::new("/b").unwrap();
        assert!(!a.eq_rfc3986(b));
        assert!(a == b); // full-decode PartialEq says equal
    }

    #[test]
    fn hex_case_insensitive() {
        let a = PctStr::new("%2f").unwrap();
        let b = PctStr::new("%2F").unwrap();
        assert!(a.eq_rfc3986(b));
        assert_eq!(h_rfc("%2f"), h_rfc("%2F"));
    }

    #[test]
    fn ord_consistent() {
        let a = PctStr::new("a").unwrap();
        let b = PctStr::new("%62").unwrap(); // %62 = 'b'
        assert_eq!(a.cmp_rfc3986(b), core::cmp::Ordering::Less);
    }

    #[test]
    fn hash_matches_eq() {
        let pairs: &[(&str, &str)] = &[
            ("%7Eb", "~b"),
            ("%2f", "%2F"),
            ("abc", "abc"),
            ("%2Fb", "%2fb"),
        ];
        for (x, y) in pairs {
            let a = PctStr::new(x).unwrap();
            let b = PctStr::new(y).unwrap();
            if a.eq_rfc3986(b) {
                assert_eq!(h_rfc(x), h_rfc(y), "hash mismatch for eq pair {x:?} {y:?}");
            }
        }
    }
}

#[cfg(test)]
mod fast_check_tests {
    use super::{FastCheck, PctStr, validate_fast};

    /// Cross-check `validate_fast` (plus fallback) against the original
    /// `TryDecoder`-based `validate` over a pseudo-random input population.
    #[test]
    fn fast_vs_full() {
        // Small deterministic LCG for repeatable "random" bytes.
        let mut state: u32 = 0xdead_beef;
        let mut next = || {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (state >> 8) as u8
        };

        for _ in 0..512 {
            let len = (next() as usize) % 128;
            let mut buf = Vec::with_capacity(len);
            for _ in 0..len {
                let r = next();
                // Bias towards '%' so we hit percent paths often.
                buf.push(if r < 64 { b'%' } else { r });
            }

            let fast = match validate_fast(&buf) {
                FastCheck::Valid => true,
                FastCheck::Invalid => false,
                FastCheck::NeedsFullCheck => PctStr::validate(buf.iter().copied()),
            };
            let full = PctStr::validate(buf.iter().copied());
            assert_eq!(fast, full, "mismatch for {:?}", buf);
        }
    }
}
