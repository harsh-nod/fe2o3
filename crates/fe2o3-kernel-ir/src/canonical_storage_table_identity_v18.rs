//! Inert table-content identity, independent of module bodies and owner custody.
use super::*;
use sha2::{Digest, Sha256};
use std::mem::size_of;

const DOMAIN: &[u8] = b"FE2O3/CANONICAL-STORAGE-TABLE/V18\0";
const POLICY: u16 = 1;

/// Structural uniquing key only. Equal keys do not establish source provenance,
/// graph/session/epoch custody, initialization or compatibility certificates.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CanonicalStorageTableIdentityV18 {
    digest: [u8; 32],
    encoded_length: u64,
}
impl CanonicalStorageTableIdentityV18 {
    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
    pub const fn encoded_length(&self) -> u64 {
        self.encoded_length
    }
}

type Outcome = Result<CanonicalStorageTableIdentityV18, CanonicalKernelIrReplayAdmissionErrorV18>;

struct Scope<'a, 'work> {
    budget: &'a mut Budget<'work>,
    floor: Option<usize>,
}
impl<'a, 'work> Scope<'a, 'work> {
    fn enter(budget: &'a mut Budget<'work>) -> Result<Self, ResourceError> {
        budget.charge_work(1)?;
        let floor = budget.storage_checkpoint();
        budget.reserve_storage(headers()?)?;
        Ok(Self {
            budget,
            floor: Some(floor),
        })
    }
    fn finish(&mut self) -> Result<(), ResourceError> {
        match self.floor.take() {
            Some(floor) => self.budget.rollback_storage(floor),
            None => Ok(()),
        }
    }
}
impl Drop for Scope<'_, '_> {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

fn headers() -> Result<usize, ResourceError> {
    let slots = [
        (1, size_of::<Scope<'_, '_>>()),
        (2, size_of::<Result<Scope<'_, '_>, ResourceError>>()),
        // Public method, closure, stored outcome and caller result.
        (4, size_of::<Outcome>()),
        (2, size_of::<Result<(), ResourceError>>()),
        (2, size_of::<Result<usize, ResourceError>>()),
        (2, size_of::<CanonicalStorageTableIdentityV18>()),
        (2, size_of::<Sha256>()),
        (2, 32),
    ];
    slots.into_iter().try_fold(
        crate::wire::storage_table_codec_headers_v18()?,
        |sum, (count, size)| {
            size.checked_mul(count)
                .and_then(|n| sum.checked_add(n))
                .ok_or(ResourceError::Arithmetic)
        },
    )
}

impl VerifiedCanonicalKernelIrModuleV18 {
    /// Uses the same V18 row codec, not a parallel table representation.
    /// The exact immutable owner is the only input authority. Scratch is dropped
    /// before restoring the caller floor; accepted work and first denials persist.
    /// The returned fixed-size identity is inert, not an owning graph receipt.
    pub fn storage_table_identity_with_budget_v18(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<CanonicalStorageTableIdentityV18, CanonicalKernelIrReplayAdmissionErrorV18> {
        let mut scope = Scope::enter(budget)?;
        let result = (|| {
            let length =
                crate::wire::count_storage_table_v18(self.module(), scope.budget.work_budget_v1())
                    .map_err(CanonicalKernelIrReplayAdmissionErrorV18::Encode)?;
            scope.budget.reserve_storage(length)?;
            let bytes = crate::wire::encode_counted_storage_table_v18(
                self.module(),
                length,
                scope.budget.work_budget_v1(),
            )
            .map_err(CanonicalKernelIrReplayAdmissionErrorV18::Encode)?;
            let encoded_length = u64::try_from(length).map_err(|_| ResourceError::Arithmetic)?;
            let domain_length =
                u32::try_from(DOMAIN.len()).map_err(|_| ResourceError::Arithmetic)?;
            let work = length
                .checked_add(DOMAIN.len())
                .and_then(|n| n.checked_add(4 + 2 + 8))
                .ok_or(ResourceError::Arithmetic)?;
            scope.budget.charge_work(work)?;
            let mut hash = Sha256::new();
            hash.update(domain_length.to_le_bytes());
            hash.update(DOMAIN);
            hash.update(POLICY.to_le_bytes());
            hash.update(encoded_length.to_le_bytes());
            hash.update(&bytes);
            let identity = CanonicalStorageTableIdentityV18 {
                digest: hash.finalize().into(),
                encoded_length,
            };
            drop(bytes);
            scope.budget.release_storage(length)?;
            Ok(identity)
        })();
        let released = scope.finish();
        match result {
            Err(error) => Err(error),
            Ok(identity) => {
                released?;
                Ok(identity)
            }
        }
    }
}

#[cfg(test)]
#[path = "canonical_storage_table_identity_v18_tests.rs"]
mod tests;
