use winapi::shared::minwindef::{BYTE, DWORD, WORD};

#[derive(PartialEq, PartialOrd, Debug)]
pub struct Version {
    pub major: BYTE,
    pub minor: BYTE,
}

impl Version {
    pub fn new(version: WORD) -> Self {
        Self {
            major: (version & 0xff) as BYTE,
            minor: ((version >> 8) & 0xff) as BYTE,
        }
    }

    pub fn new_explicit(major: BYTE, minor: BYTE) -> Self {
        Self { major, minor }
    }

    pub fn value(&self) -> WORD {
        ((self.minor as WORD) << 8) | self.major as WORD
    }
}

#[derive(Debug)]
pub struct VersionRange {
    pub start: Version,
    pub end: Version,
}

impl VersionRange {
    pub fn new(dw_version: DWORD) -> Self {
        Self {
            start: Version::new((dw_version >> 16) as WORD),
            end: Version::new((dw_version & 0xffff) as WORD),
        }
    }

    pub fn new_explicit(start: Version, end: Version) -> Self {
        Self { start, end }
    }

    pub fn value(&self) -> DWORD {
        ((self.start.value() as DWORD) << 16) | self.end.value() as DWORD
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        let version = Version::new(0x0102);
        assert_eq!(version.minor, 1);
        assert_eq!(version.major, 2);
        assert_eq!(version.value(), 0x0102);
    }

    #[test]
    fn test_range() {
        let range = VersionRange::new(0x01020304);
        assert_eq!(range.start.minor, 1);
        assert_eq!(range.start.major, 2);
        assert_eq!(range.end.minor, 3);
        assert_eq!(range.end.major, 4);
        assert_eq!(range.value(), 0x01020304);
    }
}
