use super::*;
use crate::production_analysis::pliron_resource_envelope::{
    ProductionAnalysisInputCensusV1, ProductionAnalysisResourceLimitV1,
    ProductionAnalysisResourcePhaseV1 as Phase, ProductionAnalysisResourceUpperBoundV1,
};
use std::mem::size_of;

#[cfg(test)]
#[path = "resources_tests.rs"]
mod tests;

// Conservative byte-sized admission units, not measured allocations. Reserve
// all bounded maps and block DNFs, including pending extensions and Vec slack.
const VIEWS: usize = MAX_CONDITIONAL_PREFIX_INPUTS_V1 + 1;
const NODES: usize = MAX_CONDITIONAL_PREFIX_BLOCKS_V1
    + MAX_CONDITIONAL_PREFIX_OPERATIONS_V1
    + MAX_CONDITIONAL_PREFIX_ARGUMENTS_V1
    + VIEWS
    + 1;
const TERM_BYTES: usize = size_of::<Vec<ConditionalPrefixGuardAtomV1>>()
    + (MAX_CONDITIONAL_PREFIX_GUARD_ATOMS_V1 + 1) * size_of::<ConditionalPrefixGuardAtomV1>();
const RECORD_BYTES: usize = size_of::<
    Option<Result<ConditionalPrefixCoverageV1, ConditionalPrefixDerivationErrorV1>>,
>() + 4 * MAX_CONDITIONAL_PREFIX_DNF_TERMS_V1 * TERM_BYTES
    + 4 * (VIEWS + 1)
        * (size_of::<ConditionalPrefixConditionV1>()
            + size_of::<ConditionalPrefixHostBindingObligationV1>());
const TEMPORARY_BYTES: usize = 1024 * NODES
    + 4 * (MAX_CONDITIONAL_PREFIX_BLOCKS_V1 + 2) * MAX_CONDITIONAL_PREFIX_DNF_TERMS_V1 * TERM_BYTES;

// Bounded table comparisons, local-schema walks, and DNF copies/sorts. This is
// independent of runtime lengths and invocation counts, including failed traces.
const ADAPTER_WORK: usize = 128
    * (MAX_CONDITIONAL_PREFIX_WORK_V1
        + NODES * NODES
        + MAX_CONDITIONAL_PREFIX_DNF_TERMS_V1
            * MAX_CONDITIONAL_PREFIX_DNF_TERMS_V1
            * (MAX_CONDITIONAL_PREFIX_GUARD_ATOMS_V1 + 1)
            * (MAX_CONDITIONAL_PREFIX_GUARD_ATOMS_V1 + 1))
    + TEMPORARY_BYTES;

pub(in crate::production_analysis::pliron_hierarchical_ownership) fn resource_upper_bound_v1(
    census: ProductionAnalysisInputCensusV1,
) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
    let overflow = || ProductionAnalysisResourceLimitV1 {
        phase: Phase::HierarchicalOwnership,
        resource: "conditional ownership resource upper bound",
    };
    let retained = RECORD_BYTES
        .checked_add(census.canonical_bytes)
        .ok_or_else(overflow)?;
    let work = ADAPTER_WORK.checked_add(retained).ok_or_else(overflow)?;
    ProductionAnalysisResourceUpperBoundV1::checked_phase(
        Phase::HierarchicalOwnership,
        work,
        retained,
        TEMPORARY_BYTES,
    )
}
