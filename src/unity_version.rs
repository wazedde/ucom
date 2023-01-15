use std::fmt::{Display, Formatter};
use std::str::{FromStr, Split};

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Copy, Clone)]
pub enum BuildType {
    Alpha,
    Beta,
    ReleaseCandidate,
    Final,
}

pub struct BuildTypeParseError;

impl BuildType {
    pub fn as_str(&self) -> &str {
        match self {
            BuildType::Alpha => "a",
            BuildType::Beta => "b",
            BuildType::ReleaseCandidate => "rc",
            BuildType::Final => "f",
        }
    }

    pub fn find_in(s: &str) -> Option<BuildType> {
        if s.contains('f') {
            Some(BuildType::Final)
        } else if s.contains('b') {
            Some(BuildType::Beta)
        } else if s.contains('a') {
            Some(BuildType::Alpha)
        } else if s.contains("rc") {
            Some(BuildType::ReleaseCandidate)
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
    type Err = BuildTypeParseError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "a" => Ok(BuildType::Alpha),
            "b" => Ok(BuildType::Beta),
            "rc" => Ok(BuildType::ReleaseCandidate),
            "f" => Ok(BuildType::Final),
            _ => Err(BuildTypeParseError),
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

pub struct UnityVersionParseError;

impl FromStr for UnityVersion {
    type Err = UnityVersionParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split('.');
        let year = parts
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or(UnityVersionParseError)?;
        let point = parts
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or(UnityVersionParseError)?;

        let build_type = BuildType::find_in(s).ok_or(UnityVersionParseError)?;

        let mut build_parts: Split<&str> = parts
            .next()
            .ok_or(UnityVersionParseError)?
            .split(build_type.as_str());
        let patch = build_parts
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or(UnityVersionParseError)?;
        let build = build_parts
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or(UnityVersionParseError)?;

        Ok(UnityVersion {
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
