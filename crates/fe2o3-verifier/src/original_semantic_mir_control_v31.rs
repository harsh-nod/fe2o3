//! Concrete block-local MIR interpretation for scalar branches and loops.
//! Every reachable assignment is retained. Calls require an authenticated
//! source-instance context; memory and exceptional control remain refused.

use super::{
    AssignmentV30, Error, ExpressionV30, Function, LocalRole, NodeV30, Operand, Resource, Result,
    Rvalue, ScalarV30, SourceProgramV30, Statement, Terminator, Type, Writer,
    boundary::{Boundaries, ControlInput},
    call_transfers::CallContext,
    interpreter_headers_v31, vector,
};
use fe2o3_mir_model::{
    SsaBlockIdV1 as Block, SsaConstructionPlanV1 as Plan, SsaValueV1 as Value,
    SsaVariableIdV1 as Variable,
};
use std::mem::size_of;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct LiveIn {
    pub local: u32,
    pub value: Value,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum Branch {
    Goto(Block),
    Switch {
        selector: usize,
        cases: Vec<(u128, Block)>,
        otherwise: Block,
    },
    Return,
    /// Exact source-owned transfer row; this is not a finite expression leaf.
    Call {
        transfer: usize,
        continuation: Block,
    },
}

pub(super) struct SourceBlock {
    pub program: SourceProgramV30,
    pub live: Vec<LiveIn>,
    pub branch: Branch,
    pub changed: Vec<bool>,
}

pub(super) struct SourceControl {
    pub blocks: Vec<Option<SourceBlock>>,
    pub initial: Vec<(u32, u32)>,
    pub entry: Block,
    pub locals: usize,
    pub arguments: usize,
    pub types: Vec<ScalarV30>,
}

impl SourceControl {
    pub(super) fn derive(
        types: &[Type],
        function: &Function,
        plan: &Plan,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        Self::derive_inner(types, function, plan, None, out)
    }

    pub(super) fn derive_instance(
        types: &[Type],
        function: &Function,
        plan: &Plan,
        calls: &CallContext<'_, '_, '_, '_>,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        calls.check_original(types, function, plan, out)?;
        Self::derive_inner(types, function, plan, Some(calls), out)
    }

    fn derive_inner(
        types: &[Type],
        function: &Function,
        plan: &Plan,
        calls: Option<&CallContext<'_, '_, '_, '_>>,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        out.budget.reserve_storage(headers())?;
        if function.blocks().len() != plan.resources().input_blocks() {
            return Err(Error::Statement(
                "original MIR control plan block census differs",
            ));
        }
        let entry = Block::new(function.entry().index());
        let mut successors: Vec<Vec<Block>> = vector(function.blocks().len(), out)?;
        for block in function.blocks() {
            out.budget.charge_work(2)?;
            let terminator = block.terminator().kind();
            if !matches!(
                terminator,
                Terminator::Goto(_) | Terminator::SwitchInt { .. } | Terminator::Return
            ) && !(calls.is_some() && matches!(terminator, Terminator::Call(_)))
            {
                return Err(Error::Statement(
                    "original MIR memory, call or exceptional control is not modeled",
                ));
            }
            let mut edges = vector(terminator.edge_count(), out)?;
            terminator.try_for_each_edge(|edge| {
                out.budget.charge_work(1)?;
                edges.push(Block::new(edge.target().index()));
                Ok::<_, Error>(())
            })?;
            successors.push(edges);
        }
        let boundaries = Boundaries::derive(
            plan,
            ControlInput {
                entry,
                successors: &successors,
            },
            out,
        )?;
        let arguments = function.abi().source_input_types().len();
        let mut initial = vector(arguments, out)?;
        let mut seen = vector(arguments, out)?;
        let mut local_types = vector(function.locals().len(), out)?;
        out.budget.charge_work(arguments)?;
        seen.resize(arguments, false);
        for (local, declaration) in function.locals().iter().enumerate() {
            out.budget.charge_work(2)?;
            let scalar = ScalarV30::from_source(types, declaration.ty())?;
            local_types.push(scalar);
            if let LocalRole::Argument(argument) = declaration.role() {
                if scalar == ScalarV30::Unit
                    || function.abi().source_input_types().get(argument as usize)
                        != Some(&declaration.ty())
                    || seen.get(argument as usize) != Some(&false)
                {
                    return Err(Error::Statement(
                        "original MIR control argument roster differs",
                    ));
                }
                seen[argument as usize] = true;
                initial.push((
                    u32::try_from(local).map_err(|_| Resource::Arithmetic)?,
                    argument,
                ));
            }
        }
        if initial.len() != arguments {
            return Err(Error::Statement("original MIR control argument is missing"));
        }
        let mut result = Self {
            blocks: vector(function.blocks().len(), out)?,
            initial,
            entry,
            locals: function.locals().len(),
            arguments,
            types: local_types,
        };
        for (ordinal, block) in function.blocks().iter().enumerate() {
            out.budget.charge_work(3)?;
            let original = Block::new(u32::try_from(ordinal).map_err(|_| Resource::Arithmetic)?);
            let Some(live) = plan.live_in(original) else {
                result.blocks.push(None);
                continue;
            };
            let statements = block.statements();
            let node_capacity = statements
                .len()
                .checked_mul(3)
                .and_then(|n| n.checked_add(live.len()))
                .and_then(|n| n.checked_add(1))
                .ok_or(Resource::Arithmetic)?;
            let mut program = SourceProgramV30 {
                nodes: vector(node_capacity, out)?,
                assignments: vector(statements.len(), out)?,
                arguments: function.locals().len(),
                returned: None,
                statements: statements.len(),
                locals: vector(function.locals().len(), out)?,
            };
            out.budget.charge_work(function.locals().len())?;
            program.locals.resize(function.locals().len(), None);
            let mut entry_values = vector(live.len(), out)?;
            let mut changed = vector(function.locals().len(), out)?;
            out.budget.charge_work(function.locals().len())?;
            changed.resize(function.locals().len(), false);
            for &variable in live {
                out.budget.charge_work(3)?;
                let local = variable.get() as usize;
                let declaration = function.locals().get(local).ok_or(Error::Statement(
                    "original MIR control live local is absent",
                ))?;
                let value = boundaries.value(original, variable, out)?;
                entry_values.push(LiveIn {
                    local: variable.get(),
                    value,
                });
                let node = program.push(
                    NodeV30 {
                        scalar: ScalarV30::from_source(types, declaration.ty())?,
                        expression: ExpressionV30::Argument(variable.get()),
                    },
                    out,
                )?;
                program.locals[local] = Some(node);
            }
            for (statement, row) in statements.iter().enumerate() {
                program.statement(types, function, statement, row.kind(), out)?;
                changes(row.kind(), &mut changed, out)?;
            }
            let branch = match block.terminator().kind() {
                Terminator::Goto(edge) => Branch::Goto(Block::new(edge.target().index())),
                Terminator::SwitchInt {
                    discriminant,
                    targets,
                } => {
                    let selector = program.operand(types, function, discriminant, out)?;
                    moved(discriminant, &mut changed)?;
                    let scalar = program.nodes[selector].scalar;
                    if scalar == ScalarV30::Unit {
                        return Err(Error::Statement("original MIR unit switch is not modeled"));
                    }
                    let mut cases = vector(targets.values().len(), out)?;
                    for target in targets.values() {
                        out.budget.charge_work(2)?;
                        if target.value() >= (1u128 << scalar.width()) {
                            return Err(Error::Statement(
                                "original MIR switch constant is out of range",
                            ));
                        }
                        cases.push((target.value(), Block::new(target.edge().target().index())));
                    }
                    Branch::Switch {
                        selector,
                        cases,
                        otherwise: Block::new(targets.otherwise().target().index()),
                    }
                }
                Terminator::Return => {
                    program.return_value(types, function, out)?;
                    Branch::Return
                }
                Terminator::Call(call) => {
                    let calls =
                        calls.ok_or(Error::Statement("original MIR call context is absent"))?;
                    let (index, transfer) = calls.call(ordinal, out)?;
                    let instance = calls.instance(out)?;
                    let destination = call.destination().ok_or(Resource::Accounting)?;
                    if transfer.site
                        != instance
                            .blocks
                            .start
                            .checked_add(ordinal)
                            .ok_or(Resource::Arithmetic)?
                        || transfer.returned.continuation
                            != instance
                                .blocks
                                .start
                                .checked_add(destination.edge().target().index() as usize)
                                .ok_or(Resource::Arithmetic)?
                    {
                        return Err(Error::Statement("original MIR body call site differs"));
                    }
                    for &global in &transfer.reads {
                        out.budget.charge_work(2)?;
                        let local = global
                            .checked_sub(instance.locals.start)
                            .ok_or(Resource::Accounting)?;
                        if program.locals.get(local).copied().flatten().is_none() {
                            return Err(Error::Statement(
                                "original MIR call argument is undefined",
                            ));
                        }
                    }
                    Branch::Call {
                        transfer: index,
                        continuation: Block::new(destination.edge().target().index()),
                    }
                }
                _ => return Err(Error::Statement("original MIR control scope changed")),
            };
            result.blocks.push(Some(SourceBlock {
                program,
                live: entry_values,
                branch,
                changed,
            }));
        }
        result.check_edges(calls, out)?;
        Ok(result)
    }

    fn check_edges(
        &self,
        calls: Option<&CallContext<'_, '_, '_, '_>>,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        out.budget.charge_work(self.blocks.len())?;
        for block in self.blocks.iter().flatten() {
            match &block.branch {
                Branch::Goto(target) => self.edge(block, *target, out)?,
                Branch::Switch {
                    cases, otherwise, ..
                } => {
                    for &(_, target) in cases {
                        self.edge(block, target, out)?;
                    }
                    self.edge(block, *otherwise, out)?;
                }
                Branch::Return => {}
                Branch::Call {
                    transfer,
                    continuation,
                } => {
                    let context = calls.ok_or(Resource::Accounting)?;
                    let transfer = context.transfer(*transfer, out)?;
                    let target = self
                        .blocks
                        .get(continuation.get() as usize)
                        .and_then(Option::as_ref)
                        .ok_or(Error::Statement("original MIR call continuation is absent"))?;
                    for live in &target.live {
                        let log = if transfer.moved.is_empty() {
                            0
                        } else {
                            transfer.moved.len().ilog2() as usize + 1
                        };
                        out.budget
                            .charge_work(log.checked_add(3).ok_or(Resource::Arithmetic)?)?;
                        let global = transfer
                            .caller_locals
                            .start
                            .checked_add(live.local as usize)
                            .ok_or(Resource::Arithmetic)?;
                        if global == transfer.returned.destination {
                            continue;
                        }
                        if transfer.moved.binary_search(&global).is_ok()
                            || block
                                .program
                                .locals
                                .get(live.local as usize)
                                .copied()
                                .flatten()
                                .is_none()
                        {
                            return Err(Error::Statement(
                                "original MIR call continuation local is undefined",
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn edge(&self, source: &SourceBlock, target: Block, out: &mut Writer<'_, '_>) -> Result<()> {
        out.budget.charge_work(2)?;
        let target = self
            .blocks
            .get(target.get() as usize)
            .and_then(Option::as_ref)
            .ok_or(Error::Statement("original MIR control target is absent"))?;
        for live in &target.live {
            out.budget.charge_work(2)?;
            if source
                .program
                .locals
                .get(live.local as usize)
                .copied()
                .flatten()
                .is_none()
            {
                return Err(Error::Statement(
                    "original MIR control edge local is undefined",
                ));
            }
        }
        Ok(())
    }
}

fn moved(operand: &Operand, changed: &mut [bool]) -> Result<()> {
    if let Operand::Move(place) = operand {
        *changed
            .get_mut(place.local().index() as usize)
            .ok_or(Resource::Accounting)? = true;
    }
    Ok(())
}

// This is read from the original MIR, independently of the SSA event stream.
// Unmentioned dead locals retain their prior state, including definedness.
fn changes(statement: &Statement, changed: &mut [bool], out: &mut Writer<'_, '_>) -> Result<()> {
    out.budget.charge_work(5)?;
    match statement {
        Statement::Assign(assignment) => {
            *changed
                .get_mut(assignment.destination().local().index() as usize)
                .ok_or(Resource::Accounting)? = true;
            match assignment.value().kind() {
                Rvalue::Use(operand) | Rvalue::Unary { operand, .. } => moved(operand, changed)?,
                Rvalue::Binary { left, right, .. } => {
                    moved(left, changed)?;
                    moved(right, changed)?;
                }
                _ => {
                    return Err(Error::Statement(
                        "original MIR source change is not modeled",
                    ));
                }
            }
        }
        Statement::StorageLive(local) | Statement::StorageDead(local) => {
            *changed
                .get_mut(local.index() as usize)
                .ok_or(Resource::Accounting)? = true;
        }
        Statement::Deinitialize(place) => {
            *changed
                .get_mut(place.local().index() as usize)
                .ok_or(Resource::Accounting)? = true;
        }
        Statement::Nop => {}
        _ => {
            return Err(Error::Statement(
                "original MIR source change is not modeled",
            ));
        }
    }
    Ok(())
}

fn headers() -> usize {
    interpreter_headers_v31()
        + size_of::<SourceControl>()
        + size_of::<Result<SourceControl>>()
        + size_of::<SourceBlock>()
        + size_of::<SourceProgramV30>()
        + size_of::<Boundaries<'_>>()
        + size_of::<Result<Boundaries<'_>>>()
        + size_of::<Vec<Vec<Block>>>()
        + size_of::<Vec<Block>>()
        + size_of::<Vec<bool>>()
        + size_of::<Vec<bool>>()
        + size_of::<Vec<ScalarV30>>()
        + size_of::<Vec<LiveIn>>()
        + size_of::<Vec<(u32, u32)>>()
        + size_of::<Vec<(u128, Block)>>()
        + size_of::<Branch>()
        + size_of::<LiveIn>()
        + size_of::<AssignmentV30>()
        + size_of::<&[Type]>()
        + size_of::<&Function>()
        + size_of::<&Plan>()
        + size_of::<&mut Writer<'_, '_>>()
        + size_of::<Result<()>>()
        + size_of::<Variable>()
        + size_of::<(&Statement, &mut [bool], &mut Writer<'_, '_>, Result<()>)>()
        + size_of::<(&Operand, &mut [bool], Result<()>)>()
        + size_of::<Option<&CallContext<'_, '_, '_, '_>>>()
        + size_of::<(
            &[Type],
            &Function,
            &Plan,
            &CallContext<'_, '_, '_, '_>,
            &mut Writer<'_, '_>,
            Result<SourceControl>,
        )>()
}

#[cfg(test)]
#[path = "original_semantic_mir_control_v31_tests.rs"]
pub(super) mod tests;
