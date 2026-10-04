// Included in the existing shared-engine test module to use its real backend.
include!("shared_memory_gfx950_observed_multiwave_engine_tests.rs");

#[test]
fn fixed_join_real_engine_roots_code_kernarg_release_before_close() {
    let mut core = observed_core();
    let mut initialized = false;
    gfx950_observed::exercise_finite_join_resources(
        &mut core,
        0,
        |mapping| {
            initialized = true;
            assert_eq!(mapping.bytes.len(), 4096);
            // Fake mapping marker only. Linux's existing helper/tests construct
            // and acquire actual AtomicU32; this fixture exercises engine custody.
            mapping.bytes[..24].fill(7);
            Ok(())
        },
        |stage, engine| {
            if stage == "read" {
                assert_eq!(
                    engine
                        .allocations
                        .iter()
                        .map(|r| r.layout.requested_bytes)
                        .collect::<Vec<_>>(),
                    [1024, 1536, 24, 4096, 280]
                );
                assert_eq!(
                    engine.allocations[3].profile,
                    SharedGttProfileV1::Executable
                );
                assert_eq!(
                    engine.allocations[3].phase,
                    SharedAllocationPhaseV1::GpuAccessibleExecutable
                );
                assert_eq!(engine.allocations[4].profile, SharedGttProfileV1::Kernarg);
            }
        },
    )
    .unwrap();
    assert!(initialized);
    assert!(
        core.test_engine()
            .allocations
            .iter()
            .all(|r| r.is_fully_released())
    );
    assert_eq!(core.test_engine().backend.free_calls, 5);
    core.close().unwrap();
    assert_eq!(core.test_phase(), Gfx950ObservedMemoryPhaseV1::Closed);
}

#[test]
fn fixed_join_actual_engine_rejects_swapped_foreign_stale_alias_alignment_and_extent() {
    for mutation in 1..=6 {
        let mut core = observed_core();
        assert!(
            gfx950_observed::exercise_finite_join_resources(
                &mut core,
                mutation,
                |_| Ok(()),
                |_, _| {}
            )
            .is_err()
        );
        assert_eq!(core.test_phase(), Gfx950ObservedMemoryPhaseV1::Quarantined);
        assert_eq!(
            core.test_engine().phase(),
            SharedMemorySessionPhaseV1::Quarantined
        );
        assert_eq!(core.test_engine().backend.free_calls, 0);
        let operations = core.test_engine().backend.operations.clone();
        assert!(core.close().is_err());
        assert_eq!(operations, core.test_engine().backend.operations);
    }
}

#[test]
fn fixed_join_real_engine_allocation_map_and_atomic_initializer_errors_retain_custody() {
    for stage in ["input", "payload", "state", "code", "kernarg"] {
        for operation in [
            "reserve_va",
            "alloc",
            "map_cpu",
            "prepare_cpu_mapping",
            "map_gpu",
        ] {
            let mut core = observed_core();
            assert!(
                gfx950_observed::exercise_finite_join_resources(
                    &mut core,
                    0,
                    |_| Ok(()),
                    |at, e| {
                        if at == stage {
                            e.backend.fail_operation = Some(operation);
                        }
                    }
                )
                .is_err(),
                "{stage}/{operation}"
            );
            assert_eq!(core.test_phase(), Gfx950ObservedMemoryPhaseV1::Quarantined);
            assert_eq!(
                core.test_engine().phase(),
                SharedMemorySessionPhaseV1::Quarantined
            );
            assert_eq!(core.test_engine().backend.free_calls, 0);
        }
    }
    let mut core = observed_core();
    assert!(
        gfx950_observed::exercise_finite_join_resources(
            &mut core,
            0,
            |_| Err(MemorySessionError::InvalidAllocationAuthority),
            |_, _| {}
        )
        .is_err()
    );
    assert_eq!(core.test_engine().backend.map_gpu_calls, 2);
    assert_eq!(core.test_phase(), Gfx950ObservedMemoryPhaseV1::Quarantined);
}

#[test]
fn fixed_join_actual_engine_seal_read_release_and_close_errors_never_reactivate() {
    for (stage, operation) in [
        ("code", "protect_cpu_read_only"),
        ("read", "currentness"),
        ("release", "unmap_gpu"),
        ("release", "unmap_cpu"),
        ("release", "free"),
        ("release", "release_va_reservation"),
    ] {
        let mut core = observed_core();
        assert!(
            gfx950_observed::exercise_finite_join_resources(
                &mut core,
                0,
                |_| Ok(()),
                |at, e| {
                    if at == stage {
                        e.backend.fail_operation = Some(operation);
                    }
                }
            )
            .is_err()
        );
        assert_eq!(core.test_phase(), Gfx950ObservedMemoryPhaseV1::Quarantined);
        let operations = core.test_engine().backend.operations.clone();
        assert!(core.close().is_err());
        assert_eq!(operations, core.test_engine().backend.operations);
    }
    let mut core = observed_core();
    assert!(
        gfx950_observed::exercise_finite_join_resources(
            &mut core,
            0,
            |_| Ok(()),
            |at, e| {
                if at == "read" {
                    e.allocations[0].mapping.as_mut().unwrap().bytes[0] ^= 1;
                }
            }
        )
        .is_err()
    );
    assert_eq!(core.test_engine().backend.free_calls, 0);
    let mut core = observed_core();
    gfx950_observed::exercise_finite_join_resources(&mut core, 0, |_| Ok(()), |_, _| {}).unwrap();
    core.test_engine().backend.fail_operation = Some("currentness");
    assert!(core.close().is_err());
    assert_eq!(core.test_phase(), Gfx950ObservedMemoryPhaseV1::Quarantined);
}

#[test]
fn observed_barrier_real_signal_resource_preserves_root_and_releases_private_resources() {
    let mut core = observed_core();
    let root = core.initialize(vec![29; 1024].into_boxed_slice()).unwrap();
    let id = core.test_engine().allocations[0].id;
    let session = core.test_engine().session_id;
    gfx950_observed::exercise_barrier_resources(
        &mut core,
        |mapping| {
            assert_eq!(mapping.bytes.len(), 4096);
            // Fake storage marker only; actual signal atomics are initialized by
            // the Linux backend and are not simulated in this resource-only test.
            mapping.bytes.fill(9);
            Ok(())
        },
        |_| {},
        |engine| {
            assert_eq!(engine.allocations.len(), 6);
            assert_eq!(engine.allocations[5].layout.requested_bytes, 4096);
            assert_eq!(
                engine.allocations[5].profile,
                SharedGttProfileV1::HostVisibleCoherent
            );
            assert_eq!(engine.backend.map_gpu_calls, 6);
        },
    )
    .unwrap();
    assert_eq!(
        (
            core.test_engine().allocations[0].id,
            core.test_engine().session_id
        ),
        (id, session)
    );
    assert!(
        core.test_engine().allocations[1..]
            .iter()
            .all(|record| record.is_fully_released())
    );
    assert_eq!(core.test_phase(), Gfx950ObservedMemoryPhaseV1::Active);
    assert_eq!(core.read(&root).unwrap().as_ref(), &[29; 1024]);
    core.release(root).unwrap();
    core.close().unwrap();
}

#[test]
fn observed_barrier_signal_allocation_or_initialization_failure_quarantines_whole_owner() {
    for operation in [
        "reserve_va",
        "alloc",
        "map_cpu",
        "prepare_cpu_mapping",
        "initializer",
    ] {
        let mut core = observed_core();
        assert!(
            gfx950_observed::exercise_barrier_resources(
                &mut core,
                |_| if operation == "initializer" {
                    Err(MemorySessionError::InvalidAllocationAuthority)
                } else {
                    Ok(())
                },
                |engine| {
                    if operation != "initializer" {
                        engine.backend.fail_operation = Some(operation);
                    }
                },
                |_| panic!("no release after signal initialization failure"),
            )
            .is_err()
        );
        assert_eq!(core.test_phase(), Gfx950ObservedMemoryPhaseV1::Quarantined);
        assert_eq!(
            core.test_engine().phase(),
            SharedMemorySessionPhaseV1::Quarantined
        );
        assert!(core.test_engine().allocations.len() >= 4);
        assert_eq!(core.test_engine().backend.free_calls, 0);
        assert_eq!(core.test_engine().backend.map_gpu_calls, 0);
        let before = core.test_engine().backend.operations.clone();
        assert!(core.close().is_err());
        assert_eq!(core.test_engine().backend.operations, before);
    }
}

#[test]
fn observed_barrier_signal_release_failure_does_not_release_queue_or_retry() {
    for operation in ["unmap_gpu", "unmap_cpu", "free", "release_va_reservation"] {
        let mut core = observed_core();
        assert!(
            gfx950_observed::exercise_barrier_resources(
                &mut core,
                |_| Ok(()),
                |_| {},
                |engine| {
                    engine.backend.fail_operation = Some(operation);
                },
            )
            .is_err()
        );
        assert_eq!(core.test_phase(), Gfx950ObservedMemoryPhaseV1::Quarantined);
        let engine = core.test_engine();
        assert!(engine.backend.unmap_gpu_calls <= 1 && engine.backend.free_calls <= 1);
        assert!(
            engine.allocations[..4]
                .iter()
                .all(|record| !record.is_fully_released())
        );
        let before = engine.backend.operations.clone();
        assert!(core.close().is_err());
        assert_eq!(core.test_engine().backend.operations, before);
    }
}

#[test]
fn observed_queue_real_resource_helpers_preserve_original_initialized_root() {
    let mut core = observed_core();
    let root = core.initialize(vec![19; 1024].into_boxed_slice()).unwrap();
    let id = core.test_engine().allocations[0].id;
    let session = core.test_engine().session_id;
    gfx950_observed::exercise_queue_resources(&mut core, |engine| {
        assert_eq!(engine.allocations.len(), 5);
        assert_eq!(engine.backend.map_gpu_calls, 5);
        assert_eq!(engine.allocations[4].layout.requested_bytes, 0xad68000);
        assert_eq!(engine.allocations[1].profile, SharedGttProfileV1::AqlQueue);
    })
    .unwrap();
    assert_eq!(core.test_phase(), Gfx950ObservedMemoryPhaseV1::Active);
    assert_eq!(
        (
            core.test_engine().allocations[0].id,
            core.test_engine().session_id
        ),
        (id, session)
    );
    assert!(
        core.test_engine().allocations[1..]
            .iter()
            .all(|r| r.is_fully_released())
    );
    assert_eq!(core.read(&root).unwrap().as_ref(), &[19; 1024]);
    core.release(root).unwrap();
    core.close().unwrap();
}

#[test]
fn observed_queue_real_resource_allocation_failures_retain_engine_without_cleanup() {
    for operation in ["reserve_va", "alloc", "map_cpu", "prepare_cpu_mapping"] {
        let mut core = observed_core();
        core.test_engine().backend.fail_operation = Some(operation);
        assert!(gfx950_observed::exercise_queue_resources(&mut core, |_| {}).is_err());
        assert_eq!(core.test_phase(), Gfx950ObservedMemoryPhaseV1::Quarantined);
        assert_eq!(
            core.test_engine().phase(),
            SharedMemorySessionPhaseV1::Quarantined
        );
        assert_eq!(core.test_engine().backend.free_calls, 0);
        assert!(core.close().is_err());
    }
}

#[test]
fn observed_queue_real_resource_release_failures_do_not_continue_or_retry() {
    for operation in ["unmap_gpu", "unmap_cpu", "free", "release_va_reservation"] {
        let mut core = observed_core();
        assert!(
            gfx950_observed::exercise_queue_resources(&mut core, |engine| {
                engine.backend.fail_operation = Some(operation);
            })
            .is_err()
        );
        assert_eq!(core.test_phase(), Gfx950ObservedMemoryPhaseV1::Quarantined);
        let engine = core.test_engine();
        assert!(engine.backend.unmap_gpu_calls <= 1);
        assert!(engine.backend.free_calls <= 1);
        let before = engine.backend.operations.clone();
        assert!(core.close().is_err());
        assert_eq!(core.test_engine().backend.operations, before);
    }
}

#[test]
fn observed_queue_real_resource_seal_and_map_failures_never_release_native_roots() {
    for operation in ["protect_cpu_read_only", "map_gpu"] {
        let mut core = observed_core();
        core.test_engine().backend.fail_operation = Some(operation);
        assert!(gfx950_observed::exercise_queue_resources(&mut core, |_| {}).is_err());
        assert_eq!(core.test_phase(), Gfx950ObservedMemoryPhaseV1::Quarantined);
        assert_eq!(core.test_engine().allocations.len(), 4);
        assert_eq!(core.test_engine().backend.free_calls, 0);
        assert_eq!(core.test_engine().backend.unmap_gpu_calls, 0);
        assert!(core.close().is_err());
    }
}

fn observed_core() -> gfx950_observed::ObservedMemoryCore<FakeBackend> {
    gfx950_observed::ObservedMemoryCore::acquire(FakeBackend::good()).unwrap()
}

#[test]
fn gfx950_observed_copy_map_read_release_uses_shared_engine() {
    let mut core = observed_core();
    let source = (0..1024)
        .map(|i| (i % 251) as u8)
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let token = core.initialize(source.clone()).unwrap();
    assert_eq!(token.requested_bytes(), 1024);
    assert_eq!(token.alignment(), 4096);
    assert_eq!(core.read(&token).unwrap(), source);
    assert_eq!(core.test_engine().backend.map_gpu_calls, 1);
    core.release(token).unwrap();
    let backend = &core.test_engine().backend;
    assert_eq!(
        (
            backend.unmap_gpu_calls,
            backend.free_calls,
            backend.release_va_calls
        ),
        (1, 1, 1)
    );
    assert_eq!(
        backend.last_unmapped_bytes.as_ref().unwrap()[..1024],
        source[..]
    );
    core.close().unwrap();
    assert_eq!(core.test_phase(), Gfx950ObservedMemoryPhaseV1::Closed);
    assert!(core.close().is_err());
    assert!(core.initialize(vec![1].into_boxed_slice()).is_err());
}

#[test]
fn gfx950_observed_three_roots_preserve_identity_and_reverse_release() {
    let mut core = observed_core();
    let mut roots = Vec::new();
    for (size, fill) in [(1024, 17), (1536, 33), (24, 49)] {
        roots.push(
            core.initialize(vec![fill; size].into_boxed_slice())
                .unwrap(),
        );
    }
    for (token, (size, fill)) in roots.iter().zip([(1024, 17), (1536, 33), (24, 49)]) {
        assert_eq!(core.read(token).unwrap().as_ref(), vec![fill; size]);
    }
    for token in roots.into_iter().rev() {
        core.release(token).unwrap();
    }
    assert_eq!(core.test_engine().backend.free_calls, 3);
    core.close().unwrap();
}

#[test]
fn gfx950_observed_foreign_equal_extent_token_quarantines_without_read() {
    let mut first = observed_core();
    let mut second = observed_core();
    let token = first.initialize(vec![9; 24].into_boxed_slice()).unwrap();
    assert!(second.read(&token).is_err());
    assert_eq!(
        second.test_phase(),
        Gfx950ObservedMemoryPhaseV1::Quarantined
    );
    assert_eq!(second.test_engine().backend.free_calls, 0);
    assert!(second.close().is_err());
    assert_eq!(first.read(&token).unwrap().as_ref(), &[9; 24]);
    first.release(token).unwrap();
    first.close().unwrap();
}

#[test]
fn gfx950_observed_close_with_live_mapping_is_terminal_and_never_frees() {
    let mut core = observed_core();
    let token = core.initialize(vec![1; 24].into_boxed_slice()).unwrap();
    assert!(core.close().is_err());
    assert_eq!(core.test_phase(), Gfx950ObservedMemoryPhaseV1::Quarantined);
    assert!(core.release(token).is_err());
    assert_eq!(core.test_engine().backend.free_calls, 0);
}

#[test]
fn gfx950_observed_acquire_and_initialization_backend_failures_are_not_success() {
    let mut backend = FakeBackend::good();
    backend.fail_operation = Some("acquire_vm");
    assert!(gfx950_observed::ObservedMemoryCore::acquire(backend).is_err());
    for operation in [
        "reserve_va",
        "alloc",
        "map_cpu",
        "prepare_cpu_mapping",
        "map_gpu",
    ] {
        let mut core = observed_core();
        core.test_engine().backend.fail_operation = Some(operation);
        assert!(
            core.initialize(vec![7; 24].into_boxed_slice()).is_err(),
            "{operation}"
        );
        assert_eq!(core.test_phase(), Gfx950ObservedMemoryPhaseV1::Quarantined);
        let calls = core.test_engine().backend.alloc_calls;
        assert!(core.initialize(vec![7; 24].into_boxed_slice()).is_err());
        assert_eq!(core.test_engine().backend.alloc_calls, calls);
    }
}

#[test]
fn gfx950_observed_partial_or_malformed_mapping_is_retained_not_retried() {
    for progress in [0, 2] {
        let mut core = observed_core();
        core.test_engine().backend.map_progress = progress;
        assert!(core.initialize(vec![7; 24].into_boxed_slice()).is_err());
        assert_eq!(core.test_engine().backend.map_gpu_calls, 1);
        assert_eq!(core.test_engine().backend.free_calls, 0);
        assert!(core.close().is_err());
    }
}

#[test]
fn gfx950_observed_read_currentness_failure_returns_no_staged_bytes() {
    let mut core = observed_core();
    let token = core.initialize(vec![7; 24].into_boxed_slice()).unwrap();
    let next = core.test_engine().backend.currentness_calls + 2;
    core.test_engine().backend.fail_currentness_at = Some(next);
    assert!(core.read(&token).is_err());
    assert_eq!(core.test_phase(), Gfx950ObservedMemoryPhaseV1::Quarantined);
    assert!(core.release(token).is_err());
    assert_eq!(core.test_engine().backend.free_calls, 0);
}

#[test]
fn gfx950_observed_unmap_free_and_va_failures_never_retry_cleanup() {
    for operation in ["unmap_gpu", "unmap_cpu", "free", "release_va_reservation"] {
        let mut core = observed_core();
        let token = core.initialize(vec![7; 24].into_boxed_slice()).unwrap();
        core.test_engine().backend.fail_operation = Some(operation);
        assert!(core.release(token).is_err(), "{operation}");
        assert_eq!(core.test_phase(), Gfx950ObservedMemoryPhaseV1::Quarantined);
        let before = (
            core.test_engine().backend.unmap_gpu_calls,
            core.test_engine().backend.free_calls,
        );
        assert!(core.close().is_err());
        assert_eq!(
            (
                core.test_engine().backend.unmap_gpu_calls,
                core.test_engine().backend.free_calls
            ),
            before
        );
    }
}

#[test]
fn gfx950_observed_bad_alignment_or_post_map_currentness_never_returns_token() {
    let mut core = observed_core();
    core.test_engine().backend.fixed_va = Some(0x20001);
    assert!(core.initialize(vec![7; 24].into_boxed_slice()).is_err());
    assert_eq!(core.test_engine().backend.alloc_calls, 0);
    let mut successful = observed_core();
    let token = successful
        .initialize(vec![7; 24].into_boxed_slice())
        .unwrap();
    let count = successful.test_engine().backend.currentness_calls;
    successful.release(token).unwrap();
    successful.close().unwrap();
    let mut core = observed_core();
    core.test_engine().backend.fail_currentness_at = Some(count);
    assert!(core.initialize(vec![7; 24].into_boxed_slice()).is_err());
    assert_eq!(core.test_engine().backend.map_gpu_calls, 1);
    assert_eq!(core.test_engine().backend.free_calls, 0);
}
