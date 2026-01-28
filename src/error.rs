/// Encoding error.
///
/// Raised when a given input string is not percent-encoded as expected.
#[derive(Debug, Clone, thiserror::Error)]
#[error("invalid percent-encoded string")]
pub struct InvalidPctString<T>(pub T);

impl<T> InvalidPctString<T> {
	pub fn map<U>(self, f: impl FnOnce(T) -> U) -> InvalidPctString<U> {
		InvalidPctString(f(self.0))
	}
}

impl<T: ?Sized + ToOwned> InvalidPctString<&T> {
	pub fn into_owned(self) -> InvalidPctString<T::Owned> {
		self.map(T::to_owned)
	}
}
