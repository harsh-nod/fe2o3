//! Lossless private edge coordinates with a distinct absent-table-slot value.
use std::num::NonZeroU64;

use super::SsaPlannerErrorV1;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct IncomingEdge(NonZeroU64);

impl IncomingEdge {
    pub(super) fn new(source: usize, ordinal: usize) -> Result<Self, SsaPlannerErrorV1> {
        let source = u32::try_from(source).map_err(|_| SsaPlannerErrorV1::IdentityOverflow)?;
        let ordinal = u32::try_from(ordinal).map_err(|_| SsaPlannerErrorV1::IdentityOverflow)?;
        // Admitted block/edge limits are below u32::MAX. Keep the addition
        // checked so an out-of-domain pair cannot collide with None.
        let encoded = ((u64::from(source) << 32) | u64::from(ordinal))
            .checked_add(1)
            .and_then(NonZeroU64::new)
            .ok_or(SsaPlannerErrorV1::IdentityOverflow)?;
        Ok(Self(encoded))
    }

    pub(super) fn indices(self) -> (usize, usize) {
        let encoded = self.0.get() - 1;
        ((encoded >> 32) as usize, (encoded as u32) as usize)
    }
}
