use std::ops::{Deref, DerefMut};

#[derive(Debug)]
pub enum Vec1Err {
    VecIsEmpty,
    CannotPopLastElement,
}

/// A non-empty list.
pub struct Vec1<T>(Vec<T>);

impl<T> DerefMut for Vec1<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T> Deref for Vec1<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> Vec1<T> {
    /// Creates a new non-empty list.
    pub fn new(first: T) -> Self {
        Self(vec![first])
    }

    /// Converts the list into a Vec.
    pub fn into_vec(self) -> Vec<T> {
        self.0
    }

    /// Returns the first value.
    pub fn first(&self) -> &T {
        &self[0]
    }

    /// Returns the last value.
    pub fn last(&self) -> &T {
        self.0.last().expect("Vec1 should never be empty")
    }

    /// Returns a mutable reference to the first value.
    pub fn first_mut(&mut self) -> &mut T {
        &mut self[0]
    }

    /// Returns a mutable reference to the last value.
    pub fn last_mut(&mut self) -> &mut T {
        self.0.last_mut().expect("Vec1 should never be empty")
    }

    /// Pushes a value to the end of the list.
    pub fn push(&mut self, value: T) {
        self.0.push(value);
    }

    /// Pops the last value from the list.
    /// Returns an error if attempting to pop the last value.
    pub fn pop(&mut self) -> Result<T, Vec1Err> {
        if self.len() == 1 {
            Err(Vec1Err::CannotPopLastElement)
        } else {
            Ok(self.0.pop().unwrap())
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

impl<T> TryFrom<Vec<T>> for Vec1<T> {
    type Error = Vec1Err;

    fn try_from(value: Vec<T>) -> Result<Self, Self::Error> {
        if value.is_empty() {
            Err(Vec1Err::VecIsEmpty)
        } else {
            Ok(Self(value))
        }
    }
}

impl<T> From<Vec1<T>> for Vec<T> {
    fn from(v: Vec1<T>) -> Self {
        v.0
    }
}

impl<T> AsRef<[T]> for Vec1<T> {
    fn as_ref(&self) -> &[T] {
        &self.0
    }
}
