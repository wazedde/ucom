use std::fmt::{Display, Formatter};
use std::str::{FromStr, Split};

#[derive(Debug)]
pub struct ParseError;

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Copy, Clone)]
pub enum BuildType {
    Alpha,
    Beta,
    ReleaseCandidate,
    Final,
}

impl BuildType {
    #[must_use]
    pub const fn as_str(&self) -> &str {
        match self {
            Self::Alpha => "a",
            Self::Beta => "b",
            Self::ReleaseCandidate => "rc",
            Self::Final => "f",
        }
    }

    #[must_use]
    pub fn find_in(s: &str) -> Option<Self> {
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
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for BuildType {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "a" => Ok(Self::Alpha),
            "b" => Ok(Self::Beta),
            "rc" => Ok(Self::ReleaseCandidate),
            "f" => Ok(Self::Final),
            _ => Err(ParseError),
        }
    }
}

pub type VersionYear = u16;
pub type VersionPoint = u8;
pub type VersionPatch = u8;
pub type VersionBuild = u8;

/// The Unity version separated into its components.
#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Copy, Clone)]
pub struct UnityVersion {
    pub year: VersionYear,
    pub point: VersionPoint,
    pub patch: VersionPatch,
    pub build_type: BuildType,
    pub build: VersionBuild,
}

impl FromStr for UnityVersion {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split('.');
        let year = parts
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or(ParseError)?;
        let point = parts
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or(ParseError)?;

        let build_type = BuildType::find_in(s).ok_or(ParseError)?;

        let mut build_parts: Split<'_, &str> =
            parts.next().ok_or(ParseError)?.split(build_type.as_str());
        let patch = build_parts
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or(ParseError)?;
        let build = build_parts
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or(ParseError)?;

        Ok(Self {
            year,
            point,
            patch,
            build_type,
            build,
        })
    }
}

impl Display for UnityVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}.{}.{}{}{}",
            self.year, self.point, self.patch, self.build_type, self.build
        )
    }
}

#[cfg(test)]
mod version_tests {
    use std::str::FromStr;

    use super::{BuildType, UnityVersion};

    #[test]
    fn test_version_from_string_f() {
        let version = UnityVersion::from_str("2021.2.14f1").unwrap();
        assert_eq!(version.year, 2021);
        assert_eq!(version.point, 2);
        assert_eq!(version.patch, 14);
        assert_eq!(version.build_type, BuildType::Final);
        assert_eq!(version.build, 1);
    }

    #[test]
    fn test_version_from_string_b() {
        let version = UnityVersion::from_str("2021.1.1b3").unwrap();
        assert_eq!(version.year, 2021);
        assert_eq!(version.point, 1);
        assert_eq!(version.patch, 1);
        assert_eq!(version.build_type, BuildType::Beta);
        assert_eq!(version.build, 3);
    }

    #[test]
    fn test_version_from_string_a() {
        let version = UnityVersion::from_str("2021.1.1a3").unwrap();
        assert_eq!(version.year, 2021);
        assert_eq!(version.point, 1);
        assert_eq!(version.patch, 1);
        assert_eq!(version.build_type, BuildType::Alpha);
        assert_eq!(version.build, 3);
    }

    #[test]
    fn test_version_from_string_rc() {
        let version = UnityVersion::from_str("2021.1.1rc1").unwrap();
        assert_eq!(version.year, 2021);
        assert_eq!(version.point, 1);
        assert_eq!(version.patch, 1);
        assert_eq!(version.build_type, BuildType::ReleaseCandidate);
        assert_eq!(version.build, 1);
    }

    #[test]
    fn test_version_from_string_invalid_build_type() {
        let version = UnityVersion::from_str("2021.1.1x1");
        assert!(version.is_err());
    }

    #[test]
    fn test_version_from_string_invalid() {
        let version = UnityVersion::from_str("2021.1.1");
        assert!(version.is_err());
    }

    #[test]
    fn test_version_to_string() {
        let version = UnityVersion::from_str("2021.2.14f1").unwrap();
        assert_eq!(version.to_string(), "2021.2.14f1");

        let version = UnityVersion::from_str("2019.1.1b1").unwrap();
        assert_eq!(version.to_string(), "2019.1.1b1");

        let version = UnityVersion::from_str("2020.1.1a3").unwrap();
        assert_eq!(version.to_string(), "2020.1.1a3");

        let version = UnityVersion::from_str("2022.2.1rc2").unwrap();
        assert_eq!(version.to_string(), "2022.2.1rc2");
    }
}
