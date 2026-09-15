//! Bounded source-safety authentication, not terminal or callee admission.
//! The collector must retain these bodies and recursively check every call,
//! including a user implementation of `From::from` reached from Result.

use rustc_abi::{ExternAbi, VariantIdx};
use rustc_hir::{Safety, def::DefKind, def_id::DefId};
use rustc_middle::mir::{
    AggregateKind, BasicBlock, BinOp, Body, Const, ConstValue, NonDivergingIntrinsic, Operand,
    Place, ProjectionElem, Rvalue, StatementKind, SwitchTargets, TerminatorKind, UnwindAction,
    UnwindTerminateReason,
};
use rustc_middle::ty::{
    EarlyBinder, Instance, InstanceKind, Ty, TyCtxt, TyKind, TypeVisitableExt, TypingEnv,
};
use rustc_span::Symbol;

const MAX_LOCALS: usize = 32;
const MAX_BLOCKS: usize = 32;
const MAX_STATEMENTS: usize = 30;
const MAX_SCOPES: usize = 32;

// O(MAX_BLOCKS^2 + MAX_STATEMENTS) MIR work; at most two symbolic paths,
// counting conversion cleanup. Rustc owns normalization and trait-query limits.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Operation {
    ResultFromResidual,
    OptionFromResidual,
    OptionOkOr,
    OptionBranch,
    ResultBranch,
}

struct Contract<'tcx> {
    operation: Operation,
    input: Ty<'tcx>,
    output: Ty<'tcx>,
    payload: Ty<'tcx>,
    converted: Ty<'tcx>,
    residual: Ty<'tcx>,
    error: Option<Ty<'tcx>>,
    input_payload_variant: VariantIdx,
    input_break_variant: VariantIdx,
    payload_discriminant: u128,
    empty_discriminant: u128,
    output_payload_variant: VariantIdx,
    output_break_variant: VariantIdx,
    residual_variant: VariantIdx,
    conversion: Option<(DefId, Instance<'tcx>)>,
}

/// Discharges only the core wrapper's source observation. The caller must still
/// collect the wrapper and recursively admit the exact concrete From callee.
pub(crate) fn authenticate_reviewed_safe_core_result_try_helper_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> bool {
    let Some(contract) = contract(tcx, instance) else {
        return false;
    };
    let body = tcx.instance_mir(instance.def);
    reviewed_body(tcx, instance, body, &contract)
}

fn normalize<'tcx, T>(tcx: TyCtxt<'tcx>, value: T) -> Option<T>
where
    T: rustc_middle::ty::TypeFoldable<TyCtxt<'tcx>>,
{
    tcx.try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), value)
        .ok()
}

fn normalized_ty<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    ty: Ty<'tcx>,
) -> Option<Ty<'tcx>> {
    let ty = instance
        .try_instantiate_mir_and_normalize_erasing_regions(
            tcx,
            TypingEnv::fully_monomorphized(),
            EarlyBinder::bind(ty),
        )
        .ok()?;
    (!ty.has_non_region_param()
        && !ty.has_infer()
        && !ty.has_aliases()
        && !ty.has_escaping_bound_vars())
    .then_some(ty)
}

fn variant(tcx: TyCtxt<'_>, ty: Ty<'_>, definition: DefId, fields: usize) -> Option<VariantIdx> {
    let TyKind::Adt(adt, _) = ty.kind() else {
        return None;
    };
    let (index, variant) = adt
        .variants()
        .iter_enumerated()
        .find(|(_, v)| v.def_id == definition)?;
    (adt.is_enum()
        && adt.variants().len() == 2
        && variant.fields.len() == fields
        && tcx.parent(definition) == adt.did())
    .then_some(index)
}

fn adt<'tcx>(tcx: TyCtxt<'tcx>, definition: DefId, args: &[Ty<'tcx>]) -> Ty<'tcx> {
    Ty::new_adt(
        tcx,
        tcx.adt_def(definition),
        tcx.mk_args_from_iter(args.iter().copied().map(rustc_middle::ty::GenericArg::from)),
    )
}

fn contract<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> Option<Contract<'tcx>> {
    let core = tcx.lang_items().sized_trait()?.krate;
    let definition = instance.def_id();
    if !matches!(instance.def, InstanceKind::Item(_))
        || definition.krate != core
        || tcx.crate_name(core).as_str() != "core"
        || !tcx.is_mir_available(definition)
    {
        return None;
    }
    let item = tcx.opt_associated_item(definition)?;
    let trait_item = item.trait_item_def_id();
    let operation = if trait_item.is_some() && trait_item == tcx.lang_items().from_residual_fn() {
        match instance.args.len() {
            1 => Operation::OptionFromResidual,
            3 => Operation::ResultFromResidual,
            _ => return None,
        }
    } else if trait_item.is_some() && trait_item == tcx.lang_items().branch_fn() {
        match instance.args.len() {
            1 => Operation::OptionBranch,
            2 => Operation::ResultBranch,
            _ => return None,
        }
    } else if trait_item.is_none() && item.name().as_str() == "ok_or" {
        Operation::OptionOkOr
    } else {
        return None;
    };
    if !item.is_fn()
        || trait_item.is_some_and(|id| id.krate != core)
        || instance.args.len()
            != match operation {
                Operation::ResultFromResidual => 3,
                Operation::OptionBranch | Operation::OptionFromResidual => 1,
                Operation::ResultBranch | Operation::OptionOkOr => 2,
            }
        || instance.args.iter().any(|arg| arg.as_type().is_none())
    {
        return None;
    }
    let args = normalize(tcx, instance.args)?;
    if args.has_non_region_param()
        || args.has_infer()
        || args.has_aliases()
        || args.has_escaping_bound_vars()
    {
        return None;
    }
    let implementation = tcx.impl_of_assoc(definition)?;
    if implementation.krate != core {
        return None;
    }
    let option = tcx.lang_items().option_type()?;
    let trait_ref = if let Some(trait_item) = trait_item {
        if !tcx.impl_is_of_trait(implementation) {
            return None;
        }
        let trait_ref = normalize(
            tcx,
            tcx.impl_trait_ref(implementation).instantiate(tcx, args),
        )?;
        if tcx.trait_of_assoc(trait_item) != Some(trait_ref.def_id) {
            return None;
        }
        Some(trait_ref)
    } else {
        if operation != Operation::OptionOkOr
            || tcx.impl_is_of_trait(implementation)
            || !tcx.inherent_impls(option).contains(&implementation)
            || tcx
                .associated_items(implementation)
                .in_definition_order()
                .find(|item| item.is_fn() && item.name().as_str() == "ok_or")?
                .def_id
                != definition
        {
            return None;
        }
        None
    };
    let result = tcx.parent(tcx.lang_items().result_ok_variant()?);
    let control_flow = tcx.parent(tcx.lang_items().cf_continue_variant()?);
    if [option, result, control_flow]
        .iter()
        .any(|id| id.krate != core)
    {
        return None;
    }
    // Infallible has no lang/diagnostic item on the pinned compiler. Resolve its
    // definition in the actual core From trait's owning module, not by crate text.
    let from_trait = tcx.get_diagnostic_item(Symbol::intern("From"))?;
    if from_trait.krate != core || tcx.def_kind(from_trait) != DefKind::Trait {
        return None;
    }
    let convert = tcx.parent(from_trait);
    let infallible = tcx.module_children(convert).iter().find_map(|child| {
        let id = child.res.opt_def_id()?;
        (child.ident.name.as_str() == "Infallible"
            && tcx.parent(id) == convert
            && id.krate == core
            && tcx.def_kind(id) == DefKind::Enum)
            .then_some(id)
    })?;
    if !tcx.adt_def(infallible).variants().is_empty() || tcx.generics_of(infallible).count() != 0 {
        return None;
    }
    let never = adt(tcx, infallible, &[]);
    let (residual, residual_variant, error) = if operation == Operation::ResultBranch {
        let error = args.type_at(1);
        let residual = adt(tcx, result, &[never, error]);
        (
            residual,
            variant(tcx, residual, tcx.lang_items().result_err_variant()?, 1)?,
            Some(error),
        )
    } else {
        let residual = adt(tcx, option, &[never]);
        (
            residual,
            variant(tcx, residual, tcx.lang_items().option_none_variant()?, 0)?,
            (operation == Operation::OptionOkOr).then(|| args.type_at(1)),
        )
    };
    let (
        input,
        output,
        payload,
        converted,
        input_payload,
        input_empty,
        out_payload,
        out_break,
        conversion,
    ) = match operation {
        Operation::ResultFromResidual => {
            let trait_ref = trait_ref?;
            let (t, e, f) = (args.type_at(0), args.type_at(1), args.type_at(2));
            let input = adt(tcx, result, &[never, e]);
            let output = adt(tcx, result, &[t, f]);
            let from_residual = tcx.get_diagnostic_item(Symbol::intern("FromResidual"))?;
            if trait_ref.def_id != from_residual
                || from_residual.krate != core
                || trait_ref.args.len() != 2
                || trait_ref.args.type_at(0) != output
                || trait_ref.args.type_at(1) != input
            {
                return None;
            }
            let from = tcx
                .associated_items(from_trait)
                .in_definition_order()
                .find(|item| item.is_fn() && item.name().as_str() == "from")?
                .def_id;
            let from_args = tcx.mk_args(&[f.into(), e.into()]);
            let resolved =
                Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), from, from_args)
                    .ok()??;
            if !matches!(resolved.def, InstanceKind::Item(_))
                || !tcx.is_mir_available(resolved.def_id())
                || tcx
                    .opt_associated_item(resolved.def_id())?
                    .trait_item_def_id()
                    != Some(from)
                || !safe_signature(tcx, from, from_args, [e], f)
                || !safe_signature(tcx, resolved.def_id(), resolved.args, [e], f)
            {
                return None;
            }
            let err = tcx.lang_items().result_err_variant()?;
            let ok = tcx.lang_items().result_ok_variant()?;
            (
                input,
                output,
                e,
                f,
                variant(tcx, input, err, 1)?,
                variant(tcx, input, ok, 1)?,
                variant(tcx, output, err, 1)?,
                variant(tcx, output, ok, 1)?,
                Some((from, resolved)),
            )
        }
        Operation::OptionFromResidual => {
            let trait_ref = trait_ref?;
            let output = adt(tcx, option, &[args.type_at(0)]);
            let from_residual = tcx.get_diagnostic_item(Symbol::intern("FromResidual"))?;
            if trait_ref.def_id != from_residual
                || from_residual.krate != core
                || trait_ref.args.len() != 2
                || trait_ref.self_ty() != output
                || trait_ref.args.type_at(1) != residual
            {
                return None;
            }
            (
                residual,
                output,
                never,
                args.type_at(0),
                variant(tcx, residual, tcx.lang_items().option_some_variant()?, 1)?,
                variant(tcx, residual, tcx.lang_items().option_none_variant()?, 0)?,
                variant(tcx, output, tcx.lang_items().option_some_variant()?, 1)?,
                variant(tcx, output, tcx.lang_items().option_none_variant()?, 0)?,
                None,
            )
        }
        Operation::OptionOkOr => {
            let payload = args.type_at(0);
            let input = adt(tcx, option, &[payload]);
            let output = adt(tcx, result, &[payload, error?]);
            (
                input,
                output,
                payload,
                payload,
                variant(tcx, input, tcx.lang_items().option_some_variant()?, 1)?,
                variant(tcx, input, tcx.lang_items().option_none_variant()?, 0)?,
                variant(tcx, output, tcx.lang_items().result_ok_variant()?, 1)?,
                variant(tcx, output, tcx.lang_items().result_err_variant()?, 1)?,
                None,
            )
        }
        Operation::OptionBranch => {
            let trait_ref = trait_ref?;
            let payload = args.type_at(0);
            let input = adt(tcx, option, &[payload]);
            let output = adt(tcx, control_flow, &[residual, payload]);
            if Some(trait_ref.def_id) != tcx.lang_items().try_trait()
                || trait_ref.args.len() != 1
                || trait_ref.self_ty() != input
            {
                return None;
            }
            (
                input,
                output,
                payload,
                payload,
                variant(tcx, input, tcx.lang_items().option_some_variant()?, 1)?,
                variant(tcx, input, tcx.lang_items().option_none_variant()?, 0)?,
                variant(tcx, output, tcx.lang_items().cf_continue_variant()?, 1)?,
                variant(tcx, output, tcx.lang_items().cf_break_variant()?, 1)?,
                None,
            )
        }
        Operation::ResultBranch => {
            let trait_ref = trait_ref?;
            let payload = args.type_at(0);
            let input = adt(tcx, result, &[payload, error?]);
            let output = adt(tcx, control_flow, &[residual, payload]);
            if Some(trait_ref.def_id) != tcx.lang_items().try_trait()
                || trait_ref.args.len() != 1
                || trait_ref.self_ty() != input
            {
                return None;
            }
            (
                input,
                output,
                payload,
                payload,
                variant(tcx, input, tcx.lang_items().result_ok_variant()?, 1)?,
                variant(tcx, input, tcx.lang_items().result_err_variant()?, 1)?,
                variant(tcx, output, tcx.lang_items().cf_continue_variant()?, 1)?,
                variant(tcx, output, tcx.lang_items().cf_break_variant()?, 1)?,
                None,
            )
        }
    };
    let expected_self = trait_ref.map_or(input, |trait_ref| trait_ref.self_ty());
    if normalize(tcx, tcx.type_of(implementation).instantiate(tcx, args))? != expected_self
        || [input, output, payload, converted, residual]
            .iter()
            .any(|ty| ty.needs_drop(tcx, TypingEnv::fully_monomorphized()))
    {
        return None;
    }
    if let (Some(trait_item), Some(trait_ref)) = (trait_item, trait_ref) {
        if !safe_signature(tcx, definition, args, [input], output)
            || !safe_signature(tcx, trait_item, trait_ref.args, [input], output)
        {
            return None;
        }
        let resolved = Instance::try_resolve(
            tcx,
            TypingEnv::fully_monomorphized(),
            trait_item,
            trait_ref.args,
        )
        .ok()??;
        if resolved.def != instance.def || normalize(tcx, resolved.args)? != args {
            return None;
        }
    } else if operation != Operation::OptionOkOr
        || !safe_signature(tcx, definition, args, [input, error?], output)
    {
        return None;
    }
    let TyKind::Adt(input_adt, _) = input.kind() else {
        return None;
    };
    Some(Contract {
        operation,
        input,
        output,
        payload,
        converted,
        residual,
        error,
        input_payload_variant: input_payload,
        input_break_variant: input_empty,
        payload_discriminant: input_adt.discriminant_for_variant(tcx, input_payload).val,
        empty_discriminant: input_adt.discriminant_for_variant(tcx, input_empty).val,
        output_payload_variant: out_payload,
        output_break_variant: out_break,
        residual_variant,
        conversion,
    })
}

fn safe_signature<'tcx, const N: usize>(
    tcx: TyCtxt<'tcx>,
    definition: DefId,
    args: rustc_middle::ty::GenericArgsRef<'tcx>,
    inputs: [Ty<'tcx>; N],
    output: Ty<'tcx>,
) -> bool {
    let signature =
        tcx.instantiate_bound_regions_with_erased(tcx.fn_sig(definition).instantiate(tcx, args));
    normalize(tcx, signature).is_some_and(|signature| {
        signature.safety == Safety::Safe
            && signature.abi == ExternAbi::Rust
            && !signature.c_variadic
            && !signature.has_aliases()
            && !signature.has_non_region_param()
            && !signature.has_infer()
            && !signature.has_escaping_bound_vars()
            && signature.inputs() == inputs
            && signature.output() == output
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Value {
    Uninitialized,
    Input(bool),
    ConsumedInput(bool),
    Payload,
    Error,
    Converted,
    ResidualNone,
    ResidualError,
    ReturnPayload,
    ReturnBreak,
    Discriminant(u128),
    Integer(u128),
    ProvenInhabitedInput,
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
    let argument_count = if contract.operation == Operation::OptionOkOr {
        2
    } else {
        1
    };
    if body.arg_count != argument_count
        || !(argument_count + 1..=MAX_LOCALS).contains(&body.local_decls.len())
        || !(1..=MAX_BLOCKS).contains(&body.basic_blocks.len())
        || body
            .basic_blocks
            .iter()
            .map(|b| b.statements.len())
            .sum::<usize>()
            > MAX_STATEMENTS
        || !(1..=MAX_SCOPES).contains(&body.source_scopes.len())
        || body
            .source_scopes
            .iter()
            .any(|s| s.inlined.is_some() || s.inlined_parent_scope.is_some())
        || body.coroutine.is_some()
        || body.spread_arg.is_some()
        || body.tainted_by_errors.is_some()
        || body.source.instance != instance.def
        || body.source.promoted.is_some()
    {
        return false;
    }
    let Some(local_types) = body
        .local_decls
        .iter()
        .map(|d| normalized_ty(tcx, instance, d.ty))
        .collect::<Option<Vec<_>>>()
    else {
        return false;
    };
    let allowed = [
        contract.input,
        contract.output,
        contract.payload,
        contract.converted,
        contract.residual,
        tcx.types.isize,
        tcx.types.bool,
    ];
    if local_types[..2] != [contract.output, contract.input]
        || (contract.operation == Operation::OptionOkOr && Some(local_types[2]) != contract.error)
        || local_types.iter().any(|ty| {
            (!allowed.contains(ty) && Some(*ty) != contract.error)
                || ty.needs_drop(tcx, TypingEnv::fully_monomorphized())
        })
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
    let mut visited = [false; MAX_BLOCKS];
    let variants: &[bool] = match contract.operation {
        Operation::ResultFromResidual => &[true],
        Operation::OptionFromResidual => &[false],
        _ => &[false, true],
    };
    for &payload in variants {
        let mut values = vec![Value::Uninitialized; check.local_types.len()];
        values[1] = Value::Input(payload);
        if contract.operation == Operation::OptionOkOr {
            values[2] = Value::Error;
        }
        if !check.path(
            BasicBlock::from_usize(0),
            values,
            payload,
            false,
            &mut visited,
        ) {
            return false;
        }
    }
    // The only unvisited blocks admitted are empty impossible/unwind sinks.
    // In particular, assumptions, moves, and calls cannot hide in dead code.
    body.basic_blocks.iter_enumerated().all(|(index, block)| {
        visited[index.as_usize()]
            || (block.statements.is_empty()
                && matches!(
                    block.terminator().kind,
                    TerminatorKind::Unreachable | TerminatorKind::UnwindResume
                ))
    })
}

impl<'tcx> BodyCheck<'_, 'tcx> {
    fn inhabited_input_discriminant(&self) -> Option<u128> {
        match self.contract.operation {
            Operation::ResultFromResidual => Some(self.contract.payload_discriminant),
            Operation::OptionFromResidual => Some(self.contract.empty_discriminant),
            _ => None,
        }
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
            ] if self.local_types.get(place.local.as_usize()) == Some(&self.contract.input)
                && field.as_usize() == 0 =>
            {
                let expected = if *variant == self.contract.input_payload_variant {
                    self.contract.payload
                } else if *variant == self.contract.input_break_variant
                    && self.contract.operation == Operation::ResultBranch
                {
                    self.contract.error?
                } else {
                    return None;
                };
                (normalized_ty(self.tcx, self.instance, *ty) == Some(expected)).then_some(expected)
            }
            _ => None,
        }
    }

    fn constant(&self, operand: &Operand<'tcx>) -> Option<(Ty<'tcx>, Value)> {
        let Operand::Constant(constant) = operand else {
            return None;
        };
        let Const::Val(value, ty) = constant.const_ else {
            return None;
        };
        let ty = normalized_ty(self.tcx, self.instance, ty)?;
        if ty == self.contract.residual
            && value == ConstValue::ZeroSized
            && self.contract.operation == Operation::OptionBranch
        {
            return Some((ty, Value::ResidualNone));
        }
        let integer = if ty == self.tcx.types.bool {
            u128::from(constant.const_.try_to_bool()?)
        } else if ty == self.tcx.types.isize {
            constant
                .const_
                .try_to_scalar_int()?
                .try_to_bits(self.tcx.data_layout.pointer_size())
                .ok()?
        } else {
            return None;
        };
        (integer <= 1).then_some((ty, Value::Integer(integer)))
    }

    fn closed_operand_ty(&self, operand: &Operand<'tcx>) -> Option<Ty<'tcx>> {
        match operand {
            Operand::Copy(place) | Operand::Move(place) => self.closed_place_ty(*place),
            Operand::Constant(_) => self.constant(operand).map(|(ty, _)| ty),
            _ => None,
        }
    }

    fn aggregate(
        &self,
        kind: &AggregateKind<'tcx>,
        operands: &[Operand<'tcx>],
    ) -> Option<(Ty<'tcx>, Value)> {
        let AggregateKind::Adt(definition, variant, args, None, None) = kind else {
            return None;
        };
        // Check DefId before querying adt_def on a potentially mutated aggregate.
        let TyKind::Adt(output, _) = self.contract.output.kind() else {
            return None;
        };
        let TyKind::Adt(residual, _) = self.contract.residual.kind() else {
            return None;
        };
        if *definition != output.did() && *definition != residual.did() {
            return None;
        }
        if args.len() != self.tcx.generics_of(*definition).count()
            || args.iter().any(|arg| arg.as_type().is_none())
        {
            return None;
        }
        let ty = normalized_ty(
            self.tcx,
            self.instance,
            Ty::new_adt(self.tcx, self.tcx.adt_def(*definition), args),
        )?;
        if self.contract.operation == Operation::OptionFromResidual {
            return (ty == self.contract.output
                && *variant == self.contract.output_break_variant
                && operands.is_empty())
            .then_some((ty, Value::ReturnBreak));
        }
        if self.contract.operation == Operation::OptionBranch
            && ty == self.contract.residual
            && *variant == self.contract.residual_variant
            && operands.is_empty()
        {
            return Some((ty, Value::ResidualNone));
        }
        if self.contract.operation == Operation::ResultBranch
            && ty == self.contract.residual
            && *variant == self.contract.residual_variant
            && operands.len() == 1
            && self.closed_operand_ty(&operands[0]) == self.contract.error
        {
            return Some((ty, Value::ResidualError));
        }
        if ty != self.contract.output || operands.len() != 1 {
            return None;
        }
        let (payload, value) = if *variant == self.contract.output_payload_variant {
            (self.contract.converted, Value::ReturnPayload)
        } else if self.contract.operation != Operation::ResultFromResidual
            && *variant == self.contract.output_break_variant
        {
            (
                if self.contract.operation == Operation::OptionOkOr {
                    self.contract.error?
                } else {
                    self.contract.residual
                },
                Value::ReturnBreak,
            )
        } else {
            return None;
        };
        (self.closed_operand_ty(&operands[0]) == Some(payload)).then_some((ty, value))
    }

    fn closed_rvalue_ty(&self, value: &Rvalue<'tcx>) -> Option<Ty<'tcx>> {
        match value {
            Rvalue::Use(operand) => self.closed_operand_ty(operand),
            Rvalue::Discriminant(place) => (self.local_place_ty(*place)
                == Some(self.contract.input))
            .then_some(self.tcx.types.isize),
            Rvalue::BinaryOp(BinOp::Eq, operands)
                if self.inhabited_input_discriminant().is_some()
                    && self.closed_operand_ty(&operands.0) == Some(self.tcx.types.isize)
                    && self.closed_operand_ty(&operands.1) == Some(self.tcx.types.isize) =>
            {
                Some(self.tcx.types.bool)
            }
            Rvalue::Aggregate(kind, operands) => {
                self.aggregate(kind, &operands.raw).map(|(ty, _)| ty)
            }
            _ => None,
        }
    }

    fn exact_conversion(&self, func: &Operand<'tcx>) -> bool {
        let Some((method, expected)) = self.contract.conversion else {
            return false;
        };
        let Operand::Constant(callee) = func else {
            return false;
        };
        let Const::Val(ConstValue::ZeroSized, ty) = callee.const_ else {
            return false;
        };
        let TyKind::FnDef(definition, args) = ty.kind() else {
            return false;
        };
        if *definition != method
            || args.len() != 2
            || args.iter().any(|arg| arg.as_type().is_none())
        {
            return false;
        }
        let Some(ty) = normalized_ty(self.tcx, self.instance, ty) else {
            return false;
        };
        let TyKind::FnDef(definition, args) = ty.kind() else {
            return false;
        };
        *definition == method
            && args.len() == 2
            && args.get(0).and_then(|a| a.as_type()) == Some(self.contract.converted)
            && args.get(1).and_then(|a| a.as_type()) == Some(self.contract.payload)
            && matches!(Instance::try_resolve(self.tcx, TypingEnv::fully_monomorphized(), *definition, args), Ok(Some(actual)) if actual == expected)
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
            UnwindAction::Terminate(UnwindTerminateReason::InCleanup) => cleanup,
            _ => false,
        }
    }

    fn closed_conversion_unwind(&self, unwind: UnwindAction) -> bool {
        match unwind {
            // Abort targets cannot unwind across this call. This says nothing
            // about the callee's body, which still requires recursive admission.
            UnwindAction::Unreachable => !self.tcx.sess.panic_strategy().unwinds(),
            UnwindAction::Continue | UnwindAction::Cleanup(_) => self.closed_unwind(unwind, false),
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
                .all(|t| self.closed_target(*t, cleanup))
            && targets.iter().enumerate().all(|(i, (value, _))| {
                (if ty == self.tcx.types.bool {
                    value <= 1
                } else {
                    value == self.contract.payload_discriminant
                        || value == self.contract.empty_discriminant
                }) && targets.iter().take(i).all(|(earlier, _)| earlier != value)
            })
    }

    fn closed_body_shapes(&self) -> bool {
        let mut calls = 0;
        for block in self.body.basic_blocks.iter() {
            for statement in &block.statements {
                match &statement.kind {
                    StatementKind::Assign(assignment)
                        if self
                            .local_place_ty(assignment.0)
                            .is_some_and(|ty| self.closed_rvalue_ty(&assignment.1) == Some(ty)) => {
                    }
                    StatementKind::Intrinsic(intrinsic)
                        if matches!(&**intrinsic, NonDivergingIntrinsic::Assume(operand)
                        if self.inhabited_input_discriminant().is_some() && !block.is_cleanup
                            && self.closed_operand_ty(operand) == Some(self.tcx.types.bool)) => {}
                    StatementKind::StorageLive(local) | StatementKind::StorageDead(local)
                        if self.local_types.get(local.as_usize()).is_some() => {}
                    StatementKind::Nop => {}
                    _ => return false,
                }
            }
            match block.terminator.as_ref().map(|t| &t.kind) {
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
                    && self.exact_conversion(func)
                    && args.len() == 1
                    && self.closed_operand_ty(&args[0].node) == Some(self.contract.payload)
                    && self.local_place_ty(*destination) == Some(self.contract.converted)
                    && self.closed_target(*target, false)
                    && self.closed_conversion_unwind(*unwind) =>
                {
                    calls += 1;
                }
                _ => return false,
            }
        }
        if calls != usize::from(self.contract.operation == Operation::ResultFromResidual) {
            return false;
        }
        // Every edge is in range before indexing, including dead/cleanup edges.
        let mut finished = [false; MAX_BLOCKS];
        for _ in 0..self.body.basic_blocks.len() {
            let Some((index, _)) = self.body.basic_blocks.iter_enumerated().find(|(i, b)| {
                !finished[i.as_usize()]
                    && b.terminator()
                        .successors()
                        .all(|next| finished[next.as_usize()])
            }) else {
                return false;
            };
            finished[index.as_usize()] = true;
        }
        true
    }

    fn place(&self, values: &[Value], place: Place<'tcx>) -> Option<Value> {
        self.closed_place_ty(place)?;
        let value = *values.get(place.local.as_usize())?;
        if place.projection.is_empty() {
            (value != Value::Uninitialized).then_some(value)
        } else {
            let ProjectionElem::Downcast(_, variant) = place.projection[0] else {
                return None;
            };
            match value {
                Value::Input(true) if variant == self.contract.input_payload_variant => {
                    Some(Value::Payload)
                }
                Value::Input(false)
                    if variant == self.contract.input_break_variant
                        && self.contract.error.is_some() =>
                {
                    Some(Value::Error)
                }
                _ => None,
            }
        }
    }

    fn operand(&self, values: &mut [Value], operand: &Operand<'tcx>) -> Option<Value> {
        match operand {
            Operand::Copy(place) | Operand::Move(place) => {
                let value = self.place(values, *place)?;
                if matches!(value, Value::ConsumedInput(_)) {
                    return None;
                }
                if matches!(operand, Operand::Move(_)) {
                    values[place.local.as_usize()] = if place.projection.is_empty() {
                        Value::Uninitialized
                    } else {
                        Value::ConsumedInput(value == Value::Payload)
                    };
                }
                Some(value)
            }
            Operand::Constant(_) => self.constant(operand).map(|(_, value)| value),
            _ => None,
        }
    }

    fn assignment(&self, values: &mut [Value], value: &Rvalue<'tcx>) -> Option<Value> {
        match value {
            Rvalue::Use(operand) => self.operand(values, operand),
            Rvalue::Discriminant(place) => match self.place(values, *place)? {
                Value::Input(payload) | Value::ConsumedInput(payload) => {
                    Some(Value::Discriminant(if payload {
                        self.contract.payload_discriminant
                    } else {
                        self.contract.empty_discriminant
                    }))
                }
                _ => None,
            },
            Rvalue::BinaryOp(BinOp::Eq, operands) => {
                let lhs = self.operand(values, &operands.0)?;
                let rhs = self.operand(values, &operands.1)?;
                let discriminant = self.inhabited_input_discriminant()?;
                (lhs == Value::Discriminant(discriminant) && rhs == Value::Integer(discriminant))
                    .then_some(Value::ProvenInhabitedInput)
            }
            Rvalue::Aggregate(kind, operands) => {
                let (_, value) = self.aggregate(kind, &operands.raw)?;
                let expected = match value {
                    Value::ReturnPayload
                        if self.contract.operation == Operation::ResultFromResidual =>
                    {
                        Value::Converted
                    }
                    Value::ReturnPayload => Value::Payload,
                    Value::ReturnBreak if self.contract.operation == Operation::ResultBranch => {
                        Value::ResidualError
                    }
                    Value::ReturnBreak if self.contract.operation == Operation::OptionOkOr => {
                        Value::Error
                    }
                    Value::ReturnBreak
                        if self.contract.operation == Operation::OptionFromResidual =>
                    {
                        return Some(value);
                    }
                    Value::ReturnBreak => Value::ResidualNone,
                    Value::ResidualError => Value::Error,
                    Value::ResidualNone => return Some(value),
                    _ => return None,
                };
                (self.operand(values, &operands.raw[0]) == Some(expected)).then_some(value)
            }
            _ => None,
        }
    }

    fn statement(&self, values: &mut [Value], statement: &StatementKind<'tcx>) -> bool {
        match statement {
            StatementKind::Assign(assignment) => {
                let Some(value) = self.assignment(values, &assignment.1) else {
                    return false;
                };
                values[assignment.0.local.as_usize()] = value;
                true
            }
            StatementKind::Intrinsic(intrinsic) => {
                matches!(&**intrinsic, NonDivergingIntrinsic::Assume(operand)
                if self.operand(values, operand) == Some(Value::ProvenInhabitedInput))
            }
            StatementKind::StorageLive(local) | StatementKind::StorageDead(local) => {
                values[local.as_usize()] = Value::Uninitialized;
                true
            }
            StatementKind::Nop => true,
            _ => false,
        }
    }

    fn path(
        &self,
        mut block: BasicBlock,
        mut values: Vec<Value>,
        payload: bool,
        cleanup: bool,
        visited: &mut [bool; MAX_BLOCKS],
    ) -> bool {
        let mut calls = 0;
        for _ in 0..MAX_BLOCKS {
            let data = &self.body.basic_blocks[block];
            if data.is_cleanup != cleanup {
                return false;
            }
            visited[block.as_usize()] = true;
            if !data
                .statements
                .iter()
                .all(|s| self.statement(&mut values, &s.kind))
            {
                return false;
            }
            block = match &data.terminator().kind {
                TerminatorKind::Goto { target } => *target,
                TerminatorKind::SwitchInt { discr, targets } => {
                    let Some(Value::Integer(value) | Value::Discriminant(value)) =
                        self.operand(&mut values, discr)
                    else {
                        return false;
                    };
                    targets.target_for_value(value)
                }
                TerminatorKind::Drop { place, target, .. } => {
                    // No-drop types only, but a drop still consumes its place.
                    if self.operand(&mut values, &Operand::Move(*place)).is_none() {
                        return false;
                    }
                    *target
                }
                TerminatorKind::Call {
                    args,
                    destination,
                    target: Some(target),
                    unwind,
                    ..
                } if !cleanup
                    && calls == 0
                    && self.contract.operation == Operation::ResultFromResidual
                    && self.operand(&mut values, &args[0].node) == Some(Value::Payload) =>
                {
                    values[destination.local.as_usize()] = Value::Uninitialized;
                    if let UnwindAction::Cleanup(target) = unwind {
                        if !self.path(*target, values.clone(), payload, true, visited) {
                            return false;
                        }
                    }
                    calls += 1;
                    values[destination.local.as_usize()] = Value::Converted;
                    *target
                }
                TerminatorKind::Return if !cleanup => {
                    return values[0]
                        == if payload {
                            Value::ReturnPayload
                        } else {
                            Value::ReturnBreak
                        }
                        && calls
                            == usize::from(
                                self.contract.operation == Operation::ResultFromResidual,
                            );
                }
                TerminatorKind::UnwindResume if cleanup => return true,
                _ => return false,
            };
        }
        false
    }
}

#[cfg(test)]
#[path = "core_result_try_v1/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "core_result_try_v1/device_tests.rs"]
mod device_tests;
