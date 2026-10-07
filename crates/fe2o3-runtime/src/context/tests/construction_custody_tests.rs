use super::*;
use crate::{
    RuntimeAsyncCurrentThreadInitErrorV1, RuntimeAsyncCurrentThreadOwnedEngineV1,
    RuntimeAsyncEngineConfigV1, RuntimeAsyncOwnedEngineV1, RuntimeAsyncOwnedSpawnErrorV1,
    RuntimeAsyncProgressConfigV1, RuntimeWorkerRequestOwnerV1,
};
use fe2o3_resource_accounting::{
    ResourceCreditAccountV1, ResourceKindV1, ResourceVectorV1, RetainedResourceCreditsV1,
};
use std::{
    cell::RefCell,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
    sync::{Arc, Mutex, Weak, atomic::AtomicUsize},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Hook {
    Enumerate,
    Profile,
}

#[derive(Default)]
struct Probe {
    drops: AtomicUsize,
    calls: Mutex<Vec<&'static str>>,
}

struct ConstructionBackend {
    inner: MockBackend,
    hook: Hook,
    failure: MockMemoryFailure,
    payload: RefCell<Option<Box<u64>>>,
    probe: Arc<Probe>,
    identity: Arc<()>,
    credits: Option<RetainedResourceCreditsV1>,
    abort_on_drop: bool,
    malformed_profile: bool,
    _thread_local: Rc<()>,
}

struct Observation {
    probe: Arc<Probe>,
    identity: Weak<()>,
    payload_address: usize,
    account: ResourceCreditAccountV1,
}

impl ConstructionBackend {
    fn new(hook: Hook, failure: MockMemoryFailure) -> (Self, Observation) {
        let probe = Arc::new(Probe::default());
        let identity = Arc::new(());
        let payload = Box::new(0x1234);
        let charge = ResourceVectorV1::ZERO.with(ResourceKindV1::AllocationRecords, 1);
        let account = ResourceCreditAccountV1::new(charge, 1).unwrap();
        let observation = Observation {
            probe: probe.clone(),
            identity: Arc::downgrade(&identity),
            payload_address: (&*payload as *const u64) as usize,
            account: account.clone(),
        };
        (
            Self {
                inner: MockBackend::default(),
                hook,
                failure,
                payload: RefCell::new(Some(payload)),
                probe,
                identity,
                credits: Some(account.reserve(charge).unwrap().retain()),
                abort_on_drop: false,
                malformed_profile: false,
                _thread_local: Rc::new(()),
            },
            observation,
        )
    }

    fn enter(&self, hook: Hook) -> Result<(), RuntimeBackendFailureV1<MockError>> {
        self.probe.calls.lock().unwrap().push(match hook {
            Hook::Enumerate => "enumerate",
            Hook::Profile => "profile",
        });
        if self.hook != hook {
            return Ok(());
        }
        if self.failure == MockMemoryFailure::Panic {
            resume_unwind(self.payload.borrow_mut().take().unwrap());
        }
        mock_memory_failure_v1(self.failure)
    }
}

impl Drop for ConstructionBackend {
    fn drop(&mut self) {
        self.probe.drops.fetch_add(1, Ordering::SeqCst);
        if self.abort_on_drop {
            std::process::abort();
        }
    }
}

macro_rules! delegate {
    ($($name:ident($($arg:ident: $ty:ty),*) -> $out:ty;)*) => {$ (
        fn $name(&mut self, $($arg: $ty),*) -> Result<$out, RuntimeBackendFailureV1<Self::Error>> {
            self.inner.$name($($arg),*)
        }
    )*};
}

impl RuntimeBackendV1 for ConstructionBackend {
    type Error = MockError;

    fn enumerate_devices_v1(
        &mut self,
    ) -> Result<Vec<BackendDeviceDescriptionV1>, RuntimeBackendFailureV1<Self::Error>> {
        self.enter(Hook::Enumerate)?;
        self.inner.enumerate_devices_v1()
    }

    fn allocation_admission_profile_v1(
        &self,
    ) -> Result<RuntimeAllocationAdmissionProfileV1, RuntimeBackendFailureV1<Self::Error>> {
        self.enter(Hook::Profile)?;
        Ok(if self.malformed_profile {
            RuntimeAllocationAdmissionProfileV1::Required(Vec::new())
        } else {
            RuntimeAllocationAdmissionProfileV1::Legacy
        })
    }

    delegate! {
        create_stream_v1(device: u64) -> u64;
        destroy_stream_v1(stream: u64) -> ();
        allocate_v1(device: u64, kind: RuntimeMemoryKindV1, bytes: u64, alignment: u64) -> u64;
        release_allocation_v1(allocation: u64) -> ();
        write_allocation_v1(allocation: u64, offset: u64, bytes: &[u8]) -> ();
        read_allocation_v1(allocation: u64, offset: u64, bytes: &mut [u8]) -> ();
        load_module_v1(device: u64, image: &[u8]) -> u64;
        unload_module_v1(module: u64) -> ();
        resolve_kernel_v1(module: u64, name: &str, signature: [u8; 32]) -> u64;
        submit_v1(launch: BackendLaunchV1<'_>) -> u64;
        poll_v1(submission: u64) -> BackendPollV1;
        wait_v1(submission: u64, deadline: Instant) -> BackendPollV1;
        release_submission_v1(submission: u64) -> ();
        record_event_v1(stream: u64, submission: u64) -> u64;
        release_event_v1(event: u64) -> ();
        peer_copy_v1(stream: u64, source: BackendMemoryRegionV1, destination: BackendMemoryRegionV1, dependencies: &[u64]) -> u64;
    }
}

impl RuntimeFlushBackendV1 for ConstructionBackend {
    delegate! { flush_stream_v1(stream: u64) -> (); }
}

impl RuntimeOwnedShutdownBackendV1 for ConstructionBackend {
    fn shutdown_owned_v1(&mut self) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.probe.calls.lock().unwrap().push("shutdown");
        self.credits
            .take()
            .unwrap()
            .release_after_disposal()
            .unwrap();
        Ok(())
    }
}

impl Observation {
    fn assert_retained(&self, calls: &[&str]) {
        assert_eq!(
            self.probe.drops.load(Ordering::SeqCst),
            0,
            "backend dropped"
        );
        assert_eq!(self.identity.strong_count(), 1);
        assert_eq!(*self.probe.calls.lock().unwrap(), calls);
        let usage = self.account.usage();
        assert_eq!(usage.retained_records, 1);
        assert_eq!(usage.quarantined_records, 0);
    }

    fn assert_original(&self, backend: &ConstructionBackend) {
        assert!(Weak::ptr_eq(
            &self.identity,
            &Arc::downgrade(&backend.identity)
        ));
        assert_eq!(
            (&**backend.payload.borrow().as_ref().unwrap() as *const u64) as usize,
            self.payload_address
        );
    }

    fn assert_dropped(&self) {
        assert_eq!(self.probe.drops.load(Ordering::SeqCst), 1);
        assert_eq!(self.identity.strong_count(), 0);
    }
}

fn calls(hook: Hook) -> &'static [&'static str] {
    match hook {
        Hook::Enumerate => &["enumerate"],
        Hook::Profile => &["enumerate", "profile"],
    }
}

#[test]
fn constructor_panics_retain_backend_credits_and_original_payload() {
    for hook in [Hook::Enumerate, Hook::Profile] {
        for path in 0..3 {
            let (backend, observation) = ConstructionBackend::new(hook, MockMemoryFailure::Panic);
            let panic = catch_unwind(AssertUnwindSafe(|| match path {
                0 => {
                    let _ = RuntimeContextV1::open(backend);
                }
                1 => {
                    let _ = RuntimeContextV1::open_with_version_journal_v1(backend, 2, 2);
                }
                _ => {
                    let _ = RuntimeWorkerRequestOwnerV1::open(backend);
                }
            }))
            .expect_err("injected panic must propagate");
            let payload = panic.downcast::<u64>().unwrap();
            assert_eq!(
                (&*payload as *const u64) as usize,
                observation.payload_address
            );
            observation.assert_retained(calls(hook));
        }
    }
}

#[test]
fn returned_backend_errors_preserve_original_owner_and_failure_class() {
    for hook in [Hook::Enumerate, Hook::Profile] {
        for failure in [
            MockMemoryFailure::Rejected,
            MockMemoryFailure::Quiescent,
            MockMemoryFailure::Terminal,
        ] {
            for worker in [false, true] {
                let (backend, observation) = ConstructionBackend::new(hook, failure);
                let error = if worker {
                    RuntimeWorkerRequestOwnerV1::open(backend).err().unwrap()
                } else {
                    RuntimeContextV1::open_with_version_journal_v1(backend, 2, 2)
                        .err()
                        .unwrap()
                };
                observation.assert_retained(calls(hook));
                let (backend, error) = error.into_parts();
                observation.assert_original(&backend);
                match (failure, error) {
                    (MockMemoryFailure::Rejected, RuntimeErrorV1::BackendRejected(e)) => {
                        assert_eq!(e.0, "allocation rejected")
                    }
                    (MockMemoryFailure::Quiescent, RuntimeErrorV1::BackendQuiescent(e)) => {
                        assert_eq!(e.0, "allocation quiescent failure")
                    }
                    (MockMemoryFailure::Terminal, RuntimeErrorV1::BackendTerminal(e)) => {
                        assert_eq!(e.0, "allocation terminal failure")
                    }
                    pair => panic!("failure changed: {pair:?}"),
                }
                drop(backend);
                observation.assert_dropped();
                assert_eq!(observation.account.usage().quarantined_records, 1);
            }
        }
    }
}

#[test]
fn validation_errors_return_backend_without_accidental_retention() {
    for case in 0..7 {
        let (mut backend, observation) =
            ConstructionBackend::new(Hook::Profile, MockMemoryFailure::None);
        let (a, w) = match case {
            0 => (0, 1),
            1 => (1, 0),
            2 => (usize::MAX, 1),
            3 => (1, usize::MAX),
            _ => (2, 2),
        };
        backend.inner.device_name_len = if case == 4 {
            MAX_RUNTIME_DEVICE_NAME_BYTES_V1 + 1
        } else {
            0
        };
        backend.malformed_profile = case == 5;
        let error = if case == 6 {
            RuntimeWorkerRequestOwnerV1::open(backend).err().unwrap()
        } else {
            RuntimeContextV1::open_with_version_journal_v1(backend, a, w)
                .err()
                .unwrap()
        };
        let (backend, error) = error.into_parts();
        assert!(matches!(error, RuntimeErrorV1::Validation(_)));
        observation.assert_original(&backend);
        observation.assert_retained(match case {
            0..=3 => &[],
            4 => &["enumerate"],
            _ => &["enumerate", "profile"],
        });
        drop(backend);
        observation.assert_dropped();
    }
}

#[test]
fn success_transfers_backend_once_and_legacy_error_keeps_drop_semantics() {
    for journal in [false, true] {
        let (backend, observation) =
            ConstructionBackend::new(Hook::Profile, MockMemoryFailure::None);
        let context = if journal {
            RuntimeContextV1::open_with_version_journal_v1(backend, 2, 2).unwrap()
        } else {
            RuntimeContextV1::open(backend).unwrap()
        };
        observation.assert_retained(&["enumerate", "profile"]);
        let mut backend = context.shutdown().ok().unwrap();
        observation.assert_original(&backend);
        observation.assert_retained(&["enumerate", "profile"]);
        backend.shutdown_owned_v1().unwrap();
        drop(backend);
        observation.assert_dropped();
        assert_eq!(observation.account.usage().used, ResourceVectorV1::ZERO);
    }
    for hook in [Hook::Enumerate, Hook::Profile] {
        let (backend, observation) = ConstructionBackend::new(hook, MockMemoryFailure::Rejected);
        assert!(RuntimeContextV1::open(backend).is_err());
        observation.assert_dropped();
        assert_eq!(observation.account.usage().quarantined_records, 1);
    }
}

#[test]
fn async_factories_report_panics_without_dropping_backend() {
    for hook in [Hook::Enumerate, Hook::Profile] {
        for background in [false, true] {
            let config =
                RuntimeAsyncEngineConfigV1::new(2, 2, 1, 1, Duration::from_millis(1)).unwrap();
            let progress = RuntimeAsyncProgressConfigV1::new(2, 1).unwrap();
            if background {
                let (sender, receiver) = std::sync::mpsc::channel();
                let result = RuntimeAsyncOwnedEngineV1::spawn_with_progress(
                    move || {
                        let (backend, observation) =
                            ConstructionBackend::new(hook, MockMemoryFailure::Panic);
                        sender.send(observation).ok().unwrap();
                        RuntimeContextV1::open(backend)
                    },
                    config,
                    progress,
                );
                assert!(matches!(
                    result,
                    Err(RuntimeAsyncOwnedSpawnErrorV1::InitializerPanicked)
                ));
                receiver.recv().unwrap().assert_retained(calls(hook));
            } else {
                let (backend, observation) =
                    ConstructionBackend::new(hook, MockMemoryFailure::Panic);
                let result = RuntimeAsyncCurrentThreadOwnedEngineV1::new_with_progress(
                    || RuntimeContextV1::open(backend),
                    config,
                    progress,
                );
                assert!(matches!(
                    result,
                    Err(RuntimeAsyncCurrentThreadInitErrorV1::InitializerPanicked)
                ));
                observation.assert_retained(calls(hook));
            }
        }
    }
}

#[test]
fn aborting_backend_destructor_is_not_entered_on_constructor_unwind() {
    const CHILD: &str = "FE2O3_CONSTRUCTION_CUSTODY_ABORT_CHILD";
    if std::env::var_os(CHILD).is_some() {
        for hook in [Hook::Enumerate, Hook::Profile] {
            let (mut backend, observation) =
                ConstructionBackend::new(hook, MockMemoryFailure::Panic);
            backend.abort_on_drop = true;
            assert!(catch_unwind(AssertUnwindSafe(|| RuntimeContextV1::open(backend))).is_err());
            observation.assert_retained(calls(hook));
        }
        return;
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "context::tests::construction_custody_tests::aborting_backend_destructor_is_not_entered_on_constructor_unwind", "--nocapture"])
        .env(CHILD, "1")
        .output().unwrap();
    assert!(
        output.status.success(),
        "child failed: {:?}\n{}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
