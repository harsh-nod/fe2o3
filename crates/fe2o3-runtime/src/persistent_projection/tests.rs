use super::*;

use crate::synthetic_cov6;

fn prepared(hsaco: &[u8], count: usize, offset: usize) -> PreparedGfx942RuntimeDispatchV1 {
    let mut explicit = vec![0; 16];
    explicit[8..].copy_from_slice(&4u64.to_le_bytes());
    prepare_gfx942_runtime_dispatch_v1(
        hsaco,
        "vecadd",
        Gfx942RuntimeDispatchInputsV1::new(
            explicit,
            (0..count)
                .map(|index| {
                    Gfx942RuntimeDispatchBufferV1::new(
                        vec![index as u8 + 1; 32 + index * 4],
                        Gfx942RuntimeBufferAccessV1::ReadWrite,
                    )
                    .unwrap()
                })
                .collect(),
            vec![Gfx942KfdDispatchPointerFixupV1::new(0, 0, offset, 4)],
            AqlDispatchGeometryV1::new([130, 2, 1], [64, 2, 1]).unwrap(),
            0,
            4321,
        ),
    )
    .unwrap()
}

#[test]
fn persistent_projection_retains_complete_roster_identity_timeout_and_allocations() {
    let hsaco = synthetic_cov6::preparation_module();
    let mut source = prepared(&hsaco, 3, 8);
    let identity = source.identity();
    let digest = source.dispatch_contract_sha256();
    let parts = source.request.into_parts_v1();
    let image = (
        parts.executable_image.as_ptr(),
        parts.executable_image.capacity(),
    );
    let buffers = (parts.buffers.as_ptr(), parts.buffers.capacity());
    let pointers = parts
        .buffers
        .iter()
        .map(|buffer| buffer.bytes().as_ptr())
        .collect::<Vec<_>>();
    let fixups = (
        parts.pointer_fixups.as_ptr(),
        parts.pointer_fixups.capacity(),
    );
    let policies = (
        source.buffer_policies.as_ptr(),
        source.buffer_policies.capacity(),
    );
    source.request = rebuild(parts);
    let projection = source.into_persistent_projection_v1(&hsaco).unwrap();
    assert_eq!(projection.identity(), identity);
    assert_eq!(projection.dispatch_contract_sha256(), digest);
    assert_eq!(projection.timeout_milliseconds(), 4321);
    assert_eq!(projection.buffers.len(), 3);
    for (index, buffer) in projection.buffers.iter().enumerate() {
        assert_eq!(buffer.bytes(), vec![index as u8 + 1; 32 + index * 4]);
    }
    assert_eq!(
        projection.pointer_fixups[0],
        Gfx942KfdDispatchPointerFixupV1::new(0, 0, 8, 4)
    );
    assert_eq!(projection.packet.buffer_count(), 1);
    assert_eq!(
        projection.packet.geometry(),
        AqlDispatchGeometryV1::new([130, 2, 1], [64, 2, 1]).unwrap()
    );
    assert_eq!(projection.kernarg_alignment(), 16);
    assert_eq!(
        (
            projection.executable_image.as_ptr(),
            projection.executable_image.capacity()
        ),
        image
    );
    assert_eq!(
        (projection.buffers.as_ptr(), projection.buffers.capacity()),
        buffers
    );
    assert_eq!(
        projection
            .buffers
            .iter()
            .map(|buffer| buffer.bytes().as_ptr())
            .collect::<Vec<_>>(),
        pointers
    );
    assert_eq!(
        (
            projection.pointer_fixups.as_ptr(),
            projection.pointer_fixups.capacity()
        ),
        fixups
    );
    assert_eq!(
        (
            projection.buffer_policies.as_ptr(),
            projection.buffer_policies.capacity()
        ),
        policies
    );
}

fn rebuild(parts: Gfx942KfdDispatchRequestPartsV1) -> Gfx942KfdDispatchRequestV1 {
    Gfx942KfdDispatchRequestV1::new(
        parts.executable_image,
        parts.descriptor_offset,
        parts.kernarg_template,
        parts.kernarg_alignment,
        parts.buffers,
        parts.pointer_fixups,
        parts.geometry,
        parts.private_segment_size,
        parts.group_segment_size,
        parts.timeout_milliseconds,
    )
    .unwrap()
}

#[test]
fn persistent_projection_rejects_each_substituted_request_coordinate() {
    let hsaco = synthetic_cov6::preparation_module();
    for axis in 0..13 {
        let mut source = prepared(&hsaco, 2, 0);
        let mut parts = source.request.into_parts_v1();
        match axis {
            0 => parts.executable_image[0] ^= 1,
            1 => parts.descriptor_offset += 64,
            2 => parts.kernarg_template[8] ^= 1,
            3 => parts.kernarg_template[16] ^= 1,
            4 => parts.kernarg_template[16 + 200] ^= 1,
            5 => parts.kernarg_alignment = 32,
            6 => parts.buffers[1] = Gfx942KfdDispatchBufferV1::new(vec![9; 36]).unwrap(),
            7 => parts.pointer_fixups[0] = Gfx942KfdDispatchPointerFixupV1::new(0, 1, 0, 4),
            8 => parts.pointer_fixups[0] = Gfx942KfdDispatchPointerFixupV1::new(0, 0, 4, 4),
            9 => parts.pointer_fixups[0] = Gfx942KfdDispatchPointerFixupV1::new(0, 0, 0, 8),
            10 => parts.geometry = AqlDispatchGeometryV1::new([128, 2, 1], [64, 2, 1]).unwrap(),
            11 => parts.group_segment_size = 4,
            _ => parts.timeout_milliseconds += 1,
        }
        source.request = rebuild(parts);
        assert!(
            source.into_persistent_projection_v1(&hsaco).is_err(),
            "axis {axis}"
        );
    }
}

fn native_packet(
    parts: Gfx942KfdDispatchRequestPartsV1,
    dynamic: u32,
) -> Gfx942FixedDispatchPacketV1 {
    Gfx942FixedDispatchPacketV1::new(
        0,
        parts.geometry,
        dynamic,
        parts.kernarg_template.into_boxed_slice(),
        vec![Gfx942DispatchBufferBindingV1::new(0, 0, 0, 32)].into_boxed_slice(),
    )
}

#[test]
fn persistent_projection_native_policy_rejects_every_mutated_hidden_byte_and_invalid_unused_layout()
{
    let hsaco = synthetic_cov6::preparation_module();
    let kernel = validate(&hsaco, AdmittedProfile::Gfx942XnackOffCov6)
        .unwrap()
        .bind_kernel("vecadd")
        .unwrap()
        .reconcile_dispatch_abi(
            [1; 32],
            &[KernelGlobalBufferAbiV1::new(
                0,
                "a_ptr",
                0,
                4,
                ArgumentAccess::ReadWrite,
            )],
        )
        .unwrap();
    assert!(
        project_gfx942_fixed_host_packet_v1(
            &kernel,
            native_packet(prepared(&hsaco, 2, 0).request.into_parts_v1(), 0),
            &[32, 36]
        )
        .is_ok()
    );
    for offset in 0..256 {
        let mut parts = prepared(&hsaco, 2, 0).request.into_parts_v1();
        parts.kernarg_template[16 + offset] ^= 1;
        assert!(
            project_gfx942_fixed_host_packet_v1(&kernel, native_packet(parts, 0), &[32, 36])
                .is_err(),
            "byte {offset}"
        );
    }
    for invalid in [0, usize::MAX, 1usize << 40] {
        assert!(matches!(
            project_gfx942_fixed_host_packet_v1(
                &kernel,
                native_packet(prepared(&hsaco, 2, 0).request.into_parts_v1(), 0),
                &[32, invalid]
            ),
            Err(Gfx942DispatchBindingErrorV1::InvalidData { index: 1, .. })
        ));
    }
}

#[test]
fn persistent_projection_preserves_dynamic_lds_and_nondefault_timeout() {
    let hsaco = synthetic_cov6::dynamic_preparation_module();
    let source = prepare_gfx942_runtime_dispatch_v1(
        &hsaco,
        "vecadd",
        Gfx942RuntimeDispatchInputsV1::new(
            vec![0; 16],
            vec![
                Gfx942RuntimeDispatchBufferV1::new(
                    vec![3; 32],
                    Gfx942RuntimeBufferAccessV1::ReadWrite,
                )
                .unwrap(),
            ],
            vec![Gfx942KfdDispatchPointerFixupV1::new(0, 0, 0, 4)],
            AqlDispatchGeometryV1::new([130, 1, 1], [64, 1, 1]).unwrap(),
            1024,
            60_000,
        ),
    )
    .unwrap();
    let projected = source.into_persistent_projection_v1(&hsaco).unwrap();
    assert_eq!(projected.packet.dynamic_group_segment_bytes(), 1024);
    assert_eq!(projected.description.packet_group_segment_bytes, 1024);
    assert_eq!(projected.timeout_milliseconds, 60_000);
}

#[test]
fn persistent_projection_preserves_reordered_fixups_and_rejects_alias_or_missing_buffer_shapes() {
    let hsaco = synthetic_cov6::three_binding_preparation_module();
    for mode in 0..4 {
        let fixups = match mode {
            0 => vec![
                Gfx942KfdDispatchPointerFixupV1::new(16, 2, 4, 4),
                Gfx942KfdDispatchPointerFixupV1::new(0, 0, 8, 4),
                Gfx942KfdDispatchPointerFixupV1::new(8, 1, 0, 4),
            ],
            1 => vec![
                Gfx942KfdDispatchPointerFixupV1::new(0, 0, 0, 4),
                Gfx942KfdDispatchPointerFixupV1::new(8, 0, 8, 4),
                Gfx942KfdDispatchPointerFixupV1::new(16, 2, 0, 4),
            ],
            2 => vec![Gfx942KfdDispatchPointerFixupV1::new(0, 0, 0, 4)],
            _ => vec![],
        };
        let expected = fixups.clone();
        let buffers = if mode == 3 {
            vec![]
        } else {
            (0..3)
                .map(|_| {
                    Gfx942RuntimeDispatchBufferV1::new(
                        vec![3; 32],
                        Gfx942RuntimeBufferAccessV1::ReadWrite,
                    )
                    .unwrap()
                })
                .collect()
        };
        let source = prepare_gfx942_runtime_dispatch_v1(
            &hsaco,
            "vecadd",
            Gfx942RuntimeDispatchInputsV1::new(
                vec![0; 32],
                buffers,
                fixups,
                AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
                0,
                4321,
            ),
        )
        .unwrap();
        let projected = source.into_persistent_projection_v1(&hsaco);
        if mode == 0 {
            assert_eq!(projected.unwrap().pointer_fixups, expected);
        } else {
            assert!(projected.is_err(), "mode {mode}");
        }
    }
}

#[test]
fn persistent_projection_checks_canonical_image_data_and_zero_ranges_without_allocation() {
    let hsaco = synthetic_cov6::preparation_module();
    let kernel = validate(&hsaco, AdmittedProfile::Gfx942XnackOffCov6)
        .unwrap()
        .bind_kernel("vecadd")
        .unwrap();
    let parts = prepared(&hsaco, 1, 0).request.into_parts_v1();
    let mut image = parts.executable_image;
    let plan = kernel.envelope().materialization();
    assert!(image_matches_materialization(&image, plan));
    for index in (0..image.len()).step_by(257).chain([image.len() - 1]) {
        image[index] ^= 1;
        assert!(!image_matches_materialization(&image, plan), "byte {index}");
        image[index] ^= 1;
    }
    assert!(!image_matches_materialization(
        &image[..image.len() - 1],
        plan
    ));
    let source = prepared(&hsaco, 1, 0);
    let mut substituted = hsaco.clone();
    *substituted.last_mut().unwrap() ^= 1;
    assert!(source.into_persistent_projection_v1(&substituted).is_err());
}

#[test]
fn persistent_projection_enforces_actual_fixed_roster_limit_without_truncation() {
    let hsaco = synthetic_cov6::preparation_module();
    assert_eq!(
        prepared(&hsaco, 16, 0)
            .into_persistent_projection_v1(&hsaco)
            .unwrap()
            .buffers
            .len(),
        16
    );
    assert!(matches!(
        prepared(&hsaco, 17, 0).into_persistent_projection_v1(&hsaco),
        Err(Gfx942RuntimeProjectionErrorV1::FixedDispatch(_))
    ));
}

#[test]
fn persistent_projection_rejects_substituted_description_and_final_policy() {
    let hsaco = synthetic_cov6::preparation_module();
    for axis in 0..9 {
        let mut value = prepared(&hsaco, 2, 0);
        match axis {
            0 => value.description.finalized_hsaco_length += 1,
            1 => value.description.dispatch_contract_sha256[0] ^= 1,
            2 => value.description.descriptor_offset += 64,
            3 => value.description.static_group_segment_bytes += 4,
            4 => value.description.dynamic_group_segment_bytes += 4,
            5 => value.description.packet_group_segment_bytes += 4,
            6 => value.buffer_policies[1].byte_length -= 1,
            7 => value.buffer_policies[1].access = Gfx942RuntimeBufferAccessV1::ReadOnly,
            _ => {
                value.buffer_policies.pop();
            }
        }
        assert!(
            value.into_persistent_projection_v1(&hsaco).is_err(),
            "axis {axis}"
        );
    }
}
