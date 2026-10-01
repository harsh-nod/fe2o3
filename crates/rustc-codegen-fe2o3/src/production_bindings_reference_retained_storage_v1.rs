//! Named reference-effect heap branch only, not whole production bindings.
use super::AuthenticatedProductionBindings;
use fe2o3_kernel_ir::LogicalStorageCounterV1;
use fe2o3_verifier::portable_reference_v1::retained_storage_v1::ReferenceRetainedStorageErrorV1;

impl AuthenticatedProductionBindings {
    /// Exclude the enclosing bindings header, preserve the SAME bounded counter,
    /// and propagate storage/depth refusal. All other fields below remain named
    /// obligations; they are neither counted here nor treated as zero.
    pub(super) fn charge_reference_effect_retained_heap_v1(
        &self,
        counter: &mut LogicalStorageCounterV1,
    ) -> Result<(), ReferenceRetainedStorageErrorV1> {
        let Self {
            context_entries: _,
            rustc_identity_inventory: _,
            rustc_preflight_plan: _,
            rustc_target: _,
            reference_effect_bindings,
            debug_source_files: _,
            debug_source_scopes: _,
            debug_source_variables: _,
            debug_capture_gap: _,
            typed_descriptor_roots: _,
            transaction: _,
        } = self;
        reference_effect_bindings.charge_retained_heap_storage_v1(counter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bindings_delegation_is_type_checked_without_fabricating_authority() {
        let _: fn(
            &AuthenticatedProductionBindings,
            &mut LogicalStorageCounterV1,
        ) -> Result<(), ReferenceRetainedStorageErrorV1> =
            AuthenticatedProductionBindings::charge_reference_effect_retained_heap_v1;
    }
}
