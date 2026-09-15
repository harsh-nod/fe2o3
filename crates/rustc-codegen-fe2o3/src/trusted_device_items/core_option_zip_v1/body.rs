use super::*;
use rustc_middle::mir::{
    AggregateKind, BasicBlock, Local, MentionedItem, MirPhase, Place, ProjectionElem, RuntimePhase,
    Rvalue, StatementKind, TerminatorKind, UnwindAction, UnwindTerminateReason,
    VarDebugInfoContents,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Value {
    Dead,
    Empty,
    Input { role: usize, some: bool },
    Inputs { some: [bool; 2], full: [bool; 2] },
    Payload(usize),
    Pair,
    Output(bool),
    Bool(bool),
    Discriminant(u128),
}
impl Value {
    fn owns(self) -> bool {
        match self {
            Self::Input { some, .. } | Self::Output(some) => some,
            Self::Inputs { full, .. } => full.into_iter().any(|v| v),
            Self::Payload(_) | Self::Pair => true,
            _ => false,
        }
    }
}
#[derive(Clone, Copy)]
struct State {
    values: [Value; MAX_LOCALS],
    some: [bool; 2],
    dropped: [bool; 2],
}
struct Check<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    body: &'a Body<'tcx>,
    contract: &'a Contract<'tcx>,
    work: usize,
}

pub(super) fn reviewed<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    contract: &Contract<'tcx>,
) -> bool {
    if instance != contract.instance
        || body.source.instance != instance.def
        || body.source.promoted.is_some()
        || body.phase != MirPhase::Runtime(RuntimePhase::Optimized)
        || !body.is_polymorphic
        || body.injection_phase.is_some()
        || body.tainted_by_errors.is_some()
        || body.coroutine.is_some()
        || body.spread_arg.is_some()
        || body.arg_count != 2
        || !(3..=MAX_LOCALS).contains(&body.local_decls.len())
        || !(1..=MAX_BLOCKS).contains(&body.basic_blocks.len())
        || !(1..=8).contains(&body.source_scopes.len())
        || body.var_debug_info.len() > 32
        || !body.user_type_annotations.is_empty()
        || body.required_consts.as_ref().is_none_or(|c| !c.is_empty())
        || body.mentioned_items.as_ref().is_some_and(|c| c.len() > 3)
        || body.coverage_info_hi.is_some()
        || body.function_coverage_info.is_some()
    {
        return false;
    }
    let allowed = [
        contract.inputs[0],
        contract.inputs[1],
        contract.input_pair,
        contract.parameters[0],
        contract.parameters[1],
        contract.payload_pair,
        contract.output,
        tcx.types.bool,
        tcx.types.isize,
    ];
    if body.local_decls.iter().any(|l| !allowed.contains(&l.ty) || l.user_ty.is_some() || l.source_info.scope.as_usize() >= body.source_scopes.len())
        || body.local_decls[Local::from_usize(0)].ty != contract.output
        || body.local_decls[Local::from_usize(1)].ty != contract.inputs[0]
        || body.local_decls[Local::from_usize(2)].ty != contract.inputs[1]
        || body.var_debug_info.iter().any(|d| d.composite.is_some() || d.source_info.scope.as_usize() >= body.source_scopes.len()
            || !matches!(d.value, VarDebugInfoContents::Place(p) if p.projection.is_empty() && p.local.as_usize() < body.local_decls.len()))
        || body.mentioned_items.as_ref().is_some_and(|items| items.iter().any(|item|
            !matches!(item.node, MentionedItem::Drop(ty) if contract.parameters.contains(&ty))))
    { return false; }
    for (i, scope) in body.source_scopes.iter_enumerated() {
        if scope.inlined.is_some()
            || scope.inlined_parent_scope.is_some()
            || match scope.parent_scope {
                None => i.as_usize() != 0,
                Some(p) => p.as_usize() >= i.as_usize(),
            }
        {
            return false;
        }
    }
    let mut check = Check {
        tcx,
        body,
        contract,
        work: 0,
    };
    if !check.closed_shapes() {
        return false;
    }
    for left in [false, true] {
        for right in [false, true] {
            let some = [left, right];
            let mut state = State {
                values: [Value::Empty; MAX_LOCALS],
                some,
                dropped: [false; 2],
            };
            for block in body.basic_blocks.iter() {
                for statement in &block.statements {
                    if let StatementKind::StorageLive(local) = statement.kind {
                        state.values[local.as_usize()] = Value::Dead;
                    }
                }
            }
            state.values[1] = Value::Input {
                role: 0,
                some: left,
            };
            state.values[2] = Value::Input {
                role: 1,
                some: right,
            };
            if !check.path(BasicBlock::from_usize(0), state, false) {
                return false;
            }
        }
    }
    true
}

impl<'tcx> Check<'_, 'tcx> {
    fn local(&self, p: Place<'tcx>) -> Option<usize> {
        (p.projection.is_empty() && p.local.as_usize() < self.body.local_decls.len())
            .then_some(p.local.as_usize())
    }
    fn ty(&self, v: Value) -> Option<Ty<'tcx>> {
        let c = self.contract;
        Some(match v {
            Value::Input { role, .. } => c.inputs[role],
            Value::Inputs { .. } => c.input_pair,
            Value::Payload(role) => c.parameters[role],
            Value::Pair => c.payload_pair,
            Value::Output(_) => c.output,
            Value::Bool(_) => self.tcx.types.bool,
            Value::Discriminant(_) => self.tcx.types.isize,
            _ => return None,
        })
    }
    fn pair_field(&self, p: Place<'tcx>, payload: bool) -> Option<usize> {
        if self.body.local_decls.get(p.local)?.ty != self.contract.input_pair {
            return None;
        }
        let [ProjectionElem::Field(field, option), rest @ ..] = p.projection.as_slice() else {
            return None;
        };
        let role = field.as_usize();
        if self.contract.inputs.get(role) != Some(option) {
            return None;
        }
        if payload {
            let [
                ProjectionElem::Downcast(_, variant),
                ProjectionElem::Field(field, ty),
            ] = rest
            else {
                return None;
            };
            if *variant != self.contract.some
                || field.as_usize() != 0
                || *ty != self.contract.parameters[role]
            {
                return None;
            }
        } else if !rest.is_empty() {
            return None;
        }
        Some(role)
    }
    fn operand_ty(&self, operand: &Operand<'tcx>) -> Option<Ty<'tcx>> {
        match operand {
            Operand::Copy(p) | Operand::Move(p) if p.projection.is_empty() => {
                self.body.local_decls.get(p.local).map(|l| l.ty)
            }
            Operand::Move(p) => self
                .pair_field(*p, true)
                .map(|role| self.contract.parameters[role]),
            Operand::Constant(c)
                if c.user_ty.is_none()
                    && c.const_.ty() == self.tcx.types.bool
                    && c.const_.try_to_bool().is_some() =>
            {
                Some(self.tcx.types.bool)
            }
            _ => None,
        }
    }
    fn rvalue_ty(&self, r: &Rvalue<'tcx>) -> Option<Ty<'tcx>> {
        let c = self.contract;
        match r {
            Rvalue::Use(op) => self.operand_ty(op),
            Rvalue::Discriminant(p) if self.pair_field(*p, false).is_some() => {
                Some(self.tcx.types.isize)
            }
            Rvalue::Aggregate(kind, ops) if ops.len() <= 2 => match &**kind {
                AggregateKind::Tuple if ops.len() == 2 => {
                    let types = [self.operand_ty(&ops.raw[0])?, self.operand_ty(&ops.raw[1])?];
                    if types == c.inputs {
                        Some(c.input_pair)
                    } else if types == c.parameters {
                        Some(c.payload_pair)
                    } else {
                        None
                    }
                }
                AggregateKind::Adt(id, variant, args, None, None)
                    if *id == c.option
                        && *args
                            == match c.output.kind() {
                                TyKind::Adt(_, args) => *args,
                                _ => return None,
                            } =>
                {
                    if *variant == c.none && ops.is_empty()
                        || *variant == c.some
                            && ops.len() == 1
                            && self.operand_ty(&ops.raw[0]) == Some(c.payload_pair)
                    {
                        Some(c.output)
                    } else {
                        None
                    }
                }
                _ => None,
            },
            _ => None,
        }
    }
    fn edge(&self, block: BasicBlock, cleanup: bool) -> bool {
        self.body
            .basic_blocks
            .get(block)
            .is_some_and(|b| b.is_cleanup == cleanup)
    }
    fn closed_unwind(&self, unwind: UnwindAction, cleanup: bool) -> bool {
        match unwind {
            UnwindAction::Cleanup(b) => !cleanup && self.edge(b, true),
            UnwindAction::Continue | UnwindAction::Unreachable => !cleanup,
            UnwindAction::Terminate(UnwindTerminateReason::InCleanup) => cleanup,
            _ => false,
        }
    }
    fn closed_shapes(&self) -> bool {
        let mut statements = 0;
        for block in self.body.basic_blocks.iter() {
            if block.statements.len() > MAX_STATEMENTS - statements
                || !block.after_last_stmt_debuginfos.is_empty()
            {
                return false;
            }
            statements += block.statements.len();
            for statement in &block.statements {
                if !statement.debuginfos.is_empty()
                    || statement.source_info.scope.as_usize() >= self.body.source_scopes.len()
                {
                    return false;
                }
                match &statement.kind {
                    StatementKind::Assign(a)
                        if self.local(a.0).is_some()
                            && self.rvalue_ty(&a.1)
                                == Some(self.body.local_decls[a.0.local].ty) => {}
                    StatementKind::StorageLive(l) | StatementKind::StorageDead(l)
                        if (3..self.body.local_decls.len()).contains(&l.as_usize()) => {}
                    StatementKind::Nop => {}
                    _ => return false,
                }
            }
            let Some(t) = &block.terminator else {
                return false;
            };
            if t.source_info.scope.as_usize() >= self.body.source_scopes.len() {
                return false;
            }
            match &t.kind {
                TerminatorKind::Goto { target } if self.edge(*target, block.is_cleanup) => {}
                TerminatorKind::SwitchInt { discr, targets }
                    if (1..=3).contains(&targets.all_targets().len())
                        && targets.all_values().len() + 1 == targets.all_targets().len()
                        && targets
                            .all_targets()
                            .iter()
                            .all(|t| self.edge(*t, block.is_cleanup))
                        && self.operand_ty(discr).is_some_and(|t| {
                            t == self.tcx.types.bool || t == self.tcx.types.isize
                        })
                        && targets.iter().enumerate().all(|(i, (value, _))| {
                            targets.iter().take(i).all(|(other, _)| value != other)
                        }) => {}
                TerminatorKind::Drop {
                    place,
                    target,
                    unwind,
                    replace: false,
                    drop: None,
                    async_fut: None,
                } if self.pair_field(*place, true).is_some()
                    && self.edge(*target, block.is_cleanup)
                    && self.closed_unwind(*unwind, block.is_cleanup) => {}
                TerminatorKind::Return if !block.is_cleanup => {}
                TerminatorKind::UnwindResume if block.is_cleanup => {}
                TerminatorKind::Unreachable if !block.is_cleanup && block.statements.is_empty() => {
                }
                _ => return false,
            }
        }
        true
    }
    fn operand(&self, s: &mut State, op: &Operand<'tcx>, transfer_copy: bool) -> Option<Value> {
        if let Operand::Constant(c) = op {
            if c.user_ty.is_none() && c.const_.ty() == self.tcx.types.bool {
                return c.const_.try_to_bool().map(Value::Bool);
            }
            return None;
        }
        let (p, copy) = match op {
            Operand::Copy(p) => (*p, true),
            Operand::Move(p) => (*p, false),
            _ => return None,
        };
        let index = p.local.as_usize();
        if let Some(local) = self.local(p) {
            let value = s.values[local];
            if self.ty(value) != Some(self.body.local_decls[p.local].ty)
                || copy
                    && !transfer_copy
                    && !matches!(value, Value::Bool(_) | Value::Discriminant(_))
            {
                return None;
            }
            // Pinned optimized MIR uses last-use Copy into generic aggregates.
            // Treat these as linear transfers, never as duplicated ownership.
            if !copy || transfer_copy {
                s.values[local] = Value::Empty;
            }
            return Some(value);
        }
        let role = self.pair_field(p, true)?;
        let Value::Inputs { some, mut full } = s.values[index] else {
            return None;
        };
        if copy || !some[role] || !full[role] {
            return None;
        }
        full[role] = false;
        s.values[index] = Value::Inputs { some, full };
        Some(Value::Payload(role))
    }
    fn statement(&self, s: &mut State, statement: &StatementKind<'tcx>) -> Option<()> {
        match statement {
            StatementKind::Assign(a) => {
                let v = match &a.1 {
                    Rvalue::Use(op) => self.operand(s, op, false)?,
                    Rvalue::Discriminant(p) => {
                        let role = self.pair_field(*p, false)?;
                        let Value::Inputs { some, .. } = s.values[p.local.as_usize()] else {
                            return None;
                        };
                        Value::Discriminant(self.contract.discriminants[usize::from(some[role])])
                    }
                    Rvalue::Aggregate(kind, ops) if ops.len() <= 2 => match &**kind {
                        AggregateKind::Tuple if ops.len() == 2 => {
                            let a = self.operand(s, &ops.raw[0], true)?;
                            let b = self.operand(s, &ops.raw[1], true)?;
                            match (a, b) {
                                (
                                    Value::Input { role: 0, some: a },
                                    Value::Input { role: 1, some: b },
                                ) => Value::Inputs {
                                    some: [a, b],
                                    full: [a, b],
                                },
                                (Value::Payload(0), Value::Payload(1)) => Value::Pair,
                                _ => return None,
                            }
                        }
                        AggregateKind::Adt(_, variant, _, None, None)
                            if *variant == self.contract.none && ops.is_empty() =>
                        {
                            Value::Output(false)
                        }
                        AggregateKind::Adt(_, variant, _, None, None)
                            if *variant == self.contract.some
                                && ops.len() == 1
                                && self.operand(s, &ops.raw[0], true)? == Value::Pair =>
                        {
                            Value::Output(true)
                        }
                        _ => return None,
                    },
                    _ => return None,
                };
                let local = self.local(a.0)?;
                if s.values[local] == Value::Dead
                    || s.values[local].owns()
                    || self.ty(v) != Some(self.body.local_decls[a.0.local].ty)
                {
                    return None;
                }
                s.values[local] = v;
            }
            StatementKind::StorageLive(l) => {
                if s.values[l.as_usize()] != Value::Dead {
                    return None;
                }
                s.values[l.as_usize()] = Value::Empty;
            }
            StatementKind::StorageDead(l) => {
                if s.values[l.as_usize()] == Value::Dead || s.values[l.as_usize()].owns() {
                    return None;
                }
                s.values[l.as_usize()] = Value::Dead;
            }
            StatementKind::Nop => {}
            _ => return None,
        }
        Some(())
    }
    fn unwind(&mut self, action: UnwindAction, s: State, cleanup: bool) -> bool {
        match action {
            UnwindAction::Cleanup(target) => !cleanup && self.path(target, s, true),
            // No current frame-owned payload may disappear even for no-drop
            // instantiations; generic drop obligations must remain explicit.
            UnwindAction::Continue => !cleanup && s.values.iter().all(|v| !v.owns()),
            UnwindAction::Unreachable => !cleanup && !self.tcx.sess.panic_strategy().unwinds(),
            UnwindAction::Terminate(UnwindTerminateReason::InCleanup) => cleanup,
            _ => false,
        }
    }
    fn path(&mut self, mut block: BasicBlock, mut s: State, cleanup: bool) -> bool {
        let mut seen = [false; MAX_BLOCKS];
        loop {
            if !self.edge(block, cleanup) || seen[block.as_usize()] {
                return false;
            }
            seen[block.as_usize()] = true;
            let data = &self.body.basic_blocks[block];
            self.work += data.statements.len() + 1;
            if self.work > MAX_WORK
                || data
                    .statements
                    .iter()
                    .any(|t| self.statement(&mut s, &t.kind).is_none())
            {
                return false;
            }
            block = match &data.terminator().kind {
                TerminatorKind::Goto { target } => *target,
                TerminatorKind::SwitchInt { discr, targets } => {
                    let value = match self.operand(&mut s, discr, false) {
                        Some(Value::Bool(v)) if targets.iter().all(|(key, _)| key <= 1) => {
                            u128::from(v)
                        }
                        Some(Value::Discriminant(v))
                            if targets
                                .iter()
                                .all(|(key, _)| self.contract.discriminants.contains(&key)) =>
                        {
                            v
                        }
                        _ => return false,
                    };
                    targets.target_for_value(value)
                }
                TerminatorKind::Drop {
                    place,
                    target,
                    unwind,
                    ..
                } => {
                    let Some(role) = self.pair_field(*place, true) else {
                        return false;
                    };
                    if s.some == [true, true]
                        || s.dropped[role]
                        || self.operand(&mut s, &Operand::Move(*place), false)
                            != Some(Value::Payload(role))
                    {
                        return false;
                    }
                    s.dropped[role] = true;
                    if !self.unwind(*unwind, s, cleanup) {
                        return false;
                    }
                    *target
                }
                TerminatorKind::Return if !cleanup => {
                    return s.values[0] == Value::Output(s.some == [true, true])
                        && s.dropped
                            == if s.some == [true, true] {
                                [false, false]
                            } else {
                                s.some
                            }
                        && s.values[1..].iter().all(|v| !v.owns());
                }
                TerminatorKind::UnwindResume if cleanup => {
                    return s.values.iter().all(|v| !v.owns());
                }
                _ => return false,
            };
        }
    }
}
