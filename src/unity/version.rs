use serde::{Deserialize, Deserializer};
use serde::{Serialize, Serializer, de};
use std::fmt;
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use strum::Display;

//
// Error  implementation
//

/// Errors that can occur when parsing a [`Version`].
#[derive(Debug, Eq, PartialEq)]
pub struct ParseError;

impl std::error::Error for ParseError {}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "failed to parse Version")
    }
}

//
// Type aliases for the version components.
//

pub type Major = u16;
pub type Minor = u8;
pub type Patch = u8;
pub type BuildNumber = u8;

//
// Version components
//

/// The type of build.
#[derive(Display, Debug, Eq, PartialEq, Ord, PartialOrd, Copy, Clone, Hash)]
pub enum BuildType {
    Alpha,
    Beta,
    ReleaseCandidate,
    Final,
    FinalPatch,
}

impl FromStr for BuildType {
    type Err = ParseError;

    /// Returns the [`BuildType`] from a string.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.contains('f') {
            Ok(Self::Final)
        } else if s.contains('b') {
            Ok(Self::Beta)
        } else if s.contains('a') {
            Ok(Self::Alpha)
        } else if s.contains("rc") {
            Ok(Self::ReleaseCandidate)
        } else if s.contains('p') {
            Ok(Self::FinalPatch)
        } else {
            Err(ParseError)
        }
    }
}

impl BuildType {
    /// Returns the [`BuildType`] for an exact short name (`a`, `b`, `rc`, `f` or `p`).
    pub fn from_short_str(s: &str) -> Option<Self> {
        [
            Self::Alpha,
            Self::Beta,
            Self::ReleaseCandidate,
            Self::Final,
            Self::FinalPatch,
        ]
        .into_iter()
        .find(|build_type| build_type.to_short_str() == s)
    }

    /// Returns the short name of the [`BuildType`].
    pub const fn to_short_str(self) -> &'static str {
        match self {
            Self::Alpha => "a",
            Self::Beta => "b",
            Self::ReleaseCandidate => "rc",
            Self::Final => "f",
            Self::FinalPatch => "p",
        }
    }
}

/// The Unity version separated into its components.
#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Copy, Clone, Hash)]
pub struct Version {
    pub major: Major,
    pub minor: Minor,
    pub patch: Patch,
    pub build_type: BuildType,
    pub build: BuildNumber,
}

impl FromStr for Version {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // version format: major.minor.patch.build_type.build
        let mut parts = s.split('.');

        let major = parts
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or(ParseError)?;

        let minor = parts
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or(ParseError)?;

        let build_part = parts.next().ok_or(ParseError)?;
        let build_type: BuildType = build_part.parse()?;

        let (patch, build) = build_part
            .split_once(build_type.to_short_str())
            .and_then(|(l, r)| l.parse().ok().zip(r.parse().ok()))
            .ok_or(ParseError)?;

        Ok(Self {
            major,
            minor,
            patch,
            build_type,
            build,
        })
    }
}

impl Display for Version {
    /// Writes the version, for example `2022.3.5f1`. Width, fill and alignment are honoured.
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let Self {
            major,
            minor,
            patch,
            build_type,
            build,
        } = self;
        let build_type = build_type.to_short_str();

        if f.width().is_none() && f.precision().is_none() {
            write!(f, "{major}.{minor}.{patch}{build_type}{build}")
        } else {
            f.pad(&format!("{major}.{minor}.{patch}{build_type}{build}"))
        }
    }
}

impl<'de> Deserialize<'de> for Version {
    /// Deserializes a [`Version`] from a string.
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::from_str(&s).map_err(de::Error::custom)
    }
}

impl Serialize for Version {
    /// Serializes a [`Version`] to a string.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

impl Version {
    /// Returns whether both versions have the same major and minor version.
    pub const fn same_minor(self, other: Self) -> bool {
        self.major == other.major && self.minor == other.minor
    }

    /// Returns the `major.minor` part of this version.
    pub fn to_major_minor_string(self) -> String {
        format!("{}.{}", self.major, self.minor)
    }

    /// Returns the length of the string representation of this version, without formatting it.
    pub const fn format_len(self) -> usize {
        Self::count_digits(self.major as u32)
            + Self::count_digits(self.minor as u32)
            + Self::count_digits(self.patch as u32)
            + self.build_type.to_short_str().len()
            + Self::count_digits(self.build as u32)
            + 2 // The 2 dots
    }

    const fn count_digits(number: u32) -> usize {
        match number.checked_ilog10() {
            Some(log) => log as usize + 1,
            None => 1,
        }
    }
}

//
// Tests
//

#[cfg(test)]
mod tests {
    use super::{BuildType, ParseError, Version};

    #[test]
    fn versions_are_parsed_by_component() {
        for (text, parts) in [
            ("2021.2.14f1", (2021, 2, 14, BuildType::Final, 1)),
            ("2021.1.1b3", (2021, 1, 1, BuildType::Beta, 3)),
            ("2021.1.1a3", (2021, 1, 1, BuildType::Alpha, 3)),
            ("2021.1.1rc1", (2021, 1, 1, BuildType::ReleaseCandidate, 1)),
        ] {
            let v = text.parse::<Version>().unwrap();
            assert_eq!(
                (v.major, v.minor, v.patch, v.build_type, v.build),
                parts,
                "{text}"
            );
        }
    }

    #[test]
    fn invalid_versions_are_rejected() {
        assert_eq!("2021.1.1x1".parse::<Version>(), Err(ParseError));
        assert_eq!("2021.1.1".parse::<Version>(), Err(ParseError));
    }

    #[test]
    fn versions_display_as_parsed() {
        for text in [
            "2021.2.14f1",
            "2019.1.1b1",
            "2020.1.1a3",
            "2022.2.1rc2",
            "5.102.5f123",
        ] {
            let v = text.parse::<Version>().unwrap();
            assert_eq!(v.to_string(), text);
            assert_eq!(v.format_len(), text.len(), "{text}");
        }
    }

    #[test]
    fn display_honours_the_width() {
        let v = "2022.3.5f1".parse::<Version>().unwrap();
        assert_eq!(format!("{v:<12}|"), "2022.3.5f1  |");
        assert_eq!(format!("{v:>12}|"), "  2022.3.5f1|");
        assert_eq!(format!("{v:<4}|"), "2022.3.5f1|");
        assert_eq!(format!("{v}|"), "2022.3.5f1|");
    }
}
