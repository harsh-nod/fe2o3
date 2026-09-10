//! Closed authentication of two safe-core Option helpers. This discharges only
//! cross-crate source observation; caller custody and recursive MIR admission
//! remain mandatory, including admission of the concrete callback body.

use rustc_abi::{ExternAbi, FieldIdx, VariantIdx};
use rustc_hir::{Safety, def::DefKind, def_id::DefId};
use rustc_middle::mir::{
    AggregateKind, BasicBlock, Body, Const, ConstValue, Operand, Place, ProjectionElem, Rvalue,
    StatementKind, SwitchTargets, TerminatorKind, UnwindAction, UnwindTerminateReason,
};
use rustc_middle::ty::{EarlyBinder, Instance, InstanceKind, Ty, TyCtxt, TyKind, TypingEnv};

const MAX_LOCALS: usize = 32;
const MAX_BLOCKS: usize = 32;
const MAX_STATEMENTS: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Operation {
    UnwrapOr,
    AndThen,
}

struct Contract<'tcx> {
    operation: Operation,
    option: DefId,
    some: VariantIdx,
    none: VariantIdx,
    some_discriminant: u128,
    none_discriminant: u128,
    input: Ty<'tcx>,
    payload: Ty<'tcx>,
    second: Ty<'tcx>,
    output: Ty<'tcx>,
}

/// No source-name-only or crate-name-only authentication is sufficient here.
pub(crate) fn authenticate_reviewed_safe_core_option_helper_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> bool {
    let Some(contract) = contract(tcx, instance) else {
        return false;
    };
    reviewed_body(tcx, instance, tcx.instance_mir(instance.def), &contract)
}

fn contract<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> Option<Contract<'tcx>> {
    let core = tcx.lang_items().sized_trait()?.krate;
    let option = tcx.lang_items().option_type()?;
    let definition = instance.def_id();
    if !matches!(instance.def, InstanceKind::Item(_))
        || definition.krate != core
        || option.krate != core
        || tcx.crate_name(core).as_str() != "core"
        || !tcx.is_mir_available(definition)
    {
        return None;
    }
    let operation = match tcx.item_name(definition).as_str() {
        "unwrap_or" => Operation::UnwrapOr,
        "and_then" => Operation::AndThen,
        _ => return None,
    };
    let implementation = tcx.impl_of_assoc(definition)?;
    if !tcx.opt_associated_item(definition)?.is_fn()
        || implementation.krate != core
        || tcx.impl_is_of_trait(implementation)
        || instance.args.len()
            != match operation {
                Operation::UnwrapOr => 1,
                Operation::AndThen => 3,
            }
        || instance.args.iter().any(|arg| arg.as_type().is_none())
    {
        return None;
    }
    let input = tcx.type_of(implementation).instantiate(tcx, instance.args);
    let TyKind::Adt(adt, arguments) = input.kind() else {
        return None;
    };
    if adt.did() != option || arguments.len() != 1 || adt.variants().len() != 2 {
        return None;
    }
    let payload = arguments.type_at(0);
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(definition).instantiate(tcx, instance.args),
    );
    let [receiver, second] = signature.inputs() else {
        return None;
    };
    if signature.safety != Safety::Safe
        || signature.abi != ExternAbi::Rust
        || signature.c_variadic
        || *receiver != input
        || payload != instance.args.type_at(0)
    {
        return None;
    }
    let output = signature.output();
    match operation {
        Operation::UnwrapOr if *second == payload && output == payload => {}
        Operation::AndThen
            if *second == instance.args.type_at(2)
                && matches!(second.kind(), TyKind::Closure(..))
                && matches!(output.kind(), TyKind::Adt(result, args)
                    if result.did() == option && args.len() == 1
                        && args.type_at(0) == instance.args.type_at(1)) => {}
        _ => return None,
    }
    if [input, payload, *second, output]
        .iter()
        .any(|ty| ty.needs_drop(tcx, TypingEnv::fully_monomorphized()))
    {
        return None;
    }
    let some = adt
        .variants()
        .iter_enumerated()
        .find(|(_, variant)| Some(variant.def_id) == tcx.lang_items().option_some_variant())?
        .0;
    let none = adt
        .variants()
        .iter_enumerated()
        .find(|(_, variant)| Some(variant.def_id) == tcx.lang_items().option_none_variant())?
        .0;
    if adt.variant(some).fields.len() != 1 || !adt.variant(none).fields.is_empty() {
        return None;
    }
    Some(Contract {
        operation,
        option,
        some,
        none,
        some_discriminant: adt.discriminant_for_variant(tcx, some).val,
        none_discriminant: adt.discriminant_for_variant(tcx, none).val,
        input,
        payload,
        second: *second,
        output,
    })
}

fn normalized_ty<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    ty: Ty<'tcx>,
) -> Option<Ty<'tcx>> {
    instance
        .try_instantiate_mir_and_normalize_erasing_regions(
            tcx,
            TypingEnv::fully_monomorphized(),
            EarlyBinder::bind(ty),
        )
        .ok()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Value {
    Uninitialized,
    InputOption(bool),
    ConsumedSome,
    Payload,
    Default,
    Callback,
    ArgumentTuple,
    CallResult,
    None,
    Integer(u128),
}

struct BodyCheck<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &'a Body<'tcx>,
    contract: &'a Contract<'tcx>,
    local_types: Vec<Ty<'tcx>>,
}

fn reviewed_body<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    contract: &Contract<'tcx>,
) -> bool {
    if body.arg_count != 2
        || !(3..=MAX_LOCALS).contains(&body.local_decls.len())
        || !(1..=MAX_BLOCKS).contains(&body.basic_blocks.len())
        || body
            .basic_blocks
            .iter()
            .map(|block| block.statements.len())
            .sum::<usize>()
            > MAX_STATEMENTS
        || !(1..=32).contains(&body.source_scopes.len())
        || body
            .source_scopes
            .iter()
            .any(|scope| scope.inlined.is_some())
    {
        return false;
    }
    let Some(local_types) = body
        .local_decls
        .iter()
        .map(|declaration| normalized_ty(tcx, instance, declaration.ty))
        .collect::<Option<Vec<_>>>()
    else {
        return false;
    };
    let tuple = Ty::new_tup(tcx, &[contract.payload]);
    let allowed = [
        contract.input,
        contract.payload,
        contract.second,
        contract.output,
        tuple,
        tcx.types.bool,
        tcx.types.isize,
    ];
    if local_types[..3] != [contract.output, contract.input, contract.second]
        || local_types
            .iter()
            .any(|ty| !allowed.contains(ty) || ty.needs_drop(tcx, TypingEnv::fully_monomorphized()))
    {
        return false;
    }
    let check = BodyCheck {
        tcx,
        instance,
        body,
        contract,
        local_types,
    };
    if !check.closed_body_shapes() {
        return false;
    }
    [false, true].into_iter().all(|some| {
        let mut values = vec![Value::Uninitialized; check.local_types.len()];
        values[1] = Value::InputOption(some);
        values[2] = match contract.operation {
            Operation::UnwrapOr => Value::Default,
            Operation::AndThen => Value::Callback,
        };
        check.path(BasicBlock::from_usize(0), values, some, false)
    })
}

impl<'tcx> BodyCheck<'_, 'tcx> {
    fn closed_body_shapes(&self) -> bool {
        let mut calls = 0;
        // Path evaluation cannot inspect operands or edges in dead blocks.
        for block in self.body.basic_blocks.iter() {
            for statement in &block.statements {
                match &statement.kind {
                    StatementKind::Assign(assignment)
                        if self
                            .local_place_ty(assignment.0)
                            .is_some_and(|ty| self.closed_rvalue_ty(&assignment.1) == Some(ty)) => {
                    }
                    StatementKind::SetDiscriminant {
                        place,
                        variant_index,
                    } if place.projection.is_empty()
                        && *variant_index == self.contract.none
                        && self.local_types.get(place.local.as_usize())
                            == Some(&self.contract.output)
                        && self.contract.operation == Operation::AndThen => {}
                    StatementKind::StorageLive(local) | StatementKind::StorageDead(local)
                        if self.local_types.get(local.as_usize()).is_some() => {}
                    StatementKind::Nop => {}
                    _ => return false,
                }
            }
            match block.terminator.as_ref().map(|terminator| &terminator.kind) {
                Some(TerminatorKind::Goto { target })
                    if self.closed_target(*target, block.is_cleanup) => {}
                Some(TerminatorKind::SwitchInt { discr, targets })
                    if self.closed_switch(discr, targets, block.is_cleanup) => {}
                Some(TerminatorKind::Return) if !block.is_cleanup => {}
                Some(TerminatorKind::Unreachable) => {}
                Some(TerminatorKind::UnwindResume) if block.is_cleanup => {}
                Some(TerminatorKind::Drop {
                    place,
                    target,
                    unwind,
                    drop: None,
                    async_fut: None,
                    ..
                }) if self.local_place_ty(*place).is_some()
                    && self.closed_target(*target, block.is_cleanup)
                    && self.closed_unwind(*unwind, block.is_cleanup) => {}
                Some(TerminatorKind::Call {
                    func,
                    args,
                    destination,
                    target: Some(target),
                    unwind,
                    ..
                }) if !block.is_cleanup
                    && self.contract.operation == Operation::AndThen
                    && self.exact_callback(func)
                    && args.len() == 2
                    && self.closed_operand_ty(&args[0].node) == Some(self.contract.second)
                    && self.closed_operand_ty(&args[1].node)
                        == Some(Ty::new_tup(self.tcx, &[self.contract.payload]))
                    && self.local_place_ty(*destination) == Some(self.contract.output)
                    && self.closed_target(*target, false)
                    && self.closed_unwind(*unwind, false) =>
                {
                    calls += 1
                }
                _ => return false,
            }
        }
        calls == usize::from(self.contract.operation == Operation::AndThen) && self.acyclic_cfg()
    }

    fn acyclic_cfg(&self) -> bool {
        // At most 32 blocks and three successors each, including unwind edges.
        let mut finished = [false; MAX_BLOCKS];
        for _ in 0..self.body.basic_blocks.len() {
            let Some((index, _)) =
                self.body
                    .basic_blocks
                    .iter_enumerated()
                    .find(|(index, block)| {
                        !finished[index.as_usize()]
                            && block
                                .terminator()
                                .successors()
                                .all(|next| finished[next.as_usize()])
                    })
            else {
                return false;
            };
            finished[index.as_usize()] = true;
        }
        true
    }

    fn closed_target(&self, target: BasicBlock, cleanup: bool) -> bool {
        self.body
            .basic_blocks
            .get(target)
            .is_some_and(|block| block.is_cleanup == cleanup)
    }

    fn closed_unwind(&self, unwind: UnwindAction, cleanup: bool) -> bool {
        match unwind {
            UnwindAction::Unreachable => true,
            UnwindAction::Continue => !cleanup,
            UnwindAction::Cleanup(target) => !cleanup && self.closed_target(target, true),
            // All retained drops have been proven trivial; preserve the MIR
            // cleanup form without admitting a callback that can terminate.
            UnwindAction::Terminate(UnwindTerminateReason::InCleanup) => cleanup,
            _ => false,
        }
    }

    fn closed_switch(&self, discr: &Operand<'tcx>, targets: &SwitchTargets, cleanup: bool) -> bool {
        let Some(ty) = self.closed_operand_ty(discr) else {
            return false;
        };
        (ty == self.tcx.types.bool || ty == self.tcx.types.isize)
            && (1..=3).contains(&targets.all_targets().len())
            && targets.all_values().len() + 1 == targets.all_targets().len()
            && targets
                .all_targets()
                .iter()
                .all(|target| self.closed_target(*target, cleanup))
            && targets.iter().enumerate().all(|(index, (value, _))| {
                (if ty == self.tcx.types.bool {
                    value <= 1
                } else {
                    value == self.contract.some_discriminant
                        || value == self.contract.none_discriminant
                }) && targets
                    .iter()
                    .take(index)
                    .all(|(earlier, _)| earlier != value)
            })
    }

    fn local_place_ty(&self, place: Place<'tcx>) -> Option<Ty<'tcx>> {
        place
            .projection
            .is_empty()
            .then(|| self.local_types.get(place.local.as_usize()).copied())
            .flatten()
    }

    fn closed_place_ty(&self, place: Place<'tcx>) -> Option<Ty<'tcx>> {
        match place.projection.as_slice() {
            [] => self.local_place_ty(place),
            [
                ProjectionElem::Downcast(_, variant),
                ProjectionElem::Field(field, ty),
            ] => (self.local_types.get(place.local.as_usize()) == Some(&self.contract.input)
                && *variant == self.contract.some
                && field.as_usize() == 0
                && normalized_ty(self.tcx, self.instance, *ty) == Some(self.contract.payload))
            .then_some(self.contract.payload),
            _ => None,
        }
    }

    fn closed_operand_ty(&self, operand: &Operand<'tcx>) -> Option<Ty<'tcx>> {
        match operand {
            Operand::Copy(place) | Operand::Move(place) => self.closed_place_ty(*place),
            Operand::Constant(constant)
                if constant.const_.ty() == self.tcx.types.bool
                    && constant.const_.try_to_bool().is_some() =>
            {
                Some(self.tcx.types.bool)
            }
            _ => None,
        }
    }

    fn closed_rvalue_ty(&self, rvalue: &Rvalue<'tcx>) -> Option<Ty<'tcx>> {
        match rvalue {
            Rvalue::Use(operand) => self.closed_operand_ty(operand),
            Rvalue::Discriminant(place) => (self.local_place_ty(*place)
                == Some(self.contract.input))
            .then_some(self.tcx.types.isize),
            Rvalue::Aggregate(kind, operands) => match &**kind {
                AggregateKind::Tuple => (operands.len() == 1
                    && self.closed_operand_ty(&operands[FieldIdx::from_usize(0)])
                        == Some(self.contract.payload))
                .then(|| Ty::new_tup(self.tcx, &[self.contract.payload])),
                AggregateKind::Adt(definition, variant, args, None, None) => (*definition
                    == self.contract.option
                    && *variant == self.contract.none
                    && operands.is_empty()
                    && normalized_ty(
                        self.tcx,
                        self.instance,
                        Ty::new_adt(self.tcx, self.tcx.adt_def(*definition), args),
                    ) == Some(self.contract.output))
                .then_some(self.contract.output),
                _ => None,
            },
            _ => None,
        }
    }

    fn place(&self, values: &[Value], place: Place<'tcx>) -> Option<Value> {
        let value = *values.get(place.local.as_usize())?;
        match place.projection.as_slice() {
            [] if value != Value::Uninitialized => Some(value),
            [
                ProjectionElem::Downcast(_, variant),
                ProjectionElem::Field(field, ty),
            ] if value == Value::InputOption(true)
                && *variant == self.contract.some
                && field.as_usize() == 0
                && normalized_ty(self.tcx, self.instance, *ty) == Some(self.contract.payload) =>
            {
                Some(Value::Payload)
            }
            _ => None,
        }
    }

    fn operand(&self, values: &mut [Value], operand: &Operand<'tcx>) -> Option<Value> {
        match operand {
            Operand::Copy(place) | Operand::Move(place) => {
                let value = self.place(values, *place)?;
                if value == Value::ConsumedSome {
                    return None;
                }
                if matches!(operand, Operand::Move(_)) {
                    values[place.local.as_usize()] = if place.projection.is_empty() {
                        Value::Uninitialized
                    } else {
                        Value::ConsumedSome
                    };
                }
                Some(value)
            }
            Operand::Constant(constant) if constant.const_.ty() == self.tcx.types.bool => constant
                .const_
                .try_to_bool()
                .map(|value| Value::Integer(u128::from(value))),
            _ => None,
        }
    }

    fn assignment(&self, values: &mut [Value], value: &Rvalue<'tcx>) -> Option<Value> {
        match value {
            Rvalue::Use(operand) => self.operand(values, operand),
            Rvalue::Discriminant(place) => match self.place(values, *place)? {
                Value::InputOption(true) | Value::ConsumedSome => {
                    Some(Value::Integer(self.contract.some_discriminant))
                }
                Value::InputOption(false) => Some(Value::Integer(self.contract.none_discriminant)),
                _ => None,
            },
            Rvalue::Aggregate(kind, operands) => match &**kind {
                AggregateKind::Tuple
                    if operands.len() == 1
                        && self.operand(values, &operands[FieldIdx::from_usize(0)])
                            == Some(Value::Payload) =>
                {
                    Some(Value::ArgumentTuple)
                }
                AggregateKind::Adt(definition, variant, args, None, None)
                    if *definition == self.contract.option
                        && *variant == self.contract.none
                        && operands.is_empty()
                        && normalized_ty(
                            self.tcx,
                            self.instance,
                            Ty::new_adt(self.tcx, self.tcx.adt_def(*definition), args),
                        ) == Some(self.contract.output) =>
                {
                    Some(Value::None)
                }
                _ => None,
            },
            _ => None,
        }
    }

    fn statement(&self, values: &mut [Value], statement: &StatementKind<'tcx>) -> bool {
        match statement {
            StatementKind::Assign(assignment) => {
                let (place, value) = &**assignment;
                if !place.projection.is_empty() {
                    return false;
                }
                let Some(value) = self.assignment(values, value) else {
                    return false;
                };
                let Some(destination) = values.get_mut(place.local.as_usize()) else {
                    return false;
                };
                *destination = value;
                true
            }
            StatementKind::SetDiscriminant {
                place,
                variant_index,
            } if place.projection.is_empty()
                && *variant_index == self.contract.none
                && self.local_types.get(place.local.as_usize()) == Some(&self.contract.output)
                && self.contract.operation == Operation::AndThen =>
            {
                values[place.local.as_usize()] = Value::None;
                true
            }
            StatementKind::StorageLive(local) | StatementKind::StorageDead(local) => {
                let Some(value) = values.get_mut(local.as_usize()) else {
                    return false;
                };
                *value = Value::Uninitialized;
                true
            }
            StatementKind::Nop => true,
            _ => false,
        }
    }

    fn exact_callback(&self, operand: &Operand<'tcx>) -> bool {
        let Operand::Constant(callee) = operand else {
            return false;
        };
        if !matches!(callee.const_, Const::Val(ConstValue::ZeroSized, _)) {
            return false;
        }
        let TyKind::FnDef(definition, arguments) = callee.const_.ty().kind() else {
            return false;
        };
        let Some(fn_once) = self.tcx.lang_items().fn_once_trait() else {
            return false;
        };
        if self.tcx.lang_items().sized_trait().map(|item| item.krate) != Some(fn_once.krate)
            || self.tcx.trait_of_assoc(*definition) != Some(fn_once)
            || self.tcx.item_name(*definition).as_str() != "call_once"
        {
            return false;
        }
        let Ok(arguments) = self
            .instance
            .try_instantiate_mir_and_normalize_erasing_regions(
                self.tcx,
                TypingEnv::fully_monomorphized(),
                EarlyBinder::bind(*arguments),
            )
        else {
            return false;
        };
        let tuple = Ty::new_tup(self.tcx, &[self.contract.payload]);
        if arguments.len() != 2
            || arguments.get(0).and_then(|arg| arg.as_type()) != Some(self.contract.second)
            || arguments.get(1).and_then(|arg| arg.as_type()) != Some(tuple)
        {
            return false;
        }
        let signature = self.tcx.instantiate_bound_regions_with_erased(
            self.tcx
                .fn_sig(*definition)
                .instantiate(self.tcx, arguments),
        );
        let Ok(signature) = self
            .tcx
            .try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), signature)
        else {
            return false;
        };
        if signature.safety != Safety::Safe
            || signature.abi != ExternAbi::RustCall
            || signature.c_variadic
            || signature.inputs() != [self.contract.second, tuple]
            || signature.output() != self.contract.output
        {
            return false;
        }
        let Ok(Some(resolved)) = Instance::try_resolve(
            self.tcx,
            TypingEnv::fully_monomorphized(),
            *definition,
            arguments,
        ) else {
            return false;
        };
        let TyKind::Closure(closure, closure_args) = self.contract.second.kind() else {
            return false;
        };
        match resolved.def {
            InstanceKind::Item(definition) => {
                definition == *closure
                    && resolved.args == *closure_args
                    && self.tcx.def_kind(definition) == DefKind::Closure
            }
            InstanceKind::ClosureOnceShim { call_once, .. } => {
                call_once == *definition && resolved.args == arguments
            }
            _ => false,
        }
    }

    // Each input variant follows a concrete, acyclic path. Callback unwinding is
    // checked separately; no other call, memory effect, or dropping type is admitted.
    fn path(
        &self,
        mut block: BasicBlock,
        mut values: Vec<Value>,
        some: bool,
        cleanup: bool,
    ) -> bool {
        let mut visited = [false; MAX_BLOCKS];
        let mut calls = 0;
        loop {
            let Some(data) = self.body.basic_blocks.get(block) else {
                return false;
            };
            if visited[block.as_usize()] || data.is_cleanup != cleanup {
                return false;
            }
            visited[block.as_usize()] = true;
            if !data
                .statements
                .iter()
                .all(|statement| self.statement(&mut values, &statement.kind))
            {
                return false;
            }
            let Some(terminator) = &data.terminator else {
                return false;
            };
            block = match &terminator.kind {
                TerminatorKind::Goto { target } => *target,
                TerminatorKind::SwitchInt { discr, targets } => {
                    let Some(Value::Integer(value)) = self.operand(&mut values, discr) else {
                        return false;
                    };
                    targets.target_for_value(value)
                }
                TerminatorKind::Drop { place, target, .. }
                    if place.projection.is_empty()
                        && self.local_types.get(place.local.as_usize()).is_some() =>
                {
                    *target
                }
                TerminatorKind::Call {
                    func,
                    args,
                    destination,
                    target: Some(target),
                    unwind,
                    ..
                } if !cleanup
                    && some
                    && self.contract.operation == Operation::AndThen
                    && calls == 0
                    && destination.projection.is_empty()
                    && self.local_types.get(destination.local.as_usize())
                        == Some(&self.contract.output)
                    && args.len() == 2
                    && self.operand(&mut values, &args[0].node) == Some(Value::Callback)
                    && self.operand(&mut values, &args[1].node) == Some(Value::ArgumentTuple)
                    && self.exact_callback(func) =>
                {
                    match unwind {
                        UnwindAction::Continue | UnwindAction::Unreachable => {}
                        UnwindAction::Cleanup(target)
                            if self.path(*target, values.clone(), some, true) => {}
                        _ => return false,
                    }
                    calls += 1;
                    values[destination.local.as_usize()] = Value::CallResult;
                    *target
                }
                TerminatorKind::Return if !cleanup => {
                    let (expected, expected_calls) = match (self.contract.operation, some) {
                        (Operation::UnwrapOr, false) => (Value::Default, 0),
                        (Operation::UnwrapOr, true) => (Value::Payload, 0),
                        (Operation::AndThen, false) => (Value::None, 0),
                        (Operation::AndThen, true) => (Value::CallResult, 1),
                    };
                    return values[0] == expected && calls == expected_calls;
                }
                TerminatorKind::UnwindResume if cleanup => return true,
                _ => return false,
            };
        }
    }
}

#[cfg(test)]
mod tests;
