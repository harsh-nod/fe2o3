//! Generic concurrent-effect verification for ranked Pliron memory.
//!
//! Sparse SSA propagation supplies index formulas. A conservative symbolic
//! fast path proves equal full-rank affine maps injective for any launch size.
//! Remaining cases are evaluated over a bounded static launch domain and
//! indexed by logical allocation plus element coordinate. Exact fallback is
//! O(invocations * effects * rank), never pairwise in invocation count.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt,
};

use dialect_gpu::{AddressSpaceAttr, FenceOp};
use dialect_kernel::{
    AccessKindAttr, AllocationEffectOp, AtomicOrderingAttr, AtomicScopeAttr,
    CheckedRowStripedIndex2DOp, DYNAMIC_EXTENT, IndexConstantOp, IndexEqualBranchArgsOp,
    IndexEqualBranchOp, IndexLessThanBranchArgsOp, IndexLessThanBranchOp, InvocationIndexOp,
    MAX_RANKED_MEMORY_RANK, MemorySpaceAttr, RankedAccessOp, RankedViewOp,
    is_supported_allocation_effect_contract_v1,
};
use pliron::{
    builtin::{op_interfaces::OneRegionInterface, ops::FuncOp},
    common_traits::Named,
    context::Context,
    op::Op,
    operation::Operation,
    value::{DefiningEntity, Value},
};

use crate::production_analysis::pliron_analysis_manager::PlironAnalysisManagerV1;
use crate::production_analysis::pliron_analysis_witness::{
    MAX_BOUNDS_WITNESS_EVALUATION_STEPS_V1, evaluate_raw_index_at_invocation_v1,
};
use crate::production_analysis::pliron_function_inventory::BoundedPlironFunctionInventoryV1;
use crate::production_analysis::pliron_invocation_trace::{
    PlironExecutionLayoutV1, PlironTraceFailureV1, static_invocation_shape_for_resource_v1,
};
use crate::production_analysis::pliron_presburger_adapter::PlironPresburgerAnalysisV1;
use crate::production_analysis::pliron_provenance_alias::MAX_PLIRON_PROVENANCE_DIAGNOSTIC_BYTES_V1;
#[cfg(test)]
use crate::production_analysis::pliron_ranked_bounds::run_pliron_ranked_bounds_check_with_analyses_v1;
use crate::production_analysis::pliron_sparse_index::SparseAffineIndexV1;
use crate::production_analysis::{
    ProductionAnalysisInputCensusV1, ProductionAnalysisResourceLimitV1,
    ProductionAnalysisResourceLimitsV1, ProductionAnalysisResourcePhaseV1,
    ProductionAnalysisResourceUpperBoundV1,
};
use crate::{
    KernelCheckPassKindV1, KernelCheckStatusV1, MAX_PRESBURGER_WORK_UNITS_V1,
    PresburgerCollisionDecisionV1, PresburgerMachineIntSemanticsV1,
    PresburgerMachineRangeDecisionV1, SparseIndexAnalysisV1, SparseIndexFailureV1,
};

pub const MAX_PLIRON_RACE_INVOCATIONS_V1: u64 = 65_536;
pub const MAX_PLIRON_RACE_EFFECT_INSTANCES_V1: usize = 1_048_576;
pub const MAX_PLIRON_RACE_FINDINGS_V1: usize = 4_096;
const RAW_INDEX_EVALUATION_STACK_OPS_PER_VISIT_V1: usize = 8;
const RAW_INDEX_EVALUATION_QUERY_MAP_LIFECYCLE_WORK_V1: usize = 3;
// The shared witness evaluator reserves hash-table capacity at two logical
// units per field and stack capacity at two units per four-field frame.
const RAW_INDEX_EVALUATION_MAP_STORAGE_PER_OPERATION_V1: usize = 6;
const RAW_INDEX_EVALUATION_STACK_FRAME_STORAGE_V1: usize = 8;

fn race_resource_overflow_v1() -> ProductionAnalysisResourceLimitV1 {
    ProductionAnalysisResourceLimitV1 {
        phase: ProductionAnalysisResourcePhaseV1::RaceFreedom,
        resource: "race-analysis resource upper bound",
    }
}

fn checked_race_mul_v1(lhs: usize, rhs: usize) -> Result<usize, ProductionAnalysisResourceLimitV1> {
    lhs.checked_mul(rhs).ok_or_else(race_resource_overflow_v1)
}

fn checked_race_sum_v1(items: &[usize]) -> Result<usize, ProductionAnalysisResourceLimitV1> {
    items.iter().try_fold(0_usize, |total, item| {
        total
            .checked_add(*item)
            .ok_or_else(race_resource_overflow_v1)
    })
}

const RACE_NAME_CENSUS_SCRATCH_V1: usize = 16;
// "allocation origin " plus the full decimal u64 origin.
const RACE_ALLOCATION_NAME_BYTES_V1: usize = 18 + 20;

#[derive(Clone, Copy, Debug)]
struct RaceNameCensusV1 {
    name_storage: usize,
    scan_work: usize,
    execution_lookup_work: usize,
}

#[derive(Default)]
struct RaceNameScanV1 {
    operands: usize,
    results: usize,
    arguments: usize,
    attributes: usize,
    ranked_accesses: usize,
    allocation_effects: usize,
    max_view_name: usize,
    max_index_name: usize,
}

fn race_name_census_error_v1() -> ProductionAnalysisResourceLimitV1 {
    ProductionAnalysisResourceLimitV1 {
        phase: ProductionAnalysisResourcePhaseV1::RaceFreedom,
        resource: "race name census mismatch",
    }
}

fn race_name_prefix_v1(
    total: &mut usize,
    count: usize,
    limit: usize,
) -> Result<(), ProductionAnalysisResourceLimitV1> {
    *total = total
        .checked_add(count)
        .ok_or_else(race_resource_overflow_v1)?;
    if *total > limit {
        return Err(race_name_census_error_v1());
    }
    Ok(())
}

fn race_name_lookup_work_v1(
    census: ProductionAnalysisInputCensusV1,
) -> Result<usize, ProductionAnalysisResourceLimitV1> {
    checked_race_sum_v1(&[
        census.max_operation_arity.max(census.block_arguments),
        checked_race_mul_v1(census.attributes, 20)?,
    ])
}

fn race_name_scan_work_v1(
    census: ProductionAnalysisInputCensusV1,
) -> Result<usize, ProductionAnalysisResourceLimitV1> {
    checked_race_sum_v1(&[
        32,
        checked_race_mul_v1(census.blocks, 8)?,
        checked_race_mul_v1(census.operations, 12)?,
        checked_race_mul_v1(census.operands, 4)?,
        checked_race_mul_v1(
            checked_race_sum_v1(&[census.operands, census.operations])?,
            checked_race_sum_v1(&[race_name_lookup_work_v1(census)?, 80])?,
        )?,
    ])
}

// Check each defining span before a name/type query performs its index search.
fn require_race_name_value_v1(
    context: &Context,
    function: &FuncOp,
    value: Value,
    census: ProductionAnalysisInputCensusV1,
) -> Result<(), ProductionAnalysisResourceLimitV1> {
    let region = function.get_region(context);
    let (arity, attributes, parent) = match value.defining_entity() {
        DefiningEntity::Op(pointer) => {
            let definition = pointer.deref(context);
            (
                definition.get_num_results(),
                definition.attributes.0.len(),
                definition
                    .get_parent_block()
                    .and_then(|block| block.deref(context).get_parent_region()),
            )
        }
        DefiningEntity::Block(pointer) => {
            let definition = pointer.deref(context);
            (
                definition.get_num_arguments(),
                definition.attributes.0.len(),
                definition.get_parent_region(),
            )
        }
    };
    if parent != Some(region)
        || arity > census.max_operation_arity.max(census.block_arguments)
        || attributes > census.attributes
    {
        return Err(race_name_census_error_v1());
    }
    Ok(())
}

// Immediate, private same-manager inventory use; this is not a cached owner.
fn collect_race_name_census_v1(
    context: &Context,
    function: &FuncOp,
    inventory: &BoundedPlironFunctionInventoryV1,
    census: ProductionAnalysisInputCensusV1,
    limits: ProductionAnalysisResourceLimitsV1,
    mut name_bytes: impl FnMut(Value) -> Option<usize>,
) -> Result<RaceNameCensusV1, ProductionAnalysisResourceLimitV1> {
    let work = race_name_scan_work_v1(census)?;
    limits.require(
        ProductionAnalysisResourcePhaseV1::RaceFreedom,
        ProductionAnalysisResourceUpperBoundV1::checked_phase(
            ProductionAnalysisResourcePhaseV1::RaceFreedom,
            work,
            0,
            RACE_NAME_CENSUS_SCRATCH_V1,
        )?,
    )?;
    if inventory.blocks().len() != census.blocks
        || inventory.operations().len() != census.operations
    {
        return Err(race_name_census_error_v1());
    }
    let epoch = context
        .ir_mutation_attempt_epoch()
        .map_err(|_| race_name_census_error_v1())?;
    let mut scan = RaceNameScanV1::default();
    race_name_prefix_v1(
        &mut scan.attributes,
        function.get_operation().deref(context).attributes.0.len(),
        census.attributes,
    )?;
    for pointer in inventory.blocks() {
        let block = pointer.deref(context);
        if block.get_parent_region() != Some(function.get_region(context)) {
            return Err(race_name_census_error_v1());
        }
        race_name_prefix_v1(
            &mut scan.arguments,
            block.get_num_arguments(),
            census.block_arguments,
        )?;
        race_name_prefix_v1(
            &mut scan.attributes,
            block.attributes.0.len(),
            census.attributes,
        )?;
    }
    for site in inventory.operations() {
        let pointer = site.pointer();
        let raw = pointer.deref(context);
        if raw.get_parent_block() != inventory.blocks().get(site.block()).copied()
            || raw
                .get_num_operands()
                .checked_add(raw.get_num_results())
                .ok_or_else(race_resource_overflow_v1)?
                > census.max_operation_arity
        {
            return Err(race_name_census_error_v1());
        }
        race_name_prefix_v1(&mut scan.operands, raw.get_num_operands(), census.operands)?;
        race_name_prefix_v1(&mut scan.results, raw.get_num_results(), census.results)?;
        race_name_prefix_v1(
            &mut scan.attributes,
            raw.attributes.0.len(),
            census.attributes,
        )?;
        if Operation::is_op::<AllocationEffectOp>(pointer, context) {
            race_name_prefix_v1(&mut scan.allocation_effects, 1, census.allocation_effects)?;
        }
        let Some(access) = Operation::get_op::<RankedAccessOp>(pointer, context) else {
            continue;
        };
        race_name_prefix_v1(&mut scan.ranked_accesses, 1, census.ranked_accesses)?;
        if raw.get_num_operands() == 0 {
            return Err(race_name_census_error_v1());
        }
        for operand in raw.operands() {
            require_race_name_value_v1(context, function, operand, census)?;
        }
        // checked_success performs a type query; its last-operand search was
        // bounded above. Borrow the same index range without allocating indices().
        let end = raw.get_num_operands() - usize::from(access.checked_success(context).is_some());
        if end == 0 {
            return Err(race_name_census_error_v1());
        }
        let view_bytes = name_bytes(raw.get_operand(0)).ok_or_else(race_resource_overflow_v1)?;
        scan.max_view_name = scan.max_view_name.max(view_bytes);
        for index in 1..end {
            let bytes = name_bytes(raw.get_operand(index)).ok_or_else(race_resource_overflow_v1)?;
            scan.max_index_name = scan.max_index_name.max(bytes);
        }
    }
    if scan.operands != census.operands
        || scan.results != census.results
        || scan.arguments != census.block_arguments
        || scan.attributes != census.attributes
        || scan.ranked_accesses != census.ranked_accesses
        || scan.allocation_effects != census.allocation_effects
        || context
            .ir_mutation_attempt_epoch()
            .map_err(|_| race_name_census_error_v1())?
            != epoch
    {
        return Err(race_name_census_error_v1());
    }
    Ok(RaceNameCensusV1 {
        name_storage: scan
            .max_view_name
            .max(scan.max_index_name)
            .max(RACE_ALLOCATION_NAME_BYTES_V1),
        scan_work: work,
        execution_lookup_work: race_name_lookup_work_v1(census)?,
    })
}

fn raw_index_evaluation_resource_upper_bound_v1(
    operations: usize,
    queries: usize,
    rank: usize,
) -> Result<(usize, usize), ProductionAnalysisResourceLimitV1> {
    let visit_cap = MAX_BOUNDS_WITNESS_EVALUATION_STEPS_V1
        .checked_add(1)
        .ok_or_else(race_resource_overflow_v1)?;
    // The evaluator rejects on the first visit beyond this code-enforced cap.
    let visits = queries
        .checked_mul(operations.max(1))
        .unwrap_or(visit_cap)
        .min(visit_cap);
    let admitted_queries = queries.min(visit_cap);
    let work = checked_race_sum_v1(&[
        checked_race_mul_v1(visits, RAW_INDEX_EVALUATION_STACK_OPS_PER_VISIT_V1)?,
        checked_race_mul_v1(
            admitted_queries,
            RAW_INDEX_EVALUATION_QUERY_MAP_LIFECYCLE_WORK_V1,
        )?,
    ])?;
    let stack_frames = operations
        .checked_mul(2)
        .and_then(|frames| frames.checked_add(1))
        .ok_or_else(race_resource_overflow_v1)?;
    let temporary = checked_race_sum_v1(&[
        checked_race_mul_v1(
            operations,
            RAW_INDEX_EVALUATION_MAP_STORAGE_PER_OPERATION_V1,
        )?,
        checked_race_mul_v1(stack_frames, RAW_INDEX_EVALUATION_STACK_FRAME_STORAGE_V1)?,
        rank,
    ])?;
    Ok((work, temporary))
}

/// Bounds symbolic pair reasoning and the exact-address fallback before the
/// race pass collects effects. The authenticated census counts both ranked
/// accesses and whole-allocation effects, including non-global effects that
/// the pass may later discard. Pair work
/// is bounded by the same Presburger ceiling checked by the implementation;
/// exact enumeration is bounded by the authenticated static launch and the
/// effect-instance cap. A successful report is clean and retains no findings.
pub(crate) fn preflight_race_resource_upper_bound_v1(
    context: &Context,
    function: &FuncOp,
    inventory: Option<&BoundedPlironFunctionInventoryV1>,
    census: ProductionAnalysisInputCensusV1,
    sparse: &SparseIndexAnalysisV1,
    layout: Option<PlironExecutionLayoutV1>,
    limits: ProductionAnalysisResourceLimitsV1,
) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
    let inventory = inventory.ok_or_else(race_name_census_error_v1)?;
    let names =
        collect_race_name_census_v1(context, function, inventory, census, limits, |value| {
            value.unique_name_byte_len(context)
        })?;
    let extents = layout
        .as_ref()
        .map(|layout| layout.global_extents.as_slice())
        .unwrap_or_else(|| sparse.launch_extents());
    race_resource_upper_bound_for_shape_v1(
        census,
        names,
        static_invocation_shape_for_resource_v1(sparse, layout),
        presburger_invocation_shape_for_resource_v1(extents),
        limits,
    )
}

fn presburger_invocation_shape_for_resource_v1(extents: &[u64]) -> Option<(usize, usize)> {
    let invocations = extents.iter().try_fold(1_u128, |count, extent| {
        count.checked_mul(u128::from(*extent))
    })?;
    if invocations <= u128::from(MAX_PLIRON_RACE_INVOCATIONS_V1) {
        return None;
    }
    // With no relevant pair the relation loop allocates nothing. With at
    // least one pair, this is the minimum admission charge in disjointness_v1.
    let minimum_work = invocations
        .checked_mul((extents.len() as u128).checked_add(1)?)?
        .checked_mul(2)?;
    if minimum_work > MAX_PRESBURGER_WORK_UNITS_V1 as u128 {
        return None;
    }
    Some((usize::try_from(invocations).ok()?, extents.len()))
}

fn presburger_relation_resource_upper_bound_v1(
    shape: Option<(usize, usize)>,
    pairs: usize,
) -> Result<(usize, usize), ProductionAnalysisResourceLimitV1> {
    let Some((invocations, launch_rank)) = shape else {
        return Ok((0, 0));
    };
    let minimum_pair_work = checked_race_mul_v1(
        checked_race_mul_v1(invocations, checked_race_sum_v1(&[launch_rank, 1])?)?,
        2,
    )?;
    if minimum_pair_work == 0 {
        return Err(race_resource_overflow_v1());
    }
    let admitted_pairs = pairs.min(MAX_PRESBURGER_WORK_UNITS_V1 / minimum_pair_work);
    if admitted_pairs == 0 {
        return Ok((0, 0));
    }
    let rank = launch_rank.max(MAX_RANKED_MEMORY_RANK);
    let rank_squared = checked_race_mul_v1(rank, rank)?;
    // Two overflow walks and two relation walks per pair. Each walk evaluates
    // rank outputs with rank coefficients; reserve cloning, hashing and drop
    // work as well as the traversal itself.
    let per_invocation_work = checked_race_sum_v1(&[
        checked_race_mul_v1(rank_squared, 16)?,
        checked_race_mul_v1(rank, 128)?,
        256,
    ])?;
    let work = checked_race_mul_v1(
        checked_race_mul_v1(invocations, admitted_pairs)?,
        per_invocation_work,
    )?;
    // One pair is live at a time: owner keys plus first/alternate coordinates
    // use the exact-address map's capacity allowance. Also retain two fact
    // vectors, two maps, traversal state and a transient collision witness.
    let temporary = checked_race_sum_v1(&[
        checked_race_mul_v1(
            invocations,
            checked_race_sum_v1(&[checked_race_mul_v1(rank, 9)?, 64])?,
        )?,
        checked_race_mul_v1(rank_squared, 8)?,
        checked_race_mul_v1(rank, 64)?,
        128,
    ])?;
    Ok((work, temporary))
}

fn race_resource_upper_bound_for_shape_v1(
    census: ProductionAnalysisInputCensusV1,
    names: RaceNameCensusV1,
    invocation_shape: Option<(usize, usize)>,
    presburger_shape: Option<(usize, usize)>,
    limits: ProductionAnalysisResourceLimitsV1,
) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
    finish_race_resource_preflight_v1(
        calculate_race_resource_upper_bound_for_shape_v1(
            census,
            names,
            invocation_shape,
            presburger_shape,
        ),
        limits,
        trace_race_resource_preflight_v1,
    )
}

#[derive(Clone, Copy)]
struct RaceResourcePreflightNumbersV1 {
    census: ProductionAnalysisInputCensusV1,
    invocation_shape: Option<(usize, usize)>,
    presburger_shape: Option<(usize, usize)>,
    effects: usize,
    effect_pairs: usize,
    pairs: usize,
    rank: usize,
    potential_effect_instances: usize,
    charged_effect_instances: usize,
    retained_effect_instances: usize,
    retained_finding_count: usize,
    name_storage: usize,
    per_finding_storage: usize,
    work: usize,
    raw_evaluation_work: usize,
    presburger_work: usize,
    symbolic_work: usize,
    effect_state: usize,
    address_state: usize,
    attempted_finding: usize,
    conflict_class_storage: usize,
    raw_evaluation_temporary: usize,
    presburger_temporary: usize,
    temporary: usize,
    bound: ProductionAnalysisResourceUpperBoundV1,
}

fn finish_race_resource_preflight_v1(
    numbers: Result<RaceResourcePreflightNumbersV1, ProductionAnalysisResourceLimitV1>,
    limits: ProductionAnalysisResourceLimitsV1,
    observe: impl FnOnce(&RaceResourcePreflightNumbersV1, ProductionAnalysisResourceLimitsV1),
) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
    let numbers = numbers?;
    observe(&numbers, limits);
    limits.require(
        ProductionAnalysisResourcePhaseV1::RaceFreedom,
        numbers.bound,
    )
}

fn trace_race_resource_preflight_v1(
    numbers: &RaceResourcePreflightNumbersV1,
    limits: ProductionAnalysisResourceLimitsV1,
) {
    if std::env::var_os("FE2O3_TRACE_RANKED_CUSTODY_V1").is_none() {
        return;
    }
    write_race_resource_preflight_v1(true, &mut std::io::stderr().lock(), numbers, limits);
}

fn write_race_resource_preflight_v1(
    enabled: bool,
    writer: &mut impl std::io::Write,
    numbers: &RaceResourcePreflightNumbersV1,
    limits: ProductionAnalysisResourceLimitsV1,
) {
    if !enabled {
        return;
    }
    let (static_invocations, static_rank) = numbers.invocation_shape.unwrap_or((0, 0));
    let (presburger_invocations, presburger_rank) = numbers.presburger_shape.unwrap_or((0, 0));
    // Only this fixed roster of numeric fields is emitted; I/O cannot admit the phase.
    let _ = writeln!(
        writer,
        concat!(
            "RACE_RESOURCE_PREFLIGHT_V1 stage=local_require units=logical",
            " blocks={} operations={} successors={} ranked_accesses={}",
            " allocation_effects={} identifier_bytes={} canonical_bytes={}",
            " static_present={} static_invocations={} static_rank={}",
            " presburger_present={} presburger_invocations={} presburger_rank={}",
            " effects={} effect_pairs={} pairs={} rank={} potential_instances={}",
            " charged_instances={} retained_instances={} finding_count={}",
            " name_storage={} per_finding_storage={} core_work={} raw_work={}",
            " presburger_work={} symbolic_work={} effect_state={} address_state={}",
            " attempted_finding={} conflict_class_storage={} raw_temporary={}",
            " presburger_temporary={} temporary={} work={} retained={} peak={}",
            " remaining_work={} remaining_peak={} word_bits={}"
        ),
        numbers.census.blocks,
        numbers.census.operations,
        numbers.census.successors,
        numbers.census.ranked_accesses,
        numbers.census.allocation_effects,
        numbers.census.identifier_bytes,
        numbers.census.canonical_bytes,
        usize::from(numbers.invocation_shape.is_some()),
        static_invocations,
        static_rank,
        usize::from(numbers.presburger_shape.is_some()),
        presburger_invocations,
        presburger_rank,
        numbers.effects,
        numbers.effect_pairs,
        numbers.pairs,
        numbers.rank,
        numbers.potential_effect_instances,
        numbers.charged_effect_instances,
        numbers.retained_effect_instances,
        numbers.retained_finding_count,
        numbers.name_storage,
        numbers.per_finding_storage,
        numbers.work,
        numbers.raw_evaluation_work,
        numbers.presburger_work,
        numbers.symbolic_work,
        numbers.effect_state,
        numbers.address_state,
        numbers.attempted_finding,
        numbers.conflict_class_storage,
        numbers.raw_evaluation_temporary,
        numbers.presburger_temporary,
        numbers.temporary,
        numbers.bound.work_upper_bound(),
        numbers.bound.retained_storage_upper_bound(),
        numbers.bound.peak_storage_upper_bound(),
        limits.max_work(),
        limits.max_peak_storage(),
        usize::BITS,
    );
}

fn race_cfg_resource_bound_v1(
    census: ProductionAnalysisInputCensusV1,
) -> Result<(usize, usize), ProductionAnalysisResourceLimitV1> {
    if census.blocks == 0 {
        return Ok((0, 0));
    }
    const R: usize = MAX_RANKED_MEMORY_RANK;
    // A guard introduces one finite bound. Each coordinate can only weaken
    // after initialization; together they change at most once per guard.
    let height = checked_race_sum_v1(&[census.blocks.min(census.successors / 2), 1])?;
    let visits = checked_race_sum_v1(&[checked_race_mul_v1(census.blocks - 1, height)?, 1])?;
    let edges = checked_race_mul_v1(height, census.successors)?
        .min(checked_race_mul_v1(visits, census.max_successor_arity)?);
    let queued = if census.max_successor_arity == 0 {
        1
    } else {
        visits - (visits - 1).div_ceil(census.max_successor_arity)
    };
    // The queue term covers capacity growth and overlapping old/new backing.
    // Map operations use the existing fixed-cost logical-unit convention;
    // this is not a bound on hash probes, allocator internals or RSS.
    Ok((
        checked_race_sum_v1(&[
            checked_race_mul_v1(8 * R + 32, census.blocks)?,
            checked_race_mul_v1(2 * R + 32, visits)?,
            checked_race_mul_v1(8 * R + 16, edges)?,
            32,
        ])?,
        checked_race_sum_v1(&[
            checked_race_mul_v1(6 * R + 12, census.blocks)?,
            checked_race_mul_v1(4, queued)?,
            8 * R + 32,
        ])?,
    ))
}

fn calculate_race_resource_upper_bound_for_shape_v1(
    census: ProductionAnalysisInputCensusV1,
    names: RaceNameCensusV1,
    invocation_shape: Option<(usize, usize)>,
    presburger_shape: Option<(usize, usize)>,
) -> Result<RaceResourcePreflightNumbersV1, ProductionAnalysisResourceLimitV1> {
    let (cfg_work, cfg_storage) = race_cfg_resource_bound_v1(census)?;
    let effects = checked_race_sum_v1(&[census.ranked_accesses, census.allocation_effects])?;
    let effect_pairs = effects
        .checked_add(1)
        .and_then(|next| effects.checked_mul(next))
        .map(|ordered| ordered / 2)
        .ok_or_else(race_resource_overflow_v1)?;
    let pairs = effect_pairs.min(MAX_PRESBURGER_WORK_UNITS_V1);
    let (invocations, launch_rank) = invocation_shape.unwrap_or((0, 3));
    // Every early return before exact enumeration retains at most one finding.
    // The symbolic proofs still run, including separately bounded relation maps.
    let exact_fallback_reachable = invocations > 1;
    let potential_effect_instances = if exact_fallback_reachable {
        checked_race_mul_v1(invocations, effects)?
    } else {
        0
    };
    let charged_effect_instances = potential_effect_instances.min(
        MAX_PLIRON_RACE_EFFECT_INSTANCES_V1
            .checked_add(1)
            .ok_or_else(race_resource_overflow_v1)?,
    );
    let retained_effect_instances =
        potential_effect_instances.min(MAX_PLIRON_RACE_EFFECT_INSTANCES_V1);
    let rank = launch_rank.max(MAX_RANKED_MEMORY_RANK);
    let (raw_evaluation_work, raw_evaluation_temporary) = if exact_fallback_reachable {
        let raw_evaluation_queries = checked_race_mul_v1(
            checked_race_sum_v1(&[effects, charged_effect_instances])?,
            MAX_RANKED_MEMORY_RANK,
        )?;
        raw_index_evaluation_resource_upper_bound_v1(
            census.operations,
            raw_evaluation_queries,
            rank,
        )?
    } else {
        (0, 0)
    };
    let (presburger_work, presburger_temporary) =
        presburger_relation_resource_upper_bound_v1(presburger_shape, pairs)?;
    let symbolic_work = if invocations == 0 {
        MAX_PRESBURGER_WORK_UNITS_V1
    } else {
        checked_race_mul_v1(pairs, MAX_RANKED_MEMORY_RANK * MAX_RANKED_MEMORY_RANK + 16)?
    };
    let work = checked_race_sum_v1(&[
        names.scan_work,
        checked_race_mul_v1(
            effects
                .checked_add(1)
                .ok_or_else(race_resource_overflow_v1)?,
            checked_race_sum_v1(&[
                names.execution_lookup_work,
                checked_race_mul_v1(names.name_storage, 4)?,
                64,
            ])?,
        )?,
        checked_race_mul_v1(census.operations, 32)?,
        cfg_work,
        symbolic_work,
        // Both checked-layout recognizers together perform at most twelve
        // owner-bound root queries (four fixed operations each) and three
        // fixed-width root comparisons per effect pair. No new owner/storage.
        checked_race_mul_v1(pairs, 51)?,
        checked_race_mul_v1(
            charged_effect_instances,
            rank.checked_mul(4)
                .and_then(|n| n.checked_add(16))
                .ok_or_else(race_resource_overflow_v1)?,
        )?,
        raw_evaluation_work,
        presburger_work,
    ])?;
    let name_storage = names.name_storage;
    let effect_state = checked_race_mul_v1(
        effects,
        MAX_RANKED_MEMORY_RANK
            .checked_add(16)
            .and_then(|items| items.checked_add(name_storage))
            .ok_or_else(race_resource_overflow_v1)?,
    )?;
    let address_state = checked_race_mul_v1(
        retained_effect_instances,
        rank.checked_mul(9)
            .and_then(|n| n.checked_add(64))
            .ok_or_else(race_resource_overflow_v1)?,
    )?;
    let per_finding_storage = checked_race_sum_v1(&[
        checked_race_mul_v1(rank, 3)?,
        name_storage,
        MAX_PLIRON_PROVENANCE_DIAGNOSTIC_BYTES_V1,
        160,
    ])?;
    // Exact fallback deduplicates ordered static effect pairs, not invocation
    // coordinates. Reversing the witness order can produce a distinct class.
    let (retained_finding_count, conflict_class_storage) = if exact_fallback_reachable {
        let conflict_classes = checked_race_mul_v1(effects, effects)?;
        (
            conflict_classes.clamp(1, MAX_PLIRON_RACE_FINDINGS_V1),
            checked_race_mul_v1(
                MAX_PLIRON_RACE_FINDINGS_V1
                    .checked_add(1)
                    .ok_or_else(race_resource_overflow_v1)?,
                16,
            )?,
        )
    } else {
        (1, 0)
    };
    let retained_findings = checked_race_mul_v1(retained_finding_count, per_finding_storage)?;
    let attempted_finding = per_finding_storage;
    let temporary = checked_race_sum_v1(&[
        checked_race_mul_v1(name_storage, 2)?,
        RACE_NAME_CENSUS_SCRATCH_V1,
        effect_state,
        address_state,
        attempted_finding,
        conflict_class_storage,
        cfg_storage,
        checked_race_mul_v1(effects, 8)?,
        raw_evaluation_temporary,
        presburger_temporary,
    ])?;
    let bound = ProductionAnalysisResourceUpperBoundV1::checked_phase(
        ProductionAnalysisResourcePhaseV1::RaceFreedom,
        checked_race_sum_v1(&[
            work,
            checked_race_mul_v1(
                retained_finding_count
                    .checked_add(1)
                    .ok_or_else(race_resource_overflow_v1)?,
                per_finding_storage,
            )?,
        ])?,
        retained_findings,
        temporary,
    )?;
    Ok(RaceResourcePreflightNumbersV1 {
        census,
        invocation_shape,
        presburger_shape,
        effects,
        effect_pairs,
        pairs,
        rank,
        potential_effect_instances,
        charged_effect_instances,
        retained_effect_instances,
        retained_finding_count,
        name_storage,
        per_finding_storage,
        work,
        raw_evaluation_work,
        presburger_work,
        symbolic_work,
        effect_state,
        address_state,
        attempted_finding,
        conflict_class_storage,
        raw_evaluation_temporary,
        presburger_temporary,
        temporary,
        bound,
    })
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RankedRaceLocationV1 {
    block: usize,
    operation: usize,
}

impl RankedRaceLocationV1 {
    pub const fn block(self) -> usize {
        self.block
    }

    pub const fn operation(self) -> usize {
        self.operation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RankedRaceWitnessV1 {
    location: RankedRaceLocationV1,
    access: AccessKindAttr,
    invocation: Vec<u64>,
    grid: u64,
    workgroup: Option<u64>,
    subgroup: Option<u64>,
    lane: Option<u64>,
    atomic_scope: Option<AtomicScopeAttr>,
}

impl RankedRaceWitnessV1 {
    pub const fn location(&self) -> RankedRaceLocationV1 {
        self.location
    }

    pub const fn access(&self) -> AccessKindAttr {
        self.access
    }

    pub fn invocation(&self) -> &[u64] {
        &self.invocation
    }

    pub const fn grid(&self) -> u64 {
        self.grid
    }

    pub const fn workgroup(&self) -> Option<u64> {
        self.workgroup
    }

    pub const fn subgroup(&self) -> Option<u64> {
        self.subgroup
    }

    pub const fn lane(&self) -> Option<u64> {
        self.lane
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RankedRaceFindingV1 {
    BoundsPrerequisiteRejected,
    SparseIndexAnalysisFailed {
        detail: String,
    },
    DynamicLaunchExtent {
        dimension: usize,
    },
    LaunchDomainTooLarge {
        invocations: u64,
        limit: u64,
    },
    UnresolvedIndex {
        block: usize,
        operation: usize,
        dimension: usize,
        value: String,
    },
    EffectInstanceLimitExceeded {
        actual: usize,
        limit: usize,
    },
    FindingLimitExceeded {
        actual: usize,
        limit: usize,
    },
    ConflictingEffects {
        view: String,
        indices: Vec<u64>,
        first: RankedRaceWitnessV1,
        second: RankedRaceWitnessV1,
    },
    ExecutionLayoutUnavailable {
        detail: String,
    },
    AllocationContractUnavailable {
        detail: String,
    },
    InsufficientAtomicScope {
        view: String,
        indices: Vec<u64>,
        first: RankedRaceWitnessV1,
        second: RankedRaceWitnessV1,
    },
    HappensBeforeIncomplete {
        view: String,
        detail: String,
    },
}

impl fmt::Display for RankedRaceFindingV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BoundsPrerequisiteRejected => formatter.write_str(
                "error[FE2O3-RACE-000]: ranked bounds prerequisite rejected before race analysis",
            ),
            Self::SparseIndexAnalysisFailed { detail } => write!(
                formatter,
                "error[FE2O3-RACE-003]: sparse index analysis failed before race analysis: {detail}",
            ),
            Self::DynamicLaunchExtent { dimension } => write!(
                formatter,
                "error[FE2O3-RACE-002]: cannot prove race freedom for dynamic launch dimension {dimension}; help: retain a bounded launch contract or supply a symbolic disjointness proof",
            ),
            Self::LaunchDomainTooLarge { invocations, limit } => write!(
                formatter,
                "error[FE2O3-RACE-003]: static launch has {invocations} invocations, exceeding exact race-analysis limit {limit}",
            ),
            Self::UnresolvedIndex {
                block,
                operation,
                dimension,
                value,
            } => write!(
                formatter,
                "error[FE2O3-RACE-002]: cannot prove race freedom at block {block} op {operation}; access dimension {dimension} has unresolved index {value}; the index has no supported checked structured contract with independently validated success, value, extent, and uniform-layout semantics; help: express the address with explicit affine index operations plus finite no-wrap and extent guards",
            ),
            Self::EffectInstanceLimitExceeded { actual, limit } => write!(
                formatter,
                "error[FE2O3-RACE-003]: concurrent effect instance count {actual} exceeds analysis limit {limit}",
            ),
            Self::FindingLimitExceeded { actual, limit } => write!(
                formatter,
                "error[FE2O3-RACE-003]: race finding count {actual} exceeds analysis limit {limit}",
            ),
            Self::ConflictingEffects {
                view,
                indices,
                first,
                second,
            } => write!(
                formatter,
                "error[FE2O3-RACE-001]: potentially conflicting incompatible {:?}/{:?} effects on {view}{indices:?}; first writer/reader: invocation {:?} at block {} op {}; second writer/reader: invocation {:?} at block {} op {}; failed proof: distinct concurrent invocations do not imply disjoint memory coordinates; help: include an invocation-owned coordinate, use a disjoint view, or use a compatible atomic operation",
                first.access,
                second.access,
                first.invocation,
                first.location.block,
                first.location.operation,
                second.invocation,
                second.location.block,
                second.location.operation,
            ),
            Self::ExecutionLayoutUnavailable { detail } => write!(
                formatter,
                "error[FE2O3-RACE-002]: scoped concurrency analysis is incomplete: {detail}",
            ),
            Self::AllocationContractUnavailable { detail } => write!(
                formatter,
                "error[FE2O3-RACE-002]: allocation alias analysis is incomplete: {detail}",
            ),
            Self::InsufficientAtomicScope {
                view,
                indices,
                first,
                second,
            } => write!(
                formatter,
                "error[FE2O3-RACE-004]: overlapping atomic effects on {view}{indices:?} use scopes {:?}/{:?} that do not cover invocations {:?}/{:?}; failed proof: cross-workgroup overlap requires compatible device-scope atomics",
                first.atomic_scope, second.atomic_scope, first.invocation, second.invocation,
            ),
            Self::HappensBeforeIncomplete { view, detail } => write!(
                formatter,
                "error[FE2O3-RACE-002]: happens-before analysis for conflicting ordinary effects on {view} is incomplete: {detail}",
            ),
        }
    }
}

impl RankedRaceFindingV1 {
    pub const fn status(&self) -> KernelCheckStatusV1 {
        match self {
            Self::ConflictingEffects { .. } | Self::InsufficientAtomicScope { .. } => {
                KernelCheckStatusV1::Rejected
            }
            Self::BoundsPrerequisiteRejected
            | Self::SparseIndexAnalysisFailed { .. }
            | Self::DynamicLaunchExtent { .. }
            | Self::LaunchDomainTooLarge { .. }
            | Self::UnresolvedIndex { .. }
            | Self::EffectInstanceLimitExceeded { .. }
            | Self::FindingLimitExceeded { .. }
            | Self::ExecutionLayoutUnavailable { .. }
            | Self::AllocationContractUnavailable { .. }
            | Self::HappensBeforeIncomplete { .. } => KernelCheckStatusV1::Incomplete,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RankedRaceReportV1 {
    findings: Vec<RankedRaceFindingV1>,
}

super::pliron_report_payload_receipt::impl_empty_findings_payload_v1!(RankedRaceReportV1);

impl RankedRaceReportV1 {
    pub const fn pass(&self) -> KernelCheckPassKindV1 {
        KernelCheckPassKindV1::RaceFreedom
    }

    pub fn status(&self) -> KernelCheckStatusV1 {
        self.findings
            .iter()
            .fold(KernelCheckStatusV1::Clean, |status, finding| {
                status.join(finding.status())
            })
    }

    pub fn findings(&self) -> &[RankedRaceFindingV1] {
        &self.findings
    }

    pub fn is_clean(&self) -> bool {
        self.status() == KernelCheckStatusV1::Clean
    }

    pub const fn grants_compiler_refinement_authority(&self) -> bool {
        false
    }

    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RankedRaceCheckErrorV1 {
    report: RankedRaceReportV1,
}

impl RankedRaceCheckErrorV1 {
    pub fn report(&self) -> &RankedRaceReportV1 {
        &self.report
    }
}

impl fmt::Display for RankedRaceCheckErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, finding) in self.report.findings.iter().enumerate() {
            if index != 0 {
                formatter.write_str("\n")?;
            }
            finding.fmt(formatter)?;
        }
        Ok(())
    }
}

impl std::error::Error for RankedRaceCheckErrorV1 {}

include!("pliron_race/execution_v1.rs");
include!("pliron_race/disjointness_v1.rs");
include!("pliron_race/affine_invocations_v1.rs");
include!("pliron_race/resource_tests.rs");
