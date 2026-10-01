//! Retained execution plus its actual policy; no runtime/authority constructor.

use super::AuthenticatedMirPlironPerCompilationVerificationV1;
use fe2o3_kernel_ir::{LogicalStorageCounterV1, LogicalStorageErrorV1};

impl AuthenticatedMirPlironPerCompilationVerificationV1 {
    /// Charge the exact original execution and policy heap payloads.
    ///
    /// The caller already charges this inline wrapper/header once. Execution
    /// delegates its retained receipt chain; the policy contributes LOGICAL
    /// BTree key payload at actual length, not physical nodes/capacity. This
    /// complete named logical model is not allocator/peak/RSS measurement.
    ///
    /// All child events share this checked byte/item counter. Each event costs
    /// one item. Stop at the first error and discard the incomplete observation;
    /// prior accepted charges are not rolled back. No extra root is charged.
    pub(crate) fn charge_retained_heap_storage_v1(
        &self,
        counter: &mut LogicalStorageCounterV1,
    ) -> Result<(), LogicalStorageErrorV1> {
        let Self {
            execution,
            _staging_policy,
        } = self;
        execution.visit_retained_heap_storage_v1(|count, width| {
            let bytes = count
                .checked_mul(width)
                .ok_or(LogicalStorageErrorV1::Arithmetic)?;
            counter.charge(bytes, 1)
        })?;
        _staging_policy.visit_retained_logical_key_payload_v1(|count, width| {
            let bytes = count
                .checked_mul(width)
                .ok_or(LogicalStorageErrorV1::Arithmetic)?;
            counter.charge(bytes, 1)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observe(
        owner: &AuthenticatedMirPlironPerCompilationVerificationV1,
        counter: &mut LogicalStorageCounterV1,
    ) -> Result<(), LogicalStorageErrorV1> {
        owner.charge_retained_heap_storage_v1(counter)
    }

    #[test]
    fn aggregate_join_storage_is_nameable_without_constructing_runtime_authority() {
        let _: fn(
            &AuthenticatedMirPlironPerCompilationVerificationV1,
            &mut LogicalStorageCounterV1,
        ) -> Result<(), LogicalStorageErrorV1> = observe;
    }
}
