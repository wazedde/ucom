use std::borrow::Cow;
use std::fmt::{Display, Formatter};
use std::path::Path;

/// An extension trait for paths that provides methods to normalize paths for the current platform.
pub trait PlatformConsistentPathExt {
    /// Returns a normalized path for the current platform.
    ///
    /// On Windows, this replaces `/` with `\` separators. If replacement occurs,
    /// a new `PathBuf` is allocated and returned via `Cow::Owned`.
    /// On non-Windows platforms, or on Windows if no `/` separators are present,
    /// this returns a `Cow::Borrowed` referencing the original path slice, performing no allocations.
    ///
    /// Note: Uses `to_string_lossy` internally on Windows, potentially replacing
    /// invalid UTF-8 sequences.
    fn normalized(&self) -> Cow<'_, Path>;

    /// Returns an object that implements `Display` for the normalized path.
    ///
    /// This allows displaying the path consistent with platform conventions
    /// without necessarily allocating a `String` if the path doesn't need changes.
    ///
    /// ```rust
    /// use std::path::Path;
    /// // Assuming PlatformConsistentPathExt is in scope
    /// let p = Path::new("foo/bar");
    /// // On Windows, prints "foo\bar"
    /// // On Linux/macOS, prints "foo/bar"
    /// println!("{}", p.normalized_display());
    /// ```
    fn normalized_display(&self) -> impl Display + '_ {
        struct PathDisplay<'a> {
            path: Cow<'a, Path>,
        }

        impl Display for PathDisplay<'_> {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                self.path.display().fmt(f)
            }
        }

        PathDisplay {
            path: self.normalized(),
        }
    }
}

impl PlatformConsistentPathExt for Path {
    fn normalized(&self) -> Cow<'_, Path> {
        #[cfg(target_os = "windows")]
        {
            let path = self.to_string_lossy();
            if path.contains('/') {
                return Cow::Owned(path.replace('/', "\\").into());
            }
        }

        Cow::Borrowed(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "windows")]
    fn test_normalized_display() {
        let path = Path::new("foo/bar");
        assert_eq!(
            path.normalized_display().to_string(),
            "foo\\bar",
            "Should replace forward slashes with backslashes"
        );

        let path = Path::new("foo\\bar");
        assert_eq!(
            path.normalized_display().to_string(),
            "foo\\bar",
            "Should not change path with backslashes"
        );

        let path = Path::new("foo\\bar/baz");
        assert_eq!(
            path.normalized_display().to_string(),
            "foo\\bar\\baz",
            "Should replace forward slashes with backslashes"
        );

        let path = Path::new("foo/bar");
        assert_eq!(
            path.normalized_display().to_string(),
            Path::new("foo\\bar").to_string_lossy(),
            "Should replace forward slashes with backslashes"
        );

        let path = Path::new("foo\\bar");
        assert_eq!(
            path.normalized_display().to_string(),
            Path::new("foo\\bar").to_string_lossy(),
            "Should not change path with backslashes"
        );

        let path = Path::new("foo\\bar/baz");
        assert_eq!(
            path.normalized_display().to_string(),
            Path::new("foo\\bar\\baz").to_string_lossy(),
            "Should replace forward slashes with backslashes"
        );
    }

    #[test]
    #[cfg(not(target_os = "windows"))]
    fn test_normalized_display() {
        let path = Path::new("foo/bar");
        assert_eq!(
            path.normalized_display().to_string(),
            "foo/bar",
            "Should not change path"
        );

        let path = Path::new("foo/bar");
        assert_eq!(
            path.normalized_display().to_string(),
            Path::new("foo/bar").to_string_lossy(),
            "Should not change path"
        );
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_cow_behavior_windows() {
        // Path without forward slashes should borrow
        let path = Path::new("foo\\bar");
        let result = path.normalized();
        match result {
            Cow::Borrowed(_) => {} // Expected for already normalized paths
            Cow::Owned(_) => panic!("Should return Cow::Borrowed for already normalized paths"),
        }

        // Path with forward slashes should allocate
        let path = Path::new("foo/bar");
        let result = path.normalized();
        match result {
            Cow::Borrowed(_) => panic!("Should return Cow::Owned when normalization is needed"),
            Cow::Owned(_) => {} // Expected when normalization is needed
        }
    }

    #[test]
    #[cfg(not(target_os = "windows"))]
    fn test_cow_behavior_unix() {
        // On non-Windows platforms, should always borrow
        let path = Path::new("foo/bar");
        let result = path.normalized();
        match result {
            Cow::Borrowed(_) => {} // Expected
            Cow::Owned(_) => panic!("Should always return Cow::Borrowed on non-Windows"),
        }
    }
}
