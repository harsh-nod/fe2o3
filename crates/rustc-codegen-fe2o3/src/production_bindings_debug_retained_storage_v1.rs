//! Complete named debug-source branch of authenticated production bindings.
//! Other bindings fields and the enclosing owner header are intentionally not
//! included. This is neither a whole-bindings report nor an admission path.

use super::AuthenticatedProductionBindings;
use crate::rustc_semantic_plan_v1::{
    RetainedDebugSourceScopeV2, RetainedDebugSourceVariableClassV2, RetainedDebugSourceVariableV2,
};
use fe2o3_kernel_ir::{
    DebugSourceMapFileV1, LogicalStorageCounterV1, LogicalStorageErrorV1,
    ProductionSemanticDebugProducerGapV1,
};

fn fixed<T: Copy>(_: &T) {}

impl AuthenticatedProductionBindings {
    /// Charge only the three owned debug arrays and their nested heap.
    ///
    /// The enclosing caller accounts this bindings header once, including all
    /// three Box handles and the optional gap. This method charges a branch
    /// visit, each Box payload, each row visit, and each live String capacity.
    /// It uses the caller's original bounded counter; there is no hidden walk
    /// or fresh unlimited counter. First refusal stops immediately. Prefix
    /// charges remain: discard the enclosing observation on any error.
    ///
    /// Named excluded fields below remain obligations of a future complete
    /// bindings join. No successful subtotal is a full-owner or 128 MiB claim.
    pub(super) fn charge_debug_source_retained_heap_v1(
        &self,
        counter: &mut LogicalStorageCounterV1,
    ) -> Result<(), LogicalStorageErrorV1> {
        let Self {
            context_entries: _,
            rustc_identity_inventory: _,
            rustc_preflight_plan: _,
            rustc_target: _,
            reference_effect_bindings: _,
            debug_source_files,
            debug_source_scopes,
            debug_source_variables,
            debug_capture_gap,
            typed_descriptor_roots: _,
            transaction: _,
        } = self;
        charge_retained_debug_sources_v1(
            debug_source_files,
            debug_source_scopes,
            debug_source_variables,
            debug_capture_gap,
            counter,
        )
    }
}

/// Same branch walk over borrowed actual owners, also used by original producer
/// conversion controls. Box references deliberately prevent accidentally using
/// Vec length as backing capacity after a representation change. This performs
/// no validation or authentication and constructs no compiler-stage owner.
pub(crate) fn charge_retained_debug_sources_v1(
    files: &Box<[DebugSourceMapFileV1]>,
    scopes: &Box<[RetainedDebugSourceScopeV2]>,
    variables: &Box<[RetainedDebugSourceVariableV2]>,
    gap: &Option<ProductionSemanticDebugProducerGapV1>,
    counter: &mut LogicalStorageCounterV1,
) -> Result<(), LogicalStorageErrorV1> {
    counter.charge(0, 1)?;
    fixed(gap);
    match gap {
        None
        | Some(
            ProductionSemanticDebugProducerGapV1::MultipleKirFunctionBodies
            | ProductionSemanticDebugProducerGapV1::NoStatementCorrespondence
            | ProductionSemanticDebugProducerGapV1::SourceMapUnavailable
            | ProductionSemanticDebugProducerGapV1::ResourceLimit
            | ProductionSemanticDebugProducerGapV1::CanonicalKirV7ProjectionUnavailable
            | ProductionSemanticDebugProducerGapV1::SourceObservationUnrepresentable
            | ProductionSemanticDebugProducerGapV1::SemanticMapConstructionUnavailable
            | ProductionSemanticDebugProducerGapV1::SemanticMapEncodingUnavailable
            | ProductionSemanticDebugProducerGapV1::FragmentConstructionUnavailable
            | ProductionSemanticDebugProducerGapV1::CarrierConstructionUnavailable
            | ProductionSemanticDebugProducerGapV1::ReceiptExtensionConstructionUnavailable
            | ProductionSemanticDebugProducerGapV1::CorrespondenceValidationUnavailable
            | ProductionSemanticDebugProducerGapV1::CanonicalKirModuleMismatch
            | ProductionSemanticDebugProducerGapV1::LegacyBareAssociationNoAttachment,
        ) => {}
    }
    counter.array::<DebugSourceMapFileV1>(files.len())?;
    counter.array::<RetainedDebugSourceScopeV2>(scopes.len())?;
    counter.array::<RetainedDebugSourceVariableV2>(variables.len())?;
    for file in files.iter() {
        counter.charge(0, 1)?;
        file.visit_retained_heap_storage_v1(|count, width| {
            let bytes = count
                .checked_mul(width)
                .ok_or(LogicalStorageErrorV1::Arithmetic)?;
            counter.charge(bytes, 1)
        })?;
    }
    for scope in scopes.iter() {
        counter.charge(0, 1)?;
        let RetainedDebugSourceScopeV2 {
            identity,
            function,
            parent_identity,
            depth,
            source,
        } = scope;
        fixed(scope);
        fixed(identity);
        fixed(function);
        fixed(parent_identity);
        fixed(depth);
        fixed(source);
    }
    for variable in variables.iter() {
        counter.charge(0, 1)?;
        let RetainedDebugSourceVariableV2 {
            identity,
            function,
            name,
            scope_identity,
            class,
            entry_value_preserved,
        } = variable;
        fixed(identity);
        fixed(function);
        fixed(scope_identity);
        fixed(class);
        fixed(entry_value_preserved);
        match class {
            RetainedDebugSourceVariableClassV2::Local(local) => fixed(local),
            RetainedDebugSourceVariableClassV2::Unrepresented => {}
        }
        let _: &Option<String> = name;
        if let Some(name) = name {
            counter.string(name)?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "production_bindings_debug_retained_storage_v1_tests.rs"]
mod tests;
