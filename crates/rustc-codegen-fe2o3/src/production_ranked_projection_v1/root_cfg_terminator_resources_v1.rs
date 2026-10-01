//! Shared ordinary terminator projection with a closed original-ledger mode.
//! Unknown switches retain every original successor in first-seen order.
//! This is NOT participation verification, a ranked CFG, or an access certificate.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
use std::mem::size_of;

type Result<T, E = ProductionRankedProjectionErrorV1> = std::result::Result<T, E>;

pub(super) fn reserve_owned(
    facts: &mut impl ProjectedAssertionFactsV1,
    owned: &mut usize,
    bytes: usize,
) -> Result<()> {
    let next = owned
        .checked_add(bytes)
        .ok_or_else(|| ranked_projection_source_v1::resource(Resource::Arithmetic))?;
    facts.reserve_scalar_private_storage_v1(bytes)?;
    *owned = next;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn collect_owned_successors(
    targets: &SemanticSwitchTargetsV1,
    otherwise: usize,
    elide_fallback: bool,
    capacity: usize,
    target: &impl Fn(SemanticBlockIdV1) -> Result<usize>,
    facts: &mut impl ProjectedAssertionFactsV1,
    owned: &mut usize,
) -> Result<Vec<usize>> {
    // Pay the complete finite ordered-dedup upper bound before allocation,
    // iterator construction, target checking, or first comparison. Unlike the
    // legacy HashSet this uses no private hash-table backing.
    let work = capacity
        .checked_mul(capacity)
        .and_then(|n| {
            capacity
                .checked_mul(8)
                .and_then(|extra| n.checked_add(extra))
        })
        .and_then(|n| n.checked_add(32))
        .ok_or_else(|| ranked_projection_source_v1::resource(Resource::Arithmetic))?;
    facts.charge_private_array_work(work)?;
    let bytes = capacity
        .checked_mul(size_of::<usize>())
        .ok_or_else(|| ranked_projection_source_v1::resource(Resource::Arithmetic))?;
    reserve_owned(facts, owned, bytes)?;
    let mut successors = Vec::new();
    successors
        .try_reserve_exact(capacity)
        .map_err(|_| ranked_projection_source_v1::resource(Resource::Allocation))?;
    if successors.capacity() != capacity {
        return Err(ranked_projection_source_v1::resource(Resource::Allocation));
    }
    for row in targets.values() {
        let successor = target(row.edge().target())?;
        if !successors.contains(&successor) {
            successors.push(successor);
        }
    }
    if !elide_fallback && !successors.contains(&otherwise) {
        successors.push(otherwise);
    }
    Ok(successors)
}

// Strict projection only needs an unprojected local. Check the slice length
// before touching any projection; the ordinary None route retains its helper.
// For an empty projection list the old transparent scan is vacuously true,
// and for every nonempty list its final is_empty test would return None.
fn operand_local(operand: &SemanticOperandV1, strict: bool) -> Option<SemanticLocalIdV1> {
    if strict {
        raw_operand_place(operand)
            .filter(|place| place.projections().is_empty())
            .map(SemanticPlaceV1::local)
    } else {
        simple_operand_local(operand)
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn project(
    function: &SemanticFunctionDeclV1,
    block_index: usize,
    callables: &[SemanticCallableDeclV1],
    non_bounds_assert_proved: bool,
    assertion_facts: &mut impl ProjectedAssertionFactsV1,
    switch_predicates: &[Option<GuardPredicateV1>],
    deterministic_switches: &[Option<ProjectedDeterministicSwitchV1>],
    mut owned: Option<&mut usize>,
) -> Result<ProjectedCfgTerminatorV1, ProductionRankedProjectionErrorV1> {
    if let Some(owned) = owned.as_deref_mut() {
        // The initial nominal route has no authenticated predicate/induction
        // substitutions yet. Refuse supplied tables before any nested Clone.
        assertion_facts.charge_private_array_work(32)?;
        if !switch_predicates.is_empty() || !deterministic_switches.is_empty() {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "metered nominal CFG requires original conservative switch projection",
            ));
        }
        reserve_owned(
            assertion_facts,
            owned,
            size_of::<SemanticSourceProvenanceV1>(),
        )?;
    }
    let block = function.blocks().get(block_index).ok_or(
        ProductionRankedProjectionErrorV1::Unsupported("a semantic CFG block outside the function"),
    )?;
    let target = |target: fe2o3_mir_model::semantic_mir_v1::SemanticBlockIdV1| {
        let target = target.index() as usize;
        (target < function.blocks().len()).then_some(target).ok_or(
            ProductionRankedProjectionErrorV1::Unsupported(
                "a semantic CFG edge outside the function",
            ),
        )
    };
    match block.terminator().kind() {
        SemanticTerminatorKindV1::Goto(edge) => {
            Ok(ProjectedCfgTerminatorV1::Branch(target(edge.target())?))
        }
        SemanticTerminatorKindV1::SwitchInt {
            discriminant,
            targets,
        } => {
            let predicate = operand_local(discriminant, owned.is_some()).and_then(|discriminant| {
                switch_predicates
                    .get(discriminant.index() as usize)
                    .and_then(Clone::clone)
            });
            let Some(predicate) = predicate else {
                if let Some(projected) = deterministic_switches
                    .get(block_index)
                    .and_then(Clone::clone)
                {
                    return Ok(ProjectedCfgTerminatorV1::ExactSwitch(projected));
                }
                let successor_capacity = targets.values().len().checked_add(1).ok_or(
                    ProductionRankedProjectionErrorV1::Unsupported(
                        "analysis switch successor count overflow",
                    ),
                )?;
                if successor_capacity > MAX_RANKED_BOUNDS_EDGES {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "analysis switch edge count exceeds the ranked edge limit",
                    ));
                }
                let otherwise = target(targets.otherwise().target())?;
                let elide_fallback = targets.values().len() == 2
                    && targets.values()[0].value() == 0
                    && targets.values()[1].value() == 1
                    && switch_fallback_is_empty_unreachable_v1(function, otherwise);
                let successors = if let Some(owned) = owned.as_deref_mut() {
                    collect_owned_successors(
                        targets,
                        otherwise,
                        elide_fallback,
                        successor_capacity,
                        &target,
                        assertion_facts,
                        owned,
                    )?
                } else {
                    let mut successors = Vec::new();
                    successors
                        .try_reserve_exact(successor_capacity)
                        .map_err(|_| {
                            ProductionRankedProjectionErrorV1::Unsupported(
                                "analysis switch successor storage cannot be reserved",
                            )
                        })?;
                    let mut seen = HashSet::new();
                    seen.try_reserve(successor_capacity).map_err(|_| {
                        ProductionRankedProjectionErrorV1::Unsupported(
                            "analysis switch successor set cannot be reserved",
                        )
                    })?;
                    for successor in targets.values() {
                        let successor = target(successor.edge().target())?;
                        if seen.insert(successor) {
                            successors.push(successor);
                        }
                    }
                    if !elide_fallback && seen.insert(otherwise) {
                        successors.push(otherwise);
                    }
                    successors
                };
                if successors.len() > MAX_RANKED_BOUNDS_BLOCKS {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "analysis switch successor count exceeds the ranked block limit",
                    ));
                }
                return match successors.as_slice() {
                    [] => Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "analysis switch has no successor",
                    )),
                    [successor] => Ok(ProjectedCfgTerminatorV1::Branch(*successor)),
                    [first_block, second_block] => Ok(ProjectedCfgTerminatorV1::AnalysisSplit {
                        first_block: *first_block,
                        second_block: *second_block,
                    }),
                    _ => Ok(ProjectedCfgTerminatorV1::AnalysisMultiSplit { blocks: successors }),
                };
            };
            if targets.values().len() == 1 {
                let explicit = &targets.values()[0];
                let explicit_block = target(explicit.edge().target())?;
                let otherwise_block = target(targets.otherwise().target())?;
                return match explicit.value() {
                    0 => Ok(ProjectedCfgTerminatorV1::Predicate {
                        predicate,
                        true_block: otherwise_block,
                        false_block: explicit_block,
                    }),
                    1 => Ok(ProjectedCfgTerminatorV1::Predicate {
                        predicate,
                        true_block: explicit_block,
                        false_block: otherwise_block,
                    }),
                    _ => Err(ProductionRankedProjectionErrorV1::Incomplete(
                        "a comparison predicate switch retained a non-boolean explicit value",
                    )),
                };
            }
            let zero = targets.values().iter().find(|target| target.value() == 0);
            let one = targets.values().iter().find(|target| target.value() == 1);
            if targets.values().len() != 2 || zero.is_none() || one.is_none() {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "a comparison predicate switch whose exact boolean variants were not retained",
                ));
            }
            let otherwise = target(targets.otherwise().target())?;
            if !switch_fallback_is_empty_unreachable_v1(function, otherwise) {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "a comparison predicate switch with a reachable non-boolean successor",
                ));
            }
            let one = one.ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                "a comparison predicate switch lost its true variant",
            ))?;
            let zero = zero.ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                "a comparison predicate switch lost its false variant",
            ))?;
            Ok(ProjectedCfgTerminatorV1::Predicate {
                predicate,
                true_block: target(one.edge().target())?,
                false_block: target(zero.edge().target())?,
            })
        }
        SemanticTerminatorKindV1::Call(call) => {
            if matches!(call.unwind(), SemanticUnwindActionV1::Cleanup(_)) {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "a call with cleanup control flow before exact unwind projection",
                ));
            }
            match call.destination() {
                Some(destination) => Ok(ProjectedCfgTerminatorV1::Branch(target(
                    destination.edge().target(),
                )?)),
                None if matches!(
                    callables.get(call.callee().index() as usize),
                    Some(SemanticCallableDeclV1::CompilerIntrinsic {
                        operation: SemanticCompilerIntrinsicOperationV1::Trap,
                        ..
                    })
                ) =>
                {
                    Ok(ProjectedCfgTerminatorV1::Trap)
                }
                None => Ok(ProjectedCfgTerminatorV1::Return),
            }
        }
        SemanticTerminatorKindV1::Assert {
            condition,
            expected,
            message,
            target: edge,
            ..
        } => {
            if !matches!(message, SemanticAssertMessageV1::BoundsCheck { .. }) {
                let masked_source_proof = assertion_facts.masked_assertion_source_proved_v1(
                    function,
                    block_index,
                    *expected,
                    edge.target(),
                )?;
                // A contradictory actual graph constant overrides a source proof.
                let graph_condition =
                    assertion_facts.condition(block_index, *expected, edge.target())?;
                if !projected_assertion_is_proved_v1(
                    graph_condition,
                    *expected,
                    non_bounds_assert_proved || masked_source_proof,
                ) {
                    return Err(ProductionRankedProjectionErrorV1::UnprovenAssert {
                        block: block_index,
                        kind: semantic_assert_kind_v1(message),
                        expected: *expected,
                        condition_local: operand_local(condition, owned.is_some())
                            .map(SemanticLocalIdV1::index),
                        source: Box::new(block.terminator().source()),
                    });
                }
            }
            Ok(ProjectedCfgTerminatorV1::Branch(target(edge.target())?))
        }
        SemanticTerminatorKindV1::Drop { target: edge, .. } => {
            Ok(ProjectedCfgTerminatorV1::Branch(target(edge.target())?))
        }
        SemanticTerminatorKindV1::FalseEdge { .. } => {
            Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a false edge before exact semantic CFG normalization",
            ))
        }
        SemanticTerminatorKindV1::Return
        | SemanticTerminatorKindV1::TailCall(_)
        | SemanticTerminatorKindV1::UnwindResume
        | SemanticTerminatorKindV1::UnwindTerminate
        | SemanticTerminatorKindV1::Abort
        | SemanticTerminatorKindV1::Unreachable => Ok(ProjectedCfgTerminatorV1::Return),
    }
}

#[cfg(test)]
#[path = "root_cfg_terminator_resources_v1_tests.rs"]
mod tests;
