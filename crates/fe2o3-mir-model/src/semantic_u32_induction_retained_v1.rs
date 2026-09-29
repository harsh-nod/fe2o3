//! Complete-CFG V1 DATA with externally retained partial ownership.
//! No source authority, replacement ledger, refund, retry or ordinary route.
use super::*;
use crate::semantic_mir_v1::SemanticControlFlowEdgeV1;
use std::mem::size_of;
type AnalysisResult<T> = Result<T, SemanticU32InductionAnalysisErrorV1>;
type Saved<E> = SemanticU32InductionMeteredErrorV1<E>;

/// A fixed status; the full owning failure remains in the retained component.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticU32InductionRetainedFailureV1;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Fresh,
    Terminal,
    Complete,
}

struct Reachability {
    avoided: Option<usize>,
    visited: Vec<bool>,
    pending: Vec<usize>,
}
struct Payload {
    graph: SemanticCfgV1,
    inventory: SemanticInventoryV1,
    reaches: Vec<Reachability>,
    certificates: Vec<SemanticU32InductionNoOverflowCertificateV1>,
    report: Option<SemanticU32InductionNoOverflowReportV1>,
}
impl Payload {
    fn new() -> Self {
        Self {
            graph: SemanticCfgV1 {
                entry: 0,
                successors: Vec::new(),
                predecessors: Vec::new(),
                reachable_scope: None,
            },
            inventory: SemanticInventoryV1 {
                definitions: Vec::new(),
                address_or_projection_hazard: Vec::new(),
                direct_copy_alias: Vec::new(),
                use_counts: Vec::new(),
                checked_additions: Vec::new(),
            },
            reaches: Vec::new(),
            certificates: Vec::new(),
            report: None,
        }
    }
}

/// Inert semantic analysis owner. The caller retains this owner outside every
/// relevant postflight and drops it before refunding its accepted meter credits.
/// The source borrow prevents an equal detached source or address reuse from
/// becoming a matching completed loan. It grants no compiler authority.
pub struct RetainedSemanticU32InductionV1<'s, E> {
    phase: Phase,
    source: Option<(&'s AdmittedInertSemanticMirV1, SemanticFunctionIdV1)>,
    payload: Payload,
    work: usize,
    failure: Option<Saved<E>>,
}
struct Adapter<'a, M: SemanticU32InductionBoundSnapshotMeterV1> {
    original: &'a mut M,
    failure: &'a mut Option<Saved<M::Error>>,
}
impl<M: SemanticU32InductionBoundSnapshotMeterV1> bound_snapshot::InternalMeter for Adapter<'_, M> {
    fn strict_resources(&self) -> bool {
        true
    }
    fn strict_failure(&mut self, arithmetic: bool) {
        if self.failure.is_none() {
            *self.failure = Some(if arithmetic {
                Saved::Arithmetic
            } else {
                Saved::Allocation
            });
        }
    }
    fn charge_work(&mut self, amount: usize) -> AnalysisResult<()> {
        if let Err(error) = self.original.charge_work(amount) {
            *self.failure = Some(Saved::Meter(error));
            return Err(SemanticU32InductionAnalysisErrorV1::Storage);
        }
        Ok(())
    }
    fn reserve_storage(&mut self, amount: usize) -> AnalysisResult<()> {
        if let Err(error) = self.original.reserve_storage(amount) {
            *self.failure = Some(Saved::Meter(error));
            return Err(SemanticU32InductionAnalysisErrorV1::Storage);
        }
        Ok(())
    }
}
struct WorkLease<'a, 'm> {
    budget: WorkBudgetV1<'m>,
    saved: &'a mut usize,
}
impl Drop for WorkLease<'_, '_> {
    fn drop(&mut self) {
        // Fixed copy only: no meter operation, refund or fallible cleanup.
        *self.saved = self.budget.used;
    }
}

impl<'s, E> RetainedSemanticU32InductionV1<'s, E> {
    pub fn new() -> Self {
        Self {
            phase: Phase::Fresh,
            source: None,
            payload: Payload::new(),
            work: 0,
            failure: None,
        }
    }
    /// Original complete-CFG V1 only. The external adapter is responsible for
    /// actual Budget identity/sticky-denial custody; this generic model meter
    /// does not pretend its address is a canonical ledger identity.
    pub fn prepare_into<M: SemanticU32InductionBoundSnapshotMeterV1<Error = E>>(
        &mut self,
        source: &'s AdmittedInertSemanticMirV1,
        function: SemanticFunctionIdV1,
        limits: SemanticU32InductionAnalysisLimitsV1,
        meter: &mut M,
    ) -> Result<(), SemanticU32InductionRetainedFailureV1> {
        let fresh = self.phase == Phase::Fresh;
        self.phase = Phase::Terminal;
        if !fresh {
            return Err(SemanticU32InductionRetainedFailureV1);
        }
        let Some(frame) = retained_frame::<M>() else {
            self.failure = Some(Saved::Arithmetic);
            return Err(SemanticU32InductionRetainedFailureV1);
        };
        let result = {
            let payload = &mut self.payload;
            let bound = &mut self.source;
            let mut adapter = Adapter {
                original: meter,
                failure: &mut self.failure,
            };
            let mut lease = WorkLease {
                budget: WorkBudgetV1 {
                    used: 0,
                    limit: limits.work_units,
                    meter: Some(&mut adapter),
                },
                saved: &mut self.work,
            };
            let budget = &mut lease.budget;
            (|| {
                // Original strict entry admission/source lookup order.
                budget.strict_storage(frame)?;
                budget.extra(128)?;
                let declaration = source.functions().get(function.index() as usize).ok_or(
                    SemanticU32InductionAnalysisErrorV1::InvalidModel(
                        "the requested semantic function is outside the admitted function table",
                    ),
                )?;
                *bound = Some((source, function));
                payload.prepare(
                    source.types(),
                    declaration,
                    source.semantic_sha256(),
                    function,
                    limits,
                    budget,
                )
            })()
        };
        match result {
            Ok(()) if self.failure.is_none() => {
                self.phase = Phase::Complete;
                Ok(())
            }
            Err(error) => {
                if self.failure.is_none() {
                    self.failure = Some(Saved::Analysis(error));
                }
                Err(SemanticU32InductionRetainedFailureV1)
            }
            Ok(()) => Err(SemanticU32InductionRetainedFailureV1),
        }
    }
    pub fn completed_for(
        &self,
        source: &AdmittedInertSemanticMirV1,
        function: SemanticFunctionIdV1,
    ) -> Result<&SemanticU32InductionNoOverflowReportV1, SemanticU32InductionRetainedFailureV1>
    {
        if self.phase != Phase::Complete
            || self.failure.is_some()
            || !self
                .source
                .is_some_and(|(saved, id)| std::ptr::eq(saved, source) && id == function)
        {
            return Err(SemanticU32InductionRetainedFailureV1);
        }
        self.payload
            .report
            .as_ref()
            .ok_or(SemanticU32InductionRetainedFailureV1)
    }
    pub fn failure(&self) -> Option<&Saved<E>> {
        self.failure.as_ref()
    }
    pub fn work_units_observed(&self) -> usize {
        self.work
    }
}

fn filled_into<T: Copy>(
    values: &mut Vec<T>,
    length: usize,
    value: T,
    budget: &mut WorkBudgetV1<'_>,
) -> AnalysisResult<()> {
    budget.extra(length)?;
    budget.strict_reserve(values, length)?;
    values.resize(length, value);
    Ok(())
}
fn nested_into<T>(
    values: &mut Vec<Vec<T>>,
    length: usize,
    budget: &mut WorkBudgetV1<'_>,
) -> AnalysisResult<()> {
    budget.extra(length)?;
    budget.strict_reserve(values, length)?;
    values.resize_with(length, Vec::new);
    Ok(())
}

impl Payload {
    fn prepare(
        &mut self,
        types: &[SemanticTypeDeclV1],
        declaration: &SemanticFunctionDeclV1,
        semantic_mir_sha256: InertSemanticMirSha256V1,
        function: SemanticFunctionIdV1,
        limits: SemanticU32InductionAnalysisLimitsV1,
        budget: &mut WorkBudgetV1<'_>,
    ) -> AnalysisResult<()> {
        if limits.work_units > MAX_SEMANTIC_U32_INDUCTION_WORK_V1
            || limits.certificates > MAX_SEMANTIC_U32_INDUCTION_CERTIFICATES_V1
        {
            return Err(SemanticU32InductionAnalysisErrorV1::InvalidLimits {
                requested_work: limits.work_units,
                maximum_work: MAX_SEMANTIC_U32_INDUCTION_WORK_V1,
                requested_certificates: limits.certificates,
                maximum_certificates: MAX_SEMANTIC_U32_INDUCTION_CERTIFICATES_V1,
            });
        }

        graph_into(declaration, &mut self.graph, &mut self.reaches, budget)?;
        inventory_into(declaration, &self.graph, &mut self.inventory, budget)?;
        budget.reserve_vec(
            &mut self.certificates,
            self.inventory
                .checked_additions
                .len()
                .min(limits.certificates),
            false,
        )?;
        let context = CandidateProofContextV1 {
            types,
            function: declaration,
            semantic_mir_sha256,
            function_id: function,
            graph: &self.graph,
            inventory: &self.inventory,
        };
        for candidate in &self.inventory.checked_additions {
            budget.charge(1)?;
            if let Some(certificate) = prove_candidate_retained(
                &context,
                *candidate,
                BoundResolverV1::Legacy,
                budget,
                &mut self.reaches,
            )?
            .map(|proved| proved.certificate)
            {
                let actual = self.certificates.len().saturating_add(1);
                if actual > limits.certificates {
                    return Err(SemanticU32InductionAnalysisErrorV1::CertificateLimit {
                        actual,
                        limit: limits.certificates,
                    });
                }
                budget.extra(std::mem::size_of::<
                    SemanticU32InductionNoOverflowCertificateV1,
                >())?;
                self.certificates.push(certificate);
            }
        }
        budget.extra(3 * std::mem::size_of::<SemanticU32InductionNoOverflowReportV1>())?;
        prepay_box(&self.certificates, budget)?;
        let certificates = std::mem::take(&mut self.certificates).into_boxed_slice();
        self.report = Some(SemanticU32InductionNoOverflowReportV1 {
            semantic_mir_sha256,
            function,
            function_identity: declaration.identity(),
            checked_additions_examined: self.inventory.checked_additions.len(),
            certificates,
            work_units: budget.used,
            reachable_scope: false,
            ssa_scope_work_units: 0,
            reachable_blocks: None,
        });
        Ok(())
    }
}
fn prepay_box<T>(values: &Vec<T>, budget: &mut WorkBudgetV1<'_>) -> AnalysisResult<()> {
    if values.len() != values.capacity() {
        let bytes = values
            .len()
            .checked_mul(size_of::<T>())
            .ok_or_else(|| budget.strict_failure(true))?;
        budget.extra(bytes)?;
        budget.strict_storage(bytes)?;
    }
    Ok(())
}

fn graph_into(
    function: &SemanticFunctionDeclV1,
    graph: &mut SemanticCfgV1,
    reaches: &mut Vec<Reachability>,
    budget: &mut WorkBudgetV1,
) -> AnalysisResult<()> {
    let block_count = function.blocks().len();
    let entry = function.entry().index() as usize;
    if block_count == 0 || entry >= block_count {
        return Err(SemanticU32InductionAnalysisErrorV1::InvalidControlFlow(
            "the semantic function has no valid entry block",
        ));
    }
    graph.entry = entry;
    nested_into(&mut graph.successors, block_count, budget)?;
    nested_into(&mut graph.predecessors, block_count, budget)?;
    let successors = &mut graph.successors;
    let predecessors = &mut graph.predecessors;
    budget.charge(block_count)?;
    for (source, block) in function.blocks().iter().enumerate() {
        block.terminator().kind().try_for_each_edge(|edge| {
            budget.charge(1)?;
            let target = edge.target().index() as usize;
            if target >= block_count {
                return Err(SemanticU32InductionAnalysisErrorV1::InvalidControlFlow(
                    "a semantic CFG edge is outside the block table",
                ));
            }
            budget.reserve_vec(&mut successors[source], 1, false)?;
            budget.reserve_vec(&mut predecessors[target], 1, false)?;
            budget.extra(2)?;
            successors[source].push(target);
            predecessors[target].push(source);
            Ok(())
        })?;
    }
    let reachable = reachable_retained(graph, None, reaches, budget)?;
    budget.extra(reachable.len())?;
    if reachable.iter().any(|reachable| !reachable) {
        return Err(SemanticU32InductionAnalysisErrorV1::InvalidControlFlow(
            "the semantic CFG contains an unreachable block",
        ));
    }
    Ok(())
}

fn inventory_into(
    function: &SemanticFunctionDeclV1,
    graph: &SemanticCfgV1,
    inventory: &mut SemanticInventoryV1,
    budget: &mut WorkBudgetV1,
) -> AnalysisResult<()> {
    let local_count = function.locals().len();
    filled_into(
        &mut inventory.definitions,
        local_count,
        DefinitionSummaryV1::default(),
        budget,
    )?;
    filled_into(
        &mut inventory.address_or_projection_hazard,
        local_count,
        false,
        budget,
    )?;
    filled_into(&mut inventory.direct_copy_alias, local_count, false, budget)?;
    filled_into(&mut inventory.use_counts, local_count, 0_usize, budget)?;
    let definitions = &mut inventory.definitions;
    let address_or_projection_hazard = &mut inventory.address_or_projection_hazard;
    let direct_copy_alias = &mut inventory.direct_copy_alias;
    let use_counts = &mut inventory.use_counts;
    let checked_additions = &mut inventory.checked_additions;
    budget.charge(local_count)?;

    for (block_index, block) in function.blocks().iter().enumerate() {
        budget.charge(1)?;
        if !graph.is_in_scope(block_index) {
            continue;
        }
        for (statement_index, statement) in block.statements().iter().enumerate() {
            budget.charge(1)?;
            let site = DefinitionSiteV1 {
                block: block_index,
                position: DefinitionPositionV1::Statement(statement_index),
            };
            match statement.kind() {
                SemanticStatementKindV1::Assign(assignment) => {
                    record_definition(
                        assignment.destination(),
                        site,
                        definitions,
                        address_or_projection_hazard,
                        budget,
                    )?;
                    if let SemanticRvalueKindV1::CheckedBinary(checked) = assignment.value().kind()
                        && checked.operation() == SemanticCheckedBinaryOpV1::Add
                    {
                        budget.reserve_vec(checked_additions, 1, false)?;
                        budget.extra(std::mem::size_of::<CandidateSiteV1>())?;
                        checked_additions.push(CandidateSiteV1 {
                            block: block_index,
                            statement: statement_index,
                        });
                    }
                    if let SemanticRvalueKindV1::Use(source) = assignment.value().kind()
                        && let Some(place) = operand_place(source)
                        && place.projections().is_empty()
                        && place.local() != assignment.destination().local()
                    {
                        let slot = local_slot_mut(
                            direct_copy_alias,
                            place.local(),
                            "a copied semantic local is outside the local table",
                        )?;
                        *slot = true;
                    }
                    inspect_rvalue(
                        assignment.value().kind(),
                        address_or_projection_hazard,
                        use_counts,
                        budget,
                    )?;
                }
                SemanticStatementKindV1::Store(store) => {
                    record_definition(
                        store.destination(),
                        site,
                        definitions,
                        address_or_projection_hazard,
                        budget,
                    )?;
                    mark_address_hazard(store.destination(), address_or_projection_hazard, budget)?;
                    inspect_operand(
                        store.value(),
                        address_or_projection_hazard,
                        use_counts,
                        budget,
                    )?;
                }
                SemanticStatementKindV1::AtomicRmw(atomic) => {
                    record_definition(
                        atomic.destination(),
                        site,
                        definitions,
                        address_or_projection_hazard,
                        budget,
                    )?;
                    mark_address_hazard(atomic.address(), address_or_projection_hazard, budget)?;
                    inspect_operand(
                        atomic.value(),
                        address_or_projection_hazard,
                        use_counts,
                        budget,
                    )?;
                }
                SemanticStatementKindV1::AtomicCompareExchange(atomic) => {
                    record_definition(
                        atomic.destination(),
                        site,
                        definitions,
                        address_or_projection_hazard,
                        budget,
                    )?;
                    mark_address_hazard(atomic.address(), address_or_projection_hazard, budget)?;
                    inspect_operand(
                        atomic.expected(),
                        address_or_projection_hazard,
                        use_counts,
                        budget,
                    )?;
                    inspect_operand(
                        atomic.replacement(),
                        address_or_projection_hazard,
                        use_counts,
                        budget,
                    )?;
                }
                SemanticStatementKindV1::SetDiscriminant { place, .. }
                | SemanticStatementKindV1::Deinitialize(place) => record_definition(
                    place,
                    site,
                    definitions,
                    address_or_projection_hazard,
                    budget,
                )?,
                SemanticStatementKindV1::Assume(operand) => {
                    inspect_operand(operand, address_or_projection_hazard, use_counts, budget)?
                }
                SemanticStatementKindV1::StorageLive(_)
                | SemanticStatementKindV1::StorageDead(_)
                | SemanticStatementKindV1::Nop => {}
            }
        }
        inspect_terminator(
            block,
            block_index,
            definitions,
            address_or_projection_hazard,
            use_counts,
            budget,
        )?;
    }
    Ok(())
}

fn reachable_retained<'a>(
    graph: &SemanticCfgV1,
    avoided: Option<usize>,
    reaches: &'a mut Vec<Reachability>,
    budget: &mut WorkBudgetV1,
) -> AnalysisResult<&'a [bool]> {
    budget.strict_reserve(reaches, 1)?;
    budget.extra(size_of::<Reachability>())?;
    reaches.push(Reachability {
        avoided,
        visited: Vec::new(),
        pending: Vec::new(),
    });
    let Reachability {
        visited, pending, ..
    } = reaches.last_mut().expect("just attached reachability");
    filled_into(visited, graph.successors.len(), false, budget)?;
    if avoided == Some(graph.entry) {
        return Ok(visited);
    }
    budget.reserve_vec(pending, graph.successors.len(), false)?;
    budget.extra(1)?;
    visited[graph.entry] = true;
    pending.push(graph.entry);
    while let Some(block) = pending.pop() {
        budget.charge(1)?;
        for successor in &graph.successors[block] {
            budget.charge(1)?;
            if !visited[*successor] && avoided != Some(*successor) {
                budget.extra(1)?;
                visited[*successor] = true;
                pending.push(*successor);
            }
        }
    }
    Ok(visited)
}

fn dominates_retained(
    graph: &SemanticCfgV1,
    reaches: &mut Vec<Reachability>,
    dominator: usize,
    block: usize,
    budget: &mut WorkBudgetV1,
) -> Result<bool, SemanticU32InductionAnalysisErrorV1> {
    if dominator >= graph.successors.len() || block >= graph.successors.len() {
        return Err(SemanticU32InductionAnalysisErrorV1::InvalidControlFlow(
            "a dominance query is outside the block table",
        ));
    }
    if !graph.is_in_scope(dominator) || !graph.is_in_scope(block) {
        return Ok(false);
    }
    if dominator == block {
        return Ok(true);
    }
    Ok(!reachable_retained(graph, Some(dominator), reaches, budget)?[block])
}

fn prove_candidate_retained(
    context: &CandidateProofContextV1<'_>,
    candidate: CandidateSiteV1,
    resolver: BoundResolverV1<'_>,
    budget: &mut WorkBudgetV1<'_>,
    reaches: &mut Vec<Reachability>,
) -> Result<Option<ProvedCandidateV1>, SemanticU32InductionAnalysisErrorV1> {
    // Fixed candidate bindings/comparisons; dynamic scans retain their existing
    // per-row charges. This extra charge is invisible to legacy paths/counters.
    budget.extra(256)?;
    let CandidateProofContextV1 {
        types,
        function,
        semantic_mir_sha256,
        function_id,
        graph,
        inventory,
    } = *context;
    let Some(candidate_block) = function.blocks().get(candidate.block) else {
        return Err(SemanticU32InductionAnalysisErrorV1::InvalidModel(
            "a checked-add candidate is outside the block table",
        ));
    };
    let Some(candidate_statement) = candidate_block.statements().get(candidate.statement) else {
        return Err(SemanticU32InductionAnalysisErrorV1::InvalidModel(
            "a checked-add candidate is outside its block statement table",
        ));
    };
    let SemanticStatementKindV1::Assign(checked_assignment) = candidate_statement.kind() else {
        return Ok(None);
    };
    let SemanticRvalueKindV1::CheckedBinary(checked) = checked_assignment.value().kind() else {
        return Ok(None);
    };
    if checked.operation() != SemanticCheckedBinaryOpV1::Add
        || !checked_assignment.destination().projections().is_empty()
        || checked_assignment.value().result_type() != checked_assignment.destination().ty()
    {
        return Ok(None);
    }
    let result_local = checked_assignment.destination().local();
    let result_ty = checked_assignment.destination().ty();
    let Some(result_decl) = function.locals().get(result_local.index() as usize) else {
        return Err(SemanticU32InductionAnalysisErrorV1::InvalidModel(
            "a checked-result local is outside the local table",
        ));
    };
    let Some(induction_place) = exact_operand_place(checked.left()) else {
        return Ok(None);
    };
    let induction = induction_place.local();
    let induction_ty = induction_place.ty();
    if !is_exact_u32(types, induction_ty)
        || !is_exact_u32_constant(checked.right(), induction_ty, 1)
        || !is_exact_checked_u32_tuple(types, result_ty, induction_ty)
        || checked.left().ty() != induction_ty
        || checked.right().ty() != induction_ty
        || result_decl.ty() != result_ty
        || result_decl.role() != SemanticLocalRoleV1::Temporary
    {
        return Ok(None);
    }

    let candidate_definition = DefinitionSiteV1 {
        block: candidate.block,
        position: DefinitionPositionV1::Statement(candidate.statement),
    };
    if !definition(inventory, result_local)?.is_unique_at(candidate_definition)
        || use_count(inventory, result_local)? != 2
        || local(inventory.address_or_projection_hazard.as_slice(), induction)?
    {
        return Ok(None);
    }
    let Some(induction_decl) = function.locals().get(induction.index() as usize) else {
        return Err(SemanticU32InductionAnalysisErrorV1::InvalidModel(
            "an induction local is outside the local table",
        ));
    };
    if induction_decl.ty() != induction_ty
        || induction_decl.role() != SemanticLocalRoleV1::Temporary
    {
        return Ok(None);
    }

    let SemanticTerminatorKindV1::Assert {
        condition,
        expected,
        message,
        target,
        unwind,
    } = candidate_block.terminator().kind()
    else {
        return Ok(None);
    };
    let SemanticAssertMessageV1::Overflow {
        operation,
        left: message_left,
        right: message_right,
    } = message
    else {
        return Ok(None);
    };
    if *expected
        || *operation != SemanticBinaryOpV1::Add
        || !same_operand_value(message_left, checked.left())
        || !same_operand_value(message_right, checked.right())
        || !field_operand_matches(
            condition,
            result_local,
            1,
            bool_type_of_checked_tuple(types, result_ty)?,
        )
        || target.role() != crate::semantic_mir_v1::SemanticEdgeRoleV1::AssertSuccess
        || !matches!(unwind, SemanticUnwindActionV1::Unreachable)
    {
        return Ok(None);
    }
    let update_block_index = target.target().index() as usize;
    if !graph.has_unique_predecessor(update_block_index, candidate.block) {
        return Ok(None);
    }
    let Some(update_block) = function.blocks().get(update_block_index) else {
        return Err(SemanticU32InductionAnalysisErrorV1::InvalidControlFlow(
            "an assertion-success edge is outside the block table",
        ));
    };
    let SemanticTerminatorKindV1::Goto(backedge) = update_block.terminator().kind() else {
        return Ok(None);
    };
    if backedge.role() != crate::semantic_mir_v1::SemanticEdgeRoleV1::Goto {
        return Ok(None);
    }
    let header_index = backedge.target().index() as usize;
    let Some(header) = function.blocks().get(header_index) else {
        return Err(SemanticU32InductionAnalysisErrorV1::InvalidControlFlow(
            "an induction backedge is outside the block table",
        ));
    };

    let mut update_site = None;
    for site in [
        definition(inventory, induction)?.first,
        definition(inventory, induction)?.second,
    ]
    .into_iter()
    .flatten()
    {
        budget.charge(1)?;
        if site.block != update_block_index {
            continue;
        }
        let DefinitionPositionV1::Statement(statement_index) = site.position else {
            return Ok(None);
        };
        let Some(statement) = update_block.statements().get(statement_index) else {
            return Err(SemanticU32InductionAnalysisErrorV1::InvalidModel(
                "an induction definition is outside its block statement table",
            ));
        };
        let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
            return Ok(None);
        };
        if !is_exact_destination(assignment.destination(), induction, induction_ty)
            || assignment.value().result_type() != induction_ty
            || !matches!(
                assignment.value().kind(),
                SemanticRvalueKindV1::Use(operand)
                    if field_operand_matches(operand, result_local, 0, induction_ty)
            )
            || update_site.replace(site).is_some()
        {
            return Ok(None);
        }
    }
    let Some(update_site) = update_site else {
        return Ok(None);
    };

    let mut guard_match = None;
    for (statement_index, statement) in header.statements().iter().enumerate() {
        budget.charge(1)?;
        let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
            continue;
        };
        let SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::LessThan,
            left,
            right,
        } = assignment.value().kind()
        else {
            continue;
        };
        let Some(left_place) = exact_operand_place(left) else {
            continue;
        };
        let guard_induction = if left_place.local() == induction && left_place.ty() == induction_ty
        {
            Some((induction, None))
        } else {
            match_guard_induction_snapshot_v1(
                types,
                function,
                header,
                header_index,
                statement_index,
                induction,
                induction_ty,
                left_place,
                inventory,
            )?
        };
        if let Some((guard_induction, guard_snapshot)) = guard_induction
            && guard_match
                .replace((
                    statement_index,
                    assignment,
                    right,
                    guard_induction,
                    guard_snapshot,
                ))
                .is_some()
        {
            return Ok(None);
        }
    }
    let Some((
        guard_statement,
        guard_assignment,
        bound_operand,
        guard_induction,
        guard_induction_snapshot,
    )) = guard_match
    else {
        return Ok(None);
    };
    let Some(bound_place) = exact_operand_place(bound_operand) else {
        return Ok(None);
    };
    let guard_bound = bound_place.local();
    let (bound, bound_snapshot) = match resolver {
        BoundResolverV1::Legacy => (guard_bound, None),
        BoundResolverV1::Snapshot(lifetimes) => {
            let Some(binding) = bound_snapshot::resolve(
                context,
                header_index,
                guard_statement,
                bound_place,
                lifetimes,
                budget,
            )?
            else {
                return Ok(None);
            };
            binding
        }
    };
    let bound_ty = bound_place.ty();
    let predicate = guard_assignment.destination().local();
    let predicate_ty = guard_assignment.destination().ty();
    let Some(predicate_decl) = function.locals().get(predicate.index() as usize) else {
        return Err(SemanticU32InductionAnalysisErrorV1::InvalidModel(
            "an induction predicate is outside the local table",
        ));
    };
    let guard_definition = DefinitionSiteV1 {
        block: header_index,
        position: DefinitionPositionV1::Statement(guard_statement),
    };
    let Some(bound_decl) = function.locals().get(bound.index() as usize) else {
        return Err(SemanticU32InductionAnalysisErrorV1::InvalidModel(
            "an induction bound is outside the local table",
        ));
    };
    if induction == bound
        || !is_exact_u32(types, bound_ty)
        || bound_ty != induction_ty
        || bound_decl.ty() != bound_ty
        || !bound_decl.role().is_entry_argument()
        || definition(inventory, bound)?.count != 0
        || local(inventory.address_or_projection_hazard.as_slice(), bound)?
        || (bound_snapshot.is_none() && local(inventory.direct_copy_alias.as_slice(), bound)?)
        || !guard_assignment.destination().projections().is_empty()
        || guard_assignment.value().result_type() != predicate_ty
        || !is_exact_bool(types, predicate_ty)
        || predicate_decl.ty() != predicate_ty
        || predicate_decl.role() != SemanticLocalRoleV1::Temporary
        || !definition(inventory, predicate)?.is_unique_at(guard_definition)
        || use_count(inventory, predicate)? != 1
    {
        return Ok(None);
    }

    let SemanticTerminatorKindV1::SwitchInt {
        discriminant,
        targets,
    } = header.terminator().kind()
    else {
        return Ok(None);
    };
    if !exact_operand_place(discriminant)
        .is_some_and(|place| place.local() == predicate && place.ty() == predicate_ty)
        || targets.values().len() != 1
        || targets.values()[0].value() != 0
        || targets.values()[0].edge().role()
            != crate::semantic_mir_v1::SemanticEdgeRoleV1::SwitchValue
        || targets.otherwise().role() != crate::semantic_mir_v1::SemanticEdgeRoleV1::SwitchOtherwise
    {
        return Ok(None);
    }
    let exit_index = targets.values()[0].edge().target().index() as usize;
    let body_entry_index = targets.otherwise().target().index() as usize;
    if exit_index == body_entry_index
        || !graph.has_unique_predecessor(body_entry_index, header_index)
        || !graph.has_unique_predecessor(exit_index, header_index)
        || !graph.has_unique_predecessor(update_block_index, candidate.block)
        || !dominates_retained(graph, reaches, body_entry_index, candidate.block, budget)?
        || !dominates_retained(graph, reaches, header_index, update_block_index, budget)?
    {
        return Ok(None);
    }

    let header_predecessors = graph.predecessors(header_index)?;
    if header_predecessors.len() != 2 || !header_predecessors.contains(&update_block_index) {
        return Ok(None);
    }
    let Some(preheader_index) = header_predecessors
        .iter()
        .copied()
        .find(|predecessor| *predecessor != update_block_index)
    else {
        return Ok(None);
    };
    let Some(preheader) = function.blocks().get(preheader_index) else {
        return Err(SemanticU32InductionAnalysisErrorV1::InvalidControlFlow(
            "an induction preheader is outside the block table",
        ));
    };
    if !matches!(
        preheader.terminator().kind(),
        SemanticTerminatorKindV1::Goto(edge)
            if edge.role() == crate::semantic_mir_v1::SemanticEdgeRoleV1::Goto
                && edge.target().index() as usize == header_index
    ) || !dominates_retained(graph, reaches, preheader_index, header_index, budget)?
        || dominates_retained(graph, reaches, header_index, preheader_index, budget)?
    {
        return Ok(None);
    }

    let mut initialization_site = None;
    for site in [
        definition(inventory, induction)?.first,
        definition(inventory, induction)?.second,
    ]
    .into_iter()
    .flatten()
    {
        budget.charge(1)?;
        if site.block != preheader_index {
            continue;
        }
        let DefinitionPositionV1::Statement(statement_index) = site.position else {
            return Ok(None);
        };
        let Some(statement) = preheader.statements().get(statement_index) else {
            return Err(SemanticU32InductionAnalysisErrorV1::InvalidModel(
                "an induction initialization is outside its block statement table",
            ));
        };
        let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
            return Ok(None);
        };
        if !is_exact_destination(assignment.destination(), induction, induction_ty)
            || assignment.value().result_type() != induction_ty
            || !matches!(
                assignment.value().kind(),
                SemanticRvalueKindV1::Use(operand)
                    if is_exact_u32_constant(operand, induction_ty, 0)
            )
            || initialization_site.replace(site).is_some()
        {
            return Ok(None);
        }
    }
    let Some(initialization_site) = initialization_site else {
        return Ok(None);
    };
    if !definition(inventory, induction)?.is_exact_pair(initialization_site, update_site) {
        return Ok(None);
    }

    let Some(exit) = function.blocks().get(exit_index) else {
        return Err(SemanticU32InductionAnalysisErrorV1::InvalidControlFlow(
            "an induction exit is outside the block table",
        ));
    };
    let Some(body_entry) = function.blocks().get(body_entry_index) else {
        return Err(SemanticU32InductionAnalysisErrorV1::InvalidControlFlow(
            "an induction body entry is outside the block table",
        ));
    };
    if let BoundResolverV1::Snapshot(lifetimes) = resolver {
        for snapshot in [
            guard_induction_snapshot.map(|site| (guard_induction, site)),
            bound_snapshot.map(|site| (guard_bound, site)),
        ]
        .into_iter()
        .flatten()
        {
            if !bound_snapshot::lifetime_matches(
                lifetimes,
                snapshot.0,
                snapshot.1,
                header_index,
                body_entry_index,
                exit_index,
                budget,
            )? {
                return Ok(None);
            }
        }
    }
    budget.extra(3 * std::mem::size_of::<SemanticU32InductionNoOverflowCertificateV1>())?;
    let certificate = SemanticU32InductionNoOverflowCertificateV1 {
        semantic_mir_sha256,
        function: function_id,
        function_identity: function.identity(),
        induction: place_binding(types, function, induction)?,
        guard_induction: place_binding(types, function, guard_induction)?,
        bound: place_binding(types, function, bound)?,
        predicate: place_binding(types, function, predicate)?,
        checked_result: place_binding(types, function, result_local)?,
        preheader: block_site(preheader_index, preheader)?,
        header: block_site(header_index, header)?,
        body_entry: block_site(body_entry_index, body_entry)?,
        exit: block_site(exit_index, exit)?,
        initialization: statement_site(preheader_index, preheader, initialization_site)?,
        guard_induction_snapshot: guard_induction_snapshot
            .map(|site| statement_site(header_index, header, site))
            .transpose()?,
        guard: statement_site(header_index, header, guard_definition)?,
        checked_addition: statement_site(candidate.block, candidate_block, candidate_definition)?,
        update: statement_site(update_block_index, update_block, update_site)?,
    };
    // Legacy reports retain exactly their old fields and metered work. Only the
    // separately typed new family can retain this additional bound relation.
    let (guard_bound, bound_snapshot) = match resolver {
        BoundResolverV1::Legacy => (certificate.bound, None),
        BoundResolverV1::Snapshot(_) => (
            place_binding(types, function, guard_bound)?,
            bound_snapshot
                .map(|site| statement_site(header_index, header, site))
                .transpose()?,
        ),
    };
    Ok(Some(ProvedCandidateV1 {
        certificate,
        guard_bound,
        bound_snapshot,
    }))
}

const RETAINED_FRAME_ROWS: usize = 33;
fn retained_frame_rows<M: SemanticU32InductionBoundSnapshotMeterV1>() -> [usize; RETAINED_FRAME_ROWS]
{
    [
        size_of::<RetainedSemanticU32InductionV1<'static, M::Error>>(), // owner
        size_of::<(
            Payload,
            Phase,
            Option<(&AdmittedInertSemanticMirV1, SemanticFunctionIdV1)>,
            Option<Saved<M::Error>>,
            usize,
        )>(), // constructor
        size_of::<(
            SemanticCfgV1,
            SemanticInventoryV1,
            Vec<Reachability>,
            Vec<SemanticU32InductionNoOverflowCertificateV1>,
            Option<SemanticU32InductionNoOverflowReportV1>,
        )>(), // payload constructor
        size_of::<(
            &mut RetainedSemanticU32InductionV1<'static, M::Error>,
            &AdmittedInertSemanticMirV1,
            SemanticFunctionIdV1,
            SemanticU32InductionAnalysisLimitsV1,
            &mut M,
            bool,
        )>(), // entry arguments
        size_of::<(
            Adapter<'_, M>,
            WorkLease<'static, 'static>,
            &mut WorkBudgetV1<'static>,
            &mut usize,
            &mut Option<Saved<M::Error>>,
        )>(), // adapter and guard
        size_of::<(
            &mut Payload,
            &mut Option<(&AdmittedInertSemanticMirV1, SemanticFunctionIdV1)>,
            &mut WorkBudgetV1<'static>,
            &AdmittedInertSemanticMirV1,
            SemanticFunctionIdV1,
            SemanticU32InductionAnalysisLimitsV1,
            usize,
        )>(), // entry split borrows/capture
        size_of::<(
            Option<&SemanticFunctionDeclV1>,
            &SemanticFunctionDeclV1,
            &[SemanticTypeDeclV1],
            InertSemanticMirSha256V1,
            Option<(&AdmittedInertSemanticMirV1, SemanticFunctionIdV1)>,
        )>(), // source loan/result
        size_of::<(
            AnalysisResult<()>,
            AnalysisResult<()>,
            Result<(), SemanticU32InductionRetainedFailureV1>,
            Result<(), SemanticU32InductionRetainedFailureV1>,
            SemanticU32InductionRetainedFailureV1,
            Saved<M::Error>,
            M::Error,
            Option<Saved<M::Error>>,
        )>(), // entry outcomes/errors
        size_of::<(
            &mut Adapter<'_, M>,
            usize,
            bool,
            Result<(), M::Error>,
            AnalysisResult<()>,
            M::Error,
            &mut M,
            &mut Option<Saved<M::Error>>,
        )>(), // meter forwarding
        size_of::<(&mut WorkLease<'static, 'static>, &mut usize, usize)>(), // work guard drop
        size_of::<(
            &RetainedSemanticU32InductionV1<'static, M::Error>,
            &AdmittedInertSemanticMirV1,
            SemanticFunctionIdV1,
            Option<(&AdmittedInertSemanticMirV1, SemanticFunctionIdV1)>,
            Result<&SemanticU32InductionNoOverflowReportV1, SemanticU32InductionRetainedFailureV1>,
            Option<&Saved<M::Error>>,
            Option<&SemanticU32InductionNoOverflowReportV1>,
            (&AdmittedInertSemanticMirV1, SemanticFunctionIdV1),
            bool,
            usize,
        )>(), // completed/error/work getters
        size_of::<(
            &mut Payload,
            &[SemanticTypeDeclV1],
            &SemanticFunctionDeclV1,
            InertSemanticMirSha256V1,
            SemanticFunctionIdV1,
            SemanticU32InductionAnalysisLimitsV1,
            &mut WorkBudgetV1<'static>,
            AnalysisResult<()>,
        )>(), // payload entry
        size_of::<(
            &SemanticFunctionDeclV1,
            &mut SemanticCfgV1,
            &mut Vec<Reachability>,
            &mut WorkBudgetV1<'static>,
            AnalysisResult<()>,
            &mut Vec<Vec<usize>>,
            &mut Vec<Vec<usize>>,
        )>(), // graph invocation
        size_of::<(
            &mut Vec<Vec<usize>>,
            &mut Vec<Vec<usize>>,
            &mut WorkBudgetV1<'static>,
            usize,
            usize,
        )>(), // edge closure captures
        size_of::<(
            SemanticControlFlowEdgeV1,
            SemanticControlFlowEdgeV1,
            SemanticBlockIdV1,
            SemanticBlockIdV1,
            u32,
            usize,
            AnalysisResult<()>,
        )>(), // edge by-value vertices
        size_of::<(
            &SemanticCfgV1,
            &[bool],
            &Vec<bool>,
            std::slice::Iter<'static, bool>,
            &bool,
            usize,
            usize,
            &SemanticBasicBlockV1,
            bool,
        )>(), // graph/read traversal aliases
        size_of::<(
            &SemanticFunctionDeclV1,
            &SemanticCfgV1,
            &mut SemanticInventoryV1,
            &mut WorkBudgetV1<'static>,
            &mut Vec<DefinitionSummaryV1>,
            &mut Vec<bool>,
            &mut Vec<bool>,
            &mut Vec<usize>,
            &mut Vec<CandidateSiteV1>,
            AnalysisResult<()>,
        )>(), // inventory invocation and fields
        size_of::<(
            &mut [DefinitionSummaryV1],
            &mut [bool],
            &mut [bool],
            &mut [usize],
            &[CandidateSiteV1],
            &Vec<CandidateSiteV1>,
        )>(), // inventory coerced slices
        size_of::<(
            &CandidateProofContextV1<'static>,
            CandidateSiteV1,
            BoundResolverV1<'static>,
            &mut WorkBudgetV1<'static>,
            &mut Vec<Reachability>,
            AnalysisResult<Option<ProvedCandidateV1>>,
            AnalysisResult<Option<ProvedCandidateV1>>,
            ProvedCandidateV1,
            SemanticU32InductionNoOverflowCertificateV1,
        )>(), // candidate retained bridge
        size_of::<(
            CandidateProofContextV1<'static>,
            std::slice::Iter<'static, CandidateSiteV1>,
            &CandidateSiteV1,
            usize,
            usize,
            &SemanticInventoryV1,
            &SemanticCfgV1,
            &mut Vec<SemanticU32InductionNoOverflowCertificateV1>,
        )>(), // candidate iteration
        size_of::<(
            &SemanticCfgV1,
            &mut Vec<Reachability>,
            usize,
            usize,
            &mut WorkBudgetV1<'static>,
            AnalysisResult<bool>,
            AnalysisResult<&[bool]>,
            &[bool],
            bool,
        )>(), // dominance retained bridge
        size_of::<(
            &SemanticCfgV1,
            Option<usize>,
            &mut Vec<Reachability>,
            &mut WorkBudgetV1<'static>,
            AnalysisResult<&[bool]>,
            Reachability,
            &mut Reachability,
            &mut Vec<bool>,
            &mut Vec<usize>,
        )>(), // reachability entry/capture
        size_of::<(
            Option<usize>,
            usize,
            std::slice::Iter<'static, usize>,
            &usize,
            &Vec<usize>,
            &[usize],
            bool,
            AnalysisResult<()>,
        )>(), // reachability walk
        size_of::<(
            &mut Vec<DefinitionSummaryV1>,
            usize,
            DefinitionSummaryV1,
            &mut WorkBudgetV1<'static>,
            AnalysisResult<()>,
            &mut [DefinitionSummaryV1],
        )>(), // filled definition instantiation
        size_of::<(
            &mut Vec<bool>,
            usize,
            bool,
            &mut WorkBudgetV1<'static>,
            AnalysisResult<()>,
            &mut [bool],
        )>(), // filled bool instantiation
        size_of::<(
            &mut Vec<usize>,
            usize,
            usize,
            &mut WorkBudgetV1<'static>,
            AnalysisResult<()>,
            &mut [usize],
        )>(), // filled usize instantiation
        size_of::<(
            &mut Vec<Vec<usize>>,
            usize,
            &mut WorkBudgetV1<'static>,
            AnalysisResult<()>,
            Vec<usize>,
            &mut [Vec<usize>],
        )>(), // nested usize instantiation
        size_of::<(
            &mut Vec<Reachability>,
            usize,
            &mut WorkBudgetV1<'static>,
            AnalysisResult<()>,
            Reachability,
            Option<&mut Reachability>,
            &mut Reachability,
        )>(), // retained reach header allocation
        size_of::<(
            &Vec<SemanticU32InductionNoOverflowCertificateV1>,
            &mut WorkBudgetV1<'static>,
            usize,
            usize,
            Option<usize>,
            AnalysisResult<()>,
            bool,
        )>(), // retained final admission
        size_of::<(
            Vec<SemanticU32InductionNoOverflowCertificateV1>,
            Box<[SemanticU32InductionNoOverflowCertificateV1]>,
            SemanticU32InductionNoOverflowReportV1,
            Option<SemanticU32InductionNoOverflowReportV1>,
            &mut Vec<SemanticU32InductionNoOverflowCertificateV1>,
            usize,
            SemanticFunctionIdentityV1,
        )>(), // report ownership commit
        size_of::<(usize, usize, Option<usize>, Option<usize>, &usize)>(), // frame arithmetic/results
        size_of::<(
            [usize; RETAINED_FRAME_ROWS],
            [usize; RETAINED_FRAME_ROWS],
            std::array::IntoIter<usize, RETAINED_FRAME_ROWS>,
            Option<usize>,
            usize,
            usize,
        )>(), // retained frame array/result/fold
        size_of::<(
            &mut WorkBudgetV1<'static>,
            &mut Vec<Reachability>,
            usize,
            usize,
            usize,
            usize,
            Option<usize>,
            Result<(), std::collections::TryReserveError>,
            std::collections::TryReserveError,
            AnalysisResult<()>,
            bool,
        )>(), // new Reachability strict-reserve instantiation
    ]
}
fn retained_frame<M: SemanticU32InductionBoundSnapshotMeterV1>() -> Option<usize> {
    // The unchanged strict donor frame remains separate, never residual funding.
    let original = strict_resources::frame_storage_v1::<M>()?;
    retained_frame_rows::<M>()
        .into_iter()
        .try_fold(original, usize::checked_add)
}

#[cfg(test)]
#[path = "semantic_u32_induction_retained_v1_tests.rs"]
mod tests;
