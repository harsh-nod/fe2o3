//! CPU-only preparation of captured compiler output, not Worker admission or GPU execution.
use fe2o3_aql::AqlDispatchGeometryV1;
use fe2o3_kfd::Gfx942KfdDispatchPointerFixupV1;
use fe2o3_runtime::{
    Gfx942RuntimeBufferAccessV1, Gfx942RuntimeDispatchBufferV1, Gfx942RuntimeDispatchInputsV1,
    Gfx942RuntimePreparationErrorV1, prepare_gfx942_runtime_dispatch_v1,
};

const FILL: &[u8] =
    include_bytes!("../../fe2o3-hsaco/tests/fixtures/rust-fill-write-only-gfx942/kernel.hsaco");

fn inputs(length: u64, explicit_size: usize, dynamic_lds: u32) -> Gfx942RuntimeDispatchInputsV1 {
    let mut explicit = vec![0; explicit_size];
    if let Some(bytes) = explicit.get_mut(8..16) {
        bytes.copy_from_slice(&length.to_le_bytes());
    }
    Gfx942RuntimeDispatchInputsV1::new(
        explicit,
        vec![
            Gfx942RuntimeDispatchBufferV1::new(
                vec![0xa5; usize::try_from(length * 4).unwrap()],
                Gfx942RuntimeBufferAccessV1::WriteOnly,
            )
            .unwrap(),
        ],
        vec![Gfx942KfdDispatchPointerFixupV1::new(0, 0, 0, 4)],
        AqlDispatchGeometryV1::new(
            [u32::try_from(length.div_ceil(64) * 64).unwrap(), 1, 1],
            [64, 1, 1],
        )
        .unwrap(),
        dynamic_lds,
        5_000,
    )
}

#[test]
fn captured_fill_preserves_exact_explicit_only_kernarg_and_pointer_fixup() {
    for length in [64, 65, 4097] {
        let prepared =
            prepare_gfx942_runtime_dispatch_v1(FILL, "fill_write_only", inputs(length, 16, 0))
                .unwrap();
        assert_eq!(prepared.kernel_name(), "fill_write_only");
        assert_eq!(prepared.packet_group_segment_bytes(), 0);
        let request = prepared.into_unchecked_kfd_request().into_parts_v1();
        assert_eq!(request.kernarg_template.len(), 16);
        assert_eq!(&request.kernarg_template[..8], &[0; 8]);
        assert_eq!(&request.kernarg_template[8..], &length.to_le_bytes());
        assert_eq!(
            request.pointer_fixups,
            vec![Gfx942KfdDispatchPointerFixupV1::new(0, 0, 0, 4)]
        );
        assert_eq!(request.buffers.len(), 1);
    }
}

#[test]
fn captured_fill_rejects_truncated_or_phantom_kernarg_suffix() {
    for size in [0, 8, 15, 17, 272] {
        assert!(
            matches!(
                prepare_gfx942_runtime_dispatch_v1(FILL, "fill_write_only", inputs(64, size, 0)),
                Err(Gfx942RuntimePreparationErrorV1::KernargLayout)
            ),
            "accepted explicit size {size}"
        );
    }
}

#[test]
fn captured_fill_rejects_dynamic_lds_without_a_hidden_field() {
    assert!(matches!(
        prepare_gfx942_runtime_dispatch_v1(FILL, "fill_write_only", inputs(64, 16, 4)),
        Err(Gfx942RuntimePreparationErrorV1::HiddenArgument {
            detail: "dynamic LDS requested without ABI field",
            ..
        })
    ));
}

#[test]
fn captured_fill_projects_to_the_persistent_packet_without_native_work() {
    for length in [64, 65, 4097] {
        let prepared =
            prepare_gfx942_runtime_dispatch_v1(FILL, "fill_write_only", inputs(length, 16, 0))
                .unwrap();
        let identity = prepared.identity();
        let contract = prepared.dispatch_contract_sha256();
        let projected = prepared.into_persistent_projection_v1(FILL).unwrap();
        assert_eq!(projected.identity(), identity);
        assert_eq!(projected.dispatch_contract_sha256(), contract);
        assert_eq!(
            projected.packet().geometry().grid(),
            [length.div_ceil(64) as u32 * 64, 1, 1]
        );
        assert_eq!(projected.packet().buffer_count(), 1);
        assert_eq!(
            projected.buffer_access(0),
            Some(Gfx942RuntimeBufferAccessV1::WriteOnly)
        );
        assert_eq!(
            projected.buffers()[0].bytes(),
            vec![0xa5; length as usize * 4]
        );
        assert_eq!(
            projected.pointer_fixups(),
            &[Gfx942KfdDispatchPointerFixupV1::new(0, 0, 0, 4)]
        );
        assert_eq!(projected.timeout_milliseconds(), 5_000);
        let storage = projected.into_generated_storage_v1();
        assert_eq!(storage.dispatch_contract_sha256(), contract);
        assert_eq!(storage.identity(), identity);
        assert_eq!(
            storage.buffer_access(0),
            Some(Gfx942RuntimeBufferAccessV1::WriteOnly)
        );
    }
}

#[test]
fn explicit_only_profile_keeps_workgroup_and_fixup_checks() {
    for wrong_workgroup in [true, false] {
        let result = prepare_gfx942_runtime_dispatch_v1(
            FILL,
            "fill_write_only",
            Gfx942RuntimeDispatchInputsV1::new(
                vec![0; 16],
                vec![
                    Gfx942RuntimeDispatchBufferV1::new(
                        vec![0xa5; 256],
                        Gfx942RuntimeBufferAccessV1::WriteOnly,
                    )
                    .unwrap(),
                ],
                vec![Gfx942KfdDispatchPointerFixupV1::new(
                    if wrong_workgroup { 0 } else { 16 },
                    0,
                    0,
                    4,
                )],
                AqlDispatchGeometryV1::new(
                    [64, 1, 1],
                    [if wrong_workgroup { 32 } else { 64 }, 1, 1],
                )
                .unwrap(),
                0,
                5_000,
            ),
        );
        if wrong_workgroup {
            assert!(matches!(
                result,
                Err(Gfx942RuntimePreparationErrorV1::WorkgroupMismatch)
            ));
        } else {
            assert!(matches!(
                result,
                Err(Gfx942RuntimePreparationErrorV1::KfdRequest(_))
            ));
        }
    }
}
