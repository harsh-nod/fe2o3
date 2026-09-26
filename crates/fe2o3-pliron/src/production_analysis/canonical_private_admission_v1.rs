//! Closed private graph evidence, derived only from the checked inventory.
//! The census is eligibility evidence until every original candidate is joined.
use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirCallEffectDecisionV1 as CallDecision, CanonicalKirCallEffectErrorV1 as CallError,
    CanonicalKirCallEffectsV1 as CallEffects, CanonicalKirInventoryV1 as Inventory,
    CanonicalKirPrivateCellCensusErrorV1 as CellError,
    CanonicalKirPrivateCellCensusV1 as CellCensus,
    CheckedCanonicalKirPrivateMemoryV1 as PhysicalMemory,
};
use fe2o3_kernel_ir::{
    AddressSpace, CanonicalKirOperationCoordinateV1 as Coordinate,
    KirLocalMemoryEffectRefV1 as Effect, OperationKind as Kind, Terminator, Type,
};

#[path = "canonical_private_admission_resources_v1.rs"]
mod private_resources;

/// This enumeration is diagnostic only, never permission to extend a reader.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalPrivateRequirementV1 {
    CompleteCells,
    PrivateEffects,
    ScalarInterface,
    ClosedLanguage,
    DefinedAcyclicCalls,
    NativeJoin,
    StageCoverage,
    CalleeCompletion,
    TerminalPairs,
}

use crate::kir_bridge_v1::canonical_ranked_v1::private_profile::NativeCanonicalPrivateProjectionV1;
use crate::production_analysis::pliron_pipeline::canonical_private_v1::{
    CanonicalPrivatePipelineOutcomeV1, CanonicalPrivatePipelineReportV1,
};

struct PrivateReportRowV1 {
    outcome: CanonicalPrivatePipelineOutcomeV1,
    history: CanonicalRankedPolicyHistoryV1,
}

/// Paired diagnostic proof over one exact original neutral module. Neither raw
/// native endpoints nor an independently constructible permission are exposed.
///
/// ```compile_fail
/// use fe2o3_pliron::CheckedCanonicalPrivatePoliciesV1;
/// fn forge() { let _ = CheckedCanonicalPrivatePoliciesV1 {}; }
/// ```
/// ```compile_fail
/// use fe2o3_pliron::CheckedCanonicalPrivatePoliciesV1;
/// fn detach(x: &CheckedCanonicalPrivatePoliciesV1<'_, '_>) { let _ = (*x).clone(); }
/// ```
pub struct CheckedCanonicalPrivatePoliciesV1<'s, 'g> {
    owner: &'g Owner,
    reports: &'s [PrivateReportRowV1],
    guard: &'s Guard,
    observation: CanonicalRankedPolicyResourceObservationV1,
}

impl<'g> CheckedCanonicalPrivatePoliciesV1<'_, 'g> {
    pub(super) fn invalid_query(&self, ordinal: usize) -> Failure {
        self.guard.invalid(ordinal)
    }
    pub fn owner(&self, budget: &mut Budget<'_>) -> Result<&'g Owner, Failure> {
        self.guard.query(budget)?;
        Ok(self.owner)
    }
    pub fn function_count(&self, budget: &mut Budget<'_>) -> Result<usize, Failure> {
        self.guard.query(budget)?;
        Ok(self.reports.len())
    }
    pub fn report(
        &self,
        function: usize,
        budget: &mut Budget<'_>,
    ) -> Result<&CanonicalPrivatePipelineReportV1, Failure> {
        self.guard.query(budget)?;
        self.reports
            .get(function)
            .map(|row| &row.outcome.report)
            .ok_or_else(|| self.guard.invalid(function))
    }
    pub fn history(
        &self,
        function: usize,
        budget: &mut Budget<'_>,
    ) -> Result<CanonicalRankedPolicyHistoryV1, Failure> {
        self.guard.query(budget)?;
        self.reports
            .get(function)
            .map(|row| row.history)
            .ok_or_else(|| self.guard.invalid(function))
    }
    pub fn observation(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<CanonicalRankedPolicyResourceObservationV1, Failure> {
        self.guard.query(budget)?;
        Ok(self.observation)
    }
    /// All full-compiler obligations remain pending, including source equivalence.
    pub const fn pending_obligations(&self) -> Obligations {
        Obligations::NONE
            .with(Obligation::ExactScalarSemantics)
            .with(Obligation::Control)
            .with(Obligation::Bounds)
            .with(Obligation::Provenance)
            .with(Obligation::Initialization)
            .with(Obligation::RaceFreedom)
            .with(Obligation::Lifetime)
            .with(Obligation::Ordering)
            .with(Obligation::Convergence)
            .with(Obligation::TrapBehavior)
            .with(Obligation::CallEffects)
            .with(Obligation::CallControl)
            .with(Obligation::Launch)
            .with(Obligation::Target)
            .with(Obligation::Tensor)
            .with(Obligation::Assembly)
            .with(Obligation::Contract)
            .with(Obligation::ReferenceRefinement)
            .with(Obligation::SourceMetadata)
    }
    pub const fn ranked_verification_is_complete(&self) -> bool {
        false
    }
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

/// Derives a complete private-only graph/call proof and invokes all nine actual
/// policies for every definition. Source facts must be independently paired by
/// the lowerer; no full-compiler obligation or operational authority is granted.
/// Rejected callback values and panic payloads are destroyed before refund.
pub fn with_canonical_private_policy_checks_v1<'w, T>(
    checked: &mut CheckedCanonicalRankedViewV1<'_, '_, '_, '_>,
    budget: &mut Budget<'w>,
    callback: impl for<'s, 'g> FnOnce(
        &CheckedCanonicalPrivatePoliciesV1<'s, 'g>,
        &mut Budget<'w>,
    ) -> Result<T, Failure>,
) -> Result<T, CanonicalRankedPolicyChecksErrorV1> {
    with_private_checks(checked, budget, Limits::production_hard_ceiling(), callback)
}

fn with_private_checks<'w, T>(
    checked: &mut CheckedCanonicalRankedViewV1<'_, '_, '_, '_>,
    budget: &mut Budget<'w>,
    limits: Limits,
    callback: impl for<'s, 'g> FnOnce(
        &CheckedCanonicalPrivatePoliciesV1<'s, 'g>,
        &mut Budget<'w>,
    ) -> Result<T, Failure>,
) -> Result<T, CanonicalRankedPolicyChecksErrorV1> {
    let mut analysis = private_resources::PrivateAnalysisV1::new(limits);
    let result = protected(budget, |budget| {
        // This exact-floor foundation query must precede nested reservations.
        let inventory = checked.inventory(budget)?;
        let owner = inventory.owner();
        budget.reserve_storage(checked_add(
            size_of::<private_resources::PrivateAnalysisV1>(),
            checked_add(
                size_of::<CheckedCanonicalPrivatePoliciesV1<'_, '_>>(),
                checked_add(
                    size_of::<Guard>(),
                    checked_add(
                        size_of::<std::thread::Result<Result<T, Failure>>>(),
                        drain_header(),
                    )?,
                )?,
            )?,
        )?)?;
        let facts = CanonicalPrivateGraphFactsV1::derive(inventory, budget)?;
        let mut projection = NativeCanonicalPrivateProjectionV1::import(&facts, budget)?;
        let mut reports = reserve_rows::<PrivateReportRowV1>(inventory.functions().len(), budget)?;
        for ordinal in 0..inventory.functions().len() {
            let outcome =
                projection.with_function(ordinal, budget, |input| analysis.invoke(input))??;
            let history = analysis.last.ok_or(Failure::InvocationAccounting)?;
            reports.push(PrivateReportRowV1 { outcome, history });
            projection.check_epoch()?;
        }
        facts.require_completed(reports.len(), budget)?;
        projection.check(budget)?;
        let guard = Guard::new(budget);
        let view = CheckedCanonicalPrivatePoliciesV1 {
            owner,
            reports: &reports,
            guard: &guard,
            observation: analysis.observation(),
        };
        let result = guard.callback(budget, |budget| callback(&view, budget));
        if let Err(error) = projection.check_epoch() {
            drop(result);
            return Err(error);
        }
        drop(projection);
        drop(reports);
        drop(facts);
        analysis.release_reports()?;
        result
    });
    result.map_err(|failure| CanonicalRankedPolicyChecksErrorV1 {
        failure,
        observation: analysis.observation(),
        last_invocation: analysis.last,
    })
}

pub(crate) fn refuse(
    need: CanonicalPrivateRequirementV1,
    coordinate: Option<Coordinate>,
) -> Failure {
    Failure::PrivateRequirement {
        requirement: need,
        coordinate,
    }
}

pub(super) fn with_trap_checks<'w, T>(
    checked: &mut CheckedCanonicalRankedViewV1<'_, '_, '_, '_>,
    budget: &mut Budget<'w>,
    limits: Limits,
    callback: impl for<'s, 'g> FnOnce(
        &traps::CheckedCanonicalTrapPoliciesV1<'s, 'g>,
        &mut Budget<'w>,
    ) -> Result<T, Failure>,
) -> Result<T, CanonicalRankedPolicyChecksErrorV1> {
    let mut analysis = private_resources::PrivateAnalysisV1::new(limits);
    let result = protected(budget, |budget| {
        let inventory = checked.inventory(budget)?;
        let owner = inventory.owner();
        budget.reserve_storage(checked_add(
            size_of::<private_resources::PrivateAnalysisV1>(),
            size_of::<Guard>(),
        )?)?;
        traps::reserve_header::<T>(budget)?;
        let terminals = traps::CanonicalTrapPairsGraphFactsV1::derive(inventory, budget)?;
        let facts =
            CanonicalPrivateGraphFactsV1::derive_with_terminals(inventory, &terminals, budget)?;
        let mut projection = NativeCanonicalPrivateProjectionV1::import(&facts, budget)?;
        let mut reports =
            reserve_rows::<PrivateReportRowV1>(terminals.definitions().len(), budget)?;
        for &coordinate in terminals.definitions() {
            budget.charge_work(1)?;
            let ordinal = coordinate.0 as usize;
            let outcome =
                projection.with_function(ordinal, budget, |input| analysis.invoke(input))??;
            let history = analysis.last.ok_or(Failure::InvocationAccounting)?;
            if history.function() != ordinal {
                return Err(Failure::InvocationAccounting);
            }
            reports.push(PrivateReportRowV1 { outcome, history });
            projection.check_epoch()?;
        }
        facts.require_completed(reports.len(), budget)?;
        projection.check(budget)?;
        let guard = Guard::new(budget);
        let inner = CheckedCanonicalPrivatePoliciesV1 {
            owner,
            reports: &reports,
            guard: &guard,
            observation: analysis.observation(),
        };
        let view = traps::CheckedCanonicalTrapPoliciesV1::new(inner, &terminals);
        let result = guard.callback(budget, |budget| callback(&view, budget));
        if let Err(error) = projection.check_epoch() {
            drop(result);
            return Err(error);
        }
        drop(projection);
        drop(reports);
        drop(facts);
        drop(terminals);
        analysis.release_reports()?;
        result
    });
    result.map_err(|failure| CanonicalRankedPolicyChecksErrorV1 {
        failure,
        observation: analysis.observation(),
        last_invocation: analysis.last,
    })
}

pub(crate) fn scalar(ty: &Type) -> bool {
    matches!(ty, Type::Scalar(_) | Type::Unit)
}

enum PrivateMemoryEvidenceV1<'i, 'g> {
    Census(CellCensus<'i, 'g>),
    Physical(PhysicalMemory<'i, 'g>),
}
impl<'i, 'g> PrivateMemoryEvidenceV1<'i, 'g> {
    fn belongs_to(&self, inventory: &Inventory<'_>) -> bool {
        match self {
            Self::Census(cells) => cells.belongs_to(inventory),
            Self::Physical(proof) => proof.is_for(inventory),
        }
    }
    #[cfg(test)]
    fn census(&self) -> Option<&CellCensus<'i, 'g>> {
        match self {
            Self::Census(cells) => Some(cells),
            Self::Physical(_) => None,
        }
    }
}

pub(crate) struct CanonicalPrivateGraphFactsV1<'i, 'g> {
    inventory: &'i Inventory<'g>,
    cells: PrivateMemoryEvidenceV1<'i, 'g>,
    call_effects: CallEffects<'i, 'g>,
    completion_order: Vec<usize>,
    terminals: Option<&'i traps::CanonicalTrapPairsGraphFactsV1<'i, 'g>>,
}

impl<'i, 'g> CanonicalPrivateGraphFactsV1<'i, 'g> {
    fn derive(inventory: &'i Inventory<'g>, budget: &mut Budget<'_>) -> Result<Self, Failure> {
        Self::derive_profile(inventory, None, budget)
    }

    pub(super) fn derive_with_terminals(
        inventory: &'i Inventory<'g>,
        terminals: &'i traps::CanonicalTrapPairsGraphFactsV1<'i, 'g>,
        budget: &mut Budget<'_>,
    ) -> Result<Self, Failure> {
        Self::derive_profile(inventory, Some(terminals), budget)
    }

    fn derive_profile(
        inventory: &'i Inventory<'g>,
        terminals: Option<&'i traps::CanonicalTrapPairsGraphFactsV1<'i, 'g>>,
        budget: &mut Budget<'_>,
    ) -> Result<Self, Failure> {
        if terminals.is_some_and(|facts| !facts.belongs_to(inventory)) {
            return Err(Failure::ExactGraph);
        }
        budget.reserve_storage(size_of::<Self>())?;
        let (cells, receipt) = CellCensus::derive(inventory, Default::default(), budget).map_err(
            |error| match error {
                CellError::Resource(resource) => Failure::Resource(resource),
                CellError::Panicked => Failure::Panicked,
                _ => refuse(CanonicalPrivateRequirementV1::CompleteCells, None),
            },
        )?;
        budget.reserve_storage(receipt.retained_storage())?;
        Self::finish_profile(
            inventory,
            PrivateMemoryEvidenceV1::Census(cells),
            terminals,
            budget,
        )
    }

    fn finish_profile(
        inventory: &'i Inventory<'g>,
        cells: PrivateMemoryEvidenceV1<'i, 'g>,
        terminals: Option<&'i traps::CanonicalTrapPairsGraphFactsV1<'i, 'g>>,
        budget: &mut Budget<'_>,
    ) -> Result<Self, Failure> {
        let (call_effects, receipt) = CallEffects::derive(inventory, budget).map_err(call_error)?;
        budget.reserve_storage(receipt.retained_storage())?;
        let mut facts = Self {
            inventory,
            cells,
            call_effects,
            completion_order: reserve_rows(inventory.functions().len(), budget)?,
            terminals,
        };
        facts.check_language(budget)?;
        facts.close_calls(budget)?;
        Ok(facts)
    }

    pub(crate) fn inventory(&self) -> &'i Inventory<'g> {
        self.inventory
    }

    pub(crate) fn memory_belongs_to(&self, inventory: &Inventory<'_>) -> bool {
        self.cells.belongs_to(inventory)
    }

    pub(crate) fn has_terminal_pairs(&self) -> bool {
        self.terminals.is_some()
    }

    pub(crate) fn terminal_facts(&self) -> Option<&traps::CanonicalTrapPairsGraphFactsV1<'i, 'g>> {
        self.terminals
    }

    pub(crate) fn charge_terminal_lookups(
        &self,
        count: usize,
        budget: &mut Budget<'_>,
    ) -> Result<(), Failure> {
        if let Some(terminals) = self.terminals {
            budget.charge_work(
                count
                    .checked_mul(terminals.pair_count())
                    .ok_or(Resource::Arithmetic)?,
            )?;
        }
        Ok(())
    }

    pub(crate) fn terminator_kind(
        &self,
        coordinate: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1,
    ) -> PrivateOperationKindV1 {
        if self.terminals.is_some_and(|facts| facts.is_end(coordinate)) {
            PrivateOperationKindV1::TrapEnd
        } else {
            PrivateOperationKindV1::Scalar
        }
    }

    pub(crate) fn operation_kind(&self, ordinal: usize) -> Result<PrivateOperationKindV1, Failure> {
        let operation = self
            .inventory
            .operations()
            .get(ordinal)
            .ok_or_else(|| refuse(CanonicalPrivateRequirementV1::NativeJoin, None))?;
        Ok(match operation.operation.kind {
            Kind::Alloca { .. } => PrivateOperationKindV1::Allocate,
            Kind::GetElementPointer { .. } => PrivateOperationKindV1::Address,
            Kind::Load { .. } => PrivateOperationKindV1::Read,
            Kind::Store { .. } => PrivateOperationKindV1::Write,
            Kind::Call { .. }
                if self
                    .terminals
                    .is_some_and(|facts| facts.is_call(operation.coordinate)) =>
            {
                PrivateOperationKindV1::TrapCall
            }
            Kind::Call { .. } => PrivateOperationKindV1::Call,
            _ => PrivateOperationKindV1::Scalar,
        })
    }

    fn check_language(&self, budget: &mut Budget<'_>) -> Result<(), Failure> {
        use CanonicalPrivateRequirementV1 as Need;
        if !self.cells.belongs_to(self.inventory) || !self.call_effects.belongs_to(self.inventory) {
            return Err(refuse(Need::CompleteCells, None));
        }
        for function in self.inventory.functions() {
            budget.charge_work(1)?;
            if function.function.body.is_none() {
                if self
                    .terminals
                    .is_some_and(|facts| facts.is_declaration(function.coordinate))
                {
                    continue;
                }
                return Err(refuse(Need::DefinedAcyclicCalls, None));
            }
            for ty in function
                .function
                .signature
                .parameters
                .iter()
                .chain(&function.function.signature.results)
            {
                budget.charge_work(1)?;
                if !scalar(ty) {
                    return Err(refuse(Need::ScalarInterface, None));
                }
            }
        }
        for block in self.inventory.blocks() {
            budget.charge_work(1)?;
            self.charge_terminal_lookups(1, budget)?;
            if !matches!(
                block.terminator,
                Terminator::Branch { .. }
                    | Terminator::ConditionalBranch { .. }
                    | Terminator::Switch { .. }
                    | Terminator::IntegerSwitch { .. }
                    | Terminator::Return { .. }
            ) && !(matches!(block.terminator, Terminator::Unreachable)
                && self
                    .terminals
                    .is_some_and(|facts| facts.is_end(block.coordinate)))
            {
                return Err(refuse(Need::ClosedLanguage, None));
            }
            if let Terminator::Switch {
                selector, cases, ..
            } = block.terminator
            {
                budget.charge_work(1)?;
                let function = self
                    .inventory
                    .functions()
                    .get(block.coordinate.function.0 as usize)
                    .ok_or_else(|| refuse(Need::NativeJoin, None))?;
                if !crate::kir_bridge_v1::source_legacy_representable(
                    function.function,
                    *selector,
                    cases,
                    budget,
                )? {
                    return Err(Failure::UnsupportedGraph {
                        function: block.coordinate.function.0 as usize,
                        block: Some(block.coordinate.block as usize),
                        operation: None,
                    });
                }
            }
            for parameter in &block.block.parameters {
                budget.charge_work(1)?;
                if !scalar(&parameter.ty) {
                    return Err(refuse(Need::ScalarInterface, None));
                }
            }
        }
        let mut counts = [0usize; 4];
        for (ordinal, row) in self.inventory.operations().iter().enumerate() {
            budget.charge_work(2)?;
            let fail = |need| refuse(need, Some(row.coordinate));
            if !row.compiler_ordering().is_empty() {
                return Err(fail(Need::PrivateEffects));
            }
            match &row.operation.kind {
                Kind::Alloca { .. } => {
                    match &self.cells {
                        PrivateMemoryEvidenceV1::Census(cells) => {
                            budget.charge_work(cells.allocations().len())?;
                            if cells
                                .allocations()
                                .iter()
                                .filter(|cell| cell.allocation == row.coordinate)
                                .count()
                                != 1
                            {
                                return Err(fail(Need::CompleteCells));
                            }
                        }
                        PrivateMemoryEvidenceV1::Physical(proof) => {
                            budget.charge_work(1)?;
                            if !proof.operation(ordinal) {
                                return Err(fail(Need::CompleteCells));
                            }
                        }
                    }
                    counts[0] = checked_add(counts[0], 1)?;
                }
                Kind::GetElementPointer { .. } => {
                    match &self.cells {
                        PrivateMemoryEvidenceV1::Census(cells) => {
                            budget.charge_work(cells.addresses().len())?;
                            if cells
                                .addresses()
                                .iter()
                                .filter(|address| {
                                    address.producer == row.coordinate
                                        && address.producer != address.allocation
                                })
                                .count()
                                != 1
                            {
                                return Err(fail(Need::CompleteCells));
                            }
                        }
                        PrivateMemoryEvidenceV1::Physical(proof) => {
                            budget.charge_work(1)?;
                            if !proof.operation(ordinal) {
                                return Err(fail(Need::CompleteCells));
                            }
                        }
                    }
                    counts[1] = checked_add(counts[1], 1)?;
                }
                Kind::Load { .. } | Kind::Store { .. } => {
                    match &self.cells {
                        PrivateMemoryEvidenceV1::Census(cells) => {
                            budget.charge_work(cells.accesses().len())?;
                            if cells
                                .accesses()
                                .iter()
                                .filter(|access| access.operation == row.coordinate)
                                .count()
                                != 1
                            {
                                return Err(fail(Need::CompleteCells));
                            }
                        }
                        PrivateMemoryEvidenceV1::Physical(proof) => {
                            budget.charge_work(1)?;
                            if !proof.operation(ordinal) {
                                return Err(fail(Need::CompleteCells));
                            }
                        }
                    }
                    counts[2] = checked_add(counts[2], 1)?;
                }
                Kind::Call { .. } => {
                    budget.charge_work(self.inventory.calls().len())?;
                    if self
                        .inventory
                        .calls()
                        .iter()
                        .filter(|call| call.coordinate == row.coordinate)
                        .count()
                        != 1
                    {
                        return Err(fail(Need::DefinedAcyclicCalls));
                    }
                    counts[3] = checked_add(counts[3], 1)?;
                }
                Kind::Constant(_)
                | Kind::Unary { .. }
                | Kind::Binary { .. }
                | Kind::Compare { .. }
                | Kind::Cast { .. }
                | Kind::Select { .. } => {
                    if !row.effects.is_empty() {
                        return Err(fail(Need::PrivateEffects));
                    }
                    for result in &row.operation.results {
                        budget.charge_work(1)?;
                        if !scalar(&result.ty) {
                            return Err(fail(Need::ScalarInterface));
                        }
                    }
                    for operand in &self.inventory.uses()[row.operands.clone()] {
                        budget.charge_work(1)?;
                        if !scalar(self.inventory.definitions()[operand.definition].ty) {
                            return Err(fail(Need::ScalarInterface));
                        }
                    }
                }
                _ => return Err(fail(Need::ClosedLanguage)),
            }
            for effect in &self.inventory.effects()[row.effects.clone()] {
                budget.charge_work(1)?;
                if !matches!(
                    effect.effect,
                    Effect::Allocate(AddressSpace::Private)
                        | Effect::Read(AddressSpace::Private)
                        | Effect::Write(AddressSpace::Private)
                ) {
                    return Err(fail(Need::PrivateEffects));
                }
            }
        }
        if let PrivateMemoryEvidenceV1::Census(cells) = &self.cells {
            if counts[0] != cells.allocations().len()
                || checked_add(counts[0], counts[1])? != cells.addresses().len()
                || counts[2] != cells.accesses().len()
            {
                return Err(refuse(Need::CompleteCells, None));
            }
        }
        if counts[3] != self.inventory.calls().len() {
            return Err(refuse(Need::CompleteCells, None));
        }
        Ok(())
    }

    fn close_calls(&mut self, budget: &mut Budget<'_>) -> Result<(), Failure> {
        use CanonicalPrivateRequirementV1 as Need;
        let count = self.inventory.functions().len();
        let mut complete = reserve_rows::<bool>(count, budget)?;
        budget.charge_work(count)?;
        complete.resize(count, false);
        for call in self.inventory.calls() {
            budget.charge_work(2)?;
            let target = call
                .target
                .and_then(|target| self.inventory.functions().get(target.0 as usize))
                .ok_or_else(|| refuse(Need::DefinedAcyclicCalls, Some(call.coordinate)))?;
            budget.charge_work(checked_add(
                call.callee.len(),
                target.function.id.as_str().len(),
            )?)?;
            self.charge_terminal_lookups(1, budget)?;
            let terminal = self.terminals.is_some_and(|facts| {
                facts.is_call(call.coordinate) && facts.is_declaration(target.coordinate)
            });
            if (target.function.body.is_none() && !terminal)
                || call.callee != target.function.id.as_str()
            {
                return Err(refuse(Need::DefinedAcyclicCalls, Some(call.coordinate)));
            }
            budget.charge_work(self.inventory.operations().len())?;
            let row = self
                .inventory
                .operations()
                .iter()
                .find(|row| row.coordinate == call.coordinate);
            // Lookup cost is charged before inspecting the original roster.
            // The exact operation pointer is also checked, not just its ordinal.
            let row =
                row.ok_or_else(|| refuse(Need::DefinedAcyclicCalls, Some(call.coordinate)))?;
            if !std::ptr::eq(row.operation, call.operation) {
                return Err(refuse(Need::DefinedAcyclicCalls, Some(call.coordinate)));
            }
            for definition in row
                .results
                .clone()
                .map(|i| &self.inventory.definitions()[i])
                .chain(
                    self.inventory.uses()[row.operands.clone()]
                        .iter()
                        .map(|operand| &self.inventory.definitions()[operand.definition]),
                )
            {
                budget.charge_work(1)?;
                if !scalar(definition.ty) {
                    return Err(refuse(Need::ScalarInterface, Some(call.coordinate)));
                }
            }
        }
        // No recursion and no dependence on the effect reader's optional call
        // summary shortcut. Remove actual leaves until every definition is done.
        while self.completion_order.len() < count {
            let before = self.completion_order.len();
            for function in self.inventory.functions() {
                budget.charge_work(1)?;
                let ordinal = function.coordinate.0 as usize;
                if complete[ordinal] {
                    continue;
                }
                let mut ready = true;
                for call in &self.inventory.calls()[function.calls.clone()] {
                    budget.charge_work(1)?;
                    let target = call
                        .target
                        .ok_or_else(|| refuse(Need::DefinedAcyclicCalls, Some(call.coordinate)))?;
                    ready &= complete[target.0 as usize];
                }
                if ready {
                    complete[ordinal] = true;
                    self.completion_order.push(ordinal);
                }
            }
            if before == self.completion_order.len() {
                return Err(refuse(Need::DefinedAcyclicCalls, None));
            }
        }
        for function in self.inventory.functions() {
            if self
                .terminals
                .is_some_and(|facts| facts.is_declaration(function.coordinate))
            {
                budget.charge_work(1)?;
                continue;
            }
            if self
                .call_effects
                .decision(function.coordinate, budget)
                .map_err(call_error)?
                == CallDecision::Incomplete
            {
                return Err(refuse(Need::PrivateEffects, None));
            }
        }
        Ok(())
    }

    fn require_completed(&self, reports: usize, budget: &mut Budget<'_>) -> Result<(), Failure> {
        let expected = self
            .terminals
            .map_or(self.inventory.functions().len(), |facts| {
                facts.definitions().len()
            });
        if reports != expected {
            return Err(refuse(
                CanonicalPrivateRequirementV1::CalleeCompletion,
                None,
            ));
        }
        for &ordinal in &self.completion_order {
            budget.charge_work(1)?;
            for call in &self.inventory.calls()[self.inventory.functions()[ordinal].calls.clone()] {
                budget.charge_work(1)?;
                let target = call.target.ok_or_else(|| {
                    refuse(
                        CanonicalPrivateRequirementV1::CalleeCompletion,
                        Some(call.coordinate),
                    )
                })?;
                let complete = if let Some(facts) = self.terminals {
                    budget.charge_work(facts.definitions().len())?;
                    facts.is_declaration(target) || facts.definitions().contains(&target)
                } else {
                    (target.0 as usize) < reports
                };
                if !complete {
                    return Err(refuse(
                        CanonicalPrivateRequirementV1::CalleeCompletion,
                        Some(call.coordinate),
                    ));
                }
            }
        }
        Ok(())
    }
}

fn call_error(error: CallError) -> Failure {
    match error {
        CallError::Resource(resource) => Failure::Resource(resource),
        _ => refuse(CanonicalPrivateRequirementV1::DefinedAcyclicCalls, None),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PrivateOperationKindV1 {
    Scalar,
    Allocate,
    Address,
    Read,
    Write,
    Call,
    TrapCall,
    TrapEnd,
}

#[path = "canonical_private_memory_admission_v1.rs"]
mod physical_memory;
pub use physical_memory::{
    CheckedCanonicalPrivateMemoryPoliciesV1, with_canonical_private_memory_policy_checks_v1,
};

#[cfg(test)]
#[path = "canonical_private_admission_v1_tests.rs"]
pub(crate) mod tests;

#[cfg(test)]
#[path = "canonical_trap_pipeline_v1_tests.rs"]
mod trap_tests;
