//! Invariant source SSA values extend a bounds guard through its success region.

use super::*;
use fe2o3_mir_model::{SsaBlockIdV1, SsaConstructionPlanV1, SsaResolvedEventV1, SsaValueV1};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Invariant {
    Unpromoted,
    Unseen,
    Definition(SsaValueV1),
    Changed,
}

#[cfg(test)]
pub(super) const INVARIANT_ROW_BYTES_FOR_TEST: usize = std::mem::size_of::<Invariant>();

fn invalid(detail: &'static str) -> ProductionRankedProjectionErrorV1 {
    ProductionRankedProjectionErrorV1::Incomplete(detail)
}

fn resource(
    error: fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1,
) -> ProductionRankedProjectionErrorV1 {
    ProductionRankedProjectionErrorV1::CanonicalAssertions(
        canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(error),
    )
}

fn observe(slot: &mut Invariant, value: Option<SsaValueV1>) {
    let Some(value @ SsaValueV1::Definition(_)) = value else {
        *slot = Invariant::Changed;
        return;
    };
    *slot = match *slot {
        Invariant::Unseen => Invariant::Definition(value),
        Invariant::Definition(previous) if previous == value => Invariant::Definition(value),
        Invariant::Unpromoted => Invariant::Unpromoted,
        _ => Invariant::Changed,
    };
}

/// The caller supplies the existing plan of this exact selected source body.
/// This is deliberately stronger than pointwise equality: any kill, phi,
/// redefinition, or observable local storage disables the new CFG extension.
/// Existing immediate-success projection does not acquire this requirement.
pub(super) fn authenticate_invariants(
    function: &SemanticFunctionDeclV1,
    plan: &SsaConstructionPlanV1,
    checks: &mut [ProjectedBoundsCheckV1],
    facts: &mut impl ProjectedAssertionFactsV1,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if checks.is_empty() || !facts.checked_control_enabled_v1() {
        return Ok(());
    }
    facts.with_checked_control_scope_v1(|facts| {
        facts.charge_private_array_work(4)?;
        let count = function.locals().len();
        let bytes = count
            .checked_mul(std::mem::size_of::<Invariant>())
            .and_then(|n| n.checked_add(std::mem::size_of::<Vec<Invariant>>()))
            .ok_or_else(|| {
                resource(fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)
            })?;
        facts.reserve_checked_control_storage_v1(bytes)?;
        // Pay and check each actual allocation before the next fallible action.
        facts.charge_private_array_work(4)?;
        let mut values = Vec::new();
        values.try_reserve_exact(count).map_err(|_| {
            resource(fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Allocation)
        })?;
        reconcile_capacity(&values, count, facts)?;
        facts.charge_private_array_work(count)?;
        values.resize(count, Invariant::Unpromoted);
        for variable in plan.promoted_variables() {
            facts.charge_private_array_work(2)?;
            *values.get_mut(variable.get() as usize).ok_or_else(|| {
                invalid("bounds SSA variable is outside the source local table")
            })? = Invariant::Unseen;
        }
        for argument in plan.entry_definitions() {
            facts.charge_private_array_work(3)?;
            let slot = values
                .get_mut(argument.variable().get() as usize)
                .ok_or_else(|| {
                    invalid("bounds SSA entry variable is outside the source local table")
                })?;
            observe(slot, Some(argument.value()));
        }
        for block in 0..function.blocks().len() {
            facts.charge_private_array_work(2)?;
            let Some(events) = plan.resolved_events(SsaBlockIdV1::new(block as u32)) else {
                continue;
            };
            for (_, event) in events {
                facts.charge_private_array_work(4)?;
                let (variable, value) = match *event {
                    SsaResolvedEventV1::Use { variable, value }
                    | SsaResolvedEventV1::Define { variable, value } => (variable, Some(value)),
                    SsaResolvedEventV1::Kill { variable, .. } => (variable, None),
                };
                let slot = values.get_mut(variable.get() as usize).ok_or_else(|| {
                    invalid("bounds SSA event variable is outside the source local table")
                })?;
                observe(slot, value);
            }
        }
        for check in checks {
            facts.charge_private_array_work(12)?;
            check.invariant_ssa = [
                check.condition_local,
                check.index_local,
                check.length_local,
                check.slice_local,
            ]
            .iter()
            .all(|local| {
                matches!(
                    values.get(local.index() as usize),
                    Some(Invariant::Definition(_))
                )
            });
        }
        drop(values);
        Ok(())
    })
}

pub(super) fn controls(
    check: ProjectedBoundsCheckV1,
    block: usize,
    dominance: Option<&SemanticEnumPayloadDominanceV1>,
) -> bool {
    check.access_block == block
        || (check.invariant_ssa
            && dominance.is_some_and(|dominance| {
                dominance.block_dominates(
                    SemanticBlockIdV1::from_index(check.access_block as u32),
                    SemanticBlockIdV1::from_index(block as u32),
                )
            }))
}

fn lookup_payment_enabled(
    checks: &[ProjectedBoundsCheckV1],
    facts: &mut impl ProjectedAssertionFactsV1,
) -> Result<bool, ProductionRankedProjectionErrorV1> {
    if checks.is_empty() || !facts.checked_control_enabled_v1() {
        return Ok(false);
    }
    facts.charge_private_array_work(checks.len())?;
    Ok(checks.iter().any(|check| check.invariant_ssa))
}

fn prepay_place(
    checks: usize,
    place: &SemanticPlaceV1,
    facts: &mut impl ProjectedAssertionFactsV1,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    facts.charge_private_array_work(3)?;
    // At most one lookup per original place projection. Each scans at most
    // `checks` rows twice (immediate then extended), with one O(1) interval
    // query and fixed identity comparisons per row in the extended pass.
    // Paying every projection also covers early failures and non-index places.
    let work = checks
        .checked_mul(12)
        .and_then(|n| n.checked_mul(place.projections().len()))
        .ok_or_else(|| {
            resource(fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)
        })?;
    facts.charge_private_array_work(work)
}

fn prepay_operand(
    checks: usize,
    operand: &SemanticOperandV1,
    facts: &mut impl ProjectedAssertionFactsV1,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    facts.charge_private_array_work(1)?;
    match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
            prepay_place(checks, place, facts)
        }
        SemanticOperandV1::Constant(_) => Ok(()),
    }
}

/// A cost-only preflight of the existing place projector, not a second
/// definition/alias analysis. Original operand multiplicity is not deduplicated.
pub(super) fn prepay_statement_lookups(
    checks: &[ProjectedBoundsCheckV1],
    statement: &SemanticStatementKindV1,
    facts: &mut impl ProjectedAssertionFactsV1,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if !lookup_payment_enabled(checks, facts)? {
        return Ok(());
    }
    facts.charge_private_array_work(1)?;
    let count = checks.len();
    match statement {
        SemanticStatementKindV1::Assign(assignment) => {
            prepay_place(count, assignment.destination(), facts)?;
            assignment
                .value()
                .kind()
                .try_visit_operands(|operand| prepay_operand(count, operand, facts))?;
            match assignment.value().kind() {
                SemanticRvalueKindV1::Borrow { place, .. }
                | SemanticRvalueKindV1::AddressOf { place, .. }
                | SemanticRvalueKindV1::Length(place)
                | SemanticRvalueKindV1::Discriminant(place) => prepay_place(count, place, facts),
                SemanticRvalueKindV1::Load(load) => prepay_place(count, load.source(), facts),
                _ => Ok(()),
            }
        }
        SemanticStatementKindV1::Store(store) => {
            prepay_place(count, store.destination(), facts)?;
            prepay_operand(count, store.value(), facts)
        }
        SemanticStatementKindV1::AtomicRmw(atomic) => {
            prepay_place(count, atomic.destination(), facts)?;
            prepay_place(count, atomic.address(), facts)?;
            prepay_operand(count, atomic.value(), facts)
        }
        SemanticStatementKindV1::AtomicCompareExchange(atomic) => {
            prepay_place(count, atomic.destination(), facts)?;
            prepay_place(count, atomic.address(), facts)?;
            prepay_operand(count, atomic.expected(), facts)?;
            prepay_operand(count, atomic.replacement(), facts)
        }
        SemanticStatementKindV1::SetDiscriminant { place, .. }
        | SemanticStatementKindV1::Deinitialize(place) => prepay_place(count, place, facts),
        SemanticStatementKindV1::Assume(condition) => prepay_operand(count, condition, facts),
        SemanticStatementKindV1::StorageLive(_)
        | SemanticStatementKindV1::StorageDead(_)
        | SemanticStatementKindV1::Nop => Ok(()),
    }
}

pub(super) fn prepay_terminator_lookups(
    checks: &[ProjectedBoundsCheckV1],
    terminator: &SemanticTerminatorKindV1,
    facts: &mut impl ProjectedAssertionFactsV1,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if !lookup_payment_enabled(checks, facts)? {
        return Ok(());
    }
    facts.charge_private_array_work(1)?;
    let count = checks.len();
    match terminator {
        SemanticTerminatorKindV1::SwitchInt { discriminant, .. }
        | SemanticTerminatorKindV1::Assert {
            condition: discriminant,
            ..
        } => prepay_operand(count, discriminant, facts),
        SemanticTerminatorKindV1::Call(call) => {
            if let Some(destination) = call.destination() {
                prepay_place(count, destination.place(), facts)?;
            }
            for argument in call.arguments() {
                prepay_operand(count, argument, facts)?;
            }
            Ok(())
        }
        SemanticTerminatorKindV1::TailCall(call) => {
            for argument in call.arguments() {
                prepay_operand(count, argument, facts)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Rows are emitted in source block order, once per guard, before projection.
pub(super) fn source_guard(
    checks: &[ProjectedBoundsCheckV1],
    block: usize,
    successor: SemanticBlockIdV1,
    facts: &mut impl ProjectedAssertionFactsV1,
) -> Result<Option<ProjectedBoundsCheckV1>, ProductionRankedProjectionErrorV1> {
    let mut start = 0;
    let mut end = checks.len();
    while start < end {
        facts.charge_private_array_work(2)?;
        let middle = start + (end - start) / 2;
        let check = checks[middle];
        match check.guard_block.cmp(&block) {
            std::cmp::Ordering::Less => start = middle + 1,
            std::cmp::Ordering::Greater => end = middle,
            std::cmp::Ordering::Equal => {
                facts.charge_private_array_work(3)?;
                return Ok((check.must_authorize_access
                    && check.invariant_ssa
                    && check.access_block == successor.index() as usize)
                    .then_some(check));
            }
        }
    }
    Ok(None)
}

#[cfg(test)]
pub(super) fn deferred_guard(
    checks: &[ProjectedBoundsCheckV1],
    block: usize,
    successor: SemanticBlockIdV1,
    facts: &mut impl ProjectedAssertionFactsV1,
) -> Result<bool, ProductionRankedProjectionErrorV1> {
    Ok(source_guard(checks, block, successor, facts)?.is_some())
}

// Allocation and this reconciliation are prepaid as one step. Excess is
// recorded before any later work denial; the caller drops the vector before
// restoring the accounting scope, including when this reservation is denied.
pub(super) fn reconcile_capacity<T>(
    values: &Vec<T>,
    requested: usize,
    facts: &mut impl ProjectedAssertionFactsV1,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    let extra = values
        .capacity()
        .checked_sub(requested)
        .and_then(|n| n.checked_mul(std::mem::size_of::<T>()))
        .ok_or_else(|| {
            resource(fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)
        })?;
    if extra != 0 {
        facts.reserve_checked_control_storage_v1(extra)?;
    }
    Ok(())
}

#[derive(Clone, Copy, Default)]
pub(super) struct EmptyDefaultIncomingV1 {
    eligible: bool,
    other: bool,
}

impl EmptyDefaultIncomingV1 {
    pub(super) fn eligible_only(self) -> bool {
        self.eligible && !self.other
    }
}

fn predicate_empty_default(
    function: &SemanticFunctionDeclV1,
    source: usize,
    terminator: &ProjectedCfgTerminatorV1,
    reached: bool,
) -> Option<usize> {
    if !reached {
        return None;
    }
    let ProjectedCfgTerminatorV1::Predicate {
        true_block,
        false_block,
        ..
    } = terminator
    else {
        return None;
    };
    let SemanticTerminatorKindV1::SwitchInt { targets, .. } =
        function.blocks().get(source)?.terminator().kind()
    else {
        return None;
    };
    if targets.values().len() != 2 {
        return None;
    }
    let zero = targets.values().iter().find(|target| target.value() == 0)?;
    let one = targets.values().iter().find(|target| target.value() == 1)?;
    let otherwise = targets.otherwise().target().index() as usize;
    (zero.edge().target().index() as usize == *false_block
        && one.edge().target().index() as usize == *true_block
        && targets.otherwise().role() == SemanticEdgeRoleV1::SwitchOtherwise
        && switch_fallback_is_empty_unreachable_v1(function, otherwise))
    .then_some(otherwise)
}

// Numeric projection bookkeeping only. The caller keeps the returned allocation
// inside its checked-control storage scope; these rows are not source authority.
pub(super) fn empty_default_incoming(
    function: &SemanticFunctionDeclV1,
    terminators: &[ProjectedCfgTerminatorV1],
    reachable: &[bool],
    facts: &mut impl ProjectedAssertionFactsV1,
) -> Result<Vec<EmptyDefaultIncomingV1>, ProductionRankedProjectionErrorV1> {
    facts.charge_private_array_work(12)?;
    let count = function.blocks().len();
    if terminators.len() != count || reachable.len() != count {
        return Err(invalid("empty-default source CFG row count differs"));
    }
    let bytes = count
        .checked_mul(std::mem::size_of::<EmptyDefaultIncomingV1>())
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Vec<EmptyDefaultIncomingV1>>()))
        .ok_or_else(|| {
            resource(fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)
        })?;
    facts.reserve_checked_control_storage_v1(bytes)?;
    let mut incoming = Vec::new();
    incoming.try_reserve_exact(count).map_err(|_| {
        resource(fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Allocation)
    })?;
    reconcile_capacity(&incoming, count, facts)?;
    facts.charge_private_array_work(count)?;
    incoming.resize(count, EmptyDefaultIncomingV1::default());
    for (source, block) in function.blocks().iter().enumerate() {
        facts.charge_private_array_work(12)?;
        let default =
            predicate_empty_default(function, source, &terminators[source], reachable[source]);
        block.terminator().kind().try_for_each_edge(|edge| {
            facts.charge_private_array_work(4)?;
            let target = edge.target().index() as usize;
            let row = incoming
                .get_mut(target)
                .ok_or_else(|| invalid("empty-default source edge target is absent"))?;
            if default == Some(target) && edge.role() == SemanticEdgeRoleV1::SwitchOtherwise {
                row.eligible = true;
            } else {
                row.other = true;
            }
            Ok::<(), ProductionRankedProjectionErrorV1>(())
        })?;
    }
    Ok(incoming)
}

fn empty_default_coverage_candidate(
    function: &SemanticFunctionDeclV1,
    projected: &[ProjectedSemanticBlockV1],
    reachable: &[bool],
    index: usize,
    coverage: fe2o3_lower_mir_kernel::ProductionSourceOutputBlockCoverageV1,
) -> bool {
    reachable.get(index) == Some(&false)
        && matches!(
            coverage.disposition(),
            fe2o3_lower_mir_kernel::ProductionSourceOutputBlockV1::Materialized {
                executable: true,
                ..
            }
        )
        && index != function.entry().index() as usize
        && switch_fallback_is_empty_unreachable_v1(function, index)
        && projected
            .get(index)
            .is_some_and(|block| block.items.is_empty())
        && coverage.source_statements() == 0
        && coverage.original_operations() == Some(0)
}

// The ordinary all-matching path retains the original query/charge order and
// does not enter a storage scope or inspect any new structural candidates.
pub(super) fn verify_all_live_coverage(
    function: &SemanticFunctionDeclV1,
    projected: &[ProjectedSemanticBlockV1],
    terminators: &[ProjectedCfgTerminatorV1],
    reachable: &[bool],
    facts: &mut impl ProjectedAssertionFactsV1,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    for (index, reached) in reachable.iter().copied().enumerate() {
        facts.charge_private_array_work(2)?;
        let coverage = facts.checked_block_coverage_v1(index)?;
        if reached
            != matches!(
                coverage.disposition(),
                fe2o3_lower_mir_kernel::ProductionSourceOutputBlockV1::Materialized {
                    executable: true,
                    ..
                }
            )
        {
            verify_all_live_empty_default_suffix(
                function,
                projected,
                terminators,
                reachable,
                index,
                coverage,
                facts,
            )?;
            break;
        }
    }
    Ok(())
}

// Enter only after the old loop's first mismatch and successful live coverage
// query. Consume its suffix once, without revisiting earlier rows or queries.
fn verify_all_live_empty_default_suffix(
    function: &SemanticFunctionDeclV1,
    projected: &[ProjectedSemanticBlockV1],
    terminators: &[ProjectedCfgTerminatorV1],
    reachable: &[bool],
    first: usize,
    first_coverage: fe2o3_lower_mir_kernel::ProductionSourceOutputBlockCoverageV1,
    facts: &mut impl ProjectedAssertionFactsV1,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    const MISMATCH: &str = "checked all-live source CFG and sealed block disposition disagree";
    facts.charge_private_array_work(6)?;
    if !empty_default_coverage_candidate(function, projected, reachable, first, first_coverage) {
        return Err(invalid(MISMATCH));
    }
    facts.with_checked_control_scope_v1(|facts| {
        let incoming = empty_default_incoming(function, terminators, reachable, facts)?;
        if projected.len() != incoming.len() {
            return Err(invalid("empty-default projected block count differs"));
        }
        for index in first..incoming.len() {
            let coverage = if index == first {
                first_coverage
            } else {
                facts.charge_private_array_work(2)?;
                facts.checked_block_coverage_v1(index)?
            };
            facts.charge_private_array_work(6)?;
            let live = matches!(
                coverage.disposition(),
                fe2o3_lower_mir_kernel::ProductionSourceOutputBlockV1::Materialized {
                    executable: true,
                    ..
                }
            );
            if reachable[index] == live {
                continue;
            }
            if !incoming[index].eligible_only()
                || !empty_default_coverage_candidate(
                    function, projected, reachable, index, coverage,
                )
            {
                return Err(invalid(MISMATCH));
            }
        }
        drop(incoming);
        Ok(())
    })
}

pub(super) fn retain_all_live_source_guards(
    terminators: &mut [ProjectedCfgTerminatorV1],
    checks: &[ProjectedBoundsCheckV1],
    facts: &mut impl ProjectedAssertionFactsV1,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if !facts.checked_control_enabled_v1() {
        return Ok(());
    }
    for check in checks {
        facts.charge_private_array_work(3)?;
        if !check.retain_source_guard {
            continue;
        }
        if !check.invariant_ssa || !check.must_authorize_access {
            return Err(invalid(
                "cross-block bounds guard lost its source invariant",
            ));
        }
        let terminator = terminators
            .get_mut(check.guard_block)
            .ok_or_else(|| invalid("cross-block bounds guard source block is absent"))?;
        if *terminator != ProjectedCfgTerminatorV1::Branch(check.access_block) {
            return Err(invalid("cross-block bounds guard success control changed"));
        }
        match facts.condition(
            check.guard_block,
            true,
            SemanticBlockIdV1::from_index(check.access_block as u32),
        )? {
            canonical_assertion_facts_v1::ProjectedAssertionConditionV1::Bool(true) => {}
            canonical_assertion_facts_v1::ProjectedAssertionConditionV1::Dynamic
            | canonical_assertion_facts_v1::ProjectedAssertionConditionV1::Unknown => {
                *terminator = ProjectedCfgTerminatorV1::BoundsAssert {
                    index: check.index,
                    extent: check.extent,
                    success: check.access_block,
                };
            }
            _ => {
                return Err(invalid(
                    "cross-block bounds guard lacks a live checked condition",
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn verify_consumption(
    checks: &mut [ProjectedBoundsCheckV1],
    blocks: &[ProjectedSemanticBlockV1],
    views: &[Option<ProjectedViewV1>],
    dominance: &SemanticEnumPayloadDominanceV1,
    frame: Option<&checked_control_v1::CheckedControlFrameV1>,
    facts: &mut impl ProjectedAssertionFactsV1,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if !facts.checked_control_enabled_v1() {
        // Preserve the legacy N-only grammar and cost surface. Cross-block
        // authority is never installed for this path.
        let missing = checks.iter().any(|check| {
            check.must_authorize_access
                && blocks.get(check.access_block).is_none_or(|block| {
                    !block.items.iter().any(|item| match item {
                        ProjectedBlockItemV1::Effect {
                            operation:
                                ProductionRankedOperationV1::Access { indices, .. }
                                | ProductionRankedOperationV1::AtomicAccess { indices, .. },
                            ..
                        } => indices.contains(&check.index),
                        ProjectedBlockItemV1::Guarded(access) => {
                            access.indices.contains(&check.index)
                                && access.comparisons.contains(&(check.index, check.extent))
                        }
                        _ => false,
                    })
                })
        });
        return if missing {
            Err(invalid(
                "a Rust bounds assertion does not authorize one matching projected access",
            ))
        } else {
            Ok(())
        };
    }
    for check in checks {
        facts.charge_private_array_work(3)?;
        if !check.must_authorize_access
            || frame.is_some_and(|frame| !frame.projects_block(check.guard_block))
        {
            continue;
        }
        let expected_view = views
            .get(check.slice_local.index() as usize)
            .and_then(Option::as_ref)
            .map(|view| view.result);
        let mut used = false;
        let mut immediate_used = false;
        for (block_index, block) in blocks.iter().enumerate() {
            facts.charge_private_array_work(2)?;
            if !controls(*check, block_index, Some(dominance)) {
                continue;
            }
            for item in &block.items {
                facts.charge_private_array_work(3)?;
                if let ProjectedBlockItemV1::Guarded(access) = item {
                    facts.charge_private_array_work(access.indices.len())?;
                    let index_matches = access.indices.contains(&check.index);
                    facts.charge_private_array_work(access.comparisons.len())?;
                    let matches = Some(access.view) == expected_view
                        && index_matches
                        && access.comparisons.contains(&(check.index, check.extent));
                    used |= matches;
                    immediate_used |= matches
                        && block_index == check.access_block
                        && access.checked_success.is_none();
                }
            }
        }
        if !used {
            return Err(invalid(
                "a Rust bounds assertion does not authorize one matching projected access",
            ));
        }
        check.retain_source_guard = check.invariant_ssa && !immediate_used;
    }
    Ok(())
}
