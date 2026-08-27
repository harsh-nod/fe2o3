use std::collections::BTreeSet;

use fe2o3_kernel_ir::*;

fn profile() -> TiledGemmLdsV1Profile {
    TiledGemmLdsV1Profile::exact_gfx942_xnack_minus_cov6()
}

fn operations(module: &Module) -> &[Operation] {
    &module.functions[0].body.as_ref().unwrap().blocks[0].operations
}

fn operations_mut(module: &mut Module) -> &mut Vec<Operation> {
    &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations
}

fn assert_rejected(module: Module) {
    assert!(
        verify_tiled_gemm_lds_v1_module(&module, &profile()).is_err(),
        "hostile mutation was admitted"
    );
}

fn matrix_positions(module: &Module) -> Vec<usize> {
    operations(module)
        .iter()
        .enumerate()
        .filter_map(|(index, operation)| {
            matches!(operation.kind, OperationKind::Matrix(_)).then_some(index)
        })
        .collect()
}

fn allocation_positions(module: &Module) -> Vec<usize> {
    operations(module)
        .iter()
        .enumerate()
        .filter_map(|(index, operation)| {
            matches!(operation.kind, OperationKind::WorkgroupMemory(_)).then_some(index)
        })
        .collect()
}

fn barrier_position(module: &Module) -> usize {
    operations(module)
        .iter()
        .position(|operation| matches!(operation.kind, OperationKind::WorkgroupBarrier(_)))
        .expect("canonical workgroup barrier")
}

#[test]
fn xor4_lane_maps_are_bounded_bijective_and_match_transposed_b_staging() {
    let mut a_inputs = BTreeSet::new();
    let mut b_inputs = BTreeSet::new();
    let mut c_outputs = BTreeSet::new();
    let mut a_lds = BTreeSet::new();
    let mut b_lds = BTreeSet::new();

    for lane in 0..TILED_GEMM_LDS_V1_LANES {
        for component in 0..TILED_GEMM_LDS_V1_FRAGMENT_ELEMENTS {
            let row = lane % 16;
            let depth = 4 * (lane / 16) + component;
            let coordinate = tiled_gemm_lds_v1_fragment_coordinate(lane, component).unwrap();
            assert_eq!(coordinate, (row, depth));

            let a = tiled_gemm_lds_v1_a_index(lane, component).unwrap();
            let b = tiled_gemm_lds_v1_b_index(lane, component).unwrap();
            let c = tiled_gemm_lds_v1_c_index(lane, component).unwrap();
            let physical = tiled_gemm_lds_v1_lds_index(lane, component).unwrap();
            assert_eq!(a, row * 16 + depth);
            assert_eq!(b, depth * 16 + row);
            assert_eq!(c, b);
            assert_eq!(
                physical,
                row * 16 + (depth ^ ((row & 3) * 4)),
                "exact XOR4 physical address"
            );
            assert!(a_inputs.insert(a));
            assert!(b_inputs.insert(b));
            assert!(c_outputs.insert(c));
            assert!(a_lds.insert(physical));
            assert!(b_lds.insert(physical));
        }
    }

    let complete = (0..TILED_GEMM_LDS_V1_ELEMENTS).collect::<BTreeSet<_>>();
    assert_eq!(a_inputs, complete);
    assert_eq!(b_inputs, complete);
    assert_eq!(c_outputs, complete);
    assert_eq!(a_lds, complete);
    assert_eq!(b_lds, complete);

    for (lane, component) in [(64, 0), (0, 4), (64, 4), (u32::MAX, u32::MAX)] {
        assert_eq!(tiled_gemm_lds_v1_fragment_coordinate(lane, component), None);
        assert_eq!(tiled_gemm_lds_v1_a_index(lane, component), None);
        assert_eq!(tiled_gemm_lds_v1_b_index(lane, component), None);
        assert_eq!(tiled_gemm_lds_v1_c_index(lane, component), None);
        assert_eq!(tiled_gemm_lds_v1_lds_index(lane, component), None);
    }
    for (row, column) in [(16, 0), (0, 16), (16, 16), (u32::MAX, u32::MAX)] {
        assert_eq!(tiled_gemm_lds_v1_xor4_index(row, column), None);
    }
}

#[test]
fn each_output_and_each_lds_cell_has_exactly_one_lane_component_owner() {
    let mut output_owners = vec![None; TILED_GEMM_LDS_V1_ELEMENTS as usize];
    let mut lds_owners = vec![None; TILED_GEMM_LDS_V1_ELEMENTS as usize];
    for lane in 0..TILED_GEMM_LDS_V1_LANES {
        for component in 0..TILED_GEMM_LDS_V1_FRAGMENT_ELEMENTS {
            let owner = (lane, component);
            let output = tiled_gemm_lds_v1_c_index(lane, component).unwrap() as usize;
            let lds = tiled_gemm_lds_v1_lds_index(lane, component).unwrap() as usize;
            assert_eq!(output_owners[output].replace(owner), None);
            assert_eq!(lds_owners[lds].replace(owner), None);
        }
    }
    assert!(output_owners.into_iter().all(|owner| owner.is_some()));
    assert!(lds_owners.into_iter().all(|owner| owner.is_some()));
}

#[test]
fn canonical_graph_has_exact_lds_barrier_mfma_and_output_shape() {
    let module = tiled_gemm_lds_v1_module();
    verify_tiled_gemm_lds_v1_module(&module, &profile()).expect("canonical LDS tiled GEMM");

    assert_eq!(module.functions.len(), 1);
    assert_eq!(module.kernels.len(), 1);
    assert_eq!(
        module.functions[0].id.as_str(),
        TILED_GEMM_LDS_V1_FUNCTION_ID
    );
    assert_eq!(module.kernels[0].id.as_str(), TILED_GEMM_LDS_V1_KERNEL_ID);
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
    assert_eq!(
        module.kernels[0].domain,
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64)
        }
    );
    assert_eq!(
        module.kernels[0].workgroup_size,
        Some(WorkgroupSize::new(64, 1, 1))
    );

    let operations = operations(&module);
    assert_eq!(operations.len(), 63);
    let allocations = allocation_positions(&module);
    assert_eq!(allocations.len(), 2);
    let allocation_results = allocations
        .iter()
        .map(|index| operations[*index].results[0].id)
        .collect::<Vec<_>>();
    assert_ne!(allocation_results[0], allocation_results[1]);
    for position in allocations {
        let OperationKind::WorkgroupMemory(memory) = &operations[position].kind else {
            unreachable!()
        };
        assert_eq!(memory.element, Type::Scalar(ScalarType::Bf16));
        assert_eq!(
            memory.extent,
            WorkgroupMemoryExtent::Static(TILED_GEMM_LDS_V1_ELEMENTS)
        );
        assert_eq!(memory.alignment, TILED_GEMM_LDS_V1_LDS_ALIGNMENT);
    }

    let matrices = matrix_positions(&module);
    assert_eq!(matrices.len(), 5);
    assert!(matches!(
        operations[matrices[0]].kind,
        OperationKind::Matrix(MatrixOperation {
            kind: MatrixOperationKind::LdsStore { .. },
            ..
        })
    ));
    assert!(matches!(
        operations[matrices[1]].kind,
        OperationKind::Matrix(MatrixOperation {
            kind: MatrixOperationKind::LdsStore { .. },
            ..
        })
    ));
    assert!(matches!(
        operations[matrices[2]].kind,
        OperationKind::Matrix(MatrixOperation {
            kind: MatrixOperationKind::LdsLoad { .. },
            ..
        })
    ));
    assert!(matches!(
        operations[matrices[3]].kind,
        OperationKind::Matrix(MatrixOperation {
            kind: MatrixOperationKind::LdsLoad { .. },
            ..
        })
    ));
    assert!(matches!(
        operations[matrices[4]].kind,
        OperationKind::Matrix(MatrixOperation {
            kind: MatrixOperationKind::MultiplyAccumulate { .. },
            ..
        })
    ));
    let barrier = barrier_position(&module);
    assert!(matrices[0] < barrier && matrices[1] < barrier);
    assert!(barrier < matrices[2] && barrier < matrices[3] && matrices[3] < matrices[4]);

    let mut store_bases = Vec::new();
    let mut load_bases = Vec::new();
    for position in matrices.iter().copied() {
        let OperationKind::Matrix(matrix) = &operations[position].kind else {
            unreachable!()
        };
        match matrix.kind {
            MatrixOperationKind::LdsStore { base, profile, .. } => {
                assert_eq!(profile.layout, MatrixLayout::RowMajorXor4);
                store_bases.push(base);
            }
            MatrixOperationKind::LdsLoad { base, profile } => {
                assert_eq!(profile.layout, MatrixLayout::RowMajorXor4);
                assert_eq!(operations[position].results.len(), 4);
                load_bases.push(base);
            }
            MatrixOperationKind::MultiplyAccumulate { profile, .. } => {
                assert_eq!(profile, MatrixMultiplyProfile::bf16_f32_m16n16k16_wave64());
                assert_eq!(operations[position].results.len(), 4);
            }
            MatrixOperationKind::ScaledMultiplyAccumulate { .. } => {
                unreachable!("the gfx942 BF16 fixture cannot contain a scaled gfx950 MFMA")
            }
        }
    }
    assert_eq!(store_bases, allocation_results);
    assert_eq!(load_bases, allocation_results);

    let global_loads = operations
        .iter()
        .filter(|operation| {
            matches!(
                operation.kind,
                OperationKind::Load {
                    access: MemoryAccess {
                        address_space: AddressSpace::Global,
                        ..
                    },
                    ..
                }
            )
        })
        .count();
    let global_stores = operations
        .iter()
        .filter(|operation| {
            matches!(
                operation.kind,
                OperationKind::Store {
                    access: MemoryAccess {
                        address_space: AddressSpace::Global,
                        ..
                    },
                    ..
                }
            )
        })
        .count();
    assert_eq!(global_loads, 8);
    assert_eq!(global_stores, 4);

    let effects = operations
        .iter()
        .flat_map(Operation::memory_effects)
        .collect::<Vec<_>>();
    assert_eq!(
        effects
            .iter()
            .filter(|effect| **effect == MemoryEffect::Allocate(AddressSpace::Workgroup))
            .count(),
        2
    );
    assert_eq!(
        effects
            .iter()
            .filter(|effect| **effect == MemoryEffect::Write(AddressSpace::Workgroup))
            .count(),
        2
    );
    assert_eq!(
        effects
            .iter()
            .filter(|effect| **effect == MemoryEffect::Read(AddressSpace::Workgroup))
            .count(),
        2
    );
    assert_eq!(
        effects
            .iter()
            .filter(|effect| matches!(effect, MemoryEffect::Synchronize { .. }))
            .count(),
        1
    );
}

#[test]
fn exact_profile_accounts_for_two_separate_static_tiles() {
    let profile = profile();
    assert_eq!(profile.lds_allocations, 2);
    assert_eq!(profile.lds_elements_per_allocation, 256);
    assert_eq!(profile.lds_bytes_per_allocation, 512);
    assert_eq!(profile.static_lds_bytes, 1024);
    assert_eq!(profile.lds_alignment, 16);
    assert_eq!(profile.output_elements_per_lane, 4);
    assert_eq!(
        profile.static_lds_bytes,
        profile.lds_allocations * profile.lds_bytes_per_allocation
    );
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
    mutated!(m, 15);
    mutated!(n, 17);
    mutated!(k, 32);
    mutated!(a_elements, 255);
    mutated!(b_elements, 257);
    mutated!(c_elements, 0);
    mutated!(tile_rows, 2);
    mutated!(tile_columns, 2);
    mutated!(depth_tiles, 2);
    mutated!(wave_width, WaveWidth::Wave32);
    mutated!(launch_extent_x, 128);
    mutated!(workgroup_size, WorkgroupSize::new(32, 2, 1));
    mutated!(lds_allocations, 1);
    mutated!(lds_elements_per_allocation, 512);
    mutated!(lds_bytes_per_allocation, 1024);
    mutated!(static_lds_bytes, 512);
    mutated!(lds_alignment, 2);
    mutated!(output_elements_per_lane, 8);

    assert_eq!(mutations.len(), 20);
    for mutation in mutations {
        assert_eq!(
            verify_tiled_gemm_lds_v1_module(&tiled_gemm_lds_v1_module(), &mutation),
            Err(TiledGemmLdsV1Error::UnsupportedProfile)
        );
    }
}

#[test]
fn deleting_duplicating_or_reordering_every_operation_fails_closed() {
    let canonical = tiled_gemm_lds_v1_module();
    let operation_count = operations(&canonical).len();

    for index in 0..operation_count {
        let mut deleted = canonical.clone();
        operations_mut(&mut deleted).remove(index);
        assert_rejected(deleted);

        let mut duplicated = canonical.clone();
        let duplicate = operations(&duplicated)[index].clone();
        operations_mut(&mut duplicated).insert(index, duplicate);
        assert_rejected(duplicated);
    }

    for index in 0..operation_count - 1 {
        let mut reordered = canonical.clone();
        operations_mut(&mut reordered).swap(index, index + 1);
        assert_rejected(reordered);
    }
}

#[test]
fn mutating_every_ssa_result_identity_and_type_fails_closed() {
    let canonical = tiled_gemm_lds_v1_module();
    for operation_index in 0..operations(&canonical).len() {
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
}

#[test]
fn lds_alias_extent_layout_and_provenance_mutations_fail_closed() {
    let canonical = tiled_gemm_lds_v1_module();
    let allocations = allocation_positions(&canonical);
    let matrices = matrix_positions(&canonical);
    let a_base = operations(&canonical)[allocations[0]].results[0].id;

    for &allocation_position in &allocations {
        for extent in [
            WorkgroupMemoryExtent::Static(255),
            WorkgroupMemoryExtent::Static(257),
            WorkgroupMemoryExtent::Dynamic,
            WorkgroupMemoryExtent::DynamicAtLeast(256),
        ] {
            let mut mutation = canonical.clone();
            let OperationKind::WorkgroupMemory(memory) =
                &mut operations_mut(&mut mutation)[allocation_position].kind
            else {
                unreachable!()
            };
            memory.extent = extent;
            assert_rejected(mutation);
        }

        for alignment in [1, 2, 32] {
            let mut mutation = canonical.clone();
            let OperationKind::WorkgroupMemory(memory) =
                &mut operations_mut(&mut mutation)[allocation_position].kind
            else {
                unreachable!()
            };
            memory.alignment = alignment;
            assert_rejected(mutation);
        }
    }

    for position in matrices.iter().take(4).copied() {
        let mut alias = canonical.clone();
        let OperationKind::Matrix(matrix) = &mut operations_mut(&mut alias)[position].kind else {
            unreachable!()
        };
        match &mut matrix.kind {
            MatrixOperationKind::LdsLoad { base, .. }
            | MatrixOperationKind::LdsStore { base, .. } => *base = a_base,
            MatrixOperationKind::MultiplyAccumulate { .. }
            | MatrixOperationKind::ScaledMultiplyAccumulate { .. } => unreachable!(),
        }
        if operations(&alias) != operations(&canonical) {
            assert_rejected(alias);
        }

        let mut layout = canonical.clone();
        let OperationKind::Matrix(matrix) = &mut operations_mut(&mut layout)[position].kind else {
            unreachable!()
        };
        match &mut matrix.kind {
            MatrixOperationKind::LdsLoad { profile, .. }
            | MatrixOperationKind::LdsStore { profile, .. } => {
                profile.fragment_elements = 8;
            }
            MatrixOperationKind::MultiplyAccumulate { .. }
            | MatrixOperationKind::ScaledMultiplyAccumulate { .. } => unreachable!(),
        }
        assert_rejected(layout);
    }
}

#[test]
fn barrier_and_matrix_hostile_mutations_fail_closed() {
    let canonical = tiled_gemm_lds_v1_module();
    let barrier = barrier_position(&canonical);
    let matrices = matrix_positions(&canonical);

    let mut removed_barrier = canonical.clone();
    operations_mut(&mut removed_barrier).remove(barrier);
    assert_rejected(removed_barrier);

    let mut late_barrier = canonical.clone();
    let barrier_operation = operations_mut(&mut late_barrier).remove(barrier);
    let mfma = matrix_positions(&late_barrier)[4];
    operations_mut(&mut late_barrier).insert(mfma + 1, barrier_operation);
    assert_rejected(late_barrier);

    for mutate in 0..4 {
        let mut module = canonical.clone();
        let OperationKind::WorkgroupBarrier(value) = &mut operations_mut(&mut module)[barrier].kind
        else {
            unreachable!()
        };
        match mutate {
            0 => value.memory_scope = SynchronizationScope::Subgroup,
            1 => value.semantics.ordering = MemoryOrdering::Release,
            2 => value.semantics.address_spaces = BTreeSet::from([AddressSpace::Global]),
            3 => value.convergence = Convergence::uniform(SynchronizationScope::Subgroup),
            _ => unreachable!(),
        }
        assert_rejected(module);
    }

    for position in matrices {
        for mutation in 0..2 {
            let mut module = canonical.clone();
            let OperationKind::Matrix(matrix) = &mut operations_mut(&mut module)[position].kind
            else {
                unreachable!()
            };
            if mutation == 0 {
                matrix.active_lanes = 32;
            } else {
                matrix.convergence = Convergence::uniform(SynchronizationScope::Workgroup);
            }
            assert_rejected(module);
        }
    }
}

#[test]
fn mfma_operand_profile_and_output_store_mutations_fail_closed() {
    let canonical = tiled_gemm_lds_v1_module();
    let mfma_position = *matrix_positions(&canonical).last().unwrap();

    for mutation in 0..5 {
        let mut module = canonical.clone();
        let OperationKind::Matrix(matrix) = &mut operations_mut(&mut module)[mfma_position].kind
        else {
            unreachable!()
        };
        let MatrixOperationKind::MultiplyAccumulate {
            lhs,
            rhs,
            accumulator,
            profile,
        } = &mut matrix.kind
        else {
            unreachable!()
        };
        match mutation {
            0 => lhs[0] = rhs[0],
            1 => rhs[3] = lhs[3],
            2 => accumulator[1] = lhs[1],
            3 => profile.k = 8,
            4 => profile.wave_width = WaveWidth::Wave32,
            _ => unreachable!(),
        }
        assert_rejected(module);
    }

    let stores = operations(&canonical)
        .iter()
        .enumerate()
        .filter_map(|(index, operation)| {
            matches!(operation.kind, OperationKind::Store { .. }).then_some(index)
        })
        .collect::<Vec<_>>();
    assert_eq!(stores.len(), 4);
    for position in stores {
        for mutation in 0..3 {
            let mut module = canonical.clone();
            let OperationKind::Store {
                pointer,
                value,
                access,
            } = &mut operations_mut(&mut module)[position].kind
            else {
                unreachable!()
            };
            match mutation {
                0 => *pointer = ValueId(0),
                1 => *value = ValueId(0),
                2 => access.alignment = 1,
                _ => unreachable!(),
            }
            assert_rejected(module);
        }
    }
}

#[test]
fn identity_launch_signature_and_every_capability_mutation_fail_closed() {
    let canonical = tiled_gemm_lds_v1_module();

    let mut module_id = canonical.clone();
    module_id.id = "hostile::module".into();
    assert_rejected(module_id);

    let mut function_id = canonical.clone();
    function_id.functions[0].id = "hostile_function".into();
    assert_rejected(function_id);

    let mut kernel_id = canonical.clone();
    kernel_id.kernels[0].id = "hostile_kernel".into();
    assert_rejected(kernel_id);

    let mut entry = canonical.clone();
    entry.kernels[0].entry = "hostile_entry".into();
    assert_rejected(entry);

    let mut extent = canonical.clone();
    extent.kernels[0].domain = LaunchDomain::D1 {
        x: LaunchExtent::Static(128),
    };
    assert_rejected(extent);

    let mut workgroup = canonical.clone();
    workgroup.kernels[0].workgroup_size = Some(WorkgroupSize::new(32, 2, 1));
    assert_rejected(workgroup);

    for parameter in 0..3 {
        let mut signature = canonical.clone();
        signature.functions[0].signature.parameters[parameter] = Type::Scalar(ScalarType::U64);
        assert_rejected(signature);

        let mut parameter_id = canonical.clone();
        parameter_id.functions[0].body.as_mut().unwrap().parameters[parameter] = ValueId(99);
        assert_rejected(parameter_id);
    }

    for level in 0..3 {
        let capabilities = match level {
            0 => &canonical.required_capabilities,
            1 => &canonical.functions[0].required_capabilities,
            2 => &canonical.kernels[0].required_capabilities,
            _ => unreachable!(),
        };
        for capability in capabilities {
            let mut removed = canonical.clone();
            match level {
                0 => &mut removed.required_capabilities,
                1 => &mut removed.functions[0].required_capabilities,
                2 => &mut removed.kernels[0].required_capabilities,
                _ => unreachable!(),
            }
            .remove(capability);
            assert_rejected(removed);
        }

        let mut added = canonical.clone();
        match level {
            0 => &mut added.required_capabilities,
            1 => &mut added.functions[0].required_capabilities,
            2 => &mut added.kernels[0].required_capabilities,
            _ => unreachable!(),
        }
        .insert(TargetCapability::Float64);
        assert_rejected(added);
    }
}

#[test]
fn zero_lds_v1_remains_distinct_and_accepted() {
    let zero_lds = tiled_gemm_v1_module();
    verify_tiled_gemm_v1_module(
        &zero_lds,
        &TiledGemmV1Profile::exact_gfx942_xnack_minus_cov6(),
    )
    .expect("existing zero-LDS V1 remains accepted");
    assert_ne!(zero_lds, tiled_gemm_lds_v1_module());
    assert!(!operations(&zero_lds).iter().any(|operation| {
        matches!(
            operation.kind,
            OperationKind::WorkgroupMemory(_) | OperationKind::WorkgroupBarrier(_)
        )
    }));
}
