//! Compact rollback records; the complete ordered journal is retained.
use super::*;

pub(super) struct ScopedChange {
    pub(super) variable: u32,
    pub(super) previous: Option<SsaValueV1>,
}

impl ScopedChange {
    pub(super) fn new(
        variable: usize,
        previous: Option<SsaValueV1>,
    ) -> Result<Self, SsaPlannerErrorV1> {
        // This is a compact promotable index, not a source variable identity.
        // The map already bounds it; keep the conversion checked locally.
        Ok(Self {
            variable: u32::try_from(variable).map_err(|_| SsaPlannerErrorV1::IdentityOverflow)?,
            previous,
        })
    }
}

#[cfg(test)]
#[path = "scoped_change_v1/tests.rs"]
mod tests;
