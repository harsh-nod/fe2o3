#![forbid(unsafe_code)]

use std::{error::Error, fmt};

const INVALID_ID: u32 = u32::MAX;

/// Kernel credential identity pinned for the independently operated external-anchor service.
///
/// This inert value identifies the expected Unix-socket peer UID and GID. It grants no endpoint,
/// signing, monotonic-storage, compiler, publication, or execution authority. Production evidence
/// must also retain the exact peer process and verify observations under the distinct anchor key in
/// the compiler issuer policy.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerExecutionExternalAnchorServiceIdentityV1 {
    uid: u32,
    gid: u32,
}

impl CompilerExecutionExternalAnchorServiceIdentityV1 {
    /// Constructs one dedicated non-root external-anchor service identity.
    pub const fn new(
        uid: u32,
        gid: u32,
    ) -> Result<Self, CompilerExecutionExternalAnchorServiceIdentityErrorV1> {
        if uid == 0 || uid == INVALID_ID {
            return Err(CompilerExecutionExternalAnchorServiceIdentityErrorV1::InvalidUid);
        }
        if gid == 0 || gid == INVALID_ID {
            return Err(CompilerExecutionExternalAnchorServiceIdentityErrorV1::InvalidGid);
        }
        Ok(Self { uid, gid })
    }

    /// Returns the exact expected effective UID from `SO_PEERCRED`.
    pub const fn uid(self) -> u32 {
        self.uid
    }

    /// Returns the exact expected effective GID from `SO_PEERCRED`.
    pub const fn gid(self) -> u32 {
        self.gid
    }
}

impl fmt::Debug for CompilerExecutionExternalAnchorServiceIdentityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilerExecutionExternalAnchorServiceIdentityV1")
            .field("uid", &self.uid)
            .field("gid", &self.gid)
            .field("authority", &"none")
            .finish()
    }
}

/// Invalid dedicated external-anchor service credential identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CompilerExecutionExternalAnchorServiceIdentityErrorV1 {
    /// UID zero and the Linux `-1` sentinel are not dedicated service identities.
    InvalidUid,
    /// GID zero and the Linux `-1` sentinel are not dedicated service identities.
    InvalidGid,
}

impl fmt::Display for CompilerExecutionExternalAnchorServiceIdentityErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidUid => "invalid compiler external-anchor service UID",
            Self::InvalidGid => "invalid compiler external-anchor service GID",
        })
    }
}

impl Error for CompilerExecutionExternalAnchorServiceIdentityErrorV1 {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_rejects_privileged_and_sentinel_credentials() {
        assert_eq!(
            CompilerExecutionExternalAnchorServiceIdentityV1::new(0, 1),
            Err(CompilerExecutionExternalAnchorServiceIdentityErrorV1::InvalidUid)
        );
        assert_eq!(
            CompilerExecutionExternalAnchorServiceIdentityV1::new(u32::MAX, 1),
            Err(CompilerExecutionExternalAnchorServiceIdentityErrorV1::InvalidUid)
        );
        assert_eq!(
            CompilerExecutionExternalAnchorServiceIdentityV1::new(1, 0),
            Err(CompilerExecutionExternalAnchorServiceIdentityErrorV1::InvalidGid)
        );
        assert_eq!(
            CompilerExecutionExternalAnchorServiceIdentityV1::new(1, u32::MAX),
            Err(CompilerExecutionExternalAnchorServiceIdentityErrorV1::InvalidGid)
        );
        let identity = CompilerExecutionExternalAnchorServiceIdentityV1::new(1, 2).unwrap();
        assert_eq!(identity.uid(), 1);
        assert_eq!(identity.gid(), 2);
    }
}
