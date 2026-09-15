//! Use-site value identities for checked arithmetic. No MIR is rewritten.

use std::collections::HashSet;

use super::arithmetic_identity_calls_v1::{ArithmeticCallTransferV1, ArithmeticIdentityCallsV1};
use super::{DominatorIntervalsV1, SemanticOptionDominanceErrorV1, WorkBudgetV1};
use crate::semantic_mir_v1::{
    SemanticConstantValueV1, SemanticFunctionDeclV1, SemanticLocalRoleV1, SemanticOperandV1,
    SemanticProjectionKindV1, SemanticRvalueKindV1, SemanticScalarValueV1, SemanticStatementKindV1,
    SemanticTerminatorKindV1, SemanticTypeIdV1,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) struct ArithmeticSiteV1 {
    pub block: usize,
    pub statement: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ArithmeticConditionSourceV1 {
    Overflow(ArithmeticSiteV1),
    Definition(ArithmeticSiteV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceValueV1 {
    Argument(usize, SemanticTypeIdV1),
    Constant(SemanticScalarValueV1, SemanticTypeIdV1),
    Definition(ArithmeticSiteV1, SemanticTypeIdV1),
    Overflow(ArithmeticSiteV1, SemanticTypeIdV1),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct QueryV1 {
    site: ArithmeticSiteV1,
    local: usize,
    overflow_field: bool,
    ty: SemanticTypeIdV1,
}

enum StepV1 {
    Value(SourceValueV1),
    Alias(QueryV1),
    Predecessors,
    Unknown,
}

pub(super) struct ArithmeticProvenanceV1<'a> {
    function: &'a SemanticFunctionDeclV1,
    dominators: &'a DominatorIntervalsV1,
    escaped: Vec<bool>,
    cyclic: Vec<Option<bool>>,
    calls: Option<&'a ArithmeticIdentityCallsV1<'a>>,
}

impl<'a> ArithmeticProvenanceV1<'a> {
    #[cfg(test)]
    pub fn new(
        function: &'a SemanticFunctionDeclV1,
        dominators: &'a DominatorIntervalsV1,
        budget: &mut WorkBudgetV1,
    ) -> Result<Self, SemanticOptionDominanceErrorV1> {
        Self::new_with_calls(function, dominators, None, budget)
    }

    pub fn new_with_calls(
        function: &'a SemanticFunctionDeclV1,
        dominators: &'a DominatorIntervalsV1,
        calls: Option<&'a ArithmeticIdentityCallsV1<'a>>,
        budget: &mut WorkBudgetV1,
    ) -> Result<Self, SemanticOptionDominanceErrorV1> {
        let mut escaped = filled(function.locals().len(), false, budget)?;
        for block in function.blocks() {
            budget.charge(block.statements().len().saturating_add(1))?;
            for statement in block.statements() {
                if let SemanticStatementKindV1::Assign(assignment) = statement.kind()
                    && let SemanticRvalueKindV1::Borrow { place, .. }
                    | SemanticRvalueKindV1::AddressOf { place, .. } = assignment.value().kind()
                {
                    let slot = escaped.get_mut(place.local().index() as usize).ok_or(
                        SemanticOptionDominanceErrorV1::InvalidControlFlow(
                            "an arithmetic borrow is outside the local table",
                        ),
                    )?;
                    // Even shared or later borrows are excluded: this analysis
                    // deliberately has no alias-effect or interior-mutation model.
                    *slot = true;
                }
            }
        }
        Ok(Self {
            function,
            dominators,
            escaped,
            cyclic: filled(function.blocks().len(), None, budget)?,
            calls,
        })
    }

    pub fn same_value(
        &mut self,
        left: &SemanticOperandV1,
        left_site: ArithmeticSiteV1,
        right: &SemanticOperandV1,
        right_site: ArithmeticSiteV1,
        budget: &mut WorkBudgetV1,
    ) -> Result<bool, SemanticOptionDominanceErrorV1> {
        if left.ty() != right.ty() {
            return Ok(false);
        }
        let Some(left) = self.value_at(left, left_site, budget)? else {
            return Ok(false);
        };
        Ok(self.value_at(right, right_site, budget)? == Some(left))
    }

    pub fn condition_source(
        &mut self,
        operand: &SemanticOperandV1,
        site: ArithmeticSiteV1,
        budget: &mut WorkBudgetV1,
    ) -> Result<Option<ArithmeticConditionSourceV1>, SemanticOptionDominanceErrorV1> {
        Ok(match self.value_at(operand, site, budget)? {
            Some(SourceValueV1::Overflow(site, _)) => {
                Some(ArithmeticConditionSourceV1::Overflow(site))
            }
            Some(SourceValueV1::Definition(site, _)) => {
                Some(ArithmeticConditionSourceV1::Definition(site))
            }
            _ => None,
        })
    }

    fn value_at(
        &mut self,
        operand: &SemanticOperandV1,
        site: ArithmeticSiteV1,
        budget: &mut WorkBudgetV1,
    ) -> Result<Option<SourceValueV1>, SemanticOptionDominanceErrorV1> {
        budget.charge(1)?;
        if let SemanticOperandV1::Constant(_) = operand {
            return Ok(constant_value(operand));
        }
        let Some(query) = operand_query(operand, site) else {
            return Ok(None);
        };
        let mut pending = Vec::new();
        push(&mut pending, query, budget)?;
        let mut visited = HashSet::new();
        let mut agreed = None;
        while let Some(query) = pending.pop() {
            budget.charge(1)?;
            visited
                .try_reserve(1)
                .map_err(|_| SemanticOptionDominanceErrorV1::Storage)?;
            if !visited.insert(query) {
                continue;
            }
            match self.step(query, budget)? {
                StepV1::Unknown => return Ok(None),
                StepV1::Value(value) => {
                    if !agree(&mut agreed, value) {
                        return Ok(None);
                    }
                }
                StepV1::Alias(next) => push(&mut pending, next, budget)?,
                StepV1::Predecessors => {
                    if query.site.block == self.function.entry().index() as usize {
                        if query.overflow_field
                            || !matches!(
                                self.function.locals()[query.local].role(),
                                SemanticLocalRoleV1::Argument(_)
                            )
                            || !agree(&mut agreed, SourceValueV1::Argument(query.local, query.ty))
                        {
                            return Ok(None);
                        }
                    }
                    for &predecessor in &self.dominators.predecessors[query.site.block] {
                        budget.charge(1)?;
                        if !self.dominators.is_reachable(predecessor) {
                            continue;
                        }
                        let block = &self.function.blocks()[predecessor];
                        let mut previous = QueryV1 {
                            site: ArithmeticSiteV1 {
                                block: predecessor,
                                statement: block.statements().len(),
                            },
                            ..query
                        };
                        if matches!(block.terminator().kind(), SemanticTerminatorKindV1::Call(_)) {
                            let transfer =
                                self.calls
                                    .map_or(ArithmeticCallTransferV1::Unknown, |calls| {
                                        calls.transfer(
                                            self.function,
                                            predecessor,
                                            query.site.block,
                                            query.local,
                                            query.ty,
                                            query.overflow_field,
                                        )
                                    });
                            match transfer {
                                ArithmeticCallTransferV1::Unknown => return Ok(None),
                                ArithmeticCallTransferV1::Preserved => (),
                                ArithmeticCallTransferV1::Returned(operand) => {
                                    if let Some(value) = constant_value(operand) {
                                        if !agree(&mut agreed, value) {
                                            return Ok(None);
                                        }
                                        continue;
                                    }
                                    let Some(alias) = operand_query(operand, previous.site) else {
                                        return Ok(None);
                                    };
                                    previous = alias;
                                }
                            }
                        } else if terminator_kills(block.terminator().kind(), query.local) {
                            return Ok(None);
                        }
                        push(&mut pending, previous, budget)?;
                    }
                }
            }
        }
        // A cycle with no reaching source cannot establish a value. A cycle
        // containing only copies can agree on a single invariant source.
        Ok(agreed)
    }

    fn step(
        &mut self,
        query: QueryV1,
        budget: &mut WorkBudgetV1,
    ) -> Result<StepV1, SemanticOptionDominanceErrorV1> {
        let Some(local) = self.function.locals().get(query.local) else {
            return Err(SemanticOptionDominanceErrorV1::InvalidControlFlow(
                "an arithmetic use is outside the local table",
            ));
        };
        if self.escaped[query.local]
            || (!query.overflow_field && local.ty() != query.ty)
            || !self.dominators.is_reachable(query.site.block)
        {
            return Ok(StepV1::Unknown);
        }
        let block = &self.function.blocks()[query.site.block];
        for statement_index in (0..query.site.statement).rev() {
            budget.charge(1)?;
            let kind = block.statements()[statement_index].kind();
            match kind {
                SemanticStatementKindV1::Assign(assignment) => {
                    let destination = assignment.destination();
                    if !destination.projections().is_empty() {
                        return Ok(StepV1::Unknown);
                    }
                    if destination.local().index() as usize == query.local {
                        if destination.ty() != local.ty()
                            || assignment.value().result_type() != local.ty()
                        {
                            return Ok(StepV1::Unknown);
                        }
                        let site = ArithmeticSiteV1 {
                            block: query.site.block,
                            statement: statement_index,
                        };
                        return match assignment.value().kind() {
                            SemanticRvalueKindV1::Use(operand) if operand.ty() == local.ty() => {
                                if !query.overflow_field
                                    && let Some(value) = constant_value(operand)
                                {
                                    return Ok(StepV1::Value(value));
                                }
                                let Some(mut next) = operand_query(operand, site) else {
                                    return Ok(StepV1::Unknown);
                                };
                                if query.overflow_field {
                                    if next.overflow_field {
                                        return Ok(StepV1::Unknown);
                                    }
                                    next.overflow_field = true;
                                    next.ty = query.ty;
                                }
                                Ok(StepV1::Alias(next))
                            }
                            SemanticRvalueKindV1::Use(_)
                            | SemanticRvalueKindV1::Borrow { .. }
                            | SemanticRvalueKindV1::AddressOf { .. } => Ok(StepV1::Unknown),
                            value => {
                                // A static site is not a dynamic definition version
                                // when it can execute repeatedly. Fail closed there.
                                if self.block_is_cyclic(site.block, budget)? {
                                    return Ok(StepV1::Unknown);
                                }
                                if query.overflow_field {
                                    Ok(if matches!(value, SemanticRvalueKindV1::CheckedBinary(_)) {
                                        StepV1::Value(SourceValueV1::Overflow(site, query.ty))
                                    } else {
                                        StepV1::Unknown
                                    })
                                } else {
                                    Ok(StepV1::Value(SourceValueV1::Definition(site, query.ty)))
                                }
                            }
                        };
                    }
                    let mut moved = false;
                    assignment.value().kind().try_visit_operands(|operand| {
                        budget.charge(1)?;
                        moved |= moves_local(operand, query.local);
                        Ok::<_, SemanticOptionDominanceErrorV1>(())
                    })?;
                    if moved {
                        return Ok(StepV1::Unknown);
                    }
                }
                SemanticStatementKindV1::Store(_)
                | SemanticStatementKindV1::AtomicRmw(_)
                | SemanticStatementKindV1::AtomicCompareExchange(_) => return Ok(StepV1::Unknown),
                SemanticStatementKindV1::SetDiscriminant { place, .. }
                | SemanticStatementKindV1::Deinitialize(place) => {
                    if !place.projections().is_empty()
                        || place.local().index() as usize == query.local
                    {
                        return Ok(StepV1::Unknown);
                    }
                }
                SemanticStatementKindV1::StorageLive(local)
                | SemanticStatementKindV1::StorageDead(local) => {
                    if local.index() as usize == query.local {
                        return Ok(StepV1::Unknown);
                    }
                }
                SemanticStatementKindV1::Assume(operand) => {
                    if moves_local(operand, query.local) {
                        return Ok(StepV1::Unknown);
                    }
                }
                SemanticStatementKindV1::Nop => {}
            }
        }
        Ok(StepV1::Predecessors)
    }

    fn block_is_cyclic(
        &mut self,
        root: usize,
        budget: &mut WorkBudgetV1,
    ) -> Result<bool, SemanticOptionDominanceErrorV1> {
        if let Some(cyclic) = self.cyclic[root] {
            return Ok(cyclic);
        }
        let mut seen = filled(self.function.blocks().len(), false, budget)?;
        let mut pending = Vec::new();
        push(&mut pending, root, budget)?;
        while let Some(block) = pending.pop() {
            for &predecessor in &self.dominators.predecessors[block] {
                budget.charge(1)?;
                if !self.dominators.is_reachable(predecessor) {
                    continue;
                }
                if predecessor == root {
                    self.cyclic[root] = Some(true);
                    return Ok(true);
                }
                if !seen[predecessor] {
                    seen[predecessor] = true;
                    push(&mut pending, predecessor, budget)?;
                }
            }
        }
        self.cyclic[root] = Some(false);
        Ok(false)
    }
}

fn constant_value(operand: &SemanticOperandV1) -> Option<SourceValueV1> {
    let SemanticOperandV1::Constant(constant) = operand else {
        return None;
    };
    let SemanticConstantValueV1::Scalar(value) = constant.value() else {
        return None;
    };
    Some(SourceValueV1::Constant(*value, constant.ty()))
}

fn operand_query(operand: &SemanticOperandV1, site: ArithmeticSiteV1) -> Option<QueryV1> {
    let (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = operand else {
        return None;
    };
    let overflow_field = match place.projections() {
        [] => false,
        [field] if field.kind() == SemanticProjectionKindV1::Field(1) => true,
        _ => return None,
    };
    Some(QueryV1 {
        site,
        local: place.local().index() as usize,
        overflow_field,
        ty: place.ty(),
    })
}

fn moves_local(operand: &SemanticOperandV1, local: usize) -> bool {
    matches!(operand, SemanticOperandV1::Move(place) if place.local().index() as usize == local)
}

fn terminator_kills(terminator: &SemanticTerminatorKindV1, local: usize) -> bool {
    match terminator {
        SemanticTerminatorKindV1::Call(_)
        | SemanticTerminatorKindV1::TailCall(_)
        | SemanticTerminatorKindV1::Drop { .. } => true,
        SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => {
            moves_local(discriminant, local)
        }
        SemanticTerminatorKindV1::Assert { condition, .. } => moves_local(condition, local),
        SemanticTerminatorKindV1::Goto(_)
        | SemanticTerminatorKindV1::FalseEdge { .. }
        | SemanticTerminatorKindV1::Return
        | SemanticTerminatorKindV1::UnwindResume
        | SemanticTerminatorKindV1::UnwindTerminate
        | SemanticTerminatorKindV1::Abort
        | SemanticTerminatorKindV1::Unreachable => false,
    }
}

fn agree(agreed: &mut Option<SourceValueV1>, value: SourceValueV1) -> bool {
    match agreed {
        Some(previous) => *previous == value,
        None => {
            *agreed = Some(value);
            true
        }
    }
}

fn filled<T: Clone>(
    count: usize,
    value: T,
    budget: &mut WorkBudgetV1,
) -> Result<Vec<T>, SemanticOptionDominanceErrorV1> {
    budget.charge(count)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| SemanticOptionDominanceErrorV1::Storage)?;
    values.resize(count, value);
    Ok(values)
}

fn push<T>(
    values: &mut Vec<T>,
    value: T,
    budget: &mut WorkBudgetV1,
) -> Result<(), SemanticOptionDominanceErrorV1> {
    budget.charge(1)?;
    values
        .try_reserve(1)
        .map_err(|_| SemanticOptionDominanceErrorV1::Storage)?;
    values.push(value);
    Ok(())
}

#[cfg(test)]
mod tests;
