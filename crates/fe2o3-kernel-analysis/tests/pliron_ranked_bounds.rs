use dialect_kernel::{
    AccessKindAttr, AlgorithmOp, BranchOp, DIALECT_NAME, DeterministicJoinOp, DimensionOp,
    GeneralGemmOp, IndexConstantOp, IndexEqualBranchOp, IndexLessThanBranchOp, IndexType,
    RankedAccessOp, RankedViewOp, RankedViewType, ReturnOp, TrapOp, register_dialect,
};
use fe2o3_kernel_analysis::{
    KernelCheckPassKindV1, KernelCheckStatusV1, MAX_RANKED_BOUNDS_BLOCKS, MAX_RANKED_BOUNDS_EDGES,
    MAX_RANKED_BOUNDS_FINDINGS, RankedBoundsFindingV1,
    require_pliron_ranked_bounds_before_lowering_v1, run_pliron_ranked_bounds_check_v1,
};
use pliron::{
    basic_block::BasicBlock,
    builtin::{
        op_interfaces::OneRegionInterface,
        ops::FuncOp,
        types::{FunctionType, UnitType},
    },
    common_traits::Named,
    context::{Context, Ptr},
    dialect::DialectName,
    op::Op,
    operation::Operation,
    r#type::TypeHandle,
    value::Value,
};

fn setup() -> Context {
    let mut context = Context::new();
    let dialect = DialectName::try_new(DIALECT_NAME).unwrap();
    register_dialect(&mut context, &dialect).unwrap();
    context
}

fn function(context: &mut Context, name: &str, arguments: usize) -> (FuncOp, Vec<Value>) {
    let index_type: TypeHandle = IndexType::get(context).into();
    let function_type = FunctionType::get(context, vec![index_type; arguments], vec![]);
    let function = FuncOp::new(context, name.try_into().unwrap(), function_type);
    let entry = function.get_entry_block(context);
    let arguments = entry.deref(context).arguments().collect();
    (function, arguments)
}

fn block(context: &mut Context, function: &FuncOp, label: &str) -> Ptr<BasicBlock> {
    let block = BasicBlock::new(context, Some(label.try_into().unwrap()), vec![]);
    block.insert_at_back(function.get_region(context), context);
    block
}

fn append<O: Op>(context: &Context, block: Ptr<BasicBlock>, operation: &O) {
    operation.get_operation().insert_at_back(block, context);
}

#[test]
fn malformed_function_fails_structural_verification_before_analysis() {
    let context = &mut setup();
    let (function, _) = function(context, "unterminated", 0);

    let report = run_pliron_ranked_bounds_check_v1(context, &function);
    assert_eq!(
        report.findings(),
        &[RankedBoundsFindingV1::StructuralVerificationFailed]
    );
}

#[test]
fn kernel_return_rejects_a_nonvoid_function_signature() {
    let context = &mut setup();
    let unit: TypeHandle = UnitType::get(context).into();
    let function_type = FunctionType::get(context, vec![], vec![unit]);
    let function = FuncOp::new(context, "nonvoid".try_into().unwrap(), function_type);
    let ret = ReturnOp::new(context);
    append(context, function.get_entry_block(context), &ret);

    assert_eq!(
        run_pliron_ranked_bounds_check_v1(context, &function).findings(),
        &[RankedBoundsFindingV1::StructuralVerificationFailed]
    );
}

#[test]
fn kernel_trap_is_a_distinct_valid_void_terminator() {
    let context = &mut setup();
    let (function, _) = function(context, "trapping", 0);
    let trap = TrapOp::new(context);
    append(context, function.get_entry_block(context), &trap);

    assert!(run_pliron_ranked_bounds_check_v1(context, &function).is_clean());

    let unit: TypeHandle = UnitType::get(context).into();
    let function_type = FunctionType::get(context, vec![], vec![unit]);
    let nonvoid = FuncOp::new(context, "nonvoid_trap".try_into().unwrap(), function_type);
    let trap = TrapOp::new(context);
    append(context, nonvoid.get_entry_block(context), &trap);
    assert_eq!(
        run_pliron_ranked_bounds_check_v1(context, &nonvoid).findings(),
        &[RankedBoundsFindingV1::StructuralVerificationFailed]
    );
}

#[test]
fn unreachable_blocks_are_rejected_by_the_closed_kernel_cfg() {
    let context = &mut setup();
    let (function, _) = function(context, "unreachable", 0);
    let entry = function.get_entry_block(context);
    let dead = block(context, &function, "dead");
    let entry_return = ReturnOp::new(context);
    let dead_return = ReturnOp::new(context);
    append(context, entry, &entry_return);
    append(context, dead, &dead_return);

    let report = run_pliron_ranked_bounds_check_v1(context, &function);
    assert_eq!(
        report.findings(),
        &[RankedBoundsFindingV1::UnreachableBlock { block: 1 }]
    );
}

#[test]
fn block_resource_limit_fails_closed_before_dataflow() {
    let context = &mut setup();
    let (function, _) = function(context, "too_many_blocks", 0);
    let mut current = function.get_entry_block(context);
    for index in 0..MAX_RANKED_BOUNDS_BLOCKS {
        let next = block(context, &function, &format!("b{index}"));
        let branch = BranchOp::new(context, next);
        append(context, current, &branch);
        current = next;
    }
    let ret = ReturnOp::new(context);
    append(context, current, &ret);

    let report = run_pliron_ranked_bounds_check_v1(context, &function);
    assert_eq!(
        report.findings(),
        &[RankedBoundsFindingV1::ResourceLimitExceeded {
            resource: "basic block",
            limit: MAX_RANKED_BOUNDS_BLOCKS,
            actual: MAX_RANKED_BOUNDS_BLOCKS + 1,
        }]
    );
}

#[test]
fn effecting_general_gemm_operation_is_rejected_by_the_closed_allowlist() {
    let context = &mut setup();
    let (function, _) = function(context, "foreign_effect", 0);
    let entry = function.get_entry_block(context);
    let gemm = GeneralGemmOp::canonical(context);
    let ret = ReturnOp::new(context);
    append(context, entry, &gemm);
    append(context, entry, &ret);

    let report = run_pliron_ranked_bounds_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert_eq!(
        report.findings(),
        &[RankedBoundsFindingV1::UnsupportedOperation {
            block: 0,
            operation: 0,
            kind: "kernel.general_gemm".to_owned(),
        }]
    );
    assert!(require_pliron_ranked_bounds_before_lowering_v1(context, &function).is_err());
}

#[test]
fn unknown_operation_in_an_unreachable_block_is_still_rejected() {
    let context = &mut setup();
    let (function, _) = function(context, "dead_unknown", 0);
    let entry = function.get_entry_block(context);
    let dead = block(context, &function, "dead");
    let entry_return = ReturnOp::new(context);
    let unknown = AlgorithmOp::new(context, 1).unwrap();
    let dead_return = ReturnOp::new(context);
    append(context, entry, &entry_return);
    append(context, dead, &unknown);
    append(context, dead, &dead_return);

    let report = run_pliron_ranked_bounds_check_v1(context, &function);
    assert_eq!(
        report.findings(),
        &[RankedBoundsFindingV1::UnsupportedOperation {
            block: 1,
            operation: 0,
            kind: "kernel.algorithm_root".to_owned(),
        }]
    );
    assert!(!report.is_clean());
}

#[test]
fn oversized_successor_vector_fails_before_structural_verification() {
    let context = &mut setup();
    let (function, _) = function(context, "successor_amplification", 0);
    let entry = function.get_entry_block(context);
    let branch = BranchOp::new(context, entry);
    for _ in 0..MAX_RANKED_BOUNDS_EDGES {
        Operation::push_successor(branch.get_operation(), context, entry);
    }
    append(context, entry, &branch);

    assert_eq!(
        run_pliron_ranked_bounds_check_v1(context, &function).findings(),
        &[RankedBoundsFindingV1::ResourceLimitExceeded {
            resource: "CFG edge",
            limit: MAX_RANKED_BOUNDS_EDGES,
            actual: MAX_RANKED_BOUNDS_EDGES + 1,
        }]
    );
}

#[test]
fn hostile_finding_amplification_is_bounded() {
    let context = &mut setup();
    let (function, _) = function(context, "finding_amplification", 0);
    let entry = function.get_entry_block(context);
    let view_type = RankedViewType::new(context, 32, false, vec![1; 8]).unwrap();
    let view = RankedViewOp::new(context, view_type, vec![]).unwrap();
    let index = IndexConstantOp::new(context, 1);
    append(context, entry, &view);
    append(context, entry, &index);
    for _ in 0..(MAX_RANKED_BOUNDS_FINDINGS / 8 + 1) {
        let access = RankedAccessOp::new(
            context,
            AccessKindAttr::Read,
            view.result(context),
            vec![index.result(context); 8],
        )
        .unwrap();
        append(context, entry, &access);
    }
    let ret = ReturnOp::new(context);
    append(context, entry, &ret);

    assert_eq!(
        run_pliron_ranked_bounds_check_v1(context, &function).findings(),
        &[RankedBoundsFindingV1::ResourceLimitExceeded {
            resource: "finding",
            limit: MAX_RANKED_BOUNDS_FINDINGS,
            actual: MAX_RANKED_BOUNDS_FINDINGS + 1,
        }]
    );
}

#[test]
fn static_ranked_accesses_are_proved_without_explicit_guards() {
    let context = &mut setup();
    let (function, _) = function(context, "static_image_copy", 0);
    let entry = function.get_entry_block(context);
    let view_type = RankedViewType::new(context, 32, false, vec![32, 64]).unwrap();
    let view = RankedViewOp::new(context, view_type, vec![]).unwrap();
    let row = IndexConstantOp::new(context, 4);
    let column = IndexConstantOp::new(context, 7);
    let access = RankedAccessOp::new(
        context,
        AccessKindAttr::Read,
        view.result(context),
        vec![row.result(context), column.result(context)],
    )
    .unwrap();
    let ret = ReturnOp::new(context);
    append(context, entry, &view);
    append(context, entry, &row);
    append(context, entry, &column);
    append(context, entry, &access);
    append(context, entry, &ret);

    let report = run_pliron_ranked_bounds_check_v1(context, &function);
    assert_eq!(report.pass(), KernelCheckPassKindV1::MemoryBounds);
    assert_eq!(report.status(), KernelCheckStatusV1::Clean);
    assert!(report.findings().is_empty());
    assert!(!report.grants_compiler_refinement_authority());
    assert!(!report.grants_artifact_or_launch_authority());
}

#[test]
fn statically_out_of_bounds_dimension_has_exact_diagnostic() {
    let context = &mut setup();
    let (function, _) = function(context, "static_image_oob", 0);
    let entry = function.get_entry_block(context);
    let view_type = RankedViewType::new(context, 32, false, vec![32, 64]).unwrap();
    let view = RankedViewOp::new(context, view_type, vec![]).unwrap();
    let row = IndexConstantOp::new(context, 4);
    let column = IndexConstantOp::new(context, 64);
    let access = RankedAccessOp::new(
        context,
        AccessKindAttr::Read,
        view.result(context),
        vec![row.result(context), column.result(context)],
    )
    .unwrap();
    let ret = ReturnOp::new(context);
    append(context, entry, &view);
    append(context, entry, &row);
    append(context, entry, &column);
    append(context, entry, &access);
    append(context, entry, &ret);

    let report = run_pliron_ranked_bounds_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Rejected);
    assert_eq!(report.findings().len(), 1);
    assert!(matches!(
        &report.findings()[0],
        RankedBoundsFindingV1::StaticOutOfBounds {
            access: AccessKindAttr::Read,
            dimension: 1,
            index: 64,
            extent: 64,
            ..
        }
    ));
    assert_eq!(
        report.findings()[0].to_string(),
        "error[FE2O3-BOUNDS-001]: statically out-of-bounds Read at block 0 op 3; access: v0 dimension 1; required: 64 < 64",
    );
    let error = require_pliron_ranked_bounds_before_lowering_v1(context, &function).unwrap_err();
    assert_eq!(error.report(), &report);
    assert_eq!(error.to_string(), report.findings()[0].to_string());
}

#[test]
fn unguarded_dynamic_access_reports_each_unproved_dimension() {
    let context = &mut setup();
    let (function, arguments) = function(context, "dynamic_image_unguarded", 4);
    let [row, column, height, width]: [Value; 4] = arguments.try_into().unwrap();
    let entry = function.get_entry_block(context);
    let view_type = RankedViewType::new(context, 32, false, vec![0, 0]).unwrap();
    let view = RankedViewOp::new(context, view_type, vec![height, width]).unwrap();
    let access = RankedAccessOp::new(
        context,
        AccessKindAttr::Read,
        view.result(context),
        vec![row, column],
    )
    .unwrap();
    let ret = ReturnOp::new(context);
    append(context, entry, &view);
    append(context, entry, &access);
    append(context, entry, &ret);

    let report = run_pliron_ranked_bounds_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert_eq!(report.findings().len(), 2);
    assert!(matches!(
        report.findings()[0],
        RankedBoundsFindingV1::UnprovedBound { dimension: 0, .. }
    ));
    assert!(matches!(
        report.findings()[1],
        RankedBoundsFindingV1::UnprovedBound { dimension: 1, .. }
    ));
    let RankedBoundsFindingV1::UnprovedBound {
        view: view_name,
        index,
        extent,
        ..
    } = &report.findings()[0]
    else {
        unreachable!()
    };
    assert_eq!(
        view_name,
        &view.result(context).unique_name(context).to_string()
    );
    assert_eq!(index, &row.unique_name(context).to_string());
    assert_eq!(extent, &height.unique_name(context).to_string());
    let diagnostic = report.findings()[0].to_string();
    assert!(diagnostic.contains("unproven bound"));
    assert!(diagnostic.contains("guard every path to the access"));
}

#[test]
fn equality_control_does_not_establish_a_dynamic_range_bound() {
    let context = &mut setup();
    let (function, arguments) = function(context, "equality_is_not_a_bound", 2);
    let [index, length]: [Value; 2] = arguments.try_into().unwrap();
    let entry = function.get_entry_block(context);
    let access_block = block(context, &function, "access");
    let exit = block(context, &function, "exit");
    let view_type = RankedViewType::new(context, 32, false, vec![0]).unwrap();
    let view = RankedViewOp::new(context, view_type, vec![length]).unwrap();
    let summary = DeterministicJoinOp::new(context, vec![index, length]);
    let zero = IndexConstantOp::new(context, 0);
    let branch = IndexEqualBranchOp::new(
        context,
        summary.result(context),
        zero.result(context),
        access_block,
        exit,
    );
    let access = RankedAccessOp::new(
        context,
        AccessKindAttr::Read,
        view.result(context),
        vec![index],
    )
    .unwrap();
    let to_exit = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    append(context, entry, &view);
    append(context, entry, &summary);
    append(context, entry, &zero);
    append(context, entry, &branch);
    append(context, access_block, &access);
    append(context, access_block, &to_exit);
    append(context, exit, &ret);

    let report = run_pliron_ranked_bounds_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert!(matches!(
        report.findings(),
        [RankedBoundsFindingV1::UnprovedBound { dimension: 0, .. }]
    ));
}

#[test]
fn nested_dynamic_guards_prove_a_two_dimensional_access() {
    let context = &mut setup();
    let (function, arguments) = function(context, "dynamic_image_guarded", 4);
    let [row, column, height, width]: [Value; 4] = arguments.try_into().unwrap();
    let entry = function.get_entry_block(context);
    let column_guard = block(context, &function, "column_guard");
    let access_block = block(context, &function, "access");
    let exit = block(context, &function, "exit");
    let view_type = RankedViewType::new(context, 32, true, vec![0, 0]).unwrap();
    let view = RankedViewOp::new(context, view_type, vec![height, width]).unwrap();
    let rows = DimensionOp::new(context, view.result(context), 0).unwrap();
    let columns = DimensionOp::new(context, view.result(context), 1).unwrap();
    let row_branch =
        IndexLessThanBranchOp::new(context, row, rows.result(context), column_guard, exit);
    let column_branch =
        IndexLessThanBranchOp::new(context, column, columns.result(context), access_block, exit);
    let access = RankedAccessOp::new(
        context,
        AccessKindAttr::Write,
        view.result(context),
        vec![row, column],
    )
    .unwrap();
    let to_exit = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    append(context, entry, &view);
    append(context, entry, &rows);
    append(context, entry, &columns);
    append(context, entry, &row_branch);
    append(context, column_guard, &column_branch);
    append(context, access_block, &access);
    append(context, access_block, &to_exit);
    append(context, exit, &ret);

    let report = run_pliron_ranked_bounds_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Clean);
}

#[test]
fn guard_against_the_runtime_shape_operand_proves_the_access() {
    let context = &mut setup();
    let (function, arguments) = function(context, "dynamic_vector_guarded", 2);
    let [index, length]: [Value; 2] = arguments.try_into().unwrap();
    let entry = function.get_entry_block(context);
    let access_block = block(context, &function, "access");
    let exit = block(context, &function, "exit");
    let view_type = RankedViewType::new(context, 32, false, vec![0]).unwrap();
    let view = RankedViewOp::new(context, view_type, vec![length]).unwrap();
    let branch = IndexLessThanBranchOp::new(context, index, length, access_block, exit);
    let access = RankedAccessOp::new(
        context,
        AccessKindAttr::Read,
        view.result(context),
        vec![index],
    )
    .unwrap();
    let to_exit = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    append(context, entry, &view);
    append(context, entry, &branch);
    append(context, access_block, &access);
    append(context, access_block, &to_exit);
    append(context, exit, &ret);

    assert!(run_pliron_ranked_bounds_check_v1(context, &function).is_clean());
}

#[test]
fn false_guard_edge_does_not_prove_the_access() {
    let context = &mut setup();
    let (function, arguments) = function(context, "false_edge_access", 2);
    let [index, length]: [Value; 2] = arguments.try_into().unwrap();
    let entry = function.get_entry_block(context);
    let access_block = block(context, &function, "access");
    let exit = block(context, &function, "exit");
    let view_type = RankedViewType::new(context, 32, false, vec![0]).unwrap();
    let view = RankedViewOp::new(context, view_type, vec![length]).unwrap();
    let branch = IndexLessThanBranchOp::new(context, index, length, exit, access_block);
    let access = RankedAccessOp::new(
        context,
        AccessKindAttr::Read,
        view.result(context),
        vec![index],
    )
    .unwrap();
    let to_exit = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    append(context, entry, &view);
    append(context, entry, &branch);
    append(context, access_block, &access);
    append(context, access_block, &to_exit);
    append(context, exit, &ret);

    let report = run_pliron_ranked_bounds_check_v1(context, &function);
    assert!(matches!(
        report.findings(),
        [RankedBoundsFindingV1::UnprovedBound { dimension: 0, .. }]
    ));
}

#[test]
fn conditional_with_the_same_successor_does_not_manufacture_a_guard() {
    let context = &mut setup();
    let (function, arguments) = function(context, "same_successor", 2);
    let [index, length]: [Value; 2] = arguments.try_into().unwrap();
    let entry = function.get_entry_block(context);
    let access_block = block(context, &function, "access");
    let view_type = RankedViewType::new(context, 32, false, vec![0]).unwrap();
    let view = RankedViewOp::new(context, view_type, vec![length]).unwrap();
    let branch = IndexLessThanBranchOp::new(context, index, length, access_block, access_block);
    let access = RankedAccessOp::new(
        context,
        AccessKindAttr::Read,
        view.result(context),
        vec![index],
    )
    .unwrap();
    let ret = ReturnOp::new(context);
    append(context, entry, &view);
    append(context, entry, &branch);
    append(context, access_block, &access);
    append(context, access_block, &ret);

    let report = run_pliron_ranked_bounds_check_v1(context, &function);
    assert!(matches!(
        report.findings(),
        [RankedBoundsFindingV1::UnprovedBound { dimension: 0, .. }]
    ));
}

#[test]
fn unknown_index_into_static_shape_is_proved_by_the_matching_guard() {
    let context = &mut setup();
    let (function, arguments) = function(context, "static_vector_guarded", 1);
    let [index]: [Value; 1] = arguments.try_into().unwrap();
    let entry = function.get_entry_block(context);
    let access_block = block(context, &function, "access");
    let exit = block(context, &function, "exit");
    let view_type = RankedViewType::new(context, 32, false, vec![64]).unwrap();
    let view = RankedViewOp::new(context, view_type, vec![]).unwrap();
    let length = IndexConstantOp::new(context, 64);
    let branch =
        IndexLessThanBranchOp::new(context, index, length.result(context), access_block, exit);
    let access = RankedAccessOp::new(
        context,
        AccessKindAttr::Read,
        view.result(context),
        vec![index],
    )
    .unwrap();
    let to_exit = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    append(context, entry, &view);
    append(context, entry, &length);
    append(context, entry, &branch);
    append(context, access_block, &access);
    append(context, access_block, &to_exit);
    append(context, exit, &ret);

    assert!(
        require_pliron_ranked_bounds_before_lowering_v1(context, &function)
            .unwrap()
            .is_clean()
    );
}

#[test]
fn partial_guard_reports_only_the_unsafe_dimension() {
    let context = &mut setup();
    let (function, arguments) = function(context, "dynamic_image_partial_guard", 4);
    let [row, column, height, width]: [Value; 4] = arguments.try_into().unwrap();
    let entry = function.get_entry_block(context);
    let access_block = block(context, &function, "access");
    let exit = block(context, &function, "exit");
    let view_type = RankedViewType::new(context, 16, false, vec![0, 0]).unwrap();
    let view = RankedViewOp::new(context, view_type, vec![height, width]).unwrap();
    let columns = DimensionOp::new(context, view.result(context), 1).unwrap();
    let branch =
        IndexLessThanBranchOp::new(context, column, columns.result(context), access_block, exit);
    let access = RankedAccessOp::new(
        context,
        AccessKindAttr::Read,
        view.result(context),
        vec![row, column],
    )
    .unwrap();
    let to_exit = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    append(context, entry, &view);
    append(context, entry, &columns);
    append(context, entry, &branch);
    append(context, access_block, &access);
    append(context, access_block, &to_exit);
    append(context, exit, &ret);

    let report = run_pliron_ranked_bounds_check_v1(context, &function);
    assert_eq!(report.findings().len(), 1);
    assert!(matches!(
        report.findings()[0],
        RankedBoundsFindingV1::UnprovedBound { dimension: 0, .. }
    ));
    let diagnostic = report.findings()[0].to_string();
    assert!(diagnostic.contains("unproven bound"));
    assert!(diagnostic.contains("dimension 0"));
}

#[test]
fn a_guard_for_another_view_cannot_prove_this_view_safe() {
    let context = &mut setup();
    let (function, arguments) = function(context, "wrong_view_guard", 4);
    let [row, input_height, other_height, width]: [Value; 4] = arguments.try_into().unwrap();
    let entry = function.get_entry_block(context);
    let access_block = block(context, &function, "access");
    let exit = block(context, &function, "exit");
    let input_type = RankedViewType::new(context, 32, false, vec![0, 1]).unwrap();
    let other_type = RankedViewType::new(context, 32, false, vec![0, 0]).unwrap();
    let input = RankedViewOp::new(context, input_type, vec![input_height]).unwrap();
    let other = RankedViewOp::new(context, other_type, vec![other_height, width]).unwrap();
    let other_rows = DimensionOp::new(context, other.result(context), 0).unwrap();
    let branch =
        IndexLessThanBranchOp::new(context, row, other_rows.result(context), access_block, exit);
    let zero = IndexConstantOp::new(context, 0);
    let access = RankedAccessOp::new(
        context,
        AccessKindAttr::Read,
        input.result(context),
        vec![row, zero.result(context)],
    )
    .unwrap();
    let to_exit = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    append(context, entry, &input);
    append(context, entry, &other);
    append(context, entry, &other_rows);
    append(context, entry, &zero);
    append(context, entry, &branch);
    append(context, access_block, &access);
    append(context, access_block, &to_exit);
    append(context, exit, &ret);

    let report = run_pliron_ranked_bounds_check_v1(context, &function);
    assert_eq!(report.findings().len(), 1);
    assert!(matches!(
        report.findings()[0],
        RankedBoundsFindingV1::UnprovedBound { dimension: 0, .. }
    ));
}

#[test]
fn path_merge_intersects_facts_instead_of_using_path_insensitive_union() {
    let context = &mut setup();
    let (function, arguments) = function(context, "merged_guard", 2);
    let [index, extent]: [Value; 2] = arguments.try_into().unwrap();
    let entry = function.get_entry_block(context);
    let guarded = block(context, &function, "guarded");
    let bypass = block(context, &function, "bypass");
    let join = block(context, &function, "join");
    let view_type = RankedViewType::new(context, 32, false, vec![0]).unwrap();
    let view = RankedViewOp::new(context, view_type, vec![extent]).unwrap();
    let size = DimensionOp::new(context, view.result(context), 0).unwrap();
    let branch = IndexLessThanBranchOp::new(context, index, size.result(context), guarded, bypass);
    let guarded_to_join = BranchOp::new(context, join);
    let bypass_to_join = BranchOp::new(context, join);
    let access = RankedAccessOp::new(
        context,
        AccessKindAttr::Read,
        view.result(context),
        vec![index],
    )
    .unwrap();
    let ret = ReturnOp::new(context);
    append(context, entry, &view);
    append(context, entry, &size);
    append(context, entry, &branch);
    append(context, guarded, &guarded_to_join);
    append(context, bypass, &bypass_to_join);
    append(context, join, &access);
    append(context, join, &ret);

    let report = run_pliron_ranked_bounds_check_v1(context, &function);
    assert_eq!(report.findings().len(), 1);
    assert!(matches!(
        report.findings()[0],
        RankedBoundsFindingV1::UnprovedBound { dimension: 0, .. }
    ));
}

#[test]
fn dominating_guard_remains_valid_across_a_loop_backedge() {
    let context = &mut setup();
    let (function, arguments) = function(context, "guarded_vector_loop", 2);
    let [index, extent]: [Value; 2] = arguments.try_into().unwrap();
    let entry = function.get_entry_block(context);
    let body = block(context, &function, "body");
    let exit = block(context, &function, "exit");
    let view_type = RankedViewType::new(context, 32, false, vec![0]).unwrap();
    let view = RankedViewOp::new(context, view_type, vec![extent]).unwrap();
    let size = DimensionOp::new(context, view.result(context), 0).unwrap();
    let branch = IndexLessThanBranchOp::new(context, index, size.result(context), body, exit);
    let access = RankedAccessOp::new(
        context,
        AccessKindAttr::Read,
        view.result(context),
        vec![index],
    )
    .unwrap();
    let backedge = BranchOp::new(context, body);
    let ret = ReturnOp::new(context);
    append(context, entry, &view);
    append(context, entry, &size);
    append(context, entry, &branch);
    append(context, body, &access);
    append(context, body, &backedge);
    append(context, exit, &ret);

    let report = run_pliron_ranked_bounds_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Clean);
}

#[test]
fn rank_one_vector_and_rank_three_volume_use_the_same_non_gemm_pass() {
    let context = &mut setup();

    let (vector, _) = function(context, "vector_lookup", 0);
    let vector_entry = vector.get_entry_block(context);
    let vector_type = RankedViewType::new(context, 32, false, vec![128]).unwrap();
    let vector_view = RankedViewOp::new(context, vector_type, vec![]).unwrap();
    let vector_index = IndexConstantOp::new(context, 127);
    let vector_access = RankedAccessOp::new(
        context,
        AccessKindAttr::Read,
        vector_view.result(context),
        vec![vector_index.result(context)],
    )
    .unwrap();
    let vector_return = ReturnOp::new(context);
    append(context, vector_entry, &vector_view);
    append(context, vector_entry, &vector_index);
    append(context, vector_entry, &vector_access);
    append(context, vector_entry, &vector_return);
    assert!(run_pliron_ranked_bounds_check_v1(context, &vector).is_clean());

    let (volume, _) = function(context, "volume_sample", 0);
    let volume_entry = volume.get_entry_block(context);
    let volume_type = RankedViewType::new(context, 16, false, vec![8, 16, 32]).unwrap();
    let volume_view = RankedViewOp::new(context, volume_type, vec![]).unwrap();
    let z = IndexConstantOp::new(context, 7);
    let y = IndexConstantOp::new(context, 15);
    let x = IndexConstantOp::new(context, 31);
    let volume_access = RankedAccessOp::new(
        context,
        AccessKindAttr::Read,
        volume_view.result(context),
        vec![z.result(context), y.result(context), x.result(context)],
    )
    .unwrap();
    let volume_return = ReturnOp::new(context);
    append(context, volume_entry, &volume_view);
    append(context, volume_entry, &z);
    append(context, volume_entry, &y);
    append(context, volume_entry, &x);
    append(context, volume_entry, &volume_access);
    append(context, volume_entry, &volume_return);
    assert!(run_pliron_ranked_bounds_check_v1(context, &volume).is_clean());
}
