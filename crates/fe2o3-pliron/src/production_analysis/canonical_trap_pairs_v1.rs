//! Exact terminal-pair shape on the authoritative graph, never assertion truth.
use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirDefinitionRefV1, CanonicalKirEdgeRefV1, CanonicalKirInventoryV1 as Inventory,
    CanonicalKirUseRefV1,
};
use fe2o3_kernel_ir::{
    CanonicalKirBlockCoordinateV1 as BlockCoordinate,
    CanonicalKirFunctionCoordinateV1 as FunctionCoordinate,
    CanonicalKirOperationCoordinateV1 as OperationCoordinate, FunctionRole, OperationKind,
    ScalarType, Terminator, Type,
};
use std::ops::Range;

#[path = "canonical_trap_resources_v1.rs"]
mod resources;

#[path = "canonical_trap_shape_v1.rs"]
mod shape;
pub use shape::{CheckedCanonicalTrapShapeV1, with_canonical_trap_shape_v1};

pub(super) fn refuse() -> Failure {
    private::refuse(private::CanonicalPrivateRequirementV1::TerminalPairs, None)
}

pub(super) fn reserve_header<T>(budget: &mut Budget<'_>) -> Result<(), Failure> {
    resources::reserve_header::<T>(budget)
}

/// A real diagnostic call and its terminal Unreachable in the same original N.
/// The pair is not a proof that its incoming failure edges are infeasible.
pub struct CanonicalTrapPairV1 {
    call: OperationCoordinate,
    declaration: FunctionCoordinate,
    incoming: Range<usize>,
}
impl CanonicalTrapPairV1 {
    pub const fn call(&self) -> OperationCoordinate {
        self.call
    }
    pub const fn terminal_block(&self) -> BlockCoordinate {
        self.call.block
    }
    pub const fn declaration(&self) -> FunctionCoordinate {
        self.declaration
    }
    pub fn incoming_edges(&self) -> Range<usize> {
        self.incoming.clone()
    }
}

/// Exact original condition/use/definition and both ordered edge payloads.
/// `success_when` records graph polarity, not a proved source value.
pub struct CanonicalTrapIncomingEdgeV1<'i, 'g> {
    condition: &'i CanonicalKirUseRefV1,
    definition: &'i CanonicalKirDefinitionRefV1<'g>,
    success: &'i CanonicalKirEdgeRefV1<'g>,
    failure: &'i CanonicalKirEdgeRefV1<'g>,
    success_when: bool,
}
impl<'i, 'g> CanonicalTrapIncomingEdgeV1<'i, 'g> {
    pub const fn condition(&self) -> &'i CanonicalKirUseRefV1 {
        self.condition
    }
    pub const fn definition(&self) -> &'i CanonicalKirDefinitionRefV1<'g> {
        self.definition
    }
    pub const fn success(&self) -> &'i CanonicalKirEdgeRefV1<'g> {
        self.success
    }
    pub const fn failure(&self) -> &'i CanonicalKirEdgeRefV1<'g> {
        self.failure
    }
    pub const fn success_when(&self) -> bool {
        self.success_when
    }
}

pub(crate) struct CanonicalTrapPairsGraphFactsV1<'i, 'g> {
    inventory: &'i Inventory<'g>,
    pairs: Vec<CanonicalTrapPairV1>,
    incoming: Vec<CanonicalTrapIncomingEdgeV1<'i, 'g>>,
    definitions: Vec<FunctionCoordinate>,
    declaration: Option<FunctionCoordinate>,
}
impl<'i, 'g> CanonicalTrapPairsGraphFactsV1<'i, 'g> {
    pub(crate) fn derive(
        inventory: &'i Inventory<'g>,
        budget: &mut Budget<'_>,
    ) -> Result<Self, Failure> {
        budget.reserve_storage(size_of::<Self>())?;
        let mut facts = Self {
            inventory,
            pairs: reserve_rows(inventory.blocks().len(), budget)?,
            incoming: reserve_rows(inventory.edges().len(), budget)?,
            definitions: reserve_rows(inventory.functions().len(), budget)?,
            declaration: None,
        };
        for function in inventory.functions() {
            budget.charge_work(1)?;
            if function.function.body.is_some() {
                facts.definitions.push(function.coordinate);
                continue;
            }
            let function_value = function.function;
            if !resources::is_trap(&function_value.id, budget)?
                || function_value.role != FunctionRole::ExternalImport
                || !function_value.signature.parameters.is_empty()
                || !function_value.signature.results.is_empty()
                || facts.declaration.replace(function.coordinate).is_some()
            {
                return Err(refuse());
            }
            // The connected owner's reserved-declaration verification already
            // checked the exact capability contract; retain this actual row.
        }
        for block in inventory.blocks() {
            budget.charge_work(1)?;
            if !matches!(block.terminator, Terminator::Unreachable) {
                continue;
            }
            if block.coordinate.block == 0
                || !block.block.parameters.is_empty()
                || block.operations.len() != 1
            {
                return Err(refuse());
            }
            let operation = &inventory.operations()[block.operations.start];
            let OperationKind::Call { callee, arguments } = &operation.operation.kind else {
                return Err(refuse());
            };
            budget.charge_work(1)?;
            if !arguments.is_empty()
                || !operation.operation.results.is_empty()
                || !operation.effects.is_empty()
                || !operation.compiler_ordering().is_empty()
                || !resources::is_trap(callee, budget)?
            {
                return Err(refuse());
            }
            let declaration = facts.declaration.ok_or_else(refuse)?;
            let target = inventory
                .function_for_name(callee.as_str(), budget)
                .map_err(resources::inventory_error)?
                .ok_or_else(refuse)?;
            if target.coordinate != declaration {
                return Err(refuse());
            }
            let start = facts.incoming.len();
            for edge in inventory.edges() {
                budget.charge_work(1)?;
                if edge.target != block.coordinate {
                    continue;
                }
                let function = &inventory.functions()[edge.coordinate.source.function.0 as usize];
                let source_index = function
                    .blocks
                    .start
                    .checked_add(edge.coordinate.source.block as usize)
                    .ok_or(Resource::Arithmetic)?;
                let source = inventory.blocks().get(source_index).ok_or_else(refuse)?;
                if source.coordinate != edge.coordinate.source
                    || source.edges.len() != 2
                    || source.terminator_uses.is_empty()
                    || !edge.arguments.is_empty()
                {
                    return Err(refuse());
                }
                let Terminator::ConditionalBranch { condition, .. } = source.terminator else {
                    return Err(refuse());
                };
                let failure_slot = edge.coordinate.successor as usize;
                if failure_slot > 1 {
                    return Err(refuse());
                }
                let success = &inventory.edges()[source.edges.start + 1 - failure_slot];
                if success.target == block.coordinate
                    || success.coordinate.source != source.coordinate
                    || success.coordinate.successor as usize != 1 - failure_slot
                    || source.terminator_uses.len()
                        != success
                            .arguments
                            .len()
                            .checked_add(1)
                            .ok_or(Resource::Arithmetic)?
                {
                    return Err(refuse());
                }
                let condition_use = &inventory.uses()[source.terminator_uses.start];
                let definition = &inventory.definitions()[condition_use.definition];
                budget.charge_work(1)?;
                if condition_use.value != *condition
                    || condition_use.coordinate
                        != (fe2o3_kernel_ir::CanonicalKirUseCoordinateV1::TerminatorOperand {
                            block: source.coordinate,
                            operand: 0,
                        })
                    || *definition.ty != Type::Scalar(ScalarType::Bool)
                {
                    return Err(refuse());
                }
                facts.incoming.push(CanonicalTrapIncomingEdgeV1 {
                    condition: condition_use,
                    definition,
                    success,
                    failure: edge,
                    success_when: failure_slot == 1,
                });
            }
            if facts.incoming.len() == start {
                return Err(refuse());
            }
            facts.pairs.push(CanonicalTrapPairV1 {
                call: operation.coordinate,
                declaration,
                incoming: start..facts.incoming.len(),
            });
        }
        // Every reserved Trap occurrence must be an exact terminal pair. Other
        // calls are checked by the sealed private/defined-call certificate.
        for call in inventory.calls() {
            budget.charge_work(1)?;
            let OperationKind::Call { callee, .. } = &call.operation.kind else {
                return Err(refuse());
            };
            if resources::is_trap(callee, budget)? {
                budget.charge_work(facts.pairs.len())?;
                if !facts.is_call(call.coordinate) || call.target != facts.declaration {
                    return Err(refuse());
                }
            }
        }
        for incoming in &facts.incoming {
            budget.charge_work(checked_add(1, facts.pairs.len())?)?;
            if facts.is_end(incoming.success.target) {
                return Err(refuse());
            }
        }
        Ok(facts)
    }
    pub(crate) fn belongs_to(&self, inventory: &Inventory<'_>) -> bool {
        std::ptr::eq(self.inventory, inventory)
    }
    pub(crate) fn is_declaration(&self, coordinate: FunctionCoordinate) -> bool {
        self.declaration == Some(coordinate)
    }
    pub(crate) fn is_call(&self, coordinate: OperationCoordinate) -> bool {
        self.pairs.iter().any(|pair| pair.call == coordinate)
    }
    pub(crate) fn is_end(&self, coordinate: BlockCoordinate) -> bool {
        self.pairs.iter().any(|pair| pair.call.block == coordinate)
    }
    pub(crate) fn pair_count(&self) -> usize {
        self.pairs.len()
    }
    pub(crate) fn first_call(&self) -> Option<OperationCoordinate> {
        self.pairs.first().map(|pair| pair.call)
    }
    pub(crate) fn definitions(&self) -> &[FunctionCoordinate] {
        &self.definitions
    }
}

/// Callback-scoped shape evidence paired with nine real reports per definition.
/// No predicate truth, source equivalence, artifact or launch grant is exposed.
///
/// ```compile_fail
/// use fe2o3_pliron::CheckedCanonicalTrapPoliciesV1;
/// fn forge() { let _ = CheckedCanonicalTrapPoliciesV1 {}; }
/// ```
/// ```compile_fail
/// use fe2o3_pliron::CanonicalTrapPairV1;
/// fn forge() { let _ = CanonicalTrapPairV1 {}; }
/// ```
pub struct CheckedCanonicalTrapPoliciesV1<'s, 'g> {
    inner: private::CheckedCanonicalPrivatePoliciesV1<'s, 'g>,
    facts: &'s CanonicalTrapPairsGraphFactsV1<'s, 'g>,
}
impl<'s, 'g> CheckedCanonicalTrapPoliciesV1<'s, 'g> {
    pub(super) fn new(
        inner: private::CheckedCanonicalPrivatePoliciesV1<'s, 'g>,
        facts: &'s CanonicalTrapPairsGraphFactsV1<'s, 'g>,
    ) -> Self {
        Self { inner, facts }
    }
    pub fn owner(&self, budget: &mut Budget<'_>) -> Result<&'g Owner, Failure> {
        self.inner.owner(budget)
    }
    pub fn module_function_count(&self, budget: &mut Budget<'_>) -> Result<usize, Failure> {
        self.inner.owner(budget)?;
        Ok(self.facts.inventory.functions().len())
    }
    pub fn definition_count(&self, budget: &mut Budget<'_>) -> Result<usize, Failure> {
        self.inner.function_count(budget)
    }
    pub fn definition_coordinate(
        &self,
        ordinal: usize,
        budget: &mut Budget<'_>,
    ) -> Result<FunctionCoordinate, Failure> {
        self.inner.owner(budget)?;
        self.facts
            .definitions
            .get(ordinal)
            .copied()
            .ok_or_else(|| self.inner.invalid_query(ordinal))
    }
    pub fn pair_count(&self, budget: &mut Budget<'_>) -> Result<usize, Failure> {
        self.inner.owner(budget)?;
        Ok(self.facts.pairs.len())
    }
    pub fn pair(
        &self,
        ordinal: usize,
        budget: &mut Budget<'_>,
    ) -> Result<&CanonicalTrapPairV1, Failure> {
        self.inner.owner(budget)?;
        self.facts
            .pairs
            .get(ordinal)
            .ok_or_else(|| self.inner.invalid_query(ordinal))
    }
    pub fn incoming_edge(
        &self,
        ordinal: usize,
        budget: &mut Budget<'_>,
    ) -> Result<&CanonicalTrapIncomingEdgeV1<'s, 'g>, Failure> {
        self.inner.owner(budget)?;
        self.facts
            .incoming
            .get(ordinal)
            .ok_or_else(|| self.inner.invalid_query(ordinal))
    }
    pub fn report(&self, definition: usize, budget: &mut Budget<'_>)
    -> Result<&crate::production_analysis::pliron_pipeline::canonical_private_v1::CanonicalPrivatePipelineReportV1, Failure>{
        self.inner.report(definition, budget)
    }
    pub fn history(
        &self,
        definition: usize,
        budget: &mut Budget<'_>,
    ) -> Result<CanonicalRankedPolicyHistoryV1, Failure> {
        self.inner.history(definition, budget)
    }
    pub fn observation(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<CanonicalRankedPolicyResourceObservationV1, Failure> {
        self.inner.observation(budget)
    }
    pub const fn pending_obligations(&self) -> Obligations {
        self.inner.pending_obligations()
    }
    pub const fn ranked_verification_is_complete(&self) -> bool {
        false
    }
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

pub fn with_canonical_trap_policy_checks_v1<'w, T>(
    checked: &mut CheckedCanonicalRankedViewV1<'_, '_, '_, '_>,
    budget: &mut Budget<'w>,
    callback: impl for<'s, 'g> FnOnce(
        &CheckedCanonicalTrapPoliciesV1<'s, 'g>,
        &mut Budget<'w>,
    ) -> Result<T, Failure>,
) -> Result<T, CanonicalRankedPolicyChecksErrorV1> {
    private::with_trap_checks(checked, budget, Limits::production_hard_ceiling(), callback)
}

#[cfg(test)]
#[path = "canonical_trap_pairs_v1_tests.rs"]
pub(super) mod tests;
