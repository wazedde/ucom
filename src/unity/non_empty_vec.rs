#[derive(Debug)]
pub enum NonEmptyVecErr {
    VecIsEmpty,
}

/// A non-empty list of values.
pub struct NonEmptyVec<T> {
    inner: Vec<T>,
}

#[allow(dead_code)]
impl<T> NonEmptyVec<T> {
    /// Creates a new non-empty list.
    pub fn new(first: T) -> Self {
        let inner = vec![first];
        Self { inner }
    }

    /// Creates a new non-empty list from a vector.
    pub fn from_vec(value: Vec<T>) -> Result<Self, NonEmptyVecErr> {
        if value.is_empty() {
            Err(NonEmptyVecErr::VecIsEmpty)
        } else {
            Ok(Self { inner: value })
        }
    }

    /// Returns the value at the given index.
    pub fn get(&self, index: usize) -> Option<&T> {
        self.inner.get(index)
    }

    /// Returns a mutable reference to the value at the given index.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.inner.get_mut(index)
    }

    /// Returns the first value.
    pub fn first(&self) -> &T {
        &self.inner[0]
    }

    /// Returns the last value.
    pub fn last(&self) -> &T {
        &self.inner[self.inner.len() - 1]
    }

    /// Returns a mutable reference to the first value.
    pub fn first_mut(&mut self) -> &mut T {
        &mut self.inner[0]
    }

    /// Returns a mutable reference to the last value.
    pub fn last_mut(&mut self) -> &mut T {
        let len = self.inner.len();
        &mut self.inner[len - 1]
    }

    /// Returns an iterator over the values.
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.inner.iter()
    }

    /// Returns a mutable iterator over the values.
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, T> {
        self.inner.iter_mut()
    }

    /// Returns the number of values.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Pushes a value to the end of the list.
    pub fn push(&mut self, value: T) {
        self.inner.push(value);
    }

    /// Appends the values from the other list.
    pub fn append(&mut self, other: &mut Vec<T>) {
        self.inner.append(other);
    }

    /// Extends the list with the values from the other list.
    pub fn extend(&mut self, other: Vec<T>) {
        self.inner.extend(other);
    }

    /// Unstable sorts the values in place.
    pub fn sort_unstable(&mut self)
    where
        T: Ord,
    {
        self.inner.sort_unstable();
    }

    /// Unstable sorts the values in place by the key.
    pub fn sort_unstable_by_key<K, F>(&mut self, f: F)
    where
        F: FnMut(&T) -> K,
        K: Ord,
    {
        self.inner.sort_unstable_by_key(f);
    }

    /// Unstable sorts the values in place by the comparison function.
    pub fn sort_unstable_by<F>(&mut self, compare: F)
    where
        F: FnMut(&T, &T) -> std::cmp::Ordering,
    {
        self.inner.sort_unstable_by(compare);
    }

    /// Sorts the values in place.
    pub fn sort(&mut self)
    where
        T: Ord,
    {
        self.inner.sort();
    }

    /// Sorts the values in place by the key.
    pub fn sort_by_key<K, F>(&mut self, f: F)
    where
        F: FnMut(&T) -> K,
        K: Ord,
    {
        self.inner.sort_by_key(f);
    }

    /// Sorts the values in place by the comparison function.
    pub fn sort_by<F>(&mut self, compare: F)
    where
        F: FnMut(&T, &T) -> std::cmp::Ordering,
    {
        self.inner.sort_by(compare);
    }
}

impl<T> TryFrom<Vec<T>> for NonEmptyVec<T> {
    type Error = NonEmptyVecErr;

    /// Tries to create a non-empty list from a vector.
    fn try_from(value: Vec<T>) -> Result<Self, Self::Error> {
        if value.is_empty() {
            Err(NonEmptyVecErr::VecIsEmpty)
        } else {
            Ok(Self { inner: value })
        }
    }
}

#[allow(clippy::from_over_into)]
impl<T> Into<Vec<T>> for NonEmptyVec<T> {
    /// Converts the non-empty list into a vector.
    fn into(self) -> Vec<T> {
        self.inner
    }
}

impl<T> AsRef<Vec<T>> for NonEmptyVec<T> {
    fn as_ref(&self) -> &Vec<T> {
        &self.inner
    }
}
