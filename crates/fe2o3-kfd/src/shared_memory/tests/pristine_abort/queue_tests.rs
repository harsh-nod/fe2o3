use super::*;
use crate::shared_memory::queue_cleanup::{self, QueueResourceCleanupCustodyV1};

struct QueueFixture {
    memory: PristineAbortMemoryFixtureV1,
    custody: QueueResourceCleanupCustodyV1,
    _host: Host,
    _device: Gfx942DeviceMemoryDispatchAuthorityV1,
}

impl QueueFixture {
    fn new() -> Self {
        let mut memory = PristineAbortMemoryFixtureV1::new();
        let ring = memory.allocate::<AqlQueueGttV1>(4096);
        let ring = memory.map(ring);
        let ring = memory.retain::<AqlRingResourceRoleV1, _, _>(ring);
        let control = memory.allocate::<UserptrAqlControlGttV1>(4096);
        let control = memory.map(control);
        let control = memory.retain::<AqlControlResourceRoleV1, _, _>(control);
        let eop = memory.executable::<AqlEndOfPipeResourceRoleV1>();
        let context = memory.executable::<AqlContextSaveResourceRoleV1>();
        let host = memory.host();
        let device = memory.device();
        Self {
            custody: QueueResourceCleanupCustodyV1::new(
                ring.into_token(),
                control.into_token(),
                eop.into_token(),
                context.into_token(),
            ),
            memory,
            _host: host,
            _device: device,
        }
    }

    fn snapshot(&self) -> Snapshot {
        let mut snapshot = self.memory.memory_snapshot();
        snapshot.controls = self.custody.observation().controls.into();
        snapshot
    }

    fn release(&mut self, fault: Option<(usize, Stage, Fault)>) -> Result<(), MemorySessionError> {
        let f = &mut self.memory.fixture;
        let mut projection = control_cleanup::ProjectionV1::new(&mut f.foundation, f.vm);
        if let Some((index, stage, fault)) = fault {
            projection.fault = Some((stage, fault));
            projection.skip_fault_matches = index;
        }
        queue_cleanup::release_v1(&mut f.engine, &mut projection, &mut self.custody, || {
            self.memory.process_poisoned += 1;
        })
    }

    fn reject_retry(&mut self) {
        let b = &mut self.memory.fixture.engine.backend;
        b.cleanup_fault = None;
        b.fail_currentness_at = None;
        b.panic_currentness_at = None;
        b.unmap_outcome_at = None;
        let before = self.snapshot();
        let custody = self.custody.observation();
        assert!(matches!(
            self.release(None),
            Err(MemorySessionError::InvalidAllocationAuthority)
        ));
        assert_eq!(self.snapshot(), before);
        assert_eq!(self.custody.observation(), custody);
    }

    fn check_retention(&self, before: &Snapshot) {
        let after = self.snapshot();
        assert_eq!(after.identity, before.identity);
        assert_eq!(after.usage, before.usage);
        assert_eq!(after.devices, before.devices);
        assert_eq!(after.storage, before.storage);
        for (index, owner) in after.controls.iter().enumerate() {
            assert_eq!(owner.identity, before.controls[index].identity);
            assert_eq!(owner.profile_type, before.controls[index].profile_type);
            assert_eq!(owner.layout, before.controls[index].layout);
            if !owner.started {
                assert_eq!(owner, &before.controls[index]);
            }
        }
        for before in &before.records {
            if !after.controls.iter().any(|c| c.identity.id == before.id) {
                assert_eq!(
                    after.records.iter().find(|r| r.id == before.id).unwrap(),
                    before
                );
            }
        }
    }
}

fn release_calls(before: &Snapshot, index: usize) -> Vec<CleanupCallV1> {
    let r = record(before, index);
    let m = r.mapping.as_ref().unwrap();
    let cpu = CleanupCallV1::UnmapCpu(m.address, m.pointer, m.bytes.len());
    let free = CleanupCallV1::Free(r.handle.unwrap());
    if index == 1 {
        assert!(r.userptr);
        vec![free, cpu]
    } else {
        assert!(!r.userptr);
        let va = r.reservation.unwrap();
        vec![cpu, free, CleanupCallV1::ReleaseVa(va.0, va.1)]
    }
}

fn all_calls(before: &Snapshot) -> Vec<CleanupCallV1> {
    let mut calls = before.calls.clone();
    for i in 0..4 {
        calls.push(CleanupCallV1::UnmapGpu(
            record(before, i).handle.unwrap(),
            0,
        ));
    }
    for i in 0..4 {
        calls.extend(release_calls(before, i));
    }
    calls
}

fn disposal_progress(
    index: usize,
    succeeded: usize,
    failed: Option<bool>,
) -> [(bool, Option<bool>); 3] {
    let order: &[usize] = if index == 1 { &[1, 0] } else { &[0, 1, 2] };
    assert!(succeeded <= order.len());
    let mut progress = [(false, None); 3];
    for &slot in order.iter().take(succeeded) {
        progress[slot] = (true, Some(true));
    }
    if let Some(panic) = failed {
        assert!(succeeded < order.len());
        progress[order[succeeded]] = (true, (!panic).then_some(false));
    }
    progress
}

fn retained_va_after(before: &Snapshot, refunded: usize) -> u64 {
    before.retained_va
        - before
            .controls
            .iter()
            .take(refunded)
            .map(|c| c.layout.gpu_va_bytes())
            .sum::<u64>()
}

fn expected_model(
    f: &QueueFixture,
    before: &Snapshot,
    unmapped: usize,
    released: usize,
) -> MemoryLifecycleStateV1 {
    let mut model = before.model.clone();
    for c in before.controls.iter().take(unmapped) {
        let (_, _, mapping) = model_keys(f.memory.fixture.vm, c.identity.id, c.identity.generation);
        model = project_unmap(&model, mapping).unwrap();
    }
    for c in before.controls.iter().take(released) {
        let (reservation, allocation, mapping) =
            model_keys(f.memory.fixture.vm, c.identity.id, c.identity.generation);
        model = project_release(&model, reservation, allocation, mapping).unwrap();
    }
    model
}

fn expected_queue_record(
    before: &Snapshot,
    index: usize,
    unmapped: bool,
    native_calls: usize,
    disposed: bool,
    settled: bool,
) -> RecordSnapshot {
    let mut r = record(before, index).clone();
    if unmapped {
        r.phase = if index < 2 {
            SharedAllocationPhaseV1::CpuWritable
        } else {
            SharedAllocationPhaseV1::ExecutableImmutable
        };
    }
    if index == 1 {
        if native_calls >= 1 {
            r.free_attempted = true;
            r.handle = None;
        }
        if native_calls >= 2 {
            r.mapping = None;
        }
    } else {
        if native_calls >= 1 {
            r.mapping = None;
        }
        if native_calls >= 2 {
            r.free_attempted = true;
            r.handle = None;
        }
    }
    if disposed {
        r.reservation = None;
    }
    if settled {
        r.phase = SharedAllocationPhaseV1::Released;
        r.indexed = None;
    }
    r
}

impl Snapshot {
    pub(crate) fn assert_constructed_queue_failure_v1(
        &self,
        memory: &crate::shared_memory::PreparationMemoryFixtureV1,
        custody: &QueueResourceCleanupCustodyV1,
        failed_call: usize,
        panic: bool,
    ) {
        let after = memory.primary_queue_cleanup_snapshot_v1(custody);
        let state = custody.observation();
        let calls = all_calls(self);
        assert_eq!(after.calls, calls[..=self.calls.len() + failed_call]);
        let (index, done) = if failed_call < 4 {
            (failed_call, 0)
        } else {
            let mut offset = 4;
            (0..4)
                .find_map(|i| {
                    let count = release_calls(self, i).len();
                    if failed_call < offset + count {
                        Some((i, failed_call - offset))
                    } else {
                        offset += count;
                        None
                    }
                })
                .unwrap()
        };
        let released = if failed_call < 4 { 0 } else { index };
        let unmapped = if failed_call < 4 { index } else { 4 };
        assert_eq!(
            (state.started, state.failed, state.unmapped, state.released),
            (true, true, unmapped, released)
        );
        let mut model = self.model.clone();
        for (i, c) in self.controls.iter().enumerate().take(unmapped) {
            let (reservation, allocation, mapping) =
                model_keys(memory.fixture.vm, c.identity.id, c.identity.generation);
            model = project_unmap(&model, mapping).unwrap();
            if i < released {
                model = project_release(&model, reservation, allocation, mapping).unwrap();
            }
        }
        assert_eq!(after.model, model);
        assert_eq!(after.retained_va, retained_va_after(self, released));
        assert_eq!(after.identity, self.identity);
        assert_eq!(after.certificate, self.certificate);
        assert_eq!(after.devices, self.devices);
        assert_eq!(after.usage, self.usage);
        assert_eq!(after.storage, self.storage);
        for i in 0..4 {
            assert_eq!(after.controls[i].identity, self.controls[i].identity);
            assert_eq!(
                after.controls[i].disposal,
                if i < released {
                    disposal_progress(i, release_calls(self, i).len(), None)
                } else if failed_call >= 4 && i == index {
                    disposal_progress(i, done, Some(panic))
                } else {
                    [(false, None); 3]
                }
            );
            let mut expected = expected_queue_record(
                self,
                i,
                i < unmapped,
                if i < released {
                    release_calls(self, i).len()
                } else if i == index {
                    done
                } else {
                    0
                },
                i < released,
                i < released,
            );
            if i == index
                && matches!(
                    calls[self.calls.len() + failed_call],
                    CleanupCallV1::Free(_)
                )
            {
                expected.free_attempted = true;
            }
            assert_eq!(record(&after, i), &expected);
            if !after.controls[i].started {
                assert_eq!(after.controls[i], self.controls[i]);
            }
        }
        for r in &self.records {
            if !self.controls.iter().any(|c| c.identity.id == r.id) {
                assert_eq!(
                    after.records.iter().find(|a| a.id == r.id).unwrap(),
                    r,
                    "untouched dispatch and signal record"
                );
            }
        }
        let failing = &after.controls[index];
        assert!(failing.failed && !failing.native_disposed);
        if failed_call < 4 {
            assert_eq!(
                failing.unmap,
                (true, (!panic).then_some(false), (!panic).then_some(1))
            );
        }
    }
}

#[test]
fn queue_cleanup_four_real_resources_keep_two_phase_order_and_unrelated_backing() {
    let mut f = QueueFixture::new();
    let before = f.snapshot();
    let watermark = (
        f.memory.fixture.engine.next_id,
        f.memory.fixture.ownership.next_live_loan_generation,
    );
    f.release(None).unwrap();
    let after = f.snapshot();
    assert!(f.custody.is_complete());
    let state = f.custody.observation();
    assert_eq!(
        (state.started, state.failed, state.unmapped, state.released),
        (true, false, 4, 4)
    );
    assert_eq!(after.calls, all_calls(&before));
    assert_eq!(after.model, expected_model(&f, &before, 4, 4));
    let bytes: u64 = before
        .controls
        .iter()
        .map(|c| c.layout.gpu_va_bytes())
        .sum();
    assert_eq!(after.retained_va, before.retained_va - bytes);
    for i in 0..4 {
        assert_eq!(
            record(&after, i),
            &expected_queue_record(
                &before,
                i,
                true,
                release_calls(&before, i).len(),
                true,
                true
            )
        );
        assert_eq!(after.controls[i].owner, "NativeDisposed");
        assert_eq!(
            after.controls[i].disposal,
            disposal_progress(i, release_calls(&before, i).len(), None)
        );
        assert_eq!(after.controls[i].stage, Stage::Complete);
        assert_eq!(
            after.controls[i].state_type,
            if i < 2 {
                std::any::TypeId::of::<GttCpuWritableV1>()
            } else {
                std::any::TypeId::of::<GttExecutableImmutableV1>()
            }
        );
    }
    assert_eq!(
        after.controls[1].disposal,
        [(true, Some(true)), (true, Some(true)), (false, None)]
    );
    assert_eq!(
        (
            f.memory.fixture.engine.next_id,
            f.memory.fixture.ownership.next_live_loan_generation
        ),
        watermark
    );
    f.check_retention(&before);
    f.reject_retry();
}

#[test]
fn queue_cleanup_native_failures_preserve_exact_batch_prefix_and_original_panic() {
    for failed_call in 0..15 {
        for panic in [false, true] {
            let mut f = QueueFixture::new();
            let before = f.snapshot();
            let calls = all_calls(&before);
            assert!(before.calls.is_empty());
            let op = match calls[failed_call] {
                CleanupCallV1::UnmapGpu(..) => "unmap_gpu",
                CleanupCallV1::UnmapCpu(..) => "unmap_cpu",
                CleanupCallV1::Free(..) => "free",
                CleanupCallV1::ReleaseVa(..) => "release_va_reservation",
            };
            f.memory.fixture.engine.backend.cleanup_fault = Some((failed_call + 1, op, panic));
            let result = catch_unwind(AssertUnwindSafe(|| f.release(None)));
            if panic {
                assert_eq!(
                    result.unwrap_err().downcast_ref::<(&str, &str)>(),
                    Some(&("N2 native panic", op))
                );
            } else {
                assert!(
                    matches!(result.unwrap(), Err(MemorySessionError::Injected(got)) if got == op)
                );
            }
            let after = f.snapshot();
            let state = f.custody.observation();
            assert!(state.started && state.failed && !f.custody.is_complete());
            let (index, done) = if failed_call < 4 {
                (failed_call, 0)
            } else {
                let mut offset = 4;
                (0..4)
                    .find_map(|i| {
                        let count = release_calls(&before, i).len();
                        if failed_call < offset + count {
                            Some((i, failed_call - offset))
                        } else {
                            offset += count;
                            None
                        }
                    })
                    .unwrap()
            };
            let released = if failed_call < 4 { 0 } else { index };
            let unmapped = if failed_call < 4 { index } else { 4 };
            assert_eq!((state.unmapped, state.released), (unmapped, released));
            assert_eq!(after.calls, calls[..=failed_call]);
            assert_eq!(after.model, expected_model(&f, &before, unmapped, released));
            assert_eq!(after.retained_va, retained_va_after(&before, released));
            for i in 0..4 {
                assert_eq!(
                    after.controls[i].disposal,
                    if i < released {
                        disposal_progress(i, release_calls(&before, i).len(), None)
                    } else if failed_call >= 4 && i == index {
                        disposal_progress(i, done, Some(panic))
                    } else {
                        [(false, None); 3]
                    }
                );
                let mut expected = expected_queue_record(
                    &before,
                    i,
                    i < unmapped,
                    if i < released {
                        release_calls(&before, i).len()
                    } else if i == index {
                        done
                    } else {
                        0
                    },
                    i < released,
                    i < released,
                );
                if i == index && op == "free" {
                    expected.free_attempted = true;
                }
                assert_eq!(record(&after, i), &expected);
            }
            let owner = &state.controls[index];
            assert!(owner.failed && !owner.native_disposed);
            assert_eq!(
                owner.stage,
                if failed_call < 4 {
                    Stage::NativeUnmap
                } else {
                    Stage::NativeRelease
                }
            );
            if failed_call < 4 {
                assert_eq!(
                    owner.unmap,
                    (true, (!panic).then_some(false), (!panic).then_some(1))
                );
            }
            if index == 1 {
                assert_eq!(owner.disposal[2], (false, None));
            }
            assert_eq!(after.phase, SharedMemorySessionPhaseV1::Quarantined);
            f.check_retention(&before);
            f.reject_retry();
        }
    }
}

#[test]
fn queue_cleanup_currentness_sweep_preserves_userptr_reservation_and_disposal_boundaries() {
    for point in 1..=24 {
        for panic in [false, true] {
            let mut f = QueueFixture::new();
            let before = f.snapshot();
            f.memory.fail_currentness(point, panic);
            let result = catch_unwind(AssertUnwindSafe(|| f.release(None)));
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
            let state = f.custody.observation();
            let release = point > 8;
            let index = if release {
                (point - 9) / 4
            } else {
                (point - 1) / 2
            };
            let at = if release {
                (point - 9) % 4 + 1
            } else {
                (point - 1) % 2 + 1
            };
            let unmapped = if release { 4 } else { index };
            let released = if release { index } else { 0 };
            assert_eq!((state.unmapped, state.released), (unmapped, released));
            assert!(state.failed);
            assert_eq!(after.model, expected_model(&f, &before, unmapped, released));
            assert_eq!(after.currentness, before.currentness + point);
            let successful = if release {
                (at - 1).min(release_calls(&before, index).len())
            } else {
                0
            };
            assert_eq!(after.retained_va, retained_va_after(&before, released));
            let calls = if release {
                4 + (0..released)
                    .map(|i| release_calls(&before, i).len())
                    .sum::<usize>()
                    + successful
            } else {
                index + usize::from(at == 2)
            };
            assert_eq!(
                after.calls,
                all_calls(&before)[..before.calls.len() + calls]
            );
            for i in 0..4 {
                assert_eq!(
                    after.controls[i].disposal,
                    if i < released {
                        disposal_progress(i, release_calls(&before, i).len(), None)
                    } else if release && i == index {
                        disposal_progress(i, successful, None)
                    } else {
                        [(false, None); 3]
                    }
                );
            }
            assert_eq!(
                record(&after, index),
                &expected_queue_record(
                    &before,
                    index,
                    release,
                    successful,
                    release && at == 4,
                    false
                )
            );
            assert_eq!(state.controls[index].native_disposed, release && at == 4);
            assert_eq!(
                state.controls[index].owner,
                if !release {
                    "Mapped"
                } else if at == 4 {
                    "NativeDisposed"
                } else {
                    "Unmapped"
                }
            );
            if index == 1 {
                assert_eq!(state.controls[index].disposal[2], (false, None));
            }
            for i in 0..4 {
                if i != index {
                    assert_eq!(
                        record(&after, i),
                        &expected_queue_record(
                            &before,
                            i,
                            i < unmapped,
                            if i < released {
                                release_calls(&before, i).len()
                            } else {
                                0
                            },
                            i < released,
                            i < released
                        )
                    );
                }
            }
            assert_eq!(after.phase, SharedMemorySessionPhaseV1::Quarantined);
            f.check_retention(&before);
            f.reject_retry();
        }
    }
}

#[test]
fn queue_cleanup_model_failure_keeps_native_success_separate_from_committed_prefix() {
    for index in 0..4 {
        for stage in [
            Stage::UnmapProjection,
            Stage::UnmapCommit,
            Stage::ReleaseProjection,
            Stage::ReleaseCommit,
        ] {
            for panic in [false, true] {
                let mut f = QueueFixture::new();
                let before = f.snapshot();
                let result = catch_unwind(AssertUnwindSafe(|| {
                    f.release(Some((
                        index,
                        stage,
                        if panic { Fault::Panic } else { Fault::Error },
                    )))
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
                let state = f.custody.observation();
                let release = matches!(stage, Stage::ReleaseProjection | Stage::ReleaseCommit);
                let disposed = stage == Stage::ReleaseCommit;
                assert_eq!(
                    (state.unmapped, state.released),
                    if release { (4, index) } else { (index, 0) }
                );
                assert!(state.failed && state.controls[index].failed);
                assert_eq!(state.controls[index].native_disposed, disposed);
                assert_eq!(state.controls[index].stage, stage);
                assert_eq!(
                    after.retained_va,
                    retained_va_after(&before, state.released + usize::from(disposed))
                );
                for i in 0..4 {
                    assert_eq!(
                        after.controls[i].disposal,
                        if i < state.released || (disposed && i == index) {
                            disposal_progress(i, release_calls(&before, i).len(), None)
                        } else {
                            [(false, None); 3]
                        }
                    );
                }
                assert_eq!(
                    after.model,
                    expected_model(&f, &before, state.unmapped, state.released)
                );
                assert_eq!(
                    record(&after, index),
                    &expected_queue_record(
                        &before,
                        index,
                        true,
                        if disposed {
                            release_calls(&before, index).len()
                        } else {
                            0
                        },
                        disposed,
                        disposed
                    )
                );
                f.check_retention(&before);
                f.reject_retry();
            }
        }
    }
}

#[test]
fn queue_cleanup_unmap_partial_results_never_retag_or_advance_failed_resource() {
    for index in 0..4 {
        for (prefix, errno) in [(0, false), (0, true), (1, true), (2, false), (2, true)] {
            let mut f = QueueFixture::new();
            let before = f.snapshot();
            let at = f.memory.fixture.engine.backend.unmap_gpu_calls + index + 1;
            f.memory.fixture.engine.backend.unmap_outcome_at = Some((at, prefix, errno));
            let error = f.release(None).unwrap_err();
            match error {
                MemorySessionError::Injected("unmap_gpu") => assert!(prefix <= 1 && errno),
                MemorySessionError::KernelResultMalformed(detail) => assert_eq!(
                    detail,
                    if prefix > 1 {
                        "shared UNMAP_MEMORY_FROM_GPU cumulative n_success"
                    } else {
                        "shared UNMAP_MEMORY_FROM_GPU full prefix"
                    }
                ),
                other => panic!("unexpected error: {other:?}"),
            }
            let after = f.snapshot();
            let state = f.custody.observation();
            assert_eq!((state.unmapped, state.released), (index, 0));
            assert!(state.failed);
            assert_eq!(
                state.controls[index].unmap,
                (true, Some(!errno), Some(prefix))
            );
            assert_eq!(state.controls[index].owner, "Mapped");
            assert_eq!(after.retained_va, before.retained_va);
            for control in &after.controls {
                assert_eq!(control.disposal, [(false, None); 3]);
            }
            assert_eq!(after.model, expected_model(&f, &before, index, 0));
            assert_eq!(record(&after, index), record(&before, index));
            assert_eq!(&after.controls[index + 1..], &before.controls[index + 1..]);
            f.check_retention(&before);
            f.reject_retry();
        }
    }
}

#[test]
fn queue_cleanup_certified_revision_boundaries_preserve_exact_prefix() {
    for headroom in 0_usize..=8 {
        let mut f = QueueFixture::new();
        let m = &mut f.memory.fixture;
        m.foundation
            .mint_invariant_certificate(m.engine.session_id, m.device, m.vm)
            .unwrap();
        m.foundation
            .set_certificate_revision_for_test(u64::MAX - headroom as u64)
            .unwrap();
        let before = f.snapshot();
        let result = f.release(None);
        let after = f.snapshot();
        let state = f.custody.observation();
        let unmapped = headroom.min(4);
        let released = headroom.saturating_sub(4);
        let complete = headroom == 8;
        if complete {
            result.unwrap();
        } else {
            assert!(matches!(
                result,
                Err(MemorySessionError::Model(
                    "queue foundation certificate revision exhausted"
                ))
            ));
            let index = if headroom < 4 { headroom } else { released };
            assert!(state.controls[index].failed);
            assert_eq!(
                state.controls[index].stage,
                if headroom < 4 {
                    Stage::UnmapPreflight
                } else {
                    Stage::ReleasePreflight
                }
            );
        }
        assert_eq!((state.started, state.failed), (true, !complete));
        assert_eq!((state.unmapped, state.released), (unmapped, released));
        assert_eq!(f.custody.is_complete(), complete);
        assert_eq!(after.model, expected_model(&f, &before, unmapped, released));
        let calls = unmapped
            + (0..released)
                .map(|i| release_calls(&before, i).len())
                .sum::<usize>();
        assert_eq!(
            after.calls,
            all_calls(&before)[..before.calls.len() + calls]
        );
        assert_eq!(
            after.currentness,
            before.currentness + 2 * unmapped + 4 * released
        );
        assert_eq!(after.retained_va, retained_va_after(&before, released));
        assert_eq!(
            after.process_poisoned,
            before.process_poisoned + usize::from(!complete)
        );
        let mut certificate = before.certificate.unwrap();
        certificate.6 = u64::MAX;
        certificate.7 = after.certificate.as_ref().unwrap().7;
        let memory = &f.memory.fixture;
        memory
            .foundation
            .authenticate(
                memory.engine.session_id,
                memory.device,
                memory.vm,
                certificate.0,
            )
            .unwrap();
        assert_eq!(after.certificate, Some(certificate));
        assert_eq!(
            after.phase,
            if complete {
                SharedMemorySessionPhaseV1::Active
            } else {
                SharedMemorySessionPhaseV1::Quarantined
            }
        );
        for i in 0..4 {
            let count = if i < released {
                release_calls(&before, i).len()
            } else {
                0
            };
            assert_eq!(
                record(&after, i),
                &expected_queue_record(&before, i, i < unmapped, count, i < released, i < released)
            );
            assert_eq!(
                after.controls[i].disposal,
                disposal_progress(i, count, None)
            );
            assert_eq!(
                after.controls[i].unmap,
                if i < unmapped {
                    (true, Some(true), Some(1))
                } else {
                    (false, None, None)
                }
            );
        }
        f.check_retention(&before);
        f.reject_retry();
    }
}
