//! Private parameter-bound reasoning over an already replayed execution view.
//! No new MIR, source identity, certificate constructor or proof authority.

use super::*;
use crate::semantic_direct_call_expansion_v1::{
    SemanticCallInstanceIdV1, SemanticExpandedRootV1, SemanticExpandedStatementOriginV1 as Origin,
    SemanticExpandedTerminatorOriginV1 as TerminatorOrigin,
};
use crate::semantic_mir_v1::{
    SemanticAssignmentV1, SemanticCallableDeclV1, SemanticConstantValueV1,
};

type Result<T> = std::result::Result<T, SemanticU32InductionAnalysisErrorV1>;

#[derive(Clone, Copy)]
struct Parameter {
    definition: DefinitionSiteV1,
    parent_argument: Option<SemanticLocalIdV1>,
}

pub(super) struct ExpandedBoundsV1<'a> {
    source: &'a AdmittedInertSemanticMirV1,
    view: &'a SemanticExpandedRootV1,
    parameters: Vec<Option<Parameter>>,
    ordinary_alias: Vec<bool>,
    invalidations: Vec<Vec<DefinitionSiteV1>>,
}

impl<'a> ExpandedBoundsV1<'a> {
    pub(super) fn analyze(
        source: &'a AdmittedInertSemanticMirV1,
        view: &'a SemanticExpandedRootV1,
        budget: &mut WorkBudgetV1,
    ) -> Result<Self> {
        let locals = view.body().locals().len();
        budget.charge(locals * 4)?;
        let mut opened = fallible_filled_vec(locals, None)?;
        let mut result = Self {
            source,
            view,
            parameters: fallible_filled_vec(locals, None)?,
            ordinary_alias: fallible_filled_vec(locals, false)?,
            invalidations: fallible_nested_vec(locals)?,
        };
        for (block_index, block) in view.body().blocks().iter().enumerate() {
            budget.charge(1)?;
            let origins = &view.block_origins()[block_index];
            for (index, statement) in block.statements().iter().enumerate() {
                budget.charge(1)?;
                let site = DefinitionSiteV1 {
                    block: block_index,
                    position: DefinitionPositionV1::Statement(index),
                };
                if let Origin::FrameStorageLive { callee, local } = origins.statements()[index]
                    && let SemanticStatementKindV1::StorageLive(execution_local) = statement.kind()
                {
                    let origin = view.local_origins()[execution_local.index() as usize];
                    if origin.instance() == callee && origin.local() == local {
                        opened[execution_local.index() as usize] = Some(site);
                    }
                }
                match statement.kind() {
                    SemanticStatementKindV1::Assign(assignment) => {
                        let parameter = result.parameter(assignment, site, &opened);
                        if let Some(parameter) = parameter {
                            let slot = &mut result.parameters
                                [assignment.destination().local().index() as usize];
                            if slot.replace(parameter).is_some() {
                                return Err(SemanticU32InductionAnalysisErrorV1::InvalidModel(
                                    "duplicate execution parameter transfer",
                                ));
                            }
                        }
                        if let SemanticRvalueKindV1::Use(value) = assignment.value().kind()
                            && let Some(place) = exact_operand_place(value)
                            && place.local() != assignment.destination().local()
                            && parameter.and_then(|p| p.parent_argument) != Some(place.local())
                        {
                            result.ordinary_alias[place.local().index() as usize] = true;
                        }
                        assignment.value().kind().try_visit_operands(|operand| {
                            result.record_move(operand, site, budget)
                        })?;
                    }
                    SemanticStatementKindV1::Store(store) => {
                        result.record_move(store.value(), site, budget)?
                    }
                    SemanticStatementKindV1::AtomicRmw(atomic) => {
                        result.record_move(atomic.value(), site, budget)?
                    }
                    SemanticStatementKindV1::AtomicCompareExchange(atomic) => {
                        result.record_move(atomic.expected(), site, budget)?;
                        result.record_move(atomic.replacement(), site, budget)?;
                    }
                    SemanticStatementKindV1::StorageLive(local)
                    | SemanticStatementKindV1::StorageDead(local) => {
                        result.invalidate(*local, site, budget)?
                    }
                    SemanticStatementKindV1::Deinitialize(place) => {
                        result.invalidate(place.local(), site, budget)?
                    }
                    SemanticStatementKindV1::Assume(operand) => {
                        result.record_move(operand, site, budget)?
                    }
                    SemanticStatementKindV1::SetDiscriminant { .. }
                    | SemanticStatementKindV1::Nop => {}
                }
            }
            result.record_terminator(block_index, block.terminator().kind(), budget)?;
        }
        Ok(result)
    }

    fn argument(&self, local: SemanticLocalIdV1) -> Option<(SemanticCallInstanceIdV1, u32)> {
        let execution = self.view.body().locals().get(local.index() as usize)?;
        let origin = *self.view.local_origins().get(local.index() as usize)?;
        let function = self
            .source
            .functions()
            .get(origin.function().index() as usize)?;
        let original = function.locals().get(origin.local().index() as usize)?;
        let SemanticLocalRoleV1::Argument(argument) = original.role() else {
            return None;
        };
        if !is_exact_u32(self.source.types(), original.ty())
            || execution.ty() != original.ty()
            || function.abi().source_input_types().get(argument as usize) != Some(&original.ty())
            || execution.role()
                != if origin.instance().index() == 0 {
                    SemanticLocalRoleV1::Argument(argument)
                } else {
                    SemanticLocalRoleV1::Temporary
                }
        {
            return None;
        }
        Some((origin.instance(), argument))
    }

    fn parameter(
        &self,
        assignment: &SemanticAssignmentV1,
        site: DefinitionSiteV1,
        opened: &[Option<DefinitionSiteV1>],
    ) -> Option<Parameter> {
        let DefinitionPositionV1::Statement(index) = site.position else {
            return None;
        };
        let block = self.view.block_origins().get(site.block)?;
        let Origin::ParameterTransfer { callee, argument } = *block.statements().get(index)? else {
            return None;
        };
        let destination = assignment.destination();
        if self.argument(destination.local()) != Some((callee, argument))
            || !destination.projections().is_empty()
            || assignment.value().result_type() != destination.ty()
            || block.terminator() != (TerminatorOrigin::CallEntry { callee })
        {
            return None;
        }
        let child = self.view.instances().get(callee.index() as usize)?;
        if child.parent() != Some(block.instance()) || child.call_block() != Some(block.block()) {
            return None;
        }
        let live = opened[destination.local().index() as usize]?;
        if live.block != site.block || position(live) >= index {
            return None;
        }
        let SemanticRvalueKindV1::Use(value) = assignment.value().kind() else {
            return None;
        };
        if value.ty() != destination.ty() {
            return None;
        }
        let parent = self
            .source
            .functions()
            .get(block.function().index() as usize)?;
        let SemanticTerminatorKindV1::Call(call) = parent
            .blocks()
            .get(block.block().index() as usize)?
            .terminator()
            .kind()
        else {
            return None;
        };
        if !matches!(self.source.callables().get(call.callee().index() as usize),
            Some(SemanticCallableDeclV1::Defined { function }) if *function == child.function())
        {
            return None;
        }
        let original = call.arguments().get(argument as usize)?;
        let parent_argument = match (value, original) {
            (SemanticOperandV1::Constant(value), SemanticOperandV1::Constant(original))
                if matches!(value.value(), SemanticConstantValueV1::Scalar(scalar)
                    if scalar.size_bytes() == 4 && scalar.bits() <= u128::from(u32::MAX))
                    && value == original =>
            {
                None
            }
            (SemanticOperandV1::Copy(value), SemanticOperandV1::Copy(original))
            | (SemanticOperandV1::Move(value), SemanticOperandV1::Move(original))
                if value.projections().is_empty()
                    && original.projections().is_empty()
                    && value.ty() == original.ty()
                    && self
                        .argument(value.local())
                        .is_some_and(|(instance, _)| instance == block.instance())
                    && self.view.local_origins()[value.local().index() as usize].local()
                        == original.local() =>
            {
                Some(value.local())
            }
            _ => return None,
        };
        Some(Parameter {
            definition: site,
            parent_argument,
        })
    }

    pub(super) fn same_frame(
        &self,
        bound: SemanticLocalIdV1,
        locals: &[SemanticLocalIdV1],
        blocks: &[usize],
        statements: &[Option<DefinitionSiteV1>],
    ) -> bool {
        let instance = self.view.local_origins()[bound.index() as usize].instance();
        locals
            .iter()
            .all(|local| self.view.local_origins()[local.index() as usize].instance() == instance)
            && blocks
                .iter()
                .all(|block| self.view.block_origins()[*block].instance() == instance)
            && statements.iter().flatten().all(|site| {
                let DefinitionPositionV1::Statement(index) = site.position else {
                    return false;
                };
                matches!(
                    self.view.block_origins()[site.block]
                        .statements()
                        .get(index),
                    Some(Origin::Source { .. })
                )
            })
    }

    pub(super) fn proves_bound(
        &self,
        mut bound: SemanticLocalIdV1,
        mut use_site: DefinitionSiteV1,
        inventory: &SemanticInventoryV1,
        graph: &SemanticCfgV1,
        budget: &mut WorkBudgetV1,
    ) -> Result<bool> {
        // Transfer chains strictly ascend the replayed instance tree. Each link
        // proves a value at its actual use; a Copy/Move alone grants nothing.
        loop {
            budget.charge(1)?;
            let Some((instance, _)) = self.argument(bound) else {
                return Ok(false);
            };
            let index = bound.index() as usize;
            if self.ordinary_alias[index] || inventory.address_or_projection_hazard[index] {
                return Ok(false);
            }
            let parameter = self.parameters[index];
            if instance.index() == 0 {
                if definition(inventory, bound)?.count != 0 {
                    return Ok(false);
                }
            } else {
                let Some(parameter) = parameter else {
                    return Ok(false);
                };
                if !definition(inventory, bound)?.is_unique_at(parameter.definition)
                    || (parameter.definition.block == use_site.block
                        && position(parameter.definition) >= position(use_site))
                    || !graph.dominates(parameter.definition.block, use_site.block, budget)?
                {
                    return Ok(false);
                }
            }
            for invalidation in &self.invalidations[index] {
                budget.charge(1)?;
                if reaches_without_reinitialization(
                    *invalidation,
                    use_site,
                    parameter.map(|p| p.definition),
                    graph,
                    budget,
                )? {
                    return Ok(false);
                }
            }
            match parameter {
                Some(Parameter {
                    definition,
                    parent_argument: Some(parent),
                }) => {
                    let Some((parent_instance, _)) = self.argument(parent) else {
                        return Ok(false);
                    };
                    if self.view.instances()[instance.index() as usize].parent()
                        != Some(parent_instance)
                    {
                        return Ok(false);
                    }
                    bound = parent;
                    use_site = definition;
                }
                _ => return Ok(true),
            }
        }
    }

    fn invalidate(
        &mut self,
        local: SemanticLocalIdV1,
        site: DefinitionSiteV1,
        budget: &mut WorkBudgetV1,
    ) -> Result<()> {
        budget.charge(1)?;
        let sites = &mut self.invalidations[local.index() as usize];
        if sites.last() != Some(&site) {
            sites
                .try_reserve(1)
                .map_err(|_| SemanticU32InductionAnalysisErrorV1::Storage)?;
            sites.push(site);
        }
        Ok(())
    }

    fn record_move(
        &mut self,
        operand: &SemanticOperandV1,
        site: DefinitionSiteV1,
        budget: &mut WorkBudgetV1,
    ) -> Result<()> {
        budget.charge(1)?;
        if let SemanticOperandV1::Move(place) = operand {
            self.invalidate(place.local(), site, budget)?;
        }
        Ok(())
    }

    fn record_terminator(
        &mut self,
        block: usize,
        terminator: &SemanticTerminatorKindV1,
        budget: &mut WorkBudgetV1,
    ) -> Result<()> {
        budget.charge(1)?;
        let site = DefinitionSiteV1 {
            block,
            position: DefinitionPositionV1::Terminator,
        };
        match terminator {
            SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => {
                self.record_move(discriminant, site, budget)?
            }
            SemanticTerminatorKindV1::Call(call) => {
                for operand in call.arguments() {
                    self.record_move(operand, site, budget)?;
                }
            }
            SemanticTerminatorKindV1::TailCall(call) => {
                for operand in call.arguments() {
                    self.record_move(operand, site, budget)?;
                }
            }
            SemanticTerminatorKindV1::Drop { place, .. } => {
                self.invalidate(place.local(), site, budget)?
            }
            SemanticTerminatorKindV1::Assert {
                condition, message, ..
            } => {
                self.record_move(condition, site, budget)?;
                match message {
                    SemanticAssertMessageV1::BoundsCheck { length, index }
                    | SemanticAssertMessageV1::Overflow {
                        left: length,
                        right: index,
                        ..
                    } => {
                        self.record_move(length, site, budget)?;
                        self.record_move(index, site, budget)?;
                    }
                    SemanticAssertMessageV1::DivisionByZero(operand)
                    | SemanticAssertMessageV1::RemainderByZero(operand) => {
                        self.record_move(operand, site, budget)?
                    }
                    SemanticAssertMessageV1::MisalignedPointerDereference {
                        required_alignment,
                        found_alignment,
                    } => {
                        self.record_move(required_alignment, site, budget)?;
                        self.record_move(found_alignment, site, budget)?;
                    }
                    SemanticAssertMessageV1::NullPointerDereference
                    | SemanticAssertMessageV1::ResumedAfterReturn
                    | SemanticAssertMessageV1::ResumedAfterPanic => {}
                }
            }
            SemanticTerminatorKindV1::Goto(_)
            | SemanticTerminatorKindV1::FalseEdge { .. }
            | SemanticTerminatorKindV1::Return
            | SemanticTerminatorKindV1::UnwindResume
            | SemanticTerminatorKindV1::UnwindTerminate
            | SemanticTerminatorKindV1::Abort
            | SemanticTerminatorKindV1::Unreachable => {}
        }
        Ok(())
    }
}

fn position(site: DefinitionSiteV1) -> usize {
    match site.position {
        DefinitionPositionV1::Statement(index) => index,
        DefinitionPositionV1::Terminator => usize::MAX,
    }
}

/// Can a storage/Move invalidation reach a later read without the parameter's
/// unique definition executing again? The initial partial block is distinct
/// from a backedge re-entry, including when the invalidation is the read itself.
fn reaches_without_reinitialization(
    from: DefinitionSiteV1,
    to: DefinitionSiteV1,
    definition: Option<DefinitionSiteV1>,
    graph: &SemanticCfgV1,
    budget: &mut WorkBudgetV1,
) -> Result<bool> {
    let reset = definition.filter(|definition| {
        definition.block == from.block && position(*definition) > position(from)
    });
    if from.block == to.block
        && position(from) < position(to)
        && reset.is_none_or(|reset| position(to) <= position(reset))
    {
        return Ok(true);
    }
    if reset.is_some() {
        return Ok(false);
    }
    budget.charge(graph.successors.len() * 2)?;
    let mut visited = fallible_filled_vec(graph.successors.len(), false)?;
    let mut pending = Vec::new();
    pending
        .try_reserve(graph.successors.len())
        .map_err(|_| SemanticU32InductionAnalysisErrorV1::Storage)?;
    for &successor in &graph.successors[from.block] {
        budget.charge(1)?;
        if !visited[successor] {
            visited[successor] = true;
            pending.push(successor);
        }
    }
    while let Some(block) = pending.pop() {
        budget.charge(1)?;
        let reset = definition.filter(|definition| definition.block == block);
        if block == to.block && reset.is_none_or(|reset| position(to) <= position(reset)) {
            return Ok(true);
        }
        if reset.is_some() {
            continue;
        }
        for &successor in &graph.successors[block] {
            budget.charge(1)?;
            if !visited[successor] {
                visited[successor] = true;
                pending.push(successor);
            }
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site(block: usize, statement: usize) -> DefinitionSiteV1 {
        DefinitionSiteV1 {
            block,
            position: DefinitionPositionV1::Statement(statement),
        }
    }

    fn cycle() -> SemanticCfgV1 {
        SemanticCfgV1 {
            entry: 0,
            successors: vec![vec![1], vec![0]],
            predecessors: vec![vec![1], vec![0]],
        }
    }

    #[test]
    fn expanded_bound_invalidation_distinguishes_reentry_from_same_block_suffix() {
        let graph = cycle();
        for (from, to, reset, expected) in [
            (site(0, 0), site(0, 2), Some(site(0, 1)), false),
            (site(0, 0), site(0, 1), Some(site(0, 2)), true),
            (site(0, 2), site(0, 1), Some(site(0, 0)), false),
            (site(0, 1), site(0, 1), None, true),
            (site(1, 0), site(0, 1), Some(site(0, 0)), false),
            (site(1, 0), site(0, 0), Some(site(0, 1)), true),
        ] {
            let mut budget = WorkBudgetV1::new(100);
            assert_eq!(
                reaches_without_reinitialization(from, to, reset, &graph, &mut budget).unwrap(),
                expected
            );
        }
    }

    #[test]
    fn expanded_bound_reachability_charges_storage_before_early_reset() {
        let mut budget = WorkBudgetV1::new(3);
        assert!(matches!(
            reaches_without_reinitialization(
                site(1, 0),
                site(0, 1),
                Some(site(0, 0)),
                &cycle(),
                &mut budget,
            ),
            Err(SemanticU32InductionAnalysisErrorV1::WorkLimit { .. })
        ));
        assert_eq!(budget.used, 4);
    }
}
