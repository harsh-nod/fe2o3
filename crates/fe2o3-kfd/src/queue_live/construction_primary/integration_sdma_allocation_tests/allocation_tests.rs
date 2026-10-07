use super::*;

#[test]
fn constructed_sdma_allocation_success_preserves_extents_owners_and_refunds() {
    for host in [false, true] {
        for bytes in [1, 17, 4097] {
            let mut f = AllocationParent::new();
            let e = &f.parent.engine;
            let host_before = e.backend.session.coherent_insertion_snapshot_v1();
            let device_before = e.backend.session.insertion_memory_snapshot_v1();
            let accounting = e.backend.session.observation();
            let resources = original_resource_ids(&f.parent);
            let signal = Memory::primary_token_identity(f.parent.signals.as_ref().unwrap());
            let loan = e.backend.session.primary_loan_state_v1(&e.foundation);
            let buffer = f.allocate(host, bytes).unwrap();
            assert!(f.custody.is_none() && !f.parent.poisoned);
            assert_eq!(f.outstanding, 1);
            assert_eq!(f.calls, ["loan", "retake"]);
            assert!(buffer.belongs_to(f.parent.key));
            assert_eq!(buffer.pool_generation(), 1);
            assert_eq!(buffer.requested_bytes(), bytes as u64);
            assert_eq!(
                buffer.physical_bytes(),
                if host {
                    bytes as u64
                } else {
                    (bytes as u64).next_multiple_of(4096)
                }
            );
            assert!(
                buffer
                    .certified_full_host_content_sha256(bytes as u64)
                    .is_none()
            );
            let memory = &f.parent.engine.backend.session;
            assert_eq!(
                memory.primary_loan_state_v1(&f.parent.engine.foundation),
                (loan.0, None, loan.2 + 1)
            );
            memory
                .primary_authenticate(&f.parent.engine.foundation)
                .unwrap();
            assert_eq!(original_resource_ids(&f.parent), resources);
            assert_eq!(
                Memory::primary_token_identity(f.parent.signals.as_ref().unwrap()),
                signal
            );
            f.parent
                .sdma
                .as_ref()
                .unwrap()
                .preflight_retained_sdma_release_v1(f.parent.key, f.parent.queue_id)
                .unwrap();
            memory.assert_original_records_unchanged(&accounting);
            memory.primary_assert_accounts_and_records(memory.primary_session_id());
            if host {
                memory.coherent_assert_prefix_for_length_v1(
                    &host_before,
                    bytes,
                    None,
                    CoherentInsertionPrefixV1 {
                        calls: [5, 1, 1, 1, 1],
                        operations: vec!["map_cpu", "prepare_cpu_mapping", "map_gpu"],
                        copied: 0,
                        record_phase: Some("GpuAccessibleMutable"),
                        pending: None,
                    },
                );
                HOST_TRACE.with(|t| {
                    assert_eq!(t.borrow().stages, [1, 0, 1]);
                    assert!(t.borrow().source.is_none());
                });
            } else {
                memory.insertion_assert_native_prefix_with_layout_v1(
                    &device_before,
                    DeviceInsertionPrefixV1 {
                        calls: [4, 1, 1, 0, 1],
                        phase: Some("Mapped"),
                        handle: true,
                        cpu_writable: None,
                        written: false,
                        operations: &["map_gpu"],
                    },
                    device_layout(bytes),
                    None,
                );
            }
            f.release(buffer);
            f.shutdown();
        }
    }
}

#[test]
fn constructed_sdma_allocation_retake_failure_keeps_exact_completed_owner() {
    for host in [false, true] {
        for fault in [
            Fault::Error,
            Fault::Panic,
            Fault::AfterError,
            Fault::AfterPanic,
            Fault::Regression,
        ] {
            let mut f = AllocationParent::new();
            let memory = &f.parent.engine.backend.session;
            let id = if host {
                memory.coherent_insertion_snapshot_v1().next_id
            } else {
                memory.insertion_memory_snapshot_v1().next_id
            };
            let loan = memory.primary_loan_state_v1(&f.parent.engine.foundation);
            let resources = original_resource_ids(&f.parent);
            f.closing = fault;
            let result = catch_unwind(AssertUnwindSafe(|| f.allocate(host, 17)));
            match fault {
                Fault::Panic | Fault::AfterPanic => assert_eq!(
                    result.unwrap_err().downcast_ref::<&str>(),
                    Some(&if fault == Fault::Panic {
                        "allocation retake"
                    } else {
                        "allocation retake after"
                    })
                ),
                Fault::Regression => assert!(matches!(
                    result.unwrap(),
                    Err(ComputeAqlQueueSessionErrorV1::Memory(
                        MemorySessionError::Model("fixture live foundation reclaim")
                    ))
                )),
                Fault::Error | Fault::AfterError => {
                    let expected = if fault == Fault::Error {
                        "allocation retake"
                    } else {
                        "allocation retake after"
                    };
                    assert!(
                        matches!(result.unwrap(), Err(ComputeAqlQueueSessionErrorV1::Contract(actual)) if actual == expected)
                    );
                }
                Fault::None => unreachable!(),
            }
            assert_eq!(f.outstanding, 0);
            assert!(f.parent.poisoned);
            assert_eq!(f.calls, ["loan", "retake"]);
            let reclaimed = matches!(fault, Fault::AfterError | Fault::AfterPanic);
            let e = &f.parent.engine;
            assert_eq!(e.backend.foundation_in_engine, reclaimed);
            assert_eq!(
                e.backend.session.primary_loan_state_v1(&e.foundation),
                (loan.0, (!reclaimed).then_some(loan.2), loan.2 + 1)
            );
            assert_eq!(original_resource_ids(&f.parent), resources);
            match f.custody.as_ref().unwrap() {
                SdmaAllocationCustodyV1::Host { bytes, allocation } => {
                    assert_eq!(*bytes, 17);
                    let snapshot = allocation.coherent_snapshot_for_test().unwrap();
                    snapshot.assert_id(id);
                    HOST_TRACE.with(|t| {
                        assert_eq!(
                            Some(snapshot),
                            t.borrow().allocated.as_ref().map(|s| s.mapped())
                        )
                    });
                }
                SdmaAllocationCustodyV1::Device {
                    logical_bytes,
                    allocation,
                    ..
                } => {
                    assert_eq!(*logical_bytes, 17);
                    let snapshot = allocation.insertion_snapshot_for_test();
                    assert!(snapshot.started() && snapshot.native_started() && !snapshot.failed());
                    let (_, layout, mapped) = snapshot.lease().unwrap();
                    assert_eq!(layout, device_layout(17));
                    assert!(mapped);
                    e.backend.session.insertion_assert_allocation_partition_v1(
                        &[],
                        &[],
                        Some(allocation),
                        id,
                        device_layout(17),
                        &[],
                    );
                }
            }
            f.no_retry();
        }
    }
}

#[test]
fn constructed_sdma_allocation_rejections_preserve_loan_precedence_and_retry() {
    for host in [false, true] {
        for exhausted in [false, true] {
            let mut f = AllocationParent::new();
            if exhausted {
                f.parent
                    .engine
                    .backend
                    .session
                    .primary_expire_loan_generation_v1();
            }
            let e = &f.parent.engine;
            let before_host = e.backend.session.coherent_insertion_snapshot_v1();
            let before_device = e.backend.session.insertion_memory_snapshot_v1();
            let before_model = e.backend.session.insertion_model_snapshot_v1(&e.foundation);
            let loan = e.backend.session.primary_loan_state_v1(&e.foundation);
            let result = f.allocate(host, 0);
            if exhausted {
                assert!(matches!(
                    result,
                    Err(ComputeAqlQueueSessionErrorV1::Memory(
                        MemorySessionError::Model("fixture live foundation loan")
                    ))
                ));
                assert_eq!(f.calls, ["loan"]);
            } else {
                match result {
                    Err(ComputeAqlQueueSessionErrorV1::Sdma(
                        crate::sdma::Gfx942SdmaErrorV1::Memory(
                            MemorySessionError::InvalidRequestedSize,
                        ),
                    )) => assert!(host),
                    Err(ComputeAqlQueueSessionErrorV1::Sdma(
                        crate::sdma::Gfx942SdmaErrorV1::Memory(
                            MemorySessionError::InvalidDeviceMemorySize,
                        ),
                    )) => assert!(!host),
                    _ => panic!("exact zero-size rejection required"),
                }
                assert_eq!(f.calls, ["loan", "retake"]);
            }
            assert!(f.custody.is_none() && !f.parent.poisoned);
            assert_eq!(f.outstanding, 0);
            let e = &f.parent.engine;
            assert!(
                e.backend.session.coherent_insertion_snapshot_v1() == before_host,
                "host records unchanged"
            );
            assert!(
                e.backend.session.insertion_memory_snapshot_v1() == before_device,
                "device records unchanged"
            );
            let expected_model = if exhausted {
                before_model
            } else {
                before_model.checkpoint_released().unwrap()
            };
            assert!(
                e.backend.session.insertion_model_snapshot_v1(&e.foundation) == expected_model,
                "only normal loan checkpointing changes model state"
            );
            assert_eq!(
                e.backend.session.primary_loan_state_v1(&e.foundation),
                (loan.0, None, loan.2 + u64::from(!exhausted))
            );
            if !exhausted {
                let buffer = f.allocate(host, 17).unwrap();
                f.release(buffer);
                f.shutdown();
            }
        }
        let mut f = AllocationParent::new();
        f.outstanding = usize::MAX;
        assert!(matches!(
            f.allocate(host, 17),
            Err(ComputeAqlQueueSessionErrorV1::Contract(
                "SDMA buffer ledger exhausted"
            ))
        ));
        assert!(f.calls.is_empty() && f.custody.is_none());
        assert_eq!(f.outstanding, usize::MAX);
    }
}

#[test]
fn constructed_sdma_allocation_opening_failure_never_calls_native_or_commits() {
    for host in [false, true] {
        for fault in [Fault::Error, Fault::Panic] {
            let mut f = AllocationParent::new();
            let e = &f.parent.engine;
            let before = e.backend.session.data_release_snapshot_v1(&e.foundation);
            let loan = e.backend.session.primary_loan_state_v1(&e.foundation);
            f.opening = fault;
            let result = catch_unwind(AssertUnwindSafe(|| f.allocate(host, 17)));
            if fault == Fault::Panic {
                assert_eq!(
                    result.unwrap_err().downcast_ref::<&str>(),
                    Some(&"allocation loan")
                );
                assert!(f.parent.poisoned && f.custody.is_some());
                f.no_retry();
            } else {
                assert!(matches!(
                    result.unwrap(),
                    Err(ComputeAqlQueueSessionErrorV1::Contract("allocation loan"))
                ));
                assert!(!f.parent.poisoned && f.custody.is_none());
            }
            assert_eq!(f.outstanding, 0);
            assert_eq!(f.calls, ["loan"]);
            let e = &f.parent.engine;
            assert!(e.backend.foundation_in_engine);
            assert!(
                e.backend.session.data_release_snapshot_v1(&e.foundation) == before,
                "opening failure changed native/model state"
            );
            assert_eq!(e.backend.session.primary_loan_state_v1(&e.foundation), loan);
        }
    }
}

#[test]
fn constructed_sdma_allocation_invalid_device_layout_retakes_without_native_effects() {
    for (bytes, alignment) in [(17, 0), (17, 3), (u64::MAX, 4096)] {
        let mut f = AllocationParent::new();
        let e = &f.parent.engine;
        let before_host = e.backend.session.coherent_insertion_snapshot_v1();
        let before_device = e.backend.session.insertion_memory_snapshot_v1();
        let loan = e.backend.session.primary_loan_state_v1(&e.foundation);
        let result =
            allocate_in_place(&mut f, SdmaAllocationRequestV1::Device { bytes, alignment });
        assert!(matches!(
            result,
            Err(ComputeAqlQueueSessionErrorV1::Sdma(
                crate::sdma::Gfx942SdmaErrorV1::Memory(
                    MemorySessionError::InvalidDeviceMemoryAlignment
                        | MemorySessionError::InvalidDeviceMemorySize
                )
            ))
        ));
        assert_eq!(f.calls, ["loan", "retake"]);
        assert!(f.custody.is_none() && !f.parent.poisoned && f.outstanding == 0);
        let e = &f.parent.engine;
        assert!(e.backend.session.coherent_insertion_snapshot_v1() == before_host);
        assert!(e.backend.session.insertion_memory_snapshot_v1() == before_device);
        assert_eq!(
            e.backend.session.primary_loan_state_v1(&e.foundation),
            (loan.0, None, loan.2 + 1)
        );
        let buffer = f.allocate(false, 17).unwrap();
        f.release(buffer);
        f.shutdown();
    }
}

#[test]
fn constructed_sdma_allocation_classified_capacity_preserves_exact_error_and_retry() {
    for host in [false, true] {
        let mut f = AllocationParent::new();
        let e = &f.parent.engine;
        let host_before = e.backend.session.coherent_insertion_snapshot_v1();
        let device_before = e.backend.session.insertion_memory_snapshot_v1();
        let account_before = e.backend.session.observation();
        let loan = e.backend.session.primary_loan_state_v1(&e.foundation);
        let failure = match classified_allocate(&mut f, host, (1 << 20) + 4096) {
            Err(failure) => failure,
            Ok(_) => panic!("capacity rejection required"),
        };
        assert_eq!(
            failure.disposition(),
            Gfx942SdmaAllocationDispositionV1::RetryableCapacity
        );
        assert!(std::error::Error::source(&failure).is_some());
        assert_eq!(failure.to_string(), failure.error().to_string());
        let detail = failure.to_string();
        match failure.into_error() {
            ComputeAqlQueueSessionErrorV1::Sdma(Gfx942SdmaErrorV1::Memory(
                MemorySessionError::HostVisibleBackingCredits(
                    fe2o3_resource_accounting::ResourceCreditErrorV1::Capacity,
                ),
            )) => assert!(host),
            ComputeAqlQueueSessionErrorV1::Sdma(Gfx942SdmaErrorV1::Memory(
                MemorySessionError::DeviceBackingCredits(
                    fe2o3_resource_accounting::ResourceCreditErrorV1::Capacity,
                ),
            )) => assert!(!host),
            _ => panic!("original typed capacity"),
        }
        assert!(f.capacity_retry_is_settled());
        assert_eq!(f.calls, ["loan", "retake"]);
        assert_eq!(f.outstanding, 0);
        let e = &f.parent.engine;
        assert!(e.backend.session.coherent_insertion_snapshot_v1() == host_before);
        assert!(e.backend.session.insertion_memory_snapshot_v1() == device_before);
        assert!(e.backend.session.observation() == account_before);
        assert_eq!(
            e.backend.session.primary_loan_state_v1(&e.foundation),
            (loan.0, None, loan.2 + 1)
        );
        let legacy = f.allocate(host, (1 << 20) + 4096).err().unwrap();
        assert_eq!(legacy.to_string(), detail);
        assert!(f.capacity_retry_is_settled());
        let buffer = classified_allocate(&mut f, host, 17).ok().unwrap();
        f.release(buffer);
        f.shutdown();
    }
}

#[test]
fn constructed_sdma_allocation_classified_capacity_requires_settlement_witness() {
    let mut f = AllocationParent::new();
    // Deny only the adapter's final witness; memory admission and model retake are real.
    f.capacity_retry_settled = false;
    let e = &f.parent.engine;
    let host = e.backend.session.coherent_insertion_snapshot_v1();
    let device = e.backend.session.insertion_memory_snapshot_v1();
    let accounting = e.backend.session.observation();
    let loan = e.backend.session.primary_loan_state_v1(&e.foundation);
    let failure = classified_allocate(&mut f, true, (1 << 20) + 4096)
        .err()
        .unwrap();
    assert_eq!(
        failure.disposition(),
        Gfx942SdmaAllocationDispositionV1::ProcessTeardown
    );
    assert!(matches!(
        failure.into_error(),
        ComputeAqlQueueSessionErrorV1::Sdma(Gfx942SdmaErrorV1::Memory(
            MemorySessionError::HostVisibleBackingCredits(
                fe2o3_resource_accounting::ResourceCreditErrorV1::Capacity
            )
        ))
    ));
    assert_eq!(f.calls, ["loan", "retake"]);
    assert!(f.custody.is_none() && !f.parent.poisoned && f.outstanding == 0);
    let e = &f.parent.engine;
    assert!(e.backend.foundation_in_engine);
    assert!(e.backend.session.coherent_insertion_snapshot_v1() == host);
    assert!(e.backend.session.insertion_memory_snapshot_v1() == device);
    assert!(e.backend.session.observation() == accounting);
    assert_eq!(
        e.backend.session.primary_loan_state_v1(&e.foundation),
        (loan.0, None, loan.2 + 1)
    );
    f.shutdown();
}

#[test]
fn constructed_sdma_allocation_classified_retake_and_native_failures_never_grant_retry() {
    for host in [false, true] {
        for closing in [Fault::Error, Fault::AfterError, Fault::Regression] {
            let mut f = AllocationParent::new();
            f.closing = closing;
            let failure = classified_allocate(&mut f, host, (1 << 20) + 4096)
                .err()
                .unwrap();
            assert_eq!(
                failure.disposition(),
                Gfx942SdmaAllocationDispositionV1::ProcessTeardown
            );
            if closing != Fault::Regression {
                assert!(matches!(
                    failure.error(),
                    ComputeAqlQueueSessionErrorV1::Contract(
                        "allocation retake" | "allocation retake after"
                    )
                ));
            }
            assert!(f.parent.poisoned && f.custody.is_some());
            assert!(!f.capacity_retry_is_settled());
            f.no_retry();
        }
        let mut f = AllocationParent::new();
        f.parent
            .engine
            .backend
            .session
            .primary_arm_native("map_gpu", false);
        let failure = classified_allocate(&mut f, host, 17).err().unwrap();
        assert_eq!(
            failure.disposition(),
            Gfx942SdmaAllocationDispositionV1::ProcessTeardown
        );
        assert!(f.parent.poisoned && f.custody.is_some());
        let calls = f.calls.clone();
        let repeat = classified_allocate(&mut f, host, 17).err().unwrap();
        assert_eq!(
            repeat.disposition(),
            Gfx942SdmaAllocationDispositionV1::ProcessTeardown
        );
        assert!(matches!(
            repeat.error(),
            ComputeAqlQueueSessionErrorV1::Contract("unfinished SDMA allocation")
        ));
        assert_eq!(f.calls, calls);
        f.no_retry();
    }
}

#[test]
fn constructed_sdma_allocation_classified_validation_and_loan_rejection_preserve_legacy_state() {
    for case in 0..3 {
        let mut f = AllocationParent::new();
        if case == 1 {
            f.opening = Fault::Error;
        }
        if case == 2 {
            f.outstanding = usize::MAX;
        }
        let failure = classified_allocate(&mut f, false, if case == 0 { 0 } else { 17 })
            .err()
            .unwrap();
        assert_eq!(
            failure.disposition(),
            Gfx942SdmaAllocationDispositionV1::ProcessTeardown
        );
        match (case, failure.into_error()) {
            (
                0,
                ComputeAqlQueueSessionErrorV1::Sdma(Gfx942SdmaErrorV1::Memory(
                    MemorySessionError::InvalidDeviceMemorySize,
                )),
            ) => (),
            (1, ComputeAqlQueueSessionErrorV1::Contract("allocation loan")) => (),
            (2, ComputeAqlQueueSessionErrorV1::Contract("SDMA buffer ledger exhausted")) => (),
            _ => panic!("original error is preserved"),
        }
        assert!(!f.parent.poisoned && f.custody.is_none());
        assert_eq!(
            f.calls.as_slice(),
            match case {
                0 => &["loan", "retake"][..],
                1 => &["loan"][..],
                _ => &[][..],
            }
        );
        f.opening = Fault::None;
        f.outstanding = 0;
        let buffer = f.allocate(false, 17).unwrap();
        f.release(buffer);
        f.shutdown();
    }
}

#[test]
fn constructed_sdma_allocation_budget_rejection_is_healthy_and_retryable() {
    for host in [false, true] {
        let mut f = AllocationParent::new();
        let e = &f.parent.engine;
        let before_host = e.backend.session.coherent_insertion_snapshot_v1();
        let before_device = e.backend.session.insertion_memory_snapshot_v1();
        let accounting = e.backend.session.observation();
        let loan = e.backend.session.primary_loan_state_v1(&e.foundation);
        let result = f.allocate(host, (1 << 20) + 4096);
        match result {
            Err(ComputeAqlQueueSessionErrorV1::Sdma(crate::sdma::Gfx942SdmaErrorV1::Memory(
                MemorySessionError::HostVisibleBackingCredits(
                    fe2o3_resource_accounting::ResourceCreditErrorV1::Capacity,
                ),
            ))) => assert!(host),
            Err(ComputeAqlQueueSessionErrorV1::Sdma(crate::sdma::Gfx942SdmaErrorV1::Memory(
                MemorySessionError::DeviceBackingCredits(
                    fe2o3_resource_accounting::ResourceCreditErrorV1::Capacity,
                ),
            ))) => assert!(!host),
            _ => panic!("exact backing-credit capacity rejection required"),
        }
        assert_eq!(f.calls, ["loan", "retake"]);
        assert!(f.custody.is_none() && !f.parent.poisoned && f.outstanding == 0);
        let e = &f.parent.engine;
        assert!(!e.backend.session.primary_is_quarantined_v1());
        assert!(e.backend.session.coherent_insertion_snapshot_v1() == before_host);
        assert!(e.backend.session.insertion_memory_snapshot_v1() == before_device);
        assert!(e.backend.session.observation() == accounting);
        assert_eq!(
            e.backend.session.primary_loan_state_v1(&e.foundation),
            (loan.0, None, loan.2 + 1)
        );
        let buffer = f.allocate(host, 17).unwrap();
        f.release(buffer);
        f.shutdown();
    }
}

#[test]
fn constructed_sdma_allocation_lower_panic_wins_secondary_retake_and_poison_panics() {
    for host in [false, true] {
        for closing in [
            Fault::None,
            Fault::Error,
            Fault::Panic,
            Fault::AfterError,
            Fault::AfterPanic,
        ] {
            for poison_panics in [false, true] {
                let mut f = AllocationParent::new();
                f.closing = closing;
                f.poison_panics = poison_panics;
                f.parent
                    .engine
                    .backend
                    .session
                    .primary_arm_native("map_gpu", true);
                let result = catch_unwind(AssertUnwindSafe(|| f.allocate(host, 17)));
                assert_eq!(
                    result.unwrap_err().downcast_ref::<(&str, &str)>(),
                    Some(&("N2 native panic", "map_gpu"))
                );
                assert!(f.parent.poisoned && f.custody.is_some());
                assert_eq!(f.outstanding, 0);
                f.no_retry();
            }
        }
    }
}

#[test]
fn constructed_sdma_allocation_native_error_and_panic_keep_original_owner_partition() {
    for host in [false, true] {
        for panic in [false, true] {
            let ops: &[&str] = if host {
                &[
                    "reserve_va",
                    "alloc",
                    "map_cpu",
                    "prepare_cpu_mapping",
                    "map_gpu",
                ]
            } else {
                &["reserve_va", "alloc", "map_gpu"]
            };
            for &op in ops {
                lower_failure(host, LowerFault::Native(op, panic));
            }
        }
    }
}

#[test]
fn constructed_sdma_allocation_currentness_failures_retain_exact_prefix() {
    for host in [false, true] {
        for panic in [false, true] {
            for check in 1..=if host { 5 } else { 4 } {
                lower_failure(host, LowerFault::Currentness(check, panic));
            }
        }
    }
}

#[test]
fn constructed_sdma_allocation_partial_and_malformed_mapping_never_return_authority() {
    for host in [false, true] {
        for (n, errno) in [(0, false), (0, true), (1, true), (2, false), (2, true)] {
            lower_failure(host, LowerFault::Map(n, errno));
        }
    }
}

#[test]
fn constructed_sdma_allocation_retake_error_precedes_lower_error() {
    for host in [false, true] {
        let mut f = AllocationParent::new();
        f.closing = Fault::Error;
        f.parent
            .engine
            .backend
            .session
            .primary_arm_native("map_gpu", false);
        assert!(matches!(
            f.allocate(host, 17),
            Err(ComputeAqlQueueSessionErrorV1::Contract("allocation retake"))
        ));
        assert!(f.parent.poisoned && f.custody.is_some());
        assert_eq!(f.outstanding, 0);
        f.no_retry();
    }
}
