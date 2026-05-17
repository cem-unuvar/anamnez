//! README §Workstation client → Updates — minimum-client-version gate.

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    pub fn parse(s: &str) -> Result<Self> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 {
            return Err(Error::OutdatedClient {
                min: "<unknown>".to_owned(),
                got: s.to_owned(),
            });
        }
        let major: u32 = parts[0]
            .parse()
            .map_err(|_| Error::Invariant("version major parse"))?;
        let minor: u32 = parts[1]
            .parse()
            .map_err(|_| Error::Invariant("version minor parse"))?;
        let patch: u32 = parts[2]
            .parse()
            .map_err(|_| Error::Invariant("version patch parse"))?;
        Ok(Self {
            major,
            minor,
            patch,
        })
    }

    #[must_use]
    pub fn display(&self) -> String {
        format!("{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Returns `Err(Error::OutdatedClient)` if `client < min`.
pub fn check(min: &Version, client: &Version) -> Result<()> {
    let cmp = (client.major, client.minor, client.patch).cmp(&(min.major, min.minor, min.patch));
    if cmp.is_lt() {
        Err(Error::OutdatedClient {
            min: min.display(),
            got: client.display(),
        })
    } else {
        Ok(())
    }
}
