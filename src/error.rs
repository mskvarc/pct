use core::fmt::{self, Display, Formatter};

/// Encoding error.
///
/// Raised when a given input string is not percent-encoded as expected.
#[derive(Debug, Clone, Copy)]
pub struct InvalidPctString<T>(pub T);

impl<T> Display for InvalidPctString<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("invalid percent-encoded string")
    }
}

impl<T> InvalidPctString<T> {
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> InvalidPctString<U> {
        InvalidPctString(f(self.0))
    }
}

#[cfg(feature = "std")]
impl<T: ?Sized + ToOwned> InvalidPctString<&T> {
    pub fn into_owned(self) -> InvalidPctString<T::Owned> {
        self.map(T::to_owned)
    }
}
