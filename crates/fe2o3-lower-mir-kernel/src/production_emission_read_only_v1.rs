// Shared immutable constructor inputs stay borrowed; standalone callers can own them.
enum EmissionReadOnlyV1<'a, T> {
    Borrowed(&'a T),
    Owned(T),
}

impl<T> From<T> for EmissionReadOnlyV1<'_, T> {
    fn from(value: T) -> Self {
        Self::Owned(value)
    }
}

impl<'a, T> From<&'a T> for EmissionReadOnlyV1<'a, T> {
    fn from(value: &'a T) -> Self {
        Self::Borrowed(value)
    }
}

impl<T> std::ops::Deref for EmissionReadOnlyV1<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        match self {
            Self::Borrowed(value) => value,
            Self::Owned(value) => value,
        }
    }
}

#[cfg(test)]
#[path = "production_emission_read_only_v1_tests.rs"]
mod emission_read_only_tests;
