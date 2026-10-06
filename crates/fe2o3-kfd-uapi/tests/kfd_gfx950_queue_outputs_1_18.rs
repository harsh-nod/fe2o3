use fe2o3_kfd_uapi::{
    KFD_GFX942_QUEUE_RESOURCE_SCHEMA_MANIFEST, KFD_GFX942_QUEUE_RESOURCE_SCHEMA_MANIFEST_SHA256,
    KFD_GFX950_DOORBELL_BYTES_V1, KFD_GFX950_PROCESS_DOORBELL_SLICE_BYTES_V1,
    KFD_GFX950_PROCESS_QUEUE_SLOTS_V1, KFD_GFX950_QUEUE_OUTPUT_PROFILE_MANIFEST_V1,
    KFD_GFX950_QUEUE_OUTPUT_PROFILE_SHA256_V1, KfdGfx942CreateQueueOutputError,
    KfdGfx950CreateQueueOutputErrorV1 as Error, admit_kfd_gfx942_create_queue_outputs,
    observe_kfd_gfx950_create_queue_outputs_v1 as observe,
};
use sha2::{Digest, Sha256};
use std::fmt::Write;

const fn raw(gpu: u32, offset: u64) -> u64 {
    (3_u64 << 62) | (((gpu & 0xffff) as u64) << 46) | offset
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut value, byte| {
        write!(value, "{byte:02x}").unwrap();
        value
    })
}

#[test]
fn gfx950_output_profile_is_independent_and_gfx942_manifest_is_unchanged() {
    assert_eq!(
        hex(&Sha256::digest(KFD_GFX950_QUEUE_OUTPUT_PROFILE_MANIFEST_V1)),
        KFD_GFX950_QUEUE_OUTPUT_PROFILE_SHA256_V1
    );
    assert_ne!(
        KFD_GFX950_QUEUE_OUTPUT_PROFILE_SHA256_V1,
        KFD_GFX942_QUEUE_RESOURCE_SCHEMA_MANIFEST_SHA256
    );
    assert_eq!(
        KFD_GFX942_QUEUE_RESOURCE_SCHEMA_MANIFEST_SHA256,
        "8ff2ac20f6001d6f5405423d78e8ad6cec109ac3370fe86d7691c5c4782c1803"
    );
    assert_eq!(
        hex(&Sha256::digest(KFD_GFX942_QUEUE_RESOURCE_SCHEMA_MANIFEST)),
        KFD_GFX942_QUEUE_RESOURCE_SCHEMA_MANIFEST_SHA256
    );
    for premise in [
        "target=gfx950:90500,gc:9.5.0,pci:0x75a0,SPX/NPS1\n",
        "module_srcversion=703B1127E578BC5D4BD6615\n",
        "module_parameters=mes:0,sched_policy:0,cwsr_enable:1\n",
        "geometry_profile_sha256=e838edc8d388cf4339ef9427175f4074ec2944af19394dd559bcbcc729fc0cbc\n",
        "queue_type=compute-aql:2,sdma-and-mes-not-admitted\n",
    ] {
        assert!(KFD_GFX950_QUEUE_OUTPUT_PROFILE_MANIFEST_V1.contains(premise));
    }
    assert_eq!(KFD_GFX950_PROCESS_QUEUE_SLOTS_V1, 1024);
    assert_eq!(KFD_GFX950_DOORBELL_BYTES_V1, 8);
    assert_eq!(KFD_GFX950_PROCESS_DOORBELL_SLICE_BYTES_V1, 8192);
}

#[test]
fn gfx950_all_queue_slots_and_doorbell_slots_are_independent() {
    for queue in 0..1024 {
        let offset = u64::from(1023 - queue) * 8;
        let output = observe(queue, raw(11429, offset), 11429).unwrap();
        assert_eq!(output.queue_id().value(), queue);
        assert_eq!(output.gpu_id(), 11429);
        assert_eq!(
            output.profile_sha256(),
            KFD_GFX950_QUEUE_OUTPUT_PROFILE_SHA256_V1
        );
        let doorbell = output.doorbell_offset();
        assert_eq!(doorbell.raw(), raw(11429, offset));
        assert_eq!(doorbell.in_process_byte_offset(), offset);
        assert_eq!(doorbell.encoded_process_slice_offset(), raw(11429, 0));
    }
}

#[test]
fn gfx950_offsets_reject_every_misalignment_and_range_boundary() {
    for offset in 0..16384 {
        let observed = observe(0, raw(73, offset), 73);
        if offset >= 8192 {
            assert_eq!(observed, Err(Error::DoorbellOffsetOutOfRange { offset }));
        } else if offset % 8 != 0 {
            assert_eq!(observed, Err(Error::DoorbellOffsetMisaligned { offset }));
        } else {
            assert_eq!(
                observed.unwrap().doorbell_offset().in_process_byte_offset(),
                offset
            );
        }
    }
    for bit in 13..46 {
        let offset = 1_u64 << bit;
        assert_eq!(
            observe(0, raw(73, offset), 73),
            Err(Error::DoorbellOffsetOutOfRange { offset })
        );
    }
    let offset = (1_u64 << 46) - 1;
    assert_eq!(
        observe(0, raw(73, offset), 73),
        Err(Error::DoorbellOffsetOutOfRange { offset })
    );
}

#[test]
fn gfx950_bad_queue_type_and_gpu_hash_have_exact_errors() {
    for queue_id in [1024, 1025, u32::MAX] {
        assert_eq!(
            observe(queue_id, raw(73, 0), 73),
            Err(Error::QueueIdOutOfRange { queue_id })
        );
    }
    for observed in 0..3 {
        assert_eq!(
            observe(0, (observed << 62) | (73 << 46), 73),
            Err(Error::DoorbellMmapType { observed })
        );
    }
    for observed in 0..=u16::MAX {
        if observed == 73 {
            continue;
        }
        assert_eq!(
            observe(0, raw(u32::from(observed), 0), 73),
            Err(Error::DoorbellGpuIdHash {
                expected: 73,
                observed
            })
        );
    }
}

#[test]
fn gfx950_full_gpu_id_is_retained_without_claiming_hash_authentication() {
    let low = observe(0, raw(73, 0), 73).unwrap();
    let high = observe(0, raw(73, 0), 0xffff_0049).unwrap();
    assert_eq!(low.doorbell_offset(), high.doorbell_offset());
    assert_ne!(low, high);
    assert_eq!(high.gpu_id(), 0xffff_0049);
}

#[test]
fn gfx942_shared_decoder_preserves_admission_and_error_precedence() {
    for queue in [0, 1, 1023, 1024, u32::MAX] {
        for offset in [0, 7, 8, 4096, 8184, 8191, 8192, (1 << 46) - 1] {
            for gpu in [0, 73, 0xffff, u32::MAX] {
                for encoded_gpu in [gpu, gpu ^ 1] {
                    for kind in 0..4 {
                        let encoded = (raw(encoded_gpu, offset) & !(3_u64 << 62)) | (kind << 62);
                        let observed = admit_kfd_gfx942_create_queue_outputs(queue, encoded, gpu);
                        let expected_error = if queue >= 1024 {
                            Some(KfdGfx942CreateQueueOutputError::QueueIdOutOfRange {
                                queue_id: queue,
                            })
                        } else if kind != 3 {
                            Some(KfdGfx942CreateQueueOutputError::DoorbellMmapType {
                                observed: kind,
                            })
                        } else if encoded_gpu & 0xffff != gpu & 0xffff {
                            Some(KfdGfx942CreateQueueOutputError::DoorbellGpuIdHash {
                                expected: gpu as u16,
                                observed: encoded_gpu as u16,
                            })
                        } else if offset >= 8192 {
                            Some(KfdGfx942CreateQueueOutputError::DoorbellOffsetOutOfRange {
                                offset,
                            })
                        } else if offset % 8 != 0 {
                            Some(KfdGfx942CreateQueueOutputError::DoorbellOffsetMisaligned {
                                offset,
                            })
                        } else {
                            None
                        };
                        assert_eq!(observed.err(), expected_error);
                        if let Ok(value) = observed {
                            assert_eq!(value.queue_id().value(), queue);
                            assert_eq!(
                                value.doorbell_offset().encoded_process_slice_offset(),
                                raw(gpu, 0)
                            );
                            assert_eq!(value.doorbell_offset().in_process_byte_offset(), offset);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn gfx950_outputs_match_source_macro_c_oracle_record() {
    let mut record = format!(
        "slots={} width={} slice={}\n",
        KFD_GFX950_PROCESS_QUEUE_SLOTS_V1,
        KFD_GFX950_DOORBELL_BYTES_V1,
        KFD_GFX950_PROCESS_DOORBELL_SLICE_BYTES_V1
    );
    for (gpu, queue, offset) in [
        (11429, 0, 8184),
        (11429, 1023, 0),
        (73, 1, 4096),
        (0xffff_0049, 7, 24),
    ] {
        let output = observe(queue, raw(gpu, offset), gpu).unwrap();
        let db = output.doorbell_offset();
        writeln!(
            record,
            "gpu={gpu:08x} queue={queue} raw={:016x} mmap={:016x} offset={}",
            db.raw(),
            db.encoded_process_slice_offset(),
            db.in_process_byte_offset()
        )
        .unwrap();
    }
    assert_eq!(
        record,
        include_str!("oracles/kfd_gfx950_queue_outputs_1_18.txt")
    );
}
