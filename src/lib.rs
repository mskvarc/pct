//! This crate provides two types, [`PctStr`] and [`PctString`], similar to `str` and [`String`],
//! representing percent-encoded strings used in URL, URI, IRI, etc.
//! You can use them to encode, decode and compare percent-encoded strings.
//!
//! # Basic usage
//!
//! You can parse/decode percent-encoded strings by building a [`PctStr`] slice over a `str` slice.
//!
//! ```
//! use pct::PctStr;
//!
//! let pct_str = PctStr::new("Hello%20World%21").unwrap();
//! assert_eq!(pct_str, "Hello World!");
//!
//! let decoded_string: String = pct_str.decode();
//! assert_eq!(decoded_string, "Hello World!")
//! ```
//!
//! To create new percent-encoded strings, use the [`PctString`] to copy or encode new strings.
//!
//! ```
//! use pct::{PctString, UriReserved};
//!
//! // Copy the given percent-encoded string.
//! let pct_string = PctString::new("Hello%20World%21").unwrap();
//!
//! // Encode the given regular string.
//! let pct_string = PctString::encode("Hello World!".chars(), UriReserved::Any);
//!
//! assert_eq!(pct_string.as_str(), "Hello%20World%21");
//! ```
//!
//! You can choose which character will be percent-encoded by the `encode` function
//! by implementing the [`Encoder`] trait.
//!
//! ```
//! use pct::{UriReserved, PctString};
//!
//! struct CustomEncoder;
//!
//! impl pct::Encoder for CustomEncoder {
//!   fn encode(&self, c: char) -> bool {
//!     UriReserved::Any.encode(c) || c.is_uppercase()
//!   }
//! }
//!
//! let pct_string = PctString::encode("Hello World!".chars(), CustomEncoder);
//! assert_eq!(pct_string.as_str(), "%48ello%20%57orld%21")
//! ```
//!
//! [`String`]: std::string::String
//! [`PctStr`]: crate::nsized::PctStr
//! [`PctString`]: crate::sized::PctString
//! [`Encoder`]: crate::encoder::Encoder
#![cfg_attr(not(feature = "std"), no_std)]

pub(crate) mod encoder;
mod error;
mod nsized;
pub(crate) mod scan;
#[cfg(feature = "std")]
mod sized;
pub(crate) mod util;

pub use encoder::*;
pub use error::*;
pub use nsized::*;
#[cfg(feature = "std")]
pub use sized::*;

#[cfg(test)]
mod tests {
    use std::convert::TryInto;

    use super::*;

    #[test]
    fn pct_encoding_invalid() {
        let s = "%FF%FE%20%4F";
        assert!(PctStr::new(s).is_err());
        let s = "%36%A";
        assert!(PctStr::new(s).is_err());
        let s = "%%32";
        assert!(PctStr::new(s).is_err());
        let s = "%%32";
        assert!(PctStr::new(s).is_err());
    }

    #[test]
    fn pct_encoding_valid() {
        let s = "%00%5C%F4%8F%BF%BD%69";
        assert!(PctStr::new(s).is_ok());
        let s = "No percent.";
        assert!(PctStr::new(s).is_ok());
        let s = "%e2%82%acwat";
        assert!(PctStr::new(s).is_ok());
    }

    #[test]
    fn try_from() {
        let s = "%00%5C%F4%8F%BF%BD%69";
        let _pcs = PctString::try_from(s).unwrap();
        let _pcs: &PctStr = s.try_into().unwrap();
    }

    #[test]
    fn encode_percent_always() {
        struct NoopEncoder;
        impl Encoder for NoopEncoder {
            fn encode(&self, _: char) -> bool {
                false
            }
        }
        let s = "%";
        let c = PctString::encode(s.chars(), NoopEncoder);
        assert_eq!(c.as_str(), "%25");
    }
}
