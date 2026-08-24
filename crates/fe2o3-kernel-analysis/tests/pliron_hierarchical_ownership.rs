use dialect_gpu::{ExecutionDomainAttr, ExecutionLayoutOp};
use dialect_kernel::{
    AccessKindAttr, BranchOp, DIALECT_NAME, DimensionOp, IndexBinaryKindAttr, IndexBinaryOp,
    IndexConstantOp, IndexLessThanBranchOp, IndexType, IndexUnknownOp, InvocationIndexOp,
    MemorySpaceAttr, OwnershipContractOp, OwnershipCoverageAttr, OwnershipPartitionAttr,
    RankedAccessOp, RankedMemoryError, RankedViewOp, RankedViewType, ReturnOp, register_dialect,
};
use fe2o3_kernel_analysis::{
    HierarchicalOverlapClassV1, HierarchicalOwnershipFindingV1, HierarchicalOwnershipLevelV1,
    HierarchicalRegionIdentityV1, KernelCheckPassKindV1, KernelCheckStatusV1,
    PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2, ProductionPlironPreloweringErrorV2,
    require_production_pliron_checks_before_lowering_v2,
    run_pliron_hierarchical_ownership_check_v1,
};
use pliron::{
    basic_block::BasicBlock,
    builtin::{op_interfaces::OneRegionInterface, ops::FuncOp, types::FunctionType},
    context::{Context, Ptr},
    dialect::DialectName,
    op::Op,
    r#type::TypeHandle,
    value::Value,
};

fn setup() -> Context {
    let mut context = Context::new();
    register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
    dialect_gpu::register_dialect(&mut context).unwrap();
    context
}

fn function(context: &mut Context, name: &str, arguments: usize) -> (FuncOp, Vec<Value>) {
    let index: TypeHandle = IndexType::get(context).into();
    let function = FuncOp::new(
        context,
        name.try_into().unwrap(),
        FunctionType::get(context, vec![index; arguments], vec![]),
    );
    let entry = function.get_entry_block(context);
    let arguments = entry.deref(context).arguments().collect();
    (function, arguments)
}

fn block(context: &mut Context, function: &FuncOp, name: &str) -> Ptr<BasicBlock> {
    let block = BasicBlock::new(context, Some(name.try_into().unwrap()), vec![]);
    block.insert_at_back(function.get_region(context), context);
    block
}

fn append<O: Op>(context: &Context, block: Ptr<BasicBlock>, operation: &O) {
    operation.get_operation().insert_at_back(block, context);
}

fn layout(
    context: &mut Context,
    global: [u64; 3],
    workgroup: [u64; 3],
    subgroup: u64,
) -> ExecutionLayoutOp {
    ExecutionLayoutOp::new_with_domain(
        context,
        41,
        global,
        workgroup,
        subgroup,
        ExecutionDomainAttr::FullPhysicalWorkgroups,
    )
}

fn view(
    context: &mut Context,
    shape: Vec<u64>,
    dynamic_extents: Vec<Value>,
    memory_space: MemorySpaceAttr,
) -> RankedViewOp {
    let ty = RankedViewType::new(context, 32, true, shape).unwrap();
    RankedViewOp::new_in_space_with_allocation_contract(
        context,
        ty,
        dynamic_extents,
        memory_space,
        17,
        17,
    )
    .unwrap()
}

fn contract(
    context: &mut Context,
    view: Value,
    partition: OwnershipPartitionAttr,
) -> OwnershipContractOp {
    OwnershipContractOp::new(context, view, OwnershipCoverageAttr::ExactView, partition).unwrap()
}

fn write(context: &mut Context, view: Value, indices: Vec<Value>) -> RankedAccessOp {
    RankedAccessOp::new(context, AccessKindAttr::Write, view, indices).unwrap()
}

fn static_1d(
    context: &mut Context,
    name: &str,
    launch: u64,
    workgroup: u64,
    subgroup: u64,
    extent: u64,
    modulus: Option<u64>,
    partition: OwnershipPartitionAttr,
) -> FuncOp {
    let (function, _) = function(context, name, 0);
    let entry = function.get_entry_block(context);
    let execution = layout(context, [launch, 1, 1], [workgroup, 1, 1], subgroup);
    let invocation = InvocationIndexOp::new(context, 0, launch);
    let output = view(context, vec![extent], vec![], MemorySpaceAttr::Global);
    let ownership = contract(context, output.result(context), partition);
    append(context, entry, &execution);
    append(context, entry, &invocation);
    append(context, entry, &output);
    append(context, entry, &ownership);
    let index = if let Some(modulus) = modulus {
        let divisor = IndexConstantOp::new(context, modulus);
        let index = IndexBinaryOp::new(
            context,
            IndexBinaryKindAttr::Remainder,
            invocation.result(context),
            divisor.result(context),
        );
        append(context, entry, &divisor);
        append(context, entry, &index);
        index.result(context)
    } else {
        invocation.result(context)
    };
    let store = write(context, output.result(context), vec![index]);
    let ret = ReturnOp::new(context);
    append(context, entry, &store);
    append(context, entry, &ret);
    function
}

#[test]
fn complete_two_dimensional_domain_builds_all_hierarchy_summaries() {
    let context = &mut setup();
    let (function, _) = function(context, "complete_partition", 0);
    let entry = function.get_entry_block(context);
    let execution = layout(context, [4, 2, 1], [4, 1, 1], 2);
    let x = InvocationIndexOp::new(context, 0, 4);
    let y = InvocationIndexOp::new(context, 1, 2);
    let output = view(context, vec![4, 2], vec![], MemorySpaceAttr::Global);
    let ownership = contract(
        context,
        output.result(context),
        OwnershipPartitionAttr::DenseRectangles,
    );
    let store = write(
        context,
        output.result(context),
        vec![x.result(context), y.result(context)],
    );
    let ret = ReturnOp::new(context);
    for operation in [
        execution.get_operation(),
        x.get_operation(),
        y.get_operation(),
        output.get_operation(),
        ownership.get_operation(),
        store.get_operation(),
        ret.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }

    let report = run_pliron_hierarchical_ownership_check_v1(context, &function);
    assert!(report.is_clean(), "{:#?}", report.findings());
    assert_eq!(report.pass(), KernelCheckPassKindV1::HierarchicalOwnership);
    assert_eq!(report.regions().len(), 15);
    let level_counts = report.regions().iter().fold([0; 4], |mut counts, region| {
        assert!(region.view().starts_with('v'));
        let index = match region.identity().level() {
            HierarchicalOwnershipLevelV1::Invocation => 0,
            HierarchicalOwnershipLevelV1::Subgroup => 1,
            HierarchicalOwnershipLevelV1::Workgroup => 2,
            HierarchicalOwnershipLevelV1::Grid => 3,
        };
        counts[index] += 1;
        assert!(region.is_dense_rectangle());
        counts
    });
    assert_eq!(level_counts, [8, 4, 2, 1]);
    let grid = report
        .regions()
        .iter()
        .find(|region| matches!(region.identity(), HierarchicalRegionIdentityV1::Grid(41)))
        .unwrap();
    assert_eq!(grid.element_count(), 8);
    assert_eq!(grid.bounds()[0].minimum(), 0);
    assert_eq!(grid.bounds()[0].maximum(), 3);
    assert_eq!(grid.bounds()[1].minimum(), 0);
    assert_eq!(grid.bounds()[1].maximum(), 1);
}

#[test]
fn overlap_witness_identifies_within_subgroup_owners() {
    let context = &mut setup();
    let function = static_1d(
        context,
        "within_subgroup_overlap",
        2,
        2,
        2,
        1,
        Some(1),
        OwnershipPartitionAttr::ExactSets,
    );
    let report = run_pliron_hierarchical_ownership_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Rejected);
    assert!(matches!(
        report.findings(),
        [HierarchicalOwnershipFindingV1::OverlappingOwners {
            class: HierarchicalOverlapClassV1::WithinSubgroup,
            coordinate,
            first,
            second,
            ..
        }] if coordinate == &[0]
            && first.invocation() == [0, 0, 0]
            && second.invocation() == [1, 0, 0]
    ));
    let diagnostic = report.findings()[0].to_string();
    assert!(diagnostic.contains("error[FE2O3-OWN-005]"));
    assert!(diagnostic.contains("hierarchy partitions must be disjoint"));
}

#[test]
fn overlap_witness_distinguishes_cross_subgroup_and_cross_workgroup() {
    for (name, launch, workgroup, subgroup, modulus, expected) in [
        (
            "cross_subgroup_overlap",
            8,
            8,
            4,
            4,
            HierarchicalOverlapClassV1::AcrossSubgroups,
        ),
        (
            "cross_workgroup_overlap",
            16,
            8,
            4,
            8,
            HierarchicalOverlapClassV1::AcrossWorkgroups,
        ),
    ] {
        let context = &mut setup();
        let function = static_1d(
            context,
            name,
            launch,
            workgroup,
            subgroup,
            modulus,
            Some(modulus),
            OwnershipPartitionAttr::ExactSets,
        );
        assert!(matches!(
            run_pliron_hierarchical_ownership_check_v1(context, &function).findings(),
            [HierarchicalOwnershipFindingV1::OverlappingOwners { class, .. }]
                if *class == expected
        ));
    }
}

#[test]
fn exact_coverage_reports_first_unowned_coordinate() {
    let context = &mut setup();
    let function = static_1d(
        context,
        "coverage_hole",
        8,
        4,
        2,
        9,
        None,
        OwnershipPartitionAttr::ExactSets,
    );
    let report = run_pliron_hierarchical_ownership_check_v1(context, &function);
    assert!(matches!(
        report.findings(),
        [HierarchicalOwnershipFindingV1::CoverageHole {
            coordinate,
            extents,
            ..
        }] if coordinate == &[8] && extents == &[9]
    ));
    assert!(
        report.findings()[0]
            .to_string()
            .contains("no invocation, subgroup, or workgroup owns that element")
    );
}

#[test]
fn out_of_range_owner_has_exact_hierarchy_witness() {
    let context = &mut setup();
    let function = static_1d(
        context,
        "out_of_range_owner",
        8,
        4,
        2,
        7,
        None,
        OwnershipPartitionAttr::ExactSets,
    );
    let report = run_pliron_hierarchical_ownership_check_v1(context, &function);
    assert!(matches!(
        report.findings(),
        [HierarchicalOwnershipFindingV1::OutOfRange {
            coordinate,
            extents,
            owner,
            ..
        }] if coordinate == &[7]
            && extents == &[7]
            && owner.invocation() == [7, 0, 0]
            && owner.workgroup() == 1
            && owner.subgroup() == 1
            && owner.lane() == 1
    ));
}

#[test]
fn dense_tile_policy_rejects_a_hole_inside_a_subgroup_rectangle() {
    let context = &mut setup();
    let (function, _) = function(context, "non_rectangular_subgroup", 0);
    let entry = function.get_entry_block(context);
    let execution = layout(context, [4, 1, 1], [4, 1, 1], 2);
    let invocation = InvocationIndexOp::new(context, 0, 4);
    let two = IndexConstantOp::new(context, 2);
    let index = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Multiply,
        invocation.result(context),
        two.result(context),
    );
    let output = view(context, vec![7], vec![], MemorySpaceAttr::Global);
    let ownership = contract(
        context,
        output.result(context),
        OwnershipPartitionAttr::DenseRectangles,
    );
    let store = write(context, output.result(context), vec![index.result(context)]);
    let ret = ReturnOp::new(context);
    for operation in [
        execution.get_operation(),
        invocation.get_operation(),
        two.get_operation(),
        index.get_operation(),
        output.get_operation(),
        ownership.get_operation(),
        store.get_operation(),
        ret.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    let report = run_pliron_hierarchical_ownership_check_v1(context, &function);
    assert!(matches!(
        report.findings(),
        [HierarchicalOwnershipFindingV1::NonRectangularRegion {
            region: HierarchicalRegionIdentityV1::Subgroup {
                workgroup: 0,
                subgroup: 0,
            },
            missing,
            ..
        }] if missing == &[1]
    ));
}

#[test]
fn guarded_dynamic_extent_proves_edge_coverage() {
    let context = &mut setup();
    let (function, _) = function(context, "guarded_dynamic_extent", 0);
    let entry = function.get_entry_block(context);
    let body = block(context, &function, "owned");
    let exit = block(context, &function, "exit");
    let execution = layout(context, [8, 1, 1], [4, 1, 1], 2);
    let invocation = InvocationIndexOp::new(context, 0, 8);
    let seven = IndexConstantOp::new(context, 7);
    let output = view(
        context,
        vec![0],
        vec![seven.result(context)],
        MemorySpaceAttr::Global,
    );
    let dimension = DimensionOp::new(context, output.result(context), 0).unwrap();
    let ownership = contract(
        context,
        output.result(context),
        OwnershipPartitionAttr::DenseRectangles,
    );
    let guard = IndexLessThanBranchOp::new(
        context,
        invocation.result(context),
        dimension.result(context),
        body,
        exit,
    );
    let store = write(
        context,
        output.result(context),
        vec![invocation.result(context)],
    );
    let to_exit = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    for operation in [
        execution.get_operation(),
        invocation.get_operation(),
        seven.get_operation(),
        output.get_operation(),
        dimension.get_operation(),
        ownership.get_operation(),
        guard.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    append(context, body, &store);
    append(context, body, &to_exit);
    append(context, exit, &ret);

    let report = run_pliron_hierarchical_ownership_check_v1(context, &function);
    assert!(report.is_clean(), "{:#?}", report.findings());
    let grid = report
        .regions()
        .iter()
        .find(|region| matches!(region.identity(), HierarchicalRegionIdentityV1::Grid(41)))
        .unwrap();
    assert_eq!(grid.element_count(), 7);
}

#[test]
fn exact_effect_domain_accepts_a_guarded_runtime_extent_without_whole_view_coverage() {
    let context = &mut setup();
    let (function, arguments) = function(context, "guarded_runtime_effect_domain", 1);
    let entry = function.get_entry_block(context);
    let body = block(context, &function, "write");
    let exit = block(context, &function, "exit");
    let execution = layout(context, [8, 1, 1], [4, 1, 1], 2);
    let invocation = InvocationIndexOp::new(context, 0, 8);
    let output = view(
        context,
        vec![0],
        vec![arguments[0]],
        MemorySpaceAttr::Global,
    );
    let dimension = DimensionOp::new(context, output.result(context), 0).unwrap();
    let ownership = OwnershipContractOp::new(
        context,
        output.result(context),
        OwnershipCoverageAttr::ExactEffectDomain,
        OwnershipPartitionAttr::ExactSets,
    )
    .unwrap();
    let guard = IndexLessThanBranchOp::new(
        context,
        invocation.result(context),
        dimension.result(context),
        body,
        exit,
    );
    for operation in [
        execution.get_operation(),
        invocation.get_operation(),
        output.get_operation(),
        dimension.get_operation(),
        ownership.get_operation(),
        guard.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    let store = write(
        context,
        output.result(context),
        vec![invocation.result(context)],
    );
    let to_exit = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    append(context, body, &store);
    append(context, body, &to_exit);
    append(context, exit, &ret);

    let report = run_pliron_hierarchical_ownership_check_v1(context, &function);
    assert!(report.is_clean(), "{:#?}", report.findings());
    assert!(report.regions().is_empty());
}

#[test]
fn exact_effect_domain_rejects_missing_guard_collision_and_duplicate_site() {
    let context = &mut setup();
    let (missing_guard, arguments) = function(context, "missing_guard", 1);
    let entry = missing_guard.get_entry_block(context);
    let execution = layout(context, [8, 1, 1], [4, 1, 1], 2);
    let invocation = InvocationIndexOp::new(context, 0, 8);
    let output = view(
        context,
        vec![0],
        vec![arguments[0]],
        MemorySpaceAttr::Global,
    );
    let ownership = OwnershipContractOp::new(
        context,
        output.result(context),
        OwnershipCoverageAttr::ExactEffectDomain,
        OwnershipPartitionAttr::ExactSets,
    )
    .unwrap();
    for operation in [
        execution.get_operation(),
        invocation.get_operation(),
        output.get_operation(),
        ownership.get_operation(),
        write(
            context,
            output.result(context),
            vec![invocation.result(context)],
        )
        .get_operation(),
        ReturnOp::new(context).get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    let report = run_pliron_hierarchical_ownership_check_v1(context, &missing_guard);
    assert!(matches!(
        report.findings(),
        [HierarchicalOwnershipFindingV1::EffectDomainIncomplete { detail }]
            if detail.contains("ranked bounds")
    ));

    let context = &mut setup();
    let (collision, _) = function(context, "effect_collision", 0);
    let entry = collision.get_entry_block(context);
    let execution = layout(context, [2, 1, 1], [2, 1, 1], 2);
    let invocation = InvocationIndexOp::new(context, 0, 2);
    let zero = IndexConstantOp::new(context, 0);
    let output = view(context, vec![1], vec![], MemorySpaceAttr::Global);
    let ownership = OwnershipContractOp::new(
        context,
        output.result(context),
        OwnershipCoverageAttr::ExactEffectDomain,
        OwnershipPartitionAttr::ExactSets,
    )
    .unwrap();
    for operation in [
        execution.get_operation(),
        invocation.get_operation(),
        zero.get_operation(),
        output.get_operation(),
        ownership.get_operation(),
        write(context, output.result(context), vec![zero.result(context)]).get_operation(),
        ReturnOp::new(context).get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    assert!(matches!(
        run_pliron_hierarchical_ownership_check_v1(context, &collision).findings(),
        [HierarchicalOwnershipFindingV1::EffectDomainIncomplete { detail }]
            if detail.contains("FE2O3-RACE-001")
    ));

    let context = &mut setup();
    let (duplicate, _) = function(context, "duplicate_effect_site", 0);
    let entry = duplicate.get_entry_block(context);
    let execution = layout(context, [1, 1, 1], [1, 1, 1], 1);
    let invocation = InvocationIndexOp::new(context, 0, 1);
    let output = view(context, vec![1], vec![], MemorySpaceAttr::Global);
    let ownership = OwnershipContractOp::new(
        context,
        output.result(context),
        OwnershipCoverageAttr::ExactEffectDomain,
        OwnershipPartitionAttr::ExactSets,
    )
    .unwrap();
    for operation in [
        execution.get_operation(),
        invocation.get_operation(),
        output.get_operation(),
        ownership.get_operation(),
        write(
            context,
            output.result(context),
            vec![invocation.result(context)],
        )
        .get_operation(),
        write(
            context,
            output.result(context),
            vec![invocation.result(context)],
        )
        .get_operation(),
        ReturnOp::new(context).get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    assert!(matches!(
        run_pliron_hierarchical_ownership_check_v1(context, &duplicate).findings(),
        [HierarchicalOwnershipFindingV1::MalformedContract { detail, .. }]
            if detail.contains("exactly one")
    ));
}

#[test]
fn runtime_only_dynamic_extent_is_incomplete_not_fabricated() {
    let context = &mut setup();
    let (function, arguments) = function(context, "runtime_dynamic_extent", 1);
    let entry = function.get_entry_block(context);
    let execution = layout(context, [8, 1, 1], [4, 1, 1], 2);
    let output = view(
        context,
        vec![0],
        vec![arguments[0]],
        MemorySpaceAttr::Global,
    );
    let ownership = contract(
        context,
        output.result(context),
        OwnershipPartitionAttr::ExactSets,
    );
    let ret = ReturnOp::new(context);
    append(context, entry, &execution);
    append(context, entry, &output);
    append(context, entry, &ownership);
    append(context, entry, &ret);

    let report = run_pliron_hierarchical_ownership_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert!(matches!(
        report.findings(),
        [HierarchicalOwnershipFindingV1::DynamicExtentIncomplete { dimension: 0, .. }]
    ));
}

#[test]
fn unresolved_index_is_incomplete_with_invocation_and_source_location() {
    let context = &mut setup();
    let (function, _) = function(context, "unresolved_coordinate", 0);
    let entry = function.get_entry_block(context);
    let execution = layout(context, [2, 1, 1], [2, 1, 1], 2);
    let output = view(context, vec![2], vec![], MemorySpaceAttr::Global);
    let ownership = contract(
        context,
        output.result(context),
        OwnershipPartitionAttr::ExactSets,
    );
    let unknown = IndexUnknownOp::new(context);
    let store = write(
        context,
        output.result(context),
        vec![unknown.result(context)],
    );
    let ret = ReturnOp::new(context);
    for operation in [
        execution.get_operation(),
        output.get_operation(),
        ownership.get_operation(),
        unknown.get_operation(),
        store.get_operation(),
        ret.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    let report = run_pliron_hierarchical_ownership_check_v1(context, &function);
    assert!(matches!(
        report.findings(),
        [HierarchicalOwnershipFindingV1::UnresolvedCoordinate {
            invocation,
            dimension: 0,
            ..
        }] if invocation == &[0, 0, 0]
    ));
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
}

#[test]
fn local_contract_verifier_rejects_readonly_and_non_global_views() {
    let context = &mut setup();
    let readonly_type = RankedViewType::new(context, 32, false, vec![1]).unwrap();
    let readonly = RankedViewOp::new(context, readonly_type, vec![]).unwrap();
    assert!(matches!(
        OwnershipContractOp::new(
            context,
            readonly.result(context),
            OwnershipCoverageAttr::ExactView,
            OwnershipPartitionAttr::ExactSets,
        ),
        Err(RankedMemoryError::WriteThroughReadOnlyView)
    ));

    let workgroup = view(context, vec![1], vec![], MemorySpaceAttr::Workgroup);
    assert!(matches!(
        OwnershipContractOp::new(
            context,
            workgroup.result(context),
            OwnershipCoverageAttr::ExactView,
            OwnershipPartitionAttr::ExactSets,
        ),
        Err(RankedMemoryError::MalformedPayload(
            "kernel.ownership_contract requires a global ranked view"
        ))
    ));
}

#[test]
fn production_pipeline_runs_ownership_after_race_and_fails_closed_on_holes() {
    assert_eq!(
        PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2,
        [
            KernelCheckPassKindV1::TensorLayout,
            KernelCheckPassKindV1::MemoryBounds,
            KernelCheckPassKindV1::AtomicLegality,
            KernelCheckPassKindV1::RaceFreedom,
            KernelCheckPassKindV1::HierarchicalOwnership,
            KernelCheckPassKindV1::BarrierConvergence,
            KernelCheckPassKindV1::WorkgroupMemory,
            KernelCheckPassKindV1::SemanticRefinement,
        ]
    );
    let context = &mut setup();
    let function = static_1d(
        context,
        "pipeline_coverage_hole",
        8,
        4,
        2,
        9,
        None,
        OwnershipPartitionAttr::ExactSets,
    );
    let error = require_production_pliron_checks_before_lowering_v2(context, &function)
        .expect_err("production lowering must reject incomplete output ownership");
    assert!(matches!(
        error,
        ProductionPlironPreloweringErrorV2::Ownership(_)
    ));
    assert!(error.to_string().contains("error[FE2O3-OWN-006]"));
}

#[test]
fn absent_contract_is_a_clean_opt_out_without_running_hierarchy_analysis() {
    let context = &mut setup();
    let (function, _) = function(context, "no_ownership_contract", 0);
    let ret = ReturnOp::new(context);
    append(context, function.get_entry_block(context), &ret);
    let report = run_pliron_hierarchical_ownership_check_v1(context, &function);
    assert!(report.is_clean());
    assert!(report.regions().is_empty());
}

#[test]
fn duplicate_and_conditional_contract_metadata_fail_closed() {
    let context = &mut setup();
    let (duplicate, _) = function(context, "duplicate_contract", 0);
    let entry = duplicate.get_entry_block(context);
    let output = view(context, vec![1], vec![], MemorySpaceAttr::Global);
    let first = contract(
        context,
        output.result(context),
        OwnershipPartitionAttr::ExactSets,
    );
    let second = contract(
        context,
        output.result(context),
        OwnershipPartitionAttr::DenseRectangles,
    );
    let ret = ReturnOp::new(context);
    append(context, entry, &output);
    append(context, entry, &first);
    append(context, entry, &second);
    append(context, entry, &ret);
    assert!(matches!(
        run_pliron_hierarchical_ownership_check_v1(context, &duplicate).findings(),
        [HierarchicalOwnershipFindingV1::DuplicateContract { .. }]
    ));

    let context = &mut setup();
    let (conditional, _) = function(context, "conditional_contract", 0);
    let entry = conditional.get_entry_block(context);
    let metadata = block(context, &conditional, "metadata");
    let output = view(context, vec![1], vec![], MemorySpaceAttr::Global);
    let ownership = contract(
        context,
        output.result(context),
        OwnershipPartitionAttr::ExactSets,
    );
    let branch = BranchOp::new(context, metadata);
    let ret = ReturnOp::new(context);
    append(context, entry, &output);
    append(context, entry, &branch);
    append(context, metadata, &ownership);
    append(context, metadata, &ret);
    assert!(matches!(
        run_pliron_hierarchical_ownership_check_v1(context, &conditional).findings(),
        [HierarchicalOwnershipFindingV1::ContractOutsideEntry { .. }]
    ));
}

#[test]
fn hierarchy_contract_without_execution_layout_is_incomplete() {
    let context = &mut setup();
    let (function, _) = function(context, "missing_hierarchy_layout", 0);
    let entry = function.get_entry_block(context);
    let output = view(context, vec![1], vec![], MemorySpaceAttr::Global);
    let ownership = contract(
        context,
        output.result(context),
        OwnershipPartitionAttr::ExactSets,
    );
    let ret = ReturnOp::new(context);
    append(context, entry, &output);
    append(context, entry, &ownership);
    append(context, entry, &ret);
    let report = run_pliron_hierarchical_ownership_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert!(matches!(
        report.findings(),
        [HierarchicalOwnershipFindingV1::ExecutionLayoutIncomplete { .. }]
    ));
}
