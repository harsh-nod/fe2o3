use super::{module, module_with_resources};
use crate::*;

const PARTIAL_GRIDS: [[u32; 3]; 4] = [[17, 8, 4], [16, 9, 4], [16, 8, 5], [17, 9, 5]];
const WORKGROUP: [u32; 3] = [8, 4, 2];

fn inputs(geometry: AqlDispatchGeometryV1) -> Gfx942RuntimeDispatchInputsV1 {
    let mut explicit = vec![0; 16];
    explicit[8..16].copy_from_slice(&2_u64.to_le_bytes());
    Gfx942RuntimeDispatchInputsV1::new(
        explicit,
        vec![
            Gfx942RuntimeDispatchBufferV1::new(
                vec![0xa5; 8],
                Gfx942RuntimeBufferAccessV1::ReadOnly,
            )
            .unwrap(),
        ],
        vec![Gfx942KfdDispatchPointerFixupV1::new(0, 0, 0, 4)],
        geometry,
        0,
        1_000,
    )
}

fn assert_prepared_contract(
    hsaco: &[u8],
    geometry: AqlDispatchGeometryV1,
    prepared: &PreparedGfx942RuntimeDispatchV1,
) {
    let expected_inputs = inputs(geometry);
    let closure = validate(hsaco, AdmittedProfile::Gfx942XnackOffCov6)
        .unwrap()
        .bind_kernel("vecadd")
        .unwrap();
    let mut image =
        vec![0; usize::try_from(closure.envelope().materialization().image_len()).unwrap()];
    closure.materialize_into(&mut image).unwrap();
    let descriptor_offset =
        closure.selected_binding().descriptor_address() - closure.envelope().plan().image_start();

    // Reconstruct the fixture's COV6 bytes independently of hidden_value/shape helpers.
    let mut kernarg = vec![0; 272];
    kernarg[..16].copy_from_slice(&expected_inputs.explicit_kernarg);
    let grid = geometry.grid();
    for (axis, width) in geometry.workgroup().into_iter().enumerate() {
        let count = grid[axis] / u32::from(width);
        let remainder = u16::try_from(grid[axis] % u32::from(width)).unwrap();
        let count_offset = 16 + axis * 4;
        kernarg[count_offset..count_offset + 4].copy_from_slice(&count.to_le_bytes());
        let size_offset = 28 + axis * 2;
        kernarg[size_offset..size_offset + 2].copy_from_slice(&width.to_le_bytes());
        let remainder_offset = 34 + axis * 2;
        kernarg[remainder_offset..remainder_offset + 2].copy_from_slice(&remainder.to_le_bytes());
    }
    kernarg[80..82].copy_from_slice(&geometry.dimensions().to_le_bytes());
    let expected = derive_dispatch_contract_sha256_v1(
        u64::try_from(hsaco.len()).unwrap(),
        closure.identity_inputs().into(),
        "vecadd",
        &image,
        descriptor_offset,
        &kernarg,
        16,
        &expected_inputs.buffers,
        &expected_inputs.pointer_fixups,
        geometry,
        0,
        1_000,
    );
    assert_eq!(prepared.dispatch_contract_sha256(), expected);
    assert_eq!(prepared.geometry(), geometry);
    assert_eq!(prepared.kernel_name(), "vecadd");
    assert_eq!(prepared.finalized_hsaco_length(), hsaco.len() as u64);
    assert_eq!(prepared.descriptor_offset(), descriptor_offset);
    assert_eq!(prepared.static_group_segment_bytes(), 0);
    assert_eq!(prepared.dynamic_group_segment_bytes(), 0);
    assert_eq!(prepared.packet_group_segment_bytes(), 0);
}

#[test]
fn original_synthetic_fixture_bytes_and_scratch_refusal_are_preserved() {
    let hsaco = module();
    assert_eq!(hsaco.len(), 17_344);
    // Captured by executing the unmodified fixture at public 413ba987.
    let expected = [
        0x70, 0x35, 0x25, 0x8b, 0xf3, 0x1d, 0xbd, 0xa9, 0x46, 0xba, 0xf6, 0x2a, 0xb6, 0x4b, 0x93,
        0x41, 0xc8, 0xd3, 0xb2, 0x13, 0xa6, 0x0a, 0xc8, 0xaf, 0xa6, 0xb1, 0x4e, 0x10, 0xb3, 0x10,
        0x23, 0x00,
    ];
    let actual: [u8; 32] = Sha256::digest(&hsaco).into();
    assert_eq!(actual, expected);
    let geometry = AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap();
    assert!(matches!(
        prepare_gfx942_runtime_dispatch_v1(&hsaco, "vecadd", inputs(geometry)),
        Err(Gfx942RuntimePreparationErrorV1::UnsupportedResource(
            "private segment scratch"
        ))
    ));
}

#[test]
fn fixture_preserves_uniform_declaration_and_resource_identity() {
    for uniform in [None, Some(false), Some(true)] {
        for private_bytes in [0, 16] {
            let hsaco = module_with_resources(private_bytes, uniform);
            let closure = validate(&hsaco, AdmittedProfile::Gfx942XnackOffCov6)
                .unwrap()
                .bind_kernel("vecadd")
                .unwrap();
            assert_eq!(
                closure
                    .selected_kernel()
                    .uniform_work_group_size_declaration(),
                uniform
            );
            assert_eq!(
                closure.resources().private_segment_fixed_size(),
                u64::from(private_bytes)
            );
        }
    }
}

#[test]
fn preparation_accepts_complete_workgroups_in_each_dimension() {
    let cases = [
        ([1, 1, 1], [1, 1, 1]),
        ([32, 1, 1], [32, 1, 1]),
        ([64, 1, 1], [32, 1, 1]),
        ([32, 2, 1], [32, 2, 1]),
        ([64, 4, 1], [32, 2, 1]),
        ([8, 4, 2], WORKGROUP),
        ([16, 8, 4], WORKGROUP),
    ];
    for uniform in [None, Some(false), Some(true)] {
        let hsaco = module_with_resources(0, uniform);
        for (grid, workgroup) in cases {
            let geometry = AqlDispatchGeometryV1::new(grid, workgroup).unwrap();
            let prepared =
                prepare_gfx942_runtime_dispatch_v1(&hsaco, "vecadd", inputs(geometry)).unwrap();
            assert_prepared_contract(&hsaco, geometry, &prepared);
        }
    }
}

#[test]
fn uniform_requirement_rejects_remainders_on_every_axis() {
    let hsaco = module_with_resources(0, Some(true));
    for grid in PARTIAL_GRIDS {
        let geometry = AqlDispatchGeometryV1::new(grid, WORKGROUP).unwrap();
        let result = prepare_gfx942_runtime_dispatch_v1(&hsaco, "vecadd", inputs(geometry));
        assert!(
            matches!(
                result,
                Err(Gfx942RuntimePreparationErrorV1::WorkgroupMismatch)
            ),
            "grid {grid:?}: {result:?}"
        );
    }
}

#[test]
fn optional_uniform_requirement_preserves_partial_group_kernargs() {
    for uniform in [None, Some(false)] {
        let hsaco = module_with_resources(0, uniform);
        for grid in PARTIAL_GRIDS {
            let geometry = AqlDispatchGeometryV1::new(grid, WORKGROUP).unwrap();
            let prepared =
                prepare_gfx942_runtime_dispatch_v1(&hsaco, "vecadd", inputs(geometry)).unwrap();
            assert_prepared_contract(&hsaco, geometry, &prepared);
        }
    }
}

#[test]
fn resource_errors_precede_uniform_and_kernarg_checks() {
    let hsaco = module_with_resources(16, Some(true));
    for grid in [[16, 8, 4], PARTIAL_GRIDS[0]] {
        let geometry = AqlDispatchGeometryV1::new(grid, WORKGROUP).unwrap();
        let mut bad_inputs = inputs(geometry);
        bad_inputs.explicit_kernarg.pop();
        assert!(matches!(
            prepare_gfx942_runtime_dispatch_v1(&hsaco, "vecadd", bad_inputs),
            Err(Gfx942RuntimePreparationErrorV1::UnsupportedResource(
                "private segment scratch"
            ))
        ));
    }
}

#[test]
fn uniform_errors_precede_kernarg_materialization() {
    for uniform in [None, Some(false), Some(true)] {
        let hsaco = module_with_resources(0, uniform);
        for grid in [[16, 8, 4], PARTIAL_GRIDS[0]] {
            let geometry = AqlDispatchGeometryV1::new(grid, WORKGROUP).unwrap();
            let mut bad_inputs = inputs(geometry);
            bad_inputs.explicit_kernarg.pop();
            let result = prepare_gfx942_runtime_dispatch_v1(&hsaco, "vecadd", bad_inputs);
            if uniform == Some(true) && grid == PARTIAL_GRIDS[0] {
                assert!(matches!(
                    result,
                    Err(Gfx942RuntimePreparationErrorV1::WorkgroupMismatch)
                ));
            } else {
                assert!(matches!(
                    result,
                    Err(Gfx942RuntimePreparationErrorV1::KernargLayout)
                ));
            }
        }
    }
}

#[test]
fn invalid_geometry_remains_a_constructor_refusal() {
    for axis in 0..3 {
        let mut grid = WORKGROUP;
        grid[axis] -= 1;
        assert_eq!(
            AqlDispatchGeometryV1::new(grid, WORKGROUP),
            Err(fe2o3_aql::AqlGeometryError::GridSmallerThanWorkgroup)
        );
        grid[axis] = 0;
        assert_eq!(
            AqlDispatchGeometryV1::new(grid, WORKGROUP),
            Err(fe2o3_aql::AqlGeometryError::ZeroGrid)
        );
    }
}
