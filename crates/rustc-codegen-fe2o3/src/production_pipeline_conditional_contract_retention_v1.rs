//! Inert contract storage owned by the existing conditional proof transaction.
use crate::compiler_descriptor::conditional_generated_fields_v1::ConditionalGeneratedFieldErrorV1 as Error;
use fe2o3_kernel_descriptor::MAX_CONDITIONAL_INVOCATION_BYTES_V1;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};

/// Non-Clone storage, not a receipt, admission token, or launch capability.
/// Only the generated-field callback copies bytes into this owner. Its original
/// root reserves them after leaving the prepaid scope and retains them only
/// after all enclosing source, graph, proof, and account postchecks succeed.
pub(crate) struct RetainedConditionalContractV1 {
    bytes: Vec<u8>,
}

impl RetainedConditionalContractV1 {
    pub(super) fn copy_unreserved(bytes: &[u8], budget: &mut Budget<'_>) -> Result<Self, Error> {
        if bytes.is_empty() || bytes.len() > MAX_CONDITIONAL_INVOCATION_BYTES_V1 {
            return Err(Error::Mismatch("bounded encoded conditional contract"));
        }
        budget.charge_work(bytes.len().checked_add(1).ok_or(Resource::Arithmetic)?)?;
        budget.reserve_storage(
            bytes
                .len()
                .checked_add(std::mem::size_of::<Self>())
                .ok_or(Resource::Arithmetic)?,
        )?;
        let mut owned = Vec::new();
        owned
            .try_reserve_exact(bytes.len())
            .map_err(|_| Resource::Allocation)?;
        if owned.capacity() != bytes.len() {
            return Err(Resource::Accounting.into());
        }
        owned.extend_from_slice(bytes);
        Ok(Self { bytes: owned })
    }

    pub(crate) fn retained_storage_v1(&self) -> usize {
        // Construction bounds capacity to the small wire limit before addition.
        std::mem::size_of::<Self>() + self.bytes.capacity()
    }

    #[cfg(test)]
    pub(crate) fn observed_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The root has already reserved `self` on the original ledger, outside the
    /// prepaid generated-field scope. Both old and new owners remain charged
    /// through replay; only a successful replay may replace the old payload.
    pub(crate) fn finish_replay_v1(
        self,
        previous: Option<Self>,
        transferred_arena: usize,
        floor: usize,
        budget: &mut Budget<'_>,
    ) -> Result<Self, Error> {
        let previous_storage = previous.as_ref().map_or(0, Self::retained_storage_v1);
        let expected = floor
            .checked_add(transferred_arena)
            .and_then(|n| n.checked_add(self.retained_storage_v1()))
            .ok_or(Resource::Arithmetic)?;
        if floor < previous_storage || budget.storage() != expected {
            return Err(Resource::Accounting.into());
        }
        if let Some(previous) = &previous {
            budget.charge_work(
                self.bytes
                    .len()
                    .checked_add(1)
                    .ok_or(Resource::Arithmetic)?,
            )?;
            if self.bytes != previous.bytes {
                return Err(Error::Mismatch("retained conditional contract changed"));
            }
        }
        // Destroy the replaced payload before refunding its charge. The lower
        // continuation has already transferred the arena into its returned owner.
        drop(previous);
        budget.release_storage(previous_storage)?;
        budget.release_storage(transferred_arena)?;
        Ok(self)
    }
}

#[cfg(test)]
#[path = "production_pipeline_conditional_contract_retention_v1_tests.rs"]
mod tests;
