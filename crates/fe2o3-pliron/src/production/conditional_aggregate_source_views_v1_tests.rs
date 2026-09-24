use super::*;
use dialect_kernel::{DYNAMIC_EXTENT, MemorySpaceAttr};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

#[test]
fn conditional_source_bounds_require_exact_dynamic_global_x_and_d1_layout() {
    for mutation in 0..6 {
        let index = ProductionRankedValueIdV1::new(0);
        let mut operations = vec![ProductionRankedOperationV1::ExecutionLayout {
            grid_identity: 1,
            global_extents: if mutation == 1 {
                [DYNAMIC_EXTENT, 2, 1]
            } else {
                [DYNAMIC_EXTENT, 1, 1]
            },
            workgroup_extents: [1, 1, 1],
            subgroup_size: 1,
            full_physical_workgroups: true,
        }];
        if mutation != 4 {
            operations.push(ProductionRankedOperationV1::InvocationIndex {
                result: index,
                dimension: if mutation == 2 { 1 } else { 0 },
                launch_extent: if mutation == 3 { 4 } else { DYNAMIC_EXTENT },
            });
        }
        let kernel = ProductionRankedKernelV1::new(
            "domain",
            1,
            vec![ProductionRankedBlockV1::new(
                operations,
                ProductionRankedTerminatorV1::Return,
            )],
        )
        .unwrap();
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        let candidate = if mutation == 5 {
            ProductionRankedValueV1::Argument(0)
        } else {
            ProductionRankedValueV1::Local(index)
        };
        assert_eq!(
            require_global_x(&kernel, candidate, &mut budget).is_ok(),
            mutation == 0
        );
    }
}

#[test]
fn whole_view_join_uses_source_not_canonical_or_adjusted_ordinal() {
    // Exercise the private descriptive join with deliberately distinct ordinals.
    // These test coordinates are not a checked relation or aggregate authority.
    let source = ProductionConditionalSourceCoordinatesV1 {
        parameter: 0,
        value: ValueId(7),
        source: 4,
        adjusted: 2,
        local: SemanticLocalIdV1::from_index(8),
        ty: SemanticTypeIdV1::from_index(9),
    };
    for output in [false, true] {
        for mutation in 0..9 {
            let view = ProductionRankedValueIdV1::new(0);
            let operation = ProductionRankedOperationV1::ViewInSpace {
                result: view,
                element_width: if mutation == 3 { 16 } else { 32 },
                writable: if mutation == 4 { !output } else { output },
                shape: if mutation == 5 {
                    vec![1]
                } else {
                    vec![DYNAMIC_EXTENT]
                },
                dynamic_extents: if mutation == 5 {
                    vec![]
                } else {
                    vec![ProductionRankedValueV1::Argument(1)]
                },
                memory_space: if mutation == 6 {
                    MemorySpaceAttr::Workgroup
                } else {
                    MemorySpaceAttr::Global
                },
                allocation_origin: match mutation {
                    1 => 1,
                    2 => 3,
                    _ => 5,
                },
                noalias_class: 0,
            };
            let kernel = ProductionRankedKernelV1::new(
                "join",
                2,
                vec![ProductionRankedBlockV1::new(
                    vec![operation],
                    ProductionRankedTerminatorV1::Return,
                )],
            )
            .unwrap();
            let mut work = Work::new(if mutation == 8 { 0 } else { usize::MAX });
            let mut budget = Budget::new(&mut work, usize::MAX);
            let result = view_extent(
                &kernel,
                ProductionRankedValueV1::Local(if mutation == 7 {
                    ProductionRankedValueIdV1::new(1)
                } else {
                    view
                }),
                source,
                4,
                output,
                &mut budget,
            );
            if mutation == 0 {
                assert_eq!(result.unwrap(), ProductionRankedValueV1::Argument(1));
            } else {
                assert!(result.is_err(), "mutation {mutation}");
            }
        }
    }
}
