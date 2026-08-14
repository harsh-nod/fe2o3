use std::collections::BTreeSet;

use fe2o3_kernel_ir::*;

fn profile() -> TiledGemmV1Profile {
    TiledGemmV1Profile::exact_gfx942_xnack_minus_cov6()
}

#[test]
fn exact_lane_maps_cover_all_fragments() {
    let mut a_indices = BTreeSet::new();
    let mut b_indices = BTreeSet::new();
    let mut cd_indices = BTreeSet::new();

    for lane in 0..64 {
        for component in 0..4 {
            let expected_a = (lane % 16, 4 * (lane / 16) + component);
            let expected_b = (4 * (lane / 16) + component, lane % 16);
            assert_eq!(
                tiled_gemm_v1_a_coordinate(lane, component),
                Some(expected_a)
            );
            assert_eq!(
                tiled_gemm_v1_b_coordinate(lane, component),
                Some(expected_b)
            );
            assert_eq!(
                tiled_gemm_v1_cd_coordinate(lane, component),
                Some(expected_b)
            );

            let a = tiled_gemm_v1_a_index(lane, component).unwrap();
            let b = tiled_gemm_v1_b_index(lane, component).unwrap();
            let cd = tiled_gemm_v1_cd_index(lane, component).unwrap();
            assert_eq!(a, expected_a.0 * 16 + expected_a.1);
            assert_eq!(b, expected_b.0 * 16 + expected_b.1);
            assert_eq!(cd, expected_b.0 * 16 + expected_b.1);
            assert!(a_indices.insert(a));
            assert!(b_indices.insert(b));
            assert!(cd_indices.insert(cd));
        }
    }

    let all_indices = (0..256).collect::<BTreeSet<_>>();
    assert_eq!(a_indices, all_indices);
    assert_eq!(b_indices, all_indices);
    assert_eq!(cd_indices, all_indices);

    for (lane, component) in [(64, 0), (0, 4), (u32::MAX, u32::MAX)] {
        assert_eq!(tiled_gemm_v1_a_coordinate(lane, component), None);
        assert_eq!(tiled_gemm_v1_b_coordinate(lane, component), None);
        assert_eq!(tiled_gemm_v1_cd_coordinate(lane, component), None);
        assert_eq!(tiled_gemm_v1_a_index(lane, component), None);
        assert_eq!(tiled_gemm_v1_b_index(lane, component), None);
        assert_eq!(tiled_gemm_v1_cd_index(lane, component), None);
    }
}

#[test]
fn each_output_has_one_lane_component_owner() {
    let mut owners = vec![None; 256];
    for lane in 0..64 {
        for component in 0..4 {
            let index = tiled_gemm_v1_cd_index(lane, component).unwrap() as usize;
            assert_eq!(owners[index].replace((lane, component)), None);
        }
    }
    assert!(owners.into_iter().all(|owner| owner.is_some()));
}

#[test]
fn canonical_graph_has_exact_types_operations_and_launch() {
    let module = tiled_gemm_v1_module();
    verify_tiled_gemm_v1_module(&module, &profile()).expect("canonical tiled GEMM");

    assert_eq!(module.functions.len(), 1);
    assert_eq!(module.kernels.len(), 1);
    assert_eq!(module.functions[0].id.as_str(), TILED_GEMM_V1_FUNCTION_ID);
    assert_eq!(module.kernels[0].id.as_str(), TILED_GEMM_V1_KERNEL_ID);
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
            Type::slice(Type::F32, AddressSpace::Global, AccessMode::ReadOnly),
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

    let operations = &module.functions[0].body.as_ref().unwrap().blocks[0].operations;
    assert_eq!(operations.len(), 64);
    let loads = operations
        .iter()
        .filter(|operation| matches!(operation.kind, OperationKind::Load { .. }))
        .collect::<Vec<_>>();
    let stores = operations
        .iter()
        .filter(|operation| matches!(operation.kind, OperationKind::Store { .. }))
        .collect::<Vec<_>>();
    let matrices = operations
        .iter()
        .filter(|operation| matches!(operation.kind, OperationKind::Matrix(_)))
        .collect::<Vec<_>>();
    assert_eq!(loads.len(), 12);
    assert_eq!(stores.len(), 4);
    assert_eq!(matrices.len(), 1);
    assert_eq!(
        loads
            .iter()
            .map(|operation| operation.results[0].ty.clone())
            .collect::<Vec<_>>(),
        [
            vec![Type::Scalar(ScalarType::Bf16); 2],
            vec![Type::F32],
            vec![Type::Scalar(ScalarType::Bf16); 2],
            vec![Type::F32],
            vec![Type::Scalar(ScalarType::Bf16); 2],
            vec![Type::F32],
            vec![Type::Scalar(ScalarType::Bf16); 2],
            vec![Type::F32],
        ]
        .concat()
    );
    assert!(stores.iter().all(|store| store.results.is_empty()));
    assert_eq!(matrices[0].results.len(), 4);
    assert!(
        matrices[0]
            .results
            .iter()
            .all(|result| result.ty == Type::F32)
    );
    let matrix_results = matrices[0]
        .results
        .iter()
        .map(|result| result.id)
        .collect::<Vec<_>>();
    let stored_values = stores
        .iter()
        .map(|store| match store.kind {
            OperationKind::Store { value, .. } => value,
            _ => unreachable!(),
        })
        .collect::<Vec<_>>();
    assert_eq!(stored_values, matrix_results);
    assert!(!operations.iter().any(|operation| {
        matches!(
            operation.kind,
            OperationKind::WorkgroupMemory(_)
                | OperationKind::WorkgroupBarrier(_)
                | OperationKind::Barrier(_)
        )
    }));

    let effects = operations
        .iter()
        .flat_map(Operation::memory_effects)
        .collect::<Vec<_>>();
    assert_eq!(
        effects
            .iter()
            .filter(|effect| **effect == MemoryEffect::Read(AddressSpace::Global))
            .count(),
        12
    );
    assert_eq!(
        effects
            .iter()
            .filter(|effect| **effect == MemoryEffect::Write(AddressSpace::Global))
            .count(),
        4
    );
}

#[test]
fn bridge_and_all_four_buffer_shapes_are_explicit() {
    let profile = profile();
    assert_eq!(profile.a_elements, 256);
    assert_eq!(profile.b_elements, 256);
    assert_eq!(profile.c_elements, 256);
    assert_eq!(profile.d_elements, 256);
    assert_eq!(profile.bf16_bridge.rust_physical_scalar, ScalarType::U16);
    assert_eq!(
        profile.bf16_bridge.kernel_ir_semantic_scalar,
        ScalarType::Bf16
    );
    assert_eq!(profile.bf16_bridge.bit_width, 16);
    assert!(profile.bf16_bridge.bit_preserving);
}

#[test]
fn malformed_profiles_fail_closed() {
    let mut profiles = Vec::new();

    let mut target = profile();
    target.target = TargetCapability::WaveWidth(WaveWidth::Wave64);
    profiles.push(target);

    let mut cov = profile();
    cov.code_object_version = 5;
    profiles.push(cov);

    let mut m = profile();
    m.m = 32;
    profiles.push(m);

    let mut n = profile();
    n.n = 8;
    profiles.push(n);

    let mut k = profile();
    k.k = 0;
    profiles.push(k);

    for field in 0..4 {
        let mut shape = profile();
        match field {
            0 => shape.a_elements = 255,
            1 => shape.b_elements = 257,
            2 => shape.c_elements = 0,
            3 => shape.d_elements = 512,
            _ => unreachable!(),
        }
        profiles.push(shape);
    }

    let mut tile_rows = profile();
    tile_rows.tile_rows = 2;
    profiles.push(tile_rows);

    let mut tile_columns = profile();
    tile_columns.tile_columns = 2;
    profiles.push(tile_columns);

    let mut depth_tiles = profile();
    depth_tiles.depth_tiles = 2;
    profiles.push(depth_tiles);

    let mut wave = profile();
    wave.wave_width = WaveWidth::Wave32;
    profiles.push(wave);

    let mut launch = profile();
    launch.workgroup_size = WorkgroupSize::new(32, 1, 1);
    profiles.push(launch);

    let mut extent = profile();
    extent.launch_extent_x = 128;
    profiles.push(extent);

    let mut bridge_type = profile();
    bridge_type.bf16_bridge.rust_physical_scalar = ScalarType::Bf16;
    profiles.push(bridge_type);

    let mut bridge_semantic = profile();
    bridge_semantic.bf16_bridge.kernel_ir_semantic_scalar = ScalarType::U16;
    profiles.push(bridge_semantic);

    let mut bridge_width = profile();
    bridge_width.bf16_bridge.bit_width = 32;
    profiles.push(bridge_width);

    let mut bridge_encoding = profile();
    bridge_encoding.bf16_bridge.bit_preserving = false;
    profiles.push(bridge_encoding);

    for malformed in profiles {
        assert_eq!(
            verify_tiled_gemm_v1_module(&tiled_gemm_v1_module(), &malformed),
            Err(TiledGemmV1Error::UnsupportedProfile)
        );
    }
}

#[test]
fn graph_mutations_cannot_change_store_ownership_or_matrix_profile() {
    let mut aliased_store = tiled_gemm_v1_module();
    let operations = &mut aliased_store.functions[0].body.as_mut().unwrap().blocks[0].operations;
    let store_indices = operations
        .iter()
        .enumerate()
        .filter_map(|(index, operation)| {
            matches!(operation.kind, OperationKind::Store { .. }).then_some(index)
        })
        .collect::<Vec<_>>();
    let first_pointer = match operations[store_indices[0]].kind {
        OperationKind::Store { pointer, .. } => pointer,
        _ => unreachable!(),
    };
    let OperationKind::Store { pointer, .. } = &mut operations[store_indices[1]].kind else {
        unreachable!()
    };
    *pointer = first_pointer;
    assert_eq!(
        verify_tiled_gemm_v1_module(&aliased_store, &profile()),
        Err(TiledGemmV1Error::NonCanonicalKernelIr)
    );

    let mut wrong_matrix = tiled_gemm_v1_module();
    let matrix = wrong_matrix.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .iter_mut()
        .find_map(|operation| match &mut operation.kind {
            OperationKind::Matrix(matrix) => Some(matrix),
            _ => None,
        })
        .unwrap();
    let MatrixOperationKind::MultiplyAccumulate {
        profile: matrix_profile,
        ..
    } = &mut matrix.kind
    else {
        unreachable!()
    };
    matrix_profile.k = 32;
    assert!(matches!(
        verify_tiled_gemm_v1_module(&wrong_matrix, &profile()),
        Err(TiledGemmV1Error::InvalidKernelIr(_))
    ));
}
