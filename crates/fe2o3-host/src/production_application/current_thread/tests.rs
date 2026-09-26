//! Real owner/command tests without executable, completion-receipt, or native authority.

use super::*;
use std::{cell::Cell, rc::Rc, time::Duration};

#[derive(Default)]
struct Trace {
    shutdowns: Cell<usize>,
    drops: Cell<usize>,
}

struct EmptyBackend {
    trace: Rc<Trace>,
    fail_shutdown: bool,
}

impl Drop for EmptyBackend {
    fn drop(&mut self) {
        self.trace.drops.set(self.trace.drops.get() + 1);
    }
}

macro_rules! unreachable_backend {
    ($($name:ident($($arg:ident: $ty:ty),*) -> $out:ty;)*) => {$ (
        fn $name(&mut self, $(_: $ty),*) -> Result<$out, RuntimeBackendFailureV1<Self::Error>> {
            panic!(concat!(stringify!($name), " cannot be reached by an empty test Context"));
        }
    )*};
}

impl RuntimeBackendV1 for EmptyBackend {
    type Error = std::io::Error;
    fn enumerate_devices_v1(
        &mut self,
    ) -> Result<Vec<BackendDeviceDescriptionV1>, RuntimeBackendFailureV1<Self::Error>> {
        Ok(vec![])
    }
    unreachable_backend! {
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

impl RuntimeFlushBackendV1 for EmptyBackend {
    unreachable_backend! { flush_stream_v1(stream: u64) -> (); }
}

impl RuntimeOwnedShutdownBackendV1 for EmptyBackend {
    fn shutdown_owned_v1(&mut self) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.trace.shutdowns.set(self.trace.shutdowns.get() + 1);
        if self.fail_shutdown {
            Err(RuntimeBackendFailureV1::Rejected(std::io::Error::other(
                "scripted native shutdown failure",
            )))
        } else {
            Ok(())
        }
    }
}

fn owner(
    fail_shutdown: bool,
) -> (
    RuntimeAsyncCurrentThreadOwnedEngineV1<EmptyBackend>,
    RuntimeAsyncProgressHandleV1<EmptyBackend>,
    Rc<Trace>,
) {
    let trace = Rc::new(Trace::default());
    let (engine, handle) = RuntimeAsyncCurrentThreadOwnedEngineV1::new_with_progress(
        || {
            RuntimeContextV1::open(EmptyBackend {
                trace: trace.clone(),
                fail_shutdown,
            })
        },
        RuntimeAsyncEngineConfigV1::default()
            .with_reply_capacity(2)
            .unwrap(),
        RuntimeAsyncProgressConfigV1::new(4, 1).unwrap(),
    )
    .unwrap();
    (engine, handle, trace)
}

#[test]
fn application_stage_driver_releases_replies_with_only_two_cells() {
    let (mut engine, handle, trace) = owner(false);
    // Hold one completed reply throughout, as a separate pending completion
    // would do, and reuse only the remaining cell across the finite stages.
    let held = handle.observer().enqueue_with_context(|_| ()).unwrap();
    for stage in [
        ProductionWorkerV3ApplicationStageV1::CreateStream,
        ProductionWorkerV3ApplicationStageV1::Prepare,
        ProductionWorkerV3ApplicationStageV1::Reserve,
        ProductionWorkerV3ApplicationStageV1::Activate,
        ProductionWorkerV3ApplicationStageV1::DestroyStream,
    ] {
        let command = handle
            .observer()
            .enqueue_with_context(|context| {
                assert!(context.devices().is_empty());
                19
            })
            .unwrap();
        assert_eq!(handle.observer().reply_cells_in_use(), 2);
        assert_eq!(
            drive::<_, _, ()>(
                &mut engine,
                command,
                stage,
                Instant::now() + Duration::from_secs(2)
            )
            .unwrap()
            .unwrap(),
            19
        );
        assert_eq!(handle.observer().reply_cells_in_use(), 1);
    }
    drop(held);
    assert_eq!(handle.observer().reply_cells_in_use(), 0);
    let drain = handle.begin_drain(16).unwrap();
    let drained = drive::<_, _, ()>(
        &mut engine,
        drain,
        ProductionWorkerV3ApplicationStageV1::Drain,
        Instant::now() + Duration::from_secs(2),
    )
    .unwrap()
    .unwrap();
    assert_eq!(drained.outcome, RuntimeAsyncDrainOutcomeV1::Quiescent);
    let shutdown = engine.shutdown();
    assert_eq!(
        shutdown.disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
    assert_eq!(trace.shutdowns.get(), 1);
    assert_eq!(trace.drops.get(), 1);
}

#[test]
fn application_expired_stage_stops_without_running_queued_command() {
    for fail_shutdown in [false, true] {
        let (mut engine, handle, trace) = owner(fail_shutdown);
        let command = handle
            .observer()
            .enqueue_with_context(|_| panic!("expired command must not run"))
            .unwrap();
        let error = drive::<_, _, ()>(
            &mut engine,
            command,
            ProductionWorkerV3ApplicationStageV1::Prepare,
            Instant::now(),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ProductionWorkerV3CurrentThreadErrorV1::Drive(
                ProductionWorkerV3ApplicationStageV1::Prepare,
                RuntimeAsyncDriveErrorV1::DeadlineExceeded
            )
        ));
        let shutdown = engine.shutdown();
        assert_eq!(trace.shutdowns.get(), 1);
        assert_eq!(trace.drops.get(), usize::from(!fail_shutdown));
        assert_eq!(shutdown.native_failure.is_some(), fail_shutdown);
        assert_eq!(
            shutdown.disposition,
            if fail_shutdown {
                RuntimeAsyncOwnedDispositionV1::RetainedUntilProcessExit
            } else {
                RuntimeAsyncOwnedDispositionV1::Released
            }
        );
    }
}

#[test]
fn application_command_panic_keeps_primary_error_and_quarantines_owner() {
    let (mut engine, handle, trace) = owner(false);
    let command = handle
        .observer()
        .enqueue_with_context(|_| panic!("scripted command panic"))
        .unwrap();
    let error = drive::<_, _, ()>(
        &mut engine,
        command,
        ProductionWorkerV3ApplicationStageV1::Activate,
        Instant::now() + Duration::from_secs(2),
    )
    .unwrap()
    .unwrap_err();
    assert_eq!(error, RuntimeAsyncEngineCallErrorV1::CommandPanicked);
    let shutdown = engine.shutdown();
    assert_eq!(
        shutdown.disposition,
        RuntimeAsyncOwnedDispositionV1::RetainedUntilProcessExit
    );
    assert_eq!(trace.shutdowns.get(), 0);
    assert_eq!(trace.drops.get(), 0);
}

#[test]
fn application_report_without_original_completion_is_never_success() {
    let report = ProductionWorkerV3CurrentThreadReportV1::<(), ()> {
        completed: None,
        error: None,
        drain: None,
        shutdown: None,
    };
    assert!(!report.is_success());
    let report = ProductionWorkerV3CurrentThreadReportV1::<(), ()>::failed(
        ProductionWorkerV3CurrentThreadErrorV1::InvalidBounds,
    );
    assert!(!report.is_success());
}

fn config() -> ProductionWorkerV3CurrentThreadConfigV1 {
    ProductionWorkerV3CurrentThreadConfigV1 {
        device_unique_id: 1,
        geometry: AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
        dynamic_group_segment_bytes: 0,
        timeout_milliseconds: 1000,
        argument_limits: GeneratedRuntimeArgumentLimitsV1::new(4096, 4096, 16),
        engine: RuntimeAsyncEngineConfigV1::default()
            .with_reply_capacity(2)
            .unwrap(),
        progress: RuntimeAsyncProgressConfigV1::new(4, 1).unwrap(),
        journal_allocation_capacity: 16,
        journal_writer_capacity: 16,
        drain_tick_budget: 16,
        deadline: Instant::now() + Duration::from_secs(2),
    }
}

#[test]
fn application_preflight_rejects_invalid_bounds_and_expired_deadline() {
    for mode in 0..8 {
        let mut config = config();
        match mode {
            0 => config.journal_allocation_capacity = 0,
            1 => config.journal_writer_capacity = 0,
            2 => config.journal_allocation_capacity = usize::MAX,
            3 => config.journal_writer_capacity = usize::MAX,
            4 => config.drain_tick_budget = 0,
            5 => config.drain_tick_budget = usize::MAX,
            6 => config.engine = config.engine.with_reply_capacity(1).unwrap(),
            _ => config.deadline = Instant::now(),
        }
        let error = config.preflight::<()>().unwrap_err();
        if mode == 7 {
            assert!(matches!(
                error,
                ProductionWorkerV3CurrentThreadErrorV1::Drive(
                    ProductionWorkerV3ApplicationStageV1::Initialize,
                    RuntimeAsyncDriveErrorV1::DeadlineExceeded
                )
            ));
        } else {
            assert!(matches!(
                error,
                ProductionWorkerV3CurrentThreadErrorV1::InvalidBounds
            ));
        }
    }
    config().preflight::<()>().unwrap();
}
