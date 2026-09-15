//! Closed Workgroup reference flows are address-transparent, not SSA authority.

use super::kernel_context_borrows_v1::KernelContextBorrowV1;
use super::matrix_borrows_v1::MatrixBorrowSitesV1;
use super::math_borrows_v1::{MathBorrowSitesV1, MathConsumerBorrowV1};
use super::global_bf16_borrows_v1::GlobalBf16BorrowV1;
use super::*;
mod context_issue;
mod allocation_borrow;
use allocation_borrow::AllocationBorrow;
mod context_call_transfers;
mod ordered_context_loans;
mod owner_proof_cache_v1;
mod flow_work_profile_v1;
#[path = "borrowed_workgroup_v1/uses_observation_v1.rs"]
mod uses_observation_v1;
use uses_observation_v1::Part as UsesWorkPart;
mod global_statement_index_v1;
mod callable_facts_cache_v1;
mod candidate_source_v1;
mod global_carrier_flow_v1;
mod math_capture_flow_v1;
mod workgroup_role_joins_v1;
mod phase_borrows;
mod grid_read_borrows_v1;
mod all_use_observation_v1;
mod matrix_access_borrow;
mod closed_lane_flow;
mod borrow_components_v1;
use matrix_access_borrow::MatrixAccessBorrow;
mod lane_work_v1;
use flow_work_profile_v1::{Profile as FlowWorkProfile, Stage as FlowWorkStage};
use context_issue::WorkgroupContextBorrowV1;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBorrowKindV1, SemanticExecutionCapabilityContractV1,
    SemanticExecutionCapabilityOperationV1 as E, SemanticSubgroupPartitionOperationV1 as P,
    SemanticWorkgroupEpochProjectionV1,
};

const MAX_FLOW_WORK: usize = 262_144;

#[derive(Clone)]
struct EpochSite {
    site: SemanticTransparentBorrowSiteV1,
    result: u32,
    receiver: u32,
    record: SemanticWorkgroupEpochProjectionV1,
}

struct Budget {
    remaining: usize,
    limit: usize,
    profile: FlowWorkProfile,
}

impl Budget {
    fn charge(&mut self, work: usize) -> Result<(), ProductionSemanticSsaErrorV1> {
        let error = ProductionSemanticSsaErrorV1::AggregateResourceLimit {
            resource: SsaPlannerResourceV1::WorkUnits,
            required: self.limit + 1,
            limit: self.limit,
        };
        let Some(remaining) = self.remaining.checked_sub(work) else {
            return Err(self.profile.failure(self.remaining, work, error));
        };
        self.remaining = remaining;
        self.profile.charged(work);
        Ok(())
    }

    fn scoped<T>(
        &mut self,
        stage: FlowWorkStage,
        action: impl FnOnce(&mut Self) -> Result<T, ProductionSemanticSsaErrorV1>,
    ) -> Result<T, ProductionSemanticSsaErrorV1> {
        let previous = self.profile.stage;
        self.profile.stage = stage;
        let result = action(self);
        self.profile.stage = previous;
        result
    }
}

pub(super) fn direct_sites(
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
) -> BTreeSet<SemanticTransparentBorrowSiteV1> {
    // Exhaustion keeps the reference storage-observable. This classifier never
    // grants issuance and cannot suppress the later SSA/lowering rejection.
    sites(function, callables, &[], MAX_FLOW_WORK, None).unwrap_or_default()
}

pub(super) fn execution_sites(
    semantic: &AdmittedInertSemanticMirV1,
    expansion: &SemanticCallExpansionV1,
    view: &SemanticExpandedRootV1,
    max_work: usize,
) -> Result<BTreeSet<SemanticTransparentBorrowSiteV1>, ProductionSemanticSsaErrorV1> {
    let bindings = expansion
        .defined_capability_bindings(semantic)
        .map_err(ProductionSemanticSsaErrorV1::CallExpansion)?;
    if !expansion
        .root(view.root())
        .is_some_and(|expected| std::ptr::eq(expected, view))
    {
        return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
    }
    let context_transfers = context_call_transfers::CheckedTransfers::new(semantic, expansion, view)?;
    let math = MathBorrowSitesV1::new(semantic, view, &bindings, max_work)?;
    let matrix = MatrixBorrowSitesV1::new(semantic, expansion, view, &bindings, max_work)?;
    let mut epochs = Vec::new();
    for binding in &bindings {
        if binding.root() != view.root() {
            continue;
        }
        let body = semantic
            .functions()
            .get(binding.contract().function().index() as usize)
            .ok_or(ProductionSemanticSsaErrorV1::ReplayMismatch)?;
        let Some(record) = body.workgroup_epoch_projection().copied() else {
            continue;
        };
        if binding.contract() != fe2o3_mir_model::semantic_mir_v1::SemanticDefinedCapabilityContractV1::WorkgroupEpochProjection(record)
            || binding.arguments().len() != 1 || binding.callee_arguments().len() != 1
            || binding.expansion_identity() != expansion.identity() || binding.root_identity() != view.identity()
        {
            return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
        }
        let site = SemanticTransparentBorrowSiteV1 {
            block: binding.expanded_entry_block().index(),
            statement: 0,
        };
        let source = view
            .block_origins()
            .get(site.block as usize)
            .ok_or(ProductionSemanticSsaErrorV1::ReplayMismatch)?;
        if source.instance() != binding.callee_instance()
            || source.function() != record.function()
            || source.statements().first()
                != Some(&SemanticExpandedStatementOriginV1::Source { statement: 0 })
        {
            return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
        }
        epochs.push(EpochSite {
            site,
            result: binding.callee_return().index(),
            receiver: binding.callee_arguments()[0].index(),
            record,
        });
    }
    sites_with_phase(
        view.body(),
        semantic.callables(),
        &epochs,
        max_work.min(MAX_FLOW_WORK),
        Some(semantic.types()),
        &math,
        &matrix,
        Some(&context_transfers),
        Some(view),
        Some(phase_borrows::Source { semantic, view, bindings: &bindings }),
        #[cfg(test)] None,
    )
}

pub(super) fn typed_direct_sites(
    function: &SemanticFunctionDeclV1,
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
) -> BTreeSet<SemanticTransparentBorrowSiteV1> {
    sites(function, callables, &[], MAX_FLOW_WORK, Some(types)).unwrap_or_default()
}

fn reference_pair(operation: E, argument: usize) -> Option<(SemanticTypeIdV1, SemanticTypeIdV1)> {
    match (operation, argument) {
        (E::Gfx950Transpose(transpose), argument) => transpose.shared_reference_pair(argument),
        (
            E::SubgroupDeriveBorrowed {
                workgroup_reference,
                workgroup,
                width: 64,
                ..
            },
            0,
        ) => Some((workgroup_reference, workgroup)),
        (
            E::SubgroupPartition(P::Derive {
                subgroup_reference,
                subgroup,
                ..
            }),
            0,
        ) => Some((subgroup_reference, subgroup)),
        (
            E::SubgroupPartition(
                P::ReduceSumF32 {
                    partition_reference,
                    partition,
                    ..
                }
                | P::ReduceMaxF32 {
                    partition_reference,
                    partition,
                    ..
                }
                | P::BroadcastF32 {
                    partition_reference,
                    partition,
                    ..
                },
            ),
            0,
        ) => Some((partition_reference, partition)),
        _ => None,
    }
}

fn contract(callable: &SemanticCallableDeclV1) -> Option<SemanticExecutionCapabilityContractV1> {
    match callable {
        SemanticCallableDeclV1::CompilerIntrinsic {
            binding,
            operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
            ..
        } if binding.identity() == contract.source_identity() => Some(*contract),
        _ => None,
    }
}

fn sites(
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    epochs: &[EpochSite],
    max_work: usize,
    types: Option<&[SemanticTypeDeclV1]>,
) -> Result<BTreeSet<SemanticTransparentBorrowSiteV1>, ProductionSemanticSsaErrorV1> {
    sites_with_math(function, callables, epochs, max_work, types, &MathBorrowSitesV1::default())
}

fn sites_with_math(
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    epochs: &[EpochSite],
    max_work: usize,
    types: Option<&[SemanticTypeDeclV1]>,
    math: &MathBorrowSitesV1,
) -> Result<BTreeSet<SemanticTransparentBorrowSiteV1>, ProductionSemanticSsaErrorV1> {
    sites_with_defined(function, callables, epochs, max_work, types, math, &MatrixBorrowSitesV1::default(), None, None)
}

fn sites_with_defined(
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    epochs: &[EpochSite],
    max_work: usize,
    types: Option<&[SemanticTypeDeclV1]>,
    math: &MathBorrowSitesV1,
    matrix: &MatrixBorrowSitesV1<'_>,
    context_transfers: Option<&context_call_transfers::CheckedTransfers<'_>>,
    observation_view: Option<&SemanticExpandedRootV1>,
) -> Result<BTreeSet<SemanticTransparentBorrowSiteV1>, ProductionSemanticSsaErrorV1> {
    sites_with_phase(function, callables, epochs, max_work, types, math, matrix, context_transfers, observation_view, None, #[cfg(test)] None)
}

fn sites_with_phase(
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    epochs: &[EpochSite],
    max_work: usize,
    types: Option<&[SemanticTypeDeclV1]>,
    math: &MathBorrowSitesV1,
    matrix: &MatrixBorrowSitesV1<'_>,
    context_transfers: Option<&context_call_transfers::CheckedTransfers<'_>>,
    observation_view: Option<&SemanticExpandedRootV1>,
    phase_source: Option<phase_borrows::Source<'_>>,
    #[cfg(test)] mut grid_probe: Option<&mut grid_read_borrows_v1::inventory_probe::Probe>,
) -> Result<BTreeSet<SemanticTransparentBorrowSiteV1>, ProductionSemanticSsaErrorV1> {
    let mut budget = Budget {
        remaining: max_work,
        limit: max_work,
        profile: FlowWorkProfile::default(),
    };
    budget.charge(callables.len() + function.locals().len() + epochs.len())?;
    let grid_reads = if let Some(source) = phase_source.as_ref() {
        grid_read_borrows_v1::Facts::new(source.semantic, source.view, source.bindings,
            &mut |work| budget.charge(work))?
    } else { grid_read_borrows_v1::Facts::default() };
    #[cfg(test)]
    let grid_reads = if let Some(probe) = grid_probe.as_ref() {
        probe.facts(function, types.expect("typed Grid component fixture"), &mut budget)?
    } else { grid_reads };
    let phase = phase_borrows::Facts::new(phase_source, &mut budget)?;
    budget.charge(math.pairs.len())?;
    budget.charge(matrix.work_units)?;
    budget.charge(matrix.pairs.len())?;
    let mut reference_types = math.pairs.clone();
    for (&reference, &owned) in &matrix.pairs {
        if reference_types.insert(reference, owned).is_some_and(|old| old != owned) {
            return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
        }
    }
    // Logical map header/entries and scan work are charged separately from
    // the bounded exact MatrixAccess validator. No physical allocator claim.
    budget.charge(3)?;
    let mut matrix_access = BTreeMap::new();
    for (index, callable) in callables.iter().enumerate() {
        budget.charge(1)?;
        if types.is_some() && matches!(callable, SemanticCallableDeclV1::CompilerIntrinsic {
            operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract }, ..
        } if matches!(contract.operation(), E::MatrixAccess { .. })) {
            budget.charge(40)?;
            if let Some(fact) = types.and_then(|types| MatrixAccessBorrow::for_callable(types, callable)) {
                budget.charge(9 + (usize::BITS - matrix_access.len().leading_zeros()) as usize)?;
                matrix_access.insert(index, fact);
            }
        }
    }
    for fact in matrix_access.values().copied() {
        for (reference, owned) in fact.pairs() {
            budget.charge(3 + (usize::BITS - reference_types.len().leading_zeros()) as usize)?;
            if reference_types.insert(reference, owned).is_some_and(|old| old != owned) {
                return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
            }
        }
    }
    for (&reference, &owned) in &phase.pairs {
        budget.charge(2)?;
        if reference_types.insert(reference, owned).is_some_and(|old| old != owned) {
            return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
        }
    }
    for (&reference, &owned) in &grid_reads.pairs {
        budget.charge(12 + (usize::BITS - reference_types.len().leading_zeros()) as usize)?;
        if reference_types.insert(reference, owned).is_some_and(|old| old != owned) {
            return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
        }
    }
    let mut mutable_context_references = BTreeMap::new();
    for (&reference, &owned) in &phase.unique {
        budget.charge(2)?;
        mutable_context_references.insert(reference, owned);
    }
    let global_matrix = types.map(|types| callables.iter()
        .filter_map(|callable| GlobalBf16BorrowV1::for_callable(types, callable))
        .collect::<Vec<_>>()).unwrap_or_default();
    budget.charge(callables.len().saturating_add(global_matrix.len().saturating_mul(32)))?;
    for fact in &global_matrix {
        for (reference, owned) in fact.pairs() {
            if reference_types.insert(reference, owned).is_some_and(|old| old != owned) {
                return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
            }
        }
    }
    for callable in callables {
        budget.charge(16)?;
        let context = types.and_then(|types| KernelContextBorrowV1::for_callable(types, callable));
        let workgroup_context = types.and_then(|types| WorkgroupContextBorrowV1::for_callable(types, callable));
        if matches!(contract(callable).map(|c| c.operation()), Some(E::LdsAllocate { .. })) {
            budget.charge(16)?;
        }
        let allocation = types.and_then(|types| AllocationBorrow::for_callable(types, callable));
        if let Some(fact) = workgroup_context {
            let (reference, owned) = fact.reference_pair();
            mutable_context_references.insert(reference, owned);
        }
        for (reference, owned) in [
            contract(callable).and_then(|contract| reference_pair(contract.operation(), 0)),
            contract(callable).and_then(|contract| match contract.operation() {
                E::Gfx950Transpose(transpose) => transpose.shared_reference_pair(1),
                _ => None,
            }),
            context.map(KernelContextBorrowV1::reference_pair),
            workgroup_context.map(WorkgroupContextBorrowV1::reference_pair),
            allocation.map(AllocationBorrow::pair),
            types.and_then(|types| MathConsumerBorrowV1::for_callable(types, callable)).map(MathConsumerBorrowV1::pair),
        ]
        .into_iter()
        .flatten()
        {
            if reference == owned
                || reference_types
                    .insert(reference, owned)
                    .is_some_and(|old| old != owned)
            {
                return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
            }
        }
    }
    for epoch in epochs {
        let ty = epoch.record.types();
        for (reference, owned) in [
            (ty.reference, ty.workgroup),
            (ty.epoch_reference, ty.epoch_type),
        ] {
            if reference == owned
                || reference_types
                    .insert(reference, owned)
                    .is_some_and(|old| old != owned)
            {
                return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
            }
        }
    }
    if reference_types.is_empty() {
        return Ok(BTreeSet::new());
    }
    let global_statements = global_statement_index_v1::GlobalStatementIndex::new(
        &global_matrix, &mut budget,
    )?;
    let carrier_leaves = phase.pairs.iter().map(|(&reference, &owned)| (reference, owned))
        .chain(matrix_access.values().map(|fact| fact.pairs()[0]));
    let math_captures = math_capture_flow_v1::Routes::new_with_matrix_and_leaves(
        function, types, callables, matrix, carrier_leaves, &mut budget,
    )?;
    let closed_lanes = closed_lane_flow::sites_with_observation(function, types, &matrix_access, callables, observation_view, &mut budget)?;
    let explicit = direct_definition_or_lifetime_locals_v1(function);
    budget.profile.stage = FlowWorkStage::Candidates;
    let mut candidates = Vec::new();
    let mut mutable_context_candidates = BTreeSet::new();
    let mut context_shared_reborrows = BTreeSet::new();
    let mut phase_ordered_candidates = BTreeSet::new();
    let mut by_reference = BTreeMap::new();
    let mut duplicates = BTreeSet::new();
    for (block, body) in function.blocks().iter().enumerate() {
        budget.charge(body.statements().len() + 1)?;
        for (statement, source) in body.statements().iter().enumerate() {
            let SemanticStatementKindV1::Assign(assignment) = source.kind() else {
                continue;
            };
            if !assignment.destination().projections().is_empty() {
                continue;
            }
            let reference = assignment.destination().ty();
            let owned = match reference_types.get(&reference).or_else(|| math_captures.owned(reference)) {
                Some(&owned) => owned,
                None => match math_captures.shared_owned(reference, &mut budget)? {
                    Some(owned) => owned,
                    None => continue,
                },
            };
            let site = SemanticTransparentBorrowSiteV1 {
                block: block as u32,
                statement: statement as u32,
            };
            budget.charge(epochs.len().saturating_mul(2))?;
            let epoch = epochs.iter().find(|epoch| epoch.site == site);
            let carrier_source = math_captures.source(assignment, &mut budget)?;
            let source_kind = if carrier_source.is_some() {
                SemanticBorrowCandidateSourceV1::TypedCarrier
            } else {
                SemanticBorrowCandidateSourceV1::Direct
            };
            let (place, source_reference, value_alias) = if let Some(place) = carrier_source {
                (place, Some(place.local().index()),
                    !matches!(assignment.value().kind(), SemanticRvalueKindV1::Borrow { .. }))
            } else if reference_types.contains_key(&reference) {
                match assignment.value().kind() {
                SemanticRvalueKindV1::Borrow {
                    kind,
                    place,
                } if assignment.value().result_type() == reference
                    && ((*kind == SemanticBorrowKindV1::Shared
                        && !mutable_context_references.contains_key(&reference))
                        || (*kind == SemanticBorrowKindV1::Mutable
                            && mutable_context_references.get(&reference) == Some(&owned))) => {
                    if let Some(epoch) = epoch {
                        if assignment.destination().local().index() != epoch.result
                            || place.local().index() != epoch.receiver
                            || reference != epoch.record.types().epoch_reference
                            || function
                                .locals()
                                .get(place.local().index() as usize)
                                .is_none_or(|local| local.ty() != epoch.record.types().reference)
                            || place.projections() != epoch.record.projection()
                        {
                            return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
                        }
                        (place, Some(place.local().index()), false)
                    } else if place.ty() == owned
                        && !epochs
                            .iter()
                            .any(|epoch| epoch.record.types().epoch_reference == reference)
                    {
                        match place.projections() {
                            [] => (place, None, false),
                            [p] if p.kind() == SemanticProjectionKindV1::Dereference
                                && function
                                    .locals()
                                    .get(place.local().index() as usize)
                                    .is_some_and(|local| local.ty() == reference
                                        || ordered_context_loans::shared_reborrow(
                                            types, local.ty(), reference, owned,
                                            &reference_types, &mutable_context_references,
                                        )) =>
                            {
                                (place, Some(place.local().index()), false)
                            }
                            _ => continue,
                        }
                    } else {
                        continue;
                    }
                }
                SemanticRvalueKindV1::Use(
                    SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place),
                ) if place.projections().is_empty() && place.ty() == reference => {
                    (place, Some(place.local().index()), true)
                }
                _ => continue,
                }
            } else {
                continue;
            };
            let local = assignment.destination().local().index();
            let index = candidates.len();
            if epoch.is_none() && source_reference.is_some_and(|source| {
                function.locals().get(source as usize).is_some_and(|local| {
                    ordered_context_loans::shared_reborrow(
                        types, local.ty(), reference, owned,
                        &reference_types, &mutable_context_references,
                    )
                })
            }) {
                budget.charge(1)?;
                context_shared_reborrows.insert(index);
            }
            let phase_unique = phase.unique.contains_key(&reference)
                || math_captures.reference(reference).is_some_and(|leaf| phase.unique.contains_key(&leaf));
            if mutable_context_references.contains_key(&reference) || phase_unique {
                mutable_context_candidates.insert(index);
            }
            if phase_unique {
                budget.charge(1)?;
                phase_ordered_candidates.insert(index);
            }
            let explicit_source = source_reference.is_some()
                || explicit.contains(&place.local())
                || function
                    .locals()
                    .get(place.local().index() as usize)
                    .is_some_and(|local| matches!(local.role(), SemanticLocalRoleV1::Argument(_)));
            candidates.push(SemanticBorrowCandidateV1 {
                site,
                source_local: place.local().index(),
                source_type: owned,
                source_reference,
                value_alias,
                source_kind: candidate_source_v1::record(source_kind, &mut budget)?,
                valid: explicit_source
                    && function.locals().get(local as usize).is_some_and(|local| {
                        local.ty() == reference && local.role() != SemanticLocalRoleV1::Return
                    }),
                consumers: 0,
                intrinsic_consumer: false,
            });
            if duplicates.contains(&local) {
                candidates[index].valid = false;
            } else if let Some(previous) = by_reference.insert(local, index) {
                candidates[previous].valid = false;
                candidates[index].valid = false;
                by_reference.remove(&local);
                duplicates.insert(local);
            }
        }
    }
    let mut observation = all_use_observation_v1::Observation::new(
        observation_view, matrix.carrier_leaves().values().map(|ty| ty.index()),
        matrix_access.values().map(|fact| fact.pairs()[0].1.index()),
        callables, &candidates,
    );
    budget.profile.stage = FlowWorkStage::Uses;
    budget.profile.uses = uses_observation_v1::Observation::new(
        observation_view.map(|view| (view.root().index(), *view.identity())),
        [function.locals().len(), candidates.len(), epochs.len(), global_matrix.len(), callables.len()],
        function as *const SemanticFunctionDeclV1 as usize,
    );
    let mut callable_facts_cache = callable_facts_cache_v1::Cache::new(function, types, callables);
    let mut direct_uses = if context_shared_reborrows.is_empty() && phase_ordered_candidates.is_empty() {
        Vec::new()
    } else {
        budget.charge(candidates.len())?;
        vec![Vec::new(); candidates.len()]
    };
    let mut lane_sites = if closed_lanes.is_empty() { Vec::new() } else {
        budget.charge(3 + candidates.len().saturating_mul(3))?;
        vec![Vec::new(); candidates.len()]
    };
    let mut grid_consumers = BTreeSet::new();
    if !grid_reads.pairs.is_empty() { budget.charge(3)?; }
    let mut matrix_captures = matrix.capture_cursor(&mut |work| budget.charge(work))?;
    budget.profile.uses.setup(UsesWorkPart::GlobalSetup);
    let mut global_carriers = global_carrier_flow_v1::Audit::new(
        function, types, &global_matrix, &global_statements, &explicit, &mut budget,
    )?;
    let mut workgroup_joins = workgroup_role_joins_v1::Joins::default();
    let mut carrier_observation = all_use_observation_v1::CarrierSite::new(observation_view);
    for (block, body) in function.blocks().iter().enumerate() {
        for (statement, source) in body.statements().iter().enumerate() {
            budget.profile.uses.statement(
                function as *const SemanticFunctionDeclV1 as usize,
                source.kind() as *const SemanticStatementKindV1 as usize,
                block as u32, statement as u32,
            );
            budget.charge(1)?;
            let site = SemanticTransparentBorrowSiteV1 { block: block as u32, statement: statement as u32 };
            budget.profile.uses.enter(UsesWorkPart::GlobalStatement, block as u32, statement as u32);
            let mut global_transport = global_carriers.statement_with_transport(site, source.kind(), &mut budget)?;
            // Every capture and closed-lane entry below comes from an Assign.
            // Other statements still invalidate references and audit Global flow.
            if !matches!(source.kind(), SemanticStatementKindV1::Assign(_)) {
                invalidate_reference_uses_in_statement_v1(
                    source.kind(), site, &by_reference, &mut candidates,
                );
                observation.after(site, "ordinary-statement", &candidates);
                continue;
            }
            budget.profile.uses.enter(UsesWorkPart::CarrierSource, block as u32, statement as u32);
            if let SemanticStatementKindV1::Assign(assignment) = source.kind() {
                let candidate_definition = by_reference.get(&assignment.destination().local().index())
                    .copied().filter(|index| candidates[*index].site == site);
                if let Some(index) = candidate_definition
                    && candidate_source_v1::is_carrier(&candidates[index],
                        #[cfg(test)] &math_captures, #[cfg(test)] assignment, &mut budget)? {
                    // Carrier nodes use the same duplicate, escape and component
                    // checks as direct references. No synthetic borrow is issued.
                    budget.profile.uses.enter(UsesWorkPart::CarrierSiblings, block as u32, statement as u32);
                    invalidate_reference_uses_in_statement_v1(source.kind(), site, &by_reference, &mut candidates);
                    let checked_secondary_fields = workgroup_joins.connect(&math_captures, assignment, index,
                        &by_reference, &mut candidates, &mut budget)?;
                    carrier_observation.emit(site, "before-carrier-siblings", checked_secondary_fields, &math_captures,
                        &by_reference, &candidates, matrix.policy_carrier_leaves());
                    math_captures.invalidate_siblings_with_global(assignment, &by_reference,
                        &mut candidates, checked_secondary_fields, global_transport.as_mut(), &mut budget)?;
                    observation.after(site, "carrier-and-siblings", &candidates);
                    continue;
                }
                carrier_observation.emit(site, "non-carrier-source", [None; 2], &math_captures,
                    &by_reference, &candidates, matrix.policy_carrier_leaves());
                budget.profile.uses.enter(UsesWorkPart::CarrierDisjoint, block as u32, statement as u32);
                if math_captures.disjoint_field_use(assignment, &mut budget)? {
                    invalidate_reference_place_v1(assignment.destination(), &by_reference, &mut candidates);
                    observation.after(site, "disjoint-field-use", &candidates);
                    continue;
                }
            }
            budget.profile.uses.enter(UsesWorkPart::ClosedLane, block as u32, statement as u32);
            if !closed_lanes.is_empty() {
                budget.charge(1 + (usize::BITS - closed_lanes.len().leading_zeros()) as usize)?;
            }
            if let Some(relation) = closed_lanes.get(&site) {
                if let Some(&index) = by_reference.get(&relation.parent) {
                    if candidates[index].source_type == relation.owned
                        && function.locals().get(relation.parent as usize)
                            .is_some_and(|local| local.ty() == relation.reference)
                    {
                        let SemanticStatementKindV1::Assign(assignment) = source.kind() else { unreachable!() };
                        invalidate_reference_place_v1(assignment.destination(), &by_reference, &mut candidates);
                        budget.charge(2 + relation.borrow_sites.len().saturating_mul(2))?;
                        lane_sites[index].extend(relation.borrow_sites.iter().copied());
                        candidates[index].consumers = candidates[index].consumers.saturating_add(1);
                        observation.after(site, "closed-lane-projection", &candidates);
                        continue;
                    }
                }
            }
            budget.profile.uses.enter(UsesWorkPart::GridCapture, block as u32, statement as u32);
            if let Some(local) = grid_reads.captured((site.block, site.statement), source.kind(),
                &mut |work| budget.charge(work))?
            {
                let SemanticStatementKindV1::Assign(assignment) = source.kind() else { unreachable!() };
                invalidate_reference_place_v1(assignment.destination(), &by_reference, &mut candidates);
                if let Some(&index) = by_reference.get(&local) {
                    budget.charge(12 + (usize::BITS - grid_consumers.len().leading_zeros()) as usize)?;
                    grid_consumers.insert(index);
                    #[cfg(test)]
                    if let Some(probe) = grid_probe.as_deref_mut() {
                        probe.before_push(site, index, &candidates, &direct_uses, budget.remaining);
                    }
                    if let Some(uses) = direct_uses.get_mut(index) {
                        budget.charge(1)?;
                        uses.push(site);
                    }
                    candidates[index].consumers = candidates[index].consumers.saturating_add(1);
                    candidates[index].intrinsic_consumer = true;
                    #[cfg(test)]
                    if let Some(probe) = grid_probe.as_deref_mut() {
                        probe.after_push(site, index, &candidates, &direct_uses, budget.remaining);
                    }
                }
                observation.after(site, "grid-original-field-read", &candidates);
                continue;
            }
            budget.profile.uses.enter(UsesWorkPart::DefinedCapture, block as u32, statement as u32);
            let math_capture = math.captured(site, source.kind());
            let matrix_capture = matrix_captures.captured(site, source.kind(), &mut |work| budget.charge(work))?;
            let matrix_read = if matrix_capture.is_none() {
                matrix.copied_matrix_field(site, source.kind(), &mut |work| budget.charge(work))?
            } else { None };
            let matrix_capture = matrix_capture.or(matrix_read.as_ref().map(|read| read.as_slice()));
            let phase_capture = phase.captured(site, source.kind());
            if usize::from(math_capture.is_some()) + usize::from(matrix_capture.is_some())
                + usize::from(phase_capture.is_some()) > 1 {
                return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
            }
            if let Some(locals) = phase_capture {
                let SemanticStatementKindV1::Assign(assignment) = source.kind() else { unreachable!() };
                invalidate_reference_place_v1(assignment.destination(), &by_reference, &mut candidates);
                for local in locals.into_iter().flatten() {
                    if let Some(&index) = by_reference.get(&local) {
                        if let Some(uses) = direct_uses.get_mut(index) {
                            budget.charge(1)?;
                            uses.push(site);
                        }
                        candidates[index].consumers = candidates[index].consumers.saturating_add(1);
                        candidates[index].intrinsic_consumer = true;
                    }
                }
                continue;
            }
            if let Some(locals) = math_capture.as_ref().map(|locals| locals.as_slice()).or(matrix_capture) {
                let SemanticStatementKindV1::Assign(assignment) = source.kind() else { unreachable!() };
                invalidate_reference_place_v1(assignment.destination(), &by_reference, &mut candidates);
                for &local in locals {
                    if let Some(&index) = by_reference.get(&local) {
                        if let Some(uses) = direct_uses.get_mut(index) {
                            budget.charge(1)?;
                            uses.push(site);
                        }
                        candidates[index].consumers = candidates[index].consumers.saturating_add(1);
                        candidates[index].intrinsic_consumer = true;
                    }
                }
                observation.after(site, "defined-capture", &candidates);
                continue;
            }
            budget.profile.uses.enter(UsesWorkPart::GlobalCapture, block as u32, statement as u32);
            let captured = global_statements.find(source.kind(), &mut budget, |fact, local| {
                let index = *by_reference.get(&local.index())?;
                let (reference, owned) = fact.pairs()[2];
                (candidates[index].source_type == owned
                    && function.locals().get(local.index() as usize).is_some_and(|ty| ty.ty() == reference))
                    .then_some(index)
            })?;
            if let Some(index) = captured {
                let SemanticStatementKindV1::Assign(assignment) = source.kind() else { unreachable!() };
                invalidate_reference_place_v1(assignment.destination(), &by_reference, &mut candidates);
                if let Some(uses) = direct_uses.get_mut(index) {
                    budget.charge(1)?;
                    uses.push(site);
                }
                candidates[index].consumers = candidates[index].consumers.saturating_add(1);
                candidates[index].intrinsic_consumer = true;
                observation.after(site, "global-capture", &candidates);
                continue;
            }
            invalidate_reference_uses_in_statement_v1(
                source.kind(),
                SemanticTransparentBorrowSiteV1 {
                    block: block as u32,
                    statement: statement as u32,
                },
                &by_reference,
                &mut candidates,
            );
            observation.after(site, "ordinary-statement", &candidates);
        }
        let terminal_site = SemanticTransparentBorrowSiteV1 {
            block: block as u32, statement: body.statements().len() as u32,
        };
        budget.profile.uses.enter(UsesWorkPart::GlobalTerminator, block as u32, body.statements().len() as u32);
        global_carriers.terminator(body.terminator().kind(), &mut budget)?;
        let SemanticTerminatorKindV1::Call(call) = body.terminator().kind() else {
            validate_reference_uses_in_terminator_v1(
                body.terminator().kind(),
                callables,
                &by_reference,
                &mut candidates,
            );
            observation.after(terminal_site, "non-call-terminator", &candidates);
            continue;
        };
        if let Some(destination) = call.destination() {
            invalidate_reference_place_v1(destination.place(), &by_reference, &mut candidates);
            observation.after(terminal_site, "call-destination", &candidates);
        }
        let mut terminal_facts = None;
        budget.profile.uses.enter(UsesWorkPart::TerminalArguments, block as u32, body.statements().len() as u32);
        budget.charge(call.arguments().len().saturating_mul(2))?;
        for (argument, operand) in call.arguments().iter().enumerate() {
            budget.profile.uses.enter(UsesWorkPart::TerminalArguments, block as u32, body.statements().len() as u32);
            let (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = operand else {
                continue;
            };
            if !place.projections().is_empty() {
                invalidate_reference_place_v1(place, &by_reference, &mut candidates);
                observation.after(terminal_site, "call-projected-argument", &candidates);
                continue;
            }
            let Some(&index) = by_reference.get(&place.local().index()) else {
                continue;
            };
            // Only a tracked direct reference needs terminal classification.
            // Every operand is still audited, including projected references.
            let (context, workgroup_context, global_matrix, contract, allocation, math_consumer, signature_matches) =
                if let Some(facts) = terminal_facts {
                    facts
                } else {
                    budget.profile.uses.enter(UsesWorkPart::TerminalCache, block as u32, body.statements().len() as u32);
                    let classified = callable_facts_cache.get(
                        function, types, callables, call.callee(), &mut budget,
                    )?;
                    let context = classified.context;
                    let workgroup_context = classified.workgroup_context;
                    let global_matrix = classified.global_matrix.as_deref().copied();
                    let contract = classified.contract.copied();
                    let allocation = classified.allocation;
                    let math_consumer = classified.math_consumer;
                    let signature_matches = contract.is_some_and(|contract| {
                        call.arguments()
                            .iter()
                            .map(SemanticOperandV1::ty)
                            .eq(contract.signature().arguments())
                            && call.destination().map(|d| d.place().ty()) == Some(contract.signature().output())
                    });
                    let facts = (context, workgroup_context, global_matrix, contract, allocation, math_consumer, signature_matches);
                    terminal_facts = Some(facts);
                    facts
                };
            let pair = contract.and_then(|contract| reference_pair(contract.operation(), argument));
            budget.profile.uses.enter(UsesWorkPart::EpochEvidence, block as u32, body.statements().len() as u32);
            budget.charge(epochs.len())?;
            let projected_epoch = contract.is_some_and(|contract| {
                matches!(contract.operation(), E::SubgroupPartition(P::Derive { epoch, .. }) if argument == 1 && epoch == place.ty())
                    && epochs.iter().any(|epoch| epoch.record.types().epoch_reference == place.ty()
                        && epoch.record.provenance() == contract.provenance()
                        && Some(epoch.record.brand()) == contract.workgroup_brand()
                        && Some(epoch.record.epoch()) == contract.epoch_before() && contract.epoch_after().is_none())
            });
            budget.profile.uses.enter(UsesWorkPart::ArgumentAcceptance, block as u32, body.statements().len() as u32);
            budget.charge(call.arguments().len().saturating_mul(2).saturating_add(4))?;
            if !matrix_access.is_empty() {
                budget.charge(1 + (usize::BITS - matrix_access.len().leading_zeros()) as usize)?;
            }
            let accepted = (signature_matches
                && (pair == Some((place.ty(), candidates[index].source_type)) || projected_epoch))
                || context.is_some_and(|context| {
                    context.accepts(call, argument, candidates[index].source_type)
                })
                || workgroup_context.is_some_and(|context| {
                    context.accepts(call, argument, candidates[index].source_type)
                })
                || matrix_access.get(&(call.callee().index() as usize)).copied()
                    .is_some_and(|fact| fact.accepts(call, argument, candidates[index].source_type))
                || allocation.is_some_and(|fact| {
                    fact.accepts(call, argument, candidates[index].source_type)
                })
                || global_matrix.is_some_and(|fact| {
                    fact.accepts(call, argument, candidates[index].source_type)
                })
                || math_consumer.is_some_and(|fact| fact.accepts(call, argument, candidates[index].source_type));
            budget.profile.uses.enter(UsesWorkPart::UseRecording, block as u32, body.statements().len() as u32);
            if accepted {
                if let Some(uses) = direct_uses.get_mut(index) {
                    budget.charge(1)?;
                    uses.push(SemanticTransparentBorrowSiteV1 {
                        block: block as u32, statement: body.statements().len() as u32,
                    });
                }
                candidates[index].consumers = candidates[index].consumers.saturating_add(1);
                candidates[index].intrinsic_consumer = true;
            } else {
                candidates[index].valid = false;
                observation.after(terminal_site, "call-unaccepted-argument", &candidates);
            }
        }
    }
    #[cfg(test)]
    if let Some(probe) = grid_probe.as_deref_mut() { probe.finish(&direct_uses); }
    #[cfg(test)]
    tests::candidate_source_tests::uses_finished(&candidates, &by_reference,
        &direct_uses, &lane_sites, &mutable_context_candidates,
        &context_shared_reborrows, &phase_ordered_candidates, &grid_consumers);
    budget.profile.finish_uses(budget.remaining);
    budget.profile.stage = FlowWorkStage::Components;
    let global = global_carriers.finish_with_fields(&mut budget)?;
    math_capture_flow_v1::Routes::resolve_global_siblings(&global.fields, &by_reference,
        &mut candidates, &mut budget)?;
    let mut accepted = BTreeSet::new();
    let mut children = vec![Vec::new(); candidates.len()];
    for (index, candidate) in candidates.iter().enumerate() {
        budget.charge(1)?;
        if let Some(parent) = candidate
            .source_reference
            .and_then(|local| by_reference.get(&local))
        {
            children[*parent].push(index);
        }
    }
    let mut owner_proofs = owner_proof_cache_v1::OwnerProofCache::default();
    workgroup_joins.start(&mut children, &mut budget)?;
    // Shared references may fork. Accept a component only when every use stays
    // inside checked forwarding or exact terminals; an escape poisons its root.
    for root in 0..candidates.len() {
        if candidates[root].source_reference.is_some() {
            continue;
        }
        let component = borrow_components_v1::members(
            &candidates, &children, root, &mut budget, |current| {
                !context_shared_reborrows.is_empty()
                    || !phase_ordered_candidates.is_empty()
                    || !mutable_context_candidates.contains(&current)
                    || candidates[current].consumers == 1
            },
        )?;
        let (visited, mut valid) = match component {
            Ok(visited) => (visited, true),
            Err(current) => {
                observation.component(root, Some(current), false, &candidates);
                (BTreeSet::new(), false)
            }
        };
        if valid && (!context_shared_reborrows.is_empty() || !phase_ordered_candidates.is_empty()) {
            budget.charge(visited.len())?;
            valid = if visited.iter().any(|node| context_shared_reborrows.contains(node) || phase_ordered_candidates.contains(node)) {
                owner_proofs.prove(&candidates[root], &mut budget, |budget| {
                    ordered_context_loans::prove_with_phase(
                        function, &candidates, &children, &direct_uses,
                        &mutable_context_candidates, root, context_transfers, Some(&phase), budget,
                    )
                })?
            } else {
                // All old components retain the one-consumer mutable rule.
                visited.iter().all(|node| !mutable_context_candidates.contains(node)
                    || candidates[*node].consumers == 1)
            };
        }
        if valid && !grid_consumers.is_empty() {
            budget.charge(visited.len().saturating_mul(1 + (usize::BITS
                - grid_consumers.len().leading_zeros()) as usize))?;
            if visited.iter().any(|node| grid_consumers.contains(node)) {
                let candidate = &candidates[root];
                valid = grid_reads.allows_root((candidate.site.block, candidate.site.statement),
                    candidate.source_local, candidate.source_type, &mut |work| budget.charge(work))?;
            }
        }
        observation.component(root, None, valid, &candidates);
        if valid {
            workgroup_joins.record(&visited, &mut budget)?;
            for &index in &visited {
                if let Some(sites) = lane_sites.get(index) {
                    budget.charge(sites.len())?;
                    accepted.extend(sites.iter().copied());
                }
            }
            accepted.extend(
                visited
                    .into_iter()
                    .filter(|i| !candidates[*i].value_alias)
                    .map(|i| candidates[i].site),
            );
        }
    }
    workgroup_joins.retain(&candidates, &children, &lane_sites, &mut accepted, &mut budget)?;
    let global_sites = global.sites;
    budget.charge(global_sites.len().saturating_mul(3 + (usize::BITS
        - accepted.len().saturating_add(global_sites.len()).leading_zeros()) as usize))?;
    accepted.extend(global_sites);
    observation.emit_with_routes(function, &candidates, &accepted,
        |ty| math_captures.owned(ty).copied(), &math_captures, &reference_types);
    #[cfg(test)]
    tests::candidate_source_tests::finished(max_work - budget.remaining);
    Ok(accepted)
}

#[cfg(test)]
mod tests;
