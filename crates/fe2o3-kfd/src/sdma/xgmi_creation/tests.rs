use super::*;
use crate::queue_linux::doorbell_release_tests::{
    cleanup_local_doorbell, local_doorbell, observe_local_doorbell,
};
use crate::sdma::creation::{SdmaNativeCreationV1, finish_native_attempt};
use crate::sdma::retained_release::fixture::{
    self, CreationAttemptObservation, CreationNativeFault, assert_creation_owner_matches_attempt,
    creation_attempt_observation, creation_owner_observation,
};
use crate::shared_memory::PreparationMemoryFixtureV1;
use std::cell::RefCell;
use std::mem::ManuallyDrop;
use std::rc::Rc;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

const ID: u32 = 100;
type Trace = Rc<RefCell<Vec<&'static str>>>;

struct Arm {
    trace: Trace,
    disarmed: bool,
}
impl Drop for Arm {
    fn drop(&mut self) {
        self.trace.borrow_mut().push(if self.disarmed {
            "arm-disarmed"
        } else {
            "arm-poison"
        });
    }
}

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

struct Script {
    route: Gfx942XgmiRouteV1,
    trace: Trace,
    fault: Option<(&'static str, bool)>,
    native_fault: CreationNativeFault,
    closing: bool,
    expected: Option<CreationAttemptObservation>,
    actual_address: usize,
    memory: PreparationMemoryFixtureV1,
    hostile: [bool; 3],
    drops: Arc<AtomicUsize>,
}

impl Script {
    fn new(route: Gfx942XgmiRouteV1) -> Self {
        Self {
            route,
            trace: Rc::default(),
            fault: None,
            native_fault: CreationNativeFault::Success,
            closing: false,
            expected: None,
            actual_address: 0,
            memory: PreparationMemoryFixtureV1::new(true),
            hostile: [false; 3],
            drops: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn step(&mut self, stage: &'static str) -> Result<(), Gfx942SdmaErrorV1> {
        self.trace.borrow_mut().push(stage);
        match self.fault {
            Some((name, true)) if name == stage => std::panic::panic_any(stage),
            Some((name, false)) if name == stage => Err(Gfx942SdmaErrorV1::Contract(stage)),
            _ => Ok(()),
        }
    }

    fn check_root(&self, root: &Gfx942NativeXgmiSdmaQueueCreationRootV1) {
        match &root.state {
            State::Pending { route, attempted } => {
                assert_eq!(*route, self.route);
                assert_eq!(
                    attempted.as_ref().map(creation_attempt_observation),
                    self.expected
                );
            }
            State::Confirmed { route, owner } => {
                assert_eq!(*route, self.route);
                assert_creation_owner_matches_attempt(owner, self.expected.as_ref().unwrap(), ID);
            }
            State::Vacant => panic!("terminalizer observed vacant root"),
        }
    }

    fn terminalize(&mut self, stage: usize, root: &Gfx942NativeXgmiSdmaQueueCreationRootV1) {
        self.check_root(root);
        self.trace
            .borrow_mut()
            .push(["source", "destination", "global"][stage]);
        if self.hostile[stage] {
            std::panic::panic_any(HostilePayload {
                stage,
                drops: self.drops.clone(),
            });
        }
    }

    fn expected_native(
        &mut self,
    ) -> (
        &mut Gfx942SdmaQueueCreationNativeObservationV1,
        &mut Option<crate::queue_linux::doorbell_release_tests::LocalDoorbellObservation>,
    ) {
        let Some(CreationAttemptObservation::Prepared {
            native, doorbell, ..
        }) = &mut self.expected
        else {
            panic!("prepared observation")
        };
        (native, doorbell)
    }
}

impl Context for Script {
    type Host = ();
    type Arm = Arm;
    fn prepare(
        &mut self,
        route: Gfx942XgmiRouteV1,
    ) -> Result<(KfdGfx942SdmaXgmiEngineId, ()), Gfx942SdmaErrorV1> {
        assert_eq!(route, self.route);
        self.step("host")?;
        Ok((
            admit_kfd_gfx942_sdma_xgmi_engine_mask(route.link().recommended_sdma_engine_id_mask())
                .unwrap(),
            (),
        ))
    }
    fn arm(&mut self) -> Result<Arm, Gfx942SdmaErrorV1> {
        self.step("arm")?;
        Ok(Arm {
            trace: self.trace.clone(),
            disarmed: false,
        })
    }
    fn validate(&mut self, route: Gfx942XgmiRouteV1) -> Result<(), Gfx942SdmaErrorV1> {
        assert_eq!(route, self.route);
        let closing = self.closing;
        self.closing = true;
        self.step(if closing { "closing" } else { "opening" })
    }
    fn key(&mut self) -> Result<QueueKeyV1, Gfx942SdmaErrorV1> {
        use fe2o3_runtime_model::*;
        self.step("key")?;
        Ok(QueueKeyV1 {
            vm: VmKeyV1 {
                device: DeviceKeyV1 {
                    physical: PhysicalDeviceIdV1(u64::from(self.route.source_gpu_id())),
                    generation: DeviceGenerationV1(1),
                },
                id: VmIdV1(1),
            },
            id: QueueInstanceIdV1(71),
            generation: QueueGenerationV1(72),
        })
    }
    fn create_owner(
        &mut self,
        key: QueueKeyV1,
        engine: KfdGfx942SdmaXgmiEngineId,
        _host: (),
        _arm: &Arm,
        attempted: &mut Option<TerminalGfx942SdmaQueueCreationV1>,
    ) -> Result<Gfx942SdmaQueueOwnerV1, Gfx942SdmaErrorV1> {
        self.step("lower")?;
        *attempted = Some(TerminalGfx942SdmaQueueCreationV1::OpaqueAfterFirstMemoryOperation);
        self.expected = Some(CreationAttemptObservation::Opaque);
        self.step("opaque")?;
        let Gfx942SdmaQueueSetV1::Generic(mut owners) =
            fixture::generic(&mut self.memory, key, Some(engine.value()))
        else {
            panic!("fixture owner")
        };
        let mut owner = owners.pop().unwrap();
        cleanup_local_doorbell(owner.doorbell.as_mut().unwrap());
        let expected = KfdIoctlCreateQueueArgs::new_sdma_xgmi_on_engine(
            KfdSdmaQueueBuffers {
                ring_base_address: 0x1000,
                write_pointer_address: 0x2000,
                read_pointer_address: 0x2080,
            },
            admit_kfd_aql_queue_ring_size(4096).unwrap(),
            self.route.source_gpu_id(),
            admit_kfd_queue_percentage(100).unwrap(),
            admit_kfd_queue_priority(0).unwrap(),
            engine,
        );
        *attempted = Some(TerminalGfx942SdmaQueueCreationV1::QueueAttempt {
            prepared: PreparedGfx942SdmaQueueV1 {
                owner: key,
                engine_index: owner.engine_index,
                ring: owner.ring.take().unwrap(),
                control: owner.control.take().unwrap(),
                completions: owner.completions.take().unwrap(),
                records: owner.records,
                xgmi_records: owner.xgmi_records,
                persistent_window_slots: owner.persistent_window_slots,
                persistent_window_records: owner.persistent_window_records,
            },
            native: Gfx942SdmaQueueCreationNativeObservationV1::UntrustedIoctlOutputs {
                actual: expected,
            },
            doorbell: None,
        });
        self.expected = attempted.as_ref().map(creation_attempt_observation);
        let Some(TerminalGfx942SdmaQueueCreationV1::QueueAttempt {
            native: Gfx942SdmaQueueCreationNativeObservationV1::UntrustedIoctlOutputs { actual },
            ..
        }) = attempted
        else {
            panic!("prepared actual arguments")
        };
        self.actual_address = actual as *mut _ as usize;
        finish_native_attempt(self, attempted, expected, "fixture doorbell".into())
    }
    fn disarm(&mut self, mut arm: Arm) {
        self.step("disarm").unwrap();
        arm.disarmed = true;
    }
    fn quarantine(&mut self, destination: bool, root: &Gfx942NativeXgmiSdmaQueueCreationRootV1) {
        self.terminalize(usize::from(destination), root);
    }
    fn poison(&mut self, root: &Gfx942NativeXgmiSdmaQueueCreationRootV1) {
        self.terminalize(2, root);
    }
}

impl SdmaNativeCreationV1 for Script {
    fn create(&mut self, actual: &mut KfdIoctlCreateQueueArgs) -> Result<(), rustix::io::Errno> {
        self.trace.borrow_mut().push("ioctl");
        assert_eq!(actual as *mut _ as usize, self.actual_address);
        if self.native_fault == CreationNativeFault::BeforeIoctlPanic {
            std::panic::panic_any(self.native_fault);
        }
        actual.queue_id = ID;
        actual.doorbell_offset = (fe2o3_kfd_uapi::KFD_MMAP_TYPE_DOORBELL
            << fe2o3_kfd_uapi::KFD_MMAP_TYPE_SHIFT)
            | (u64::from(actual.gpu_id & 0xffff) << fe2o3_kfd_uapi::KFD_MMAP_GPU_ID_HASH_SHIFT);
        if self.native_fault == CreationNativeFault::ImmutableMutation {
            actual.ring_size = 8192;
        }
        if self.native_fault == CreationNativeFault::InvalidOutput {
            actual.queue_id = u32::MAX;
        }
        *self.expected_native().0 =
            Gfx942SdmaQueueCreationNativeObservationV1::UntrustedIoctlOutputs { actual: *actual };
        if self.native_fault == CreationNativeFault::AfterIoctlPanic {
            std::panic::panic_any(self.native_fault);
        }
        if self.native_fault == CreationNativeFault::IoctlError {
            return Err(rustix::io::Errno::IO);
        }
        Ok(())
    }
    fn doorbell(
        &mut self,
        outputs: fe2o3_kfd_uapi::KfdGfx942CreateQueueOutputs,
    ) -> Result<LinuxDoorbellSliceV1, ()> {
        self.trace.borrow_mut().push("doorbell");
        assert_eq!(outputs.queue_id().value(), ID);
        *self.expected_native().0 =
            Gfx942SdmaQueueCreationNativeObservationV1::ValidatedQueueId(ID);
        if self.native_fault == CreationNativeFault::DoorbellPanic {
            std::panic::panic_any(self.native_fault);
        }
        if self.native_fault == CreationNativeFault::DoorbellError {
            return Err(());
        }
        let doorbell = local_doorbell();
        *self.expected_native().1 = Some(observe_local_doorbell(&doorbell));
        Ok(doorbell)
    }
    fn currentness(&mut self) -> Result<(), Gfx942SdmaErrorV1> {
        self.trace.borrow_mut().push("currentness");
        if self.native_fault == CreationNativeFault::CurrentnessPanic {
            std::panic::panic_any(self.native_fault);
        }
        if self.native_fault == CreationNativeFault::CurrentnessError {
            return Err(Gfx942SdmaErrorV1::Contract("currentness"));
        }
        assert_eq!(self.native_fault, CreationNativeFault::Success);
        Ok(())
    }
}

fn dispose_fixture(root: &mut Gfx942NativeXgmiSdmaQueueCreationRootV1) {
    // Local fixture mappings only. This is not a production cleanup path.
    match &mut root.state {
        State::Pending {
            attempted:
                Some(TerminalGfx942SdmaQueueCreationV1::QueueAttempt {
                    doorbell: Some(doorbell),
                    ..
                }),
            ..
        } => cleanup_local_doorbell(doorbell),
        State::Confirmed { owner, .. } => cleanup_local_doorbell(owner.doorbell.as_mut().unwrap()),
        _ => {}
    }
    root.state = State::Vacant;
}

fn assert_inert_retry(script: &mut Script, root: &mut Gfx942NativeXgmiSdmaQueueCreationRootV1) {
    let trace = script.trace.borrow().clone();
    let stage = root.terminal_stage();
    let confirmed = match &root.state {
        State::Confirmed { owner, .. } => Some(creation_owner_observation(owner)),
        _ => None,
    };
    let other_route = crate::topology::tests::admitted_xgmi_routes()
        .into_iter()
        .find(|r| *r != script.route)
        .unwrap();
    let error = create_with(script, other_route, root).err().unwrap();
    assert!(error.is_terminal());
    assert_eq!(error.terminal_stage(), stage);
    assert_eq!(*script.trace.borrow(), trace);
    script.check_root(root);
    if let State::Confirmed { owner, .. } = &root.state {
        assert_eq!(Some(creation_owner_observation(owner)), confirmed);
    }
}

#[test]
fn vacant_root_is_small_inline_and_diagnostic_only() {
    assert!(std::mem::size_of::<Gfx942NativeXgmiSdmaQueueCreationRootV1>() <= 2048);
    assert!(std::mem::size_of::<[Gfx942NativeXgmiSdmaQueueCreationRootV1; 2]>() <= 4096);
    let root = Gfx942NativeXgmiSdmaQueueCreationRootV1::default();
    assert!(root.is_vacant());
    assert_eq!(root.terminal_stage(), None);
    assert!(format!("{root:?}").contains("terminal_stage: None"));
}

#[test]
fn preflight_errors_and_panics_leave_vacant_reusable_roots() {
    for route in crate::topology::tests::admitted_xgmi_routes() {
        for stage in ["host", "arm"] {
            for panic in [false, true] {
                let mut script = Script::new(route);
                let mut root = ManuallyDrop::new(Gfx942NativeXgmiSdmaQueueCreationRootV1::new());
                script.fault = Some((stage, panic));
                let result = catch_unwind(AssertUnwindSafe(|| {
                    create_with(&mut script, route, &mut root)
                }));
                if panic {
                    assert_eq!(result.err().unwrap().downcast_ref::<&str>(), Some(&stage));
                } else {
                    assert!(!result.unwrap().err().unwrap().is_terminal());
                }
                assert!(root.is_vacant());
                assert_eq!(
                    *script.trace.borrow(),
                    if stage == "host" {
                        vec!["host"]
                    } else {
                        vec!["host", "arm"]
                    }
                );
                script.fault = None;
                script.trace.borrow_mut().clear();
                let mut queue = create_with(&mut script, route, &mut root).unwrap();
                assert_eq!(queue.route(), route);
                assert!(root.is_vacant());
                assert_creation_owner_matches_attempt(
                    queue.owner.as_ref().unwrap(),
                    script.expected.as_ref().unwrap(),
                    ID,
                );
                assert_eq!(
                    *script.trace.borrow(),
                    [
                        "host",
                        "arm",
                        "opening",
                        "key",
                        "lower",
                        "opaque",
                        "ioctl",
                        "doorbell",
                        "currentness",
                        "closing",
                        "disarm",
                        "arm-disarmed"
                    ]
                );
                cleanup_local_doorbell(queue.owner.as_mut().unwrap().doorbell.as_mut().unwrap());
                drop(ManuallyDrop::into_inner(root));
            }
        }
    }
}

#[test]
fn route_key_opaque_and_confirmed_failures_retain_custody() {
    for route in crate::topology::tests::admitted_xgmi_routes() {
        for stage in ["opening", "key", "lower", "opaque", "closing", "disarm"] {
            for panic in [false, true] {
                if stage == "disarm" && !panic {
                    continue;
                }
                let mut script = Script::new(route);
                script.fault = Some((stage, panic));
                let mut root = ManuallyDrop::new(Gfx942NativeXgmiSdmaQueueCreationRootV1::new());
                let result = catch_unwind(AssertUnwindSafe(|| {
                    create_with(&mut script, route, &mut root)
                }));
                if panic {
                    assert_eq!(result.err().unwrap().downcast_ref::<&str>(), Some(&stage));
                } else {
                    let error = result.unwrap().err().unwrap();
                    assert!(error.is_terminal());
                    assert_eq!(error.terminal_stage(), root.terminal_stage());
                    drop(error);
                }
                script.check_root(&root);
                let expected_stage = match stage {
                    "opening" | "key" | "lower" => "memory-terminal-no-queue-custody",
                    "opaque" => "create-attempt-retained",
                    _ => "confirmed-queue-retained",
                };
                assert_eq!(root.terminal_stage(), Some(expected_stage));
                if stage != "disarm" {
                    assert!(script.trace.borrow().ends_with(&[
                        "source",
                        "destination",
                        "global",
                        "arm-poison"
                    ]));
                } else {
                    assert!(script.trace.borrow().ends_with(&[
                        "arm-poison",
                        "source",
                        "destination",
                        "global"
                    ]));
                }
                assert_inert_retry(&mut script, &mut root);
                dispose_fixture(&mut root);
                drop(ManuallyDrop::into_inner(root));
            }
        }
    }
}

#[test]
fn native_prefix_errors_and_panics_preserve_exact_attempt_and_doorbell() {
    use CreationNativeFault::*;
    for route in crate::topology::tests::admitted_xgmi_routes() {
        for fault in [
            BeforeIoctlPanic,
            AfterIoctlPanic,
            IoctlError,
            ImmutableMutation,
            InvalidOutput,
            DoorbellPanic,
            DoorbellError,
            CurrentnessPanic,
            CurrentnessError,
        ] {
            let mut script = Script::new(route);
            script.native_fault = fault;
            let mut root = ManuallyDrop::new(Gfx942NativeXgmiSdmaQueueCreationRootV1::new());
            let result = catch_unwind(AssertUnwindSafe(|| {
                create_with(&mut script, route, &mut root)
            }));
            if fault.panics() {
                assert_eq!(
                    result.err().unwrap().downcast_ref::<CreationNativeFault>(),
                    Some(&fault)
                );
            } else {
                assert!(result.unwrap().err().unwrap().is_terminal());
            }
            script.check_root(&root);
            assert_eq!(root.terminal_stage(), Some("create-attempt-retained"));
            assert!(script.trace.borrow().ends_with(&[
                "source",
                "destination",
                "global",
                "arm-poison"
            ]));
            assert_inert_retry(&mut script, &mut root);
            dispose_fixture(&mut root);
            drop(ManuallyDrop::into_inner(root));
        }
    }
}

#[test]
fn independent_terminalizers_preserve_original_panic_and_forget_secondary_payloads() {
    let route = crate::topology::tests::admitted_xgmi_routes()[0];
    for stage in ["opening", "opaque", "native", "closing"] {
        for original_panic in [false, true] {
            for mask in 1..8 {
                let mut script = Script::new(route);
                script.fault = Some((stage, original_panic));
                if stage == "native" {
                    script.native_fault = if original_panic {
                        CreationNativeFault::CurrentnessPanic
                    } else {
                        CreationNativeFault::CurrentnessError
                    };
                }
                script.hostile = std::array::from_fn(|i| mask & (1 << i) != 0);
                let mut root = ManuallyDrop::new(Gfx942NativeXgmiSdmaQueueCreationRootV1::new());
                let payload = ManuallyDrop::new(
                    catch_unwind(AssertUnwindSafe(|| {
                        create_with(&mut script, route, &mut root)
                    }))
                    .err()
                    .unwrap(),
                );
                if original_panic && stage == "native" {
                    assert_eq!(
                        payload.downcast_ref::<CreationNativeFault>(),
                        Some(&CreationNativeFault::CurrentnessPanic)
                    );
                } else if original_panic {
                    assert_eq!(payload.downcast_ref::<&str>(), Some(&stage));
                } else {
                    assert_eq!(
                        payload.downcast_ref::<HostilePayload>().unwrap().stage,
                        script.hostile.iter().position(|v| *v).unwrap()
                    );
                }
                assert_eq!(script.drops.load(Ordering::SeqCst), 0);
                assert!(script.trace.borrow().ends_with(&[
                    "source",
                    "destination",
                    "global",
                    "arm-poison"
                ]));
                script.check_root(&root);
                assert_inert_retry(&mut script, &mut root);
                dispose_fixture(&mut root);
                drop(ManuallyDrop::into_inner(root));
            }
        }
    }
}

#[test]
fn diagnostic_formatting_and_drop_leave_confirmed_custody_in_the_root() {
    struct PanicWriter;
    impl fmt::Write for PanicWriter {
        fn write_str(&mut self, _: &str) -> fmt::Result {
            std::panic::panic_any("diagnostic writer");
        }
    }
    let route = crate::topology::tests::admitted_xgmi_routes()[0];
    let mut script = Script::new(route);
    script.fault = Some(("closing", false));
    let mut root = ManuallyDrop::new(Gfx942NativeXgmiSdmaQueueCreationRootV1::new());
    let error = create_with(&mut script, route, &mut root).err().unwrap();
    let before = match &root.state {
        State::Confirmed { owner, .. } => creation_owner_observation(owner),
        _ => panic!("confirmed owner"),
    };
    assert_eq!(
        error.to_string(),
        Gfx942SdmaErrorV1::Contract("closing").to_string()
    );
    let panic = catch_unwind(AssertUnwindSafe(|| {
        use fmt::Write;
        write!(PanicWriter, "{error}")
    }))
    .unwrap_err();
    assert_eq!(panic.downcast_ref::<&str>(), Some(&"diagnostic writer"));
    drop(error);
    script.check_root(&root);
    let State::Confirmed { owner, .. } = &root.state else {
        panic!("confirmed owner")
    };
    assert_eq!(creation_owner_observation(owner), before);
    dispose_fixture(&mut root);
    drop(ManuallyDrop::into_inner(root));
}

#[test]
fn occupied_root_drop_aborts_without_native_cleanup() {
    use std::os::unix::process::ExitStatusExt;
    const CHILD: &str = "FE2O3_TEST_XGMI_CREATION_DROP";
    const TEST: &str =
        "sdma::xgmi_creation::tests::occupied_root_drop_aborts_without_native_cleanup";
    let Some(stage) = std::env::var_os(CHILD) else {
        for stage in ["vacant", "opening", "opaque", "native", "closing"] {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", TEST, "--nocapture"])
                .env(CHILD, stage)
                .output()
                .unwrap();
            if stage == "vacant" {
                assert!(output.status.success());
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
    let route = crate::topology::tests::admitted_xgmi_routes()[0];
    let mut script = Script::new(route);
    let mut root = ManuallyDrop::new(Gfx942NativeXgmiSdmaQueueCreationRootV1::new());
    if stage != "vacant" {
        match stage.to_str().unwrap() {
            "opening" => script.fault = Some(("opening", false)),
            "opaque" => script.fault = Some(("opaque", false)),
            "native" => script.native_fault = CreationNativeFault::CurrentnessError,
            "closing" => script.fault = Some(("closing", false)),
            _ => panic!("invalid child mode"),
        }
        assert!(
            create_with(&mut script, route, &mut root)
                .err()
                .unwrap()
                .is_terminal()
        );
        script.check_root(&root);
    }
    drop(ManuallyDrop::into_inner(root));
    assert_eq!(stage, "vacant", "occupied root Drop returned");
}
