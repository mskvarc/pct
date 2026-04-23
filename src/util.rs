use core::fmt::{self, Display, Formatter};

use utf8_decode::Utf8Error;

#[inline]
pub(crate) fn find_percent(bytes: &[u8]) -> Option<usize> {
    #[cfg(feature = "memchr")]
    {
        memchr::memchr(b'%', bytes)
    }
    #[cfg(not(feature = "memchr"))]
    {
        bytes.iter().position(|&b| b == b'%')
    }
}

/// Lookup table: byte → hex nibble value (`0..=15`), or `0xFF` for non-hex bytes.
pub(crate) static HEX_VAL: [u8; 256] = build_hex_val();

const fn build_hex_val() -> [u8; 256] {
    let mut t = [0xFFu8; 256];
    let mut i = 0u16;
    while i < 256 {
        let b = i as u8;
        t[i as usize] = match b {
            b'0'..=b'9' => b - b'0',
            b'A'..=b'F' => b - b'A' + 10,
            b'a'..=b'f' => b - b'a' + 10,
            _ => 0xFF,
        };
        i += 1;
    }
    t
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
                let b = self.0.next().ok_or(ByteError::IncompleteEncoding)?;
                let ah = HEX_VAL[a as usize];
                let bh = HEX_VAL[b as usize];
                // Valid hex nibbles are 0..=15; invalid nibbles are 0xFF. If
                // either is 0xFF, the OR is 0xFF.
                if ah | bh == 0xFF {
                    return Err(ByteError::InvalidByte(if ah == 0xFF { a } else { b }));
                }
                Ok((ah << 4) | bh)
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
