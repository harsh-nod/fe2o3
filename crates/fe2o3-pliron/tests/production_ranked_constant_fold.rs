use dialect_kernel::{AccessKindAttr, IndexBinaryKindAttr};
use fe2o3_kernel_analysis::{KernelCheckStatusV1, PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2};
use fe2o3_pliron::{
    ProductionConstructionV1, ProductionRankedBlockV1, ProductionRankedKernelV1,
    ProductionRankedOperationV1, ProductionRankedTerminatorV1, ProductionRankedValueIdV1,
    ProductionRankedValueV1, ProductionSessionLimitsV1, compile_ranked_kernel_for_lowering_v1,
};

fn local(identity: u32) -> ProductionRankedValueV1 {
    ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(identity))
}

fn folded_access_kernel(
    name: &str,
    kind: IndexBinaryKindAttr,
    lhs: u64,
    rhs: u64,
    extent: u64,
) -> ProductionRankedKernelV1 {
    ProductionRankedKernelV1::new(
        name,
        0,
        vec![ProductionRankedBlockV1::new(
            vec![
                ProductionRankedOperationV1::ExecutionLayout {
                    grid_identity: 1,
                    global_extents: [1, 1, 1],
                    workgroup_extents: [1, 1, 1],
                    subgroup_size: 1,
                    full_physical_workgroups: true,
                },
                ProductionRankedOperationV1::View {
                    result: ProductionRankedValueIdV1::new(0),
                    element_width: 32,
                    writable: false,
                    shape: vec![extent],
                    dynamic_extents: vec![],
                    allocation_origin: 1,
                    noalias_class: 1,
                },
                ProductionRankedOperationV1::IndexConstant {
                    result: ProductionRankedValueIdV1::new(1),
                    value: lhs,
                },
                ProductionRankedOperationV1::IndexConstant {
                    result: ProductionRankedValueIdV1::new(2),
                    value: rhs,
                },
                ProductionRankedOperationV1::IndexBinary {
                    result: ProductionRankedValueIdV1::new(3),
                    kind,
                    lhs: local(1),
                    rhs: local(2),
                },
                ProductionRankedOperationV1::Access {
                    kind: AccessKindAttr::Read,
                    view: local(0),
                    indices: vec![local(3)],
                },
            ],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .expect("valid ranked recipe")
}

fn folded_value(kernel: &ProductionRankedKernelV1) -> Option<u64> {
    match &kernel.blocks()[0].operations()[4] {
        ProductionRankedOperationV1::IndexConstant { result, value }
            if *result == ProductionRankedValueIdV1::new(3) =>
        {
            Some(*value)
        }
        _ => None,
    }
}

#[test]
fn public_constructor_normalizes_all_checked_index_binary_kinds() {
    for (kind, lhs, rhs, expected) in [
        (IndexBinaryKindAttr::Add, 4, 3, 7),
        (IndexBinaryKindAttr::Multiply, 4, 3, 12),
        (IndexBinaryKindAttr::Divide, 13, 3, 4),
        (IndexBinaryKindAttr::Remainder, 13, 3, 1),
    ] {
        let kernel = folded_access_kernel("checked_index_fold", kind, lhs, rhs, 32);
        assert_eq!(folded_value(&kernel), Some(expected));
    }
}

#[test]
fn fixed_eight_stage_pipeline_observes_only_the_normalized_recipe() {
    let kernel = folded_access_kernel(
        "normalized_pipeline",
        IndexBinaryKindAttr::Multiply,
        3,
        4,
        16,
    );
    assert_eq!(folded_value(&kernel), Some(12));
    let construction =
        ProductionConstructionV1::ranked_kernel("normalized_root", kernel).expect("construction");
    let lowering =
        compile_ranked_kernel_for_lowering_v1(construction, ProductionSessionLimitsV1::default())
            .expect("normalized kernel reaches lowering");

    assert_eq!(folded_value(lowering.kernel()), Some(12));
    assert_eq!(
        lowering.production_pipeline_report().pass_order(),
        &PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2
    );
    assert_eq!(
        lowering.production_pipeline_report().status(),
        KernelCheckStatusV1::Clean
    );
    assert_eq!(
        lowering.pass_preservation_report().certificates().len(),
        PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2.len()
    );
    assert!(lowering.pass_preservation_report().is_exact_identity());
    assert!(lowering.all_mandatory_reports_are_clean());
}

#[test]
fn overflow_and_zero_divisors_are_not_silently_normalized() {
    for (kind, lhs, rhs) in [
        (IndexBinaryKindAttr::Add, u64::MAX, 1),
        (IndexBinaryKindAttr::Multiply, u64::MAX, 2),
        (IndexBinaryKindAttr::Divide, 1, 0),
        (IndexBinaryKindAttr::Remainder, 1, 0),
    ] {
        let kernel = folded_access_kernel("undefined_index", kind, lhs, rhs, 1);
        assert!(matches!(
            kernel.blocks()[0].operations()[4],
            ProductionRankedOperationV1::IndexBinary { .. }
        ));
        let construction = ProductionConstructionV1::ranked_kernel("undefined_root", kernel)
            .expect("construction");
        let error = compile_ranked_kernel_for_lowering_v1(
            construction,
            ProductionSessionLimitsV1::default(),
        )
        .expect_err("undefined index arithmetic must fail before lowering");
        assert!(
            error.to_string().contains("overflow")
                || error.to_string().contains("division")
                || error.to_string().contains("remainder")
                || error.to_string().contains("bounds"),
            "unexpected fail-closed diagnostic: {error}"
        );
    }
}

#[test]
fn nonconstant_operands_remain_available_to_dynamic_analysis() {
    let kernel = ProductionRankedKernelV1::new(
        "dynamic_index",
        1,
        vec![ProductionRankedBlockV1::new(
            vec![
                ProductionRankedOperationV1::IndexConstant {
                    result: ProductionRankedValueIdV1::new(0),
                    value: 4,
                },
                ProductionRankedOperationV1::IndexBinary {
                    result: ProductionRankedValueIdV1::new(1),
                    kind: IndexBinaryKindAttr::Add,
                    lhs: ProductionRankedValueV1::Argument(0),
                    rhs: local(0),
                },
            ],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .expect("dynamic recipe remains valid");
    assert!(matches!(
        kernel.blocks()[0].operations()[1],
        ProductionRankedOperationV1::IndexBinary { .. }
    ));
}
