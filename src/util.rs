use core::fmt::{self, Display, Formatter};

use utf8_decode::Utf8Error;

#[inline(always)]
pub fn to_digit(b: u8) -> Result<u8, ByteError> {
	match b {
		// ASCII 0..=9
		0x30..=0x39 => Ok(b - 0x30),
		// ASCII A..=F
		0x41..=0x46 => Ok(b - 0x37),
		// ASCII a..=f
		0x61..=0x66 => Ok(b - 0x57),
		_ => Err(ByteError::InvalidByte(b)),
	}
}

#[derive(Debug, Clone)]
pub enum ByteError {
	InvalidByte(u8),
	IncompleteEncoding,
	Utf8(Utf8Error),
}

impl Display for ByteError {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		match self {
			ByteError::InvalidByte(b) => write!(f, "Invalid UTF-8 byte: {:#x}", b),
			ByteError::IncompleteEncoding => f.write_str("Incomplete percent-encoding segment"),
			ByteError::Utf8(e) => e.fmt(f),
		}
	}
}

impl From<Utf8Error> for ByteError {
	fn from(value: Utf8Error) -> Self {
		Self::Utf8(value)
	}
}

impl core::error::Error for ByteError {}

/// Untrusted bytes iterator.
///
/// Iterates over the encoded bytes of a percent-encoded string.
pub struct TryEncodedBytes<B>(B);

impl<B> TryEncodedBytes<B> {
	pub fn new(bytes: B) -> Self {
		Self(bytes)
	}
}

impl<B: Iterator<Item = u8>> TryEncodedBytes<B> {
	fn try_next(&mut self, next: u8) -> Result<u8, ByteError> {
		match next {
			b'%' => {
				let a = self.0.next().ok_or(ByteError::IncompleteEncoding)?;
				let a = to_digit(a)?;
				let b = self.0.next().ok_or(ByteError::IncompleteEncoding)?;
				let b = to_digit(b)?;
				let byte = a << 4 | b;
				Ok(byte)
			}
			_ => Ok(next),
		}
	}
}

impl<B: Iterator<Item = u8>> Iterator for TryEncodedBytes<B> {
	type Item = Result<u8, ByteError>;

	fn next(&mut self) -> Option<Result<u8, ByteError>> {
		self.0.next().map(|b| self.try_next(b))
	}
}

impl<B: Iterator<Item = u8>> core::iter::FusedIterator for TryEncodedBytes<B> {}
