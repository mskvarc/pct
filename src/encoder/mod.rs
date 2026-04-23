mod iri;
mod uri;

pub use iri::*;
pub use uri::*;

/// Encoding predicate.
///
/// Instances of this trait are used along with the [`encode`] function
/// to decide which character must be percent-encoded.
///
/// This crate provides a simple implementation of the trait, [`UriReserved`]
/// encoding characters reserved in the URI syntax.
///
/// [`encode`]: crate::PctString::encode
///
/// # Example
///
/// ```
/// use pct::{PctString, UriReserved};
///
/// let pct_string = PctString::encode("Hello World!".chars(), UriReserved::Any);
/// println!("{}", pct_string.as_str()); // => Hello World%21
/// ```
///
/// Custom encoder implementation:
///
/// ```
/// use pct::{PctString, UriReserved};
///
/// struct CustomEncoder;
///
/// impl pct::Encoder for CustomEncoder {
///   fn encode(&self, c: char) -> bool {
///     UriReserved::Any.encode(c) || c.is_uppercase()
///   }
/// }
///
/// let pct_string = PctString::encode("Hello World!".chars(), CustomEncoder);
/// println!("{}", pct_string.as_str()); // => %48ello %57orld%21
/// ```
pub trait Encoder {
    /// Decide if the given character must be encoded.
    ///
    /// Note that the character `%` is always encoded even if this method returns `false` on it.
    fn encode(&self, c: char) -> bool;
}

impl<F: Fn(char) -> bool> Encoder for F {
    fn encode(&self, c: char) -> bool {
        self(c)
    }
}
