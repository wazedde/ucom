use std::error::Error;
use std::fmt::{Display, Formatter};
use std::ops::{Deref, DerefMut};

//
// Error  implementation
//

/// Errors that can occur when working with a [`Vec1`].
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum Vec1Error {
    /// Attempted to create a [`Vec1`] from an empty Vec.
    SourceVecIsEmpty,
    /// Attempted to pop the last element from a [`Vec1`].
    CannotPopLastElement,
}

const GUARANTEE_NON_EMPTY: &str = "Vec1 is guaranteed to be non-empty by construction";

impl Error for Vec1Error {}

impl Display for Vec1Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SourceVecIsEmpty => write!(f, "Source Vec is empty"),
            Self::CannotPopLastElement => write!(f, "Cannot pop last element"),
        }
    }
}

//
// Vec1 implementation
//

/// A non-empty list. This list is guaranteed to have at least one element.
pub struct Vec1<T>(Vec<T>);

impl<T> Deref for Vec1<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for Vec1<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T> TryFrom<Vec<T>> for Vec1<T> {
    type Error = Vec1Error;

    /// Creates a [`Vec1`] from a [`Vec`].
    ///
    /// # Errors
    /// Returns [`Vec1Error::SourceVecIsEmpty`] if the source Vec is empty.
    fn try_from(source: Vec<T>) -> Result<Self, Self::Error> {
        if source.is_empty() {
            Err(Vec1Error::SourceVecIsEmpty)
        } else {
            Ok(Self(source))
        }
    }
}

impl<T> From<Vec1<T>> for Vec<T> {
    /// Converts a [`Vec1`] into a [`Vec`].
    fn from(v: Vec1<T>) -> Self {
        v.0
    }
}

impl<T> AsRef<[T]> for Vec1<T> {
    /// Provides a reference to the underlying slice.
    fn as_ref(&self) -> &[T] {
        &self.0
    }
}

impl<T> Vec1<T> {
    /// Creates a new non-empty list.
    pub fn new(first: T) -> Self {
        Self(vec![first])
    }

    /// Returns the first value.
    pub fn first(&self) -> &T {
        self.0.first().expect(GUARANTEE_NON_EMPTY)
    }

    /// Returns the last value.
    pub fn last(&self) -> &T {
        self.0.last().expect(GUARANTEE_NON_EMPTY)
    }

    /// Returns a mutable reference to the first value.
    pub fn first_mut(&mut self) -> &mut T {
        self.0.first_mut().expect(GUARANTEE_NON_EMPTY)
    }

    /// Returns a mutable reference to the last value.
    pub fn last_mut(&mut self) -> &mut T {
        self.0.last_mut().expect(GUARANTEE_NON_EMPTY)
    }

    /// Pushes a value to the end of the list.
    pub fn push(&mut self, value: T) {
        self.0.push(value);
    }

    /// Pops the last value from the list.
    ///
    /// # Errors
    /// Returns [`Vec1Error::CannotPopLastElement`] if the list only contains one element.
    pub fn pop(&mut self) -> Result<T, Vec1Error> {
        if self.len() == 1 {
            Err(Vec1Error::CannotPopLastElement)
        } else {
            Ok(self.0.pop().expect(GUARANTEE_NON_EMPTY))
        }
    }

    /// Appends the values from the other list.
    pub fn append(&mut self, other: &mut Vec<T>) {
        self.0.append(other);
    }

    /// Extends the list with the values from the other list.
    pub fn extend(&mut self, other: Vec<T>) {
        self.0.extend(other);
    }
}

//
// Tests
//

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let v = Vec1::new(1);
        assert_eq!(v.first(), &1);
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn test_push_pop() {
        let mut v = Vec1::new(1);
        v.push(2);
        assert_eq!(v.last(), &2);
        assert_eq!(v.pop(), Ok(2));
        assert_eq!(v.pop(), Err(Vec1Error::CannotPopLastElement));
    }

    #[test]
    fn test_try_from_vec() {
        assert!(Vec1::try_from(Vec::<i32>::default()).is_err());
        let v = Vec1::try_from(vec![1]).unwrap();
        assert_eq!(v.first(), &1);
    }

    #[test]
    fn test_extend() {
        let mut v = Vec1::new(1);
        v.extend(vec![2, 3]);
        assert_eq!(v.as_ref(), &[1, 2, 3]);
    }

    #[test]
    fn test_append() {
        let mut v = Vec1::new(1);
        let mut other = vec![2, 3];
        v.append(&mut other);
        assert_eq!(v.as_ref(), &[1, 2, 3]);
    }

    #[test]
    fn test_last_mut() {
        let mut v = Vec1::new(1);
        v.push(2);
        assert_eq!(v.last_mut(), &mut 2);
        *v.last_mut() = 3;
        assert_eq!(v.last_mut(), &mut 3);
    }

    #[test]
    fn test_first_mut() {
        let mut v = Vec1::new(1);
        v.push(2);
        assert_eq!(v.first_mut(), &mut 1);
        *v.first_mut() = 3;
        assert_eq!(v.first_mut(), &mut 3);
    }
}
