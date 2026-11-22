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
        struct PathDisplay<'a>(Cow<'a, Path>);

        impl Display for PathDisplay<'_> {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                self.0.display().fmt(f)
            }
        }

        PathDisplay(self.normalized())
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
    fn normalized_display_uses_the_platform_separator() {
        let display = |path: &str| Path::new(path).normalized_display().to_string();
        assert_eq!(display("foo/bar"), "foo\\bar");
        assert_eq!(display("foo\\bar"), "foo\\bar");
        assert_eq!(display("foo\\bar/baz"), "foo\\bar\\baz");
    }

    #[test]
    #[cfg(not(target_os = "windows"))]
    fn normalized_display_uses_the_platform_separator() {
        assert_eq!(
            Path::new("foo/bar").normalized_display().to_string(),
            "foo/bar"
        );
    }
}
