//! Resource census for native switch callbacks, not structural admission.
//!
//! Generic def-use/dominance verification, diagnostic backtraces and locations
//! belong to the whole-verifier bound. This module neither validates keys nor
//! resolves value types, and cannot authorize a switch or a function.

use dialect_gpu::switch_v3::{SwitchOpV3, switch_key_validation_resources_v3};
use pliron::{
    context::Context,
    op::Op,
    value::{DefiningEntity, Value},
};

use super::pliron_control_edges_v1::ControlViewV1;
use super::pliron_resource_envelope::{
    ProductionAnalysisResourceLimitV1, ProductionAnalysisResourceLimitsV1,
    ProductionAnalysisResourcePhaseV1, ProductionAnalysisResourceUpperBoundV1,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct SwitchVerificationCensusV1 {
    pub(crate) traversal_work: usize,
    pub(crate) callback_work: usize,
    pub(crate) callback_scratch: usize,
}

fn failure(resource: &'static str) -> ProductionAnalysisResourceLimitV1 {
    ProductionAnalysisResourceLimitV1 {
        phase: ProductionAnalysisResourcePhaseV1::StructuralIdentity,
        resource,
    }
}

fn require(
    limits: ProductionAnalysisResourceLimitsV1,
    work: usize,
    scratch: usize,
) -> Result<(), ProductionAnalysisResourceLimitV1> {
    let phase = ProductionAnalysisResourcePhaseV1::StructuralIdentity;
    let bound = ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, work, 0, scratch)?;
    limits.require(phase, bound).map(|_| ())
}

fn roster_length(context: &Context, value: Value) -> usize {
    match value.defining_entity() {
        DefiningEntity::Op(operation) => operation.deref(context).get_num_results(),
        DefiningEntity::Block(block) => block.deref(context).get_num_arguments(),
    }
}

/// Reads only constant-size headers until the complete E/P traversal is paid.
/// Foreign definition headers affect the bound too; graph ownership is checked
/// separately, before invoking any verifier that traverses their use lists.
pub(crate) fn census_switch_verification_v1(
    context: &Context,
    switch: SwitchOpV3,
    limits: ProductionAnalysisResourceLimitsV1,
) -> Result<SwitchVerificationCensusV1, ProductionAnalysisResourceLimitV1> {
    const HEADER_WORK: usize = 128;
    const FRAME_CELLS: usize = 64;
    const MAX_LITERAL_BYTES: usize = 128;
    require(limits, HEADER_WORK, FRAME_CELLS)?;
    let control = ControlViewV1::observe(context, switch.get_operation())
        .map_err(|_| failure("native-switch verification frame"))?;
    let cases = switch
        .cases(context)
        .ok_or_else(|| failure("native-switch verification frame"))?;
    let kind = switch
        .kind(context)
        .ok_or_else(|| failure("native-switch verification frame"))?;
    let raw = switch.get_operation().deref(context);
    let edges = control.successor_count();
    let payloads = raw.get_num_operands() - 1;
    let traversal_work = edges
        .checked_mul(16)
        .and_then(|work| {
            payloads
                .checked_mul(8)
                .and_then(|payloads| work.checked_add(payloads))
        })
        .and_then(|work| work.checked_add(HEADER_WORK))
        .ok_or_else(|| failure("native-switch census work upper bound"))?;
    require(limits, traversal_work, FRAME_CELLS)?;
    let keys = switch_key_validation_resources_v3(kind, cases.bits().len())
        .map_err(|_| failure("native-switch key verification upper bound"))?;
    let mut type_work = roster_length(context, raw.get_operand(0));
    for ordinal in 0..edges {
        let edge = control
            .edge(ordinal)
            .map_err(|_| failure("native-switch verification edge"))?;
        for argument in 0..edge.argument_count() {
            let (incoming, _) = edge
                .argument_at(argument)
                .map_err(|_| failure("native-switch verification argument"))?;
            // Both callbacks search the incoming definition roster, then the
            // destination's known argument prefix, once per payload position.
            type_work = roster_length(context, incoming)
                .checked_add(argument + 1)
                .and_then(|work| work.checked_mul(2))
                .and_then(|work| type_work.checked_add(work))
                .ok_or_else(|| failure("native-switch type verification upper bound"))?;
        }
    }
    // Two 4E offset walks, two 16E edge bodies, two 8P payload bodies.
    // The fixed allowance covers header/interface checks and one bounded
    // literal diagnostic; key validation is called only by the custom verifier.
    let callback_work = edges
        .checked_mul(40)
        .and_then(|work| {
            payloads
                .checked_mul(16)
                .and_then(|payloads| work.checked_add(payloads))
        })
        .and_then(|work| work.checked_add(HEADER_WORK))
        .and_then(|work| work.checked_add(keys.work_upper_bound()))
        .and_then(|work| work.checked_add(type_work))
        .ok_or_else(|| failure("native-switch callback work upper bound"))?;
    let literal_cells = MAX_LITERAL_BYTES.div_ceil(std::mem::size_of::<usize>())
        + 2 * std::mem::size_of::<String>().div_ceil(std::mem::size_of::<usize>());
    let callback_scratch = FRAME_CELLS
        .checked_add(keys.scratch_storage_upper_bound().max(literal_cells))
        .ok_or_else(|| failure("native-switch callback storage upper bound"))?;
    require(
        limits,
        traversal_work
            .checked_add(callback_work)
            .ok_or_else(|| failure("native-switch total work upper bound"))?,
        callback_scratch,
    )?;
    Ok(SwitchVerificationCensusV1 {
        traversal_work,
        callback_work,
        callback_scratch,
    })
}

#[cfg(test)]
mod tests;
