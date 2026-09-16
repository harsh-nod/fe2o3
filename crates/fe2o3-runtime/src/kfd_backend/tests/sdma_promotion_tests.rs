use super::sdma_host_write_tests::{
    assert_terminal_retries_inert, discard_scripted_fixture, fixture, host_observation,
    retained_host, snapshot,
};
use super::*;
use fe2o3_kfd::ComputeAqlQueueSessionErrorV1;
use std::mem::ManuallyDrop;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn allocate_step() -> ScriptedSdmaStepV1 {
    ScriptedSdmaStepV1::Allocate {
        kind: ScriptedBufferKindV1::Device,
        byte_len: 8,
    }
}

#[test]
fn sdma_promotion_diagnostic_panic_roots_retryable_and_terminal_owners() {
    for mode in [
        ScriptedFailureModeV1::Retryable,
        ScriptedFailureModeV1::ProcessTeardown,
    ] {
        let (backend, _, host, device) = fixture(
            16,
            [
                allocate_step(),
                ScriptedSdmaStepV1::Promote(mode),
                ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
            ],
        );
        let mut backend = ManuallyDrop::new(backend);
        let before = snapshot(&backend, host, device);
        let buffer = backend
            .directional_sdma_ops_v1()
            .allocate_device_buffer(8, 8)
            .unwrap();
        let observation = host_observation(&buffer);
        assert_eq!(observation, (3, vec![0; 8], None));
        let failure = backend
            .directional_sdma_ops_v1()
            .promote(buffer)
            .unwrap_err();
        let original = Box::new(173_u64);
        let address = &*original as *const u64;
        let result = catch_unwind(AssertUnwindSafe(|| {
            backend
                .settle_sdma_promotion_failure_v1(failure, |_| std::panic::resume_unwind(original))
        }));
        let recovered = result.unwrap_err().downcast::<u64>().unwrap();
        assert_eq!(&*recovered as *const u64, address);
        assert_eq!(snapshot(&backend, host, device), before);
        assert_eq!(host_observation(retained_host(&backend)), observation);
        let driver = backend.scripted_sdma.as_ref().unwrap();
        assert_eq!(driver.remaining_steps(), 1);
        assert_eq!(driver.live_owner_count(), 3);
        assert_eq!(driver.unexpected_drops(), 0);
        assert_terminal_retries_inert(&mut backend, host, device);
        discard_scripted_fixture(backend);
    }
}

#[test]
fn sdma_promotion_lower_panic_seals_backend_without_losing_driver_root() {
    let (backend, _, host, device) = fixture(
        16,
        [
            allocate_step(),
            ScriptedSdmaStepV1::PromotePanic,
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ],
    );
    let mut backend = ManuallyDrop::new(backend);
    let mut expected = snapshot(&backend, host, device);
    expected.next_handle += 1;
    let result = catch_unwind(AssertUnwindSafe(|| {
        backend.allocate_v1(7, RuntimeMemoryKindV1::DeviceLocal, 8, 8)
    }));
    assert_eq!(
        result.unwrap_err().downcast_ref::<&str>(),
        Some(&"scripted SDMA promotion panic")
    );
    assert_eq!(snapshot(&backend, host, device), expected);
    assert!(backend.terminal_sdma_custody.is_none());
    assert_terminal_retries_inert(&mut backend, host, device);
    let driver = backend.scripted_sdma.as_ref().unwrap();
    assert_eq!(driver.remaining_steps(), 1);
    assert_eq!(driver.live_owner_count(), 3);
    assert_eq!(driver.unexpected_drops(), 0);
    assert_eq!(
        driver.promotion_custody().unwrap().observation(),
        (3, [0; 8].as_slice(), None)
    );
    discard_scripted_fixture(backend);
}

#[test]
fn sdma_promotion_driver_selection_panic_keeps_untransferred_runtime_root() {
    let mut backend = ManuallyDrop::new(KfdRuntimeBackendV1::mock());
    let driver = ScriptedSdmaDriverV1::new([]);
    let buffer = driver.test_host_owner(8);
    let before = host_observation(&buffer);
    let result = catch_unwind(AssertUnwindSafe(|| backend.promote_sdma_buffer_v1(buffer)));
    assert!(result.is_err() && backend.terminal);
    assert_eq!(host_observation(retained_host(&backend)), before);
    assert_eq!(driver.live_owner_count(), 1);
    assert_eq!(driver.unexpected_drops(), 0);
    // Install the observed fixture driver only to enable its test-only Drop disarm.
    backend.scripted_sdma = Some(driver);
    discard_scripted_fixture(backend);
}

#[test]
fn sdma_promotion_ordinary_rejection_keeps_neighbors_and_exact_error() {
    for terminal in [false, true] {
        let mode = if terminal {
            ScriptedFailureModeV1::ProcessTeardown
        } else {
            ScriptedFailureModeV1::Retryable
        };
        let (backend, _, host, device) = fixture(
            16,
            [
                allocate_step(),
                ScriptedSdmaStepV1::Promote(mode),
                ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
            ],
        );
        let mut backend = ManuallyDrop::new(backend);
        let mut expected = snapshot(&backend, host, device);
        expected.next_handle += 1;
        let failure = backend
            .allocate_v1(7, RuntimeMemoryKindV1::DeviceLocal, 8, 8)
            .unwrap_err();
        match failure {
            RuntimeBackendFailureV1::Terminal(error) if terminal => {
                assert_eq!(
                    error.detail(),
                    "KFD persistent device promotion: scripted promotion teardown"
                );
                assert_eq!(error.kind(), KfdRuntimeBackendErrorKindV1::Terminal);
            }
            RuntimeBackendFailureV1::Quiescent(error) if !terminal => {
                assert_eq!(
                    error.detail(),
                    "KFD persistent device promotion: scripted promotion retryable"
                );
                assert_eq!(error.kind(), KfdRuntimeBackendErrorKindV1::Native);
            }
            _ => panic!("wrong failure class"),
        }
        assert_eq!(snapshot(&backend, host, device), expected);
        assert_eq!(backend.terminal, terminal);
        let driver = backend.scripted_sdma.as_ref().unwrap();
        assert_eq!(driver.remaining_steps(), usize::from(terminal));
        assert_eq!(driver.live_owner_count(), 2 + usize::from(terminal));
        assert_eq!(driver.unexpected_drops(), 0);
        if terminal {
            assert_eq!(
                host_observation(retained_host(&backend)),
                (3, vec![0; 8], None)
            );
            assert_terminal_retries_inert(&mut backend, host, device);
        } else {
            assert!(backend.terminal_sdma_custody.is_none());
        }
        discard_scripted_fixture(backend);
    }
}

#[test]
fn sdma_promotion_public_context_handles_configured_and_unconfigured_panic() {
    for configured in [false, true] {
        let mut backend = KfdRuntimeBackendV1::mock();
        backend.native_available = true;
        backend.scripted_sdma = Some(ScriptedSdmaDriverV1::new([
            allocate_step(),
            ScriptedSdmaStepV1::PromotePanic,
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ]));
        let mut context = ManuallyDrop::new(crate::RuntimeContextV1::open(backend).unwrap());
        let device = context.devices()[0].id();
        if configured {
            context
                .configure_allocation_admission_v1(device, 8, 1)
                .unwrap();
        }
        let result = catch_unwind(AssertUnwindSafe(|| {
            context.allocate(device, RuntimeMemoryKindV1::DeviceLocal, 8, 8)
        }));
        assert_eq!(
            result.unwrap_err().downcast_ref::<&str>(),
            Some(&"scripted SDMA promotion panic")
        );
        assert_eq!(context.is_terminal(), configured);
        if configured {
            let usage = context
                .allocation_admission_usage_v1(device)
                .unwrap()
                .unwrap();
            assert_eq!(
                usage
                    .used
                    .get(crate::RuntimeResourceKindV1::RequestedAllocationBytes),
                8
            );
            assert_eq!(
                usage
                    .used
                    .get(crate::RuntimeResourceKindV1::AllocationRecords),
                1
            );
            assert_eq!(usage.quarantined_records, 1);
        }
        assert!(context.backend_mut_for_test_v1().terminal);
        assert!(
            context
                .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 8, 8)
                .is_err()
        );
        assert!(context.is_terminal());
        if configured {
            let usage = context
                .allocation_admission_usage_v1(device)
                .unwrap()
                .unwrap();
            assert_eq!(
                usage
                    .used
                    .get(crate::RuntimeResourceKindV1::RequestedAllocationBytes),
                8
            );
            assert_eq!(
                usage
                    .used
                    .get(crate::RuntimeResourceKindV1::AllocationRecords),
                1
            );
            assert_eq!(usage.quarantined_records, 1);
        }
        let backend = context.backend_mut_for_test_v1();
        assert!(backend.allocations.is_empty() && backend.staged_context_bytes == 0);
        let driver = backend.scripted_sdma.as_ref().unwrap();
        assert_eq!(driver.remaining_steps(), 1);
        assert_eq!(driver.live_owner_count(), 1);
        assert_eq!(driver.unexpected_drops(), 0);
        assert!(driver.promotion_custody().is_some());
        disarm_scripted_drop_after_inspection_v1(backend);
        drop(ManuallyDrop::into_inner(context));
    }
}

#[test]
fn sdma_promotion_native_diagnostic_stays_typed_until_rooted_formatting() {
    use crate::kfd_backend::kfd_backend_sdma_seam::SdmaOwnerDiagnosticV1;
    let (backend, _, host, device) = fixture(16, [allocate_step()]);
    let mut backend = ManuallyDrop::new(backend);
    let before = snapshot(&backend, host, device);
    let buffer = backend
        .directional_sdma_ops_v1()
        .allocate_device_buffer(8, 8)
        .unwrap();
    let failure = SdmaTransitionFailureV1::Retryable {
        detail: SdmaOwnerDiagnosticV1::Native(ComputeAqlQueueSessionErrorV1::Contract(
            "typed promotion cause",
        )),
        custody: buffer,
    };
    let caught = catch_unwind(AssertUnwindSafe(|| {
        backend.settle_sdma_promotion_failure_v1(failure, |detail| {
            assert!(matches!(
                detail,
                SdmaOwnerDiagnosticV1::Native(ComputeAqlQueueSessionErrorV1::Contract(
                    "typed promotion cause"
                ))
            ));
            std::panic::panic_any("typed diagnostic panic")
        })
    }));
    assert_eq!(
        caught.unwrap_err().downcast_ref::<&str>(),
        Some(&"typed diagnostic panic")
    );
    assert_eq!(snapshot(&backend, host, device), before);
    assert_eq!(
        host_observation(retained_host(&backend)),
        (3, vec![0; 8], None)
    );
    assert_terminal_retries_inert(&mut backend, host, device);
    discard_scripted_fixture(backend);
}

#[test]
fn sdma_promotion_terminal_drop_aborts_with_runtime_or_lower_custody() {
    use std::os::unix::process::ExitStatusExt;
    const CHILD: &str = "FE2O3_TEST_RUNTIME_PROMOTION_DROP";
    const TEST: &str = "kfd_backend::tests::sdma_promotion_tests::sdma_promotion_terminal_drop_aborts_with_runtime_or_lower_custody";
    if let Some(mode) = std::env::var_os(CHILD) {
        rustix::process::setrlimit(
            rustix::process::Resource::Core,
            rustix::process::Rlimit {
                current: Some(0),
                maximum: Some(0),
            },
        )
        .unwrap();
        let mut backend = KfdRuntimeBackendV1::mock();
        backend.native_available = true;
        backend.scripted_sdma = Some(ScriptedSdmaDriverV1::new([
            allocate_step(),
            if mode == "lower" {
                ScriptedSdmaStepV1::PromotePanic
            } else {
                ScriptedSdmaStepV1::Promote(ScriptedFailureModeV1::ProcessTeardown)
            },
        ]));
        let result = catch_unwind(AssertUnwindSafe(|| {
            backend.allocate_v1(7, RuntimeMemoryKindV1::DeviceLocal, 8, 8)
        }));
        if mode == "lower" {
            assert!(result.is_err());
        } else {
            assert!(matches!(
                result.unwrap(),
                Err(RuntimeBackendFailureV1::Terminal(_))
            ));
        }
        assert!(backend.terminal);
        let driver = backend.scripted_sdma.as_ref().unwrap();
        assert_eq!(driver.live_owner_count(), 1);
        assert_eq!(driver.unexpected_drops(), 0);
        eprintln!("promotion retained; dropping terminal backend");
        drop(backend);
        panic!("terminal Drop returned");
    }
    for mode in ["runtime", "lower"] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env(CHILD, mode)
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.signal(), Some(6), "{stderr}");
        assert!(stderr.contains("promotion retained; dropping terminal backend"));
        assert!(!stderr.contains("terminal Drop returned"));
    }
}
