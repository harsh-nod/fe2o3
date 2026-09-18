//! Closure uses and forwarding candidates; callee admission belongs to the flow resolver.

use super::*;
use crate::rustc_semantic_plan_v1::SourceClosureWorkV1;
use rustc_middle::mir::{AssertKind, AssertMessage};
use rustc_span::Spanned;

pub(super) struct ClosureUsesV1<'a> {
    pub(super) environments: &'a [ClosureEnvironmentV1],
    pub(super) closure_locals: &'a BTreeSet<Local>,
    /// Every alias maps directly to its environment root.
    pub(super) aliases: &'a BTreeMap<Local, Local>,
    /// Candidate source-argument ordinals, pending the parent's flow checks.
    pub(super) forwarding: &'a BTreeMap<usize, BTreeSet<usize>>,
}

pub(super) fn validate_uses_and_calls<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    uses: ClosureUsesV1<'_>,
    work: &mut SourceClosureWorkV1,
) -> Result<(Vec<StaticClosureCallV1>, Vec<ClosureForwardV1>, bool), ClosureProfileErrorV1> {
    let mut classify_constant = |ty, work: &mut SourceClosureWorkV1| {
        charge(work, 1)?;
        let mut ty = normalized_ty(tcx, instance, ty, "closure constant use")?;
        while let TyKind::Ref(_, pointee, _) = ty.kind() {
            charge(work, 1)?;
            ty = *pointee;
        }
        Ok(matches!(ty.kind(), TyKind::Closure(..)))
    };
    let mut scanner = ClosureUseScannerV1::new(uses, work, &mut classify_constant)?;
    for (block_index, block) in body.basic_blocks.iter_enumerated() {
        scanner.charge(1)?;
        let block_index = block_index.as_usize();
        for statement in &block.statements {
            scanner.charge(1)?;
            if let Some((destination, value)) = statement.kind.as_assign() {
                if scanner.allowed_closure_assignment(*destination, value)? {
                    continue;
                }
                if scanner.place_mentions_closure(*destination)?
                    || scanner.rvalue_mentions_closure(value)?
                {
                    return Err(ClosureProfileErrorV1::new(format!(
                        "closure value escapes through an unsupported assignment in bb{block_index}"
                    )));
                }
            } else if scanner.statement_mentions_closure(&statement.kind)? {
                return Err(ClosureProfileErrorV1::new(format!(
                    "closure value is used by an unsupported statement in bb{block_index}"
                )));
            }
        }
        if let Some(terminator) = &block.terminator {
            scanner.charge(1)?;
            scanner.validate_terminator(tcx, instance, body, block_index, &terminator.kind)?;
        }
    }
    let saw_closure_constant = scanner.saw_closure_constant;
    let (calls, forwards) = scanner.finish()?;
    Ok((calls, forwards, saw_closure_constant))
}

struct ClosureUseScannerV1<'a, 'work, 'tcx> {
    uses: ClosureUsesV1<'a>,
    work: &'work mut SourceClosureWorkV1,
    by_local: BTreeMap<Local, (&'a ClosureEnvironmentV1, usize)>,
    total_uses: usize,
    calls: Vec<StaticClosureCallV1>,
    forwards: Vec<ClosureForwardV1>,
    classify_constant: &'a mut dyn FnMut(
        Ty<'tcx>,
        &mut SourceClosureWorkV1,
    ) -> Result<bool, ClosureProfileErrorV1>,
    saw_closure_constant: bool,
    constant_uses: usize,
}

impl<'a, 'work, 'tcx> ClosureUseScannerV1<'a, 'work, 'tcx> {
    fn new(
        uses: ClosureUsesV1<'a>,
        work: &'work mut SourceClosureWorkV1,
        classify_constant: &'a mut dyn FnMut(
            Ty<'tcx>,
            &mut SourceClosureWorkV1,
        ) -> Result<bool, ClosureProfileErrorV1>,
    ) -> Result<Self, ClosureProfileErrorV1> {
        let mut by_local = BTreeMap::new();
        for environment in uses.environments {
            charge(work, 2)?; // Environment scan and map allocation.
            by_local.insert(Local::from_usize(environment.local), (environment, 0));
        }
        Ok(Self {
            uses,
            work,
            by_local,
            total_uses: 0,
            calls: Vec::new(),
            forwards: Vec::new(),
            classify_constant,
            saw_closure_constant: false,
            constant_uses: 0,
        })
    }

    fn charge(&mut self, amount: usize) -> Result<(), ClosureProfileErrorV1> {
        charge(self.work, amount)
    }

    fn root(&mut self, local: Local) -> Result<Option<Local>, ClosureProfileErrorV1> {
        resolve_alias_root(
            local,
            self.uses.closure_locals,
            self.uses.aliases,
            self.work,
        )
    }

    fn operand_root(
        &mut self,
        operand: &Operand<'tcx>,
    ) -> Result<Option<Local>, ClosureProfileErrorV1> {
        match operand_local(operand, self.work)? {
            Some(local) => self.root(local),
            None => Ok(None),
        }
    }

    fn validate_terminator(
        &mut self,
        tcx: TyCtxt<'tcx>,
        instance: Instance<'tcx>,
        body: &Body<'tcx>,
        block: usize,
        terminator: &TerminatorKind<'tcx>,
    ) -> Result<(), ClosureProfileErrorV1> {
        let reason = match terminator {
            TerminatorKind::Call {
                func,
                args,
                destination,
                ..
            } => {
                if self.place_mentions_closure(*destination)? {
                    return Err(ClosureProfileErrorV1::new(
                        "call result reconstructs a closure or receiver alias",
                    ));
                }
                return self.validate_call(tcx, instance, body, block, func, args);
            }
            TerminatorKind::TailCall { func, args, .. } => {
                let mut mentions = self.operand_mentions_closure(func)?;
                for argument in args {
                    self.charge(1)?;
                    mentions |= self.operand_mentions_closure(&argument.node)?;
                }
                mentions.then_some("closure value escapes through a tail call")
            }
            TerminatorKind::Drop { place, .. } => {
                if self.place_mentions_closure(*place)? {
                    let allowed = match place.as_local() {
                        Some(local) => self.root(local)?.is_some(),
                        None => false,
                    };
                    if !allowed {
                        return Err(ClosureProfileErrorV1::new(
                            "closure drop must consume one unprojected closure or receiver alias",
                        ));
                    }
                }
                None
            }
            TerminatorKind::SwitchInt { discr, .. } => self
                .operand_mentions_closure(discr)?
                .then_some("closure value is used as a switch discriminant"),
            TerminatorKind::Assert { cond, msg, .. } => (self.operand_mentions_closure(cond)?
                || self.assert_message_mentions_closure(msg)?)
            .then_some("closure value is used as an assertion condition or message"),
            TerminatorKind::Yield {
                value, resume_arg, ..
            } => (self.operand_mentions_closure(value)?
                || self.place_mentions_closure(*resume_arg)?)
            .then_some("closure value escapes through a coroutine yield"),
            TerminatorKind::InlineAsm { operands, .. } => {
                for operand in operands {
                    if self.inline_asm_mentions_closure(operand)? {
                        return Err(ClosureProfileErrorV1::new(
                            "closure value escapes through inline assembly",
                        ));
                    }
                }
                None
            }
            _ => None,
        };
        match reason {
            Some(reason) => Err(ClosureProfileErrorV1::new(reason)),
            None => Ok(()),
        }
    }

    fn validate_call(
        &mut self,
        tcx: TyCtxt<'tcx>,
        instance: Instance<'tcx>,
        body: &Body<'tcx>,
        block: usize,
        func: &Operand<'tcx>,
        args: &[Spanned<Operand<'tcx>>],
    ) -> Result<(), ClosureProfileErrorV1> {
        if self.operand_mentions_closure(func)? {
            return Err(ClosureProfileErrorV1::new(
                "closure value escapes through an indirect call target",
            ));
        }
        self.charge(1)?;
        // A non-Fn callee may take a closure at ordinal zero without invoking it.
        let call_kind = declared_call_kind(tcx, func).ok();
        let mut receiver = None;
        for (ordinal, argument) in args.iter().enumerate() {
            self.charge(1)?;
            if !self.operand_mentions_closure(&argument.node)? {
                continue;
            }
            if call_kind.is_some() {
                if ordinal != 0 {
                    return Err(ClosureProfileErrorV1::new(
                        "closure value escapes through a non-receiver call argument",
                    ));
                }
                receiver = Some(self.operand_root(&argument.node)?.ok_or_else(|| {
                    ClosureProfileErrorV1::new(
                        "closure receiver must be one unprojected closure or receiver alias",
                    )
                })?);
            } else if matches!(argument.node, Operand::Constant(_)) {
                self.require_forward_candidate(block, ordinal)?;
                if self.uses.environments.len() + self.constant_uses >= MAX_CLOSURES {
                    return Err(ClosureProfileErrorV1::new(format!(
                        "closure count exceeds {MAX_CLOSURES}"
                    )));
                }
                let constant = constants_v1::EmptyClosureConstantV1::observe(
                    tcx, instance, block, ordinal, self.work,
                )?;
                self.check_static_use_limit()?;
                self.charge(1)?;
                self.forwards.push(ClosureForwardV1 {
                    block,
                    argument: ordinal,
                    source: ClosureForwardSourceV1::EmptyConstant(constant),
                });
                self.total_uses += 1;
                self.constant_uses += 1;
            } else {
                self.record_forward(block, ordinal, &argument.node)?;
            }
        }
        let (Some(closure_local), Some(call_kind)) = (receiver, call_kind) else {
            return Ok(());
        };
        self.charge(1)?;
        let environment = self
            .by_local
            .get(&closure_local)
            .map(|entry| entry.0)
            .ok_or_else(|| {
                ClosureProfileErrorV1::new("closure receiver has no recorded environment")
            })?;
        if !call_kind_allowed(environment.call_kind, call_kind) {
            return Err(ClosureProfileErrorV1::new(
                "closure invoked through an incompatible Fn trait",
            ));
        }
        if args.len() != 2 {
            return Err(ClosureProfileErrorV1::new(
                "bounded closure calls require receiver plus one tuple argument",
            ));
        }
        self.charge(2)?; // Argument and operand inspection by the tuple helper.
        let argument_count = tuple_argument_count(tcx, instance, body, &args[1].node)?;
        if argument_count > MAX_CALL_ARGUMENTS {
            return Err(ClosureProfileErrorV1::new(format!(
                "closure call argument count exceeds {MAX_CALL_ARGUMENTS}"
            )));
        }
        self.charge(1)?;
        let target = resolve_direct_call(tcx, instance, func)?;
        let mut receiver_ty = normalized_ty(
            tcx,
            instance,
            args[0].node.ty(body, tcx),
            "closure receiver",
        )?;
        while let TyKind::Ref(_, pointee, _) = receiver_ty.kind() {
            self.charge(1)?;
            receiver_ty = *pointee;
        }
        let TyKind::Closure(definition, closure_args) = receiver_ty.kind() else {
            return Err(ClosureProfileErrorV1::new("closure receiver type changed"));
        };
        let requested = match call_kind {
            ClosureCallKindV1::Fn => ClosureKind::Fn,
            ClosureCallKindV1::FnMut => ClosureKind::FnMut,
            ClosureCallKindV1::FnOnce => ClosureKind::FnOnce,
        };
        let target_definition_hash = tcx.def_path_hash(target.def_id()).0.to_le_bytes();
        if tcx.def_path_hash(*definition).0.to_le_bytes() != environment.definition_hash
            || target != Instance::resolve_closure(tcx, *definition, closure_args, requested)
        {
            return Err(ClosureProfileErrorV1::new(
                "closure call did not resolve to its compiler-generated body or once shim",
            ));
        }
        authenticate_once_shim_v1(tcx, target)?;
        self.record_call(StaticClosureCallV1 {
            block,
            closure_local: closure_local.as_usize(),
            call_kind,
            argument_count,
            target_definition_hash,
        })
    }

    fn record_forward(
        &mut self,
        block: usize,
        argument: usize,
        operand: &Operand<'tcx>,
    ) -> Result<(), ClosureProfileErrorV1> {
        self.require_forward_candidate(block, argument)?;
        let closure_local = self.operand_root(operand)?.ok_or_else(|| {
            ClosureProfileErrorV1::new(
                "closure forwarding requires one unprojected closure or receiver alias",
            )
        })?;
        self.record_use(closure_local)?;
        self.charge(1)?;
        self.forwards.push(ClosureForwardV1 {
            block,
            argument,
            source: ClosureForwardSourceV1::Local(closure_local.as_usize()),
        });
        Ok(())
    }

    fn require_forward_candidate(
        &mut self,
        block: usize,
        argument: usize,
    ) -> Result<(), ClosureProfileErrorV1> {
        self.charge(2)?;
        if !self
            .uses
            .forwarding
            .get(&block)
            .is_some_and(|arguments| arguments.contains(&argument))
        {
            return Err(ClosureProfileErrorV1::new(
                "closure value escapes to a non-closure call without a forwarding candidate",
            ));
        }
        Ok(())
    }

    fn record_call(&mut self, call: StaticClosureCallV1) -> Result<(), ClosureProfileErrorV1> {
        self.record_use(Local::from_usize(call.closure_local))?;
        self.charge(1)?;
        self.calls.push(call);
        Ok(())
    }

    fn record_use(&mut self, local: Local) -> Result<(), ClosureProfileErrorV1> {
        self.check_static_use_limit()?;
        let (_, count) = self
            .by_local
            .get_mut(&local)
            .ok_or_else(|| ClosureProfileErrorV1::new("closure use has no recorded environment"))?;
        *count += 1;
        self.total_uses += 1;
        Ok(())
    }

    fn check_static_use_limit(&mut self) -> Result<(), ClosureProfileErrorV1> {
        self.charge(1)?;
        if self.total_uses >= MAX_STATIC_CALLS {
            return Err(ClosureProfileErrorV1::new(format!(
                "closure static use count exceeds {MAX_STATIC_CALLS}"
            )));
        }
        Ok(())
    }

    fn finish(
        self,
    ) -> Result<(Vec<StaticClosureCallV1>, Vec<ClosureForwardV1>), ClosureProfileErrorV1> {
        for (environment, count) in self.by_local.values() {
            charge(self.work, 1)?;
            if *count == 0 || (environment.call_kind == ClosureCallKindV1::FnOnce && *count != 1) {
                return Err(ClosureProfileErrorV1::new(format!(
                    "closure local{} has invalid static use count {count} for {:?}",
                    environment.local, environment.call_kind
                )));
            }
        }
        // MIR blocks and source-argument ordinals were visited in ascending order.
        Ok((self.calls, self.forwards))
    }

    fn allowed_closure_assignment(
        &mut self,
        destination: Place<'tcx>,
        value: &Rvalue<'tcx>,
    ) -> Result<bool, ClosureProfileErrorV1> {
        self.charge(1)?;
        let Some(destination) = destination.as_local() else {
            return Ok(false);
        };
        match value {
            Rvalue::Aggregate(kind, operands) => {
                self.charge(1)?;
                if !matches!(&**kind, AggregateKind::Closure(..))
                    || !self.uses.closure_locals.contains(&destination)
                {
                    return Ok(false);
                }
                for operand in operands {
                    if self.operand_mentions_closure(operand)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            Rvalue::Ref(_, _, source) => match source.as_local() {
                Some(source) => self.allowed_alias_assignment(destination, source),
                None => Ok(false),
            },
            Rvalue::Use(operand) => match operand_local(operand, self.work)? {
                Some(source) => self.allowed_alias_assignment(destination, source),
                None => Ok(false),
            },
            _ => Ok(false),
        }
    }

    fn allowed_alias_assignment(
        &mut self,
        destination: Local,
        source: Local,
    ) -> Result<bool, ClosureProfileErrorV1> {
        if source == destination || destination.as_usize() == 0 {
            return Ok(false);
        }
        self.charge(1)?;
        let Some(&expected_root) = self.uses.aliases.get(&destination) else {
            return Ok(false);
        };
        Ok(self.root(source)? == Some(expected_root))
    }

    fn statement_mentions_closure(
        &mut self,
        statement: &StatementKind<'tcx>,
    ) -> Result<bool, ClosureProfileErrorV1> {
        self.charge(1)?;
        match statement {
            StatementKind::Assign(_) => Ok(false),
            StatementKind::FakeRead(contents) => self.place_mentions_closure(contents.1),
            StatementKind::SetDiscriminant { place, .. }
            | StatementKind::Retag(_, place)
            | StatementKind::PlaceMention(place)
            | StatementKind::BackwardIncompatibleDropHint { place, .. } => {
                self.place_mentions_closure(**place)
            }
            StatementKind::AscribeUserType(contents, _) => self.place_mentions_closure(contents.0),
            StatementKind::Intrinsic(intrinsic) => match intrinsic.as_ref() {
                NonDivergingIntrinsic::Assume(operand) => self.operand_mentions_closure(operand),
                NonDivergingIntrinsic::CopyNonOverlapping(copy) => Ok(self
                    .operand_mentions_closure(&copy.src)?
                    || self.operand_mentions_closure(&copy.dst)?
                    || self.operand_mentions_closure(&copy.count)?),
            },
            StatementKind::StorageLive(_)
            | StatementKind::StorageDead(_)
            | StatementKind::Coverage(_)
            | StatementKind::ConstEvalCounter
            | StatementKind::Nop => Ok(false),
        }
    }

    fn assert_message_mentions_closure(
        &mut self,
        message: &AssertMessage<'tcx>,
    ) -> Result<bool, ClosureProfileErrorV1> {
        self.charge(1)?;
        match message {
            AssertKind::BoundsCheck {
                len: left,
                index: right,
            }
            | AssertKind::Overflow(_, left, right)
            | AssertKind::MisalignedPointerDereference {
                required: left,
                found: right,
            } => Ok(self.operand_mentions_closure(left)? || self.operand_mentions_closure(right)?),
            AssertKind::OverflowNeg(operand)
            | AssertKind::DivisionByZero(operand)
            | AssertKind::RemainderByZero(operand)
            | AssertKind::InvalidEnumConstruction(operand) => {
                self.operand_mentions_closure(operand)
            }
            AssertKind::ResumedAfterReturn(_)
            | AssertKind::ResumedAfterPanic(_)
            | AssertKind::ResumedAfterDrop(_)
            | AssertKind::NullPointerDereference => Ok(false),
        }
    }

    fn inline_asm_mentions_closure(
        &mut self,
        operand: &InlineAsmOperand<'tcx>,
    ) -> Result<bool, ClosureProfileErrorV1> {
        self.charge(1)?;
        match operand {
            InlineAsmOperand::In { value, .. } => self.operand_mentions_closure(value),
            InlineAsmOperand::Out {
                place: Some(place), ..
            } => self.place_mentions_closure(*place),
            InlineAsmOperand::InOut {
                in_value,
                out_place,
                ..
            } => {
                if self.operand_mentions_closure(in_value)? {
                    return Ok(true);
                }
                match out_place {
                    Some(place) => self.place_mentions_closure(*place),
                    None => Ok(false),
                }
            }
            InlineAsmOperand::Const { value } => self.constant_mentions_closure(value),
            InlineAsmOperand::Out { place: None, .. }
            | InlineAsmOperand::SymFn { .. }
            | InlineAsmOperand::SymStatic { .. }
            | InlineAsmOperand::Label { .. } => Ok(false),
        }
    }

    fn rvalue_mentions_closure(
        &mut self,
        rvalue: &Rvalue<'tcx>,
    ) -> Result<bool, ClosureProfileErrorV1> {
        self.charge(1)?;
        match rvalue {
            Rvalue::Use(operand)
            | Rvalue::Repeat(operand, _)
            | Rvalue::UnaryOp(_, operand)
            | Rvalue::Cast(_, operand, _)
            | Rvalue::WrapUnsafeBinder(operand, _) => self.operand_mentions_closure(operand),
            Rvalue::Ref(_, _, place)
            | Rvalue::RawPtr(_, place)
            | Rvalue::Discriminant(place)
            | Rvalue::CopyForDeref(place) => self.place_mentions_closure(*place),
            Rvalue::BinaryOp(_, operands) => Ok(self.operand_mentions_closure(&operands.0)?
                || self.operand_mentions_closure(&operands.1)?),
            Rvalue::Aggregate(_, operands) => {
                for operand in operands {
                    if self.operand_mentions_closure(operand)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            Rvalue::ThreadLocalRef(_) => Ok(false),
        }
    }

    fn operand_mentions_closure(
        &mut self,
        operand: &Operand<'tcx>,
    ) -> Result<bool, ClosureProfileErrorV1> {
        self.charge(1)?;
        match operand {
            // A projection changes the accessed value, not the tracked environment root.
            Operand::Copy(place) | Operand::Move(place) => self.place_mentions_closure(*place),
            Operand::Constant(constant) => self.constant_mentions_closure(constant),
            Operand::RuntimeChecks(_) => Ok(false),
        }
    }

    fn constant_mentions_closure(
        &mut self,
        constant: &rustc_middle::mir::ConstOperand<'tcx>,
    ) -> Result<bool, ClosureProfileErrorV1> {
        let mentions = (self.classify_constant)(constant.const_.ty(), self.work)?;
        self.saw_closure_constant |= mentions;
        Ok(mentions)
    }

    fn place_mentions_closure(
        &mut self,
        place: Place<'tcx>,
    ) -> Result<bool, ClosureProfileErrorV1> {
        self.charge(2)?;
        Ok(self.uses.closure_locals.contains(&place.local)
            || self.uses.aliases.contains_key(&place.local))
    }
}

pub(super) fn operand_local(
    operand: &Operand<'_>,
    work: &mut SourceClosureWorkV1,
) -> Result<Option<Local>, ClosureProfileErrorV1> {
    charge(work, 1)?;
    Ok(match operand {
        Operand::Copy(place) | Operand::Move(place) => place.as_local(),
        Operand::Constant(_) | Operand::RuntimeChecks(_) => None,
    })
}

pub(super) fn resolve_alias_root(
    local: Local,
    roots: &BTreeSet<Local>,
    aliases: &BTreeMap<Local, Local>,
    work: &mut SourceClosureWorkV1,
) -> Result<Option<Local>, ClosureProfileErrorV1> {
    charge(work, 1)?;
    if roots.contains(&local) {
        return Ok(Some(local));
    }
    charge(work, 1)?;
    let Some(&root) = aliases.get(&local) else {
        return Ok(None);
    };
    charge(work, 1)?;
    Ok(roots.contains(&root).then_some(root))
}

fn charge(work: &mut SourceClosureWorkV1, amount: usize) -> Result<(), ClosureProfileErrorV1> {
    work.charge(amount)
        .map_err(|error| ClosureProfileErrorV1::new(error.to_string()))
}

#[cfg(test)]
#[path = "uses_v1_tests.rs"]
mod tests;
