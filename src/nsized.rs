use core::{
	borrow::Borrow,
	cmp::Ordering,
	fmt::{self, Debug, Display, Formatter},
	hash::{Hash, Hasher},
};

use crate::{
	InvalidPctString, PctString,
	util::{TryEncodedBytes, to_digit},
};

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
/// use pct_str::PctStr;
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
		if Self::validate(input_bytes.iter().copied()) {
			Ok(unsafe { Self::new_unchecked(input_bytes) })
		} else {
			Err(InvalidPctString(input))
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
		unsafe { std::mem::transmute::<&[u8], &PctStr>(input.as_ref()) }
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
		self.chars().count()
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

	/// Decoding.
	///
	/// Return the string with the percent-encoded characters decoded.
	pub fn decode(&self) -> String {
		let mut decoded = String::with_capacity(self.len());
		for c in self.chars() {
			decoded.push(c)
		}

		decoded
	}
}

impl PartialEq for PctStr {
	#[inline]
	fn eq(&self, other: &PctStr) -> bool {
		let mut a = self.chars();
		let mut b = other.chars();

		loop {
			match (a.next(), b.next()) {
				(Some(a), Some(b)) if a != b => return false,
				(Some(_), None) => return false,
				(None, Some(_)) => return false,
				(None, None) => break,
				_ => (),
			}
		}

		true
	}
}

impl Eq for PctStr {}

impl PartialEq<str> for PctStr {
	#[inline]
	fn eq(&self, other: &str) -> bool {
		let mut a = self.chars();
		let mut b = other.chars();

		loop {
			match (a.next(), b.next()) {
				(Some(a), Some(b)) if a != b => return false,
				(Some(_), None) => return false,
				(None, Some(_)) => return false,
				(None, None) => break,
				_ => (),
			}
		}

		true
	}
}

impl PartialEq<PctString> for PctStr {
	#[inline]
	fn eq(&self, other: &PctString) -> bool {
		let mut a = self.chars();
		let mut b = other.chars();

		loop {
			match (a.next(), b.next()) {
				(Some(a), Some(b)) if a != b => return false,
				(Some(_), None) => return false,
				(None, Some(_)) => return false,
				(None, None) => break,
				_ => (),
			}
		}

		true
	}
}

impl PartialOrd for PctStr {
	fn partial_cmp(&self, other: &PctStr) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}

impl Ord for PctStr {
	fn cmp(&self, other: &PctStr) -> Ordering {
		let mut self_chars = self.chars();
		let mut other_chars = other.chars();

		loop {
			match (self_chars.next(), other_chars.next()) {
				(None, None) => return Ordering::Equal,
				(None, Some(_)) => return Ordering::Less,
				(Some(_), None) => return Ordering::Greater,
				(Some(a), Some(b)) => match a.cmp(&b) {
					Ordering::Less => return Ordering::Less,
					Ordering::Greater => return Ordering::Greater,
					Ordering::Equal => (),
				},
			}
		}
	}
}

impl PartialOrd<PctString> for PctStr {
	fn partial_cmp(&self, other: &PctString) -> Option<Ordering> {
		self.partial_cmp(other.as_pct_str())
	}
}

impl Hash for PctStr {
	#[inline]
	fn hash<H: Hasher>(&self, hasher: &mut H) {
		for c in self.chars() {
			c.hash(hasher)
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

impl ToOwned for PctStr {
	type Owned = PctString;

	fn to_owned(&self) -> Self::Owned {
		unsafe { PctString::new_unchecked(self.0.to_owned()) }
	}
}

impl Borrow<str> for PctStr {
	fn borrow(&self) -> &str {
		self.as_str()
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
pub struct Bytes<'a>(std::slice::Iter<'a, u8>);

impl<'a> Iterator for Bytes<'a> {
	type Item = u8;

	fn next(&mut self) -> Option<u8> {
		if let Some(next) = self.0.next().copied() {
			match next {
				b'%' => {
					let a = self.0.next().copied().unwrap();
					let a = to_digit(a).unwrap();
					let b = self.0.next().copied().unwrap();
					let b = to_digit(b).unwrap();
					let byte = a << 4 | b;
					Some(byte)
				}
				_ => Some(next),
			}
		} else {
			None
		}
	}
}

impl<'a> std::iter::FusedIterator for Bytes<'a> {}

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

impl<'a> std::iter::FusedIterator for Chars<'a> {}
