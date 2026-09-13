use super::*;
use dialect_kernel::{
    AtomicOrderingAttr, AtomicScopeAttr, MemorySpaceAttr, RequireEquivalentOp,
    SEMANTIC_TYPED_READ_SYMBOL_BASE_V1, SemanticReadOrderingAttr, SemanticReadVolatilityAttr,
    SemanticScalarKindAttr, SemanticTypedConstantOp, SemanticTypedReadOp, SemanticTypedScalarV1,
};

fn scalar() -> SemanticTypedScalarV1 {
    SemanticTypedScalarV1::new(SemanticScalarKindAttr::Float, 32).unwrap()
}

fn fixture(indices: &[u64]) -> (Context, FuncOp, RankedAccessOp, SemanticTypedReadOp) {
    let mut context = setup();
    let (function, _) = function(&mut context, "read_pair", 0);
    let entry = function.get_entry_block(&context);
    let ty = RankedViewType::new(&mut context, 32, true, vec![8; indices.len()]).unwrap();
    let view = RankedViewOp::new_in_space_with_allocation_contract(
        &mut context,
        ty,
        vec![],
        MemorySpaceAttr::Global,
        7,
        1,
    )
    .unwrap();
    append(&context, entry, &view);
    let indices = indices
        .iter()
        .map(|value| {
            let index = IndexConstantOp::new(&mut context, *value);
            append(&context, entry, &index);
            index.result(&context)
        })
        .collect::<Vec<_>>();
    let view = view.result(&context);
    let access =
        RankedAccessOp::new(&mut context, AccessKindAttr::Read, view, indices.clone()).unwrap();
    let read = SemanticTypedReadOp::new(
        &mut context,
        SEMANTIC_TYPED_READ_SYMBOL_BASE_V1,
        scalar(),
        MemorySpaceAttr::Global,
        SemanticReadVolatilityAttr::NonVolatile,
        SemanticReadOrderingAttr::Unordered,
        view,
        indices,
        None,
    )
    .unwrap();
    let ret = ReturnOp::new(&mut context);
    append(&context, entry, &access);
    append(&context, entry, &read);
    append(&context, entry, &ret);
    (context, function, access, read)
}

fn unpaired(context: &Context, function: &FuncOp, block: usize, operation: usize) {
    let report = run_pliron_ranked_bounds_check_v1(context, function);
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert_eq!(
        report.findings(),
        &[RankedBoundsFindingV1::UnpairedSemanticRead { block, operation }]
    );
    assert!(
        report.findings()[0]
            .to_string()
            .starts_with("error[FE2O3-BOUNDS-007]")
    );
}

#[test]
fn both_read_modes_pass_bounds_without_granting_equivalence() {
    for mode in [
        SemanticReadVolatilityAttr::NonVolatile,
        SemanticReadVolatilityAttr::Volatile,
    ] {
        let (mut context, function, _, read) = fixture(&[2]);
        read.set_attr_kernel_semantic_read_volatility(&mut context, mode);
        let report = run_pliron_ranked_bounds_check_v1(&context, &function);
        assert!(report.is_clean(), "{report:?}");
        assert!(!report.grants_compiler_refinement_authority());
        assert!(!report.grants_artifact_or_launch_authority());
        let value = read.result(&context);
        let requirement = RequireEquivalentOp::new(&mut context, value, value);
        requirement
            .get_operation()
            .insert_after(&context, read.get_operation());
        assert!(!crate::run_pliron_semantic_refinement_check_v1(&context, &function).is_clean());
    }
}

#[test]
fn bounds_remain_attached_to_the_original_access() {
    let (context, function, _, _) = fixture(&[8]);
    let report = run_pliron_ranked_bounds_check_v1(&context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Rejected);
    assert!(matches!(
        report.findings(),
        [RankedBoundsFindingV1::StaticOutOfBounds {
            block: 0,
            operation: 2,
            access: AccessKindAttr::Read,
            dimension: 0,
            index: 8,
            extent: 8,
            ..
        }]
    ));
}

#[test]
fn missing_intervening_or_duplicate_companions_reject() {
    let (mut context, function, access, _) = fixture(&[2]);
    Operation::erase(access.get_operation(), &mut context);
    unpaired(&context, &function, 0, 2);

    let (mut context, function, _, read) = fixture(&[2]);
    let constant = IndexConstantOp::new(&mut context, 2);
    constant
        .get_operation()
        .insert_before(&context, read.get_operation());
    unpaired(&context, &function, 0, 4);

    let (mut context, function, _, read) = fixture(&[2]);
    let view = read.view(&context);
    let indices = read.indices(&context).unwrap();
    let duplicate = SemanticTypedReadOp::new(
        &mut context,
        SEMANTIC_TYPED_READ_SYMBOL_BASE_V1 + 1,
        scalar(),
        MemorySpaceAttr::Global,
        SemanticReadVolatilityAttr::NonVolatile,
        SemanticReadOrderingAttr::Unordered,
        view,
        indices,
        None,
    )
    .unwrap();
    duplicate
        .get_operation()
        .insert_after(&context, read.get_operation());
    unpaired(&context, &function, 0, 4);
}

#[test]
fn write_atomic_ordering_and_scope_cannot_supply_a_read_result() {
    for case in 0..4 {
        let (mut context, function, access, _) = fixture(&[2]);
        match case {
            0 => access.set_attr_kernel_access_kind(&mut context, AccessKindAttr::Write),
            1 => access.set_attr_kernel_atomic_ordering(&mut context, AtomicOrderingAttr::Relaxed),
            2 => access.set_attr_kernel_atomic_scope(&mut context, AtomicScopeAttr::Device),
            _ => {
                access.set_attr_kernel_access_kind(&mut context, AccessKindAttr::AtomicRead);
                access.set_attr_kernel_atomic_ordering(&mut context, AtomicOrderingAttr::Relaxed);
                access.set_attr_kernel_atomic_scope(&mut context, AtomicScopeAttr::Device);
            }
        }
        unpaired(&context, &function, 0, 3);
    }
}

#[test]
fn pair_requires_ssa_identity_not_equal_metadata_or_constant_values() {
    for change_view in [false, true] {
        let (mut context, function, access, read) = fixture(&[2]);
        let (operand, replacement) = if change_view {
            let ty = RankedViewType::new(&mut context, 32, true, vec![8]).unwrap();
            let view = RankedViewOp::new_in_space_with_allocation_contract(
                &mut context,
                ty,
                vec![],
                MemorySpaceAttr::Global,
                7,
                1,
            )
            .unwrap();
            view.get_operation()
                .insert_before(&context, access.get_operation());
            (0, view.result(&context))
        } else {
            let index = IndexConstantOp::new(&mut context, 2);
            index
                .get_operation()
                .insert_before(&context, access.get_operation());
            (1, index.result(&context))
        };
        Operation::replace_operand(read.get_operation(), &context, operand, replacement);
        unpaired(&context, &function, 0, 4);
    }
    let (context, function, _, read) = fixture(&[2, 3]);
    let indices = read.indices(&context).unwrap();
    Operation::replace_operand(read.get_operation(), &context, 1, indices[1]);
    Operation::replace_operand(read.get_operation(), &context, 2, indices[0]);
    unpaired(&context, &function, 0, 4);
}

#[test]
fn guarded_read_is_not_an_unconditional_access_even_for_constant_guards() {
    for guard_value in [0, 1] {
        let (mut context, function, access, read) = fixture(&[2]);
        let bool_ty = SemanticTypedScalarV1::new(SemanticScalarKindAttr::Bool, 1).unwrap();
        let guard = SemanticTypedConstantOp::new(&mut context, guard_value, bool_ty);
        let fallback = SemanticTypedConstantOp::new(&mut context, 0, scalar());
        guard
            .get_operation()
            .insert_before(&context, access.get_operation());
        fallback
            .get_operation()
            .insert_before(&context, access.get_operation());
        Operation::push_operand(read.get_operation(), &context, guard.result(&context));
        Operation::push_operand(read.get_operation(), &context, fallback.result(&context));
        unpaired(&context, &function, 0, 5);
    }
}

#[test]
fn checked_access_cannot_supply_an_unconditional_read_result() {
    use dialect_kernel::{CheckedTiledIndex2DOp, DYNAMIC_EXTENT};
    let (mut context, function, access, read) = fixture(&[2]);
    let zero = IndexConstantOp::new(&mut context, 0);
    let one = IndexConstantOp::new(&mut context, 1);
    let extent = IndexConstantOp::new(&mut context, 8);
    for op in [&zero, &one, &extent] {
        op.get_operation()
            .insert_before(&context, access.get_operation());
    }
    let zero = zero.result(&context);
    let one = one.result(&context);
    let extent = extent.result(&context);
    let ty = RankedViewType::new(&mut context, 32, false, vec![DYNAMIC_EXTENT]).unwrap();
    let view = RankedViewOp::new(&mut context, ty, vec![extent]).unwrap();
    view.get_operation()
        .insert_before(&context, access.get_operation());
    let view = view.result(&context);
    let checked = CheckedTiledIndex2DOp::new_predicated(
        &mut context,
        zero,
        zero,
        one,
        extent,
        extent,
        extent,
        [1, 1, 1, 1],
    );
    checked
        .get_operation()
        .insert_before(&context, access.get_operation());
    let index = checked.result(&context);
    let success = checked.success(&context).unwrap();
    let predicated =
        RankedAccessOp::new_predicated(&mut context, AccessKindAttr::Read, view, index, success)
            .unwrap();
    predicated
        .get_operation()
        .insert_before(&context, access.get_operation());
    Operation::erase(access.get_operation(), &mut context);
    Operation::replace_operand(read.get_operation(), &context, 0, view);
    Operation::replace_operand(read.get_operation(), &context, 1, index);
    unpaired(&context, &function, 0, 8);
}

#[test]
fn paired_result_adds_no_memory_event_and_supplies_no_write_contract() {
    use crate::production_analysis::pliron_analysis_manager::PlironAnalysisManagerV1;
    use crate::production_analysis::pliron_invocation_trace::PlironTraceEventV1;
    for mode in [
        SemanticReadVolatilityAttr::NonVolatile,
        SemanticReadVolatilityAttr::Volatile,
    ] {
        let (mut context, function, access, read) = fixture(&[2]);
        dialect_gpu::register_dialect(&mut context).unwrap();
        let layout =
            dialect_gpu::ExecutionLayoutOp::new(&mut context, 1, [64, 1, 1], [64, 1, 1], 64);
        layout
            .get_operation()
            .insert_before(&context, access.view(&context).defining_op().unwrap());
        read.set_attr_kernel_semantic_read_volatility(&mut context, mode);
        assert!(run_pliron_ranked_bounds_check_v1(&context, &function).is_clean());
        let mut analyses = PlironAnalysisManagerV1::new(&function);
        analyses.prepare_exact_trace(&context, &function);
        let traces = analyses.exact_trace().unwrap();
        assert_eq!(traces.len(), 64);
        for trace in traces {
            assert!(
                matches!(trace.events.as_slice(), [PlironTraceEventV1::Memory {
                location, access: AccessKindAttr::Read, indices, ..
            }] if location.block == 0 && location.operation == 3 && indices == &[Some(2)])
            );
        }
        let view = access.view(&context);
        let indices = access.indices(&context);
        let write =
            RankedAccessOp::new(&mut context, AccessKindAttr::Write, view, indices).unwrap();
        write
            .get_operation()
            .insert_after(&context, read.get_operation());
        let effects = crate::run_pliron_effect_refinement_check_v1(&context, &function);
        assert_eq!(effects.contract_count(), 0);
        assert_eq!(effects.proved_contract_count(), 0);
        assert!(!effects.all_declared_effects_are_proved());
        assert!(!effects.grants_compiler_refinement_authority());
        assert!(!effects.grants_artifact_or_launch_authority());
        let races = crate::run_pliron_ranked_race_check_v1(&context, &function);
        assert!(races.findings().iter().any(|finding| matches!(
            finding,
            crate::RankedRaceFindingV1::ConflictingEffects { .. }
        )));
    }
}

#[test]
fn pair_cannot_cross_a_block_boundary_but_can_move_together() {
    for together in [false, true] {
        let (mut context, function, access, read) = fixture(&[2]);
        let next = block(&mut context, &function, "next");
        let entry = function.get_entry_block(&context);
        let ret = entry.deref(&context).get_terminator(&context).unwrap();
        ret.unlink(&context);
        if together {
            access.get_operation().unlink(&context);
            append(&context, next, &access);
        }
        read.get_operation().unlink(&context);
        append(&context, next, &read);
        ret.insert_at_back(next, &context);
        let branch = BranchOp::new(&mut context, next);
        append(&context, entry, &branch);
        if together {
            let report = run_pliron_ranked_bounds_check_v1(&context, &function);
            assert!(report.is_clean(), "{report:?}");
        } else {
            unpaired(&context, &function, 1, 0);
        }
    }
}

#[test]
fn malformed_read_or_access_operands_fail_structural_verification() {
    for mutate_read in [false, true] {
        for extra in [false, true] {
            let (context, function, access, read) = fixture(&[2]);
            let target = if mutate_read {
                read.get_operation()
            } else {
                access.get_operation()
            };
            if extra {
                Operation::push_operand(target, &context, read.view(&context));
            } else {
                Operation::pop_operand(target, &context);
            }
            assert_eq!(
                run_pliron_ranked_bounds_check_v1(&context, &function).findings(),
                &[RankedBoundsFindingV1::StructuralVerificationFailed]
            );
        }
    }
}
