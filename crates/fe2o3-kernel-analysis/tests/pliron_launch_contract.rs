use dialect_gpu::{ExecutionExtentAttr, ExecutionLayoutOp};
use dialect_kernel::{
    DIALECT_NAME, DYNAMIC_EXTENT, IndexConstantOp, MemorySpaceAttr, RankedViewOp, RankedViewType,
    ReturnOp, register_dialect,
};
use fe2o3_kernel_analysis::{
    KernelCheckRepairActionV1, KernelCheckStatusV1, PlironHostAllocationV1,
    PlironLaunchContractFindingV1, PlironLaunchContractInputErrorV1, PlironLaunchContractV1,
    PlironLaunchTargetLimitsV1, ProductionPlironPreloweringErrorV2,
    require_production_pliron_checks_with_target_before_lowering_v2,
    run_pliron_launch_contract_check_v1,
};
use pliron::{
    basic_block::BasicBlock,
    builtin::{ops::FuncOp, types::FunctionType},
    context::{Context, Ptr},
    dialect::DialectName,
    op::Op,
};

fn setup() -> Context {
    let mut context = Context::new();
    register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
    dialect_gpu::register_dialect(&mut context).unwrap();
    dialect_proof::register_dialect(&mut context).unwrap();
    context
}

fn function(context: &mut Context, name: &str, layout: [u64; 3], subgroup: u64) -> FuncOp {
    let function = FuncOp::new(
        context,
        name.try_into().unwrap(),
        FunctionType::get(context, vec![], vec![]),
    );
    let execution = ExecutionLayoutOp::new(context, 7, [256, 1, 1], layout, subgroup);
    append(context, function.get_entry_block(context), &execution);
    function
}

fn append<O: Op>(context: &Context, block: Ptr<BasicBlock>, operation: &O) {
    operation.get_operation().insert_at_back(block, context);
}

fn view(
    context: &mut Context,
    shape: Vec<u64>,
    memory_space: MemorySpaceAttr,
    origin: u64,
) -> RankedViewOp {
    let ty = RankedViewType::new(context, 32, true, shape).unwrap();
    RankedViewOp::new_in_space_with_allocation_contract(
        context,
        ty,
        vec![],
        memory_space,
        origin,
        origin,
    )
    .unwrap()
}

fn limits(max_lds: u64, subgroups: Vec<u64>) -> PlironLaunchTargetLimitsV1 {
    PlironLaunchTargetLimitsV1::new(
        [65_535, 65_535, 65_535],
        [1_024, 1_024, 64],
        1_024,
        subgroups,
        max_lds,
        16,
        8,
    )
    .unwrap()
}

fn contract(
    max_lds: u64,
    subgroups: Vec<u64>,
    host_bytes: u64,
    host_alignment: u64,
) -> PlironLaunchContractV1 {
    PlironLaunchContractV1::new(
        limits(max_lds, subgroups),
        vec![PlironHostAllocationV1::new(2, host_bytes, host_alignment).unwrap()],
    )
    .unwrap()
}

fn append_static_resources(context: &mut Context, function: &FuncOp, lds_elements: u64) {
    let entry = function.get_entry_block(context);
    let lds = view(context, vec![lds_elements], MemorySpaceAttr::Workgroup, 1);
    let global = view(context, vec![16], MemorySpaceAttr::Global, 2);
    let ret = ReturnOp::new(context);
    append(context, entry, &lds);
    append(context, entry, &global);
    append(context, entry, &ret);
}

#[test]
fn static_launch_lds_and_host_abi_are_checked_in_the_production_pipeline() {
    let context = &mut setup();
    let function = function(context, "target_clean", [64, 1, 1], 64);
    append_static_resources(context, &function, 1_024);
    let contract = contract(65_536, vec![32, 64], 64, 16);
    let report = require_production_pliron_checks_with_target_before_lowering_v2(
        context, &function, &contract,
    )
    .unwrap();
    assert_eq!(report.pass_order().len(), 8);
    let target = report.target_contract().unwrap();
    assert_eq!(target.workgroup_memory_bytes(), Some(4_096));
    assert_eq!(target.checked_global_allocation_count(), 1);
    assert!(!target.grants_launch_authority());
}

#[test]
fn oversized_workgroup_lds_and_host_binding_report_independent_repairs() {
    let context = &mut setup();
    let function = function(context, "target_failures", [2_048, 1, 1], 64);
    append_static_resources(context, &function, 20_000);
    let contract = contract(65_536, vec![32], 32, 8);
    let report = run_pliron_launch_contract_check_v1(context, &function, &contract);
    assert_eq!(report.status(), KernelCheckStatusV1::Rejected);
    assert!(report.findings().iter().any(|finding| matches!(
        finding,
        PlironLaunchContractFindingV1::WorkgroupExtentExceedsTarget {
            axis: 0,
            actual: 2_048,
            ..
        }
    )));
    assert!(report.findings().iter().any(|finding| matches!(
        finding,
        PlironLaunchContractFindingV1::WorkgroupMemoryExceedsTarget {
            actual: 80_000,
            limit: 65_536
        }
    )));
    assert!(report.findings().iter().any(|finding| matches!(
        finding,
        PlironLaunchContractFindingV1::UnsupportedSubgroupSize { actual: 64, .. }
    )));
    assert!(report.findings().iter().any(|finding| matches!(
        finding,
        PlironLaunchContractFindingV1::HostAllocationTooSmall {
            required: 64,
            available: 32,
            ..
        }
    )));
    assert!(report.findings().iter().any(|finding| matches!(
        finding,
        PlironLaunchContractFindingV1::HostAllocationAlignmentInsufficient {
            required: 16,
            guaranteed: 8,
            ..
        }
    )));
}

#[test]
fn origin_substitution_cannot_reuse_an_unrelated_host_descriptor() {
    let context = &mut setup();
    let function = function(context, "origin_substitution", [64, 1, 1], 64);
    append_static_resources(context, &function, 1_024);
    let substituted = PlironLaunchContractV1::new(
        limits(65_536, vec![64]),
        vec![PlironHostAllocationV1::new(3, 1_024, 64).unwrap()],
    )
    .unwrap();
    let report = run_pliron_launch_contract_check_v1(context, &function, &substituted);
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert!(report.findings().iter().any(|finding| matches!(
        finding,
        PlironLaunchContractFindingV1::MissingHostAllocation { origin: 2, .. }
    )));
}

#[test]
fn dynamic_allocation_size_fails_closed_and_duplicate_bindings_are_invalid() {
    let context = &mut setup();
    let function = function(context, "dynamic_abi", [64, 1, 1], 64);
    let entry = function.get_entry_block(context);
    let extent = IndexConstantOp::new(context, 16);
    let ty = RankedViewType::new(context, 32, true, vec![DYNAMIC_EXTENT]).unwrap();
    let global = RankedViewOp::new_in_space_with_allocation_contract(
        context,
        ty,
        vec![extent.result(context)],
        MemorySpaceAttr::Global,
        2,
        2,
    )
    .unwrap();
    let ret = ReturnOp::new(context);
    append(context, entry, &extent);
    append(context, entry, &global);
    append(context, entry, &ret);
    let contract = contract(65_536, vec![64], 64, 16);
    let report = run_pliron_launch_contract_check_v1(context, &function, &contract);
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert!(matches!(
        report.findings(),
        [PlironLaunchContractFindingV1::GlobalViewSizeUnknown { dimension: 0, .. }]
    ));

    let allocation = PlironHostAllocationV1::new(2, 64, 16).unwrap();
    assert_eq!(
        PlironLaunchContractV1::new(limits(65_536, vec![64]), vec![allocation, allocation]),
        Err(PlironLaunchContractInputErrorV1::DuplicateHostAllocation { origin: 2 })
    );
}

#[test]
fn production_error_keeps_target_specific_structured_repair() {
    let context = &mut setup();
    let function = function(context, "target_pipeline_error", [64, 1, 1], 64);
    append_static_resources(context, &function, 1_024);
    let missing_host = PlironLaunchContractV1::new(limits(65_536, vec![64]), vec![]).unwrap();
    let error = require_production_pliron_checks_with_target_before_lowering_v2(
        context,
        &function,
        &missing_host,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ProductionPlironPreloweringErrorV2::TargetContract(_)
    ));
    assert_eq!(
        error.repair_hints()[0].action(),
        KernelCheckRepairActionV1::SatisfyTargetContract
    );
    assert!(error.to_string().contains("help[FE2O3-FIX-TARGET]"));
}

#[test]
fn insufficient_alignment_never_counts_as_a_checked_host_binding() {
    let context = &mut setup();
    let alignment = function(context, "host_alignment", [64, 1, 1], 64);
    append_static_resources(context, &alignment, 1_024);
    let report = run_pliron_launch_contract_check_v1(
        context,
        &alignment,
        &contract(65_536, vec![64], 64, 8),
    );
    assert_eq!(report.checked_global_allocation_count(), 0);
    assert!(report.findings().iter().any(|finding| matches!(
        finding,
        PlironLaunchContractFindingV1::HostAllocationAlignmentInsufficient { .. }
    )));
}

#[test]
fn malformed_layout_and_global_size_overflow_never_produce_clean_reports() {
    let context = &mut setup();
    let malformed_function = FuncOp::new(
        context,
        "malformed_layout".try_into().unwrap(),
        FunctionType::get(context, vec![], vec![]),
    );
    let layout = ExecutionLayoutOp::new(context, 7, [64, 1, 1], [64, 1, 1], 64);
    layout.set_attr_gpu_execution_workgroup_x(context, ExecutionExtentAttr(0));
    let ret = ReturnOp::new(context);
    append(
        context,
        malformed_function.get_entry_block(context),
        &layout,
    );
    append(context, malformed_function.get_entry_block(context), &ret);
    let malformed_contract = contract(65_536, vec![64], 64, 16);
    assert!(matches!(
        run_pliron_launch_contract_check_v1(context, &malformed_function, &malformed_contract)
            .findings(),
        [PlironLaunchContractFindingV1::StructuralPrerequisiteRejected]
    ));

    let function = function(context, "global_overflow", [64, 1, 1], 64);
    let entry = function.get_entry_block(context);
    let global = view(
        context,
        vec![u64::MAX, u64::MAX],
        MemorySpaceAttr::Global,
        2,
    );
    let ret = ReturnOp::new(context);
    append(context, entry, &global);
    append(context, entry, &ret);
    let contract = contract(65_536, vec![64], u64::MAX, 16);
    let report = run_pliron_launch_contract_check_v1(context, &function, &contract);
    assert_eq!(report.status(), KernelCheckStatusV1::Rejected);
    assert!(matches!(
        report.findings(),
        [PlironLaunchContractFindingV1::GlobalViewSizeArithmeticOverflow { origin: 2, .. }]
    ));
}
