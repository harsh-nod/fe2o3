use super::*;

fn step(
    fixture: &mut Fixture,
    index: usize,
    finish: bool,
    fault: Option<(Stage, Fault)>,
) -> Result<(), MemorySessionError> {
    let f = &mut fixture.memory.fixture;
    let mut projection = control_cleanup::ProjectionV1::new(&mut f.foundation, f.vm);
    projection.fault = fault;
    let poison = || fixture.memory.process_poisoned += 1;
    if finish {
        control_cleanup::finish_release_v1(
            &mut f.engine,
            &mut projection,
            &mut fixture.controls[index],
            poison,
        )
    } else {
        control_cleanup::unmap_v1(
            &mut f.engine,
            &mut projection,
            &mut fixture.controls[index],
            poison,
        )
    }
}

fn unmap_all(f: &mut Fixture) {
    for i in 0..f.controls.len() {
        step(f, i, false, None).unwrap();
    }
}

fn expected_split_model(
    f: &Fixture,
    before: &Snapshot,
    unmapped: usize,
    released: usize,
) -> MemoryLifecycleStateV1 {
    let mut model = before.model.clone();
    for (i, owner) in before.controls.iter().enumerate().take(unmapped) {
        let (reservation, allocation, mapping) = model_keys(
            f.memory.fixture.vm,
            owner.identity.id,
            owner.identity.generation,
        );
        model = project_unmap(&model, mapping).unwrap();
        if i < released {
            model = project_release(&model, reservation, allocation, mapping).unwrap();
        }
    }
    model
}

fn reject_both_steps(f: &mut Fixture, index: usize) {
    f.clear_faults();
    let before = f.snapshot();
    for finish in [false, true] {
        assert!(matches!(
            step(f, index, finish, None),
            Err(MemorySessionError::InvalidAllocationAuthority)
        ));
        assert_eq!(f.snapshot(), before);
    }
    f.reject_retry(index);
}

#[test]
fn split_cleanup_unmaps_all_controls_before_releasing_any() {
    for configured in [false, true] {
        let mut f = Fixture::new(configured);
        let before = f.snapshot();
        let mut expected = before.calls.clone();
        for i in 0..f.controls.len() {
            let prior = f.snapshot();
            assert!(matches!(
                step(&mut f, i, true, None),
                Err(MemorySessionError::InvalidAllocationAuthority)
            ));
            assert_eq!(f.snapshot(), prior, "release before unmap has no effects");

            step(&mut f, i, false, None).unwrap();
            expected.push(CleanupCallV1::UnmapGpu(
                record(&before, i).handle.unwrap(),
                0,
            ));
            let after = f.snapshot();
            assert_eq!(after.calls, expected);
            assert_eq!(after.model, expected_split_model(&f, &before, i + 1, 0));
            assert_eq!(after.retained_va, before.retained_va);
            assert_eq!(after.usage, before.usage);
            assert_eq!(after.devices, before.devices);
            assert_eq!(after.storage, before.storage);
            assert_eq!(after.phase, SharedMemorySessionPhaseV1::Active);
            for j in 0..=i {
                let owner = &after.controls[j];
                assert_eq!(owner.identity, before.controls[j].identity);
                assert_eq!(owner.layout, before.controls[j].layout);
                assert_eq!(owner.profile_type, before.controls[j].profile_type);
                assert_eq!(owner.owner, "Unmapped");
                assert_eq!(owner.stage, Stage::Unmapped);
                assert!(owner.started && !owner.failed && !owner.native_disposed);
                assert_eq!(owner.disposal, [(false, None); 3]);
                assert_eq!(
                    record(&after, j),
                    &expected_record(&before, j, true, 1, false)
                );
                assert!(!f.controls[j].is_complete());
            }
            assert_eq!(&after.controls[i + 1..], &before.controls[i + 1..]);
            assert!(step(&mut f, i, false, None).is_err());
            f.reject_retry(i);
            assert_eq!(f.snapshot(), after);
        }
        for i in 0..f.controls.len() {
            step(&mut f, i, true, None).unwrap();
            let r = record(&before, i);
            let m = r.mapping.as_ref().unwrap();
            let va = r.reservation.unwrap();
            expected.extend([
                CleanupCallV1::UnmapCpu(m.address, m.pointer, m.bytes.len()),
                CleanupCallV1::Free(r.handle.unwrap()),
                CleanupCallV1::ReleaseVa(va.0, va.1),
            ]);
            let after = f.snapshot();
            assert_eq!(after.calls, expected);
            assert_eq!(after.model, expected_split_model(&f, &before, 3, i + 1));
            assert_eq!(after.usage, before.usage);
            assert_eq!(after.devices, before.devices);
            assert_eq!(after.storage, before.storage);
            check_completed_prefix(&f, &before, i + 1);
            reject_both_steps(&mut f, i);
        }
    }
}

#[test]
fn split_cleanup_projection_failures_are_terminal_in_either_phase() {
    for i in 0..3 {
        for (finish, stage) in [
            (false, Stage::UnmapPreflight),
            (false, Stage::UnmapEvidence),
            (false, Stage::NativeUnmap),
            (false, Stage::UnmapProjection),
            (false, Stage::UnmapCommit),
            (true, Stage::ReleasePreflight),
            (true, Stage::ReleaseEvidence),
            (true, Stage::ReleaseProjection),
            (true, Stage::NativeRelease),
            (true, Stage::ReleaseCommit),
        ] {
            for panic in [false, true] {
                let mut f = Fixture::new(true);
                let original = f.snapshot();
                if finish {
                    unmap_all(&mut f);
                    for j in 0..i {
                        step(&mut f, j, true, None).unwrap();
                    }
                } else {
                    for j in 0..i {
                        step(&mut f, j, false, None).unwrap();
                    }
                }
                let before = f.snapshot();
                let fault = if panic { Fault::Panic } else { Fault::Error };
                let result = catch_unwind(AssertUnwindSafe(|| {
                    step(&mut f, i, finish, Some((stage, fault)))
                }));
                if panic {
                    assert_eq!(
                        result.unwrap_err().downcast_ref::<(&str, Stage)>(),
                        Some(&("control cleanup projection", stage))
                    );
                } else {
                    assert!(matches!(
                        result.unwrap(),
                        Err(MemorySessionError::Injected("control cleanup projection"))
                    ));
                }
                let after = f.snapshot();
                let owner = &after.controls[i];
                assert_eq!(owner.identity, original.controls[i].identity);
                assert_eq!(owner.layout, original.controls[i].layout);
                assert_eq!(owner.profile_type, original.controls[i].profile_type);
                assert_eq!(owner.stage, stage);
                assert!(owner.started && owner.failed);
                assert!(!f.controls[i].is_complete());
                assert_eq!(owner.native_disposed, stage == Stage::ReleaseCommit);
                assert_eq!(after.model, before.model, "failed step cannot commit");
                assert_eq!(after.usage, before.usage);
                assert_eq!(after.devices, before.devices);
                assert_eq!(after.storage, before.storage);
                assert_eq!(&after.controls[..i], &before.controls[..i]);
                assert_eq!(&after.controls[i + 1..], &before.controls[i + 1..]);
                let native_unmapped =
                    finish || matches!(stage, Stage::UnmapProjection | Stage::UnmapCommit);
                let disposed = stage == Stage::ReleaseCommit;
                assert_eq!(
                    record(&after, i),
                    &expected_record(
                        &original,
                        i,
                        native_unmapped,
                        if disposed {
                            4
                        } else {
                            usize::from(native_unmapped)
                        },
                        disposed,
                    )
                );
                assert_eq!(after.phase, SharedMemorySessionPhaseV1::Quarantined);
                reject_both_steps(&mut f, i);
            }
        }
    }
}

#[test]
fn split_cleanup_rechecks_revision_capacity_after_other_unmaps() {
    let mut f = Fixture::new(false);
    let model = &mut f.memory.fixture;
    model
        .foundation
        .mint_invariant_certificate(model.engine.session_id, model.device, model.vm)
        .unwrap();
    model
        .foundation
        .set_certificate_revision_for_test(u64::MAX - 3)
        .unwrap();
    unmap_all(&mut f);
    let before = f.snapshot();
    assert!(matches!(
        step(&mut f, 0, true, None),
        Err(MemorySessionError::Model(
            "queue foundation certificate revision exhausted"
        ))
    ));
    let after = f.snapshot();
    assert_eq!(after.model, before.model);
    assert_eq!(after.certificate, before.certificate);
    assert_eq!(after.records, before.records);
    assert_eq!(after.calls, before.calls);
    assert_eq!(after.currentness, before.currentness);
    assert_eq!(after.retained_va, before.retained_va);
    assert_eq!(after.usage, before.usage);
    assert_eq!(after.process_poisoned, before.process_poisoned + 1);
    assert_eq!(after.controls[0].stage, Stage::ReleasePreflight);
    assert!(after.controls[0].failed);
    assert_eq!(after.phase, SharedMemorySessionPhaseV1::Quarantined);
    reject_both_steps(&mut f, 0);
}

#[test]
fn split_cleanup_poison_callback_panic_preserves_original_payload_and_custody() {
    for finish in [false, true] {
        let mut f = Fixture::new(false);
        let model = &mut f.memory.fixture;
        model
            .foundation
            .mint_invariant_certificate(model.engine.session_id, model.device, model.vm)
            .unwrap();
        model
            .foundation
            .set_certificate_revision_for_test(u64::MAX - if finish { 3 } else { 0 })
            .unwrap();
        if finish {
            unmap_all(&mut f);
        }
        let before = f.snapshot();
        let result = catch_unwind(AssertUnwindSafe(|| {
            let model = &mut f.memory.fixture;
            let mut projection =
                control_cleanup::ProjectionV1::new(&mut model.foundation, model.vm);
            let poison = || {
                f.memory.process_poisoned += 1;
                std::panic::panic_any(("cleanup poison callback", 0x1234_u64));
            };
            if finish {
                control_cleanup::finish_release_v1(
                    &mut model.engine,
                    &mut projection,
                    &mut f.controls[0],
                    poison,
                )
            } else {
                control_cleanup::unmap_v1(
                    &mut model.engine,
                    &mut projection,
                    &mut f.controls[0],
                    poison,
                )
            }
        }));
        assert_eq!(
            result.unwrap_err().downcast_ref::<(&str, u64)>(),
            Some(&("cleanup poison callback", 0x1234))
        );
        let mut expected = before;
        expected.controls[0].started = true;
        expected.controls[0].failed = true;
        expected.controls[0].stage = if finish {
            Stage::ReleasePreflight
        } else {
            Stage::UnmapPreflight
        };
        expected.process_poisoned += 1;
        expected.phase = SharedMemorySessionPhaseV1::Quarantined;
        assert_eq!(f.snapshot(), expected);
        reject_both_steps(&mut f, 0);
    }
}

#[test]
fn split_cleanup_release_currentness_failure_preserves_successful_unmap_roster() {
    for i in 0..3 {
        for point in 1..=4 {
            for panic in [false, true] {
                let mut f = Fixture::new(true);
                let original = f.snapshot();
                unmap_all(&mut f);
                for j in 0..i {
                    step(&mut f, j, true, None).unwrap();
                }
                let before = f.snapshot();
                f.memory.fail_currentness(point, panic);
                let result = catch_unwind(AssertUnwindSafe(|| step(&mut f, i, true, None)));
                if panic {
                    assert_eq!(
                        result.unwrap_err().downcast_ref::<(&str, &str)>(),
                        Some(&("N2 native panic", "currentness"))
                    );
                } else {
                    assert!(matches!(
                        result.unwrap(),
                        Err(MemorySessionError::Injected("currentness"))
                    ));
                }
                let after = f.snapshot();
                assert_eq!(after.model, before.model);
                assert_eq!(after.currentness, before.currentness + point);
                assert_eq!(after.controls[i].identity, original.controls[i].identity);
                assert_eq!(after.controls[i].stage, Stage::NativeRelease);
                assert!(after.controls[i].failed);
                assert_eq!(after.controls[i].native_disposed, point == 4);
                assert_eq!(after.controls[i].unmap, (true, Some(true), Some(1)));
                assert_eq!(
                    after.controls[i].disposal,
                    std::array::from_fn(|j| {
                        if j < point - 1 {
                            (true, Some(true))
                        } else {
                            (false, None)
                        }
                    })
                );
                assert_eq!(
                    record(&after, i),
                    &expected_record(&original, i, true, point, false)
                );
                assert_eq!(&after.controls[..i], &before.controls[..i]);
                assert_eq!(&after.controls[i + 1..], &before.controls[i + 1..]);
                assert_eq!(after.usage, before.usage);
                assert_eq!(after.devices, before.devices);
                assert_eq!(after.phase, SharedMemorySessionPhaseV1::Quarantined);
                reject_both_steps(&mut f, i);
            }
        }
    }
}
