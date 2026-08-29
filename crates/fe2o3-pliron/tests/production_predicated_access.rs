use dialect_kernel::{AccessKindAttr, MemorySpaceAttr};
use fe2o3_pliron::{
    ProductionConstructionV1, ProductionRankedBlockV1, ProductionRankedKernelErrorV1,
    ProductionRankedKernelV1, ProductionRankedOperationV1, ProductionRankedTerminatorV1,
    ProductionRankedValueIdV1, ProductionRankedValueV1, ProductionSessionLimitsV1,
    compile_ranked_kernel_for_lowering_v1,
};

const VIEW: ProductionRankedValueIdV1 = ProductionRankedValueIdV1::new(0);
const INVOCATION: ProductionRankedValueIdV1 = ProductionRankedValueIdV1::new(1);
const COMPONENT: ProductionRankedValueIdV1 = ProductionRankedValueIdV1::new(2);
const INDEX: ProductionRankedValueIdV1 = ProductionRankedValueIdV1::new(3);
const SUCCESS: ProductionRankedValueIdV1 = ProductionRankedValueIdV1::new(4);

fn local(value: ProductionRankedValueIdV1) -> ProductionRankedValueV1 {
    ProductionRankedValueV1::Local(value)
}

fn operations() -> Vec<ProductionRankedOperationV1> {
    operations_with_launch(0)
}

fn operations_with_launch(launch_extent: u64) -> Vec<ProductionRankedOperationV1> {
    vec![
        ProductionRankedOperationV1::ExecutionLayout {
            grid_identity: 1,
            global_extents: [launch_extent, 1, 1],
            workgroup_extents: [64, 1, 1],
            subgroup_size: 64,
            full_physical_workgroups: false,
        },
        ProductionRankedOperationV1::ViewInSpace {
            result: VIEW,
            element_width: 32,
            writable: true,
            shape: vec![0],
            dynamic_extents: vec![ProductionRankedValueV1::Argument(0)],
            memory_space: MemorySpaceAttr::Global,
            allocation_origin: 1,
            noalias_class: 1,
        },
        ProductionRankedOperationV1::InvocationIndex {
            result: INVOCATION,
            dimension: 0,
            launch_extent,
        },
        ProductionRankedOperationV1::IndexConstant {
            result: COMPONENT,
            value: 0,
        },
        ProductionRankedOperationV1::PredicatedCheckedTiledIndex2D {
            result: INDEX,
            success: SUCCESS,
            invocation: local(INVOCATION),
            component: local(COMPONENT),
            rows: ProductionRankedValueV1::Argument(1),
            columns: ProductionRankedValueV1::Argument(2),
            row_stride: ProductionRankedValueV1::Argument(3),
            physical_extent: ProductionRankedValueV1::Argument(0),
            lanes_per_tile: 64,
            tile_rows: 16,
            tile_columns: 16,
            elements_per_lane: 4,
        },
        ProductionRankedOperationV1::PredicatedAccess {
            kind: AccessKindAttr::Write,
            view: local(VIEW),
            index: local(INDEX),
            success: local(SUCCESS),
        },
    ]
}

fn row_operations() -> Vec<ProductionRankedOperationV1> {
    row_operations_with_launch(0)
}

fn row_operations_with_launch(launch_extent: u64) -> Vec<ProductionRankedOperationV1> {
    let mut operations = operations_with_launch(launch_extent);
    operations[4] = ProductionRankedOperationV1::PredicatedCheckedRowStripedIndex2D {
        result: INDEX,
        success: SUCCESS,
        invocation: local(INVOCATION),
        component: local(COMPONENT),
        rows: ProductionRankedValueV1::Argument(1),
        columns: ProductionRankedValueV1::Argument(2),
        row_stride: ProductionRankedValueV1::Argument(3),
        physical_extent: ProductionRankedValueV1::Argument(0),
        lanes_per_row: 64,
        elements_per_lane: 4,
    };
    operations
}

fn kernel_with_operations(
    operations: Vec<ProductionRankedOperationV1>,
) -> Result<ProductionRankedKernelV1, ProductionRankedKernelErrorV1> {
    ProductionRankedKernelV1::new(
        "predicated_access",
        4,
        vec![ProductionRankedBlockV1::new(
            operations,
            ProductionRankedTerminatorV1::Return,
        )],
    )
}

fn guarded_kernel(mut entry: Vec<ProductionRankedOperationV1>) -> ProductionRankedKernelV1 {
    let access = entry.pop().unwrap();
    ProductionRankedKernelV1::new(
        "predicated_access",
        4,
        vec![
            ProductionRankedBlockV1::new(
                entry,
                ProductionRankedTerminatorV1::IndexLessThan {
                    lhs: local(INDEX),
                    rhs: ProductionRankedValueV1::Argument(0),
                    true_block: 1,
                    false_block: 2,
                },
            ),
            ProductionRankedBlockV1::new(vec![access], ProductionRankedTerminatorV1::Return),
            ProductionRankedBlockV1::new(vec![], ProductionRankedTerminatorV1::Return),
        ],
    )
    .expect("structurally guarded recipe")
}

#[test]
fn public_predicated_recipe_materializes_and_proves_dynamic_race_freedom() {
    for operations in [operations(), row_operations()] {
        let kernel = guarded_kernel(operations);
        let construction = ProductionConstructionV1::ranked_kernel("predicated_access", kernel)
            .expect("valid construction name");
        let lowering = compile_ranked_kernel_for_lowering_v1(
            construction,
            ProductionSessionLimitsV1::default(),
        )
        .expect("the verified checked mapping is injective");
        assert!(lowering.race_report().is_clean());
        assert!(
            !lowering
                .race_report()
                .grants_compiler_refinement_authority()
        );
        assert!(!lowering.race_report().grants_artifact_or_launch_authority());
    }
}

#[test]
fn static_public_recipe_uses_the_same_checked_race_proof() {
    for operations in [operations_with_launch(64), row_operations_with_launch(64)] {
        let kernel = guarded_kernel(operations);
        let construction = ProductionConstructionV1::ranked_kernel("predicated_access", kernel)
            .expect("valid construction name");
        let lowering = compile_ranked_kernel_for_lowering_v1(
            construction,
            ProductionSessionLimitsV1::default(),
        )
        .expect("the verified checked mapping is injective");
        assert!(lowering.race_report().is_clean());
    }
}

#[test]
fn predicated_recipe_rejects_changed_pair_extent_rank_and_use_bijection() {
    let mut changed_success = operations();
    let ProductionRankedOperationV1::PredicatedAccess { success, .. } =
        changed_success.last_mut().unwrap()
    else {
        unreachable!()
    };
    *success = local(INDEX);
    assert_eq!(
        kernel_with_operations(changed_success),
        Err(ProductionRankedKernelErrorV1::InvalidShape)
    );

    let mut changed_index = operations();
    let ProductionRankedOperationV1::PredicatedAccess { index, .. } =
        changed_index.last_mut().unwrap()
    else {
        unreachable!()
    };
    *index = local(COMPONENT);
    assert_eq!(
        kernel_with_operations(changed_index),
        Err(ProductionRankedKernelErrorV1::InvalidShape)
    );

    let mut changed_extent = operations();
    let ProductionRankedOperationV1::PredicatedCheckedTiledIndex2D {
        physical_extent, ..
    } = &mut changed_extent[4]
    else {
        unreachable!()
    };
    *physical_extent = ProductionRankedValueV1::Argument(1);
    assert_eq!(
        kernel_with_operations(changed_extent),
        Err(ProductionRankedKernelErrorV1::InvalidShape)
    );

    let mut changed_rank = operations();
    let ProductionRankedOperationV1::ViewInSpace {
        shape,
        dynamic_extents,
        ..
    } = &mut changed_rank[1]
    else {
        unreachable!()
    };
    *shape = vec![0, 0];
    *dynamic_extents = vec![
        ProductionRankedValueV1::Argument(0),
        ProductionRankedValueV1::Argument(0),
    ];
    assert_eq!(
        kernel_with_operations(changed_rank),
        Err(ProductionRankedKernelErrorV1::AccessRankMismatch {
            expected: 2,
            actual: 1,
        })
    );

    let mut missing_use = operations();
    missing_use.pop();
    assert_eq!(
        kernel_with_operations(missing_use),
        Err(ProductionRankedKernelErrorV1::InvalidPredicatedAccessUse {
            success: SUCCESS,
            uses: 0,
        })
    );

    let mut reused = operations();
    reused.push(reused.last().unwrap().clone());
    assert!(kernel_with_operations(reused).is_ok());

    let mut unpaired_index_use = operations();
    unpaired_index_use.push(ProductionRankedOperationV1::Access {
        kind: AccessKindAttr::Write,
        view: local(VIEW),
        indices: vec![local(INDEX)],
    });
    assert_eq!(
        kernel_with_operations(unpaired_index_use),
        Err(ProductionRankedKernelErrorV1::InvalidPredicatedAccessIndexUse { index: INDEX })
    );
}

#[test]
fn predicated_recipe_validation_is_deterministic_and_read_only() {
    let first = kernel_with_operations(operations()).unwrap();
    let second = kernel_with_operations(operations()).unwrap();
    assert_eq!(first, second);
    assert_eq!(first, first.clone());
}
