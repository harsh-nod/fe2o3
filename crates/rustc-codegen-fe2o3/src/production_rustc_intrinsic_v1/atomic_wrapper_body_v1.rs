fn reviewed_atomic_wrapper_body_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    load: bool,
    element: Ty<'tcx>,
) -> bool {
    use rustc_middle::mir::{BasicBlock, Rvalue, StatementKind, TerminatorKind, UnwindAction};
    let body = tcx.instance_mir(instance.def);
    let arguments = if load { 2 } else { 3 };
    // This deliberately recognizes the pinned wrapper graph, not arbitrary
    // interprocedural programs. Formatting on rejected ordering arms is dead
    // for every admitted call, and cannot escape into an accepted arm.
    if body.arg_count != arguments
        || body.basic_blocks.len() != 8
        || body.local_decls.len() > 64
        || body
            .basic_blocks
            .iter()
            .any(|block| block.statements.len() > 64)
    {
        return false;
    }
    let entry = &body.basic_blocks[BasicBlock::from_u32(0)];
    let [statement] = entry.statements.as_slice() else {
        return false;
    };
    let StatementKind::Assign(assignment) = &statement.kind else {
        return false;
    };
    let (discriminant, Rvalue::Discriminant(ordering_place)) = &**assignment else {
        return false;
    };
    if discriminant
        .as_local()
        .is_none_or(|local| local.as_usize() <= arguments)
        || ordering_place
            .as_local()
            .is_none_or(|local| local.as_usize() != arguments)
    {
        return false;
    }
    let TerminatorKind::SwitchInt { discr, targets } = &entry.terminator().kind else {
        return false;
    };
    if operand_local_v1(discr) != discriminant.as_local() {
        return false;
    }
    let mut seen = [false; 8];
    seen[0] = true;
    let default = targets.otherwise();
    if default.as_usize() >= seen.len() || seen[default.as_usize()] {
        return false;
    }
    seen[default.as_usize()] = true;
    if !body.basic_blocks[default].statements.is_empty()
        || !matches!(
            body.basic_blocks[default].terminator().kind,
            TerminatorKind::Unreachable
        )
    {
        return false;
    }
    let mut seen_orderings = [false; 5];
    let mut common_return = None;
    for (discriminant, target) in targets.iter() {
        let Ok(discriminant) = usize::try_from(discriminant) else {
            return false;
        };
        if discriminant >= seen_orderings.len()
            || seen_orderings[discriminant]
            || target.as_usize() >= seen.len()
            || seen[target.as_usize()]
        {
            return false;
        }
        seen_orderings[discriminant] = true;
        seen[target.as_usize()] = true;
        let ordering = atomic_ordering_from_discriminant_v1(discriminant as u64).unwrap();
        let arm = &body.basic_blocks[target];
        let TerminatorKind::Call {
            func,
            args,
            destination,
            target: return_block,
            unwind,
            ..
        } = &arm.terminator().kind
        else {
            return false;
        };
        if !matches!(unwind, UnwindAction::Unreachable) {
            return false;
        }
        let callable = match instance.try_instantiate_mir_and_normalize_erasing_regions(
            tcx,
            ty::TypingEnv::fully_monomorphized(),
            ty::EarlyBinder::bind(func.ty(body, tcx)),
        ) {
            Ok(callable) => callable,
            Err(_) => return false,
        };
        let TyKind::FnDef(def_id, generic_args) = callable.kind() else {
            return false;
        };
        if load_store_ordering_supported_v1(load, ordering) {
            let Some(return_block) = *return_block else {
                return false;
            };
            if common_return.is_some_and(|previous| previous != return_block)
                || return_block.as_usize() >= seen.len()
            {
                return false;
            }
            common_return = Some(return_block);
            let Ok(Some(callee)) = Instance::try_resolve(
                tcx,
                ty::TypingEnv::fully_monomorphized(),
                *def_id,
                generic_args,
            ) else {
                return false;
            };
            let Ok(Some(classification)) = classify(tcx, callee) else {
                return false;
            };
            let expected_access =
                SemanticAtomicAccessV1::new(ordering, SemanticAtomicScopeV1::System);
            let expected_operation = if load {
                ProductionRustcIntrinsicOperationV1::AtomicLoad {
                    access: expected_access,
                }
            } else {
                ProductionRustcIntrinsicOperationV1::AtomicStore {
                    access: expected_access,
                }
            };
            if !reviewed_atomic_arm_relation_v1(
                arm.statements.is_empty(),
                classification.operation == expected_operation
                    && classification.element_type == element,
                args.len() == if load { 1 } else { 2 },
                args.first()
                    .and_then(|arg| operand_local_v1(&arg.node))
                    .map(|local| local.as_usize()),
                if load {
                    None
                } else {
                    args.get(1)
                        .and_then(|arg| operand_local_v1(&arg.node))
                        .map(|local| local.as_usize())
                },
                destination.as_local().map(|local| local.as_usize()),
                load,
            ) {
                return false;
            }
        } else {
            // Invalid orderings must terminate in core's panic, never return,
            // and may only prepare local formatting temporaries.
            if return_block.is_some()
                || tcx.crate_name(def_id.krate).as_str() != "core"
                || def_id.krate != instance.def_id().krate
                || tcx.def_path_str(*def_id) != "core::panicking::panic_fmt"
                || destination
                    .as_local()
                    .is_none_or(|local| local.as_usize() <= arguments)
                || args.len() != 1
            {
                return false;
            }
            if arm
                .statements
                .iter()
                .any(|statement| match &statement.kind {
                    StatementKind::StorageLive(local) | StatementKind::StorageDead(local) => {
                        local.as_usize() <= arguments
                    }
                    StatementKind::Assign(assignment) => assignment
                        .0
                        .as_local()
                        .is_none_or(|local| local.as_usize() <= arguments),
                    _ => true,
                })
            {
                return false;
            }
        }
    }
    if seen_orderings != [true; 5] {
        return false;
    }
    let Some(return_block) = common_return else {
        return false;
    };
    if seen[return_block.as_usize()] {
        return false;
    }
    seen[return_block.as_usize()] = true;
    let return_body = &body.basic_blocks[return_block];
    seen == [true; 8]
        && return_body.statements.is_empty()
        && matches!(return_body.terminator().kind, TerminatorKind::Return)
}

fn operand_local_v1(operand: &Operand<'_>) -> Option<rustc_middle::mir::Local> {
    match operand {
        Operand::Copy(place) | Operand::Move(place) => place.as_local(),
        Operand::Constant(_) | Operand::RuntimeChecks(_) => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn reviewed_atomic_arm_relation_v1(
    no_statements: bool,
    exact_intrinsic: bool,
    exact_arity: bool,
    pointer_local: Option<usize>,
    value_local: Option<usize>,
    result_local: Option<usize>,
    load: bool,
) -> bool {
    no_statements
        && exact_intrinsic
        && exact_arity
        && pointer_local == Some(1)
        && value_local == if load { None } else { Some(2) }
        && result_local == Some(0)
}

#[cfg(test)]
mod atomic_wrapper_graph_tests {
    use super::*;

    #[test]
    fn selected_atomic_wrapper_arm_rejects_effect_and_dataflow_mutants() {
        for load in [false, true] {
            let value = if load { None } else { Some(2) };
            assert!(reviewed_atomic_arm_relation_v1(
                true,
                true,
                true,
                Some(1),
                value,
                Some(0),
                load
            ));
            assert!(
                !reviewed_atomic_arm_relation_v1(false, true, true, Some(1), value, Some(0), load),
                "added statement/write"
            );
            assert!(
                !reviewed_atomic_arm_relation_v1(true, false, true, Some(1), value, Some(0), load),
                "changed ordering/kind/type"
            );
            assert!(
                !reviewed_atomic_arm_relation_v1(true, true, false, Some(1), value, Some(0), load),
                "changed arity"
            );
            assert!(
                !reviewed_atomic_arm_relation_v1(true, true, true, Some(2), value, Some(0), load),
                "substituted pointer"
            );
            assert!(
                !reviewed_atomic_arm_relation_v1(true, true, true, None, value, Some(0), load),
                "projected pointer"
            );
            assert!(
                !reviewed_atomic_arm_relation_v1(true, true, true, Some(1), Some(3), Some(0), load),
                "substituted value"
            );
            assert!(
                !reviewed_atomic_arm_relation_v1(true, true, true, Some(1), value, Some(4), load),
                "changed return"
            );
        }
    }
}
