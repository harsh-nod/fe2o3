use super::*;
use crate::authorized_execution::tests::{source_authority, source_projection};
use crate::generated_source::RuntimeGfx942GeneratedSourceMutV1;
use std::cell::RefCell;
use std::rc::Rc;

#[cfg(feature = "hardware-qualification")]
mod native;

fn shells() -> (KfdRuntimeBackendV1, GeneratedShellPlanV1) {
    let mut backend = KfdRuntimeBackendV1::mock();
    let (binding, logical) = crate::RuntimeContextV1::generated_shell_test_binding_v1(&mut backend);
    let (hsaco, projection) = source_projection();
    let authority = source_authority(&projection);
    let roster = GeneratedHostRosterV1::from_projection(&projection).unwrap();
    let mut storage = projection.into_generated_storage_v1();
    let mut source = RuntimeGfx942GeneratedSourceMutV1::new(&mut storage, &hsaco, &authority);
    let plan = backend
        .prepare_generated_shells_v1(binding, &roster, &logical)
        .unwrap();
    backend.commit_generated_shells_v1(plan, &mut source, &roster);
    (backend, plan)
}

fn native_state(phase: PhaseV1, lane: usize) -> GeneratedNativeAdoptionV1 {
    // No native handle is fabricated by these metadata-only tests.
    GeneratedNativeAdoptionV1 {
        phase,
        lane,
        native_lane: None,
        data: Vec::new(),
        returned: ReturnedDataV1::empty(),
    }
}

struct Item(usize, Rc<RefCell<Vec<usize>>>);
impl Drop for Item {
    fn drop(&mut self) {
        self.1.borrow_mut().push(self.0);
    }
}

#[test]
fn generated_returned_data_retains_exact_suffix_and_lower_handoff_on_each_failure() {
    for panic in [false, true] {
        for failed in 0..3 {
            let drops = Rc::new(RefCell::new(Vec::new()));
            let mut root = ReturnedDataV1::empty();
            root.install((0..3).map(|id| Item(id, drops.clone())).collect());
            let mut lower = None;
            let result = catch_unwind(AssertUnwindSafe(|| {
                root.release(|item| {
                    if item.0 == failed {
                        lower = Some(item);
                        if panic {
                            std::panic::panic_any(failed);
                        }
                        return Err(failed);
                    }
                    drop(item);
                    Ok(())
                })
            }));
            if panic {
                assert_eq!(*result.unwrap_err().downcast::<usize>().unwrap(), failed);
            } else {
                assert_eq!(result.unwrap(), Err(failed));
            }
            assert_eq!(root.completed, failed);
            assert_eq!(root.handed_to_lower, Some(failed));
            assert_eq!(lower.as_ref().unwrap().0, failed);
            assert_eq!(
                root.remaining
                    .as_ref()
                    .unwrap()
                    .as_slice()
                    .iter()
                    .map(|item| item.0)
                    .collect::<Vec<_>>(),
                ((failed + 1)..3).collect::<Vec<_>>()
            );
            assert_eq!(&*drops.borrow(), &(0..failed).collect::<Vec<_>>());
            assert!(
                catch_unwind(AssertUnwindSafe(
                    || root.release::<()>(|_| panic!("must not retry"))
                ))
                .is_err()
            );
            drop(lower);
            drop(root);
            assert_eq!(&*drops.borrow(), &[0, 1, 2]);
        }
    }
}

#[test]
fn generated_returned_data_success_and_callback_drop_panic_preserve_progress() {
    struct PanicDrop;
    impl Drop for PanicDrop {
        fn drop(&mut self) {
            panic!("release callback drop");
        }
    }
    let mut root = ReturnedDataV1::empty();
    root.install(vec![0, 1, 2]);
    let mut released = Vec::new();
    root.release::<()>(|item| {
        released.push(item);
        Ok(())
    })
    .unwrap();
    assert_eq!(released, [0, 1, 2]);
    assert_eq!(root.completed, 3);
    assert_eq!(root.handed_to_lower, None);
    let bomb = PanicDrop;
    let mut root = ReturnedDataV1::empty();
    root.install(vec![0, 1, 2]);
    let result = catch_unwind(AssertUnwindSafe(|| {
        root.release::<()>(move |_| {
            let _ = &bomb;
            Ok(())
        })
    }));
    assert!(result.is_err());
    assert_eq!(root.completed, 3);
    assert_eq!(root.remaining.as_ref().unwrap().len(), 0);
    // Empty DATA is not a phase transition; only the enclosing successful close is.
    let mut native = native_state(PhaseV1::Retiring, 0);
    native.returned.install(Vec::new());
    assert!(!native.is_retired());
}

#[test]
fn generated_native_phases_disable_shell_disposal_even_before_packet_transfer() {
    for phase in [
        PhaseV1::Entering,
        PhaseV1::Adopted,
        PhaseV1::Retiring,
        PhaseV1::Retired,
    ] {
        let (mut backend, plan) = shells();
        backend.generated_shells.get_mut(&plan.key).unwrap().native = Some(native_state(phase, 0));
        assert!(backend.validate_generated_shell_records_v1(&plan));
        assert!(!backend.validate_generated_shell_disposal_v1(&plan));
        assert!(backend.has_live_generated_native_v1());
        assert!(backend.retire_generated_data_v1(&plan).is_err());
        assert!(backend.generated_shells[&plan.key].control.is_some());
        assert_eq!(backend.allocations.len(), 3);
        backend.generated_shells.get_mut(&plan.key).unwrap().native = None;
        backend.dispose_generated_shells_v1(&plan);
    }
}

#[test]
fn generated_retired_disposal_requires_complete_returned_roster_and_no_control() {
    let (mut backend, plan) = shells();
    let mut native = native_state(PhaseV1::Retired, 0);
    native.returned.install(Vec::new());
    native.returned.completed = plan.count - 1;
    backend.generated_shells.get_mut(&plan.key).unwrap().native = Some(native);
    backend.generated_shells.get_mut(&plan.key).unwrap().control = None;
    assert!(!backend.validate_generated_shell_disposal_v1(&plan));
    backend
        .generated_shells
        .get_mut(&plan.key)
        .unwrap()
        .native
        .as_mut()
        .unwrap()
        .returned
        .completed = plan.count;
    assert!(!backend.has_live_generated_native_v1());
    assert!(backend.validate_generated_shell_disposal_v1(&plan));
    backend.dispose_generated_shells_v1(&plan);
    assert!(backend.allocations.is_empty());
}

#[test]
fn generated_unpublished_leases_exclude_other_streams_and_all_native_reentry() {
    let (mut backend, plan) = shells();
    let other = backend.create_stream_v1(7).unwrap();
    for lane in [0, 1] {
        backend.generated_shells.get_mut(&plan.key).unwrap().native =
            Some(native_state(PhaseV1::Entering, lane));
        backend.lease_compute_lane_v1(plan.binding.backend_stream, lane);
        assert_eq!(backend.free_compute_lane_v1(), Some(1 - lane));
        assert!(!backend.generated_empty_prefix_v1(plan.binding.backend_stream));
        assert!(backend.require_no_generated_stream_v1(other).is_ok());
        assert!(
            backend
                .destroy_stream_v1(plan.binding.backend_stream)
                .is_err()
        );
        assert!(backend.shutdown_native_v1().is_err());
        assert_eq!(
            backend
                .stream_compute_lanes
                .get(&plan.binding.backend_stream),
            Some(&lane)
        );
        assert!(!backend.generated_lease_matches_v1(&plan));
        backend.release_compute_lane_lease_v1(plan.binding.backend_stream, lane);
    }
    backend.generated_shells.get_mut(&plan.key).unwrap().native = None;
    backend.dispose_generated_shells_v1(&plan);
    assert!(backend.generated_empty_prefix_v1(plan.binding.backend_stream));
    backend
        .destroy_stream_v1(plan.binding.backend_stream)
        .unwrap();
    backend.destroy_stream_v1(other).unwrap();
}

#[test]
fn generated_native_failure_finisher_preserves_classified_error_and_first_panic() {
    for kind in 0..3 {
        let (mut backend, plan) = shells();
        backend.generated_shells.get_mut(&plan.key).unwrap().native =
            Some(native_state(PhaseV1::Entering, 0));
        let error =
            KfdRuntimeBackendErrorV1::new(KfdRuntimeBackendErrorKindV1::Busy, "original failure");
        let error = match kind {
            0 => RuntimeBackendFailureV1::Rejected(error),
            1 => RuntimeBackendFailureV1::Quiescent(error),
            _ => RuntimeBackendFailureV1::Terminal(error),
        };
        assert!(
            matches!(backend.finish_generated_native_call_v1(Ok(Err(error))), Err(RuntimeBackendFailureV1::Terminal(error)) if error.to_string().contains("original failure"))
        );
        assert!(backend.terminal);
        assert!(backend.generated_shells[&plan.key].control.is_some());
        assert!(backend.retire_generated_data_v1(&plan).is_err());
        core::mem::forget(backend);
    }
    let (mut backend, plan) = shells();
    let result = catch_unwind(AssertUnwindSafe(|| {
        backend.finish_generated_native_call_v1(Err(Box::new(123_u32)))
    }));
    assert_eq!(*result.unwrap_err().downcast::<u32>().unwrap(), 123);
    assert!(backend.terminal);
    assert!(backend.generated_shells[&plan.key].control.is_some());
    core::mem::forget(backend);
}

#[test]
fn generated_native_diagnostic_panics_only_after_terminal_state_is_installed() {
    struct BadDisplay;
    impl fmt::Display for BadDisplay {
        fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
            panic!("diagnostic panic")
        }
    }
    let (mut backend, plan) = shells();
    let result = catch_unwind(AssertUnwindSafe(|| {
        backend.generated_native_error_v1("injected", BadDisplay)
    }));
    assert!(result.is_err());
    assert!(backend.terminal);
    assert!(backend.generated_shells[&plan.key].control.is_some());
    assert_eq!(backend.allocations.len(), 3);
    core::mem::forget(backend);
}

#[test]
fn generated_stream_guards_reject_without_generated_buffer_arguments() {
    let (mut backend, plan) = shells();
    let stream = plan.binding.backend_stream;
    let next = backend.next_handle;
    let submit = backend.submit_v1(BackendLaunchV1 {
        stream,
        kernel: 0,
        explicit_kernarg: &[],
        bindings: &[],
        dependencies: &[],
        geometry: crate::RuntimeLaunchGeometryV1 {
            grid: [1; 3],
            workgroup: [1; 3],
            dynamic_shared_bytes: 0,
        },
        semantic_launch: BackendSemanticLaunchV1::Ordinary,
    });
    let copy = backend.copy_async_v1(
        stream,
        BackendMemoryRegionV1 {
            allocation: 0,
            access: RuntimeAccessV1::Read,
            byte_offset: 0,
            byte_len: 4,
        },
        BackendMemoryRegionV1 {
            allocation: 0,
            access: RuntimeAccessV1::Write,
            byte_offset: 0,
            byte_len: 4,
        },
        &[],
    );
    for result in [submit, copy] {
        assert!(
            matches!(result, Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Busy)
        );
    }
    assert_eq!(backend.next_handle, next);
    assert!(backend.pending_compute.is_empty());
    assert!(backend.active_sdma.is_empty());
    assert!(backend.generated_shells[&plan.key].control.is_some());
    backend.dispose_generated_shells_v1(&plan);
}

#[cfg(unix)]
#[test]
fn generated_native_entered_drop_fails_closed_before_owner_destruction() {
    use std::os::unix::process::ExitStatusExt;
    const CHILD: &str = "FE2O3_GENERATED_ENTERED_DROP_CHILD";
    if std::env::var_os(CHILD).is_some() {
        let (mut backend, plan) = shells();
        backend.generated_shells.get_mut(&plan.key).unwrap().native =
            Some(native_state(PhaseV1::Entering, 0));
        drop(backend);
        panic!("entered generated custody must abort on Drop");
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "kfd_backend::generated_adoption::tests::generated_native_entered_drop_fails_closed_before_owner_destruction", "--nocapture"])
        .env(CHILD, "1")
        .output().unwrap();
    assert_eq!(
        output.status.signal(),
        Some(6),
        "child stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
