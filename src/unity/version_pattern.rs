use std::fmt;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

use crate::unity::{BuildType, Version};

/// A pattern that selects Unity versions, such as `2022.3`, `2022.3.5f1`, `2022.3.*` or `202*`.
///
/// Matching rules:
/// - Each component in the pattern is compared with the version's component as a whole number,
///   so `2022.3.1` matches `2022.3.1f1` but not `2022.3.10f1`.
/// - Components that are left out match anything: `2022.3` matches every `2022.3.x` version.
/// - `*` matches any digits within a component: `202*` matches 2020 to 2029, `2022.3.6*` matches
///   patches 6 and 60 to 69.
/// - A component that is only `*`, `x` or `X` matches any value: `2022.x` is the same as `2022`.
/// - A build type may follow the patch, with or without a build number: `2022.3.62f`, `2022.3.62f1`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionPattern {
    text: String,
    major: ComponentPattern,
    minor: ComponentPattern,
    patch: ComponentPattern,
    build_type: Option<BuildType>,
    build: ComponentPattern,
}

/// Pattern for a single numeric version component.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ComponentPattern {
    /// Matches any value.
    Any,
    /// Matches exactly this value.
    Exact(u32),
    /// Matches the decimal representation of the value, where `*` matches any digits.
    Glob(String),
}

impl ComponentPattern {
    /// Parses a component consisting of digits and `*`, or a single `x`/`X` wildcard.
    fn parse(s: &str) -> Option<Self> {
        match s {
            "*" | "x" | "X" => Some(Self::Any),
            _ if s.is_empty() => None,
            _ if s.bytes().all(|b| b.is_ascii_digit()) => s.parse().ok().map(Self::Exact),
            _ if s.bytes().all(|b| b.is_ascii_digit() || b == b'*') => {
                Some(Self::Glob(s.to_owned()))
            }
            _ => None,
        }
    }

    fn matches(&self, value: u32) -> bool {
        match self {
            Self::Any => true,
            Self::Exact(expected) => value == *expected,
            Self::Glob(glob) => glob_match(glob.as_bytes(), value.to_string().as_bytes()),
        }
    }
}

/// Matches `text` against `pattern`, where `*` in the pattern matches any sequence of bytes.
fn glob_match(pattern: &[u8], text: &[u8]) -> bool {
    match pattern.split_first() {
        None => text.is_empty(),
        Some((b'*', rest)) => {
            glob_match(rest, text) || (!text.is_empty() && glob_match(pattern, &text[1..]))
        }
        Some((p, rest)) => text
            .split_first()
            .is_some_and(|(t, text_rest)| p == t && glob_match(rest, text_rest)),
    }
}

/// Error returned when a [`VersionPattern`] cannot be parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionPatternError;

impl std::error::Error for VersionPatternError {}

impl Display for VersionPatternError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "expected a version or pattern such as '2022.3.5f1', '2022.3', '2022.3.*' or '202*'"
        )
    }
}

impl FromStr for VersionPattern {
    type Err = VersionPatternError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let text = s.trim();
        // Allow a trailing dot, e.g. `2022.3.` is the same as `2022.3`.
        let body = text.strip_suffix('.').unwrap_or(text);

        let mut parts = body.split('.');
        let major = parts
            .next()
            .and_then(ComponentPattern::parse)
            .ok_or(VersionPatternError)?;

        // Components that are left out match anything.
        let minor = match parts.next() {
            Some(p) => ComponentPattern::parse(p).ok_or(VersionPatternError)?,
            None => ComponentPattern::Any,
        };

        let (patch, build_type, build) = match parts.next() {
            Some(p) => parse_patch_and_build(p).ok_or(VersionPatternError)?,
            None => (ComponentPattern::Any, None, ComponentPattern::Any),
        };

        if parts.next().is_some() {
            return Err(VersionPatternError);
        }

        Ok(Self {
            text: text.to_owned(),
            major,
            minor,
            patch,
            build_type,
            build,
        })
    }
}

/// Parses the last dot-separated part of a pattern, e.g. `5`, `5f`, `5f1`, `6*` or `*`,
/// into the patch, build type and build number patterns.
fn parse_patch_and_build(
    s: &str,
) -> Option<(ComponentPattern, Option<BuildType>, ComponentPattern)> {
    let Some(type_start) = s.find(|c: char| c.is_ascii_alphabetic() && c != 'x' && c != 'X') else {
        return Some((ComponentPattern::parse(s)?, None, ComponentPattern::Any));
    };

    let patch = ComponentPattern::parse(&s[..type_start])?;
    let rest = &s[type_start..];
    let type_len = rest
        .find(|c: char| !c.is_ascii_alphabetic())
        .unwrap_or(rest.len());
    let build_type = BuildType::from_short_str(&rest[..type_len].to_ascii_lowercase())?;

    let build = match &rest[type_len..] {
        "" => ComponentPattern::Any,
        b => ComponentPattern::parse(b)?,
    };

    Some((patch, Some(build_type), build))
}

impl Display for VersionPattern {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

impl From<Version> for VersionPattern {
    /// Creates a pattern that matches exactly the given version.
    fn from(version: Version) -> Self {
        Self {
            text: version.to_string(),
            major: ComponentPattern::Exact(version.major.into()),
            minor: ComponentPattern::Exact(version.minor.into()),
            patch: ComponentPattern::Exact(version.patch.into()),
            build_type: Some(version.build_type),
            build: ComponentPattern::Exact(version.build.into()),
        }
    }
}

impl VersionPattern {
    /// Creates a pattern that matches every version in the `major.minor` series of `version`.
    pub fn series_of(version: Version) -> Self {
        Self {
            text: version.to_major_minor_string(),
            major: ComponentPattern::Exact(version.major.into()),
            minor: ComponentPattern::Exact(version.minor.into()),
            patch: ComponentPattern::Any,
            build_type: None,
            build: ComponentPattern::Any,
        }
    }

    /// Returns true if the given version matches this pattern.
    pub fn matches(&self, version: Version) -> bool {
        self.major.matches(version.major.into())
            && self.minor.matches(version.minor.into())
            && self.patch.matches(version.patch.into())
            && self.build_type.is_none_or(|t| t == version.build_type)
            && self.build.matches(version.build.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Version {
        s.parse().unwrap()
    }

    fn p(s: &str) -> VersionPattern {
        s.parse().unwrap()
    }

    #[test]
    fn components_match_as_whole_numbers() {
        assert!(p("2022.3.1").matches(v("2022.3.1f1")));
        assert!(!p("2022.3.1").matches(v("2022.3.10f1")));
        assert!(!p("6000.1").matches(v("6000.10.0f1")));
        assert!(!p("202").matches(v("2022.3.1f1")));
    }

    #[test]
    fn missing_components_match_anything() {
        assert!(p("2022").matches(v("2022.3.62f3")));
        assert!(p("2022.3").matches(v("2022.3.62f3")));
        assert!(p("2022.3.").matches(v("2022.3.62f3")));
        assert!(!p("2022.3").matches(v("2022.2.1f1")));
    }

    #[test]
    fn build_type_and_number() {
        assert!(p("2022.3.62f").matches(v("2022.3.62f3")));
        assert!(!p("2022.3.62f").matches(v("2022.3.62p1")));
        assert!(p("2022.3.62f1").matches(v("2022.3.62f1")));
        assert!(!p("2022.3.62f1").matches(v("2022.3.62f10")));
        assert!(p("6000.5.0b").matches(v("6000.5.0b4")));
        assert!(p("2019.1.0rc1").matches(v("2019.1.0rc1")));
        assert!(p("2022.3.62F1").matches(v("2022.3.62f1")));
    }

    #[test]
    fn wildcards() {
        assert!(p("202*").matches(v("2020.1.0f1")));
        assert!(p("202*").matches(v("2023.2.1f1")));
        assert!(!p("202*").matches(v("6000.0.1f1")));
        assert!(p("2022.3.*").matches(v("2022.3.62f3")));
        assert!(p("2022.x").matches(v("2022.1.5f1")));
        assert!(p("2022.3.6*").matches(v("2022.3.6f1")));
        assert!(p("2022.3.6*").matches(v("2022.3.62f1")));
        assert!(!p("2022.3.6*").matches(v("2022.3.7f1")));
        assert!(p("*.3").matches(v("2021.3.4f1")));
        assert!(p("2022.3.62f*").matches(v("2022.3.62f3")));
    }

    #[test]
    fn exact_and_series_constructors() {
        let version = v("2022.3.5f1");
        assert!(VersionPattern::from(version).matches(version));
        assert!(!VersionPattern::from(version).matches(v("2022.3.5f2")));
        assert!(VersionPattern::series_of(version).matches(v("2022.3.62f3")));
        assert!(!VersionPattern::series_of(v("6000.1.0f1")).matches(v("6000.10.0f1")));
    }

    #[test]
    fn invalid_patterns() {
        for s in [
            "",
            ".",
            "abc",
            "2022..3",
            "2022.3.5.1",
            "2022.3.f1",
            "2022.3.5q1",
            "2022.3.5x",
            "2022-3",
        ] {
            assert!(
                s.parse::<VersionPattern>().is_err(),
                "{s:?} should not parse"
            );
        }
    }

    #[test]
    fn displays_original_text() {
        assert_eq!(p(" 2022.3.* ").to_string(), "2022.3.*");
    }
}
