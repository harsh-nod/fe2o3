// Included in the existing shared-engine test module to use its real backend.

#[test]
fn multiwave_join_real_engine_roots_code_kernarg_release_before_close() {
    let mut core = observed_core();
    let mut initialized = false;
    gfx950_observed::exercise_multiwave_join_resources(
        &mut core,
        0,
        |mapping| {
            initialized = true;
            assert_eq!(mapping.bytes.len(), 4096);
            // Fake mapping marker only. Linux's existing helper/tests construct
            // and acquire actual AtomicU32; this fixture exercises engine custody.
            mapping.bytes[..36].fill(7);
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
                    [1024, 1536, 36, 4096, 280]
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
fn multiwave_join_actual_engine_rejects_swapped_foreign_stale_alias_alignment_and_extent() {
    for mutation in 1..=6 {
        let mut core = observed_core();
        assert!(
            gfx950_observed::exercise_multiwave_join_resources(
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
fn multiwave_join_real_engine_allocation_map_and_atomic_initializer_errors_retain_custody() {
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
                gfx950_observed::exercise_multiwave_join_resources(
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
        gfx950_observed::exercise_multiwave_join_resources(
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
fn multiwave_join_actual_engine_seal_read_release_and_close_errors_never_reactivate() {
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
            gfx950_observed::exercise_multiwave_join_resources(
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
        gfx950_observed::exercise_multiwave_join_resources(
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
    gfx950_observed::exercise_multiwave_join_resources(&mut core, 0, |_| Ok(()), |_, _| {})
        .unwrap();
    core.test_engine().backend.fail_operation = Some("currentness");
    assert!(core.close().is_err());
    assert_eq!(core.test_phase(), Gfx950ObservedMemoryPhaseV1::Quarantined);
}
