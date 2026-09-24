//! Pure exact-output mutation controls; no ioctl, queue or mapping.
use super::*;

fn expected() -> KfdIoctlCreateQueueArgs {
    KfdIoctlCreateQueueArgs {
        ring_base_address: 0x10000,
        write_pointer_address: 0x900038,
        read_pointer_address: 0x900080,
        doorbell_offset: u64::MAX,
        ring_size: RING_BYTES as u32,
        gpu_id: 7,
        queue_type: KFD_IOC_QUEUE_TYPE_COMPUTE_AQL,
        queue_percentage: 100,
        queue_priority: 0,
        queue_id: u32::MAX,
        eop_buffer_address: 0xa00000,
        eop_buffer_size: 4096,
        ctx_save_restore_address: 0xb00000,
        ctx_save_restore_size: CONTEXT_BYTES_PER_XCC as u32,
        ctl_stack_size: CONTROL_STACK_BYTES,
        sdma_engine_id: 0,
        pad: 0,
    }
}
fn returned() -> KfdIoctlCreateQueueArgs {
    use fe2o3_kfd_uapi::{KFD_MMAP_GPU_ID_HASH_SHIFT, KFD_MMAP_TYPE_DOORBELL, KFD_MMAP_TYPE_SHIFT};
    KfdIoctlCreateQueueArgs {
        queue_id: 0,
        doorbell_offset: (KFD_MMAP_TYPE_DOORBELL << KFD_MMAP_TYPE_SHIFT)
            | (7 << KFD_MMAP_GPU_ID_HASH_SHIFT),
        ..expected()
    }
}
#[test]
fn zero_real_queue_id_is_allowed_but_no_other_input_is_reinterpreted() {
    checked_create_output(expected(), returned()).unwrap();
}
#[test]
fn every_immutable_create_field_is_closed() {
    let mutations: [fn(&mut KfdIoctlCreateQueueArgs); 15] = [
        |a| a.ring_base_address += 4096,
        |a| a.write_pointer_address += 8,
        |a| a.read_pointer_address += 8,
        |a| a.ring_size /= 2,
        |a| a.gpu_id += 1,
        |a| a.queue_type += 1,
        |a| a.queue_percentage -= 1,
        |a| a.queue_priority += 1,
        |a| a.eop_buffer_address += 4096,
        |a| a.eop_buffer_size += 4096,
        |a| a.ctx_save_restore_address += 4096,
        |a| a.ctx_save_restore_size += 4096,
        |a| a.ctl_stack_size += 4096,
        |a| a.sdma_engine_id = 1,
        |a| a.pad = 1,
    ];
    for mutate in mutations {
        let mut changed = returned();
        mutate(&mut changed);
        assert!(checked_create_output(expected(), changed).is_err());
    }
}
#[test]
fn unknown_queue_id_or_foreign_unaligned_doorbell_refuses() {
    for changed in [
        KfdIoctlCreateQueueArgs {
            queue_id: u32::MAX,
            ..returned()
        },
        KfdIoctlCreateQueueArgs {
            doorbell_offset: u64::MAX,
            ..returned()
        },
        KfdIoctlCreateQueueArgs {
            doorbell_offset: returned().doorbell_offset | 1,
            ..returned()
        },
        KfdIoctlCreateQueueArgs {
            doorbell_offset: 0,
            ..returned()
        },
    ] {
        assert!(checked_create_output(expected(), changed).is_err());
    }
}
#[test]
fn original_fixed_queue_arithmetic_has_no_new_output_allocation() {
    let queue = RING_BYTES + CWSR_BYTES + 3 * PAGE_BYTES + 65536;
    assert_eq!(queue, 190_296_064);
    assert_eq!(RING_BYTES, 8_388_608);
    assert_eq!(CWSR_BYTES, 181_829_632);
    assert_eq!(CONTEXT_BYTES_PER_XCC, 22_687_744);
    assert_eq!(CONTROL_STACK_BYTES, 0x3000);
}

#[test]
fn all_eight_actual_header_fields_join_payload_event_and_shared_debug_region() {
    let payload = 0x800100_u64;
    let event = 7_u32;
    for xcc in 0..XCC_COUNT {
        let mut h = [0_u8; 24];
        h[..4].copy_from_slice(&(((XCC_COUNT - xcc) * CONTEXT_BYTES_PER_XCC) as u32).to_le_bytes());
        h[4..8].copy_from_slice(&DEBUG_BYTES_TOTAL.to_le_bytes());
        h[8..16].copy_from_slice(&payload.to_le_bytes());
        h[16..20].copy_from_slice(&event.to_le_bytes());
        assert!(cwsr_header_matches(xcc, &h, payload, event));
        for byte in 0..24 {
            let mut changed = h;
            changed[byte] ^= 1;
            assert!(!cwsr_header_matches(xcc, &changed, payload, event));
        }
        assert!(!cwsr_header_matches(xcc, &h, payload + 8, event));
        assert!(!cwsr_header_matches(xcc, &h, payload, event + 1));
        assert!(!cwsr_header_matches(xcc, &h[..23], payload, event));
        assert!(!cwsr_header_matches(XCC_COUNT, &h, payload, event));
    }
}
