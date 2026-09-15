//! A summary consumer of the existing closed core source proof, not a new
//! external-helper allowlist. Reference-root and local-HIR checks are unchanged.
use super::*;

pub(super) struct ReviewedWrapping<'tcx> {
    helper: Instance<'tcx>,
    element: Ty<'tcx>,
    operation: ReferenceBinaryOpV1,
}

fn operation(name: &str) -> Option<ReferenceBinaryOpV1> {
    Some(match name {
        "wrapping_add" => ReferenceBinaryOpV1::Add,
        "wrapping_sub" => ReferenceBinaryOpV1::Subtract,
        "wrapping_mul" => ReferenceBinaryOpV1::Multiply,
        _ => return None,
    })
}

pub(super) fn authenticate<'tcx>(
    tcx: TyCtxt<'tcx>,
    helper: Instance<'tcx>,
) -> Option<ReviewedWrapping<'tcx>> {
    if !trusted_device_items::authenticate_reviewed_safe_core_wrapping_helper_v1(tcx, helper) {
        return None;
    }
    let signature = instantiated_signature(tcx, helper);
    Some(ReviewedWrapping {
        helper,
        element: signature.output(),
        operation: operation(tcx.item_name(helper.def_id()).as_str())?,
    })
}

impl<'tcx> ReviewedWrapping<'tcx> {
    pub(super) fn summary(
        &self,
        tcx: TyCtxt<'tcx>,
    ) -> Result<ReferenceEffectExpressionV1, ReferenceBindingErrorV1> {
        self.summary_from_body(tcx, tcx.instance_mir(self.helper.def))
    }

    fn summary_from_body(
        &self,
        tcx: TyCtxt<'tcx>,
        body: &Body<'tcx>,
    ) -> Result<ReferenceEffectExpressionV1, ReferenceBindingErrorV1> {
        let rejected = || {
            ReferenceBindingErrorV1::new(
                "reviewed core wrapping body does not match its exact scalar summary",
            )
        };
        if body.arg_count != 2
            || body.local_decls.len() != 3
            || body
                .local_decls
                .iter()
                .any(|local| local.ty != self.element)
            || body.basic_blocks.iter().any(|block| block.is_cleanup)
        {
            return Err(rejected());
        }
        scalar_type_v1(self.element).ok_or_else(rejected)?;
        let mut blocks = body.basic_blocks.iter();
        let observed = match (blocks.next(), blocks.next(), blocks.next()) {
            (Some(entry), None, None)
                if matches!(
                    entry.terminator.as_ref().map(|t| &t.kind),
                    Some(TerminatorKind::Return)
                ) =>
            {
                let [statement] = entry.statements.as_slice() else {
                    return Err(rejected());
                };
                let StatementKind::Assign(assignment) = &statement.kind else {
                    return Err(rejected());
                };
                let (destination, value) = &**assignment;
                let Rvalue::BinaryOp(binary, operands) = value else {
                    return Err(rejected());
                };
                if !plain(*destination, 0) || !operand(&operands.0, 1) || !operand(&operands.1, 2) {
                    return Err(rejected());
                }
                match binary {
                    BinOp::Add => ReferenceBinaryOpV1::Add,
                    BinOp::Sub => ReferenceBinaryOpV1::Subtract,
                    BinOp::Mul => ReferenceBinaryOpV1::Multiply,
                    _ => return Err(rejected()),
                }
            }
            (Some(entry), Some(exit), None)
                if entry.statements.is_empty()
                    && exit.statements.is_empty()
                    && matches!(
                        exit.terminator.as_ref().map(|t| &t.kind),
                        Some(TerminatorKind::Return)
                    ) =>
            {
                let Some(TerminatorKind::Call {
                    func,
                    args,
                    destination,
                    target: Some(target),
                    unwind: UnwindAction::Unreachable,
                    ..
                }) = entry.terminator.as_ref().map(|t| &t.kind)
                else {
                    return Err(rejected());
                };
                if target.as_u32() != 1
                    || !plain(*destination, 0)
                    || !matches!(&args[..], [left, right] if operand(&left.node, 1) && operand(&right.node, 2))
                {
                    return Err(rejected());
                }
                let Operand::Constant(function) = func else {
                    return Err(rejected());
                };
                let TyKind::FnDef(definition, arguments) = function.const_.ty().kind() else {
                    return Err(rejected());
                };
                let intrinsic = Instance::try_resolve(
                    tcx,
                    TypingEnv::fully_monomorphized(),
                    *definition,
                    arguments,
                )
                .map_err(|_| rejected())?
                .ok_or_else(rejected)?;
                let rustc_middle::ty::InstanceKind::Intrinsic(definition) = intrinsic.def else {
                    return Err(rejected());
                };
                let metadata = tcx.intrinsic(definition).ok_or_else(rejected)?;
                let signature = instantiated_signature(tcx, intrinsic);
                if definition.krate != self.helper.def_id().krate
                    || !matches!(intrinsic.args.as_slice(), [argument] if argument.as_type() == Some(self.element))
                    || signature.inputs() != [self.element, self.element]
                    || signature.output() != self.element
                    || signature.c_variadic
                {
                    return Err(rejected());
                }
                operation(metadata.name.as_str()).ok_or_else(rejected)?
            }
            _ => return Err(rejected()),
        };
        if observed != self.operation {
            return Err(rejected());
        }
        Ok(ReferenceEffectExpressionV1::Binary {
            operation: observed,
            lhs: Box::new(ReferenceEffectExpressionV1::KernelScalarArgument { argument: 0 }),
            rhs: Box::new(ReferenceEffectExpressionV1::KernelScalarArgument { argument: 1 }),
            checked: false,
        })
    }
}

fn plain(place: Place<'_>, local: usize) -> bool {
    place.local.as_usize() == local && place.projection.is_empty()
}

fn operand(value: &Operand<'_>, local: usize) -> bool {
    matches!(value, Operand::Copy(place) | Operand::Move(place) if plain(*place, local))
}

#[cfg(test)]
mod compiler_tests;
