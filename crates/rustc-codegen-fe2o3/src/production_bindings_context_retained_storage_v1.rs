//! Named context-entry heap branch only; not a full bindings observation.
use super::AuthenticatedProductionBindings;
use fe2o3_kernel_ir::{LogicalStorageCounterV1, LogicalStorageErrorV1};

impl AuthenticatedProductionBindings {
    /// The enclosing bindings header contains the context owner inline.
    /// Delegate to its complete actual-capacity/Box walk on the SAME counter.
    /// First refusal propagates; partial counters must be discarded. Other
    /// named fields remain independent obligations and are not zero-valued.
    pub(super) fn charge_context_retained_heap_v1(
        &self,
        counter: &mut LogicalStorageCounterV1,
    ) -> Result<(), LogicalStorageErrorV1> {
        let Self {
            context_entries,
            rustc_identity_inventory: _,
            rustc_preflight_plan: _,
            rustc_target: _,
            reference_effect_bindings: _,
            debug_source_files: _,
            debug_source_scopes: _,
            debug_source_variables: _,
            debug_capture_gap: _,
            typed_descriptor_roots: _,
            transaction: _,
        } = self;
        context_entries.charge_retained_heap_storage_v1(counter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authenticated_wrapper_signature_is_checked_without_fabricating_an_owner() {
        let _: fn(
            &AuthenticatedProductionBindings,
            &mut LogicalStorageCounterV1,
        ) -> Result<(), LogicalStorageErrorV1> =
            AuthenticatedProductionBindings::charge_context_retained_heap_v1;
    }
}
