//! Complete named typed-descriptor branch; not a whole bindings/action report.
use super::AuthenticatedProductionBindings;
use crate::compiler_descriptor::TypedDescriptorRootV1;
use fe2o3_kernel_ir::{LogicalStorageCounterV1, LogicalStorageErrorV1};

impl AuthenticatedProductionBindings {
    /// The enclosing caller pays this bindings header; only the actual typed
    /// descriptor Vec and nested payloads are included here. All other fields
    /// remain explicit separate obligations. First refusal retains only a
    /// partial ledger prefix, which the caller must discard. This read-only
    /// observation never constructs or authenticates a production owner.
    pub(super) fn charge_typed_descriptor_retained_heap_v1(
        &self,
        counter: &mut LogicalStorageCounterV1,
    ) -> Result<(), LogicalStorageErrorV1> {
        let Self {
            context_entries: _,
            rustc_identity_inventory: _,
            rustc_preflight_plan: _,
            rustc_target: _,
            reference_effect_bindings: _,
            debug_source_files: _,
            debug_source_scopes: _,
            debug_source_variables: _,
            debug_capture_gap: _,
            typed_descriptor_roots,
            transaction: _,
        } = self;
        TypedDescriptorRootV1::charge_roster_retained_heap_v1(typed_descriptor_roots, counter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authenticated_wrapper_is_type_checked_without_fabricating_one() {
        let _: fn(
            &AuthenticatedProductionBindings,
            &mut LogicalStorageCounterV1,
        ) -> Result<(), LogicalStorageErrorV1> =
            AuthenticatedProductionBindings::charge_typed_descriptor_retained_heap_v1;
    }
}
