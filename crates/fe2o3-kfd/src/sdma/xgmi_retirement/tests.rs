use super::*;
use crate::queue_linux::doorbell_release_tests::{cleanup_local_doorbell, release_local_doorbell};
use crate::queue_linux::{LinuxDoorbellErrorV1, LinuxDoorbellReleaseProgressV1};
use crate::sdma::retained_release::fixture::{self, creation_owner_observation};
use crate::shared_memory::{PreparationMemoryFixtureV1, SdmaResourceCleanupCustodyV1};
use std::mem::ManuallyDrop;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

const STEPS: [&str; 8] = [
    "prepare",
    "opening",
    "before-destroy",
    "destroy",
    "doorbell",
    "after-destroy",
    "resources",
    "closing",
];
const TERMINALIZERS: [&str; 3] = ["source", "destination", "global"];

struct HostilePayload {
    stage: usize,
    drops: Arc<AtomicUsize>,
}
impl Drop for HostilePayload {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
        panic!("secondary payload destructor");
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Identity {
    key: QueueKeyV1,
    queue_id: u32,
    engine: Option<u32>,
    storage: [(usize, usize, usize); 4],
    resources: [Option<SharedGttAllocationIdentityV1>; 3],
}

fn identity(owner: &Gfx942SdmaQueueOwnerV1, progress: &OwnerProgressV1) -> Identity {
    fn storage<T>(values: &Vec<T>) -> (usize, usize, usize) {
        (values.as_ptr() as usize, values.len(), values.capacity())
    }
    Identity {
        key: owner.owner,
        queue_id: owner.queue_id,
        engine: owner.engine_index,
        storage: [
            storage(&owner.records),
            storage(&owner.xgmi_records),
            storage(&owner.persistent_window_slots),
            storage(&owner.persistent_window_records),
        ],
        resources: progress.resources.as_ref().map_or_else(
            || {
                [
                    owner
                        .completions
                        .as_ref()
                        .map(|value| value.storage_identity()),
                    owner
                        .control
                        .as_ref()
                        .map(PreparationMemoryFixtureV1::primary_token_identity),
                    owner
                        .ring
                        .as_ref()
                        .map(PreparationMemoryFixtureV1::primary_token_identity),
                ]
            },
            |resources| {
                resources
                    .observation()
                    .controls
                    .each_ref()
                    .map(|control| Some(control.identity))
            },
        ),
    }
}

struct Script {
    route: Gfx942XgmiRouteV1,
    trace: Vec<&'static str>,
    fault: Option<(&'static str, bool)>,
    original_panic: Option<Box<i32>>,
    mutation: Option<bool>,
    cleanup_fault: Option<(usize, &'static str, bool)>,
    incomplete_resources: bool,
    validations: usize,
    currentness: usize,
    expected: Option<Identity>,
    memory: PreparationMemoryFixtureV1,
    destination: PreparationMemoryFixtureV1,
    global_poison: bool,
    hostile: [bool; 3],
    drops: Arc<AtomicUsize>,
}

impl Script {
    fn new(route: Gfx942XgmiRouteV1) -> Self {
        Self {
            route,
            trace: Vec::new(),
            fault: None,
            original_panic: None,
            mutation: None,
            cleanup_fault: None,
            incomplete_resources: false,
            validations: 0,
            currentness: 0,
            expected: None,
            memory: PreparationMemoryFixtureV1::new(true),
            destination: PreparationMemoryFixtureV1::new(true),
            global_poison: false,
            hostile: [false; 3],
            drops: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn step(&mut self, stage: &'static str) -> Result<(), Gfx942SdmaErrorV1> {
        self.trace.push(stage);
        match self.fault {
            Some((name, true)) if name == stage => {
                if let Some(payload) = self.original_panic.take() {
                    std::panic::panic_any(payload);
                }
                std::panic::panic_any(stage);
            }
            Some((name, false)) if name == stage => Err(Gfx942SdmaErrorV1::Contract(stage)),
            _ => Ok(()),
        }
    }

    fn set(&mut self) -> Gfx942SdmaQueueSetV1 {
        use fe2o3_runtime_model::*;
        let key = QueueKeyV1 {
            vm: VmKeyV1 {
                device: DeviceKeyV1 {
                    physical: PhysicalDeviceIdV1(u64::from(self.route.source_gpu_id())),
                    generation: DeviceGenerationV1(1),
                },
                id: VmIdV1(1),
            },
            id: QueueInstanceIdV1(71),
            generation: QueueGenerationV1(72),
        };
        fixture::generic(
            &mut self.memory,
            key,
            Some(self.route.recommended_engine_id()),
        )
    }

    fn queue_from_set(&mut self, set: Gfx942SdmaQueueSetV1) -> Gfx942NativeXgmiSdmaQueueV1 {
        let Gfx942SdmaQueueSetV1::Generic(mut owners) = set else {
            panic!("fixture profile")
        };
        let owner = owners.pop().unwrap();
        assert!(owners.is_empty());
        self.expected = Some(identity(&owner, &OwnerProgressV1::default()));
        Gfx942NativeXgmiSdmaQueueV1 {
            route: self.route,
            owner: Some(owner),
            retirement: RetirementRoot::new(),
        }
    }

    fn queue(&mut self) -> Gfx942NativeXgmiSdmaQueueV1 {
        let set = self.set();
        self.queue_from_set(set)
    }

    fn check_root(&self, root: &RetirementRoot) {
        let State::Pending {
            route,
            owner,
            progress,
        } = &root.state
        else {
            panic!("vacant root during effect")
        };
        assert_eq!(*route, self.route);
        assert_eq!(Some(identity(owner, progress)), self.expected);
        if progress.resources.is_some() {
            assert!(owner.ring.is_none() && owner.control.is_none() && owner.completions.is_none());
        }
    }

    fn terminalize(&mut self, stage: usize, root: &RetirementRoot) {
        self.check_root(root);
        let State::Pending { owner, .. } = &root.state else {
            unreachable!()
        };
        assert!(owner.poisoned);
        self.trace.push(TERMINALIZERS[stage]);
        match stage {
            0 => self.memory.primary_quarantine_release_v1(),
            1 => self.destination.primary_quarantine_release_v1(),
            _ => self.global_poison = true,
        }
        if self.hostile[stage] {
            std::panic::panic_any(HostilePayload {
                stage,
                drops: self.drops.clone(),
            });
        }
    }

    fn assert_terminal(&self, queue: &Gfx942NativeXgmiSdmaQueueV1) {
        assert!(queue.owner.is_none());
        assert!(queue.has_terminal_retirement_v1());
        self.check_root(&queue.retirement);
        assert!(self.trace.ends_with(&TERMINALIZERS));
        assert!(self.memory.primary_is_quarantined_v1());
        assert!(self.destination.primary_is_quarantined_v1());
        assert!(self.global_poison);
    }
}

impl Context for Script {
    type Memory = Self;
    fn prepare_doorbell_error(&mut self) -> Result<String, Gfx942SdmaErrorV1> {
        self.step("prepare")?;
        preallocate_doorbell_failure_message()
    }
    fn memory(&mut self) -> &mut Self {
        self
    }
    fn validate(
        &mut self,
        route: Gfx942XgmiRouteV1,
        root: &RetirementRoot,
    ) -> Result<(), Gfx942SdmaErrorV1> {
        assert_eq!(route, self.route);
        self.check_root(root);
        let closing = self.validations != 0;
        self.validations += 1;
        let State::Pending {
            owner, progress, ..
        } = &root.state
        else {
            unreachable!()
        };
        if closing {
            assert!(owner.destroyed);
            assert!(progress.resources.as_ref().unwrap().is_complete());
        } else {
            assert!(!progress.attempted && progress.resources.is_none());
            assert!(!owner.destroyed);
        }
        self.step(if closing { "closing" } else { "opening" })
    }
    fn quarantine(&mut self, destination: bool, root: &RetirementRoot) {
        self.terminalize(usize::from(destination), root);
    }
    fn poison(&mut self, root: &RetirementRoot) {
        self.terminalize(2, root);
    }
}

impl SdmaOwnerReleaseMemoryV1 for Script {
    fn sdma_release_currentness(&mut self) -> Result<(), MemorySessionError> {
        self.currentness += 1;
        let stage = if self.currentness == 1 {
            "before-destroy"
        } else {
            "after-destroy"
        };
        self.step(stage)
            .map_err(|_| MemorySessionError::KernelResultMalformed(stage))?;
        self.memory.primary_currentness()
    }
    fn sdma_destroy(
        &mut self,
        args: &mut KfdIoctlDestroyQueueArgs,
    ) -> Result<(), rustix::io::Errno> {
        assert_eq!(*args, KfdIoctlDestroyQueueArgs::new(100));
        if let Some(panic) = self.mutation {
            args.queue_id = 101;
            args.pad = 17;
            if panic {
                std::panic::panic_any("mutated destroy");
            }
        }
        self.step("destroy").map_err(|_| rustix::io::Errno::IO)
    }
    fn sdma_release_doorbell(
        &mut self,
        doorbell: &mut LinuxDoorbellSliceV1,
        progress: &mut LinuxDoorbellReleaseProgressV1,
    ) -> Result<(), LinuxDoorbellErrorV1> {
        self.trace.push("doorbell");
        let fault = self
            .fault
            .and_then(|(stage, panic)| (stage == "doorbell").then_some((1, panic)));
        release_local_doorbell(doorbell, progress, fault)
    }
    fn sdma_release_resources(
        &mut self,
        resources: &mut SdmaResourceCleanupCustodyV1,
    ) -> Result<(), MemorySessionError> {
        self.step("resources")
            .map_err(|_| MemorySessionError::KernelResultMalformed("resources"))?;
        if self.incomplete_resources {
            return Ok(());
        }
        if let Some((ordinal, operation, panic)) = self.cleanup_fault {
            self.memory
                .primary_fail_cleanup_call_v1(ordinal, operation, panic);
        }
        let before = self.memory.primary_queue_cleanup_snapshot_v1(resources);
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.memory.primary_release_sdma_resources_v1(resources)
        }));
        if let Some((11, "release_va_reservation", panic)) = self.cleanup_fault {
            before.assert_sdma_late_cleanup_v1(&self.memory, resources, panic);
        }
        match result {
            Ok(result) => result,
            Err(payload) => resume_unwind(payload),
        }
    }
}

fn dispose_fixture(queue: &mut Gfx942NativeXgmiSdmaQueueV1) {
    // Only anonymous test doorbells; this grants no production cleanup path.
    let owner = match &mut queue.retirement.state {
        State::Pending { owner, .. } => Some(owner),
        State::Vacant => queue.owner.as_mut(),
    };
    if let Some(doorbell) = owner.and_then(|owner| owner.doorbell.as_mut()) {
        cleanup_local_doorbell(doorbell);
    }
    queue.retirement.state = State::Vacant;
}

fn assert_inert_retry(script: &mut Script, queue: &mut Gfx942NativeXgmiSdmaQueueV1) {
    let observe = |queue: &Gfx942NativeXgmiSdmaQueueV1| {
        let State::Pending {
            owner, progress, ..
        } = &queue.retirement.state
        else {
            panic!("terminal custody")
        };
        (
            creation_owner_observation(owner),
            progress.request,
            progress.attempted,
            progress.result,
            progress.doorbell,
            progress.resources.as_ref().map(|value| value.observation()),
        )
    };
    let before = observe(queue);
    let calls = script.trace.clone();
    assert!(matches!(
        retire_with(queue, script),
        Err(Gfx942SdmaErrorV1::Contract(
            "XGMI retirement root is occupied"
        ))
    ));
    assert!(matches!(
        queue.require_live_queue_state_v1(),
        Err(Gfx942SdmaErrorV1::Contract(
            "XGMI retirement root is occupied"
        ))
    ));
    assert_eq!(script.trace, calls);
    assert_eq!(observe(queue), before);
}

#[test]
fn success_retires_both_route_directions_and_clears_only_after_closing() {
    assert!(std::mem::size_of::<RetirementRoot>() <= 2048);
    for route in crate::topology::tests::admitted_xgmi_routes() {
        let mut script = Script::new(route);
        let mut queue = ManuallyDrop::new(script.queue());
        assert!(queue.require_live_queue_state_v1().is_ok());
        retire_with(&mut queue, &mut script).unwrap();
        assert_eq!(script.trace, STEPS);
        assert!(!queue.has_terminal_retirement_v1());
        assert!(queue.owner.is_none());
        assert!(queue.require_live_queue_state_v1().is_err());
        assert!(retire_with(&mut queue, &mut script).is_err());
        assert_eq!(script.trace, STEPS);
        assert!(!script.memory.primary_is_quarantined_v1());
        assert!(!script.destination.primary_is_quarantined_v1());
        drop(ManuallyDrop::into_inner(queue));
    }
}

#[test]
fn preparation_errors_and_panics_leave_live_owner_unchanged_and_retryable() {
    let route = crate::topology::tests::admitted_xgmi_routes()[0];
    for panic in [false, true] {
        let mut script = Script::new(route);
        let mut queue = ManuallyDrop::new(script.queue());
        let before = creation_owner_observation(queue.owner.as_ref().unwrap());
        script.fault = Some(("prepare", panic));
        let result = catch_unwind(AssertUnwindSafe(|| retire_with(&mut queue, &mut script)));
        if panic {
            assert_eq!(result.unwrap_err().downcast_ref::<&str>(), Some(&"prepare"));
        } else {
            assert!(result.unwrap().is_err());
        }
        assert_eq!(script.trace, ["prepare"]);
        assert_eq!(
            creation_owner_observation(queue.owner.as_ref().unwrap()),
            before
        );
        assert!(queue.retirement.is_vacant());
        script.fault = None;
        script.trace.clear();
        retire_with(&mut queue, &mut script).unwrap();
        drop(ManuallyDrop::into_inner(queue));
    }
}

#[test]
fn all_five_pending_classes_reject_before_preparation_or_route_validation() {
    for route in crate::topology::tests::admitted_xgmi_routes() {
        for case in 14..=18 {
            let mut script = Script::new(route);
            let set = script.set();
            let mut set = fixture::with_generic_corruption(set, case, 1, |set| {
                let pending = fixture::generic_pending_observation(&set);
                let mut queue = ManuallyDrop::new(script.queue_from_set(set));
                let before = creation_owner_observation(queue.owner.as_ref().unwrap());
                assert!(matches!(
                    retire_with(&mut queue, &mut script),
                    Err(Gfx942SdmaErrorV1::Pending)
                ));
                assert!(queue.retirement.is_vacant());
                assert!(script.trace.is_empty());
                assert_eq!(
                    creation_owner_observation(queue.owner.as_ref().unwrap()),
                    before
                );
                let set = Gfx942SdmaQueueSetV1::Generic(vec![queue.owner.take().unwrap()]);
                assert_eq!(fixture::generic_pending_observation(&set), pending);
                drop(ManuallyDrop::into_inner(queue));
                set
            });
            fixture::cleanup_set(&mut set);
        }
    }
}

#[test]
fn malformed_or_poisoned_owners_terminalize_without_native_teardown() {
    for route in crate::topology::tests::admitted_xgmi_routes() {
        for case in [3, 4, 6, 7, 8, 9, 10, 11, 12, 13, 20] {
            let mut script = Script::new(route);
            let set = script.set();
            let mut set = fixture::with_generic_corruption(set, case, 1, |set| {
                let mut queue = ManuallyDrop::new(script.queue_from_set(set));
                assert!(retire_with(&mut queue, &mut script).is_err());
                script.assert_terminal(&queue);
                assert_eq!(script.trace, TERMINALIZERS);
                assert_inert_retry(&mut script, &mut queue);
                let State::Pending {
                    owner, progress, ..
                } = std::mem::replace(&mut queue.retirement.state, State::Vacant)
                else {
                    unreachable!()
                };
                assert!(!progress.attempted && progress.resources.is_none());
                drop(ManuallyDrop::into_inner(queue));
                Gfx942SdmaQueueSetV1::Generic(vec![owner])
            });
            fixture::cleanup_set(&mut set);
        }
    }
}

#[test]
fn invalid_engine_and_inactive_doorbell_are_not_masked_by_pending() {
    let route = crate::topology::tests::admitted_xgmi_routes()[0];
    for mode in 0..2 {
        let mut script = Script::new(route);
        let mut queue = ManuallyDrop::new(script.queue());
        let owner = queue.owner.as_mut().unwrap();
        owner.uncertain_xgmi_ticket = Some(Gfx942SdmaCopyTicketV1 {
            owner: owner.owner,
            queue_id: owner.queue_id,
            slot: 0,
            generation: 1,
        });
        if mode == 0 {
            owner.engine_index = Some(route.recommended_engine_id() + 1);
        } else {
            cleanup_local_doorbell(owner.doorbell.as_mut().unwrap());
        }
        script.expected = Some(identity(owner, &OwnerProgressV1::default()));
        assert!(matches!(
            retire_with(&mut queue, &mut script),
            Err(Gfx942SdmaErrorV1::Contract(_))
        ));
        script.assert_terminal(&queue);
        assert_eq!(script.trace, TERMINALIZERS);
        assert_inert_retry(&mut script, &mut queue);
        dispose_fixture(&mut queue);
        drop(ManuallyDrop::into_inner(queue));
    }
}

#[test]
fn every_effectful_callback_error_and_panic_retains_exact_custody_and_prefix() {
    for route in crate::topology::tests::admitted_xgmi_routes() {
        for (index, &stage) in STEPS.iter().enumerate().skip(1) {
            for panic in [false, true] {
                let mut script = Script::new(route);
                let mut queue = ManuallyDrop::new(script.queue());
                script.fault = Some((stage, panic));
                let result =
                    catch_unwind(AssertUnwindSafe(|| retire_with(&mut queue, &mut script)));
                if panic {
                    let payload = result.unwrap_err();
                    if stage == "doorbell" {
                        assert_eq!(
                            payload.downcast_ref::<(&str, usize)>(),
                            Some(&("sdma-doorbell", 1))
                        );
                    } else {
                        assert_eq!(payload.downcast_ref::<&str>(), Some(&stage));
                    }
                } else {
                    assert!(result.unwrap().is_err());
                }
                script.assert_terminal(&queue);
                assert_eq!(&script.trace[..index + 1], &STEPS[..index + 1]);
                assert_eq!(script.trace.len(), index + 4);
                let State::Pending {
                    owner, progress, ..
                } = &queue.retirement.state
                else {
                    unreachable!()
                };
                assert_eq!(progress.attempted, index >= 3);
                assert_eq!(
                    progress.request,
                    (index >= 3).then_some(KfdIoctlDestroyQueueArgs::new(100))
                );
                assert_eq!(
                    progress.result,
                    if index < 3 || (index == 3 && panic) {
                        None
                    } else if index == 3 {
                        Some(Err(rustix::io::Errno::IO))
                    } else {
                        Some(Ok(()))
                    }
                );
                assert_eq!(progress.doorbell.started, index >= 4);
                assert_eq!(progress.doorbell.attempted, index >= 4);
                assert_eq!(
                    progress.doorbell.result,
                    if index < 4 || (index == 4 && panic) {
                        None
                    } else if index == 4 {
                        Some(Err(rustix::io::Errno::IO))
                    } else {
                        Some(Ok(()))
                    }
                );
                assert_eq!(owner.destroyed, index >= 5);
                assert_eq!(progress.resources.is_some(), index >= 6);
                if let Some(resources) = &progress.resources {
                    assert_eq!(resources.is_complete(), index == 7);
                }
                assert_inert_retry(&mut script, &mut queue);
                dispose_fixture(&mut queue);
                drop(ManuallyDrop::into_inner(queue));
            }
        }
    }
}

#[test]
fn mutated_destroy_arguments_and_raw_outcomes_remain_rooted() {
    let route = crate::topology::tests::admitted_xgmi_routes()[0];
    for panic in [false, true] {
        let mut script = Script::new(route);
        let mut queue = ManuallyDrop::new(script.queue());
        script.mutation = Some(panic);
        let result = catch_unwind(AssertUnwindSafe(|| retire_with(&mut queue, &mut script)));
        if panic {
            assert_eq!(
                result.unwrap_err().downcast_ref::<&str>(),
                Some(&"mutated destroy")
            );
        } else {
            assert!(matches!(
                result.unwrap(),
                Err(Gfx942SdmaErrorV1::Contract(
                    "kernel changed immutable SDMA DESTROY_QUEUE inputs"
                ))
            ));
        }
        script.assert_terminal(&queue);
        let State::Pending { progress, .. } = &queue.retirement.state else {
            unreachable!()
        };
        assert_eq!(
            progress.request,
            Some(KfdIoctlDestroyQueueArgs {
                queue_id: 101,
                pad: 17
            })
        );
        assert!(progress.attempted);
        assert_eq!(progress.result, (!panic).then_some(Ok(())));
        assert!(!progress.doorbell.started && progress.resources.is_none());
        assert_inert_retry(&mut script, &mut queue);
        dispose_fixture(&mut queue);
        drop(ManuallyDrop::into_inner(queue));
    }
}

#[test]
fn real_resource_cleanup_errors_and_panics_retain_every_native_prefix() {
    let operations = [
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
    ];
    for route in crate::topology::tests::admitted_xgmi_routes() {
        for (failed, operation) in operations.into_iter().enumerate() {
            for panic in [false, true] {
                let mut script = Script::new(route);
                let mut queue = ManuallyDrop::new(script.queue());
                script.cleanup_fault = Some((failed + 1, operation, panic));
                let result =
                    catch_unwind(AssertUnwindSafe(|| retire_with(&mut queue, &mut script)));
                if panic {
                    assert_eq!(
                        result.unwrap_err().downcast_ref::<(&str, &str)>(),
                        Some(&("N2 native panic", operation))
                    );
                } else {
                    assert!(
                        matches!(result.unwrap(), Err(Gfx942SdmaErrorV1::Memory(MemorySessionError::Injected(got))) if got == operation)
                    );
                }
                script.assert_terminal(&queue);
                let State::Pending { progress, .. } = &queue.retirement.state else {
                    unreachable!()
                };
                let state = progress.resources.as_ref().unwrap().observation();
                let active = if failed < 3 {
                    failed
                } else if failed < 6 {
                    0
                } else if failed < 8 {
                    1
                } else {
                    2
                };
                assert_eq!(
                    (state.started, state.failed, state.unmapped, state.released),
                    (
                        true,
                        true,
                        failed.min(3),
                        if failed < 3 { 0 } else { active }
                    )
                );
                assert!(state.controls[active].failed);
                for index in 0..state.released {
                    assert_eq!(state.controls[index].owner, "NativeDisposed");
                }
                assert_eq!(&script.trace[..7], &STEPS[..7]);
                assert!(!script.trace.contains(&"closing"));
                assert_inert_retry(&mut script, &mut queue);
                dispose_fixture(&mut queue);
                drop(ManuallyDrop::into_inner(queue));
            }
        }
    }
}

#[test]
fn incomplete_resource_success_cannot_vacate_root_or_reach_closing() {
    let route = crate::topology::tests::admitted_xgmi_routes()[0];
    let mut script = Script::new(route);
    let mut queue = ManuallyDrop::new(script.queue());
    script.incomplete_resources = true;
    assert!(matches!(
        retire_with(&mut queue, &mut script),
        Err(Gfx942SdmaErrorV1::Contract("incomplete SDMA resources"))
    ));
    script.assert_terminal(&queue);
    assert!(!script.trace.contains(&"closing"));
    assert_inert_retry(&mut script, &mut queue);
    dispose_fixture(&mut queue);
    drop(ManuallyDrop::into_inner(queue));
}

#[test]
fn all_terminalizers_run_and_original_panic_wins_over_hostile_secondaries() {
    let route = crate::topology::tests::admitted_xgmi_routes()[0];
    for panic in [false, true] {
        for mask in 0..8 {
            let mut script = Script::new(route);
            let mut queue = ManuallyDrop::new(script.queue());
            script.fault = Some(("opening", panic));
            script.hostile = std::array::from_fn(|index| mask & (1 << index) != 0);
            script.original_panic = Some(Box::new(91));
            let address = &**script.original_panic.as_ref().unwrap() as *const _;
            let result = catch_unwind(AssertUnwindSafe(|| retire_with(&mut queue, &mut script)));
            if panic {
                let payload = result.unwrap_err();
                assert_eq!(
                    &**payload.downcast_ref::<Box<i32>>().unwrap() as *const _,
                    address
                );
            } else if mask != 0 {
                let payload = result.unwrap_err();
                assert_eq!(
                    payload.downcast_ref::<HostilePayload>().unwrap().stage,
                    script.hostile.iter().position(|value| *value).unwrap()
                );
                std::mem::forget(payload);
            } else {
                assert!(result.unwrap().is_err());
            }
            assert_eq!(script.drops.load(Ordering::SeqCst), 0);
            script.assert_terminal(&queue);
            assert_eq!(&script.trace[..2], &STEPS[..2]);
            assert_eq!(script.trace.len(), 5);
            assert_inert_retry(&mut script, &mut queue);
            dispose_fixture(&mut queue);
            drop(ManuallyDrop::into_inner(queue));
        }
    }
}

#[test]
fn occupied_root_drop_aborts_even_after_complete_cleanup_or_during_unwind() {
    use std::os::unix::process::ExitStatusExt;
    const CHILD: &str = "FE2O3_TEST_XGMI_RETIREMENT_DROP";
    const TEST: &str = "sdma::xgmi_retirement::tests::occupied_root_drop_aborts_even_after_complete_cleanup_or_during_unwind";
    let Some(stage) = std::env::var_os(CHILD) else {
        for stage in [
            "vacant",
            "success",
            "opening",
            "destroy",
            "resources",
            "closing",
            "unwind",
        ] {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", TEST, "--nocapture"])
                .env(CHILD, stage)
                .output()
                .unwrap();
            if matches!(stage, "vacant" | "success") {
                assert!(
                    output.status.success(),
                    "{}",
                    String::from_utf8_lossy(&output.stderr)
                );
            } else {
                assert_eq!(
                    output.status.signal(),
                    Some(6),
                    "{stage}: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }
        return;
    };
    rustix::process::setrlimit(
        rustix::process::Resource::Core,
        rustix::process::Rlimit {
            current: Some(0),
            maximum: Some(0),
        },
    )
    .unwrap();
    if stage == "vacant" {
        drop(RetirementRoot::new());
        return;
    }
    let route = crate::topology::tests::admitted_xgmi_routes()[0];
    let mut script = Script::new(route);
    let mut queue = ManuallyDrop::new(script.queue());
    let name = match stage.to_str().unwrap() {
        "success" => "success",
        "opening" => "opening",
        "destroy" => "destroy",
        "resources" => "resources",
        "closing" => "closing",
        "unwind" => "unwind",
        _ => panic!("invalid child mode"),
    };
    if name != "success" {
        script.fault = Some((if name == "unwind" { "opening" } else { name }, false));
    }
    let result = retire_with(&mut queue, &mut script);
    if name == "success" {
        result.unwrap();
    } else {
        assert!(result.is_err());
        script.assert_terminal(&queue);
    }
    let queue = ManuallyDrop::into_inner(queue);
    if name == "unwind" {
        let _ = catch_unwind(AssertUnwindSafe(move || {
            let _owned = queue;
            panic!("unwind occupied queue");
        }));
        panic!("occupied root unwind returned");
    }
    drop(queue);
    assert_eq!(name, "success", "occupied root Drop returned");
}

#[test]
fn public_operation_families_guard_before_effectful_route_validation() {
    // Source wiring complements the executable pure-state/reentry tests; it is
    // not a live-driver fault-injection claim.
    let source = include_str!("../../sdma.rs");
    let queue = source
        .split("impl Gfx942NativeXgmiSdmaQueueV1 {")
        .nth(1)
        .unwrap();
    let method_body = |method: &str| {
        queue
            .split(method)
            .nth(1)
            .unwrap_or_else(|| panic!("{method}: missing method"))
            .split("\n    }\n")
            .next()
            .unwrap()
    };
    for (method, effect) in [
        (
            "fn begin_batch_timed",
            "source.validate_gfx942_xgmi_route_with_peer_timed",
        ),
        (
            "fn submit_with_currentness",
            "Self::validate_route_currentness",
        ),
        ("fn submit_batch_with_timer", "timer.measure"),
        ("fn poll_with_timer", "timer.measure"),
        (
            "pub fn observe_progress",
            "Self::validate_route_currentness",
        ),
        (
            "fn wait_batch_for_with_currentness",
            "Self::validate_route_currentness",
        ),
        (
            "fn wait_for_with_currentness",
            "Self::validate_route_currentness",
        ),
    ] {
        let body = method_body(method);
        let guard = body
            .find("self.require_live_queue_state_v1()")
            .unwrap_or_else(|| panic!("{method}: missing live-state guard"));
        let effect = body
            .find(effect)
            .unwrap_or_else(|| panic!("{method}: missing route effect"));
        assert!(guard < effect, "{method}");
    }
    for (method, mode) in [
        ("pub fn begin_batch<'a>", "Disabled"),
        (
            "pub fn begin_batch_currentness_diagnostic_v1<'a>",
            "Enabled",
        ),
    ] {
        let body = method_body(method);
        assert!(
            body.contains(&format!(
                "self.begin_batch_timed::<crate::currentness_diagnostic::{mode}>(source, destination)"
            )),
            "{method}"
        );
        assert!(!body.contains("validate_gfx942_xgmi_route"), "{method}");
    }
}
