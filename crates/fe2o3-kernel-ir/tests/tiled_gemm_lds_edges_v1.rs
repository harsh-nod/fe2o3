use std::collections::BTreeSet;

use fe2o3_kernel_ir::*;

fn profile() -> TiledGemmLdsEdgesV1Profile {
    TiledGemmLdsEdgesV1Profile::exact_gfx942_xnack_minus_cov6()
}

fn blocks(module: &Module) -> &[BasicBlock] {
    &module.functions[0].body.as_ref().unwrap().blocks
}

fn block(module: &Module, id: BlockId) -> &BasicBlock {
    blocks(module)
        .iter()
        .find(|block| block.id == id)
        .unwrap_or_else(|| panic!("missing canonical block {id}"))
}

fn block_mut(module: &mut Module, id: BlockId) -> &mut BasicBlock {
    module.functions[0]
        .body
        .as_mut()
        .unwrap()
        .blocks
        .iter_mut()
        .find(|block| block.id == id)
        .unwrap_or_else(|| panic!("missing canonical block {id}"))
}

fn assert_rejected(module: Module) {
    assert!(
        verify_tiled_gemm_lds_edges_v1_module(&module, &profile()).is_err(),
        "hostile mutation was admitted"
    );
}

fn assert_valid_but_noncanonical(module: Module) {
    verify_module(&module).expect("mutation remains generically valid Kernel IR");
    assert_eq!(
        verify_tiled_gemm_lds_edges_v1_module(&module, &profile()),
        Err(TiledGemmLdsEdgesV1Error::NonCanonicalKernelIr)
    );
}

fn constant_result(module: &Module, constant: Constant) -> ValueId {
    blocks(module)
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| {
            (operation.kind == OperationKind::Constant(constant.clone()))
                .then_some(operation.results[0].id)
        })
        .unwrap_or_else(|| panic!("missing canonical constant {constant:?}"))
}

#[test]
fn exhaustive_coordinates_classify_every_valid_and_tail_slot() {
    let mut a_valid = BTreeSet::new();
    let mut b_valid = BTreeSet::new();
    let mut c_valid = BTreeSet::new();
    let mut a_tail = 0;
    let mut b_tail = 0;
    let mut c_tail = 0;

    for group_y in 0..TILED_GEMM_LDS_EDGES_V1_TILE_ROWS {
        for phase in 0..TILED_GEMM_LDS_EDGES_V1_PHASES {
            for lane in 0..TILED_GEMM_LDS_EDGES_V1_LANES {
                for component in 0..TILED_GEMM_LDS_EDGES_V1_FRAGMENT_ELEMENTS {
                    let (tile_row, tile_depth) =
                        tiled_gemm_lds_edges_v1_staging_coordinate(lane, component).unwrap();
                    let coordinate =
                        tiled_gemm_lds_edges_v1_a_coordinate(group_y, phase, lane, component)
                            .unwrap();
                    let row = group_y * 16 + tile_row;
                    let depth = phase * 16 + tile_depth;
                    assert_eq!((coordinate.row(), coordinate.column()), (row, depth));
                    assert_eq!(coordinate.is_valid(), row < 17 && depth < 18);
                    assert_eq!(
                        tiled_gemm_lds_edges_v1_a_index(group_y, phase, lane, component),
                        coordinate.index()
                    );
                    match coordinate {
                        TiledGemmLdsEdgesV1Coordinate::Valid { index, .. } => {
                            assert_eq!(index, row * 18 + depth);
                            assert!(index < TILED_GEMM_LDS_EDGES_V1_A_ELEMENTS);
                            assert!(a_valid.insert(index));
                        }
                        TiledGemmLdsEdgesV1Coordinate::Tail { .. } => a_tail += 1,
                    }
                }
            }
        }
    }

    for group_x in 0..TILED_GEMM_LDS_EDGES_V1_TILE_COLUMNS {
        for phase in 0..TILED_GEMM_LDS_EDGES_V1_PHASES {
            for lane in 0..TILED_GEMM_LDS_EDGES_V1_LANES {
                for component in 0..TILED_GEMM_LDS_EDGES_V1_FRAGMENT_ELEMENTS {
                    let (tile_column, tile_depth) =
                        tiled_gemm_lds_edges_v1_staging_coordinate(lane, component).unwrap();
                    let coordinate =
                        tiled_gemm_lds_edges_v1_b_coordinate(group_x, phase, lane, component)
                            .unwrap();
                    let depth = phase * 16 + tile_depth;
                    let column = group_x * 16 + tile_column;
                    assert_eq!((coordinate.row(), coordinate.column()), (depth, column));
                    assert_eq!(coordinate.is_valid(), depth < 18 && column < 19);
                    assert_eq!(
                        tiled_gemm_lds_edges_v1_b_index(group_x, phase, lane, component),
                        coordinate.index()
                    );
                    match coordinate {
                        TiledGemmLdsEdgesV1Coordinate::Valid { index, .. } => {
                            assert_eq!(index, depth * 19 + column);
                            assert!(index < TILED_GEMM_LDS_EDGES_V1_B_ELEMENTS);
                            assert!(b_valid.insert(index));
                        }
                        TiledGemmLdsEdgesV1Coordinate::Tail { .. } => b_tail += 1,
                    }
                }
            }
        }
    }

    for group_y in 0..TILED_GEMM_LDS_EDGES_V1_TILE_ROWS {
        for group_x in 0..TILED_GEMM_LDS_EDGES_V1_TILE_COLUMNS {
            let origin = tiled_gemm_lds_edges_v1_tile_origin(group_x, group_y).unwrap();
            assert_eq!(origin, (group_y * 16, group_x * 16));
            for lane in 0..TILED_GEMM_LDS_EDGES_V1_LANES {
                for component in 0..TILED_GEMM_LDS_EDGES_V1_FRAGMENT_ELEMENTS {
                    let (tile_row, tile_column) =
                        tiled_gemm_lds_edges_v1_output_coordinate(lane, component).unwrap();
                    let coordinate =
                        tiled_gemm_lds_edges_v1_c_coordinate(group_x, group_y, lane, component)
                            .unwrap();
                    let row = group_y * 16 + tile_row;
                    let column = group_x * 16 + tile_column;
                    assert_eq!((coordinate.row(), coordinate.column()), (row, column));
                    assert_eq!(coordinate.is_valid(), row < 17 && column < 19);
                    assert_eq!(
                        tiled_gemm_lds_edges_v1_c_index(group_x, group_y, lane, component,),
                        coordinate.index(),
                        "C input and output share one predicate/index"
                    );
                    match coordinate {
                        TiledGemmLdsEdgesV1Coordinate::Valid { index, .. } => {
                            assert_eq!(index, row * 19 + column);
                            assert!(index < TILED_GEMM_LDS_EDGES_V1_C_ELEMENTS);
                            assert!(c_valid.insert(index), "collapsed C ownership at {index}");
                        }
                        TiledGemmLdsEdgesV1Coordinate::Tail { .. } => c_tail += 1,
                    }
                }
            }
        }
    }

    assert_eq!(a_valid, (0..TILED_GEMM_LDS_EDGES_V1_A_ELEMENTS).collect());
    assert_eq!(b_valid, (0..TILED_GEMM_LDS_EDGES_V1_B_ELEMENTS).collect());
    assert_eq!(c_valid, (0..TILED_GEMM_LDS_EDGES_V1_C_ELEMENTS).collect());
    assert_eq!(a_tail, 1_024 - TILED_GEMM_LDS_EDGES_V1_A_ELEMENTS);
    assert_eq!(b_tail, 1_024 - TILED_GEMM_LDS_EDGES_V1_B_ELEMENTS);
    assert_eq!(c_tail, 1_024 - TILED_GEMM_LDS_EDGES_V1_C_ELEMENTS);
}

#[test]
fn exhaustive_phase_lds_and_barrier_helpers_cover_all_physical_lanes() {
    let mut valid_depths = BTreeSet::new();
    let mut tail_depths = BTreeSet::new();
    for phase in 0..TILED_GEMM_LDS_EDGES_V1_PHASES {
        for tile_depth in 0..TILED_GEMM_LDS_EDGES_V1_PHASE_K {
            let depth = tiled_gemm_lds_edges_v1_depth(phase, tile_depth).unwrap();
            assert_eq!(depth.depth(), phase * 16 + tile_depth);
            match depth {
                TiledGemmLdsEdgesV1Depth::Valid { depth } => {
                    assert!(valid_depths.insert(depth));
                }
                TiledGemmLdsEdgesV1Depth::Tail { depth } => {
                    assert!(tail_depths.insert(depth));
                }
            }
        }
    }
    assert_eq!(valid_depths, (0..18).collect());
    assert_eq!(tail_depths, (18..32).collect());

    let mut lds_cells = BTreeSet::new();
    for lane in 0..TILED_GEMM_LDS_EDGES_V1_LANES {
        for barrier in [
            TiledGemmLdsEdgesV1Barrier::Publish,
            TiledGemmLdsEdgesV1Barrier::Reuse,
        ] {
            for phase in 0..TILED_GEMM_LDS_EDGES_V1_PHASES {
                assert!(tiled_gemm_lds_edges_v1_lane_reaches_barrier(
                    phase, lane, barrier
                ));
            }
        }
        for component in 0..TILED_GEMM_LDS_EDGES_V1_FRAGMENT_ELEMENTS {
            let (row, column) =
                tiled_gemm_lds_edges_v1_staging_coordinate(lane, component).unwrap();
            let physical = tiled_gemm_lds_edges_v1_lds_index(lane, component).unwrap();
            assert_eq!(
                physical,
                row * 16 + (column ^ ((row & 3) * 4)),
                "exact XOR4 address"
            );
            assert!(lds_cells.insert(physical));
        }
    }
    assert_eq!(lds_cells, (0..256).collect());

    assert!(!tiled_gemm_lds_edges_v1_lane_reaches_barrier(
        2,
        0,
        TiledGemmLdsEdgesV1Barrier::Publish
    ));
    assert!(!tiled_gemm_lds_edges_v1_lane_reaches_barrier(
        0,
        64,
        TiledGemmLdsEdgesV1Barrier::Reuse
    ));
}

#[test]
fn helper_domains_fail_closed_outside_the_exact_physical_grid() {
    for (lane, component) in [(64, 0), (0, 4), (64, 4), (u32::MAX, u32::MAX)] {
        assert_eq!(
            tiled_gemm_lds_edges_v1_staging_coordinate(lane, component),
            None
        );
        assert_eq!(
            tiled_gemm_lds_edges_v1_output_coordinate(lane, component),
            None
        );
        assert_eq!(tiled_gemm_lds_edges_v1_lds_index(lane, component), None);
    }
    for (group_x, group_y) in [(2, 0), (0, 2), (2, 2), (u32::MAX, u32::MAX)] {
        assert_eq!(tiled_gemm_lds_edges_v1_tile_origin(group_x, group_y), None);
        assert_eq!(
            tiled_gemm_lds_edges_v1_c_coordinate(group_x, group_y, 0, 0),
            None
        );
    }
    for (phase, depth) in [(2, 0), (0, 16), (2, 16), (u32::MAX, u32::MAX)] {
        assert_eq!(tiled_gemm_lds_edges_v1_depth(phase, depth), None);
    }
    assert_eq!(tiled_gemm_lds_edges_v1_a_coordinate(2, 0, 0, 0), None);
    assert_eq!(tiled_gemm_lds_edges_v1_a_coordinate(0, 2, 0, 0), None);
    assert_eq!(tiled_gemm_lds_edges_v1_b_coordinate(2, 0, 0, 0), None);
    assert_eq!(tiled_gemm_lds_edges_v1_b_coordinate(0, 2, 0, 0), None);
    assert_eq!(tiled_gemm_lds_edges_v1_xor4_index(16, 0), None);
    assert_eq!(tiled_gemm_lds_edges_v1_xor4_index(0, 16), None);
}

#[test]
fn canonical_graph_has_predicated_memory_two_barriers_and_exact_epilogue() {
    let module = tiled_gemm_lds_edges_v1_module();
    verify_module(&module).expect("canonical graph is well-formed Kernel IR");
    verify_tiled_gemm_lds_edges_v1_module(&module, &profile()).expect("canonical Slice 4 graph");

    assert_eq!(module.functions.len(), 1);
    assert_eq!(module.kernels.len(), 1);
    assert_eq!(blocks(&module).len(), 28);
    assert_eq!(
        blocks(&module)
            .iter()
            .map(|block| block.id.0)
            .collect::<Vec<_>>(),
        (0..28).collect::<Vec<_>>()
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
    assert_eq!(
        module.kernels[0].domain,
        LaunchDomain::D2 {
            x: LaunchExtent::Static(128),
            y: LaunchExtent::Static(2),
        }
    );
    assert_eq!(
        module.kernels[0].workgroup_size,
        Some(WorkgroupSize::new(64, 1, 1))
    );

    let operations = blocks(&module)
        .iter()
        .flat_map(|block| &block.operations)
        .collect::<Vec<_>>();
    assert_eq!(
        operations
            .iter()
            .filter(|operation| matches!(operation.kind, OperationKind::WorkgroupMemory(_)))
            .count(),
        2
    );
    assert_eq!(
        operations
            .iter()
            .filter(|operation| matches!(operation.kind, OperationKind::WorkgroupBarrier(_)))
            .count(),
        2
    );
    assert_eq!(
        operations
            .iter()
            .filter(|operation| matches!(operation.kind, OperationKind::Matrix(_)))
            .count(),
        5
    );
    assert_eq!(
        operations
            .iter()
            .filter(|operation| matches!(operation.kind, OperationKind::Load { .. }))
            .count(),
        12
    );
    assert_eq!(
        operations
            .iter()
            .filter(|operation| matches!(operation.kind, OperationKind::Store { .. }))
            .count(),
        4
    );
    assert_eq!(
        blocks(&module)
            .iter()
            .filter(|block| matches!(block.terminator, Some(Terminator::ConditionalBranch { .. })))
            .count(),
        13
    );

    let allocations = block(&module, BlockId(0))
        .operations
        .iter()
        .filter_map(|operation| match &operation.kind {
            OperationKind::WorkgroupMemory(memory) => {
                assert_eq!(
                    memory.extent,
                    WorkgroupMemoryExtent::Static(TILED_GEMM_LDS_EDGES_V1_TILE_ELEMENTS)
                );
                assert_eq!(memory.alignment, TILED_GEMM_LDS_EDGES_V1_LDS_ALIGNMENT);
                Some(operation.results[0].id)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(allocations.len(), 2);
    assert_ne!(allocations[0], allocations[1]);

    for id in [3, 5, 7, 9, 11, 13, 15, 17] {
        let valid = block(&module, BlockId(id));
        assert_eq!(valid.operations.len(), 2);
        assert!(matches!(
            valid.operations[0].kind,
            OperationKind::GetElementPointer { .. }
        ));
        assert!(matches!(
            valid.operations[1].kind,
            OperationKind::Load { .. }
        ));
    }
    let phase = block(&module, BlockId(18));
    let barrier_positions = phase
        .operations
        .iter()
        .enumerate()
        .filter_map(|(index, operation)| {
            matches!(operation.kind, OperationKind::WorkgroupBarrier(_)).then_some(index)
        })
        .collect::<Vec<_>>();
    let matrix_positions = phase
        .operations
        .iter()
        .enumerate()
        .filter_map(|(index, operation)| {
            matches!(operation.kind, OperationKind::Matrix(_)).then_some(index)
        })
        .collect::<Vec<_>>();
    assert_eq!(barrier_positions.len(), 2);
    assert_eq!(matrix_positions.len(), 5);
    assert!(matrix_positions[1] < barrier_positions[0]);
    assert!(barrier_positions[0] < matrix_positions[2]);
    assert!(matrix_positions[4] < barrier_positions[1]);
    for position in matrix_positions {
        let OperationKind::Matrix(matrix) = &phase.operations[position].kind else {
            unreachable!()
        };
        match &matrix.kind {
            MatrixOperationKind::LdsStore { profile, .. }
            | MatrixOperationKind::LdsLoad { profile, .. } => {
                assert_eq!(profile.layout, MatrixLayout::RowMajorXor4);
                assert_eq!(profile.required_elements(), 256);
            }
            MatrixOperationKind::MultiplyAccumulate {
                profile,
                accumulator,
                ..
            } => {
                assert_eq!(*profile, MatrixMultiplyProfile::bf16_f32_m16n16k16_wave64());
                assert_eq!(
                    *accumulator,
                    [
                        block(&module, BlockId(1)).parameters[1].id,
                        block(&module, BlockId(1)).parameters[2].id,
                        block(&module, BlockId(1)).parameters[3].id,
                        block(&module, BlockId(1)).parameters[4].id,
                    ]
                );
            }
            MatrixOperationKind::ScaledMultiplyAccumulate { .. } => {
                unreachable!("the gfx942 BF16 fixture cannot contain a scaled gfx950 MFMA")
            }
        }
    }
    for operation in phase
        .operations
        .iter()
        .filter(|operation| matches!(operation.kind, OperationKind::WorkgroupBarrier(_)))
    {
        let OperationKind::WorkgroupBarrier(barrier) = &operation.kind else {
            unreachable!()
        };
        assert_eq!(barrier.memory_scope, SynchronizationScope::Workgroup);
        assert_eq!(barrier.semantics.ordering, MemoryOrdering::AcquireRelease);
        assert_eq!(
            barrier.semantics.address_spaces,
            BTreeSet::from([AddressSpace::Workgroup])
        );
        assert_eq!(
            barrier.convergence,
            Convergence::uniform(SynchronizationScope::Workgroup)
        );
    }

    for id in [20, 22, 24, 26] {
        let valid = block(&module, BlockId(id));
        assert_eq!(
            valid
                .operations
                .iter()
                .filter(|operation| matches!(operation.kind, OperationKind::Load { .. }))
                .count(),
            1
        );
        assert_eq!(
            valid
                .operations
                .iter()
                .filter(|operation| matches!(operation.kind, OperationKind::Store { .. }))
                .count(),
            1
        );
    }
    assert_eq!(
        constant_result(
            &module,
            Constant::F32Bits(TILED_GEMM_LDS_EDGES_V1_ALPHA_BITS)
        ),
        constant_result(&module, Constant::F32Bits(2.0f32.to_bits()))
    );
    assert_eq!(
        constant_result(
            &module,
            Constant::F32Bits(TILED_GEMM_LDS_EDGES_V1_BETA_BITS)
        ),
        constant_result(&module, Constant::F32Bits((-1.0f32).to_bits()))
    );
}

#[test]
fn every_profile_field_is_closed_to_the_exact_representative() {
    macro_rules! reject_profile {
        ($field:ident, $value:expr) => {{
            let mut mutation = profile();
            mutation.$field = $value;
            assert_eq!(
                verify_tiled_gemm_lds_edges_v1_module(
                    &tiled_gemm_lds_edges_v1_module(),
                    &mutation,
                ),
                Err(TiledGemmLdsEdgesV1Error::UnsupportedProfile)
            );
        }};
    }

    reject_profile!(target, TargetCapability::WaveWidth(WaveWidth::Wave64));
    reject_profile!(code_object_version, 5);
    reject_profile!(m, 16);
    reject_profile!(n, 18);
    reject_profile!(k, 17);
    reject_profile!(alpha_bits, 1.0f32.to_bits());
    reject_profile!(beta_bits, 0.0f32.to_bits());
    reject_profile!(a_elements, 305);
    reject_profile!(b_elements, 341);
    reject_profile!(c_elements, 322);
    reject_profile!(a_bytes, 610);
    reject_profile!(b_bytes, 682);
    reject_profile!(c_bytes, 1_288);
    reject_profile!(tile_rows, 1);
    reject_profile!(tile_columns, 1);
    reject_profile!(depth_tiles, 1);
    reject_profile!(phase_k, 8);
    reject_profile!(workgroup_count, 3);
    reject_profile!(wave_width, WaveWidth::Wave32);
    reject_profile!(launch_extent_x, 64);
    reject_profile!(launch_extent_y, 1);
    reject_profile!(workgroup_size, WorkgroupSize::new(32, 2, 1));
    reject_profile!(lds_allocations, 1);
    reject_profile!(lds_elements_per_allocation, 255);
    reject_profile!(lds_bytes_per_allocation, 1_024);
    reject_profile!(static_lds_bytes, 512);
    reject_profile!(lds_alignment, 8);
    reject_profile!(output_elements_per_lane, 8);

    assert_eq!(profile().lds_layout, MatrixLayout::RowMajorXor4);
    assert!(profile().is_exact());
}

#[test]
fn conditional_barrier_bypass_and_removed_barriers_fail_closed() {
    let canonical = tiled_gemm_lds_edges_v1_module();
    let carried = block(&canonical, BlockId(1)).parameters[1..]
        .iter()
        .map(|parameter| parameter.id)
        .collect::<Vec<_>>();
    let mut bypass = canonical.clone();
    let Some(Terminator::ConditionalBranch {
        else_target,
        else_arguments,
        ..
    }) = &mut block_mut(&mut bypass, BlockId(2)).terminator
    else {
        unreachable!()
    };
    *else_target = BlockId(19);
    *else_arguments = carried;
    assert_valid_but_noncanonical(bypass);

    for ordinal in 0..2 {
        let mut removed = canonical.clone();
        let phase = block_mut(&mut removed, BlockId(18));
        let position = phase
            .operations
            .iter()
            .enumerate()
            .filter_map(|(index, operation)| {
                matches!(operation.kind, OperationKind::WorkgroupBarrier(_)).then_some(index)
            })
            .nth(ordinal)
            .unwrap();
        phase.operations.remove(position);
        assert_valid_but_noncanonical(removed);
    }
}

#[test]
fn unguarded_tail_load_and_c_access_mutations_fail_closed() {
    let canonical = tiled_gemm_lds_edges_v1_module();
    let phase_active = block(&canonical, BlockId(1)).operations[0].results[0].id;
    let mut unguarded_a = canonical.clone();
    let Some(Terminator::ConditionalBranch { condition, .. }) =
        &mut block_mut(&mut unguarded_a, BlockId(2)).terminator
    else {
        unreachable!()
    };
    *condition = phase_active;
    assert_valid_but_noncanonical(unguarded_a);

    let c_condition = match block(&canonical, BlockId(19)).terminator.as_ref().unwrap() {
        Terminator::ConditionalBranch { condition, .. } => *condition,
        _ => unreachable!(),
    };
    let mut unguarded_c = canonical.clone();
    let producer = block_mut(&mut unguarded_c, BlockId(0))
        .operations
        .iter_mut()
        .find(|operation| operation.results.first().map(|result| result.id) == Some(c_condition))
        .unwrap();
    let OperationKind::Binary { lhs, rhs, .. } = &mut producer.kind else {
        unreachable!()
    };
    *lhs = *rhs;
    assert_valid_but_noncanonical(unguarded_c);
}

#[test]
fn phase_tail_accumulator_and_alpha_beta_mutations_fail_closed() {
    let canonical = tiled_gemm_lds_edges_v1_module();
    let phase_id = block(&canonical, BlockId(1)).parameters[0].id;
    let (phase_count, _) = match &block(&canonical, BlockId(1)).operations[0].kind {
        OperationKind::Compare { rhs, .. } => (
            *rhs,
            block(&canonical, BlockId(1)).operations[0].results[0].id,
        ),
        _ => unreachable!(),
    };

    let mut one_phase = canonical.clone();
    let producer = block_mut(&mut one_phase, BlockId(0))
        .operations
        .iter_mut()
        .find(|operation| operation.results.first().map(|result| result.id) == Some(phase_count))
        .unwrap();
    producer.kind = OperationKind::Constant(Constant::Index(1));
    assert_valid_but_noncanonical(one_phase);

    let tile_extent = constant_result(&canonical, Constant::Index(16));
    let k = constant_result(&canonical, Constant::Index(18));
    let mut dropped_k_tail = canonical.clone();
    let compare = block_mut(&mut dropped_k_tail, BlockId(2))
        .operations
        .iter_mut()
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::Compare { rhs, .. } if rhs == k
            )
        })
        .unwrap();
    let OperationKind::Compare { rhs, .. } = &mut compare.kind else {
        unreachable!()
    };
    *rhs = tile_extent;
    assert_valid_but_noncanonical(dropped_k_tail);

    let mut wrong_phase_offset = canonical.clone();
    let offset = block_mut(&mut wrong_phase_offset, BlockId(2))
        .operations
        .iter_mut()
        .find(|operation| {
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
    let OperationKind::Binary { rhs, .. } = &mut offset.kind else {
        unreachable!()
    };
    *rhs = phase_count;
    assert_valid_but_noncanonical(wrong_phase_offset);

    let f32_zero = constant_result(&canonical, Constant::F32Bits(0));
    let mut reset_accumulators = canonical.clone();
    let matrix = block_mut(&mut reset_accumulators, BlockId(18))
        .operations
        .iter_mut()
        .find_map(|operation| match &mut operation.kind {
            OperationKind::Matrix(matrix) => match matrix.kind {
                MatrixOperationKind::MultiplyAccumulate { .. } => Some(matrix),
                _ => None,
            },
            _ => None,
        })
        .unwrap();
    let MatrixOperationKind::MultiplyAccumulate { accumulator, .. } = &mut matrix.kind else {
        unreachable!()
    };
    *accumulator = [f32_zero; 4];
    assert_valid_but_noncanonical(reset_accumulators);

    for (from, to) in [
        (TILED_GEMM_LDS_EDGES_V1_ALPHA_BITS, 1.0f32.to_bits()),
        (TILED_GEMM_LDS_EDGES_V1_BETA_BITS, 0.0f32.to_bits()),
    ] {
        let mut wrong_coefficient = canonical.clone();
        let operation = block_mut(&mut wrong_coefficient, BlockId(0))
            .operations
            .iter_mut()
            .find(|operation| operation.kind == OperationKind::Constant(Constant::F32Bits(from)))
            .unwrap();
        operation.kind = OperationKind::Constant(Constant::F32Bits(to));
        assert_valid_but_noncanonical(wrong_coefficient);
    }
}

#[test]
fn collapsed_ownership_target_resource_and_layout_drift_fail_closed() {
    let canonical = tiled_gemm_lds_edges_v1_module();

    let first_c_index = block(&canonical, BlockId(20))
        .operations
        .iter()
        .find_map(|operation| match operation.kind {
            OperationKind::GetElementPointer { offset, .. } => Some(offset),
            _ => None,
        })
        .unwrap();
    let mut collapsed_c = canonical.clone();
    for operation in &mut block_mut(&mut collapsed_c, BlockId(22)).operations {
        if let OperationKind::GetElementPointer { offset, .. } = &mut operation.kind {
            *offset = first_c_index;
        }
    }
    assert_valid_but_noncanonical(collapsed_c);

    let allocation_ids = block(&canonical, BlockId(0))
        .operations
        .iter()
        .filter_map(|operation| {
            matches!(operation.kind, OperationKind::WorkgroupMemory(_))
                .then_some(operation.results[0].id)
        })
        .collect::<Vec<_>>();
    let mut aliased_lds = canonical.clone();
    for operation in &mut block_mut(&mut aliased_lds, BlockId(18)).operations {
        let OperationKind::Matrix(matrix) = &mut operation.kind else {
            continue;
        };
        match &mut matrix.kind {
            MatrixOperationKind::LdsStore { base, .. }
            | MatrixOperationKind::LdsLoad { base, .. }
                if *base == allocation_ids[1] =>
            {
                *base = allocation_ids[0];
            }
            _ => {}
        }
    }
    assert_valid_but_noncanonical(aliased_lds);

    let target = gfx942_xnack_minus_target_capability();
    let mut target_drift = canonical.clone();
    target_drift.required_capabilities.remove(&target);
    target_drift.functions[0]
        .required_capabilities
        .remove(&target);
    target_drift.kernels[0]
        .required_capabilities
        .remove(&target);
    assert_valid_but_noncanonical(target_drift);

    let mut launch_drift = canonical.clone();
    launch_drift.kernels[0].domain = LaunchDomain::D2 {
        x: LaunchExtent::Static(64),
        y: LaunchExtent::Static(4),
    };
    assert_valid_but_noncanonical(launch_drift);

    let mut resource_drift = canonical.clone();
    let allocation = block_mut(&mut resource_drift, BlockId(0))
        .operations
        .iter_mut()
        .find(|operation| matches!(operation.kind, OperationKind::WorkgroupMemory(_)))
        .unwrap();
    let OperationKind::WorkgroupMemory(memory) = &mut allocation.kind else {
        unreachable!()
    };
    memory.alignment = 32;
    assert_valid_but_noncanonical(resource_drift);

    let mut layout_drift = canonical.clone();
    let matrix = block_mut(&mut layout_drift, BlockId(18))
        .operations
        .iter_mut()
        .find_map(|operation| match &mut operation.kind {
            OperationKind::Matrix(matrix) => Some(matrix),
            _ => None,
        })
        .unwrap();
    match &mut matrix.kind {
        MatrixOperationKind::LdsStore { profile, .. } => profile.columns = 8,
        _ => unreachable!(),
    }
    assert_rejected(layout_drift);
}
