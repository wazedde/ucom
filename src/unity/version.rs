use std::fmt::{Display, Formatter};
use std::str::FromStr;

#[derive(Debug, Eq, PartialEq)]
pub struct ParseError;

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Copy, Clone)]
pub enum BuildType {
    Alpha,
    Beta,
    ReleaseCandidate,
    Final,
}

impl BuildType {
    /// Returns the short name of the build type.
    pub const fn as_short_str(&self) -> &str {
        match self {
            Self::Alpha => "a",
            Self::Beta => "b",
            Self::ReleaseCandidate => "rc",
            Self::Final => "f",
        }
    }

    /// Returns the full name of the build type.
    pub const fn as_full_str(&self) -> &str {
        match self {
            Self::Alpha => "Alpha",
            Self::Beta => "Beta",
            Self::ReleaseCandidate => "ReleaseCandidate",
            Self::Final => "Final",
        }
    }

    /// Returns the build type from a string.
    pub fn from(s: &str) -> Option<Self> {
        if s.contains('f') {
            Some(Self::Final)
        } else if s.contains('b') {
            Some(Self::Beta)
        } else if s.contains('a') {
            Some(Self::Alpha)
        } else if s.contains("rc") {
            Some(Self::ReleaseCandidate)
        } else {
            None
        }
    }
}

impl Display for BuildType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_short_str())
    }
}

pub type Major = u16;
pub type Minor = u8;
pub type Patch = u8;
pub type BuildNumber = u8;

/// The Unity version separated into its components.
#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Copy, Clone)]
pub struct Version {
    pub major: Major,
    pub minor: Minor,
    pub patch: Patch,
    pub build_type: BuildType,
    pub build: BuildNumber,
}

impl Version {
    /// Returns the length of the string representation of this version.
    pub fn len(self) -> usize {
        Self::calculate_the_number_of_digits(self.major.into())
            + Self::calculate_the_number_of_digits(self.minor.into())
            + Self::calculate_the_number_of_digits(self.patch.into())
            + self.build_type.as_short_str().len()
            + Self::calculate_the_number_of_digits(self.build.into())
            + 2 // The 2 dots
    }

    /// Returns the major.minor part of this version.
    pub fn minor_partial(self) -> String {
        format!("{}.{}", self.major, self.minor)
    }

    fn calculate_the_number_of_digits(number: usize) -> usize {
        // The maximum number of digits for a version number is 5.
        match number {
            0..=9 => 1,
            10..=99 => 2,
            100..=999 => 3,
            1000..=9999 => 4,
            _ => 5,
        }
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
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}.{}.{}{}{}",
            self.major, self.minor, self.patch, self.build_type, self.build
        )
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
            "5.102.5f123".parse::<Version>().unwrap().len(),
            "5.102.5f123".len()
        );

        assert_eq!(
            "2021.2.14f1".parse::<Version>().unwrap().len(),
            "2021.2.14f1".len()
        );

        assert_eq!(
            "2019.1.1b1".parse::<Version>().unwrap().len(),
            "2019.1.1b1".len()
        );

        assert_eq!(
            "2020.1.1a3".parse::<Version>().unwrap().len(),
            "2020.1.1a3".len()
        );

        assert_eq!(
            "2022.2.1rc2".parse::<Version>().unwrap().len(),
            "2022.2.1rc2".len()
        );
    }
}
