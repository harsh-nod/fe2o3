/// Authenticates only the source origin of the pinned bool identity wrappers.
/// Surviving executable calls still traverse ordinary checked lowering; this
/// does not grant a pure-call summary or erase an arbitrary helper body.
pub(crate) fn authenticate_reviewed_branch_hint_origin_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> bool {
    use rustc_middle::mir::{BasicBlock, Rvalue, StatementKind, TerminatorKind, UnwindAction};
    if !matches!(instance.def, InstanceKind::Item(_))
        || !instance.args.is_empty()
        || !is_reviewed_core_function_v1(tcx, instance)
        || !tcx.is_mir_available(instance.def_id())
    {
        return false;
    }
    let unlikely = match tcx
        .def_path(instance.def_id())
        .to_string_no_crate_verbose()
        .as_str()
    {
        "::intrinsics::unlikely" => true,
        "::intrinsics::likely" => false,
        _ => return false,
    };
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    if signature.safety != Safety::Safe
        || signature.abi != ExternAbi::Rust
        || signature.c_variadic
        || signature.inputs() != [tcx.types.bool]
        || signature.output() != tcx.types.bool
    {
        return false;
    }
    let body = tcx.instance_mir(instance.def);
    if body.arg_count != 1
        || body.local_decls.len() != 3
        || body.basic_blocks.len() != 5
        || body.local_decls[rustc_middle::mir::Local::from_usize(0)].ty != tcx.types.bool
        || body.local_decls[rustc_middle::mir::Local::from_usize(1)].ty != tcx.types.bool
        || body.local_decls[rustc_middle::mir::Local::from_usize(2)].ty != tcx.types.unit
    {
        return false;
    }
    let entry = &body.basic_blocks[BasicBlock::from_usize(0)];
    let TerminatorKind::SwitchInt { discr, targets } = &entry.terminator().kind else {
        return false;
    };
    if !entry.statements.is_empty()
        || operand_local_v1(discr).map(|local| local.as_usize()) != Some(1)
    {
        return false;
    }
    let mut entries = targets.iter();
    let Some((0, false_block)) = entries.next() else {
        return false;
    };
    if entries.next().is_some() {
        return false;
    }
    let true_block = targets.otherwise();
    let mut seen = [true, false, false, false, false];
    let mut common_return = None;
    for (mut block, expected) in [(false_block, false), (true_block, true)] {
        let mut saw_hint = false;
        loop {
            if block.as_usize() >= seen.len() || seen[block.as_usize()] {
                return false;
            }
            seen[block.as_usize()] = true;
            let arm = &body.basic_blocks[block];
            match &arm.terminator().kind {
                TerminatorKind::Call {
                    func,
                    args,
                    destination,
                    target: Some(target),
                    unwind: UnwindAction::Unreachable,
                    ..
                } => {
                    if saw_hint
                        || !arm.statements.is_empty()
                        || !args.is_empty()
                        || destination.as_local().map(|local| local.as_usize()) != Some(2)
                    {
                        return false;
                    }
                    let TyKind::FnDef(def_id, generics) = func.ty(body, tcx).kind() else {
                        return false;
                    };
                    let Some(intrinsic) = tcx.intrinsic(*def_id) else {
                        return false;
                    };
                    if def_id.krate != instance.def_id().krate
                        || !generics.is_empty()
                        || intrinsic.name.as_str() != "cold_path"
                    {
                        return false;
                    }
                    saw_hint = true;
                    block = *target;
                }
                TerminatorKind::Goto { target } => {
                    if saw_hint != (expected == unlikely) {
                        return false;
                    }
                    let [statement] = arm.statements.as_slice() else {
                        return false;
                    };
                    let StatementKind::Assign(assignment) = &statement.kind else {
                        return false;
                    };
                    let (destination, Rvalue::Use(Operand::Constant(constant))) = &**assignment
                    else {
                        return false;
                    };
                    if destination.as_local().map(|local| local.as_usize()) != Some(0)
                        || constant.const_.ty() != tcx.types.bool
                    {
                        return false;
                    }
                    let Some(value) = constant
                        .const_
                        .try_eval_scalar_int(tcx, ty::TypingEnv::fully_monomorphized())
                    else {
                        return false;
                    };
                    if !branch_hint_truth_relation_v1(expected, value.to_bits(value.size()))
                        || common_return.is_some_and(|previous| previous != *target)
                    {
                        return false;
                    }
                    common_return = Some(*target);
                    break;
                }
                _ => return false,
            }
        }
    }
    let Some(return_block) = common_return else {
        return false;
    };
    if return_block.as_usize() >= seen.len() || seen[return_block.as_usize()] {
        return false;
    }
    seen[return_block.as_usize()] = true;
    let returned = &body.basic_blocks[return_block];
    seen == [true; 5]
        && returned.statements.is_empty()
        && matches!(returned.terminator().kind, TerminatorKind::Return)
}

const fn branch_hint_truth_relation_v1(input: bool, output_bits: u128) -> bool {
    output_bits == if input { 1 } else { 0 }
}

#[cfg(test)]
mod branch_hint_origin_tests {
    use super::*;

    #[test]
    fn branch_hint_origin_preserves_exact_bool_truth_table() {
        assert!(branch_hint_truth_relation_v1(false, 0));
        assert!(branch_hint_truth_relation_v1(true, 1));
        assert!(!branch_hint_truth_relation_v1(false, 1));
        assert!(!branch_hint_truth_relation_v1(true, 0));
        assert!(!branch_hint_truth_relation_v1(true, 2));
        assert!(!branch_hint_truth_relation_v1(false, u128::MAX));
    }
}
