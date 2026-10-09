// Dart source: dart_style lib/src/dart_version_history.dart

//! The Dart language versions that the formatter supports.

use std::fmt;

/// A Dart language version (`major.minor`, Dart `Version(major, minor, 0)`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
}

impl Version {
    pub const fn new(major: u32, minor: u32) -> Version {
        Version { major, minor }
    }

    /// Dart `majorMinor`: `12.34`.
    pub fn major_minor(self) -> String {
        format!("{}.{}", self.major, self.minor)
    }

    /// Parses `3.7` or `3.7.0`.
    pub fn parse(text: &str) -> Option<Version> {
        let mut parts = text.trim().split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        Some(Version { major, minor })
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.0", self.major, self.minor)
    }
}

/// The Dart language versions that the formatter supports.
pub struct DartVersionHistory;

const HIGHEST_2X_MINOR_VERSION: u32 = 19;

/// The minor version number of the highest 3.x language version supported by
/// the formatter.
const HIGHEST_3X_MINOR_VERSION: u32 = 13;

impl DartVersionHistory {
    /// The lowest language version supported by the formatter.
    pub const EARLIEST: Version = Version::new(2, 12);

    /// The highest language version supported by the formatter.
    pub const LATEST: Version = Version::new(3, HIGHEST_3X_MINOR_VERSION);

    /// The highest language version that uses the short formatting style.
    pub const LATEST_SHORT_STYLE: Version = Version::new(3, 6);

    /// The lowest language version that uses the tall formatting style.
    pub const EARLIEST_TALL_STYLE: Version = Version::new(3, 7);

    /// All supported language versions.
    pub fn all() -> Vec<Version> {
        let mut all = Vec::new();
        for i in 12..=HIGHEST_2X_MINOR_VERSION {
            all.push(Version::new(2, i));
        }
        for i in 0..=HIGHEST_3X_MINOR_VERSION {
            all.push(Version::new(3, i));
        }
        all
    }

    /// Returns the Dart language version that precedes [version].
    pub fn before(version: Version) -> Version {
        let all = Self::all();
        match all.iter().position(|&v| v == version) {
            None => panic!("{version} isn't a Dart language version."),
            Some(0) => panic!("{version} is the first supported version."),
            Some(index) => all[index - 1],
        }
    }

    /// Returns the Dart language version that follows [version].
    pub fn after(version: Version) -> Version {
        let all = Self::all();
        match all.iter().position(|&v| v == version) {
            None => panic!("{version} isn't a Dart language version."),
            Some(index) if index >= all.len() - 1 => {
                panic!("{version} is the latest supported version.")
            }
            Some(index) => all[index + 1],
        }
    }
}
