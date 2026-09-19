//! One canonical local union for raw, receiver and safe-core shift temporaries.

use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBlockIdentityV1, SemanticFunctionIdentityV1, SemanticLocalIdV1, SemanticLocalIdentityV1,
};

use crate::rustc_semantic_adapter_v1::SemanticIdentityDigestV1;

// Charge the entire mixed table before a filter can skip non-wrapping rows.
pub(crate) fn prepaid_rows_v1<T, E>(
    rows: &[T],
    mut charge: impl FnMut(usize) -> Result<(), E>,
) -> Result<std::slice::Iter<'_, T>, E> {
    charge(rows.len())?;
    Ok(rows.iter())
}

pub(crate) fn identities_v1(
    function: SemanticFunctionIdentityV1,
    block: SemanticBlockIdentityV1,
    callee: SemanticFunctionIdentityV1,
) -> [SemanticLocalIdentityV1; 2] {
    [0_u8, 1].map(|ordinal| {
        let mut digest =
            SemanticIdentityDigestV1::new(b"fe2o3/semantic-mir/safe-core-wrapping-shift/local/v1");
        digest.field(function.as_bytes());
        digest.field(block.as_bytes());
        digest.field(callee.as_bytes());
        digest.field(&[ordinal]);
        SemanticLocalIdentityV1::from_sha256(digest.finish())
    })
}

#[derive(Debug)]
pub(crate) enum Error<E> {
    Resource(E),
    Allocation,
    Invalid,
}

pub(crate) struct LocalOrderV1 {
    identities: Vec<SemanticLocalIdentityV1>,
}

impl LocalOrderV1 {
    pub(crate) fn new<E>(
        raw: impl ExactSizeIterator<Item = SemanticLocalIdentityV1>,
        inserted: &[SemanticLocalIdentityV1],
        mut charge: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Self, Error<E>> {
        let count = raw
            .len()
            .checked_add(inserted.len())
            .ok_or(Error::Invalid)?;
        let count_u32 = u32::try_from(count).map_err(|_| Error::Invalid)?;
        // A logical comparison allowance is prepaid for the bounded sort;
        // subsequent lookups independently debit their binary-search allowance.
        let levels = if count_u32 <= 1 {
            0
        } else {
            u32::BITS - (count_u32 - 1).leading_zeros()
        };
        let work = count
            .checked_mul(2 * levels as usize + 2)
            .ok_or(Error::Invalid)?;
        charge(work).map_err(Error::Resource)?;
        let mut identities = Vec::new();
        identities
            .try_reserve_exact(count)
            .map_err(|_| Error::Allocation)?;
        identities.extend(raw);
        identities.extend_from_slice(inserted);
        identities.sort_unstable();
        if identities.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(Error::Invalid);
        }
        Ok(Self { identities })
    }

    pub(crate) fn index<E>(
        &self,
        identity: SemanticLocalIdentityV1,
        mut charge: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<SemanticLocalIdV1, Error<E>> {
        let work = if self.identities.is_empty() {
            1
        } else {
            (usize::BITS - self.identities.len().leading_zeros()) as usize + 1
        };
        charge(work).map_err(Error::Resource)?;
        let index = self
            .identities
            .binary_search(&identity)
            .map_err(|_| Error::Invalid)?;
        Ok(SemanticLocalIdV1::from_index(
            u32::try_from(index).map_err(|_| Error::Invalid)?,
        ))
    }
}

#[cfg(test)]
#[path = "production_safe_core_shift_locals_v1_tests.rs"]
mod tests;
