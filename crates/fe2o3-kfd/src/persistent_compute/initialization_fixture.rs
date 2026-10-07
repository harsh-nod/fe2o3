#![cfg(test)]

use super::PersistentComputeInitializationV1;

impl PersistentComputeInitializationV1 {
    pub(crate) fn from_test_parts(digest: Option<[u8; 32]>, initialized: bool) -> Self {
        match (digest, initialized) {
            (Some(digest), true) => Self::AuthenticatedH2d(digest),
            (None, true) => Self::AfterDispatch,
            (None, false) => Self::Uninitialized,
            (Some(_), false) => panic!("invalid initialization fixture"),
        }
    }
}
