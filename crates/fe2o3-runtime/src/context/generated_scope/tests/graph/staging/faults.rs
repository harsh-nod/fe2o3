//! Faults are explicit CPU backend premises, not observations of native hardware.

use super::*;
use crate::{
    BackendDeviceDescriptionV1, BackendLaunchV1, BackendMemoryRegionV1, BackendPollV1,
    RuntimeBackendFailureV1,
};

type Reply<T> = Result<T, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>;

#[derive(Clone, Copy)]
enum Fault {
    Rejected,
    Quiescent,
    Terminal,
    Panic,
}

struct BackendFault {
    inner: Backend,
    fault: Fault,
    writes: usize,
    copies: usize,
    error: KfdRuntimeBackendErrorV1,
}

impl BackendFault {
    fn new(fault: Fault) -> Self {
        let mut inner = Backend::mock_preparation_v1();
        let error = match inner.read_allocation_v1(u64::MAX, 0, &mut []) {
            Err(RuntimeBackendFailureV1::Rejected(error)) => error,
            _ => panic!("unknown mock allocation must refuse"),
        };
        Self {
            inner,
            fault,
            writes: 0,
            copies: 0,
            error,
        }
    }
}

macro_rules! forward {
    ($name:ident($($arg:ident: $ty:ty),*) -> $out:ty) => {
        fn $name(&mut self, $($arg: $ty),*) -> Reply<$out> {
            self.inner.$name($($arg),*)
        }
    };
}

impl RuntimeBackendV1 for BackendFault {
    type Error = KfdRuntimeBackendErrorV1;
    forward!(enumerate_devices_v1() -> Vec<BackendDeviceDescriptionV1>);
    forward!(create_stream_v1(device: u64) -> u64);
    forward!(destroy_stream_v1(stream: u64) -> ());
    forward!(allocate_v1(device: u64, kind: RuntimeMemoryKindV1, len: u64, alignment: u64) -> u64);
    forward!(release_allocation_v1(allocation: u64) -> ());
    forward!(read_allocation_v1(allocation: u64, offset: u64, bytes: &mut [u8]) -> ());
    forward!(load_module_v1(device: u64, bytes: &[u8]) -> u64);
    forward!(unload_module_v1(module: u64) -> ());
    forward!(resolve_kernel_v1(module: u64, name: &str, signature: [u8; 32]) -> u64);
    forward!(submit_v1(launch: BackendLaunchV1<'_>) -> u64);
    forward!(poll_v1(submission: u64) -> BackendPollV1);
    forward!(wait_v1(submission: u64, deadline: Instant) -> BackendPollV1);
    forward!(release_submission_v1(submission: u64) -> ());
    forward!(record_event_v1(stream: u64, submission: u64) -> u64);
    forward!(release_event_v1(event: u64) -> ());
    forward!(peer_copy_v1(stream: u64, source: BackendMemoryRegionV1, destination: BackendMemoryRegionV1, dependencies: &[u64]) -> u64);

    fn write_allocation_v1(&mut self, allocation: u64, offset: u64, bytes: &[u8]) -> Reply<()> {
        self.writes += 1;
        if !matches!(self.fault, Fault::Rejected) {
            self.inner
                .write_allocation_v1(allocation, offset, &bytes[..bytes.len().min(2)])?;
        }
        Err(match self.fault {
            Fault::Rejected => RuntimeBackendFailureV1::Rejected(self.error.clone()),
            Fault::Quiescent => RuntimeBackendFailureV1::Quiescent(self.error.clone()),
            Fault::Terminal => RuntimeBackendFailureV1::Terminal(self.error.clone()),
            Fault::Panic => panic!("injected partial hostwrite unwind"),
        })
    }
}

impl RuntimeFlushBackendV1 for BackendFault {
    forward!(flush_stream_v1(stream: u64) -> ());
    forward!(progress_stream_v1(stream: u64) -> ());
}

impl RuntimeAsyncCopyBackendV1 for BackendFault {
    fn copy_async_v1(
        &mut self,
        _: u64,
        _: BackendMemoryRegionV1,
        _: BackendMemoryRegionV1,
        _: &[u64],
    ) -> Reply<u64> {
        self.copies += 1;
        panic!("failed staging must not enter the copy backend")
    }
}

fn run(fault: Fault) {
    let decoded = Cell::new(0);
    let dropped = Cell::new(0);
    let owner = Arc::new(());
    let mut preparation_context = context();
    let device = preparation_context.devices()[0].id();
    let mut prepared = Some(preparation_context.bound_multi_preparation_for_test_v1(
        device,
        Borrowed {
            ticks: Cell::new(0),
            decoded: &decoded,
            dropped: &dropped,
            domain: Arc::clone(&owner),
            completion_order: None,
        },
    ));
    let mut context =
        RuntimeContextV1::open_with_version_journal_v1(BackendFault::new(fault), 8, 8).unwrap();
    let device = context.devices()[0].id();
    let stream = context.create_stream(device).unwrap();
    let staging = context
        .allocate(device, RuntimeMemoryKindV1::HostVisible, 32, 8)
        .unwrap();
    let destination = context
        .allocate(device, RuntimeMemoryKindV1::HostVisible, 32, 8)
        .unwrap();
    let mut request = chain(&context, stream);
    request
        .bind_host_staging_v1(id(2), id(1), region(staging, RuntimeAccessV1::Write))
        .unwrap();
    request
        .bind_copy(
            id(3),
            region(staging, RuntimeAccessV1::Read),
            region(destination, RuntimeAccessV1::Write),
        )
        .unwrap();
    let (slots, copies) = allocate_rosters(3).unwrap();
    let mut scope = RuntimeGfx942GeneratedScopeV1 {
        epoch: context.scope_epoch.begin().unwrap(),
        context: &mut context,
        slots,
        copies,
        graph: None,
        capacity: 3,
        deadline: Instant::now() + Duration::from_secs(30),
        identity: Rc::new(()),
        invariant: PhantomData,
        hooks: hooks(),
    };
    let ticket = scope
        .admit_graph_with_v1::<()>(request, |_, node, _| {
            assert_eq!(node, id(1));
            Ok(prepared.take().unwrap())
        })
        .unwrap();
    for _ in 0..32 {
        scope.progress_v1().unwrap();
    }
    assert_eq!((decoded.get(), dropped.get()), (1, 1));
    let mut scratch = [0x5a; 32];
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        scope.try_stage_graph_host_write_v1(
            &ticket,
            id(2),
            id(1),
            staging,
            &mut scratch,
            |domain, _| {
                assert!(domain.matches_owner(&owner));
                Ok::<_, ()>(Some(()))
            },
        )
    }));
    if matches!(fault, Fault::Panic) {
        assert!(result.is_err());
    } else {
        assert!(result.unwrap().is_err());
    }
    assert_eq!(
        (scope.context.backend.writes, scope.context.backend.copies),
        (1, 0)
    );
    assert!(
        scope
            .try_stage_graph_host_write_v1(
                &ticket,
                id(2),
                id(1),
                staging,
                &mut scratch,
                |_, _| -> Result<Option<()>, ()> { panic!("failed hostwrite cannot retry") }
            )
            .is_err()
    );
    assert_eq!(
        (scope.context.backend.writes, scope.context.backend.copies),
        (1, 0)
    );
    if matches!(fault, Fault::Rejected) {
        assert!(!scope.context.is_terminal());
        scope.drain_v1().unwrap();
        let report = scope.graph_report_v1(&ticket).unwrap().unwrap();
        assert!(
            report
                .version_inputs
                .iter()
                .all(|input| !input.available_at_issue)
        );
        assert!(
            report
                .versions
                .iter()
                .filter(|v| v.version.producer() == Some(id(2)))
                .all(|v| !v.current_at_terminal)
        );
        assert!(scope.context.submissions.is_empty());
        drop(scope);
        let mut actual = [0; 32];
        context.read_allocation(staging, 0, &mut actual).unwrap();
        assert_ne!(actual, [0x5a; 32]);
        assert!(context.cleanup().is_complete());
        assert!(preparation_context.cleanup().is_complete());
    } else {
        assert!(scope.context.is_terminal());
        assert!(scope.context.graph_reservation.is_some());
        assert!(matches!(
            scope.progress_v1(),
            Err(RuntimeGfx942ScopeErrorV1::Unknown)
        ));
        eprintln!("GRAPH_HOST_STAGING_UNKNOWN_RETAINED");
        drop(scope);
        panic!("unknown staging returned after releasing owners");
    }
}

#[test]
fn definitely_rejected_hostwrite_blocks_copy_without_retry() {
    run(Fault::Rejected);
}

#[test]
fn unknown_hostwrite_retains_graph_and_fails_stop_without_copy_or_retry() {
    const CHILD: &str = "FE2O3_GRAPH_HOST_STAGING_UNKNOWN";
    const TEST: &str = "context::generated_scope::tests::graph::staging::faults::unknown_hostwrite_retains_graph_and_fails_stop_without_copy_or_retry";
    if let Some(mode) = std::env::var_os(CHILD) {
        rustix::process::set_dumpable_behavior(rustix::process::DumpableBehavior::NotDumpable)
            .unwrap();
        rustix::process::setrlimit(
            rustix::process::Resource::Core,
            rustix::process::Rlimit {
                current: Some(0),
                maximum: Some(0),
            },
        )
        .unwrap();
        run(if mode == "quiescent" {
            Fault::Quiescent
        } else if mode == "panic" {
            Fault::Panic
        } else {
            assert_eq!(mode, "terminal");
            Fault::Terminal
        });
        panic!("unknown staging returned after releasing owners");
    }
    for mode in ["quiescent", "terminal", "panic"] {
        crate::context::generated_scope::tests::unpublished::abort_child(
            TEST,
            CHILD,
            mode,
            "GRAPH_HOST_STAGING_UNKNOWN_RETAINED",
        );
    }
}
