// Included in generated_kfd_invocation::tests alongside the existing admission tests.

use fe2o3_kernel_descriptor::{
    BuildEvidenceV1, DimensionsV1, EvidenceDigest, EvidenceIdentity, KernelAbiLayoutV1,
    LaunchConstraintsV1, ValidName,
};
use rmpv::Value;

fn launch_descriptor(
    rank: u8,
    block: BlockSizeV1,
    max_grid: [u32; 3],
    max_flat: u32,
    static_bytes: u32,
    max_dynamic_bytes: u32,
) -> KernelDescriptorV1 {
    let evidence = BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes([1; 32]),
        EvidenceDigest::from_sha256_bytes([2; 32]),
    );
    KernelDescriptorV1::new(
        crate::KernelId::from_bytes([3; 32]),
        ValidName::new("launch_test").unwrap(),
        ValidName::new("launch_test").unwrap(),
        ValidName::new("launch_test.kd").unwrap(),
        evidence,
        evidence,
        vec![],
        KernelAbiLayoutV1::new(0, 256, 8).unwrap(),
        LaunchConstraintsV1::new(
            rank,
            block,
            DimensionsV1::new(max_grid[0], max_grid[1], max_grid[2]).unwrap(),
            max_flat,
            static_bytes,
            max_dynamic_bytes,
        )
        .unwrap(),
        vec![],
    )
    .unwrap()
}

fn exact_block(x: u32) -> BlockSizeV1 {
    BlockSizeV1::Exact(DimensionsV1::new(x, 1, 1).unwrap())
}

fn inspected_launch_kernel(overrides: &[(&str, Option<Value>)]) -> InspectedKernel {
    const HSACO: &[u8] =
        include_bytes!("../../fe2o3-runtime/fixtures/trusted-gfx942-vecadd-v1/vecadd.hsaco");
    let inspected = fe2o3_hsaco::inspect(HSACO).unwrap();
    if overrides.is_empty() {
        return inspected.kernels()[0].clone();
    }
    let range = inspected.metadata_descriptor_range();
    let start = usize::try_from(range.file_offset()).unwrap();
    let end = start + usize::try_from(range.byte_len()).unwrap();
    let mut metadata = rmpv::decode::read_value(&mut &HSACO[start..end]).unwrap();
    let Value::Map(root) = &mut metadata else {
        panic!("metadata map");
    };
    let (_, Value::Array(kernels)) = root
        .iter_mut()
        .find(|(key, _)| key.as_str() == Some("amdhsa.kernels"))
        .unwrap()
    else {
        panic!("kernel array");
    };
    let Value::Map(kernel) = &mut kernels[0] else {
        panic!("kernel map");
    };
    for (name, value) in overrides {
        kernel.retain(|(key, _)| key.as_str() != Some(*name));
        if let Some(value) = value {
            kernel.push((Value::from(*name), value.clone()));
        }
    }
    let mut encoded = Vec::new();
    rmpv::encode::write_value(&mut encoded, &metadata).unwrap();
    let mut note = Vec::new();
    note.extend_from_slice(&7_u32.to_le_bytes());
    note.extend_from_slice(&u32::try_from(encoded.len()).unwrap().to_le_bytes());
    note.extend_from_slice(&32_u32.to_le_bytes());
    note.extend_from_slice(b"AMDGPU\0\0");
    note.extend_from_slice(&encoded);
    note.resize(note.len().next_multiple_of(4), 0);

    // Metadata-only ELF: it can be inspected but has no executable image or authority.
    let mut bytes = vec![0; 120];
    bytes[..64].copy_from_slice(&HSACO[..64]);
    bytes[32..40].copy_from_slice(&64_u64.to_le_bytes());
    bytes[40..48].fill(0);
    bytes[54..56].copy_from_slice(&56_u16.to_le_bytes());
    bytes[56..58].copy_from_slice(&1_u16.to_le_bytes());
    bytes[60..64].fill(0);
    bytes[64..68].copy_from_slice(&4_u32.to_le_bytes());
    bytes[68..72].copy_from_slice(&4_u32.to_le_bytes());
    bytes[72..80].copy_from_slice(&120_u64.to_le_bytes());
    bytes[96..104].copy_from_slice(&(note.len() as u64).to_le_bytes());
    bytes[104..112].copy_from_slice(&(note.len() as u64).to_le_bytes());
    bytes[112..120].copy_from_slice(&4_u64.to_le_bytes());
    bytes.extend_from_slice(&note);
    fe2o3_hsaco::inspect(&bytes).unwrap().kernels()[0].clone()
}

#[test]
fn generated_kfd_launch_checks_descriptor_counts_in_aql_global_units() {
    let physical = inspected_launch_kernel(&[]);
    let descriptor = launch_descriptor(1, exact_block(256), [2, 1, 1], 256, 0, 0);
    for extent in [256, 257, 511, 512] {
        validate_generated_kfd_launch_prerequisites(
            &descriptor,
            &physical,
            AqlDispatchGeometryV1::new([extent, 1, 1], [256, 1, 1]).unwrap(),
            0,
        )
        .unwrap();
    }
    assert!(matches!(
        validate_generated_kfd_launch_prerequisites(
            &descriptor,
            &physical,
            AqlDispatchGeometryV1::new([513, 1, 1], [256, 1, 1]).unwrap(),
            0,
        ),
        Err(Gfx942RuntimePreparationErrorV1::WorkgroupCountExceeded { axis: 0 })
    ));
    let descriptor = launch_descriptor(1, exact_block(256), [u32::MAX, 1, 1], 256, 0, 0);
    validate_generated_kfd_launch_prerequisites(
        &descriptor,
        &physical,
        AqlDispatchGeometryV1::new([u32::MAX, 1, 1], [256, 1, 1]).unwrap(),
        0,
    )
    .unwrap();
}

#[test]
fn generated_kfd_launch_checks_rank_and_exact_workgroup() {
    let physical = inspected_launch_kernel(&[]);
    let descriptor = launch_descriptor(1, exact_block(256), [2, 1, 1], 256, 0, 0);
    assert!(matches!(
        validate_generated_kfd_launch_prerequisites(
            &descriptor,
            &physical,
            AqlDispatchGeometryV1::new([256, 2, 1], [256, 1, 1]).unwrap(),
            0,
        ),
        Err(Gfx942RuntimePreparationErrorV1::UnsupportedResource(
            "launch rank exceeds descriptor"
        ))
    ));
    assert!(matches!(
        validate_generated_kfd_launch_prerequisites(
            &descriptor,
            &physical,
            AqlDispatchGeometryV1::new([256, 1, 1], [64, 1, 1]).unwrap(),
            0,
        ),
        Err(Gfx942RuntimePreparationErrorV1::WorkgroupMismatch)
    ));
    let unknown = inspected_launch_kernel(&[(".reqd_workgroup_size", None)]);
    assert!(matches!(
        validate_generated_kfd_launch_prerequisites(
            &descriptor,
            &unknown,
            AqlDispatchGeometryV1::new([256, 1, 1], [256, 1, 1]).unwrap(),
            0,
        ),
        Err(Gfx942RuntimePreparationErrorV1::WorkgroupMismatch)
    ));
}

#[test]
fn generated_kfd_launch_checks_at_most_flat_limits_and_grid_overflow() {
    let physical = inspected_launch_kernel(&[(".reqd_workgroup_size", None)]);
    let at_most = launch_descriptor(
        1,
        BlockSizeV1::AtMost(DimensionsV1::new(128, 1, 1).unwrap()),
        [u32::MAX, 1, 1],
        256,
        0,
        0,
    );
    validate_generated_kfd_launch_prerequisites(
        &at_most,
        &physical,
        AqlDispatchGeometryV1::new([256, 1, 1], [128, 1, 1]).unwrap(),
        0,
    )
    .unwrap();
    for (descriptor, threads) in [
        (at_most, 256),
        (
            launch_descriptor(1, BlockSizeV1::Any, [1, 1, 1], 128, 0, 0),
            256,
        ),
        (
            launch_descriptor(1, BlockSizeV1::Any, [1, 1, 1], 512, 0, 0),
            512,
        ),
    ] {
        assert!(matches!(
            validate_generated_kfd_launch_prerequisites(
                &descriptor,
                &physical,
                AqlDispatchGeometryV1::new([threads, 1, 1], [threads, 1, 1]).unwrap(),
                0,
            ),
            Err(Gfx942RuntimePreparationErrorV1::WorkgroupMismatch)
        ));
    }
    let descriptor = launch_descriptor(3, BlockSizeV1::Any, [1; 3], 256, 0, 0);
    assert!(matches!(
        validate_generated_kfd_launch_prerequisites(
            &descriptor,
            &physical,
            AqlDispatchGeometryV1::new([u32::MAX; 3], [1; 3]).unwrap(),
            0,
        ),
        Err(Gfx942RuntimePreparationErrorV1::UnsupportedResource(
            "global invocation count overflow"
        ))
    ));
}

#[test]
fn generated_kfd_launch_checks_physical_workgroup_counts_on_each_axis() {
    for (axis, key) in [
        (0, ".max_num_workgroups_x"),
        (1, ".max_num_workgroups_y"),
        (2, ".max_num_workgroups_z"),
    ] {
        let physical = inspected_launch_kernel(&[(key, Some(Value::from(2)))]);
        let descriptor = launch_descriptor(3, exact_block(256), [4; 3], 256, 0, 0);
        let mut grid = [256, 1, 1];
        grid[axis] = if axis == 0 { 512 } else { 2 };
        validate_generated_kfd_launch_prerequisites(
            &descriptor,
            &physical,
            AqlDispatchGeometryV1::new(grid, [256, 1, 1]).unwrap(),
            0,
        )
        .unwrap();
        grid[axis] += 1;
        assert!(matches!(
            validate_generated_kfd_launch_prerequisites(
                &descriptor,
                &physical,
                AqlDispatchGeometryV1::new(grid, [256, 1, 1]).unwrap(),
                0,
            ),
            Err(Gfx942RuntimePreparationErrorV1::WorkgroupCountExceeded { axis: actual })
                if actual == axis
        ));
    }
}

#[test]
fn generated_kfd_launch_checks_static_and_dynamic_lds_for_a_second_workgroup() {
    let physical = inspected_launch_kernel(&[
        (
            ".reqd_workgroup_size",
            Some(Value::Array(vec![64.into(), 1.into(), 1.into()])),
        ),
        (".max_flat_workgroup_size", Some(Value::from(64))),
        (".group_segment_fixed_size", Some(Value::from(256))),
    ]);
    let geometry = AqlDispatchGeometryV1::new([65, 1, 1], [64, 1, 1]).unwrap();
    let descriptor = launch_descriptor(1, exact_block(64), [2, 1, 1], 64, 256, 128);
    for dynamic in [0, 128] {
        validate_generated_kfd_launch_prerequisites(&descriptor, &physical, geometry, dynamic)
            .unwrap();
    }
    assert!(matches!(
        validate_generated_kfd_launch_prerequisites(&descriptor, &physical, geometry, 129),
        Err(Gfx942RuntimePreparationErrorV1::UnsupportedResource(
            "dynamic group segment exceeds descriptor"
        ))
    ));
    let mismatched_static = launch_descriptor(1, exact_block(64), [2, 1, 1], 64, 0, 128);
    assert!(matches!(
        validate_generated_kfd_launch_prerequisites(&mismatched_static, &physical, geometry, 0),
        Err(Gfx942RuntimePreparationErrorV1::UnsupportedResource(
            "static group segment differs from descriptor"
        ))
    ));
    let no_dynamic = launch_descriptor(1, exact_block(64), [2, 1, 1], 64, 256, 0);
    assert!(matches!(
        validate_generated_kfd_launch_prerequisites(&no_dynamic, &physical, geometry, 1),
        Err(Gfx942RuntimePreparationErrorV1::UnsupportedResource(
            "dynamic group segment exceeds descriptor"
        ))
    ));
}
