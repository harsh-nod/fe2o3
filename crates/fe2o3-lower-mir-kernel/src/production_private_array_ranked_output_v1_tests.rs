use super::*;
use fe2o3_pliron::{
    ProductionConstructionV1, ProductionRankedBlockV1, ProductionRankedKernelV1,
    ProductionRankedTerminatorV1, ProductionSessionLimitsV1, compile_ranked_kernel_for_lowering_v1,
};

const NAME: &str = "private_array_relation";
const ACCESS: (u32, u32) = (0, 3);
const VIEW: (u32, u32) = (0, 1);
const INDEX: (u32, u32) = (0, 2);

fn candidate_operations(
    view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
) -> Vec<ProductionRankedOperationV1> {
    let layout = view.source().source_launch().roots()[0].layout();
    let origin = (1u64 << 63) + 2;
    vec![
        ProductionRankedOperationV1::ExecutionLayout {
            grid_identity: layout.grid_identity(),
            global_extents: layout.global_extents(),
            workgroup_extents: layout.workgroup_extents(),
            subgroup_size: layout.subgroup_size(),
            full_physical_workgroups: layout.full_physical_workgroups(),
        },
        ProductionRankedOperationV1::ViewInSpace {
            result: ProductionRankedValueIdV1::new(0),
            element_width: 32,
            writable: true,
            shape: vec![8],
            dynamic_extents: vec![],
            memory_space: dialect_kernel::MemorySpaceAttr::Private,
            allocation_origin: origin,
            noalias_class: origin,
        },
        ProductionRankedOperationV1::IndexConstant {
            result: ProductionRankedValueIdV1::new(1),
            value: 0,
        },
        ProductionRankedOperationV1::Access {
            kind: dialect_kernel::AccessKindAttr::Write,
            view: ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0)),
            indices: vec![ProductionRankedValueV1::Local(
                ProductionRankedValueIdV1::new(1),
            )],
        },
    ]
}

fn candidate_blocks(operations: Vec<ProductionRankedOperationV1>) -> Vec<ProductionRankedBlockV1> {
    vec![ProductionRankedBlockV1::new(
        operations,
        ProductionRankedTerminatorV1::Return,
    )]
}

fn check(
    view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
    blocks: &[ProductionRankedBlockV1],
    site: Site,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    view.check_ranked_private_array_write_allocation_index(
        ARRAY_ROOT,
        ARRAY_ROOT,
        site,
        Role::Destination,
        NAME,
        blocks,
        ACCESS,
        VIEW,
        INDEX,
        budget,
    )
}

#[test]
fn genuine_sparse_checked_output_matches_ranked_allocation_index_leaf() {
    with_output(
        array_owner(ArrayCase::Write { sparse: true }),
        |view, budget| {
            view.source().semantic_ssa().verify_replay().unwrap();
            assert_eq!(operations(view.source().executable()), 7);
            assert_eq!(operations(view.output()), 6);
            assert_eq!(
                view.source().executable().module().functions[0]
                    .body
                    .as_ref()
                    .unwrap()
                    .blocks
                    .len(),
                2
            );
            assert_eq!(
                view.output().module().functions[0]
                    .body
                    .as_ref()
                    .unwrap()
                    .blocks
                    .len(),
                1
            );
            // The source and fixed optimizer are genuine. This ranked candidate is
            // built by the ordinary structured API, not the codegen root projector.
            let kernel = ProductionRankedKernelV1::new(
                NAME,
                0,
                candidate_blocks(candidate_operations(view)),
            )
            .unwrap();
            let lowering = compile_ranked_kernel_for_lowering_v1(
                ProductionConstructionV1::ranked_kernel(NAME, kernel).unwrap(),
                ProductionSessionLimitsV1::default(),
            )
            .unwrap();
            assert!(lowering.all_mandatory_reports_are_clean());
            assert!(matches!(
                view.private_array_write(
                    ARRAY_ROOT,
                    ARRAY_ROOT,
                    site(2, 1),
                    Role::Destination,
                    budget
                )
                .unwrap(),
                ProductionSourceOutputPrivateArrayAccessV1::Retained {
                    index: 0,
                    executable: true,
                    ..
                }
            ));
            let floor = budget.storage();
            check(view, lowering.kernel().blocks(), site(2, 1), budget).unwrap();
            assert_eq!(budget.storage(), floor);
            assert!(!view.grants_authority());
        },
    );
}

#[test]
fn checked_output_array_leaf_rejects_ranked_allocation_index_and_coordinate_substitutions() {
    with_output(
        array_owner(ArrayCase::Write { sparse: true }),
        |view, budget| {
            for variant in 0..12 {
                let mut operations = candidate_operations(view);
                match variant {
                    0 => {
                        if let ProductionRankedOperationV1::IndexConstant { value, .. } =
                            &mut operations[2]
                        {
                            *value = 7;
                        }
                    }
                    1 => {
                        if let ProductionRankedOperationV1::ViewInSpace { shape, .. } =
                            &mut operations[1]
                        {
                            shape[0] = 9;
                        }
                    }
                    2 => {
                        if let ProductionRankedOperationV1::ViewInSpace { element_width, .. } =
                            &mut operations[1]
                        {
                            *element_width = 64;
                        }
                    }
                    3 => {
                        if let ProductionRankedOperationV1::ViewInSpace {
                            allocation_origin, ..
                        } = &mut operations[1]
                        {
                            *allocation_origin += 1;
                        }
                    }
                    4 => {
                        if let ProductionRankedOperationV1::ViewInSpace { noalias_class, .. } =
                            &mut operations[1]
                        {
                            *noalias_class += 1;
                        }
                    }
                    5 => {
                        if let ProductionRankedOperationV1::ViewInSpace { writable, .. } =
                            &mut operations[1]
                        {
                            *writable = false;
                        }
                    }
                    6 => {
                        if let ProductionRankedOperationV1::ViewInSpace { memory_space, .. } =
                            &mut operations[1]
                        {
                            *memory_space = dialect_kernel::MemorySpaceAttr::Global;
                        }
                    }
                    7 => {
                        if let ProductionRankedOperationV1::Access { kind, .. } = &mut operations[3]
                        {
                            *kind = dialect_kernel::AccessKindAttr::Read;
                        }
                    }
                    8 => {
                        if let ProductionRankedOperationV1::Access { indices, .. } =
                            &mut operations[3]
                        {
                            indices.push(indices[0]);
                        }
                    }
                    9 => {
                        if let ProductionRankedOperationV1::ViewInSpace { result, .. } =
                            &mut operations[1]
                        {
                            *result = ProductionRankedValueIdV1::new(9);
                        }
                    }
                    10 => {
                        if let ProductionRankedOperationV1::IndexConstant { result, .. } =
                            &mut operations[2]
                        {
                            *result = ProductionRankedValueIdV1::new(9);
                        }
                    }
                    11 => {
                        if let ProductionRankedOperationV1::ViewInSpace {
                            dynamic_extents, ..
                        } = &mut operations[1]
                        {
                            dynamic_extents.push(ProductionRankedValueV1::Local(
                                ProductionRankedValueIdV1::new(1),
                            ));
                        }
                    }
                    _ => unreachable!(),
                }
                // Hostile ranked descriptors are intentionally not presented as
                // a mandatory-checked ranked owner or as a changed checked O owner.
                let blocks = candidate_blocks(operations);
                assert!(
                    check(view, &blocks, site(2, 1), budget).is_err(),
                    "variant {variant}"
                );
            }
            let blocks = candidate_blocks(candidate_operations(view));
            for (name, access, view_coordinate, index) in [
                ("foreign", ACCESS, VIEW, INDEX),
                (NAME, (0, 99), VIEW, INDEX),
                (NAME, ACCESS, INDEX, VIEW),
            ] {
                assert!(
                    view.check_ranked_private_array_write_allocation_index(
                        ARRAY_ROOT,
                        ARRAY_ROOT,
                        site(2, 1),
                        Role::Destination,
                        name,
                        &blocks,
                        access,
                        view_coordinate,
                        index,
                        budget,
                    )
                    .is_err()
                );
            }
            assert!(
                view.check_ranked_private_array_write_allocation_index(
                    ARRAY_ROOT,
                    ARRAY_ROOT,
                    site(2, 1),
                    Role::RvalueOperand(0),
                    NAME,
                    &blocks,
                    ACCESS,
                    VIEW,
                    INDEX,
                    budget,
                )
                .is_err()
            );
        },
    );
}

#[test]
fn checked_output_array_leaf_does_not_turn_omission_or_unretained_access_into_a_write() {
    with_output(omitted_write_owner(), |view, budget| {
        let blocks = candidate_blocks(candidate_operations(view));
        assert!(matches!(
            check(view, &blocks, site(1, 1), budget),
            Err(ProductionSourceOutputErrorV1::Invalid(
                "array allocation/index leaf requires an executable retained write"
            ))
        ));
    });
    with_output(
        array_owner(ArrayCase::ValueRead { local_index: false }),
        |view, budget| {
            let blocks = candidate_blocks(candidate_operations(view));
            assert!(
                view.check_ranked_private_array_write_allocation_index(
                    ARRAY_ROOT,
                    ARRAY_ROOT,
                    site(0, 2),
                    Role::RvalueOperand(0),
                    NAME,
                    &blocks,
                    ACCESS,
                    VIEW,
                    INDEX,
                    budget,
                )
                .is_err()
            );
        },
    );
}

#[test]
fn checked_output_array_leaf_has_source_derived_exact_work_and_no_new_payload() {
    with_output(
        array_owner(ArrayCase::Write { sparse: false }),
        |view, budget| {
            let blocks = candidate_blocks(candidate_operations(view));
            // Existing C query406 + fixed leaf45 + exact source name bytes22.
            const WORK: usize = 406 + 45 + NAME.len();
            assert_eq!(WORK, 473);
            let floor = budget.storage();
            for allowance in [WORK, WORK - 1] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(7 + allowance);
                {
                    let mut bounded = AssertOriginBudgetV1::new(&mut work, floor);
                    bounded.charge_work(7).unwrap();
                    bounded.reserve_storage(floor).unwrap();
                    let result = check(view, &blocks, site(0, 1), &mut bounded);
                    if allowance == WORK {
                        result.unwrap();
                        assert_eq!(bounded.work(), 7 + WORK);
                    } else {
                        assert!(
                            matches!(result, Err(ProductionSourceOutputErrorV1::PrivateArray(
                        SemanticKirPrivateArrayQueryErrorV1::Resource(AssertOriginResourceV1::Work(error))
                    )) if error.actual() == 7 + WORK && error.limit() == 7 + WORK - 1)
                        );
                        assert_eq!(bounded.work(), 7 + WORK - 1);
                        assert!(bounded.charge_work(1).is_err());
                    }
                    assert_eq!(bounded.storage(), floor);
                    assert_eq!(bounded.peak_storage(), floor);
                }
                assert_eq!(
                    work.failed_work(),
                    (allowance == WORK - 1).then_some(7 + WORK)
                );
            }
        },
    );
}
