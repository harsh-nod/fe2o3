use std::collections::BTreeSet;

use fe2o3_kernel_ir::*;

fn profile() -> TiledGemmLdsGridV1Profile {
    TiledGemmLdsGridV1Profile::exact_gfx942_xnack_minus_cov6()
}

fn operations(module: &Module) -> &[Operation] {
    &module.functions[0].body.as_ref().unwrap().blocks[0].operations
}

fn operations_mut(module: &mut Module) -> &mut Vec<Operation> {
    &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations
}

fn assert_rejected(module: Module) {
    assert!(
        verify_tiled_gemm_lds_grid_v1_module(&module, &profile()).is_err(),
        "hostile mutation was admitted"
    );
}

fn assert_valid_but_noncanonical(module: Module) {
    verify_module(&module).expect("mutation remains generically valid Kernel IR");
    assert_eq!(
        verify_tiled_gemm_lds_grid_v1_module(&module, &profile()),
        Err(TiledGemmLdsGridV1Error::NonCanonicalKernelIr)
    );
}

fn operation_positions(module: &Module, predicate: impl Fn(&OperationKind) -> bool) -> Vec<usize> {
    operations(module)
        .iter()
        .enumerate()
        .filter_map(|(index, operation)| predicate(&operation.kind).then_some(index))
        .collect()
}

fn constant_result(module: &Module, value: u64) -> ValueId {
    operations(module)
        .iter()
        .find_map(|operation| match operation.kind {
            OperationKind::Constant(Constant::Index(candidate)) if candidate == value => {
                Some(operation.results[0].id)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing canonical index constant {value}"))
}

fn invocation_result(module: &Module, kind: IndexKind, axis: Axis) -> ValueId {
    operations(module)
        .iter()
        .find_map(|operation| match operation.kind {
            OperationKind::Intrinsic(IntrinsicOperation {
                kind:
                    IntrinsicKind::InvocationIndex {
                        kind: candidate_kind,
                        axis: candidate_axis,
                    },
                ..
            }) if candidate_kind == kind && candidate_axis == axis => Some(operation.results[0].id),
            _ => None,
        })
        .expect("missing canonical invocation index")
}

fn slice_base(module: &Module, slice: ValueId) -> ValueId {
    operations(module)
        .iter()
        .find_map(|operation| match operation.kind {
            OperationKind::SliceData { slice: candidate } if candidate == slice => {
                Some(operation.results[0].id)
            }
            _ => None,
        })
        .expect("missing canonical slice base")
}

#[test]
fn exact_grid_helpers_cover_checked_padded_footprints_and_disjoint_c() {
    let mut tile_origins = BTreeSet::new();
    let mut a_indices = BTreeSet::new();
    let mut b_indices = BTreeSet::new();
    let mut c_indices = BTreeSet::new();
    let mut lds_indices = BTreeSet::new();

    for group_y in 0..TILED_GEMM_LDS_GRID_V1_TILE_ROWS {
        for group_x in 0..TILED_GEMM_LDS_GRID_V1_TILE_COLUMNS {
            let origin = tiled_gemm_lds_grid_v1_tile_origin(group_x, group_y).unwrap();
            assert_eq!(
                origin,
                (
                    group_y * TILED_GEMM_LDS_GRID_V1_TILE_EXTENT,
                    group_x * TILED_GEMM_LDS_GRID_V1_TILE_EXTENT,
                )
            );
            assert!(tile_origins.insert(origin));

            let mut workgroup_c = BTreeSet::new();
            for lane in 0..TILED_GEMM_LDS_GRID_V1_LANES {
                for component in 0..TILED_GEMM_LDS_GRID_V1_FRAGMENT_ELEMENTS {
                    let (lane_column, depth) =
                        tiled_gemm_lds_grid_v1_fragment_coordinate(lane, component).unwrap();
                    let a = tiled_gemm_lds_grid_v1_a_index(group_y, lane, component).unwrap();
                    let b = tiled_gemm_lds_grid_v1_b_index(group_x, lane, component).unwrap();
                    let c =
                        tiled_gemm_lds_grid_v1_c_index(group_x, group_y, lane, component).unwrap();
                    let lds = tiled_gemm_lds_grid_v1_lds_index(lane, component).unwrap();

                    assert_eq!(
                        a,
                        (group_y * 16 + lane_column) * TILED_GEMM_LDS_GRID_V1_LDA + depth
                    );
                    assert_eq!(
                        b,
                        depth * TILED_GEMM_LDS_GRID_V1_LDB + group_x * 16 + lane_column
                    );
                    assert_eq!(
                        c,
                        (group_y * 16 + depth) * TILED_GEMM_LDS_GRID_V1_LDC
                            + group_x * 16
                            + lane_column
                    );
                    assert_eq!(lds, lane_column * 16 + (depth ^ ((lane_column & 3) * 4)));
                    assert!(a < TILED_GEMM_LDS_GRID_V1_A_ELEMENTS);
                    assert!(b < TILED_GEMM_LDS_GRID_V1_B_ELEMENTS);
                    assert!(c < TILED_GEMM_LDS_GRID_V1_C_ELEMENTS);
                    assert!((a + 1) * 2 <= TILED_GEMM_LDS_GRID_V1_A_BYTES);
                    assert!((b + 1) * 2 <= TILED_GEMM_LDS_GRID_V1_B_BYTES);
                    assert!((c + 1) * 4 <= TILED_GEMM_LDS_GRID_V1_C_BYTES);
                    assert!(a % TILED_GEMM_LDS_GRID_V1_LDA < TILED_GEMM_LDS_GRID_V1_K);
                    assert!(b % TILED_GEMM_LDS_GRID_V1_LDB < TILED_GEMM_LDS_GRID_V1_N);
                    assert!(c % TILED_GEMM_LDS_GRID_V1_LDC < TILED_GEMM_LDS_GRID_V1_N);
                    assert!(
                        workgroup_c.insert(c),
                        "duplicate C owner inside one workgroup"
                    );
                    assert!(c_indices.insert(c), "duplicate C owner across the grid");

                    if group_x == 0 {
                        assert!(a_indices.insert(a));
                    }
                    if group_y == 0 {
                        assert!(b_indices.insert(b));
                    }
                    if group_x == 0 && group_y == 0 {
                        assert!(lds_indices.insert(lds));
                    }
                }
            }
            assert_eq!(
                workgroup_c.len(),
                TILED_GEMM_LDS_GRID_V1_TILE_ELEMENTS as usize
            );
        }
    }

    assert_eq!(
        tile_origins.len(),
        TILED_GEMM_LDS_GRID_V1_WORKGROUPS as usize
    );
    assert_eq!(lds_indices, (0..256).collect());
    assert_eq!(
        a_indices,
        (0..TILED_GEMM_LDS_GRID_V1_M)
            .flat_map(|row| {
                (0..TILED_GEMM_LDS_GRID_V1_K)
                    .map(move |depth| row * TILED_GEMM_LDS_GRID_V1_LDA + depth)
            })
            .collect()
    );
    assert_eq!(
        b_indices,
        (0..TILED_GEMM_LDS_GRID_V1_K)
            .flat_map(|depth| {
                (0..TILED_GEMM_LDS_GRID_V1_N)
                    .map(move |column| depth * TILED_GEMM_LDS_GRID_V1_LDB + column)
            })
            .collect()
    );
    assert_eq!(
        c_indices,
        (0..TILED_GEMM_LDS_GRID_V1_M)
            .flat_map(|row| {
                (0..TILED_GEMM_LDS_GRID_V1_N)
                    .map(move |column| row * TILED_GEMM_LDS_GRID_V1_LDC + column)
            })
            .collect()
    );
    assert_eq!(
        c_indices.len(),
        (TILED_GEMM_LDS_GRID_V1_M * TILED_GEMM_LDS_GRID_V1_N) as usize
    );
    assert_eq!(
        (*a_indices.last().unwrap() + 1) * 2,
        TILED_GEMM_LDS_GRID_V1_A_BYTES
    );
    assert_eq!(
        (*b_indices.last().unwrap() + 1) * 2,
        TILED_GEMM_LDS_GRID_V1_B_BYTES
    );
    assert_eq!(
        (*c_indices.last().unwrap() + 1) * 4,
        TILED_GEMM_LDS_GRID_V1_C_BYTES
    );
}

#[test]
fn checked_helpers_reject_every_out_of_domain_coordinate() {
    for (group_x, group_y) in [(3, 0), (0, 4), (3, 4), (u32::MAX, u32::MAX)] {
        assert_eq!(tiled_gemm_lds_grid_v1_tile_origin(group_x, group_y), None);
        assert_eq!(tiled_gemm_lds_grid_v1_c_index(group_x, group_y, 0, 0), None);
    }
    for (lane, component) in [(64, 0), (0, 4), (64, 4), (u32::MAX, u32::MAX)] {
        assert_eq!(
            tiled_gemm_lds_grid_v1_fragment_coordinate(lane, component),
            None
        );
        assert_eq!(tiled_gemm_lds_grid_v1_a_index(0, lane, component), None);
        assert_eq!(tiled_gemm_lds_grid_v1_b_index(0, lane, component), None);
        assert_eq!(tiled_gemm_lds_grid_v1_c_index(0, 0, lane, component), None);
        assert_eq!(tiled_gemm_lds_grid_v1_lds_index(lane, component), None);
    }
    assert_eq!(tiled_gemm_lds_grid_v1_a_index(4, 0, 0), None);
    assert_eq!(tiled_gemm_lds_grid_v1_b_index(3, 0, 0), None);
    assert_eq!(tiled_gemm_lds_grid_v1_xor4_index(16, 0), None);
    assert_eq!(tiled_gemm_lds_grid_v1_xor4_index(0, 16), None);
}

#[test]
fn canonical_graph_has_exact_grid_abi_resources_barrier_mfma_and_stores() {
    let module = tiled_gemm_lds_grid_v1_module();
    verify_tiled_gemm_lds_grid_v1_module(&module, &profile()).expect("canonical Slice 3 grid");

    assert_eq!(module.functions.len(), 1);
    assert_eq!(module.kernels.len(), 1);
    assert_eq!(module.id.as_str(), TILED_GEMM_LDS_GRID_V1_MODULE_ID);
    assert_eq!(
        module.functions[0].id.as_str(),
        TILED_GEMM_LDS_GRID_V1_FUNCTION_ID
    );
    assert_eq!(
        module.kernels[0].id.as_str(),
        TILED_GEMM_LDS_GRID_V1_KERNEL_ID
    );
    assert_eq!(
        module.functions[0].signature.parameters,
        [
            Type::slice(
                Type::Scalar(ScalarType::Bf16),
                AddressSpace::Global,
                AccessMode::ReadOnly,
            ),
            Type::slice(
                Type::Scalar(ScalarType::Bf16),
                AddressSpace::Global,
                AccessMode::ReadOnly,
            ),
            Type::slice(Type::F32, AddressSpace::Global, AccessMode::ReadWrite),
        ]
    );
    assert!(
        module.functions[0]
            .signature
            .parameters
            .iter()
            .all(|parameter| !matches!(parameter, Type::Scalar(_))),
        "runtime dimensions and strides are intentionally absent from this exact profile"
    );
    assert_eq!(
        module.kernels[0].domain,
        LaunchDomain::D2 {
            x: LaunchExtent::Static(192),
            y: LaunchExtent::Static(4),
        }
    );
    assert_eq!(
        module.kernels[0].workgroup_size,
        Some(WorkgroupSize::new(64, 1, 1))
    );
    assert!(
        module
            .required_capabilities
            .contains(&gfx942_xnack_minus_target_capability())
    );

    let operations = operations(&module);
    assert_eq!(operations.len(), 84);
    let allocations = operation_positions(&module, |kind| {
        matches!(kind, OperationKind::WorkgroupMemory(_))
    });
    assert_eq!(allocations.len(), 2);
    let allocation_bases = allocations
        .iter()
        .map(|position| operations[*position].results[0].id)
        .collect::<Vec<_>>();
    assert_ne!(allocation_bases[0], allocation_bases[1]);
    for position in allocations {
        let OperationKind::WorkgroupMemory(memory) = &operations[position].kind else {
            unreachable!()
        };
        assert_eq!(memory.element, Type::Scalar(ScalarType::Bf16));
        assert_eq!(memory.extent, WorkgroupMemoryExtent::Static(256));
        assert_eq!(memory.alignment, 16);
    }

    let invocation_indices = operations
        .iter()
        .filter_map(|operation| match operation.kind {
            OperationKind::Intrinsic(IntrinsicOperation {
                kind: IntrinsicKind::InvocationIndex { kind, axis },
                ..
            }) => Some((kind, axis)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        invocation_indices,
        [
            (IndexKind::Local, Axis::X),
            (IndexKind::Workgroup, Axis::X),
            (IndexKind::Workgroup, Axis::Y),
        ]
    );

    let matrices = operation_positions(&module, |kind| matches!(kind, OperationKind::Matrix(_)));
    assert_eq!(matrices.len(), 5);
    let barrier = operation_positions(&module, |kind| {
        matches!(kind, OperationKind::WorkgroupBarrier(_))
    });
    assert_eq!(barrier.len(), 1);
    assert!(matrices[1] < barrier[0] && barrier[0] < matrices[2]);
    let mut matrix_bases = Vec::new();
    for position in matrices {
        let OperationKind::Matrix(matrix) = &operations[position].kind else {
            unreachable!()
        };
        assert_eq!(matrix.active_lanes, 64);
        assert_eq!(
            matrix.convergence,
            Convergence::uniform(SynchronizationScope::Subgroup)
        );
        match matrix.kind {
            MatrixOperationKind::LdsStore { base, profile, .. }
            | MatrixOperationKind::LdsLoad { base, profile } => {
                assert_eq!(
                    profile,
                    MatrixLdsProfile::tile_16x16_xor4_wave64(MatrixElement::Bf16)
                );
                matrix_bases.push(base);
            }
            MatrixOperationKind::MultiplyAccumulate { profile, .. } => {
                assert_eq!(profile, MatrixMultiplyProfile::bf16_f32_m16n16k16_wave64());
            }
            MatrixOperationKind::ScaledMultiplyAccumulate { .. } => {
                unreachable!("the gfx942 BF16 fixture cannot contain a scaled gfx950 MFMA")
            }
        }
    }
    assert_eq!(
        matrix_bases,
        [
            allocation_bases[0],
            allocation_bases[1],
            allocation_bases[0],
            allocation_bases[1],
        ]
    );

    assert_eq!(
        operation_positions(&module, |kind| matches!(kind, OperationKind::Load { .. })).len(),
        8
    );
    assert_eq!(
        operation_positions(&module, |kind| matches!(kind, OperationKind::Store { .. })).len(),
        4
    );
    assert_eq!(
        operation_positions(&module, |kind| matches!(
            kind,
            OperationKind::GetElementPointer { .. }
        ))
        .len(),
        12
    );
    assert!(!operations.iter().any(|operation| matches!(
        operation.kind,
        OperationKind::Compare { .. } | OperationKind::Select { .. }
    )));
}

#[test]
fn every_profile_field_mutation_fails_closed() {
    let mut mutations = Vec::new();
    macro_rules! mutated {
        ($field:ident, $value:expr) => {{
            let mut candidate = profile();
            candidate.$field = $value;
            mutations.push(candidate);
        }};
    }

    mutated!(target, TargetCapability::WaveWidth(WaveWidth::Wave64));
    mutated!(code_object_version, 5);
    mutated!(m, 48);
    mutated!(n, 64);
    mutated!(k, 32);
    mutated!(lda, 32);
    mutated!(ldb, 78);
    mutated!(ldc, 95);
    mutated!(a_elements, 2_094);
    mutated!(b_elements, 1_232);
    mutated!(c_elements, 6_095);
    mutated!(a_bytes, 4_188);
    mutated!(b_bytes, 2_464);
    mutated!(c_bytes, 24_380);
    mutated!(tile_rows, 3);
    mutated!(tile_columns, 4);
    mutated!(depth_tiles, 2);
    mutated!(workgroup_count, 11);
    mutated!(wave_width, WaveWidth::Wave32);
    mutated!(launch_extent_x, 128);
    mutated!(launch_extent_y, 3);
    mutated!(workgroup_size, WorkgroupSize::new(32, 2, 1));
    mutated!(lds_allocations, 1);
    mutated!(lds_elements_per_allocation, 512);
    mutated!(lds_bytes_per_allocation, 1_024);
    mutated!(static_lds_bytes, 512);
    mutated!(lds_alignment, 32);
    mutated!(output_elements_per_lane, 8);

    assert_eq!(mutations.len(), 28);
    for mutation in mutations {
        assert_eq!(
            verify_tiled_gemm_lds_grid_v1_module(&tiled_gemm_lds_grid_v1_module(), &mutation),
            Err(TiledGemmLdsGridV1Error::UnsupportedProfile)
        );
    }
}

#[test]
fn deleting_duplicating_reordering_or_retyping_every_operation_fails_closed() {
    let canonical = tiled_gemm_lds_grid_v1_module();
    let operation_count = operations(&canonical).len();
    for operation_index in 0..operation_count {
        let mut deleted = canonical.clone();
        operations_mut(&mut deleted).remove(operation_index);
        assert_rejected(deleted);

        let mut duplicated = canonical.clone();
        let duplicate = operations(&duplicated)[operation_index].clone();
        operations_mut(&mut duplicated).insert(operation_index, duplicate);
        assert_rejected(duplicated);

        for result_index in 0..operations(&canonical)[operation_index].results.len() {
            let mut identity = canonical.clone();
            operations_mut(&mut identity)[operation_index].results[result_index].id =
                ValueId(u32::MAX);
            assert_rejected(identity);

            let mut ty = canonical.clone();
            let result = &mut operations_mut(&mut ty)[operation_index].results[result_index];
            result.ty = if result.ty == Type::Scalar(ScalarType::U32) {
                Type::Scalar(ScalarType::U64)
            } else {
                Type::Scalar(ScalarType::U32)
            };
            assert_rejected(ty);
        }
    }
    for operation_index in 0..operation_count - 1 {
        let mut reordered = canonical.clone();
        operations_mut(&mut reordered).swap(operation_index, operation_index + 1);
        assert_rejected(reordered);
    }
}

#[test]
fn group_mapping_stride_and_address_arithmetic_mutations_fail_closed() {
    let canonical = tiled_gemm_lds_grid_v1_module();
    let group_x = invocation_result(&canonical, IndexKind::Workgroup, Axis::X);
    let group_y = invocation_result(&canonical, IndexKind::Workgroup, Axis::Y);
    let lda = constant_result(&canonical, u64::from(TILED_GEMM_LDS_GRID_V1_LDA));
    let ldb = constant_result(&canonical, u64::from(TILED_GEMM_LDS_GRID_V1_LDB));
    let ldc = constant_result(&canonical, u64::from(TILED_GEMM_LDS_GRID_V1_LDC));

    for (kind, axis, replacement_kind, replacement_axis) in [
        (IndexKind::Workgroup, Axis::X, IndexKind::Local, Axis::X),
        (IndexKind::Workgroup, Axis::Y, IndexKind::Workgroup, Axis::X),
    ] {
        let mut mutation = canonical.clone();
        let operation = operations_mut(&mut mutation)
            .iter_mut()
            .find(|operation| {
                matches!(
                    operation.kind,
                    OperationKind::Intrinsic(IntrinsicOperation {
                        kind: IntrinsicKind::InvocationIndex {
                            kind: candidate_kind,
                            axis: candidate_axis,
                        },
                        ..
                    }) if candidate_kind == kind && candidate_axis == axis
                )
            })
            .unwrap();
        let OperationKind::Intrinsic(intrinsic) = &mut operation.kind else {
            unreachable!()
        };
        intrinsic.kind = IntrinsicKind::InvocationIndex {
            kind: replacement_kind,
            axis: replacement_axis,
        };
        assert_valid_but_noncanonical(mutation);
    }

    for (group, wrong_group) in [(group_x, group_y), (group_y, group_x)] {
        let mut mutation = canonical.clone();
        let OperationKind::Binary { lhs, .. } = &mut operations_mut(&mut mutation)
            .iter_mut()
            .find(|operation| {
                matches!(
                    operation.kind,
                    OperationKind::Binary {
                        op: BinaryOp::Multiply,
                        lhs,
                        ..
                    } if lhs == group
                )
            })
            .unwrap()
            .kind
        else {
            unreachable!()
        };
        *lhs = wrong_group;
        assert_valid_but_noncanonical(mutation);
    }

    for stride in [33_u64, 79, 96] {
        let mut mutation = canonical.clone();
        let OperationKind::Constant(Constant::Index(value)) = &mut operations_mut(&mut mutation)
            .iter_mut()
            .find(|operation| {
                matches!(operation.kind, OperationKind::Constant(Constant::Index(value)) if value == stride)
            })
            .unwrap()
            .kind
        else {
            unreachable!()
        };
        *value += 1;
        assert_valid_but_noncanonical(mutation);
    }

    for stride_id in [lda, ldb, ldc] {
        let mut mutation = canonical.clone();
        let OperationKind::Binary { op, .. } = &mut operations_mut(&mut mutation)
            .iter_mut()
            .find(|operation| {
                matches!(
                    operation.kind,
                    OperationKind::Binary {
                        op: BinaryOp::Multiply,
                        rhs,
                        ..
                    } if rhs == stride_id
                )
            })
            .unwrap()
            .kind
        else {
            unreachable!()
        };
        *op = BinaryOp::Add;
        assert_valid_but_noncanonical(mutation);
    }

    let a_base = slice_base(&canonical, ValueId(0));
    let b_base = slice_base(&canonical, ValueId(1));
    let mut wrong_a_allocation = canonical.clone();
    let OperationKind::GetElementPointer { base, .. } = &mut operations_mut(&mut wrong_a_allocation)
        .iter_mut()
        .find(|operation| {
            matches!(operation.kind, OperationKind::GetElementPointer { base, .. } if base == a_base)
        })
        .unwrap()
        .kind
    else {
        unreachable!()
    };
    *base = b_base;
    assert_valid_but_noncanonical(wrong_a_allocation);

    let c_base = slice_base(&canonical, ValueId(2));
    let zero = constant_result(&canonical, 0);
    let mut collapsed_c_owner = canonical.clone();
    let OperationKind::GetElementPointer { offset, .. } = &mut operations_mut(&mut collapsed_c_owner)
        .iter_mut()
        .find(|operation| {
            matches!(operation.kind, OperationKind::GetElementPointer { base, .. } if base == c_base)
        })
        .unwrap()
        .kind
    else {
        unreachable!()
    };
    *offset = zero;
    assert_valid_but_noncanonical(collapsed_c_owner);
}

#[test]
fn grid_resource_identity_and_target_mutations_fail_closed() {
    let canonical = tiled_gemm_lds_grid_v1_module();

    let mut module_id = canonical.clone();
    module_id.id = "hostile::grid".into();
    assert_valid_but_noncanonical(module_id);
    let mut function_id = canonical.clone();
    function_id.functions[0].id = "hostile_function".into();
    assert_rejected(function_id);
    let mut kernel_id = canonical.clone();
    kernel_id.kernels[0].id = "hostile_kernel".into();
    assert_valid_but_noncanonical(kernel_id);

    for domain in [
        LaunchDomain::D2 {
            x: LaunchExtent::Static(128),
            y: LaunchExtent::Static(4),
        },
        LaunchDomain::D2 {
            x: LaunchExtent::Static(192),
            y: LaunchExtent::Static(3),
        },
        LaunchDomain::D1 {
            x: LaunchExtent::Static(192),
        },
    ] {
        let mut mutation = canonical.clone();
        mutation.kernels[0].domain = domain;
        assert_rejected(mutation);
    }
    let mut workgroup = canonical.clone();
    workgroup.kernels[0].workgroup_size = Some(WorkgroupSize::new(32, 2, 1));
    assert_valid_but_noncanonical(workgroup);

    let exact_target = gfx942_xnack_minus_target_capability();
    let mut missing_target = canonical.clone();
    missing_target.required_capabilities.remove(&exact_target);
    missing_target.functions[0]
        .required_capabilities
        .remove(&exact_target);
    missing_target.kernels[0]
        .required_capabilities
        .remove(&exact_target);
    assert_valid_but_noncanonical(missing_target);

    let mut unexpected_target_feature = canonical.clone();
    unexpected_target_feature
        .required_capabilities
        .insert(TargetCapability::Float64);
    unexpected_target_feature.functions[0]
        .required_capabilities
        .insert(TargetCapability::Float64);
    unexpected_target_feature.kernels[0]
        .required_capabilities
        .insert(TargetCapability::Float64);
    assert_valid_but_noncanonical(unexpected_target_feature);

    let allocations = operation_positions(&canonical, |kind| {
        matches!(kind, OperationKind::WorkgroupMemory(_))
    });
    for position in allocations {
        for extent in [
            WorkgroupMemoryExtent::Static(255),
            WorkgroupMemoryExtent::Static(257),
            WorkgroupMemoryExtent::Dynamic,
            WorkgroupMemoryExtent::DynamicAtLeast(256),
        ] {
            let mut mutation = canonical.clone();
            let OperationKind::WorkgroupMemory(memory) =
                &mut operations_mut(&mut mutation)[position].kind
            else {
                unreachable!()
            };
            memory.extent = extent;
            assert_rejected(mutation);
        }
        for alignment in [2, 8, 32, 512] {
            let mut mutation = canonical.clone();
            let OperationKind::WorkgroupMemory(memory) =
                &mut operations_mut(&mut mutation)[position].kind
            else {
                unreachable!()
            };
            memory.alignment = alignment;
            assert_rejected(mutation);
        }
    }
}

#[test]
fn barrier_layout_allocation_and_output_owner_mutations_fail_closed() {
    let canonical = tiled_gemm_lds_grid_v1_module();
    let barrier = operation_positions(&canonical, |kind| {
        matches!(kind, OperationKind::WorkgroupBarrier(_))
    })[0];
    let matrices = operation_positions(&canonical, |kind| matches!(kind, OperationKind::Matrix(_)));

    let mut removed_barrier = canonical.clone();
    operations_mut(&mut removed_barrier).remove(barrier);
    assert_valid_but_noncanonical(removed_barrier);

    let mut late_barrier = canonical.clone();
    let barrier_operation = operations_mut(&mut late_barrier).remove(barrier);
    let mfma = *operation_positions(&late_barrier, |kind| {
        matches!(kind, OperationKind::Matrix(_))
    })
    .last()
    .unwrap();
    operations_mut(&mut late_barrier).insert(mfma + 1, barrier_operation);
    assert_valid_but_noncanonical(late_barrier);

    for mutation_index in 0..4 {
        let mut mutation = canonical.clone();
        let OperationKind::WorkgroupBarrier(value) =
            &mut operations_mut(&mut mutation)[barrier].kind
        else {
            unreachable!()
        };
        match mutation_index {
            0 => value.memory_scope = SynchronizationScope::Subgroup,
            1 => value.semantics.ordering = MemoryOrdering::Release,
            2 => value.semantics.address_spaces = BTreeSet::from([AddressSpace::Global]),
            3 => value.convergence = Convergence::uniform(SynchronizationScope::Subgroup),
            _ => unreachable!(),
        }
        assert_rejected(mutation);
    }

    let a_lds = operations(&canonical)
        .iter()
        .find_map(|operation| {
            matches!(operation.kind, OperationKind::WorkgroupMemory(_))
                .then_some(operation.results[0].id)
        })
        .unwrap();
    for position in matrices.iter().take(4).copied() {
        let mut layout = canonical.clone();
        let OperationKind::Matrix(matrix) = &mut operations_mut(&mut layout)[position].kind else {
            unreachable!()
        };
        match &mut matrix.kind {
            MatrixOperationKind::LdsLoad { profile, .. }
            | MatrixOperationKind::LdsStore { profile, .. } => profile.fragment_elements = 8,
            MatrixOperationKind::MultiplyAccumulate { .. }
            | MatrixOperationKind::ScaledMultiplyAccumulate { .. } => unreachable!(),
        }
        assert_rejected(layout);

        let mut alias = canonical.clone();
        let OperationKind::Matrix(matrix) = &mut operations_mut(&mut alias)[position].kind else {
            unreachable!()
        };
        match &mut matrix.kind {
            MatrixOperationKind::LdsLoad { base, .. }
            | MatrixOperationKind::LdsStore { base, .. } => *base = a_lds,
            MatrixOperationKind::MultiplyAccumulate { .. }
            | MatrixOperationKind::ScaledMultiplyAccumulate { .. } => unreachable!(),
        }
        if operations(&alias) != operations(&canonical) {
            assert_valid_but_noncanonical(alias);
        }
    }

    let mfma = *matrices.last().unwrap();
    let mut wrong_mfma = canonical.clone();
    let OperationKind::Matrix(matrix) = &mut operations_mut(&mut wrong_mfma)[mfma].kind else {
        unreachable!()
    };
    let MatrixOperationKind::MultiplyAccumulate { profile, .. } = &mut matrix.kind else {
        unreachable!()
    };
    profile.k = 8;
    assert_rejected(wrong_mfma);

    let first_store = operations(&canonical)
        .iter()
        .find(|operation| matches!(operation.kind, OperationKind::Store { .. }))
        .unwrap()
        .clone();
    let mut extra_owner = canonical.clone();
    operations_mut(&mut extra_owner).push(first_store);
    assert_valid_but_noncanonical(extra_owner);
}
