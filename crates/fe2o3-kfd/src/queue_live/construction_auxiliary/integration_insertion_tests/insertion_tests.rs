use super::*;

#[test]
fn device_insertion_constructed_primary_and_auxiliary_preserve_success_identity_and_order() {
    for ordinal in 0..3 {
        for index in [Some(0), Some(2), Some(4), None] {
            let mut f = InsertionFixture::new(ordinal);
            let before = f.memory().observation();
            let native = f.memory().insertion_memory_snapshot_v1();
            let loan = f.loan_state();
            let (mut expected, count, _) = f.context().ledger_snapshot();
            assert_eq!(count, 4);
            let (bytes, content) = input();
            let pointer = bytes.as_ptr() as usize;
            let expected_bytes = bytes.to_vec();
            let settled = f.insert(index, bytes, 4096, content);
            assert!(!settled.transport);
            let data = settled.into_result().unwrap();
            assert_eq!(data.initialized_content(), Some(content));
            expected.insert(index.unwrap_or(count), data.storage_identity());
            f.data.push(data);
            assert_eq!(f.context().ledger_snapshot(), (expected, count + 1, None));
            let after = f.memory().insertion_memory_snapshot_v1();
            assert_eq!(after.terminal, None);
            f.memory().insertion_assert_native_prefix_v1(
                &native,
                DeviceInsertionPrefixV1 {
                    calls: [6, 1, 1, 1, 1],
                    phase: Some("Mapped"),
                    handle: true,
                    cpu_writable: None,
                    written: true,
                    operations: &["map_cpu", "prepare_cpu_mapping", "unmap_cpu", "map_gpu"],
                },
                &expected_bytes,
            );
            assert_eq!(after.account, native.account);
            assert_eq!(after.terminal_storage, native.terminal_storage);
            assert_eq!(
                after.initialized_bytes.as_ref().unwrap()[..expected_bytes.len()],
                expected_bytes
            );
            assert!(after.readback_calls > 0);
            f.trace.before_retake.as_ref().unwrap().assert_source(
                pointer,
                &expected_bytes,
                content,
            );
            assert_eq!(
                f.trace.before_retake.as_ref().unwrap().stage_name(),
                "Complete"
            );
            assert_eq!(f.loan_state(), (loan.0, None, loan.2 + 1));
            f.memory().assert_original_records_unchanged(&before);
            f.assert_partition(native.next_id, expected_bytes.len());
            f.restore_and_transport(false);
        }
    }
}

#[test]
fn device_insertion_completed_output_survives_each_failed_retake() {
    for ordinal in 0..3 {
        for closing in 0..5 {
            let mut f = InsertionFixture::new(ordinal);
            let before = f.memory().observation();
            let native = f.memory().insertion_memory_snapshot_v1();
            let ledger = f.context().ledger_snapshot();
            let loan = f.loan_state();
            let trace_offset = trace().borrow().calls.len();
            let pre_occurrence = trace()
                .borrow()
                .calls
                .iter()
                .filter(|&&s| s == "auxiliary-retake")
                .count()
                + 1;
            let post_occurrence = trace()
                .borrow()
                .calls
                .iter()
                .filter(|&&s| s == "auxiliary-retake-complete")
                .count()
                + 1;
            match closing {
                0 => f.scope.parent.faults.reclaim_before = Outcome::Error,
                1 => f.scope.parent.faults.reclaim_before = Outcome::Panic,
                2 => f.scope.parent.faults.reclaim_after = Outcome::Error,
                3 => f.scope.parent.faults.reclaim_after = Outcome::Panic,
                _ => f.scope.parent.faults.regress_revision = true,
            }
            let (bytes, content) = input();
            let pointer = bytes.as_ptr() as usize;
            let expected = bytes.to_vec();
            let settled = f.insert(Some(2), bytes, 4096, content);
            assert!(settled.transport);
            assert_eq!(settled.result.is_err(), matches!(closing, 1 | 3));
            assert!(!matches!(settled.result, Ok(Ok(_))));
            match &settled.result {
                Err(payload) => assert_eq!(
                    payload.downcast_ref::<(&str, usize)>(),
                    Some(&(
                        if closing == 1 {
                            "auxiliary-retake"
                        } else {
                            "auxiliary-retake-complete"
                        },
                        if closing == 1 {
                            pre_occurrence
                        } else {
                            post_occurrence
                        }
                    ))
                ),
                Ok(Err(ComputeAqlQueueSessionErrorV1::Contract(name))) => assert_eq!(
                    *name,
                    if closing == 0 {
                        "auxiliary-retake"
                    } else {
                        "auxiliary-retake-complete"
                    }
                ),
                Ok(Err(ComputeAqlQueueSessionErrorV1::Memory(MemorySessionError::Model(name))))
                    if closing == 4 =>
                {
                    assert_eq!(*name, "fixture live foundation reclaim");
                }
                _ => panic!("unexpected retake failure provenance"),
            }
            for name in ["auxiliary-loan", "auxiliary-retake"] {
                assert_eq!(
                    trace().borrow().calls[trace_offset..]
                        .iter()
                        .filter(|&&s| s == name)
                        .count(),
                    1
                );
            }
            assert_eq!(
                (
                    f.trace.reserve_calls,
                    f.trace.prepare_calls,
                    f.trace.fail_calls
                ),
                (1, 1, 1)
            );
            assert_eq!(f.context().ledger_snapshot(), ledger);
            let after = f.memory().insertion_memory_snapshot_v1();
            let root = after.terminal.as_ref().unwrap();
            assert_eq!(
                root,
                &f.trace
                    .before_retake
                    .as_ref()
                    .unwrap()
                    .with_failure_for_test()
            );
            assert_eq!(f.trace.before_failure, f.trace.before_retake);
            root.assert_source(pointer, &expected, content);
            assert_eq!(root.stage_name(), "Complete");
            assert!(root.failed() && root.native_started());
            assert!(root.lease().unwrap().2);
            assert_eq!(root.progress(), (true, Some(true), Some(1)));
            f.memory().insertion_assert_native_prefix_v1(
                &native,
                DeviceInsertionPrefixV1 {
                    calls: [6, 1, 1, 1, 1],
                    phase: Some("Mapped"),
                    handle: true,
                    cpu_writable: None,
                    written: true,
                    operations: &["map_cpu", "prepare_cpu_mapping", "unmap_cpu", "map_gpu"],
                },
                &expected,
            );
            assert!(f.memory().insertion_terminal_output_unavailable_v1());
            assert_eq!(after.account, native.account);
            assert_eq!(after.terminal_storage, native.terminal_storage);
            assert_eq!(
                f.loan_state(),
                (
                    loan.0,
                    (!matches!(closing, 2 | 3)).then_some(loan.2),
                    loan.2 + 1
                )
            );
            f.memory().assert_original_records_unchanged(&before);
            f.assert_partition(native.next_id, expected.len());
            assert_eq!(
                f.memory().observation().phase,
                SharedMemorySessionPhaseV1::Quarantined
            );
            f.assert_terminal_retry(&after);
            f.restore_and_transport(true);
        }
    }
}

#[test]
fn device_insertion_real_capacity_growth_and_fifteen_to_sixteen_are_pre_effect_bounded() {
    for ordinal in 0..3 {
        for index in [0, 7, 15] {
            let mut f = InsertionFixture::new(ordinal);
            assert!(f.context().ledger().identities.capacity() < 16);
            while f.data.len() < 15 {
                let (bytes, content) = input();
                let settled = f.insert(None, bytes, 4096, content);
                assert!(!settled.transport);
                f.data.push(settled.into_result().unwrap());
            }
            let (mut expected, count, _) = f.context().ledger_snapshot();
            assert_eq!(count, 15);
            let native = f.memory().insertion_memory_snapshot_v1();
            let (bytes, content) = input();
            let size = bytes.len();
            let settled = f.insert(Some(index), bytes, 4096, content);
            assert!(!settled.transport);
            let data = settled.into_result().unwrap();
            expected.insert(index, data.storage_identity());
            f.data.push(data);
            assert_eq!(f.context().ledger_snapshot(), (expected, 16, None));
            f.assert_partition(native.next_id, size);
            let before = f.memory().observation();
            let ledger = f.context().ledger_snapshot();
            let counts = (
                f.trace.reserve_calls,
                f.trace.prepare_calls,
                f.trace.fail_calls,
            );
            for index in [16, 99] {
                let (bytes, content) = input();
                let settled = f.insert(Some(index), bytes, 4096, content);
                assert!(!settled.transport);
                assert!(matches!(
                    settled.result,
                    Ok(Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                        Gfx942DispatchBindingErrorV1::DataLeaseCount {
                            requested: 17,
                            maximum: 16
                        }
                    )))
                ));
                assert_eq!(f.memory().observation(), before);
                assert_eq!(f.context().ledger_snapshot(), ledger);
                assert_eq!(
                    (
                        f.trace.reserve_calls,
                        f.trace.prepare_calls,
                        f.trace.fail_calls
                    ),
                    counts
                );
            }
            f.restore_and_transport(false);
        }
    }
}

#[test]
fn device_insertion_invalid_ordinal_and_reservation_reject_before_loan() {
    for ordinal in 0..3 {
        for mode in 0..3 {
            let mut f = InsertionFixture::new(ordinal);
            let before = f.memory().observation();
            let native = f.memory().insertion_memory_snapshot_v1();
            let ledger = f.context().ledger_snapshot();
            let loan = f.loan_state();
            if mode > 0 {
                f.trace.reserve = if mode == 1 {
                    Outcome::Error
                } else {
                    Outcome::Panic
                };
            }
            let (bytes, content) = input();
            let settled = f.insert(Some(if mode == 0 { 5 } else { 2 }), bytes, 4096, content);
            assert_eq!(settled.transport, mode == 2);
            assert_eq!(settled.result.is_err(), mode == 2);
            assert!(!matches!(settled.result, Ok(Ok(_))));
            assert_eq!(f.trace.prepare_calls, 0);
            assert_eq!(f.trace.fail_calls, usize::from(mode == 2));
            assert_eq!(f.memory().observation(), before);
            assert_eq!(f.memory().insertion_memory_snapshot_v1(), native);
            assert_eq!(f.loan_state(), loan);
            assert_eq!(f.context().ledger_snapshot(), ledger);
            f.restore_and_transport(settled.transport);
        }
    }
}

#[test]
fn device_insertion_opening_failure_never_initializes_or_retakes() {
    for ordinal in 0..3 {
        for mode in 0..3 {
            let mut f = InsertionFixture::new(ordinal);
            if mode == 2 {
                f.memory_mut().primary_expire_loan_generation_v1();
            } else {
                f.scope.parent.faults.loan = if mode == 0 {
                    Outcome::Error
                } else {
                    Outcome::Panic
                };
            }
            let loan = f.loan_state();
            let before = f.memory().observation();
            let native = f.memory().insertion_memory_snapshot_v1();
            let ledger = f.context().ledger_snapshot();
            let trace_offset = trace().borrow().calls.len();
            let (bytes, content) = input();
            let settled = f.insert(Some(2), bytes, 4096, content);
            assert!(settled.transport);
            assert_eq!(settled.result.is_err(), mode == 1);
            assert!(!matches!(settled.result, Ok(Ok(_))));
            assert!(f.trace.before_retake.is_none());
            assert_eq!(
                f.trace.before_failure.as_ref().unwrap().stage_name(),
                "Source"
            );
            assert!(!f.trace.before_failure.as_ref().unwrap().native_started());
            assert_eq!(f.loan_state(), loan);
            assert_eq!(f.memory().observation(), before);
            assert_eq!(f.memory().insertion_memory_snapshot_v1(), native);
            assert_eq!(f.context().ledger_snapshot(), ledger);
            assert!(!trace().borrow().calls[trace_offset..].contains(&"auxiliary-retake"));
            f.assert_terminal_retry(&native);
            f.restore_and_transport(true);
        }
    }
}

#[test]
fn device_insertion_success_without_complete_cannot_commit_metadata() {
    let mut f = InsertionFixture::new(1);
    f.trace.skip_prepare = true;
    let before = f.memory().insertion_memory_snapshot_v1();
    let ledger = f.context().ledger_snapshot();
    let (bytes, content) = input();
    let settled = f.insert(Some(2), bytes, 4096, content);
    assert!(settled.transport);
    assert!(matches!(
        settled.result,
        Ok(Err(ComputeAqlQueueSessionErrorV1::Memory(
            MemorySessionError::InvalidDeviceMemoryAuthority
        )))
    ));
    assert_eq!(f.context().ledger_snapshot(), ledger);
    assert_eq!(f.memory().insertion_memory_snapshot_v1(), before);
    f.restore_and_transport(true);
}

#[test]
fn device_insertion_native_and_currentness_failures_retain_exact_prefixes() {
    let mut faults = vec![
        InitializerFault::Readback,
        InitializerFault::Layout,
        InitializerFault::Source,
    ];
    for panic in [false, true] {
        for (operation, stage) in [
            ("reserve_va", "Allocate"),
            ("alloc", "Allocate"),
            ("map_cpu", "CpuMap"),
            ("prepare_cpu_mapping", "CpuPrepare"),
            ("unmap_cpu", "CpuUnmap"),
            ("map_gpu", "GpuMap"),
        ] {
            faults.push(InitializerFault::Native(operation, panic, stage));
        }
        for (offset, stage) in [
            (1, "Allocate"),
            (2, "Allocate"),
            (3, "CpuCurrentness"),
            (4, "CpuClosingCurrentness"),
            (5, "GpuMap"),
            (6, "GpuMap"),
        ] {
            faults.push(InitializerFault::Currentness(offset, panic, stage));
        }
    }
    faults.extend([
        InitializerFault::Native("with_bytes_mut", true, "CpuWrite"),
        InitializerFault::Native("with_bytes", true, "CpuVerify"),
    ]);
    for (prefix, errno) in [(0, false), (0, true), (1, true), (2, false), (2, true)] {
        faults.push(InitializerFault::Map(prefix, errno));
    }
    for ordinal in 0..3 {
        for &fault in &faults {
            let mut f = InsertionFixture::new(ordinal);
            let native = f.memory().insertion_memory_snapshot_v1();
            let before = f.memory().observation();
            let ledger = f.context().ledger_snapshot();
            let loan = f.loan_state();
            fault.arm(&mut f);
            let (bytes, mut content) = input();
            let pointer = bytes.as_ptr() as usize;
            let expected = bytes.to_vec();
            if matches!(fault, InitializerFault::Source) {
                content = Gfx942DeviceContentDescriptorV1::from_bytes(
                    crate::Gfx942DeviceContentRoleV1::new([0x49; 32], 7).unwrap(),
                    &[0; 4103],
                )
                .unwrap();
            }
            let settled = f.insert(
                Some(2),
                bytes,
                if matches!(fault, InitializerFault::Layout) {
                    3
                } else {
                    4096
                },
                content,
            );
            assert!(settled.transport, "{fault:?}");
            assert_eq!(settled.result.is_err(), fault.panics(), "{fault:?}");
            assert!(!matches!(settled.result, Ok(Ok(_))), "{fault:?}");
            assert_eq!(f.context().ledger_snapshot(), ledger);
            let root = f.trace.before_failure.as_ref().unwrap();
            root.assert_source(pointer, &expected, content);
            assert_eq!(root.stage_name(), fault.stage(), "{fault:?}");
            assert_eq!(root.native_started(), fault.admitted(), "{fault:?}");
            assert!(root.failed());
            let after = f.memory().insertion_memory_snapshot_v1();
            assert_eq!(after.terminal.is_some(), fault.admitted(), "{fault:?}");
            assert_eq!(
                root.lease().map(|lease| lease.2),
                (fault.admitted() && fault.stage() != "Allocate").then_some(false),
                "{fault:?}"
            );
            assert_eq!(
                f.memory().observation().phase,
                if matches!(fault, InitializerFault::Source | InitializerFault::Layout) {
                    SharedMemorySessionPhaseV1::Active
                } else {
                    SharedMemorySessionPhaseV1::Quarantined
                },
                "{fault:?}"
            );
            let progress = match fault {
                InitializerFault::Map(prefix, errno) => (true, Some(!errno), Some(prefix)),
                InitializerFault::Native("map_gpu", true, _) => (true, None, None),
                InitializerFault::Native("map_gpu", false, _) => (true, Some(false), Some(1)),
                InitializerFault::Currentness(6, _, _) => (true, Some(true), Some(1)),
                _ => (false, None, None),
            };
            assert_eq!(root.progress(), progress);
            if let Some(terminal) = &after.terminal {
                assert_eq!(terminal, root);
            }
            assert_eq!(after.account, native.account);
            assert_eq!(after.terminal_storage, native.terminal_storage);
            f.memory()
                .insertion_assert_native_prefix_v1(&native, fault.prefix(), &expected);
            assert_eq!(f.loan_state(), (loan.0, None, loan.2 + 1));
            f.memory().assert_original_records_unchanged(&before);
            f.assert_partition(native.next_id, expected.len());
            f.assert_terminal_retry(&after);
            f.restore_and_transport(true);
        }
    }
}

#[test]
fn device_insertion_complete_stays_rooted_until_commit_ledger_access() {
    let mut f = InsertionFixture::new(1);
    f.trace.commit_panic = true;
    let ledger = f.context().ledger_snapshot();
    let native = f.memory().insertion_memory_snapshot_v1();
    let (bytes, content) = input();
    let expected = bytes.to_vec();
    let pointer = bytes.as_ptr() as usize;
    let settled = f.insert(Some(2), bytes, 4096, content);
    assert!(settled.transport);
    assert_eq!(
        settled.result.err().unwrap().downcast_ref::<&str>(),
        Some(&"insertion commit ledger access")
    );
    assert_eq!(f.context().ledger_snapshot(), ledger);
    let after = f.memory().insertion_memory_snapshot_v1();
    let root = after.terminal.as_ref().unwrap();
    root.assert_source(pointer, &expected, content);
    assert_eq!(root.stage_name(), "Complete");
    assert!(root.failed() && root.lease().unwrap().2);
    assert_eq!(
        root,
        &f.trace
            .before_retake
            .as_ref()
            .unwrap()
            .with_failure_for_test()
    );
    f.assert_partition(native.next_id, expected.len());
    assert_eq!(
        f.memory().observation().phase,
        SharedMemorySessionPhaseV1::Quarantined
    );
    f.assert_terminal_retry(&after);
    f.restore_and_transport(true);
}

#[test]
fn device_insertion_uses_a_genuine_release_hole_and_explicit_index_overrides_it() {
    for ordinal in 0..3 {
        for index in [None, Some(1)] {
            let mut f = InsertionFixture::new(ordinal);
            let old_identity = f.data[2].storage_identity();
            let mut data = Some(f.data.remove(2));
            let mut released = None;
            let (operation, retake) = f
                .scope
                .parent
                .with_preparation_custody(|memory| {
                    released = Some(memory.insertion_release_device_v1(data.take().unwrap())?);
                    Ok(())
                })
                .unwrap();
            retake.unwrap();
            operation.unwrap();
            f.released.push(released.unwrap());
            {
                let mut context = f.context();
                let ledger = context.ledger();
                assert_eq!(ledger.identities.remove(2), old_identity);
                *ledger.count -= 1;
                *ledger.next = Some(2);
            }
            let native = f.memory().insertion_memory_snapshot_v1();
            let (mut expected, count, next) = f.context().ledger_snapshot();
            assert_eq!((count, next), (3, Some(2)));
            let (bytes, content) = input();
            let size = bytes.len();
            let settled = f.insert(index, bytes, 4096, content);
            assert!(!settled.transport);
            let output = settled.into_result().unwrap();
            expected.insert(index.unwrap_or(2), output.storage_identity());
            f.data.push(output);
            assert_eq!(f.context().ledger_snapshot(), (expected, 4, None));
            f.assert_partition(native.next_id, size);
            f.restore_and_transport(false);
        }
    }
}

#[test]
fn device_insertion_operation_panic_wins_secondary_retake_error_or_panic() {
    for panic in [false, true] {
        for closing in [Outcome::Success, Outcome::Error, Outcome::Panic] {
            let mut f = InsertionFixture::new(1);
            f.memory_mut().primary_arm_native("map_gpu", panic);
            f.scope.parent.faults.reclaim_before = closing;
            let (bytes, content) = input();
            let ledger = f.context().ledger_snapshot();
            let calls = trace().borrow().calls.len();
            let settled = f.insert(Some(2), bytes, 4096, content);
            assert!(settled.transport);
            if panic {
                let payload = settled.result.err().unwrap();
                assert_eq!(
                    payload.downcast_ref::<(&str, &str)>(),
                    Some(&("N2 native panic", "map_gpu"))
                );
            } else if closing == Outcome::Error {
                assert!(matches!(
                    settled.result,
                    Ok(Err(ComputeAqlQueueSessionErrorV1::Contract(
                        "auxiliary-retake"
                    )))
                ));
            } else {
                assert_eq!(settled.result.is_err(), closing == Outcome::Panic);
            }
            assert_eq!(
                trace().borrow().calls[calls..]
                    .iter()
                    .filter(|&&call| call == "auxiliary-retake")
                    .count(),
                1
            );
            assert_eq!(f.context().ledger_snapshot(), ledger);
            assert_eq!(f.trace.panic_fail, panic || closing == Outcome::Panic);
            f.restore_and_transport(true);
        }
    }
}
