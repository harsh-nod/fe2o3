use std::collections::BTreeSet;

use fe2o3_kernel_ir::*;

fn profile() -> TiledGemmLdsK32V2Profile {
    TiledGemmLdsK32V2Profile::exact_gfx942_xnack_minus_cov6()
}

fn blocks(module: &Module) -> &[BasicBlock] {
    &module.functions[0].body.as_ref().unwrap().blocks
}

fn blocks_mut(module: &mut Module) -> &mut Vec<BasicBlock> {
    &mut module.functions[0].body.as_mut().unwrap().blocks
}

fn assert_rejected(module: Module) {
    assert!(
        verify_tiled_gemm_lds_k32_v2_module(&module, &profile()).is_err(),
        "hostile mutation was admitted"
    );
}

fn assert_valid_but_noncanonical(module: Module) {
    verify_module(&module).expect("hostile graph remains generically well-formed Kernel IR");
    assert_eq!(
        verify_tiled_gemm_lds_k32_v2_module(&module, &profile()),
        Err(TiledGemmLdsK32V2Error::NonCanonicalKernelIr)
    );
}

fn operation_positions(
    block: &BasicBlock,
    predicate: impl Fn(&OperationKind) -> bool,
) -> Vec<usize> {
    block
        .operations
        .iter()
        .enumerate()
        .filter_map(|(index, operation)| predicate(&operation.kind).then_some(index))
        .collect()
}

#[test]
fn two_phase_maps_cover_inputs_once_and_reuse_each_lds_cell() {
    let mut all_a = BTreeSet::new();
    let mut all_b = BTreeSet::new();
    let mut all_c = BTreeSet::new();

    for phase in 0..TILED_GEMM_LDS_K32_V2_PHASES {
        let mut phase_a = BTreeSet::new();
        let mut phase_b = BTreeSet::new();
        let mut phase_lds = BTreeSet::new();
        for lane in 0..TILED_GEMM_LDS_K32_V2_LANES {
            for component in 0..TILED_GEMM_LDS_K32_V2_FRAGMENT_ELEMENTS {
                let (axis, local_depth) =
                    tiled_gemm_lds_k32_v2_fragment_coordinate(lane, component).unwrap();
                assert_eq!(axis, lane % 16);
                assert_eq!(local_depth, 4 * (lane / 16) + component);

                let a = tiled_gemm_lds_k32_v2_a_index(phase, lane, component).unwrap();
                let b = tiled_gemm_lds_k32_v2_b_index(phase, lane, component).unwrap();
                let c = tiled_gemm_lds_k32_v2_c_index(lane, component).unwrap();
                let lds = tiled_gemm_lds_k32_v2_lds_index(lane, component).unwrap();
                assert_eq!(a, axis * 32 + phase * 16 + local_depth);
                assert_eq!(b, (phase * 16 + local_depth) * 16 + axis);
                assert_eq!(c, local_depth * 16 + axis);
                assert_eq!(
                    lds,
                    axis * 16 + (local_depth ^ ((axis & 3) * 4)),
                    "exact XOR4 physical address"
                );
                assert!(phase_a.insert(a));
                assert!(phase_b.insert(b));
                assert!(phase_lds.insert(lds));
                all_a.insert(a);
                all_b.insert(b);
                all_c.insert(c);
            }
        }

        assert_eq!(
            phase_lds,
            (0..TILED_GEMM_LDS_K32_V2_TILE_ELEMENTS).collect()
        );
        assert_eq!(phase_a.len(), TILED_GEMM_LDS_K32_V2_TILE_ELEMENTS as usize);
        assert_eq!(phase_b.len(), TILED_GEMM_LDS_K32_V2_TILE_ELEMENTS as usize);
    }

    assert_eq!(all_a, (0..TILED_GEMM_LDS_K32_V2_INPUT_ELEMENTS).collect());
    assert_eq!(all_b, (0..TILED_GEMM_LDS_K32_V2_INPUT_ELEMENTS).collect());
    assert_eq!(all_c, (0..TILED_GEMM_LDS_K32_V2_TILE_ELEMENTS).collect());

    for (phase, lane, component) in [(2, 0, 0), (0, 64, 0), (0, 0, 4), (u32::MAX, 0, 0)] {
        assert_eq!(tiled_gemm_lds_k32_v2_a_index(phase, lane, component), None);
        assert_eq!(tiled_gemm_lds_k32_v2_b_index(phase, lane, component), None);
    }
    for (lane, component) in [(64, 0), (0, 4), (64, 4), (u32::MAX, u32::MAX)] {
        assert_eq!(
            tiled_gemm_lds_k32_v2_fragment_coordinate(lane, component),
            None
        );
        assert_eq!(tiled_gemm_lds_k32_v2_c_index(lane, component), None);
        assert_eq!(tiled_gemm_lds_k32_v2_lds_index(lane, component), None);
    }
    for (row, column) in [(16, 0), (0, 16), (16, 16), (u32::MAX, u32::MAX)] {
        assert_eq!(tiled_gemm_lds_k32_v2_xor4_index(row, column), None);
    }
}

#[test]
fn canonical_graph_is_a_real_bounded_accumulator_carrying_loop() {
    let module = tiled_gemm_lds_k32_v2_module();
    verify_tiled_gemm_lds_k32_v2_module(&module, &profile()).expect("canonical K32 Slice 2");

    assert_eq!(module.functions.len(), 1);
    assert_eq!(module.kernels.len(), 1);
    assert_eq!(module.id.as_str(), TILED_GEMM_LDS_K32_V2_MODULE_ID);
    assert_eq!(
        module.functions[0].id.as_str(),
        TILED_GEMM_LDS_K32_V2_FUNCTION_ID
    );
    assert_eq!(
        module.kernels[0].id.as_str(),
        TILED_GEMM_LDS_K32_V2_KERNEL_ID
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
    assert_eq!(blocks(&module).len(), 4);
    assert_eq!(
        blocks(&module)
            .iter()
            .map(|block| block.operations.len())
            .collect::<Vec<_>>(),
        [33, 1, 41, 8]
    );

    let entry = &blocks(&module)[0];
    let header = &blocks(&module)[1];
    let body = &blocks(&module)[2];
    let store = &blocks(&module)[3];
    assert_eq!(header.parameters.len(), 5);
    assert_eq!(header.parameters[0].ty, Type::INDEX);
    assert!(
        header.parameters[1..]
            .iter()
            .all(|value| value.ty == Type::F32)
    );
    assert_eq!(store.parameters.len(), 4);
    assert!(store.parameters.iter().all(|value| value.ty == Type::F32));

    let Terminator::Branch {
        target,
        arguments: initial,
    } = entry.terminator.as_ref().unwrap()
    else {
        panic!("entry must initialize the loop")
    };
    assert_eq!(*target, BlockId(1));
    assert_eq!(initial.len(), 5);
    assert_eq!(initial[1..], [initial[1]; 4]);

    let Terminator::ConditionalBranch {
        then_target,
        then_arguments,
        else_target,
        else_arguments,
        ..
    } = header.terminator.as_ref().unwrap()
    else {
        panic!("header must check the exact phase bound")
    };
    assert_eq!(*then_target, BlockId(2));
    assert!(then_arguments.is_empty());
    assert_eq!(*else_target, BlockId(3));
    assert_eq!(
        else_arguments,
        &header.parameters[1..]
            .iter()
            .map(|value| value.id)
            .collect::<Vec<_>>()
    );

    let matrix_positions =
        operation_positions(body, |kind| matches!(kind, OperationKind::Matrix(_)));
    let barrier_positions = operation_positions(body, |kind| {
        matches!(kind, OperationKind::WorkgroupBarrier(_))
    });
    assert_eq!(matrix_positions.len(), 5);
    assert_eq!(barrier_positions.len(), 2);
    assert!(matrix_positions[1] < barrier_positions[0]);
    assert!(barrier_positions[0] < matrix_positions[2]);
    assert!(matrix_positions[4] < barrier_positions[1]);

    let allocation_positions = operation_positions(entry, |kind| {
        matches!(kind, OperationKind::WorkgroupMemory(_))
    });
    assert_eq!(allocation_positions.len(), 2);
    let allocation_ids = allocation_positions
        .iter()
        .map(|position| entry.operations[*position].results[0].id)
        .collect::<Vec<_>>();
    assert_ne!(allocation_ids[0], allocation_ids[1]);
    for position in allocation_positions {
        let OperationKind::WorkgroupMemory(memory) = &entry.operations[position].kind else {
            unreachable!()
        };
        assert_eq!(memory.element, Type::Scalar(ScalarType::Bf16));
        assert_eq!(
            memory.extent,
            WorkgroupMemoryExtent::Static(TILED_GEMM_LDS_K32_V2_TILE_ELEMENTS)
        );
        assert_eq!(memory.alignment, TILED_GEMM_LDS_K32_V2_LDS_ALIGNMENT);
    }

    let mut store_bases = Vec::new();
    let mut load_bases = Vec::new();
    let mut mfma_results = None;
    for position in matrix_positions {
        let operation = &body.operations[position];
        let OperationKind::Matrix(matrix) = &operation.kind else {
            unreachable!()
        };
        match &matrix.kind {
            MatrixOperationKind::LdsStore { base, profile, .. } => {
                assert_eq!(profile.layout, MatrixLayout::RowMajorXor4);
                store_bases.push(*base);
            }
            MatrixOperationKind::LdsLoad { base, profile } => {
                assert_eq!(profile.layout, MatrixLayout::RowMajorXor4);
                load_bases.push(*base);
            }
            MatrixOperationKind::MultiplyAccumulate {
                accumulator,
                profile,
                ..
            } => {
                assert_eq!(*profile, MatrixMultiplyProfile::bf16_f32_m16n16k16_wave64());
                assert_eq!(
                    *accumulator,
                    header.parameters[1..]
                        .iter()
                        .map(|value| value.id)
                        .collect::<Vec<_>>()
                        .as_slice()
                );
                mfma_results = Some(
                    operation
                        .results
                        .iter()
                        .map(|value| value.id)
                        .collect::<Vec<_>>(),
                );
            }
        }
    }
    assert_eq!(store_bases, allocation_ids);
    assert_eq!(load_bases, allocation_ids);

    let Terminator::Branch {
        target,
        arguments: backedge,
    } = body.terminator.as_ref().unwrap()
    else {
        panic!("loop body must branch back to its header")
    };
    assert_eq!(*target, BlockId(1));
    assert_eq!(backedge.len(), 5);
    assert_eq!(&backedge[1..], mfma_results.unwrap());

    let global_loads = body
        .operations
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
    let global_stores = store
        .operations
        .iter()
        .filter(|operation| matches!(operation.kind, OperationKind::Store { .. }))
        .count();
    assert_eq!(
        global_loads, 8,
        "one static load graph executes in both phases"
    );
    assert_eq!(global_stores, 4);
}

#[test]
fn exact_profile_accounts_for_reused_tiles_and_two_depth_phases() {
    let profile = profile();
    assert_eq!(profile.k, 32);
    assert_eq!(profile.depth_tiles, 2);
    assert_eq!(profile.phase_k, 16);
    assert_eq!(profile.a_elements, 512);
    assert_eq!(profile.b_elements, 512);
    assert_eq!(profile.c_elements, 256);
    assert_eq!(profile.lds_allocations, 2);
    assert_eq!(profile.lds_elements_per_allocation, 256);
    assert_eq!(profile.lds_bytes_per_allocation, 512);
    assert_eq!(profile.static_lds_bytes, 1024);
    assert_eq!(profile.lds_alignment, 16);
    assert_eq!(
        profile.static_lds_bytes,
        profile.lds_allocations * profile.lds_bytes_per_allocation
    );
}

#[test]
fn every_profile_field_mutation_fails_closed() {
    let mut mutations = Vec::new();

    let mut candidate = profile();
    candidate.target = TargetCapability::WaveWidth(WaveWidth::Wave64);
    mutations.push(candidate);
    let mut candidate = profile();
    candidate.code_object_version = 5;
    mutations.push(candidate);
    let mut candidate = profile();
    candidate.m = 15;
    mutations.push(candidate);
    let mut candidate = profile();
    candidate.n = 17;
    mutations.push(candidate);
    let mut candidate = profile();
    candidate.k = 16;
    mutations.push(candidate);
    let mut candidate = profile();
    candidate.a_elements = 511;
    mutations.push(candidate);
    let mut candidate = profile();
    candidate.b_elements = 513;
    mutations.push(candidate);
    let mut candidate = profile();
    candidate.c_elements = 255;
    mutations.push(candidate);
    let mut candidate = profile();
    candidate.tile_rows = 2;
    mutations.push(candidate);
    let mut candidate = profile();
    candidate.tile_columns = 2;
    mutations.push(candidate);
    let mut candidate = profile();
    candidate.depth_tiles = 1;
    mutations.push(candidate);
    let mut candidate = profile();
    candidate.phase_k = 8;
    mutations.push(candidate);
    let mut candidate = profile();
    candidate.wave_width = WaveWidth::Wave32;
    mutations.push(candidate);
    let mut candidate = profile();
    candidate.launch_extent_x = 128;
    mutations.push(candidate);
    let mut candidate = profile();
    candidate.workgroup_size = WorkgroupSize::new(32, 2, 1);
    mutations.push(candidate);
    let mut candidate = profile();
    candidate.lds_allocations = 1;
    mutations.push(candidate);
    let mut candidate = profile();
    candidate.lds_elements_per_allocation = 512;
    mutations.push(candidate);
    let mut candidate = profile();
    candidate.lds_bytes_per_allocation = 1024;
    mutations.push(candidate);
    let mut candidate = profile();
    candidate.static_lds_bytes = 512;
    mutations.push(candidate);
    let mut candidate = profile();
    candidate.lds_alignment = 2;
    mutations.push(candidate);
    let mut candidate = profile();
    candidate.output_elements_per_lane = 8;
    mutations.push(candidate);

    assert_eq!(mutations.len(), 21);
    for mutation in mutations {
        assert_eq!(
            verify_tiled_gemm_lds_k32_v2_module(&tiled_gemm_lds_k32_v2_module(), &mutation),
            Err(TiledGemmLdsK32V2Error::UnsupportedProfile)
        );
    }
}

#[test]
fn deleting_duplicating_or_reordering_every_operation_fails_closed() {
    let canonical = tiled_gemm_lds_k32_v2_module();

    for block_index in 0..blocks(&canonical).len() {
        let operation_count = blocks(&canonical)[block_index].operations.len();
        for operation_index in 0..operation_count {
            let mut deleted = canonical.clone();
            blocks_mut(&mut deleted)[block_index]
                .operations
                .remove(operation_index);
            assert_rejected(deleted);

            let mut duplicated = canonical.clone();
            let duplicate = blocks(&duplicated)[block_index].operations[operation_index].clone();
            blocks_mut(&mut duplicated)[block_index]
                .operations
                .insert(operation_index, duplicate);
            assert_rejected(duplicated);
        }

        for operation_index in 0..operation_count.saturating_sub(1) {
            let mut reordered = canonical.clone();
            blocks_mut(&mut reordered)[block_index]
                .operations
                .swap(operation_index, operation_index + 1);
            assert_rejected(reordered);
        }
    }
}

#[test]
fn deleted_reuse_barrier_wrong_phase_offset_and_reset_accumulator_fail_closed() {
    let canonical = tiled_gemm_lds_k32_v2_module();
    let body = &blocks(&canonical)[2];
    let barriers = operation_positions(body, |kind| {
        matches!(kind, OperationKind::WorkgroupBarrier(_))
    });
    assert_eq!(barriers.len(), 2);

    let mut missing_reuse_barrier = canonical.clone();
    blocks_mut(&mut missing_reuse_barrier)[2]
        .operations
        .remove(barriers[1]);
    assert_valid_but_noncanonical(missing_reuse_barrier);

    let phase_id = blocks(&canonical)[1].parameters[0].id;
    let phase_count = blocks(&canonical)[0]
        .operations
        .iter()
        .find_map(|operation| match operation.kind {
            OperationKind::Constant(Constant::Index(2)) => Some(operation.results[0].id),
            _ => None,
        })
        .unwrap();
    let phase_offset_position = body
        .operations
        .iter()
        .position(|operation| {
            matches!(
                operation.kind,
                OperationKind::Binary {
                    op: BinaryOp::Multiply,
                    lhs,
                    ..
                } if lhs == phase_id
            )
        })
        .unwrap();
    let mut wrong_phase_offset = canonical.clone();
    let OperationKind::Binary { rhs, .. } =
        &mut blocks_mut(&mut wrong_phase_offset)[2].operations[phase_offset_position].kind
    else {
        unreachable!()
    };
    *rhs = phase_count;
    assert_valid_but_noncanonical(wrong_phase_offset);

    let zero = blocks(&canonical)[0]
        .operations
        .iter()
        .find_map(|operation| match operation.kind {
            OperationKind::Constant(Constant::F32Bits(0)) => Some(operation.results[0].id),
            _ => None,
        })
        .unwrap();
    let mfma_position = operation_positions(body, |kind| {
        matches!(
            kind,
            OperationKind::Matrix(MatrixOperation {
                kind: MatrixOperationKind::MultiplyAccumulate { .. },
                ..
            })
        )
    })[0];
    let mut reset_accumulator = canonical.clone();
    let OperationKind::Matrix(matrix) =
        &mut blocks_mut(&mut reset_accumulator)[2].operations[mfma_position].kind
    else {
        unreachable!()
    };
    let MatrixOperationKind::MultiplyAccumulate { accumulator, .. } = &mut matrix.kind else {
        unreachable!()
    };
    *accumulator = [zero; 4];
    assert_valid_but_noncanonical(reset_accumulator);
}

#[test]
fn aliased_tiles_and_extra_output_owner_fail_closed() {
    let canonical = tiled_gemm_lds_k32_v2_module();
    let a_lds = blocks(&canonical)[0]
        .operations
        .iter()
        .find_map(|operation| {
            matches!(operation.kind, OperationKind::WorkgroupMemory(_))
                .then_some(operation.results[0].id)
        })
        .unwrap();
    let matrix_positions = operation_positions(&blocks(&canonical)[2], |kind| {
        matches!(kind, OperationKind::Matrix(_))
    });
    let mut aliased = canonical.clone();
    for position in [matrix_positions[1], matrix_positions[3]] {
        let OperationKind::Matrix(matrix) =
            &mut blocks_mut(&mut aliased)[2].operations[position].kind
        else {
            unreachable!()
        };
        match &mut matrix.kind {
            MatrixOperationKind::LdsStore { base, .. }
            | MatrixOperationKind::LdsLoad { base, .. } => *base = a_lds,
            MatrixOperationKind::MultiplyAccumulate { .. } => unreachable!(),
        }
    }
    assert_valid_but_noncanonical(aliased);

    let first_store = blocks(&canonical)[3]
        .operations
        .iter()
        .find(|operation| matches!(operation.kind, OperationKind::Store { .. }))
        .unwrap()
        .clone();
    assert!(first_store.results.is_empty());
    let mut extra_owner = canonical.clone();
    blocks_mut(&mut extra_owner)[3].operations.push(first_store);
    assert_valid_but_noncanonical(extra_owner);
}

#[test]
fn identities_launch_signature_capabilities_and_loop_edges_fail_closed() {
    let canonical = tiled_gemm_lds_k32_v2_module();

    let mut module_id = canonical.clone();
    module_id.id = "hostile::module".into();
    assert_rejected(module_id);
    let mut function_id = canonical.clone();
    function_id.functions[0].id = "hostile_function".into();
    assert_rejected(function_id);
    let mut kernel_id = canonical.clone();
    kernel_id.kernels[0].id = "hostile_kernel".into();
    assert_rejected(kernel_id);
    let mut launch = canonical.clone();
    launch.kernels[0].domain = LaunchDomain::D1 {
        x: LaunchExtent::Static(128),
    };
    assert_rejected(launch);
    let mut workgroup = canonical.clone();
    workgroup.kernels[0].workgroup_size = Some(WorkgroupSize::new(32, 2, 1));
    assert_rejected(workgroup);

    for parameter in 0..3 {
        let mut signature = canonical.clone();
        signature.functions[0].signature.parameters[parameter] = Type::Scalar(ScalarType::U64);
        assert_rejected(signature);
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
    }

    let mut unbounded = canonical.clone();
    let phase_step = blocks(&unbounded)[0]
        .operations
        .iter()
        .find_map(|operation| match operation.kind {
            OperationKind::Constant(Constant::Index(1)) => Some(operation.results[0].id),
            _ => None,
        })
        .unwrap();
    let OperationKind::Compare { rhs, .. } = &mut blocks_mut(&mut unbounded)[1].operations[0].kind
    else {
        unreachable!()
    };
    *rhs = phase_step;
    assert_valid_but_noncanonical(unbounded);

    let mut wrong_backedge = canonical.clone();
    let Terminator::Branch { target, .. } = blocks_mut(&mut wrong_backedge)[2]
        .terminator
        .as_mut()
        .unwrap()
    else {
        unreachable!()
    };
    *target = BlockId(3);
    assert_rejected(wrong_backedge);
}
