use super::*;

const UNSUPPORTED_SYNC: &str =
    "execution-capability synchronization has no exact ranked workgroup barrier contract";

fn require_workgroup_semantics(semantics: ExecutionMemorySemanticsV1) -> Result<(), &'static str> {
    if semantics
        != (ExecutionMemorySemanticsV1 {
            scope: ExecutionMemoryScopeV1::Workgroup,
            ordering: ExecutionMemoryOrderingV1::AcquireRelease,
            spaces: ExecutionMemorySpacesV1::Workgroup,
        })
    {
        return Err(UNSUPPORTED_SYNC);
    }
    Ok(())
}

/// Classifies the original callable and call without issuing source or proof authority.
/// The semantic owner, execution source carrier, and phase replay remain mandatory.
#[doc(hidden)]
pub fn project_execution_workgroup_barrier_v1(
    callable: &SemanticCallableDeclV1,
    call: &SemanticDirectCallV1,
) -> Result<Option<ProductionRankedOperationV1>, &'static str> {
    let SemanticCallableDeclV1::CompilerIntrinsic {
        binding,
        operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
        ..
    } = callable
    else {
        return Ok(None);
    };
    let semantics = match contract.operation() {
        SemanticExecutionCapabilityOperationV1::WorkgroupBarrier { semantics, .. } => semantics,
        SemanticExecutionCapabilityOperationV1::SubgroupBarrier { .. }
        | SemanticExecutionCapabilityOperationV1::WorkgroupFence { .. }
        | SemanticExecutionCapabilityOperationV1::SubgroupFence { .. } => {
            return Err(UNSUPPORTED_SYNC);
        }
        _ => return Ok(None),
    };
    require_workgroup_semantics(lower_execution_semantics_v1(semantics))?;
    if binding.identity() != contract.source_identity()
        || binding.abi().c_variadic()
        || call
            .arguments()
            .iter()
            .map(SemanticOperandV1::ty)
            .ne(contract.signature().arguments())
        || binding
            .abi()
            .source_input_types()
            .iter()
            .copied()
            .ne(contract.signature().arguments())
        || binding.abi().source_output_type() != contract.signature().output()
        || call
            .destination()
            .map(|destination| destination.place().ty())
            != Some(contract.signature().output())
    {
        return Err("execution workgroup barrier lost its exact callable or source signature");
    }
    Ok(Some(ProductionRankedOperationV1::Barrier {
        execution_scope: dialect_gpu::HierarchyAttr::Workgroup,
        memory_scope: dialect_gpu::MemoryScopeAttr::Workgroup,
        address_space: dialect_gpu::AddressSpaceAttr::Workgroup,
        order: dialect_gpu::MemoryOrderAttr::AcquireRelease,
    }))
}

pub(super) fn kir_contract(
    capability: &ExecutionCapabilityOpV1,
) -> Result<Option<NormalizedSynchronizationV1>, ProductionMirPlironTranslationErrorV1> {
    let ExecutionCapabilityOperationV1::WorkgroupBarrier { semantics, .. } = capability.operation
    else {
        // The caller's unchanged effect-count guard rejects every unclassified
        // synchronizing capability; unrelated effect-free operations stay unrelated.
        return Ok(None);
    };
    if !capability.is_complete() || require_workgroup_semantics(semantics).is_err() {
        return Err(ProductionMirPlironTranslationErrorV1::SynchronizationMismatch);
    }
    Ok(Some(NormalizedSynchronizationV1 {
        execution_scope: Some(normalize_kir_scope_v1(
            semantics.scope.synchronization_scope(),
        )),
        memory_scope: normalize_kir_scope_v1(semantics.scope.synchronization_scope()),
        ordering: normalize_kir_order_v1(semantics.ordering.memory_ordering())
            .ok_or(ProductionMirPlironTranslationErrorV1::SynchronizationMismatch)?,
        address_space: normalize_kir_address_space_v1(AddressSpace::Workgroup),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    include!("execution_synchronization_v1_tests.rs");
}
