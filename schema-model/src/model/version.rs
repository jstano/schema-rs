use std::cmp::Ordering;
use std::fmt;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct Version {
    major_version: i32,
    minor_version: i32,
    patch_version: i32,
    pre_release_suffix: bool,
}

impl Version {
    pub fn new(major_version: i32, minor_version: i32) -> Self {
        Self {
            major_version,
            minor_version,
            patch_version: 0,
            pre_release_suffix: false,
        }
    }

    pub fn with_patch(major_version: i32, minor_version: i32, patch_version: i32) -> Self {
        Self {
            major_version,
            minor_version,
            patch_version,
            pre_release_suffix: false,
        }
    }

    pub fn with_patch_and_suffix(
        major_version: i32,
        minor_version: i32,
        patch_version: i32,
        pre_release_suffix: bool,
    ) -> Self {
        Self {
            major_version,
            minor_version,
            patch_version,
            pre_release_suffix,
        }
    }

    pub fn parse(version_str: &str) -> Self {
        // Detect "-SNAPSHOT" suffix
        let mut s = version_str.trim();
        let pre = s.contains("-SNAPSHOT");
        if pre {
            if let Some(idx) = s.find("-SNAPSHOT") {
                s = &s[..idx];
            }
        }
        let parts: Vec<&str> = s.split('.').collect();
        let major = parts
            .get(0)
            .and_then(|p| p.parse::<i32>().ok())
            .unwrap_or(0);
        let minor = parts
            .get(1)
            .and_then(|p| p.parse::<i32>().ok())
            .unwrap_or(0);
        let patch = parts
            .get(2)
            .and_then(|p| p.parse::<i32>().ok())
            .unwrap_or(0);
        Self {
            major_version: major,
            minor_version: minor,
            patch_version: patch,
            pre_release_suffix: pre,
        }
    }

    pub fn major_version(&self) -> i32 {
        self.major_version
    }
    pub fn minor_version(&self) -> i32 {
        self.minor_version
    }
    pub fn patch_version(&self) -> i32 {
        self.patch_version
    }
    pub fn is_pre_release_suffix(&self) -> bool {
        self.pre_release_suffix
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.pre_release_suffix {
            if self.patch_version < 1 {
                write!(
                    f,
                    "{:02}.{:02}-SNAPSHOT",
                    self.major_version, self.minor_version
                )
            } else {
                write!(
                    f,
                    "{:02}.{:02}.{:02}-SNAPSHOT",
                    self.major_version, self.minor_version, self.patch_version
                )
            }
        } else if self.patch_version < 1 {
            write!(f, "{:02}.{:02}", self.major_version, self.minor_version)
        } else {
            write!(
                f,
                "{:02}.{:02}.{:02}",
                self.major_version, self.minor_version, self.patch_version
            )
        }
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        if std::ptr::eq(self, other) {
            return Ordering::Equal;
        }
        match self.major_version.cmp(&other.major_version) {
            Ordering::Equal => {}
            ord => return ord,
        }
        match self.minor_version.cmp(&other.minor_version) {
            Ordering::Equal => {}
            ord => return ord,
        }
        match self.patch_version.cmp(&other.patch_version) {
            Ordering::Equal => {}
            ord => return ord,
        }
        match (self.pre_release_suffix, other.pre_release_suffix) {
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            _ => Ordering::Equal,
        }
    }
}

impl From<&str> for Version {
    fn from(value: &str) -> Self {
        Version::parse(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_display() {
        let v = Version::parse("1.2");
        assert_eq!(v.major_version(), 1);
        assert_eq!(v.minor_version(), 2);
        assert_eq!(v.patch_version(), 0);
        assert!(!v.is_pre_release_suffix());
        assert_eq!(v.to_string(), "01.02");

        let v2 = Version::parse("1.2.3-SNAPSHOT");
        assert_eq!(v2.to_string(), "01.02.03-SNAPSHOT");
        assert!(v2.is_pre_release_suffix());
    }

    #[test]
    fn ordering_matches_java_logic() {
        let a = Version::with_patch(1, 2, 0);
        let b = Version::with_patch(1, 2, 1);
        assert!(a < b);

        let c = Version::with_patch_and_suffix(1, 2, 3, true);
        let d = Version::with_patch_and_suffix(1, 2, 3, false);
        // Snapshot considered greater when numbers equal
        assert!(c > d);
    }
}
