//! Optional debug conversion across the canonical receiver-local insertion.

use super::*;
use fe2o3_kernel_ir::ProductionSemanticDebugProducerGapV1;

type OptionalDebugSourcesV2 = (
    Box<[RetainedDebugSourceScopeV2]>,
    Box<[RetainedDebugSourceVariableV2]>,
    Option<ProductionSemanticDebugProducerGapV1>,
);

pub(super) fn convert_receiver_debug_sources_v2(
    function: SemanticFunctionIdV1,
    function_identity: SemanticFunctionIdentityV1,
    raw_sources: &RetainedRawBodySourceProducerV1,
    raw_to_semantic_locals: &[SemanticLocalIdV1],
    locals: &[RetainedSemanticLocalProducerV1],
    inserted: Option<ReceiverLocalV1>,
    (debug_counts, limits): (&mut RawMirPreflightCountsV1, SemanticMirLimitsV1),
) -> OptionalDebugSourcesV2 {
    if let Some(gap) = raw_sources.debug_capture_gap {
        return (Box::default(), Box::default(), Some(gap));
    }
    let converted = convert_debug_sources_v2(
        function,
        function_identity,
        &raw_sources.debug_scopes,
        &raw_sources.debug_variables,
        raw_to_semantic_locals,
        locals,
    )
    .and_then(|(scopes, mut variables)| {
        if let Some(inserted) = inserted {
            debug_counts.charge(
                SemanticMirResourceV1::ValidationWork,
                variables.len(),
                limits,
            )?;
            // Logical identities bind raw locals; only emitted numeric IDs move.
            for variable in &mut variables {
                if let RetainedDebugSourceVariableClassV2::Local(local) = &mut variable.class {
                    *local = inserted
                        .remap(*local)
                        .ok_or(ProductionSemanticPreflightErrorV1::IdentityTableMismatch)?;
                }
            }
        }
        Ok((scopes, variables))
    });
    match converted {
        Ok((scopes, variables)) => (scopes, variables, None),
        Err(_) => (
            Box::default(),
            Box::default(),
            Some(ProductionSemanticDebugProducerGapV1::ResourceLimit),
        ),
    }
}

#[cfg(test)]
mod tests;
