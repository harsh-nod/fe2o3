use super::*;
use crate::shared_memory::{SdmaResourceCleanupCustodyV1, queue_cleanup};

struct Fixture {
    memory: PristineAbortMemoryFixtureV1,
    custody: SdmaResourceCleanupCustodyV1,
}

impl Fixture {
    fn new() -> Self {
        let mut memory = PristineAbortMemoryFixtureV1::new();
        let ring = memory.allocate::<AqlQueueGttV1>(4096);
        let ring = memory.map(ring);
        let control = memory.allocate::<UserptrAqlControlGttV1>(4096);
        let control = memory.map(control);
        let completions = memory.allocate::<HostVisibleCoherentGttV1>(4096);
        let completions = memory.map(completions);
        Self {
            memory,
            custody: SdmaResourceCleanupCustodyV1::new_sdma(completions, control, ring),
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
    fn retry(&mut self) {
        let before = self.snapshot();
        let custody = self.custody.observation();
        assert!(matches!(
            self.release(None),
            Err(MemorySessionError::InvalidAllocationAuthority)
        ));
        assert_eq!(self.snapshot(), before);
        assert_eq!(self.custody.observation(), custody);
    }
    fn model(&self, before: &Snapshot, unmapped: usize, released: usize) -> MemoryLifecycleStateV1 {
        let mut model = before.model.clone();
        for c in before.controls.iter().take(unmapped) {
            let (_, _, map) =
                model_keys(self.memory.fixture.vm, c.identity.id, c.identity.generation);
            model = project_unmap(&model, map).unwrap();
        }
        for c in before.controls.iter().take(released) {
            let (reservation, allocation, map) =
                model_keys(self.memory.fixture.vm, c.identity.id, c.identity.generation);
            model = project_release(&model, reservation, allocation, map).unwrap();
        }
        model
    }
    fn identities(&self, before: &Snapshot) {
        let after = self.snapshot();
        assert_eq!(after.identity, before.identity);
        assert_eq!(after.devices, before.devices);
        assert_eq!(after.storage, before.storage);
        assert_eq!(after.usage.1, before.usage.1);
        for (old, new) in before.controls.iter().zip(&after.controls) {
            assert_eq!(
                (old.identity, old.layout, old.profile_type),
                (new.identity, new.layout, new.profile_type)
            );
            if !new.started {
                assert_eq!(old, new);
            }
        }
    }

    fn records(
        &self,
        before: &Snapshot,
        unmapped: usize,
        settled: usize,
        active: Option<(usize, usize, bool, bool)>,
    ) {
        assert_records(before, &self.snapshot(), unmapped, settled, active);
    }
}

fn assert_records(
    before: &Snapshot,
    after: &Snapshot,
    unmapped: usize,
    settled: usize,
    active: Option<(usize, usize, bool, bool)>,
) {
    let mut records = before.records.clone();
    let mut usage = before.usage;
    let mut retained = before.retained_va;
    for i in 0..3 {
        let r = records
            .iter_mut()
            .find(|r| r.id == before.controls[i].identity.id)
            .unwrap();
        let partial = active.filter(|a| a.0 == i);
        let calls = if i < settled {
            disposal(before, i).len()
        } else {
            partial.map_or(0, |a| a.1)
        };
        if i < unmapped {
            r.phase = SharedAllocationPhaseV1::CpuWritable;
        }
        if calls >= if i == 1 { 2 } else { 1 } {
            r.mapping = None;
        }
        if calls >= if i == 1 { 1 } else { 2 } {
            r.handle = None;
            r.free_attempted = true;
        }
        if partial.is_some_and(|a| a.3) {
            r.free_attempted = true;
        }
        if i < settled || partial.is_some_and(|a| a.2) {
            r.reservation = None;
        }
        if i < settled {
            r.phase = SharedAllocationPhaseV1::Released;
            r.indexed = None;
            retained -= r.layout.gpu_va_bytes();
            if r.charged {
                let host = usage.0.as_mut().unwrap();
                host.used_backing_bytes -= r.layout.gpu_va_bytes();
                host.used_allocation_records -= 1;
                host.retained_records -= 1;
                r.charged = false;
            }
        }
    }
    assert_eq!(after.records, records);
    assert_eq!(after.usage, usage);
    assert_eq!(after.retained_va, retained);
}

impl Snapshot {
    pub(crate) fn assert_sdma_late_cleanup_v1(
        &self,
        memory: &crate::shared_memory::PreparationMemoryFixtureV1,
        custody: &SdmaResourceCleanupCustodyV1,
        panic: bool,
    ) {
        let after = memory.primary_queue_cleanup_snapshot_v1(custody);
        assert_eq!(after.calls, calls(self));
        assert_records(self, &after, 3, 2, Some((2, 2, false, false)));
        let state = custody.observation();
        assert_eq!(
            (state.started, state.failed, state.unmapped, state.released),
            (true, true, 3, 2)
        );
        assert_eq!(
            state.controls[2].disposal,
            [
                (true, Some(true)),
                (true, Some(true)),
                (true, (!panic).then_some(false))
            ]
        );
        assert_eq!(state.controls[0].owner, "NativeDisposed");
        assert_eq!(state.controls[1].owner, "NativeDisposed");
        assert_eq!(state.controls[2].owner, "Unmapped");
        let mut model = self.model.clone();
        for control in &self.controls {
            let (_, _, mapping) = model_keys(
                memory.fixture.vm,
                control.identity.id,
                control.identity.generation,
            );
            model = project_unmap(&model, mapping).unwrap();
        }
        for control in self.controls.iter().take(2) {
            let (reservation, allocation, mapping) = model_keys(
                memory.fixture.vm,
                control.identity.id,
                control.identity.generation,
            );
            model = project_release(&model, reservation, allocation, mapping).unwrap();
        }
        assert_eq!(after.model, model);
    }
}

fn disposal(before: &Snapshot, index: usize) -> Vec<CleanupCallV1> {
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

fn calls(before: &Snapshot) -> Vec<CleanupCallV1> {
    let mut calls = before.calls.clone();
    for i in 0..3 {
        calls.push(CleanupCallV1::UnmapGpu(
            record(before, i).handle.unwrap(),
            0,
        ));
    }
    for i in 0..3 {
        calls.extend(disposal(before, i));
    }
    calls
}

#[test]
fn sdma_cleanup_exact_two_phase_order_and_account_refund() {
    let mut f = Fixture::new();
    let before = f.snapshot();
    f.release(None).unwrap();
    let after = f.snapshot();
    assert!(f.custody.is_complete());
    assert_eq!(after.calls, calls(&before));
    assert_eq!(after.calls.len(), 11);
    assert_eq!(after.currentness - before.currentness, 18);
    assert_eq!(after.model, f.model(&before, 3, 3));
    assert_eq!(after.retained_va, 0);
    assert_eq!(after.usage.0.unwrap().used_backing_bytes, 0);
    for c in &after.controls {
        assert_eq!(c.owner, "NativeDisposed");
        assert_eq!(c.stage, Stage::Complete);
    }
    assert_eq!(after.controls[1].disposal[2], (false, None));
    f.identities(&before);
    f.records(&before, 3, 3, None);
    f.retry();
}

#[test]
fn sdma_cleanup_all_native_failures_retain_exact_prefix_and_panic() {
    for failed in 0..11 {
        for panic in [false, true] {
            let mut f = Fixture::new();
            let before = f.snapshot();
            let all = calls(&before);
            let op = match all[failed] {
                CleanupCallV1::UnmapGpu(..) => "unmap_gpu",
                CleanupCallV1::UnmapCpu(..) => "unmap_cpu",
                CleanupCallV1::Free(..) => "free",
                CleanupCallV1::ReleaseVa(..) => "release_va_reservation",
            };
            f.memory.fixture.engine.backend.cleanup_fault = Some((failed + 1, op, panic));
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
            let index = if failed < 3 {
                failed
            } else if failed < 6 {
                0
            } else if failed < 8 {
                1
            } else {
                2
            };
            let unmapped = if failed < 3 { index } else { 3 };
            let released = if failed < 3 { 0 } else { index };
            assert_eq!((state.unmapped, state.released), (unmapped, released));
            assert_eq!(after.calls, all[..=failed]);
            assert_eq!(after.model, f.model(&before, unmapped, released));
            let offset = if failed < 3 {
                failed
            } else {
                3 + (0..index)
                    .map(|i| disposal(&before, i).len())
                    .sum::<usize>()
            };
            f.records(
                &before,
                unmapped,
                released,
                Some((index, failed - offset, false, op == "free")),
            );
            assert!(state.failed && state.controls[index].failed);
            for i in 0..3 {
                assert_eq!(
                    after.controls[i].owner,
                    if i < released {
                        "NativeDisposed"
                    } else if i < unmapped {
                        "Unmapped"
                    } else {
                        "Mapped"
                    }
                );
            }
            assert_eq!(
                after.usage.0.unwrap().used_backing_bytes,
                if released > 0 {
                    0
                } else {
                    before.usage.0.unwrap().used_backing_bytes
                }
            );
            f.identities(&before);
            f.retry();
        }
    }
}

#[test]
fn sdma_cleanup_all_currentness_boundaries_retain_unsettled_disposal() {
    for point in 1..=18 {
        for panic in [false, true] {
            let mut f = Fixture::new();
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
            let release = point > 6;
            let index = if release {
                (point - 7) / 4
            } else {
                (point - 1) / 2
            };
            let at = if release {
                (point - 7) % 4 + 1
            } else {
                (point - 1) % 2 + 1
            };
            let unmapped = if release { 3 } else { index };
            let released = if release { index } else { 0 };
            let count = if release {
                3 + (0..released)
                    .map(|i| disposal(&before, i).len())
                    .sum::<usize>()
                    + (at - 1).min(disposal(&before, index).len())
            } else {
                index + usize::from(at == 2)
            };
            let after = f.snapshot();
            let state = f.custody.observation();
            assert_eq!((state.unmapped, state.released), (unmapped, released));
            assert_eq!(after.calls, calls(&before)[..count]);
            assert_eq!(after.model, f.model(&before, unmapped, released));
            assert_eq!(after.currentness - before.currentness, point);
            f.records(
                &before,
                unmapped,
                released,
                Some((
                    index,
                    if release {
                        (at - 1).min(disposal(&before, index).len())
                    } else {
                        0
                    },
                    release && at == 4,
                    false,
                )),
            );
            assert_eq!(
                after.controls[index].owner,
                if !release {
                    "Mapped"
                } else if at == 4 {
                    "NativeDisposed"
                } else {
                    "Unmapped"
                }
            );
            assert_eq!(after.controls[index].native_disposed, release && at == 4);
            assert_eq!(
                after.usage.0.unwrap().used_backing_bytes,
                if released > 0 {
                    0
                } else {
                    before.usage.0.unwrap().used_backing_bytes
                }
            );
            f.identities(&before);
            f.retry();
        }
    }
}

#[test]
fn sdma_cleanup_projection_and_commit_boundaries_keep_native_receipts_separate() {
    for index in 0..3 {
        for stage in [
            Stage::UnmapProjection,
            Stage::UnmapCommit,
            Stage::ReleaseProjection,
            Stage::ReleaseCommit,
        ] {
            for fault in [Fault::Error, Fault::Panic] {
                let mut f = Fixture::new();
                let before = f.snapshot();
                let result =
                    catch_unwind(AssertUnwindSafe(|| f.release(Some((index, stage, fault)))));
                if matches!(fault, Fault::Panic) {
                    assert_eq!(
                        result.unwrap_err().downcast_ref::<(&str, Stage)>(),
                        Some(&("control cleanup projection", stage))
                    );
                } else {
                    assert!(result.unwrap().is_err());
                }
                let unmap = matches!(stage, Stage::UnmapProjection | Stage::UnmapCommit);
                let disposed = stage == Stage::ReleaseCommit;
                let after = f.snapshot();
                let unmapped = if unmap { index } else { 3 };
                let released = if unmap { 0 } else { index };
                let count = if unmap {
                    index + 1
                } else {
                    3 + (0..(released + usize::from(disposed)))
                        .map(|i| disposal(&before, i).len())
                        .sum::<usize>()
                };
                assert_eq!(after.calls, calls(&before)[..count]);
                assert_eq!(after.model, f.model(&before, unmapped, released));
                f.records(
                    &before,
                    if unmap { index + 1 } else { 3 },
                    released + usize::from(disposed),
                    None,
                );
                assert_eq!(
                    after.controls[index].owner,
                    if unmap || !disposed {
                        "Unmapped"
                    } else {
                        "NativeDisposed"
                    }
                );
                assert_eq!(after.controls[index].native_disposed, disposed);
                assert_eq!(
                    after.usage.0.unwrap().used_backing_bytes,
                    if released > 0 || disposed {
                        0
                    } else {
                        before.usage.0.unwrap().used_backing_bytes
                    }
                );
                f.identities(&before);
                f.retry();
            }
        }
    }
}

#[test]
fn sdma_cleanup_requires_six_revision_headroom_before_native_effects() {
    for headroom in 0..=6 {
        let mut f = Fixture::new();
        let m = &mut f.memory.fixture;
        m.foundation
            .mint_invariant_certificate(m.engine.session_id, m.device, m.vm)
            .unwrap();
        m.foundation
            .set_certificate_revision_for_test(u64::MAX - headroom)
            .unwrap();
        let before = f.snapshot();
        let result = f.release(None);
        if headroom == 6 {
            result.unwrap();
            assert!(f.custody.is_complete());
        } else {
            assert!(matches!(
                result,
                Err(MemorySessionError::Model(
                    "queue foundation certificate revision exhausted"
                ))
            ));
            let after = f.snapshot();
            assert_eq!(after.calls, before.calls);
            assert_eq!(after.currentness, before.currentness);
            assert_eq!(after.model, before.model);
            assert_eq!(after.controls, before.controls);
            assert_eq!(after.usage, before.usage);
            assert_eq!(after.process_poisoned, before.process_poisoned + 1);
            assert!(f.custody.observation().failed);
        }
        f.retry();
    }
}
