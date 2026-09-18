//! The production release driver consumes the original completed constructor.

use super::*;
use crate::queue::dispatch_binding::control_release::RetainedControlSnapshotV1;
use crate::queue::live::primary_release::{
    PrimaryReleaseMemoryV1, PrimaryReleaseParentV1, PrimaryReleasePartsV1, PrimaryReleaseStateV1,
    preflight_primary_owners_v1, primary_dispatch_release_admitted_v1,
};
use crate::queue_linux::primary_fixture::LocalRuntimeObservationV1;
use crate::shared_memory::{ControlCleanupCustodyV1, QueueResourceCleanupCustodyV1};

#[path = "integration_release_detached_tests.rs"]
mod detached_cases;
#[path = "integration_release_fault_tests.rs"]
mod fault_cases;
#[path = "integration_release_generic_sdma_tests.rs"]
mod generic_sdma_cases;
#[path = "integration_initial_bind_tests.rs"]
mod initial_bind_cases;
#[path = "integration_release_late_tests.rs"]
mod late_cases;
#[path = "integration_pool_trim_tests.rs"]
mod pool_trim_cases;
#[path = "integration_sdma_allocation_tests.rs"]
mod sdma_allocation_cases;
#[path = "integration_release_sdma_tests.rs"]
mod sdma_cases;
#[path = "integration_sdma_creation_tests.rs"]
mod sdma_creation_cases;

struct Parent {
    engine: NativeQueueEngineV1<PrimaryQueueBackendV1<Memory>>,
    key: QueueKeyV1,
    queue_id: u32,
    exception: Option<QueueExceptionStateV1<Fixture>>,
    doorbell: Option<Owner>,
    dispatch: Option<DispatchResourceOwnerV1>,
    signals: Option<CompletionSignalAuthority>,
    sdma: Option<Gfx942SdmaQueueSetV1>,
    submission: NativeAqlSubmissionOwnerV1,
    completion: CompletionSignalArenaOwnerV1,
    dependency: ComputeDependencySessionOwnerV1,
    unpublished: UnpublishedDispatchStateV1,
    detached_count: usize,
    detached_generation: Option<u64>,
    detached_identities: Vec<Gfx942FixedDispatchStorageIdentityV1>,
    detached_next: Option<usize>,
    poisoned: bool,
}

impl PrimaryReleaseParentV1<Fixture> for Parent {
    fn preflight_release(&self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if self.poisoned {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "fixture parent poisoned",
            ));
        }
        if !primary_dispatch_release_admitted_v1(
            &self.unpublished,
            self.dispatch.is_some(),
            self.detached_generation,
            self.detached_count,
            self.detached_identities.len(),
            self.detached_next,
        ) {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "unsupported or busy primary release",
            ));
        }
        self.dependency
            .ensure_idle()
            .map_err(map_dependency_target_use_error_v1)?;
        self.completion.ensure_releasable()?;
        if let Some(dispatch) = &self.dispatch {
            dispatch.ensure_releasable()?;
        }
        if let Some(sdma) = &self.sdma {
            sdma.preflight_retained_sdma_release_v1(self.key, self.queue_id)?;
        }
        preflight_primary_owners_v1::<Fixture>(
            &self.engine,
            self.key,
            self.signals.as_ref(),
            self.exception.as_ref(),
            self.doorbell.as_ref(),
        )
    }
    fn release_parts(
        &mut self,
    ) -> Result<PrimaryReleasePartsV1<'_, Fixture>, ComputeAqlQueueSessionErrorV1> {
        Ok(PrimaryReleasePartsV1 {
            engine: &mut self.engine,
            key: self.key,
            queue_id: self.queue_id,
            exception: &mut self.exception,
            doorbell: &mut self.doorbell,
            dispatch: &mut self.dispatch,
            signals: &mut self.signals,
            sdma: &mut self.sdma,
        })
    }
    fn poison_release(&mut self) {
        self.poisoned = true;
        self.unpublished.continuation = None;
        self.dependency.poison();
        self.completion.poison_owner();
        self.submission.poison();
        if let Some(dispatch) = &mut self.dispatch {
            dispatch.poison();
        }
    }
}

impl PrimaryReleaseMemoryV1 for Memory {
    fn restore_foundation(
        &mut self,
        foundation: &mut QueueModelFoundationV1,
    ) -> Result<(), MemorySessionError> {
        memory_step("restore-foundation")?;
        self.primary_restore_foundation_v1(foundation, trace().borrow().restore_foreign_vm)
    }
    fn release_queue_resources(
        &mut self,
        resources: &mut QueueResourceCleanupCustodyV1,
    ) -> Result<(), MemorySessionError> {
        memory_step("release-resources")?;
        trace().borrow_mut().release_snapshot =
            Some(self.retained_queue_cleanup_snapshot_v1(resources));
        if let Some((offset, panic)) = trace().borrow().queue_release_currentness_fault {
            self.primary_fail_currentness_v1(offset, panic);
        }
        let result = self.primary_release_queue_resources_v1(
            resources,
            trace().borrow().queue_release_projection_fault,
        );
        if result.is_ok() {
            trace().borrow_mut().post_resources_snapshot =
                Some(self.retained_cleanup_memory_snapshot_v1());
        }
        result
    }
    fn release_signals(
        &mut self,
        signals: &mut ControlCleanupCustodyV1,
    ) -> Result<(), MemorySessionError> {
        memory_step("release-signals")?;
        trace().borrow_mut().signal_snapshot =
            Some(self.retained_signal_cleanup_snapshot_v1(signals));
        if let Some((operation, panic)) = trace().borrow().signal_fault {
            self.fail_cleanup(operation, panic);
        }
        if let Some((offset, panic)) = trace().borrow().signal_currentness_fault {
            self.primary_fail_currentness_v1(offset, panic);
        }
        self.primary_release_signals_v1(signals, trace().borrow().signal_projection_fault)
    }
}

impl crate::sdma::retained_release::SdmaReleaseMemoryV1 for Memory {
    fn sdma_release_currentness(&mut self) -> Result<(), MemorySessionError> {
        memory_step("sdma-currentness")?;
        self.primary_currentness()
    }
    fn sdma_release_topology(&mut self) -> Result<(), MemorySessionError> {
        memory_step("sdma-topology")?;
        self.primary_currentness()
    }
    fn sdma_destroy(
        &mut self,
        args: &mut fe2o3_kfd_uapi::KfdIoctlDestroyQueueArgs,
    ) -> Result<(), rustix::io::Errno> {
        if let Some((id, panic)) = trace().borrow().sdma_destroy_mutation
            && id == args.queue_id
        {
            args.pad = 17;
            if panic {
                std::panic::panic_any(("mutated SDMA destroy", id));
            }
        }
        step(if args.queue_id == 101 {
            "sdma-destroy-h2d"
        } else {
            "sdma-destroy-d2h"
        })
        .map_err(|_| rustix::io::Errno::IO)
    }
    fn sdma_release_resources(
        &mut self,
        resources: &mut crate::shared_memory::SdmaResourceCleanupCustodyV1,
    ) -> Result<(), MemorySessionError> {
        memory_step("sdma-release-resources")?;
        let t = trace();
        let mut t = t.borrow_mut();
        let occurrence = t
            .calls
            .iter()
            .filter(|&&name| name == "sdma-release-resources")
            .count();
        if let Some((nth, panic)) = t.sdma_resource_native_fault
            && nth == occurrence
        {
            t.sdma_resource_snapshot = Some(self.primary_queue_cleanup_snapshot_v1(resources));
            self.primary_fail_cleanup_call_v1(11, "release_va_reservation", panic);
        }
        drop(t);
        self.primary_release_sdma_resources_v1(resources)
    }
    fn sdma_release_doorbell(
        &mut self,
        doorbell: &mut crate::queue_linux::LinuxDoorbellSliceV1,
        progress: &mut crate::queue_linux::LinuxDoorbellReleaseProgressV1,
    ) -> Result<(), crate::queue_linux::LinuxDoorbellErrorV1> {
        let occurrence = record("sdma-doorbell");
        let fault = trace().borrow().fault.and_then(|(name, nth, panic)| {
            (name == "sdma-doorbell" && nth == occurrence).then_some((nth, panic))
        });
        crate::queue_linux::doorbell_release_tests::release_local_doorbell(
            doorbell, progress, fault,
        )
    }
    fn sdma_release_poison(&mut self) {
        self.primary_quarantine_release_v1();
        trace().borrow_mut().poison = true;
    }
}

fn constructed(with_dispatch: bool) -> (Parent, Rc<RefCell<Trace>>, LocalGateV1) {
    constructed_on_gate(with_dispatch, LocalGateV1::new())
}

fn constructed_on_gate(
    with_dispatch: bool,
    gate: LocalGateV1,
) -> (Parent, Rc<RefCell<Trace>>, LocalGateV1) {
    let (memory, t) = setup_memory();
    t.borrow_mut().local_gate = Some(gate.clone());
    let complete = if with_dispatch {
        let (root, _) = setup_with_memory(memory, t.clone());
        let (mut root, result) = run(root, QueueRingBackingV1::AqlSpecial, false);
        assert!(result.is_ok(), "constructed dispatch parent");
        root.completed.take().unwrap()
    } else {
        let root = Root::<()>::new_with(memory, ());
        let (mut root, result) = run_with(root, QueueRingBackingV1::AqlSpecial, None, |_| Ok(()));
        assert!(result.is_ok(), "constructed queue parent");
        root.completed.take().unwrap()
    };
    let parent = parent_from_completed(complete);
    parent
        .engine
        .backend
        .session
        .primary_authenticate(&parent.engine.foundation)
        .unwrap();
    parent.preflight_release().unwrap();
    assert_eq!(gate.teardown_count(), 0);
    assert_eq!(gate.observation(), (false, false));
    let mut trace = t.borrow_mut();
    trace.calls.clear();
    trace.destroy = Some(0);
    drop(trace);
    (parent, t, gate)
}

fn parent_from_completed(complete: CompletedPrimaryV1<Fixture>) -> Parent {
    Parent {
        engine: complete.engine,
        key: complete.key,
        queue_id: complete.observation.queue_id,
        exception: Some(QueueExceptionStateV1 {
            runtime: complete.runtime,
            runtime_control: complete.runtime_control,
            event: complete.event,
            shadows: complete.shadows,
        }),
        doorbell: complete.doorbell,
        dispatch: complete.dispatch,
        signals: Some(complete.completion_signals),
        sdma: None,
        submission: complete.submission,
        completion: complete.completion_owner,
        dependency: complete.dependency_owner,
        unpublished: UnpublishedDispatchStateV1::default(),
        detached_count: 0,
        detached_generation: None,
        detached_identities: Vec::new(),
        detached_next: None,
        poisoned: false,
    }
}

pub(super) fn release_completed_parent_after_auxiliary(complete: CompletedPrimaryV1<Fixture>) {
    let id = complete.observation.queue_id;
    assert!(
        trace()
            .borrow()
            .create_returns
            .iter()
            .any(|record| record.0 == id)
    );
    trace().borrow_mut().destroy_target = Some(id);
    let mut parent = parent_from_completed(complete);
    let mut root = PrimaryReleaseStateV1::<Fixture>::new();
    root.release_in_place(&mut parent).unwrap();
    assert!(root.complete && !parent.poisoned);
    parent
        .engine
        .backend
        .session
        .primary_assert_all_released_v1();
    assert_eq!(
        trace()
            .borrow()
            .local_gate
            .as_ref()
            .unwrap()
            .runtime_observation(),
        LocalRuntimeObservationV1::Disabled
    );
    assert_eq!(
        trace()
            .borrow()
            .local_gate
            .as_ref()
            .unwrap()
            .teardown_count(),
        0
    );
}

fn original_resource_ids(parent: &Parent) -> [SharedGttAllocationIdentityV1; 4] {
    authority_ids(
        parent
            .engine
            .resources
            .iter()
            .find(|r| r.key == parent.key)
            .unwrap()
            .authority
            .as_ref()
            .unwrap(),
    )
}

fn assert_no_retry(
    parent: &mut Parent,
    state: &mut PrimaryReleaseStateV1<Fixture>,
    t: &Rc<RefCell<Trace>>,
) {
    let remaining = |parent: &Parent, state: &PrimaryReleaseStateV1<Fixture>| {
        let trace = t.borrow();
        let gate = trace.local_gate.as_ref().unwrap();
        (
            state.authority.as_ref().map(authority_ids),
            parent.signals.as_ref().map(Memory::primary_token_identity),
            parent
                .exception
                .as_ref()
                .map(|e| [e.runtime.identity, e.event.identity, e.shadows.identity]),
            parent.doorbell.as_ref().map(|d| d.identity),
            (
                state.started,
                state.complete,
                state.gate.is_some(),
                parent.poisoned,
                parent.engine.backend.foundation_in_engine,
            ),
            (
                gate.observation(),
                gate.runtime_observation(),
                gate.teardown_count(),
            ),
        )
    };
    let original_remaining = remaining(parent, state);
    let calls = t.borrow().calls.clone();
    let memory = parent
        .engine
        .backend
        .session
        .primary_release_memory_snapshot_v1(&parent.engine.foundation);
    let resources = state.resources.as_ref().map(|r| r.observation());
    let signals = state.signals.as_ref().map(|s| s.observation());
    let dispatch = state
        .dispatch
        .as_ref()
        .map(RetainedControlSnapshotV1::ordinary_root_v1);
    let original_dispatch = parent
        .dispatch
        .as_ref()
        .map(RetainedControlSnapshotV1::ordinary_owner_v1);
    let platform = state
        .platform
        .as_ref()
        .map(|p| (p.identities(), p.observation(), p.progress()));
    let destroy = state.destroy;
    let sdma = state.sdma.as_ref().map(|s| s.observation());
    let phase = parent.engine.phase(parent.key);
    let model = parent.engine.model.clone();
    let authority_poisoned = parent.engine.authority_poisoned;
    let release_fault = parent.engine.release_fault;
    assert!(matches!(
        state.release_in_place(parent),
        Err(ComputeAqlQueueSessionErrorV1::Contract(
            "primary release is one-shot"
        ))
    ));
    assert_eq!(t.borrow().calls, calls);
    assert_eq!(
        parent
            .engine
            .backend
            .session
            .primary_release_memory_snapshot_v1(&parent.engine.foundation),
        memory
    );
    assert_eq!(state.resources.as_ref().map(|r| r.observation()), resources);
    assert_eq!(state.signals.as_ref().map(|s| s.observation()), signals);
    assert_eq!(
        state
            .dispatch
            .as_ref()
            .map(RetainedControlSnapshotV1::ordinary_root_v1),
        dispatch
    );
    assert_eq!(
        parent
            .dispatch
            .as_ref()
            .map(RetainedControlSnapshotV1::ordinary_owner_v1),
        original_dispatch
    );
    assert_eq!(
        state
            .platform
            .as_ref()
            .map(|p| (p.identities(), p.observation(), p.progress())),
        platform
    );
    assert_eq!(state.destroy, destroy);
    assert_eq!(state.sdma.as_ref().map(|s| s.observation()), sdma);
    assert_eq!(parent.engine.phase(parent.key), phase);
    assert_eq!(parent.engine.model, model);
    assert_eq!(parent.engine.authority_poisoned, authority_poisoned);
    assert_eq!(parent.engine.release_fault, release_fault);
    assert_eq!(remaining(parent, state), original_remaining);
}

#[test]
fn constructed_primary_release_success_keeps_original_owners_and_completes_gate_last() {
    for dispatch in [false, true] {
        let (mut parent, t, gate) = constructed(dispatch);
        let ids = original_resource_ids(&parent);
        let signal_id = Memory::primary_token_identity(parent.signals.as_ref().unwrap());
        let session = parent.engine.backend.session.primary_session_id();
        let drops = t.borrow().drops;
        let owners: Vec<_> = t
            .borrow()
            .minted
            .iter()
            .copied()
            .filter(|id| id.role != Role::CreationArm)
            .collect();
        let clone = parent.exception.as_ref().unwrap().event.local_event_clone();
        let mut state = PrimaryReleaseStateV1::<Fixture>::new();
        let destroyed = state.release_in_place(&mut parent).unwrap();
        assert_eq!(destroyed.queue_id, parent.queue_id);
        assert!(state.complete && !parent.poisoned && !parent.engine.backend.foundation_in_engine);
        assert_eq!(parent.engine.backend.session.primary_session_id(), session);
        parent
            .engine
            .backend
            .session
            .primary_assert_permanently_restored_v1(&parent.engine.foundation);
        parent
            .engine
            .backend
            .session
            .primary_assert_all_released_v1();
        assert_eq!(
            state
                .resources
                .as_ref()
                .unwrap()
                .observation()
                .controls
                .map(|r| r.identity),
            ids
        );
        assert_eq!(
            state.signals.as_ref().unwrap().observation().identity,
            signal_id
        );
        assert!(
            parent.exception.is_none()
                && parent.doorbell.is_none()
                && parent.dispatch.is_none()
                && parent.signals.is_none()
        );
        assert!(state.authority.is_none() && state.gate.is_none());
        assert_eq!(state.dispatch.is_some(), dispatch);
        assert_eq!(
            state.platform.as_ref().unwrap().identities().as_slice(),
            owners
        );
        assert_eq!(
            state.platform.as_ref().unwrap().observation(),
            (false, (false, false), true, true)
        );
        assert!(
            !clone.is_active(),
            "all clones see original event destruction"
        );
        assert_eq!(t.borrow().drops, drops);
        assert_eq!(t.borrow().local_resources.live(), (1, 0, 1));
        assert_eq!(
            gate.runtime_observation(),
            LocalRuntimeObservationV1::Disabled
        );
        assert_eq!(gate.teardown_count(), 0);
        assert_eq!(gate.observation(), (false, false));
        let calls = t.borrow().calls.clone();
        let ordered = [
            "destroy",
            "destroy-event",
            "zero-payload",
            "protect-payload",
            "unmap-payload",
            "disable-runtime",
            "release-doorbell",
            "restore-foundation",
            "release-resources",
            "complete-shadows",
            "release-signals",
        ];
        let positions: Vec<_> = ordered
            .iter()
            .map(|name| calls.iter().position(|c| c == name).unwrap())
            .collect();
        assert!(positions.windows(2).all(|p| p[0] < p[1]));
        assert_no_retry(&mut parent, &mut state, &t);
        drop(state);
        drop(parent);
        drop(clone);
        assert_eq!(
            gate.observation(),
            (false, false),
            "completed fixture Drop is inert to gate"
        );
        assert_eq!(t.borrow().local_resources.live(), (0, 0, 0));
    }
}

#[test]
fn constructed_primary_release_destroy_outcomes_retain_full_parent() {
    for mode in 1..=4 {
        let (mut parent, t, gate) = constructed(true);
        t.borrow_mut().destroy = Some(mode);
        let ids = original_resource_ids(&parent);
        let before = parent
            .engine
            .backend
            .session
            .primary_release_memory_snapshot_v1(&parent.engine.foundation);
        let drops = t.borrow().drops;
        let mut state = PrimaryReleaseStateV1::<Fixture>::new();
        let result = catch_unwind(AssertUnwindSafe(|| state.release_in_place(&mut parent)));
        if mode == 3 {
            assert_eq!(
                result.unwrap_err().downcast_ref::<&str>(),
                Some(&"DESTROY panic")
            );
        } else {
            assert!(result.unwrap().is_err());
        }
        assert!(parent.poisoned && state.started && !state.complete && state.destroy.attempted);
        assert_eq!(state.destroy.returned.is_some(), mode != 3);
        assert_eq!(original_resource_ids(&parent), ids);
        assert!(parent.engine.backend.foundation_in_engine);
        assert!(parent.dispatch.is_some() && parent.signals.is_some());
        assert!(
            state.authority.is_none()
                && state.resources.is_none()
                && state.dispatch.is_none()
                && state.signals.is_none()
        );
        assert_eq!(
            state.platform.as_ref().unwrap().observation(),
            (true, (true, true), false, false)
        );
        before.assert_currentness_only_v1(
            parent
                .engine
                .backend
                .session
                .primary_release_memory_snapshot_v1(&parent.engine.foundation),
            if mode == 3 { 1 } else { 2 },
        );
        assert_eq!(t.borrow().drops, drops);
        assert_eq!(gate.teardown_count(), 1);
        assert!(gate.observation().1);
        assert!(!t.borrow().calls.contains(&"destroy-event"));
        assert_no_retry(&mut parent, &mut state, &t);
    }
}

#[test]
fn constructed_primary_release_platform_prefix_errors_and_panics_keep_suffixes() {
    for name in [
        "destroy-event",
        "zero-payload",
        "protect-payload",
        "unmap-payload",
        "disable-runtime",
        "release-doorbell",
        "release-validate-platform",
        "complete-shadows",
        "release-signals",
    ] {
        for panic in [false, true] {
            let (mut parent, t, gate) = constructed(true);
            t.borrow_mut().fault = Some((name, 1, panic));
            let ids = original_resource_ids(&parent);
            let signal_id = Memory::primary_token_identity(parent.signals.as_ref().unwrap());
            let drops = t.borrow().drops;
            let mut state = PrimaryReleaseStateV1::<Fixture>::new();
            let result = catch_unwind(AssertUnwindSafe(|| state.release_in_place(&mut parent)));
            if panic {
                assert_eq!(
                    result.unwrap_err().downcast_ref::<(&str, usize)>(),
                    Some(&(name, 1))
                );
            } else {
                assert!(result.unwrap().is_err(), "{name}");
            }
            assert!(parent.poisoned && !state.complete);
            assert_eq!(t.borrow().drops, drops);
            assert_eq!(gate.teardown_count(), 1);
            assert!(gate.observation().1);
            let (event, shadow, doorbell, complete) =
                state.platform.as_ref().unwrap().observation();
            let (runtime, payload_steps) = state.platform.as_ref().unwrap().progress();
            let (expected_runtime, pending, steps) = match name {
                "destroy-event" => ("queue-destroyed", false, 0),
                "zero-payload" => ("queue-destroyed", false, 0),
                "protect-payload" => ("queue-destroyed", false, 1),
                "unmap-payload" => ("queue-destroyed", false, 2),
                "disable-runtime" => ("event-destroyed", false, 3),
                "release-signals" => ("disabled", false, 3),
                _ => ("disabled", true, 3),
            };
            assert_eq!(
                (runtime, payload_steps),
                ((expected_runtime, pending), steps),
                "{name}"
            );
            assert_eq!(event, name == "destroy-event");
            assert_eq!(shadow, (name == "destroy-event", steps < 3));
            assert_eq!(
                doorbell,
                matches!(
                    name,
                    "release-validate-platform" | "complete-shadows" | "release-signals"
                )
            );
            assert_eq!(complete, name == "release-signals");
            if let Some(resources) = &state.resources {
                assert_eq!(resources.observation().controls.map(|r| r.identity), ids);
            } else {
                assert_eq!(original_resource_ids(&parent), ids);
            }
            if name == "release-signals" {
                assert!(
                    parent.dispatch.is_none() && state.dispatch.as_ref().unwrap().is_complete()
                );
                assert_eq!(
                    state.signals.as_ref().unwrap().observation().identity,
                    signal_id
                );
            } else {
                assert!(parent.dispatch.is_some() && state.dispatch.is_none());
                assert_eq!(
                    Memory::primary_token_identity(parent.signals.as_ref().unwrap()),
                    signal_id
                );
            }
            assert_no_retry(&mut parent, &mut state, &t);
        }
    }
}

#[test]
fn constructed_primary_release_failed_permanent_restore_keeps_authority_and_flag() {
    let (mut parent, t, gate) = constructed(true);
    t.borrow_mut().restore_foreign_vm = true;
    let ids = original_resource_ids(&parent);
    let mut state = PrimaryReleaseStateV1::<Fixture>::new();
    assert!(state.release_in_place(&mut parent).is_err());
    assert_eq!(
        parent.engine.phase(parent.key),
        Some(ComputeAqlQueuePhaseV1::Destroyed)
    );
    assert!(
        parent
            .engine
            .resources
            .iter()
            .find(|r| r.key == parent.key)
            .unwrap()
            .authority
            .is_none()
    );
    assert_eq!(authority_ids(state.authority.as_ref().unwrap()), ids);
    assert!(parent.engine.backend.foundation_in_engine);
    assert!(state.resources.is_none() && parent.dispatch.is_some() && parent.signals.is_some());
    assert!(parent.engine.foundation.is_certified_for_test());
    assert!(!t.borrow().calls.contains(&"release-resources"));
    assert_eq!(gate.teardown_count(), 1);
    assert_no_retry(&mut parent, &mut state, &t);
}

#[test]
fn constructed_primary_release_shared_lease_does_not_disable_other_constructed_queue() {
    let (mut first, first_trace, gate) = constructed(false);
    let (mut second, second_trace, _) = constructed_on_gate(false, gate.clone());
    assert_eq!(
        gate.runtime_observation(),
        LocalRuntimeObservationV1::Enabled {
            opener_pid: std::process::id(),
            leases: 2
        }
    );
    ACTIVE.with(|a| *a.borrow_mut() = Some(first_trace.clone()));
    let mut state = PrimaryReleaseStateV1::<Fixture>::new();
    state.release_in_place(&mut first).unwrap();
    assert!(!first_trace.borrow().calls.contains(&"disable-runtime"));
    assert_eq!(
        gate.runtime_observation(),
        LocalRuntimeObservationV1::Enabled {
            opener_pid: std::process::id(),
            leases: 1
        }
    );
    assert_eq!(gate.teardown_count(), 0);
    assert_eq!(gate.observation(), (false, false));
    drop(state);
    drop(first);
    assert_eq!(gate.observation(), (false, false));
    ACTIVE.with(|a| *a.borrow_mut() = Some(second_trace.clone()));
    let mut state = PrimaryReleaseStateV1::<Fixture>::new();
    state.release_in_place(&mut second).unwrap();
    assert_eq!(
        second_trace
            .borrow()
            .calls
            .iter()
            .filter(|&&c| c == "disable-runtime")
            .count(),
        1
    );
    assert_eq!(
        gate.runtime_observation(),
        LocalRuntimeObservationV1::Disabled
    );
    drop(state);
    drop(second);
    assert_eq!(gate.observation(), (false, false));
    assert_eq!(first_trace.borrow().local_resources.live(), (0, 0, 0));
    assert_eq!(second_trace.borrow().local_resources.live(), (0, 0, 0));
}

#[test]
fn constructed_primary_release_native_resource_failures_retain_exact_prefix_and_suffix() {
    let calls = [
        "unmap_gpu",
        "unmap_gpu",
        "unmap_gpu",
        "unmap_gpu",
        "unmap_cpu",
        "free",
        "release_va_reservation",
        "free",
        "unmap_cpu",
        "unmap_cpu",
        "free",
        "release_va_reservation",
        "unmap_cpu",
        "free",
        "release_va_reservation",
    ];
    for failed in [0, 3, 4, 6, 7, 8, 11, 14] {
        for panic in [false, true] {
            let (mut parent, t, gate) = constructed(true);
            let dispatch =
                RetainedControlSnapshotV1::ordinary_owner_v1(parent.dispatch.as_ref().unwrap());
            let signal_id = Memory::primary_token_identity(parent.signals.as_ref().unwrap());
            parent.engine.backend.session.primary_fail_cleanup_call_v1(
                failed + 1,
                calls[failed],
                panic,
            );
            let mut state = PrimaryReleaseStateV1::<Fixture>::new();
            let result = catch_unwind(AssertUnwindSafe(|| state.release_in_place(&mut parent)));
            if panic {
                assert_eq!(
                    result.unwrap_err().downcast_ref::<(&str, &str)>(),
                    Some(&("N2 native panic", calls[failed]))
                );
            } else {
                assert!(result.unwrap().is_err());
            }
            let before = t.borrow_mut().release_snapshot.take().unwrap();
            before.assert_constructed_queue_failure_v1(
                &parent.engine.backend.session,
                state.resources.as_ref().unwrap(),
                failed,
                panic,
            );
            dispatch.assert_restored_v1(
                RetainedControlSnapshotV1::ordinary_owner_v1(parent.dispatch.as_ref().unwrap()),
                true,
            );
            assert_eq!(
                Memory::primary_token_identity(parent.signals.as_ref().unwrap()),
                signal_id
            );
            assert!(
                state.dispatch.is_none() && state.signals.is_none() && state.authority.is_none()
            );
            assert!(
                parent.poisoned && !parent.engine.backend.foundation_in_engine && !state.complete
            );
            assert_eq!(
                state.platform.as_ref().unwrap().progress(),
                (("disabled", true), 3)
            );
            assert_eq!(gate.teardown_count(), 1);
            assert!(!t.borrow().calls.contains(&"complete-shadows"));
            assert_no_retry(&mut parent, &mut state, &t);
        }
    }
}

#[test]
fn constructed_primary_release_native_dispatch_failures_keep_signal_and_gate() {
    for index in [0, 3] {
        for (step, operation) in ["unmap_gpu", "unmap_cpu", "free", "release_va_reservation"]
            .into_iter()
            .enumerate()
        {
            for panic in [false, true] {
                let (mut parent, t, gate) = constructed(true);
                let dispatch =
                    RetainedControlSnapshotV1::ordinary_owner_v1(parent.dispatch.as_ref().unwrap());
                let signal_id = Memory::primary_token_identity(parent.signals.as_ref().unwrap());
                parent
                    .engine
                    .backend
                    .session
                    .arm_control_release_native_v1(index, operation, panic);
                let mut state = PrimaryReleaseStateV1::<Fixture>::new();
                let result = catch_unwind(AssertUnwindSafe(|| state.release_in_place(&mut parent)));
                if panic {
                    assert_eq!(
                        result.unwrap_err().downcast_ref::<(&str, &str)>(),
                        Some(&("N2 native panic", operation))
                    );
                } else {
                    assert!(result.unwrap().is_err());
                }
                assert!(state.resources.as_ref().unwrap().is_complete());
                let retained = state.dispatch.as_ref().unwrap();
                dispatch.assert_ordinary_control_prefix_v1(retained, index);
                let native = t.borrow_mut().post_resources_snapshot.take().unwrap();
                parent
                    .engine
                    .backend
                    .session
                    .primary_assert_control_failure_v1(&native, &dispatch.order_v1(), index, step);
                let active = RetainedControlSnapshotV1::root_v1(retained);
                let active = active.active_v1().unwrap();
                assert!(active.started && active.failed && !active.native_disposed);
                assert_eq!(active.identity, dispatch.order_v1()[index]);
                assert_eq!(
                    Memory::primary_token_identity(parent.signals.as_ref().unwrap()),
                    signal_id
                );
                assert!(state.signals.is_none() && !state.complete && parent.poisoned);
                assert_eq!(gate.teardown_count(), 1);
                assert!(!t.borrow().calls.contains(&"release-signals"));
                assert_no_retry(&mut parent, &mut state, &t);
            }
        }
    }
}

#[test]
fn constructed_primary_release_native_signal_failures_keep_refunds_and_gate_pending() {
    for (step, operation) in ["unmap_gpu", "unmap_cpu", "free", "release_va_reservation"]
        .into_iter()
        .enumerate()
    {
        for panic in [false, true] {
            let (mut parent, t, gate) = constructed(true);
            let signal_id = Memory::primary_token_identity(parent.signals.as_ref().unwrap());
            t.borrow_mut().signal_fault = Some((operation, panic));
            let mut state = PrimaryReleaseStateV1::<Fixture>::new();
            let result = catch_unwind(AssertUnwindSafe(|| state.release_in_place(&mut parent)));
            if panic {
                assert_eq!(
                    result.unwrap_err().downcast_ref::<(&str, &str)>(),
                    Some(&("N2 native panic", operation))
                );
            } else {
                assert!(result.unwrap().is_err());
            }
            assert!(
                state.resources.as_ref().unwrap().is_complete()
                    && state.dispatch.as_ref().unwrap().is_complete()
            );
            let signal = state.signals.as_ref().unwrap().observation();
            assert_eq!(signal.identity, signal_id);
            assert!(signal.started && signal.failed && !signal.native_disposed);
            let native = t.borrow_mut().signal_snapshot.take().unwrap();
            parent
                .engine
                .backend
                .session
                .primary_assert_control_failure_v1(&native, &[signal_id], 0, step);
            assert!(parent.dispatch.is_none() && parent.signals.is_none() && !state.complete);
            assert_eq!(gate.teardown_count(), 1);
            assert!(gate.observation().1);
            assert_no_retry(&mut parent, &mut state, &t);
        }
    }
}

#[test]
fn constructed_primary_release_currentness_failures_keep_destroy_receipt_and_memory() {
    for offset in 1..=3 {
        for panic in [false, true] {
            let (mut parent, t, gate) = constructed(true);
            let ids = original_resource_ids(&parent);
            let before = parent
                .engine
                .backend
                .session
                .primary_release_memory_snapshot_v1(&parent.engine.foundation);
            parent
                .engine
                .backend
                .session
                .primary_fail_currentness_v1(offset, panic);
            let mut state = PrimaryReleaseStateV1::<Fixture>::new();
            let result = catch_unwind(AssertUnwindSafe(|| state.release_in_place(&mut parent)));
            if panic {
                assert_eq!(
                    result.unwrap_err().downcast_ref::<(&str, &str)>(),
                    Some(&("N2 native panic", "currentness"))
                );
            } else {
                assert!(result.unwrap().is_err());
            }
            assert_eq!(state.destroy.attempted, offset > 1);
            assert_eq!(state.destroy.returned.is_some(), offset > 1);
            assert_eq!(original_resource_ids(&parent), ids);
            assert!(state.authority.is_none() && state.resources.is_none());
            assert!(
                parent.dispatch.is_some()
                    && parent.signals.is_some()
                    && parent.engine.backend.foundation_in_engine
            );
            before.assert_primary_currentness_failure_v1(
                parent
                    .engine
                    .backend
                    .session
                    .primary_release_memory_snapshot_v1(&parent.engine.foundation),
                offset,
                panic,
            );
            assert_eq!(
                state.platform.as_ref().unwrap().observation(),
                if offset < 3 {
                    (true, (true, true), false, false)
                } else {
                    (false, (false, false), true, false)
                }
            );
            assert_eq!(gate.teardown_count(), 1);
            assert_no_retry(&mut parent, &mut state, &t);
        }
    }
}
