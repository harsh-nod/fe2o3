//! Synthetic storage/transaction controls only. These tests do not create a
//! Kernel, admit a GPU artifact, register a runtime, or exercise a trap handler.

use super::*;
use std::sync::Mutex;

pub(super) static NOTIFICATION_TEST_LOCK: Mutex<()> = Mutex::new(());

fn prepared() -> MetadataStorageV1 {
    MetadataStorageV1::prepare(b"synthetic ELF storage, not loader-admitted", -4096).unwrap()
}

fn addresses(value: &MetadataStorageV1) -> [usize; 4] {
    [
        &value.record[0].root as *const _ as usize,
        &value.record[0].link as *const _ as usize,
        value.record[0].uri.as_ptr() as usize,
        value.original_elf.as_ptr() as usize,
    ]
}

fn finish_synthetic(value: &mut MetadataStorageV1) {
    // Exercise the same all-pointee selection used by Drop, but reclaim the
    // synthetic allocations here because no native observer ever received them.
    drop(value.take_storage_to_retain());
}

#[test]
fn preparation_owns_exact_bytes_and_real_internal_uri_without_notification() {
    let _lock = NOTIFICATION_TEST_LOCK.lock().unwrap();
    let before = NOTIFICATION_SIDE_EFFECT.load(Ordering::Relaxed);
    let value = prepared();
    let item = &value.record[0];
    assert!(value.is_prepared());
    assert_eq!(
        value.retained_elf(),
        b"synthetic ELF storage, not loader-admitted"
    );
    assert_eq!(item.root.version, 0);
    assert_eq!(item.root.map, 0);
    assert_eq!(item.root.state, abi::RT_CONSISTENT_V1);
    assert_eq!(item.root.reserved0, 0);
    assert_eq!(item.root.reserved1, 0);
    assert_eq!(item.root.loader_base, 0);
    assert_eq!(
        item.root.breakpoint,
        fe2o3_runtime_debug_state_v1 as *const () as usize as u64
    );
    assert_ne!(item.root.breakpoint, 0);
    let uri = format!(
        "memory://{}#offset=0x{:x}&size={}",
        std::process::id(),
        value.original_elf.as_ptr() as usize,
        value.original_elf.len()
    );
    assert_eq!(&item.uri[..uri.len()], uri.as_bytes());
    assert!(item.uri[uri.len()..].iter().all(|byte| *byte == 0));
    assert_eq!(item.link.name, item.uri.as_ptr() as usize as u64);
    assert_eq!(item.link.load_bias, (-4096_i64) as u64);
    assert_eq!(
        [item.link.dynamic, item.link.next, item.link.previous],
        [0; 3]
    );
    assert_eq!(
        value.prepared_bytes(),
        core::mem::size_of::<Record>() + value.retained_elf().len()
    );
    assert_eq!(NOTIFICATION_SIDE_EFFECT.load(Ordering::Relaxed), before);
}

#[test]
fn owner_moves_keep_all_native_pointees_stable() {
    let value = prepared();
    let expected = addresses(&value);
    let mut owners = vec![value];
    for _ in 0..32 {
        owners.push(prepared());
    }
    let value = Box::new(owners.remove(0));
    assert_eq!(addresses(&value), expected);
    assert_eq!(value.record[0].link.name, expected[2] as u64);
}

#[test]
fn preparation_cannot_publish_or_retire() {
    let _lock = NOTIFICATION_TEST_LOCK.lock().unwrap();
    let before = NOTIFICATION_SIDE_EFFECT.load(Ordering::Relaxed);
    let mut value = prepared();
    assert_eq!(value.publish_link(), Err(MetadataErrorV1::Transition));
    assert_eq!(value.retire_link(), Err(MetadataErrorV1::Transition));
    assert!(value.is_prepared());
    assert_eq!(NOTIFICATION_SIDE_EFFECT.load(Ordering::Relaxed), before);
}

#[test]
fn synthetic_add_then_delete_publishes_exact_two_phase_roster() {
    let mut value = prepared();
    let expected = addresses(&value);
    value.activate_for_test();
    let mut observed = Vec::new();
    value
        .transition(true, |row| {
            observed.push(row);
            Ok(())
        })
        .unwrap();
    assert_eq!(
        observed,
        [
            TransitionSnapshotV1 {
                state: abi::RT_ADD_V1,
                linked: false
            },
            TransitionSnapshotV1 {
                state: abi::RT_CONSISTENT_V1,
                linked: true
            },
        ]
    );
    assert_eq!(value.phase, Phase::ActivePresent);
    assert_eq!(value.record[0].root.map, expected[1] as u64);
    observed.clear();
    value
        .transition(false, |row| {
            observed.push(row);
            Ok(())
        })
        .unwrap();
    assert_eq!(
        observed,
        [
            TransitionSnapshotV1 {
                state: abi::RT_DELETE_V1,
                linked: true
            },
            TransitionSnapshotV1 {
                state: abi::RT_CONSISTENT_V1,
                linked: false
            },
        ]
    );
    assert_eq!(addresses(&value), expected);
    assert_eq!(value.phase, Phase::ActiveAbsent);
    assert_eq!(value.record[0].root.map, 0);
    value.acknowledge_detached_for_test();
}

#[test]
fn real_rendezvous_is_called_only_by_synthetic_active_transitions() {
    let _lock = NOTIFICATION_TEST_LOCK.lock().unwrap();
    let mut value = prepared();
    let before = NOTIFICATION_SIDE_EFFECT.load(Ordering::Relaxed);
    value.activate_for_test();
    assert_eq!(NOTIFICATION_SIDE_EFFECT.load(Ordering::Relaxed), before);
    value.publish_link().unwrap();
    assert_eq!(
        NOTIFICATION_SIDE_EFFECT.load(Ordering::Relaxed),
        before.wrapping_add(2)
    );
    value.retire_link().unwrap();
    assert_eq!(
        NOTIFICATION_SIDE_EFFECT.load(Ordering::Relaxed),
        before.wrapping_add(4)
    );
    value.acknowledge_detached_for_test();
}

#[test]
fn repeated_invalid_transition_does_not_notify_or_mutate_and_readd_works() {
    let mut value = prepared();
    value.activate_for_test();
    assert_eq!(
        value.transition(false, |_| panic!("unexpected notification")),
        Err(MetadataErrorV1::Transition)
    );
    value.transition(true, |_| Ok(())).unwrap();
    let before = value.snapshot();
    assert_eq!(
        value.transition(true, |_| panic!("unexpected notification")),
        Err(MetadataErrorV1::Transition)
    );
    assert_eq!(value.snapshot(), before);
    value.transition(false, |_| Ok(())).unwrap();
    value.transition(true, |_| Ok(())).unwrap();
    value.transition(false, |_| Ok(())).unwrap();
    value.acknowledge_detached_for_test();
}

#[test]
fn each_add_notification_failure_poison_retains_exact_progress() {
    for failure_at in 1..=2 {
        let mut value = prepared();
        value.activate_for_test();
        let mut count = 0;
        assert_eq!(
            value.transition(true, |_| {
                count += 1;
                if count == failure_at {
                    Err(MetadataErrorV1::Notification)
                } else {
                    Ok(())
                }
            }),
            Err(MetadataErrorV1::Notification)
        );
        assert_eq!(value.phase, Phase::Poisoned);
        assert_eq!(count, failure_at);
        assert_eq!(
            value.snapshot(),
            TransitionSnapshotV1 {
                state: if failure_at == 1 {
                    abi::RT_ADD_V1
                } else {
                    abi::RT_CONSISTENT_V1
                },
                linked: failure_at == 2,
            }
        );
        assert_eq!(
            value.transition(false, |_| panic!("poison notified")),
            Err(MetadataErrorV1::Transition)
        );
        finish_synthetic(&mut value);
    }
}

#[test]
fn each_delete_notification_failure_poison_retains_exact_progress() {
    for failure_at in 1..=2 {
        let mut value = prepared();
        value.activate_for_test();
        value.transition(true, |_| Ok(())).unwrap();
        let mut count = 0;
        assert_eq!(
            value.transition(false, |_| {
                count += 1;
                if count == failure_at {
                    Err(MetadataErrorV1::Notification)
                } else {
                    Ok(())
                }
            }),
            Err(MetadataErrorV1::Notification)
        );
        assert_eq!(value.phase, Phase::Poisoned);
        assert_eq!(
            value.snapshot(),
            TransitionSnapshotV1 {
                state: if failure_at == 1 {
                    abi::RT_DELETE_V1
                } else {
                    abi::RT_CONSISTENT_V1
                },
                linked: failure_at == 1,
            }
        );
        finish_synthetic(&mut value);
    }
}

#[test]
fn notification_unwind_leaves_poisoned_storage_retained() {
    for add in [true, false] {
        for failure_at in 1..=2 {
            let mut value = prepared();
            value.activate_for_test();
            if !add {
                value.transition(true, |_| Ok(())).unwrap();
            }
            let mut count = 0;
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = value.transition(add, |_| {
                    count += 1;
                    assert_ne!(count, failure_at, "synthetic callback unwind");
                    Ok(())
                });
            }));
            assert!(outcome.is_err());
            assert_eq!(value.phase, Phase::Poisoned);
            assert_eq!(count, failure_at);
            finish_synthetic(&mut value);
        }
    }
}

#[test]
fn process_drift_poison_refuses_before_notification() {
    let mut value = prepared();
    value.activate_for_test();
    value.opener_pid = 0;
    assert_eq!(
        value.transition(true, |_| panic!("foreign process notified")),
        Err(MetadataErrorV1::ProcessChanged)
    );
    assert_eq!(value.phase, Phase::Poisoned);
    assert_eq!(
        value.snapshot(),
        TransitionSnapshotV1 {
            state: abi::RT_CONSISTENT_V1,
            linked: false,
        }
    );
    finish_synthetic(&mut value);
}

#[test]
fn retention_takes_record_uri_and_elf_together_for_every_ambiguous_phase() {
    for phase in [Phase::ActiveAbsent, Phase::ActivePresent, Phase::Poisoned] {
        let mut value = prepared();
        let expected = addresses(&value);
        value.phase = phase; // Storage policy only; no fabricated runtime admission.
        let (record, elf) = value.take_storage_to_retain().unwrap();
        assert_eq!(&record[0].root as *const _ as usize, expected[0]);
        assert_eq!(&record[0].link as *const _ as usize, expected[1]);
        assert_eq!(record[0].uri.as_ptr() as usize, expected[2]);
        assert_eq!(elf.as_ptr() as usize, expected[3]);
        assert!(value.record.is_empty());
        assert!(value.original_elf.is_empty());
        // Native publication never occurred; safely reclaim synthetic storage.
        drop((record, elf));
    }
}

#[test]
fn prepared_and_acknowledged_detached_storage_use_normal_drop() {
    let mut value = prepared();
    assert!(value.take_storage_to_retain().is_none());
    value.activate_for_test();
    value.acknowledge_detached_for_test();
    assert!(value.take_storage_to_retain().is_none());
    assert_eq!(value.record.len(), 1);
    assert!(!value.original_elf.is_empty());
}

#[test]
fn artifact_limit_is_checked_without_large_allocation() {
    assert_eq!(require_elf_bound(0), Err(MetadataErrorV1::ArtifactBound));
    assert_eq!(require_elf_bound(1), Ok(()));
    assert_eq!(require_elf_bound(fe2o3_hsaco::MAX_HSACO_BYTES), Ok(()));
    assert_eq!(
        require_elf_bound(fe2o3_hsaco::MAX_HSACO_BYTES + 1),
        Err(MetadataErrorV1::ArtifactBound)
    );
    assert_eq!(
        require_elf_bound(usize::MAX),
        Err(MetadataErrorV1::ArtifactBound)
    );
    assert!(matches!(
        MetadataStorageV1::prepare(&[], 0),
        Err(MetadataErrorV1::ArtifactBound)
    ));
}

#[test]
fn signed_load_bias_checks_both_extremes_and_actual_zero() {
    for (mapping, image, expected) in [
        (1, 0, 1),
        (4096, 8192, -4096),
        (8192, 4096, 4096),
        (9, 9, 0),
        (i64::MAX as u64, 0, i64::MAX),
        (1, (1_u64 << 63) + 1, i64::MIN),
    ] {
        assert_eq!(checked_load_bias(mapping, image), Ok(expected));
    }
    assert_eq!(checked_load_bias(0, 0), Err(MetadataErrorV1::Mapping));
    assert_eq!(
        checked_load_bias(1_u64 << 63, 0),
        Err(MetadataErrorV1::LoadBias)
    );
    assert_eq!(
        checked_load_bias(1, (1_u64 << 63) + 2),
        Err(MetadataErrorV1::LoadBias)
    );
    assert_eq!(
        checked_load_bias(u64::MAX, 0),
        Err(MetadataErrorV1::LoadBias)
    );
    assert_eq!(
        checked_load_bias(1, u64::MAX),
        Err(MetadataErrorV1::LoadBias)
    );
}

#[test]
fn uri_numeric_extremes_are_bounded_ascii_nul_terminated() {
    let mut output = [0xff; URI_CAPACITY];
    write_uri(&mut output, u32::MAX, u64::MAX, usize::MAX).unwrap();
    let expected = format!(
        "memory://{}#offset=0x{:x}&size={}",
        u32::MAX,
        u64::MAX,
        usize::MAX
    );
    assert_eq!(&output[..expected.len()], expected.as_bytes());
    assert!(output[expected.len()..].iter().all(|byte| *byte == 0));
    for (pid, address, bytes) in [(0, 1, 1), (1, 0, 1), (1, 1, 0)] {
        assert_eq!(
            write_uri(&mut output, pid, address, bytes),
            Err(MetadataErrorV1::UriBound)
        );
        assert!(output.iter().all(|byte| *byte == 0));
    }
}

#[test]
fn fixed_uri_writer_refuses_nonascii_and_full_buffer_before_copy() {
    let mut output = [0; URI_CAPACITY];
    let mut writer = UriWriter {
        bytes: &mut output,
        used: 0,
    };
    assert!(writer.write_str("é").is_err());
    assert_eq!(writer.used, 0);
    assert!(writer.write_str(&"a".repeat(URI_CAPACITY)).is_err());
    assert_eq!(writer.used, 0);
    writer.write_str(&"a".repeat(URI_CAPACITY - 1)).unwrap();
    assert!(writer.write_str("b").is_err());
    assert_eq!(writer.used, URI_CAPACITY - 1);
    assert_eq!(output[URI_CAPACITY - 1], 0);
}
