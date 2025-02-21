use serde::{Deserialize, Deserializer};
use serde::{Serialize, Serializer, de};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use strum::Display;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ParseError;

impl std::error::Error for ParseError {}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "failed to parse Version")
    }
}

#[derive(Display, Debug, Eq, PartialEq, Ord, PartialOrd, Copy, Clone, Hash)]
pub(crate) enum BuildType {
    Alpha,
    Beta,
    ReleaseCandidate,
    Final,
    FinalPatch,
}

impl BuildType {
    /// Returns the short name of the build type.
    pub(crate) const fn as_short_str(&self) -> &str {
        match self {
            Self::Alpha => "a",
            Self::Beta => "b",
            Self::ReleaseCandidate => "rc",
            Self::Final => "f",
            Self::FinalPatch => "p",
        }
    }

    /// Returns the build type from a string.
    pub(crate) fn from(s: &str) -> Option<Self> {
        if s.contains('f') {
            Some(Self::Final)
        } else if s.contains('b') {
            Some(Self::Beta)
        } else if s.contains('a') {
            Some(Self::Alpha)
        } else if s.contains("rc") {
            Some(Self::ReleaseCandidate)
        } else if s.contains('p') {
            Some(Self::FinalPatch)
        } else {
            None
        }
    }
}

pub(crate) type Major = u16;
pub(crate) type Minor = u8;
pub(crate) type Patch = u8;
pub(crate) type BuildNumber = u8;

/// The Unity version separated into its components.
#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Copy, Clone, Hash)]
pub(crate) struct Version {
    pub(crate) major: Major,
    pub(crate) minor: Minor,
    pub(crate) patch: Patch,
    pub(crate) build_type: BuildType,
    pub(crate) build: BuildNumber,
}

impl Version {
    /// Returns the `major.minor` part of this version.
    pub(crate) fn major_minor_string(self) -> String {
        format!("{}.{}", self.major, self.minor)
    }

    /// Returns the cached string representation of this version.
    pub(crate) fn as_str(self) -> &'static str {
        // Use a thread-local cache as usage is predominantly single-threaded. This avoids having to use a Mutex.
        thread_local! {
            static VERSION_STRINGS: RefCell<HashMap<Version, &'static str>> = RefCell::new(HashMap::new());
        }

        VERSION_STRINGS.with(|versions| {
            let mut versions = versions.borrow_mut();
            *versions
                .entry(self)
                .or_insert_with(|| Box::leak(self.format_string().into_boxed_str()))
        })
    }

    /// Returns the length of the string representation of this version.
    fn string_length(self) -> usize {
        Self::count_digits(self.major.into())
            + Self::count_digits(self.minor.into())
            + Self::count_digits(self.patch.into())
            + self.build_type.as_short_str().len()
            + Self::count_digits(self.build.into())
            + 2 // The 2 dots
    }

    fn count_digits(number: usize) -> usize {
        match number {
            0..=9 => 1,
            10..=99 => 2,
            100..=999 => 3,
            1000..=9999 => 4,
            10000..=99999 => 5,
            _ => number.to_string().len(), // Fallback that, in theory, is never used.
        }
    }

    /// Formats this version into a string.
    fn format_string(self) -> String {
        let capacity = self.string_length();
        let mut s = String::with_capacity(capacity);
        // major.minor.patch.build_type.build
        s.push_str(&self.major.to_string());
        s.push('.');
        s.push_str(&self.minor.to_string());
        s.push('.');
        s.push_str(&self.patch.to_string());
        s.push_str(self.build_type.as_short_str());
        s.push_str(&self.build.to_string());
        s
    }
}

impl FromStr for Version {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
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
        let build_type = BuildType::from(build_part).ok_or(ParseError)?;

        let (patch, build) = build_part
            .split_once(build_type.as_short_str())
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
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl<'de> Deserialize<'de> for Version {
    fn deserialize<D>(deserializer: D) -> Result<Version, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Version::from_str(&s).map_err(de::Error::custom)
    }
}

impl Serialize for Version {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Don't use `as_str` here as it caches the string
        serializer.serialize_str(&self.format_string())
    }
}

#[cfg(test)]
mod version_tests {
    use crate::unity::ParseError;

    use super::{BuildType, Version};

    #[test]
    fn test_version_from_string_f() {
        let v = "2021.2.14f1".parse::<Version>().unwrap();
        assert_eq!(v.major, 2021);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 14);
        assert_eq!(v.build_type, BuildType::Final);
        assert_eq!(v.build, 1);
    }

    #[test]
    fn test_version_from_string_b() {
        let v = "2021.1.1b3".parse::<Version>().unwrap();
        assert_eq!(v.major, 2021);
        assert_eq!(v.minor, 1);
        assert_eq!(v.patch, 1);
        assert_eq!(v.build_type, BuildType::Beta);
        assert_eq!(v.build, 3);
    }

    #[test]
    fn test_version_from_string_a() {
        let v = "2021.1.1a3".parse::<Version>().unwrap();
        assert_eq!(v.major, 2021);
        assert_eq!(v.minor, 1);
        assert_eq!(v.patch, 1);
        assert_eq!(v.build_type, BuildType::Alpha);
        assert_eq!(v.build, 3);
    }

    #[test]
    fn test_version_from_string_rc() {
        let v = "2021.1.1rc1".parse::<Version>().unwrap();
        assert_eq!(v.major, 2021);
        assert_eq!(v.minor, 1);
        assert_eq!(v.patch, 1);
        assert_eq!(v.build_type, BuildType::ReleaseCandidate);
        assert_eq!(v.build, 1);
    }

    #[test]
    fn test_version_from_string_invalid_build_type() {
        assert_eq!("2021.1.1x1".parse::<Version>(), Err(ParseError));
    }

    #[test]
    fn test_version_from_string_invalid() {
        assert_eq!("2021.1.1".parse::<Version>(), Err(ParseError));
    }

    #[test]
    fn test_version_to_string() {
        assert_eq!(
            "2021.2.14f1".parse::<Version>().unwrap().to_string(),
            "2021.2.14f1"
        );

        assert_eq!(
            "2019.1.1b1".parse::<Version>().unwrap().to_string(),
            "2019.1.1b1"
        );

        assert_eq!(
            "2020.1.1a3".parse::<Version>().unwrap().to_string(),
            "2020.1.1a3"
        );

        assert_eq!(
            "2022.2.1rc2".parse::<Version>().unwrap().to_string(),
            "2022.2.1rc2"
        );
    }

    #[test]
    fn test_len() {
        assert_eq!(
            "5.102.5f123".parse::<Version>().unwrap().string_length(),
            "5.102.5f123".len()
        );

        assert_eq!(
            "2021.2.14f1".parse::<Version>().unwrap().string_length(),
            "2021.2.14f1".len()
        );

        assert_eq!(
            "2019.1.1b1".parse::<Version>().unwrap().string_length(),
            "2019.1.1b1".len()
        );

        assert_eq!(
            "2020.1.1a3".parse::<Version>().unwrap().string_length(),
            "2020.1.1a3".len()
        );

        assert_eq!(
            "2022.2.1rc2".parse::<Version>().unwrap().string_length(),
            "2022.2.1rc2".len()
        );
    }
}
