//! Runtime settlement of the genuine retained-primary teardown owner.

use super::*;
use fe2o3_kfd::{
    ComputeAqlQueueDestroyedV1, ComputeAqlQueueSessionErrorV1, PrimaryQueueReleaseCustodyV1,
};
use std::cell::RefCell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PrimaryEnvelopeFaultV1 {
    Error,
    Panic,
}

#[derive(Debug)]
struct PrimaryEnvelopePanicV1 {
    drops: Arc<AtomicUsize>,
}

impl Drop for PrimaryEnvelopePanicV1 {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

thread_local! {
    static FAULT: Cell<Option<PrimaryEnvelopeFaultV1>> = const { Cell::new(None) };
    static PANIC_PAYLOAD: RefCell<Option<Box<PrimaryEnvelopePanicV1>>> = const { RefCell::new(None) };
    static OWNER_POINTER: Cell<Option<usize>> = const { Cell::new(None) };
    static FAULT_CALLS: Cell<usize> = const { Cell::new(0) };
    static TOTAL_CALLS: Cell<Option<usize>> = const { Cell::new(None) };
}

pub(in crate::kfd_backend) fn release_or_fault(
    owner: &mut PrimaryQueueReleaseCustodyV1,
) -> Result<ComputeAqlQueueDestroyedV1, ComputeAqlQueueSessionErrorV1> {
    TOTAL_CALLS.with(|calls| {
        if let Some(count) = calls.get() {
            calls.set(Some(count.saturating_add(1)));
        }
    });
    let Some(fault) = FAULT.with(Cell::take) else {
        return owner.release_in_place();
    };
    FAULT_CALLS.with(|calls| calls.set(calls.get().saturating_add(1)));
    OWNER_POINTER.with(|pointer| {
        assert!(
            pointer
                .replace(Some(core::ptr::from_mut(owner) as usize))
                .is_none(),
            "primary envelope fault must observe one original owner"
        );
    });
    match fault {
        PrimaryEnvelopeFaultV1::Error => Err(ComputeAqlQueueSessionErrorV1::Contract(
            "deterministic runtime primary-envelope fault",
        )),
        PrimaryEnvelopeFaultV1::Panic => {
            let payload = PANIC_PAYLOAD
                .with(|slot| slot.borrow_mut().take())
                .expect("armed primary-envelope panic retains its original payload");
            std::panic::resume_unwind(payload)
        }
    }
}

#[cfg(feature = "hardware-qualification")]
fn arm_fault(fault: PrimaryEnvelopeFaultV1, payload: Option<Box<PrimaryEnvelopePanicV1>>) {
    assert_eq!(FAULT_CALLS.with(Cell::get), 0);
    assert!(TOTAL_CALLS.with(|calls| calls.replace(Some(0))).is_none());
    assert!(OWNER_POINTER.with(Cell::get).is_none());
    assert!(
        super::SELECTION
            .with(|slot| slot.replace(Some(false)))
            .is_none()
    );
    PANIC_PAYLOAD.with(|slot| {
        assert!(std::mem::replace(&mut *slot.borrow_mut(), payload).is_none());
    });
    assert!(FAULT.with(|slot| slot.replace(Some(fault))).is_none());
}

#[cfg(feature = "hardware-qualification")]
fn native_primary_backend() -> KfdRuntimeBackendV1 {
    use crate::qualification_gfx942_vecadd_v1::{
        GFX942_VECADD_QUALIFICATION_BUFFER_ALIGNMENT_V1 as ALIGNMENT,
        GFX942_VECADD_QUALIFICATION_BUFFER_BYTES_V1 as BYTES, Gfx942VecaddQualificationArgumentsV1,
        admit_gfx942_vecadd_qualification_v1,
    };
    use crate::{RuntimeContextV1, RuntimePollV1};

    let device = native_device();
    let admitted = admit_gfx942_vecadd_qualification_v1().unwrap();
    let buffers = admitted.host_buffers().unwrap();
    let mut backend = KfdRuntimeBackendV1::open_gfx942_vecadd_qualification_v1(device).unwrap();
    backend
        .configure_host_visible_backing_budget_v1(
            Gfx942HostVisibleBackingBudgetV1::new(64 * 1024 * 1024, 128).unwrap(),
        )
        .unwrap();
    backend
        .configure_device_backing_budget_v1(
            Gfx942DeviceBackingBudgetV1::new(1024 * 1024, 8).unwrap(),
        )
        .unwrap();
    backend
        .enable_profiler_v1(KfdRuntimeProfilerConfigV1::new([0x8b; 32], 128).unwrap())
        .unwrap();

    let mut context = RuntimeContextV1::open(backend).unwrap();
    let device_id = context.devices()[0].id();
    let module = context.load_module(device_id, admitted.hsaco()).unwrap();
    let kernel = context
        .resolve_kernel::<Gfx942VecaddQualificationArgumentsV1>(module, admitted.kernel_name())
        .unwrap();
    let stream = context.create_stream(device_id).unwrap();
    let allocations = [buffers.left(), buffers.right(), buffers.output()].map(|bytes| {
        let allocation = context
            .allocate(
                device_id,
                RuntimeMemoryKindV1::HostVisible,
                BYTES as u64,
                ALIGNMENT,
            )
            .unwrap();
        context.write_allocation(allocation, 0, bytes).unwrap();
        allocation
    });
    let arguments =
        Gfx942VecaddQualificationArgumentsV1::new(allocations[0], allocations[1], allocations[2])
            .unwrap();
    let mut submission = context
        .launch(stream, &kernel, &arguments, admitted.geometry(), &[])
        .unwrap();
    context.flush_stream(stream).unwrap();
    assert_eq!(
        context
            .wait(&mut submission, Duration::from_secs(10))
            .unwrap(),
        RuntimePollV1::Succeeded
    );
    let mut output = vec![0; BYTES];
    context
        .read_allocation(allocations[2], 0, &mut output)
        .unwrap();
    assert_eq!(output, buffers.expected_output());

    context.release_submission(submission).unwrap();
    for allocation in allocations.into_iter().rev() {
        context.release_allocation(allocation).unwrap();
    }
    context.destroy_stream(stream).unwrap();
    context.unload_module(module).unwrap();
    let backend = context.shutdown().unwrap();
    assert_eq!(
        backend
            .native_compute_lanes
            .iter()
            .filter_map(|lane| *lane)
            .map(|lane| lane.ordinal())
            .collect::<Vec<_>>(),
        [0]
    );
    // Public shutdown settles dispatch and pool custody before selecting primary release.
    assert!(backend.queue.is_some());
    let events = backend
        .profiler
        .as_ref()
        .unwrap()
        .recorded_events_for_test_v1();
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(
                event.event,
                KfdRuntimeProfileEventKindV1::NativeQueueCreated { .. }
            ))
            .count(),
        1
    );
    assert!(!events.iter().any(|event| matches!(
        event.event,
        KfdRuntimeProfileEventKindV1::NativeQueueDestroyed { .. }
    )));
    backend
}

#[cfg(feature = "hardware-qualification")]
fn assert_retained_terminal_primary(backend: &mut KfdRuntimeBackendV1, expected_pointer: usize) {
    assert_eq!(super::SELECTION.with(Cell::get), Some(true));
    assert!(backend.terminal && !backend.queue_retired && backend.queue.is_none());
    let retained = backend
        .primary_teardown
        .as_ref()
        .expect("terminal runtime retains the original primary Box");
    assert_eq!(
        core::ptr::from_ref(retained.as_ref()) as usize,
        expected_pointer
    );
    assert_eq!(FAULT_CALLS.with(Cell::get), 1);
    assert_eq!(TOTAL_CALLS.with(Cell::get), Some(1));

    let observation = super::PRIMARY_HOST_USAGE
        .with(|slot| slot.take())
        .expect("armed host-account observation");
    assert_eq!(observation.observations, [1, 0]);
    assert!(observation.completed.is_none());
    let before = observation
        .before
        .expect("installed primary root exposes its original host account");
    let retained_usage = backend
        .host_visible_backing_usage_v1()
        .expect("terminal primary root keeps host-account visibility");
    assert_eq!(retained_usage.budget, before.budget);
    assert_eq!(
        (
            retained_usage.used_backing_bytes,
            retained_usage.used_allocation_records,
            retained_usage.reserved_records,
            retained_usage.retained_records,
            retained_usage.quarantined_records,
        ),
        (
            before.used_backing_bytes,
            before.used_allocation_records,
            before.reserved_records,
            before.retained_records,
            before.quarantined_records,
        )
    );
    assert!(retained_usage.used_backing_bytes > 0);
    let device_before = observation
        .device_before
        .expect("configured device account remains visible through the primary root");
    assert_eq!(
        backend.device_backing_usage_v1(),
        Some(device_before),
        "terminal primary root preserves the exact device account"
    );
    assert!(!device_before.poisoned);

    let profiler = backend.profiler.as_ref().unwrap();
    let events = profiler.recorded_events_for_test_v1();
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(
                event.event,
                KfdRuntimeProfileEventKindV1::NativeQueueCreated { .. }
            ))
            .count(),
        1
    );
    assert!(!events.iter().any(|event| matches!(
        event.event,
        KfdRuntimeProfileEventKindV1::NativeQueueDestroyed { .. }
    )));
    let events_before_retry = events.to_vec();
    let dropped_events_before_retry = profiler.dropped_events_for_test_v1();
    let host_before_retry = backend.host_visible_backing_usage_v1();
    let device_before_retry = backend.device_backing_usage_v1();

    super::SELECTION.with(|slot| slot.set(Some(false)));
    assert!(matches!(
        backend.shutdown_owned_v1(),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert_eq!(super::SELECTION.with(Cell::take), Some(false));
    assert_eq!(FAULT_CALLS.with(Cell::get), 1);
    assert_eq!(TOTAL_CALLS.with(Cell::get), Some(1));
    assert_eq!(backend.host_visible_backing_usage_v1(), host_before_retry);
    assert_eq!(backend.device_backing_usage_v1(), device_before_retry);
    let profiler = backend.profiler.as_ref().unwrap();
    assert_eq!(
        profiler.recorded_events_for_test_v1(),
        events_before_retry.as_slice()
    );
    assert_eq!(
        profiler.dropped_events_for_test_v1(),
        dropped_events_before_retry
    );
    assert_eq!(
        core::ptr::from_ref(
            backend
                .primary_teardown
                .as_ref()
                .expect("terminal reentry preserves the root")
                .as_ref()
        ) as usize,
        expected_pointer
    );
}

#[cfg(feature = "hardware-qualification")]
fn enter_isolated_child(test: &str, mode: &str) -> bool {
    const CHILD: &str = "FE2O3_TEST_PRIMARY_RELEASE_ENVELOPE_CHILD";
    match std::env::var(CHILD) {
        Ok(actual) => {
            assert_eq!(actual, mode);
            true
        }
        Err(std::env::VarError::NotPresent) => {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    test,
                    "--ignored",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .env(CHILD, mode)
                .output()
                .unwrap();
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            print!("{stdout}");
            eprint!("{stderr}");
            assert!(
                output.status.success(),
                "isolated primary-envelope child failed\nstdout:\n{}\nstderr:\n{}",
                stdout,
                stderr
            );
            assert!(
                stdout.contains("running 1 test"),
                "isolated primary-envelope child did not execute exactly one harness test"
            );
            let marker = format!("native_runtime_primary_release_envelope={mode} ");
            assert_eq!(
                stdout.matches(&marker).count(),
                1,
                "isolated primary-envelope child did not emit its exact case marker"
            );
            false
        }
        Err(error) => panic!("primary-envelope child selector: {error}"),
    }
}

#[cfg(feature = "hardware-qualification")]
#[test]
#[ignore = "requires a freshly guarded MI300X, FE2O3_TEST_NATIVE_UNIQUE_ID, and isolated terminal-custody child process"]
fn native_runtime_primary_release_error_retains_installed_root() {
    const TEST: &str = "kfd_backend::retained_release_tests::primary_envelope::native_runtime_primary_release_error_retains_installed_root";
    if !enter_isolated_child(TEST, "error") {
        return;
    }

    let mut backend = std::mem::ManuallyDrop::new(native_primary_backend());
    super::PRIMARY_HOST_USAGE.with(|slot| slot.set(Some(PrimaryHostUsage::default())));
    arm_fault(PrimaryEnvelopeFaultV1::Error, None);
    let result = backend.shutdown_owned_v1();
    assert!(matches!(
        result,
        Err(RuntimeBackendFailureV1::Terminal(error))
            if error.detail().contains("deterministic runtime primary-envelope fault")
    ));
    let pointer = OWNER_POINTER
        .with(Cell::get)
        .expect("fault observed the installed owner");
    assert_retained_terminal_primary(&mut backend, pointer);
    println!(
        "native_runtime_primary_release_envelope=error retained_original_root=true terminal=true destroyed_observation=false retry_entered=false disposition=process_exit"
    );
}

#[cfg(feature = "hardware-qualification")]
#[test]
#[ignore = "requires a freshly guarded MI300X, FE2O3_TEST_NATIVE_UNIQUE_ID, and isolated terminal-custody child process"]
fn native_runtime_primary_release_panic_retains_installed_root_and_payload() {
    const TEST: &str = "kfd_backend::retained_release_tests::primary_envelope::native_runtime_primary_release_panic_retains_installed_root_and_payload";
    if !enter_isolated_child(TEST, "panic") {
        return;
    }

    let mut backend = std::mem::ManuallyDrop::new(native_primary_backend());
    super::PRIMARY_HOST_USAGE.with(|slot| slot.set(Some(PrimaryHostUsage::default())));
    let drops = Arc::new(AtomicUsize::new(0));
    let payload = Box::new(PrimaryEnvelopePanicV1 {
        drops: Arc::clone(&drops),
    });
    let payload_pointer = core::ptr::from_ref(payload.as_ref()) as usize;
    arm_fault(PrimaryEnvelopeFaultV1::Panic, Some(payload));
    let result = catch_unwind(AssertUnwindSafe(|| backend.shutdown_owned_v1()));
    let caught = result
        .expect_err("armed primary-envelope panic must propagate")
        .downcast::<PrimaryEnvelopePanicV1>()
        .expect("original primary-envelope panic payload");
    assert_eq!(
        core::ptr::from_ref(caught.as_ref()) as usize,
        payload_pointer
    );
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    drop(caught);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    let pointer = OWNER_POINTER
        .with(Cell::get)
        .expect("panic observed the installed owner");
    assert_retained_terminal_primary(&mut backend, pointer);
    println!(
        "native_runtime_primary_release_envelope=panic retained_original_root=true terminal=true destroyed_observation=false retry_entered=false original_payload=true disposition=process_exit"
    );
}
