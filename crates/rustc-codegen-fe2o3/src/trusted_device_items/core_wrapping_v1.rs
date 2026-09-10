//! Closed safe-core wrapper authentication, not an intrinsic or effect summary.
//! Accepted bodies remain in the ordinary collected call graph.

use super::{
    BinOp, Body, ExternAbi, Instance, InstanceKind, Operand, Rvalue, Safety, StatementKind,
    TerminatorKind, Ty, TyCtxt, TyKind, TypingEnv, UnwindAction,
};
use rustc_middle::mir::BasicBlockData;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WrappingOperationV1 {
    Add,
    Multiply,
}

impl WrappingOperationV1 {
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "wrapping_add" => Some(Self::Add),
            "wrapping_mul" => Some(Self::Multiply),
            _ => None,
        }
    }

    fn binary(self) -> BinOp {
        match self {
            Self::Add => BinOp::Add,
            Self::Multiply => BinOp::Mul,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct CoreWrappingContractV1 {
    item_instance: bool,
    lang_item_core_origin: bool,
    inherent_primitive_impl: bool,
    safe_signature: bool,
    rust_abi: bool,
    variadic: bool,
    generic_arguments: usize,
    exact_primitive_signature: bool,
    mir_available: bool,
}

impl CoreWrappingContractV1 {
    fn admits(self) -> bool {
        self.item_instance
            && self.lang_item_core_origin
            && self.inherent_primitive_impl
            && self.safe_signature
            && self.rust_abi
            && !self.variadic
            && self.generic_arguments == 0
            && self.exact_primitive_signature
            && self.mir_available
    }
}

/// Discharges only the missing cross-crate HIR source-safety observation.
/// Collection, intrinsic classification, MIR admission and lowering still run.
pub(crate) fn authenticate_reviewed_safe_core_wrapping_helper_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> bool {
    let Some(core) = tcx.lang_items().sized_trait() else {
        return false;
    };
    if !matches!(instance.def, InstanceKind::Item(_))
        || instance.def_id().krate != core.krate
        || tcx.crate_name(core.krate).as_str() != "core"
        || !instance.args.is_empty()
    {
        return false;
    }
    let Some(operation) = WrappingOperationV1::from_name(tcx.item_name(instance.def_id()).as_str())
    else {
        return false;
    };
    let Some(associated) = tcx.opt_associated_item(instance.def_id()) else {
        return false;
    };
    let Some(implementation) = tcx.impl_of_assoc(instance.def_id()) else {
        return false;
    };
    if !associated.is_fn()
        || tcx.impl_is_of_trait(implementation)
        || implementation.krate != core.krate
    {
        return false;
    }
    let element = tcx.type_of(implementation).instantiate_identity();
    let primitive = matches!(element.kind(), TyKind::Int(_) | TyKind::Uint(_));
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    let contract = CoreWrappingContractV1 {
        item_instance: matches!(instance.def, InstanceKind::Item(_)),
        lang_item_core_origin: instance.def_id().krate == core.krate
            && tcx.crate_name(core.krate).as_str() == "core",
        inherent_primitive_impl: primitive,
        safe_signature: signature.safety == Safety::Safe,
        rust_abi: signature.abi == ExternAbi::Rust,
        variadic: signature.c_variadic,
        generic_arguments: instance.args.len(),
        exact_primitive_signature: signature.inputs() == [element, element]
            && signature.output() == element,
        mir_available: tcx.is_mir_available(instance.def_id()),
    };
    contract.admits()
        && reviewed_wrapping_body_v1(tcx, tcx.instance_mir(instance.def), element, operation)
}

fn operand_local_v1(operand: &Operand<'_>) -> Option<usize> {
    match operand {
        Operand::Copy(place) | Operand::Move(place) if place.projection.is_empty() => {
            Some(place.local.as_usize())
        }
        _ => None,
    }
}

fn exact_wrapping_binary_v1(
    expected: WrappingOperationV1,
    observed: BinOp,
    destination: Option<usize>,
    left: Option<usize>,
    right: Option<usize>,
) -> bool {
    observed == expected.binary() && destination == Some(0) && left == Some(1) && right == Some(2)
}

fn reviewed_wrapping_binary_block_v1(
    block: &BasicBlockData<'_>,
    expected: WrappingOperationV1,
) -> bool {
    let [statement] = block.statements.as_slice() else {
        return false;
    };
    let StatementKind::Assign(assignment) = &statement.kind else {
        return false;
    };
    let (destination, rvalue) = &**assignment;
    let Rvalue::BinaryOp(operation, operands) = rvalue else {
        return false;
    };
    !block.is_cleanup
        && matches!(
            block.terminator.as_ref().map(|term| &term.kind),
            Some(TerminatorKind::Return)
        )
        && exact_wrapping_binary_v1(
            expected,
            *operation,
            destination
                .projection
                .is_empty()
                .then(|| destination.local.as_usize()),
            operand_local_v1(&operands.0),
            operand_local_v1(&operands.1),
        )
}

fn reviewed_wrapping_body_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    element: Ty<'tcx>,
    expected: WrappingOperationV1,
) -> bool {
    if body.arg_count != 2
        || body.local_decls.len() != 3
        || !(1..=2).contains(&body.basic_blocks.len())
        || body.local_decls.iter().any(|local| local.ty != element)
        || body.basic_blocks.iter().any(|block| block.is_cleanup)
        || !(1..=8).contains(&body.source_scopes.len())
        || body
            .source_scopes
            .iter()
            .any(|scope| scope.inlined.is_some())
    {
        return false;
    }
    let mut blocks = body.basic_blocks.iter();
    let entry = blocks.next().expect("nonempty body checked");
    let Some(terminator) = &entry.terminator else {
        return false;
    };
    if body.basic_blocks.len() == 1 {
        return reviewed_wrapping_binary_block_v1(entry, expected);
    }
    let tail = blocks.next().expect("two-block body checked");
    let TerminatorKind::Call {
        func,
        args,
        destination,
        target,
        unwind,
        ..
    } = &terminator.kind
    else {
        return false;
    };
    if !entry.statements.is_empty()
        || !tail.statements.is_empty()
        || !matches!(
            tail.terminator.as_ref().map(|terminator| &terminator.kind),
            Some(TerminatorKind::Return)
        )
        || !matches!(unwind, UnwindAction::Unreachable)
        || target.is_none_or(|target| target.index() != 1)
        || destination.local.as_usize() != 0
        || !destination.projection.is_empty()
        || !matches!(&args[..], [left, right] if operand_local_v1(&left.node) == Some(1)
            && operand_local_v1(&right.node) == Some(2))
    {
        return false;
    }
    let Operand::Constant(callee) = func else {
        return false;
    };
    let TyKind::FnDef(definition, arguments) = callee.const_.ty().kind() else {
        return false;
    };
    let Ok(Some(intrinsic)) = Instance::try_resolve(
        tcx,
        TypingEnv::fully_monomorphized(),
        *definition,
        arguments,
    ) else {
        return false;
    };
    let Some(core) = tcx.lang_items().sized_trait() else {
        return false;
    };
    let InstanceKind::Intrinsic(definition) = intrinsic.def else {
        return false;
    };
    let Some(metadata) = tcx.intrinsic(definition) else {
        return false;
    };
    let [argument] = intrinsic.args.as_slice() else {
        return false;
    };
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(definition).instantiate(tcx, intrinsic.args),
    );
    definition.krate == core.krate
        && WrappingOperationV1::from_name(metadata.name.as_str()) == Some(expected)
        && argument.as_type() == Some(element)
        && signature.inputs() == [element, element]
        && signature.output() == element
        && !signature.c_variadic
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustc_middle::mir::{Local, SourceInfo, Statement, Terminator};
    use rustc_span::DUMMY_SP;

    #[test]
    fn core_wrapping_helper_origin_and_signature_are_closed() {
        let exact = CoreWrappingContractV1 {
            item_instance: true,
            lang_item_core_origin: true,
            inherent_primitive_impl: true,
            safe_signature: true,
            rust_abi: true,
            variadic: false,
            generic_arguments: 0,
            exact_primitive_signature: true,
            mir_available: true,
        };
        assert!(exact.admits());
        for changed in [
            CoreWrappingContractV1 {
                item_instance: false,
                ..exact
            },
            CoreWrappingContractV1 {
                lang_item_core_origin: false,
                ..exact
            },
            CoreWrappingContractV1 {
                inherent_primitive_impl: false,
                ..exact
            },
            CoreWrappingContractV1 {
                safe_signature: false,
                ..exact
            },
            CoreWrappingContractV1 {
                rust_abi: false,
                ..exact
            },
            CoreWrappingContractV1 {
                variadic: true,
                ..exact
            },
            CoreWrappingContractV1 {
                generic_arguments: 1,
                ..exact
            },
            CoreWrappingContractV1 {
                exact_primitive_signature: false,
                ..exact
            },
            CoreWrappingContractV1 {
                mir_available: false,
                ..exact
            },
        ] {
            assert!(!changed.admits(), "{changed:?}");
        }
        for name in [
            "wrapping_sub",
            "overflowing_add",
            "unchecked_mul",
            "wrapping_add_impostor",
        ] {
            assert_eq!(WrappingOperationV1::from_name(name), None);
        }
    }

    #[test]
    fn core_wrapping_helper_binary_requires_exact_operation_and_operands() {
        for (name, expected, operation) in [
            ("wrapping_add", WrappingOperationV1::Add, BinOp::Add),
            ("wrapping_mul", WrappingOperationV1::Multiply, BinOp::Mul),
        ] {
            assert_eq!(WrappingOperationV1::from_name(name), Some(expected));
            assert!(exact_wrapping_binary_v1(
                expected,
                operation,
                Some(0),
                Some(1),
                Some(2)
            ));
            for (observed, destination, left, right) in [
                (BinOp::Sub, Some(0), Some(1), Some(2)),
                (BinOp::AddWithOverflow, Some(0), Some(1), Some(2)),
                (BinOp::MulWithOverflow, Some(0), Some(1), Some(2)),
                (BinOp::AddUnchecked, Some(0), Some(1), Some(2)),
                (BinOp::MulUnchecked, Some(0), Some(1), Some(2)),
                (operation, None, Some(1), Some(2)),
                (operation, Some(1), Some(1), Some(2)),
                (operation, Some(0), None, Some(2)),
                (operation, Some(0), Some(2), Some(1)),
                (operation, Some(0), Some(1), Some(1)),
                (operation, Some(0), Some(1), None),
            ] {
                assert!(!exact_wrapping_binary_v1(
                    expected,
                    observed,
                    destination,
                    left,
                    right
                ));
            }
        }
    }

    #[test]
    fn core_wrapping_helper_binary_mir_rejects_extra_effects_and_wrong_return() {
        for expected in [WrappingOperationV1::Add, WrappingOperationV1::Multiply] {
            let source_info = SourceInfo::outermost(DUMMY_SP);
            let statement = Statement::new(
                source_info,
                StatementKind::Assign(Box::new((
                    Local::from_usize(0).into(),
                    Rvalue::BinaryOp(
                        expected.binary(),
                        Box::new((
                            Operand::Copy(Local::from_usize(1).into()),
                            Operand::Move(Local::from_usize(2).into()),
                        )),
                    ),
                ))),
            );
            let exact = BasicBlockData::new_stmts(
                vec![statement],
                Some(Terminator {
                    source_info,
                    kind: TerminatorKind::Return,
                }),
                false,
            );
            assert!(reviewed_wrapping_binary_block_v1(&exact, expected));

            let mut changed = exact.clone();
            changed.statements.clear();
            assert!(!reviewed_wrapping_binary_block_v1(&changed, expected));
            let mut changed = exact.clone();
            changed.statements.push(Statement::new(
                source_info,
                StatementKind::StorageDead(Local::from_usize(1)),
            ));
            assert!(!reviewed_wrapping_binary_block_v1(&changed, expected));
            let mut changed = exact.clone();
            changed.terminator = None;
            assert!(!reviewed_wrapping_binary_block_v1(&changed, expected));
            let mut changed = exact.clone();
            changed.terminator.as_mut().unwrap().kind = TerminatorKind::Unreachable;
            assert!(!reviewed_wrapping_binary_block_v1(&changed, expected));
            let mut changed = exact;
            changed.is_cleanup = true;
            assert!(!reviewed_wrapping_binary_block_v1(&changed, expected));
        }
    }
}
