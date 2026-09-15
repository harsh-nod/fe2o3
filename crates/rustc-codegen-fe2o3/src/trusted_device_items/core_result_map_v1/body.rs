use super::*;
use rustc_middle::mir::{
    AggregateKind, BasicBlock, CallSource, MentionedItem, MirPhase, Place, ProjectionElem,
    RuntimePhase, Rvalue, StatementKind, TerminatorKind, UnwindAction, UnwindTerminateReason,
    VarDebugInfoContents,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Value {
    Dead,
    Empty,
    Input { err: bool, full: bool },
    Payload(bool),
    Callback,
    Tuple,
    Converted,
    Output(bool),
    Unit,
    Bool(bool),
    Discriminant(u128),
}

impl Value {
    fn owns(self) -> bool {
        matches!(
            self,
            Self::Input { full: true, .. }
                | Self::Payload(_)
                | Self::Callback
                | Self::Tuple
                | Self::Converted
                | Self::Output(_)
        )
    }
}

#[derive(Clone, Copy)]
struct State {
    values: [Value; MAX_LOCALS],
    err: bool,
    calls: usize,
    drops: usize,
}

struct Check<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &'a Body<'tcx>,
    contract: &'a Contract<'tcx>,
    visited: [bool; MAX_BLOCKS],
    work: usize,
}

pub(super) fn reviewed_body<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    contract: &Contract<'tcx>,
) -> bool {
    // Bounds precede metadata traversal, normalization or any retained state copy.
    // All execution state is fixed-size; both variants and each cleanup edge are
    // checked. Nonempty unvisited blocks are rejected rather than pruned.
    if !header(tcx, body, instance, contract) {
        return false;
    }
    let mut check = Check {
        tcx,
        instance,
        body,
        contract,
        visited: [false; MAX_BLOCKS],
        work: 0,
    };
    for err in [false, true] {
        let mut state = State {
            values: [Value::Empty; MAX_LOCALS],
            err,
            calls: 0,
            drops: 0,
        };
        for block in body.basic_blocks.iter() {
            for statement in &block.statements {
                if let StatementKind::StorageLive(local) = statement.kind {
                    if local.as_usize() < 3 || local.as_usize() >= body.local_decls.len() {
                        return false;
                    }
                    state.values[local.as_usize()] = Value::Dead;
                }
            }
        }
        state.values[1] = Value::Input { err, full: true };
        state.values[2] = Value::Callback;
        if !check.path(BasicBlock::from_usize(0), state, false, 0) {
            return false;
        }
    }
    body.basic_blocks.iter_enumerated().all(|(id, block)| {
        check.visited[id.as_usize()]
            || (!block.is_cleanup
                && block.statements.is_empty()
                && matches!(block.terminator().kind, TerminatorKind::Unreachable))
    })
}

fn header<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    instance: Instance<'tcx>,
    contract: &Contract<'tcx>,
) -> bool {
    if instance != contract.instance
        || body.source.instance != instance.def
        || body.source.promoted.is_some()
        || body.phase != MirPhase::Runtime(RuntimePhase::Optimized)
        || body.injection_phase.is_some()
        || body.tainted_by_errors.is_some()
        || !body.is_polymorphic
        || body.coroutine.is_some()
        || body.spread_arg.is_some()
        || body.arg_count != 2
        || !(3..=MAX_LOCALS).contains(&body.local_decls.len())
        || !(1..=MAX_BLOCKS).contains(&body.basic_blocks.len())
        || !(1..=MAX_SCOPES).contains(&body.source_scopes.len())
        || body.var_debug_info.len() > MAX_DEBUG_INFO
        || !body.user_type_annotations.is_empty()
        || body
            .required_consts
            .as_ref()
            .is_none_or(|items| !items.is_empty())
        || body
            .mentioned_items
            .as_ref()
            .is_some_and(|items| items.len() > MAX_MENTIONED_ITEMS)
        || body.coverage_info_hi.is_some()
        || body.function_coverage_info.is_some()
    {
        return false;
    }
    let mut statements = 0;
    for block in body.basic_blocks.iter() {
        if block.statements.len() > MAX_STATEMENTS - statements {
            return false;
        }
        statements += block.statements.len();
        if !block.after_last_stmt_debuginfos.is_empty()
            || block
                .terminator
                .as_ref()
                .is_none_or(|t| t.source_info.scope.as_usize() >= body.source_scopes.len())
            || block.statements.iter().any(|s| {
                !s.debuginfos.is_empty()
                    || s.source_info.scope.as_usize() >= body.source_scopes.len()
            })
        {
            return false;
        }
    }
    for (index, scope) in body.source_scopes.iter_enumerated() {
        if scope.inlined.is_some()
            || scope.inlined_parent_scope.is_some()
            || match scope.parent_scope {
                None => index.as_usize() != 0,
                Some(parent) => parent.as_usize() >= index.as_usize(),
            }
        {
            return false;
        }
    }
    let allowed = [
        contract.input,
        contract.output,
        contract.tuple,
        contract.parameters[0],
        contract.parameters[1],
        contract.parameters[2],
        contract.parameters[3],
        tcx.types.bool,
        tcx.types.isize,
        tcx.types.unit,
    ];
    if body.local_decls.iter().any(|local| !allowed.contains(&local.ty) || local.source_info.scope.as_usize() >= body.source_scopes.len() || local.user_ty.is_some())
        || body.var_debug_info.iter().any(|debug| debug.composite.is_some()
            || debug.source_info.scope.as_usize() >= body.source_scopes.len()
            || !matches!(debug.value, VarDebugInfoContents::Place(place) if place.projection.is_empty() && place.local.as_usize() < body.local_decls.len()))
        || body.mentioned_items.as_ref().is_some_and(|items| items.iter().any(|item| match item.node {
            MentionedItem::Fn(ty) => ty != contract.callback,
            MentionedItem::Drop(ty) => ty != contract.parameters[3] && ty != contract.output,
            _ => true,
        }))
    { return false; }
    body.local_decls.raw[..3].iter().map(|local| local.ty).eq([
        contract.output,
        contract.input,
        contract.parameters[3],
    ])
}

impl<'tcx> Check<'_, 'tcx> {
    fn value_ty(&self, value: Value) -> Option<Ty<'tcx>> {
        let c = self.contract;
        Some(match value {
            Value::Input { .. } => c.input,
            Value::Payload(err) => c.parameters[usize::from(err)],
            Value::Callback => c.parameters[3],
            Value::Tuple => c.tuple,
            Value::Converted => c.parameters[2],
            Value::Output(_) => c.output,
            Value::Unit => self.tcx.types.unit,
            Value::Bool(_) => self.tcx.types.bool,
            Value::Discriminant(_) => self.tcx.types.isize,
            _ => return None,
        })
    }

    fn local(&self, place: Place<'tcx>) -> Option<usize> {
        (place.projection.is_empty() && place.local.as_usize() < self.body.local_decls.len())
            .then_some(place.local.as_usize())
    }

    fn put(&self, state: &mut State, place: Place<'tcx>, value: Value) -> Option<()> {
        let local = self.local(place)?;
        if state.values[local] == Value::Dead
            || state.values[local].owns()
            || self.value_ty(value) != Some(self.body.local_decls[place.local].ty)
        {
            return None;
        }
        state.values[local] = value;
        Some(())
    }

    fn operand(
        &self,
        state: &mut State,
        operand: &Operand<'tcx>,
        transfer_copy: bool,
    ) -> Option<Value> {
        let (place, copy) = match operand {
            Operand::Move(place) => (*place, false),
            Operand::Copy(place) => (*place, true),
            Operand::Constant(c) if c.user_ty.is_none() && c.const_.ty() == self.tcx.types.bool => {
                return c.const_.try_to_bool().map(Value::Bool);
            }
            Operand::Constant(c)
                if c.user_ty.is_none()
                    && matches!(c.const_, Const::Val(ConstValue::ZeroSized, ty) if ty == self.tcx.types.unit) =>
            {
                return Some(Value::Unit);
            }
            _ => return None,
        };
        let index = place.local.as_usize();
        let ty = self.body.local_decls.get(place.local)?.ty;
        let value = state.values[index];
        if place.projection.is_empty() {
            if self.value_ty(value) != Some(ty)
                || matches!(value, Value::Input { full: false, .. })
                || (copy
                    && !matches!(value, Value::Unit | Value::Bool(_) | Value::Discriminant(_))
                    && !transfer_copy)
            {
                return None;
            }
            // Pinned optimized core uses last-use Copy into aggregates even for
            // generic non-Copy T/E. Consume that ownership token: any subsequent
            // use, drop, overwrite or duplicate transfer fails the same proof.
            if !copy || transfer_copy {
                state.values[index] = Value::Empty;
            }
            return Some(value);
        }
        let [
            ProjectionElem::Downcast(_, variant),
            ProjectionElem::Field(field, field_ty),
        ] = place.projection.as_slice()
        else {
            return None;
        };
        let Value::Input { err, full: true } = value else {
            return None;
        };
        let role = usize::from(err);
        if copy
            || ty != self.contract.input
            || *variant != self.contract.variants[role]
            || field.as_usize() != 0
            || *field_ty != self.contract.parameters[role]
        {
            return None;
        }
        state.values[index] = Value::Input { err, full: false };
        Some(Value::Payload(err))
    }

    fn statement(&self, state: &mut State, statement: &StatementKind<'tcx>) -> Option<()> {
        match statement {
            StatementKind::Assign(a) => {
                let value = match &a.1 {
                    Rvalue::Use(operand) => self.operand(state, operand, false)?,
                    Rvalue::Discriminant(place) => {
                        let local = self.local(*place)?;
                        if self.body.local_decls[place.local].ty != self.contract.input {
                            return None;
                        }
                        let Value::Input { err, .. } = state.values[local] else {
                            return None;
                        };
                        Value::Discriminant(self.contract.discriminants[usize::from(err)])
                    }
                    Rvalue::Aggregate(kind, operands) if operands.len() == 1 => {
                        let value = self.operand(state, &operands.raw[0], true)?;
                        match &**kind {
                            AggregateKind::Tuple if value == Value::Payload(true) => Value::Tuple,
                            AggregateKind::Adt(id, variant, args, None, None)
                                if *id == self.contract.result
                                    && *args
                                        == match self.contract.output.kind() {
                                            TyKind::Adt(_, args) => *args,
                                            _ => return None,
                                        } =>
                            {
                                if *variant == self.contract.variants[0]
                                    && value == Value::Payload(false)
                                {
                                    Value::Output(false)
                                } else if *variant == self.contract.variants[1]
                                    && value == Value::Converted
                                {
                                    Value::Output(true)
                                } else {
                                    return None;
                                }
                            }
                            _ => return None,
                        }
                    }
                    _ => return None,
                };
                self.put(state, a.0, value)
            }
            StatementKind::StorageLive(local) | StatementKind::StorageDead(local) => {
                if local.as_usize() < 3 || local.as_usize() >= self.body.local_decls.len() {
                    return None;
                }
                let value = &mut state.values[local.as_usize()];
                if matches!(statement, StatementKind::StorageLive(_)) {
                    if *value != Value::Dead {
                        return None;
                    }
                    *value = Value::Empty;
                } else {
                    if value.owns() || *value == Value::Dead {
                        return None;
                    }
                    *value = Value::Dead;
                }
                Some(())
            }
            StatementKind::Nop => Some(()),
            _ => None,
        }
    }

    fn edge(&self, target: BasicBlock, cleanup: bool) -> bool {
        self.body
            .basic_blocks
            .get(target)
            .is_some_and(|b| b.is_cleanup == cleanup)
    }

    fn exit_without_leak(&self, state: &State) -> bool {
        state.values.iter().copied().all(|value| {
            if !value.owns() {
                return true;
            }
            let c = self.contract;
            let ty = match value {
                Value::Input { err, .. } => c.concrete[usize::from(err)],
                Value::Output(err) => c.concrete[if err { 2 } else { 0 }],
                Value::Payload(err) => c.concrete[usize::from(err)],
                Value::Callback => c.concrete[3],
                Value::Tuple => Ty::new_tup(self.tcx, &[c.concrete[1]]),
                Value::Converted => c.concrete[2],
                _ => return false,
            };
            !ty.needs_drop(self.tcx, TypingEnv::fully_monomorphized())
        })
    }

    fn unwind(&mut self, unwind: UnwindAction, state: State, cleanup: bool, depth: usize) -> bool {
        match unwind {
            UnwindAction::Continue => {
                !cleanup
                    && (!self.tcx.sess.panic_strategy().unwinds() || self.exit_without_leak(&state))
            }
            UnwindAction::Unreachable => !self.tcx.sess.panic_strategy().unwinds(),
            UnwindAction::Cleanup(target) => {
                !cleanup && self.edge(target, true) && self.path(target, state, true, depth + 1)
            }
            // This edge is rustc's double-panic behavior for a retained cleanup
            // drop. The normal edge must still consume all remaining ownership.
            UnwindAction::Terminate(UnwindTerminateReason::InCleanup) => cleanup,
            _ => false,
        }
    }

    fn path(
        &mut self,
        mut block: BasicBlock,
        mut state: State,
        cleanup: bool,
        depth: usize,
    ) -> bool {
        if depth > 1 {
            return false;
        }
        let mut visited = [false; MAX_BLOCKS];
        loop {
            if !self.edge(block, cleanup) || visited[block.as_usize()] {
                return false;
            }
            visited[block.as_usize()] = true;
            self.visited[block.as_usize()] = true;
            let data = &self.body.basic_blocks[block];
            self.work += data.statements.len() + 1;
            if self.work > MAX_WORK
                || data
                    .statements
                    .iter()
                    .any(|s| self.statement(&mut state, &s.kind).is_none())
            {
                return false;
            }
            block = match &data.terminator().kind {
                TerminatorKind::Goto { target } if self.edge(*target, cleanup) => *target,
                TerminatorKind::SwitchInt { discr, targets } => {
                    if !(1..=3).contains(&targets.all_targets().len())
                        || targets.all_values().len() + 1 != targets.all_targets().len()
                        || targets
                            .all_targets()
                            .iter()
                            .any(|target| !self.edge(*target, cleanup))
                    {
                        return false;
                    }
                    let mut seen = [None; 2];
                    for (index, (value, _)) in targets.iter().enumerate() {
                        if seen.contains(&Some(value)) {
                            return false;
                        }
                        seen[index] = Some(value);
                    }
                    let value = match self.operand(&mut state, discr, false) {
                        Some(Value::Bool(value)) if targets.iter().all(|(key, _)| key <= 1) => {
                            u128::from(value)
                        }
                        Some(Value::Discriminant(value))
                            if targets
                                .iter()
                                .all(|(key, _)| self.contract.discriminants.contains(&key)) =>
                        {
                            value
                        }
                        _ => return false,
                    };
                    targets.target_for_value(value)
                }
                TerminatorKind::Call {
                    func,
                    args,
                    destination,
                    target: Some(target),
                    unwind,
                    call_source: CallSource::Normal,
                    ..
                } if !cleanup
                    && state.err
                    && state.calls == 0
                    && state.drops == 0
                    && args.len() == 2
                    && exact_callback(self.tcx, self.instance, func, self.contract)
                    && matches!(args[0].node, Operand::Move(_))
                    && matches!(args[1].node, Operand::Move(_))
                    && self.operand(&mut state, &args[0].node, false) == Some(Value::Callback)
                    && self.operand(&mut state, &args[1].node, false) == Some(Value::Tuple)
                    && self.edge(*target, false) =>
                {
                    state.calls += 1;
                    if !self.unwind(*unwind, state, false, depth)
                        || self
                            .put(&mut state, *destination, Value::Converted)
                            .is_none()
                    {
                        return false;
                    }
                    *target
                }
                TerminatorKind::Drop {
                    place,
                    target,
                    unwind,
                    replace: false,
                    drop: None,
                    async_fut: None,
                } => {
                    let Some(local) = self.local(*place) else {
                        return false;
                    };
                    if self.value_ty(state.values[local])
                        != Some(self.body.local_decls[place.local].ty)
                    {
                        return false;
                    }
                    match state.values[local] {
                        Value::Callback
                            if !cleanup && !state.err && state.calls == 0 && state.drops == 0 =>
                        {
                            state.drops += 1
                        }
                        Value::Output(false) if cleanup && !state.err && state.drops == 1 => {}
                        _ => return false,
                    }
                    state.values[local] = Value::Empty;
                    if !self.edge(*target, cleanup) || !self.unwind(*unwind, state, cleanup, depth)
                    {
                        return false;
                    }
                    *target
                }
                TerminatorKind::Return if !cleanup => {
                    return state.values[0] == Value::Output(state.err)
                        && state.calls == usize::from(state.err)
                        && state.drops == usize::from(!state.err)
                        && state.values[1..].iter().all(|value| !value.owns());
                }
                TerminatorKind::UnwindResume if cleanup => return self.exit_without_leak(&state),
                _ => return false,
            };
        }
    }
}

#[cfg(test)]
pub(super) fn check_work_boundaries<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    contract: &Contract<'tcx>,
) {
    assert!(header(tcx, body, instance, contract));
    let mut state = State {
        values: [Value::Empty; MAX_LOCALS],
        err: false,
        calls: 0,
        drops: 0,
    };
    for statement in body.basic_blocks.iter().flat_map(|b| &b.statements) {
        if let StatementKind::StorageLive(local) = statement.kind {
            state.values[local.as_usize()] = Value::Dead;
        }
    }
    state.values[1] = Value::Input {
        err: false,
        full: true,
    };
    state.values[2] = Value::Callback;
    let make = |work| Check {
        tcx,
        instance,
        body,
        contract,
        visited: [false; MAX_BLOCKS],
        work,
    };
    let mut baseline = make(0);
    assert!(baseline.path(BasicBlock::from_usize(0), state, false, 0));
    let cost = baseline.work;
    assert!(cost > 1 && cost < MAX_WORK);
    for (remaining, expected) in [(cost + 1, true), (cost, true), (cost - 1, false)] {
        assert_eq!(
            make(MAX_WORK - remaining).path(BasicBlock::from_usize(0), state, false, 0),
            expected
        );
    }
}
