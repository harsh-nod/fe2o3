use super::*;

mod async_tests;
mod writer_tests;
use crate::{RuntimeResourceKindV1 as K, RuntimeResourceVectorV1};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct Diagnostic {
    drops: Arc<AtomicUsize>,
}

impl fmt::Debug for Diagnostic {
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        panic!("allocation settlement must not debug-format the diagnostic")
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        panic!("allocation settlement must not format the diagnostic")
    }
}

impl Error for Diagnostic {}

impl Drop for Diagnostic {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

// Keep MockBackend on the default SPI so legacy failure tests exercise that path.
#[derive(Default)]
struct AllocationOnlyBackend {
    inner: MockBackend,
    diagnostic: Option<Box<Diagnostic>>,
    panic_before_outcome: bool,
    attempts: usize,
    write_failure: MockMemoryFailure,
    write_diagnostic: Option<Box<Diagnostic>>,
    write_prefix: usize,
    write_calls: usize,
    release_failure: MockMemoryFailure,
    release_diagnostic: Option<Box<Diagnostic>>,
    release_calls: usize,
    failure_call: Option<DiagnosticCall>,
    call_failure: MockMemoryFailure,
    call_diagnostic: Option<Box<Diagnostic>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DiagnosticCall {
    Submit,
    Poll,
    Wait,
    Cancel,
    Drain,
    Flush,
    CreateStream,
    DestroyStream,
    RecordEvent,
    ReleaseEvent,
    ReleaseSubmission,
    LoadModule,
    UnloadModule,
    ResolveKernel,
    Read,
}

impl AllocationOnlyBackend {
    fn fail_call(
        &mut self,
        call: DiagnosticCall,
    ) -> Result<(), RuntimeBackendFailureV1<Box<Diagnostic>>> {
        if self.failure_call != Some(call) {
            return Ok(());
        }
        self.failure_call = None;
        diagnostic_failure(
            core::mem::take(&mut self.call_failure),
            &mut self.call_diagnostic,
        )
    }
}

fn diagnostic_failure(
    failure: MockMemoryFailure,
    diagnostic: &mut Option<Box<Diagnostic>>,
) -> Result<(), RuntimeBackendFailureV1<Box<Diagnostic>>> {
    if failure == MockMemoryFailure::None {
        return Ok(());
    }
    let error = diagnostic.take().expect("scripted diagnostic");
    match failure {
        MockMemoryFailure::Rejected => Err(RuntimeBackendFailureV1::Rejected(error)),
        MockMemoryFailure::Quiescent => Err(RuntimeBackendFailureV1::Quiescent(error)),
        MockMemoryFailure::Terminal => Err(RuntimeBackendFailureV1::Terminal(error)),
        MockMemoryFailure::Panic => std::panic::resume_unwind(error),
        MockMemoryFailure::None => unreachable!(),
    }
}

impl RuntimeBackendV1 for AllocationOnlyBackend {
    type Error = Box<Diagnostic>;

    fn enumerate_devices_v1(
        &mut self,
    ) -> Result<Vec<BackendDeviceDescriptionV1>, RuntimeBackendFailureV1<Self::Error>> {
        Ok(self.inner.enumerate_devices_v1().unwrap())
    }

    fn allocate_v1(
        &mut self,
        device: u64,
        kind: RuntimeMemoryKindV1,
        bytes: u64,
        alignment: u64,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        match self.allocate_with_outcome_v1(device, kind, bytes, alignment)? {
            RuntimeBackendAllocationOutcomeV1::Allocated(handle) => Ok(handle),
            RuntimeBackendAllocationOutcomeV1::SettledNoOwner(error) => {
                Err(RuntimeBackendFailureV1::Quiescent(error))
            }
        }
    }

    fn allocate_with_outcome_v1(
        &mut self,
        device: u64,
        kind: RuntimeMemoryKindV1,
        bytes: u64,
        alignment: u64,
    ) -> Result<RuntimeBackendAllocationOutcomeV1<Self::Error>, RuntimeBackendFailureV1<Self::Error>>
    {
        self.attempts += 1;
        if let Some(error) = self.diagnostic.take() {
            if self.panic_before_outcome {
                std::panic::resume_unwind(error);
            }
            return Ok(RuntimeBackendAllocationOutcomeV1::SettledNoOwner(error));
        }
        Ok(RuntimeBackendAllocationOutcomeV1::Allocated(
            self.inner
                .allocate_v1(device, kind, bytes, alignment)
                .unwrap(),
        ))
    }

    fn release_allocation_v1(
        &mut self,
        allocation: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.release_calls += 1;
        diagnostic_failure(
            core::mem::take(&mut self.release_failure),
            &mut self.release_diagnostic,
        )?;
        self.inner.release_allocation_v1(allocation).unwrap();
        Ok(())
    }

    fn create_stream_v1(
        &mut self,
        device: u64,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.fail_call(DiagnosticCall::CreateStream)?;
        Ok(self.inner.create_stream_v1(device).unwrap())
    }
    fn destroy_stream_v1(
        &mut self,
        stream: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.fail_call(DiagnosticCall::DestroyStream)?;
        self.inner.destroy_stream_v1(stream).unwrap();
        Ok(())
    }
    fn write_allocation_v1(
        &mut self,
        allocation: u64,
        offset: u64,
        bytes: &[u8],
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.write_calls += 1;
        let failure = core::mem::take(&mut self.write_failure);
        if failure == MockMemoryFailure::Rejected {
            return diagnostic_failure(failure, &mut self.write_diagnostic);
        }
        let length = if failure == MockMemoryFailure::None {
            bytes.len()
        } else {
            self.write_prefix.min(bytes.len())
        };
        self.inner
            .write_allocation_v1(allocation, offset, &bytes[..length])
            .unwrap();
        diagnostic_failure(failure, &mut self.write_diagnostic)
    }
    fn read_allocation_v1(
        &mut self,
        allocation: u64,
        offset: u64,
        bytes: &mut [u8],
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.fail_call(DiagnosticCall::Read)?;
        self.inner
            .read_allocation_v1(allocation, offset, bytes)
            .unwrap();
        Ok(())
    }
    fn load_module_v1(
        &mut self,
        device: u64,
        image: &[u8],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.fail_call(DiagnosticCall::LoadModule)?;
        Ok(self.inner.load_module_v1(device, image).unwrap())
    }
    fn unload_module_v1(
        &mut self,
        module: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.fail_call(DiagnosticCall::UnloadModule)?;
        self.inner.unload_module_v1(module).unwrap();
        Ok(())
    }
    fn resolve_kernel_v1(
        &mut self,
        module: u64,
        name: &str,
        signature: [u8; 32],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.fail_call(DiagnosticCall::ResolveKernel)?;
        Ok(self
            .inner
            .resolve_kernel_v1(module, name, signature)
            .unwrap())
    }
    fn submit_v1(
        &mut self,
        launch: BackendLaunchV1<'_>,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.fail_call(DiagnosticCall::Submit)?;
        Ok(self.inner.submit_v1(launch).unwrap())
    }
    fn poll_v1(
        &mut self,
        submission: u64,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>> {
        self.fail_call(DiagnosticCall::Poll)?;
        Ok(self.inner.poll_v1(submission).unwrap())
    }
    fn wait_v1(
        &mut self,
        submission: u64,
        deadline: Instant,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>> {
        self.fail_call(DiagnosticCall::Wait)?;
        Ok(self.inner.wait_v1(submission, deadline).unwrap())
    }
    fn release_submission_v1(
        &mut self,
        submission: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.fail_call(DiagnosticCall::ReleaseSubmission)?;
        self.inner.release_submission_v1(submission).unwrap();
        Ok(())
    }
    fn record_event_v1(
        &mut self,
        stream: u64,
        submission: u64,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.fail_call(DiagnosticCall::RecordEvent)?;
        Ok(self.inner.record_event_v1(stream, submission).unwrap())
    }
    fn release_event_v1(&mut self, event: u64) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.fail_call(DiagnosticCall::ReleaseEvent)?;
        self.inner.release_event_v1(event).unwrap();
        Ok(())
    }
    fn peer_copy_v1(
        &mut self,
        _: u64,
        _: BackendMemoryRegionV1,
        _: BackendMemoryRegionV1,
        _: &[u64],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        unreachable!()
    }
}

impl RuntimeCancellationBackendV1 for AllocationOnlyBackend {
    fn cancel_v1(
        &mut self,
        submission: u64,
    ) -> Result<BackendCancellationV1, RuntimeBackendFailureV1<Self::Error>> {
        self.fail_call(DiagnosticCall::Cancel)?;
        Ok(self.inner.cancel_v1(submission).unwrap())
    }
    fn drain_v1(
        &mut self,
        submission: u64,
        deadline: Instant,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>> {
        self.fail_call(DiagnosticCall::Drain)?;
        Ok(self.inner.drain_v1(submission, deadline).unwrap())
    }
}

impl RuntimeFlushBackendV1 for AllocationOnlyBackend {
    fn flush_stream_v1(&mut self, stream: u64) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.fail_call(DiagnosticCall::Flush)?;
        self.inner.flush_stream_v1(stream).unwrap();
        Ok(())
    }
}

#[test]
fn journal_settled_no_owner_refunds_without_replacing_diagnostic_or_panic_payload() {
    for credits in [false, true] {
        for panic in [false, true] {
            let mut context = RuntimeContextV1::open_with_version_journal_v1(
                AllocationOnlyBackend::default(),
                1,
                1,
            )
            .unwrap();
            let device = context.devices()[0].id();
            if credits {
                context
                    .configure_allocation_admission_v1(device, 8, 1)
                    .unwrap();
            }
            let drops = Arc::new(AtomicUsize::new(0));
            let diagnostic = Box::new(Diagnostic {
                drops: drops.clone(),
            });
            let pointer = &*diagnostic as *const Diagnostic;
            context.backend.diagnostic = Some(diagnostic);
            context.backend.panic_before_outcome = panic;
            let result = catch_unwind(AssertUnwindSafe(|| {
                context.allocate(device, RuntimeMemoryKindV1::DeviceLocal, 8, 8)
            }));
            if panic {
                let payload = result.unwrap_err().downcast::<Diagnostic>().unwrap();
                assert_eq!(&*payload as *const Diagnostic, pointer);
                assert!(context.is_terminal());
                assert_eq!(
                    context
                        .version_journal_usage_v1()
                        .unwrap()
                        .provisional_records,
                    1
                );
                drop(payload);
            } else {
                let Err(RuntimeErrorV1::BackendQuiescent(error)) = result.unwrap() else {
                    panic!("wrong diagnostic class");
                };
                assert_eq!(&*error as *const Diagnostic, pointer);
                assert_eq!(
                    context
                        .version_journal_usage_v1()
                        .unwrap()
                        .allocation_records,
                    0
                );
                assert!(context.cleanup().is_complete());
                drop(error);
            }
            assert_eq!(context.next_identity, 2);
            assert_eq!(context.backend.attempts, 1);
            assert_eq!(drops.load(Ordering::SeqCst), 1);
            let report = context.cleanup();
            assert_eq!(
                report.allocation_credit_records_v1(),
                usize::from(credits && panic)
            );
            assert_eq!(report.allocation_journal_records_v1(), usize::from(panic));
        }
    }
}

#[test]
fn allocation_settlement_preserves_diagnostic_identity_neighbors_and_exact_credit_vector() {
    for configured in [false, true] {
        for neighbor in [false, true] {
            let mut context = RuntimeContextV1::open(AllocationOnlyBackend::default()).unwrap();
            let device = context.devices()[0].id();
            let other = context.devices()[1].id();
            if configured {
                context
                    .configure_allocation_admission_v1(
                        device,
                        if neighbor { 16 } else { 8 },
                        if neighbor { 2 } else { 1 },
                    )
                    .unwrap();
                context
                    .configure_allocation_admission_v1(other, 8, 1)
                    .unwrap();
            }
            let mut retained = vec![
                context
                    .allocate(other, RuntimeMemoryKindV1::HostVisible, 8, 8)
                    .unwrap(),
            ];
            if neighbor {
                retained.push(
                    context
                        .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 8, 8)
                        .unwrap(),
                );
            }
            let before = context.allocation_admission_usage_v1(device).unwrap();
            let other_before = context.allocation_admission_usage_v1(other).unwrap();
            let memory = context.backend.inner.memory.clone();
            let backend_handles = context.backend_allocations.clone();
            let allocation_count = context.allocations.len();
            let attempts = context.backend.attempts;
            for retry in 1..=3 {
                let drops = Arc::new(AtomicUsize::new(0));
                let error = Box::new(Diagnostic {
                    drops: drops.clone(),
                });
                let address = &*error as *const Diagnostic;
                context.backend.diagnostic = Some(error);
                let Err(RuntimeErrorV1::BackendQuiescent(error)) =
                    context.allocate(device, RuntimeMemoryKindV1::DeviceLocal, 8, 8)
                else {
                    panic!("wrong settled outcome");
                };
                assert_eq!(&*error as *const Diagnostic, address);
                assert_eq!(drops.load(Ordering::SeqCst), 0);
                assert!(!context.is_terminal());
                assert_eq!(context.backend.attempts, attempts + retry);
                assert_eq!(context.allocations.len(), allocation_count);
                assert_eq!(context.backend_allocations, backend_handles);
                assert_eq!(context.backend.inner.memory, memory);
                assert!(context.backend.inner.cleanup_log.is_empty());
                assert_eq!(
                    context.allocation_admission_usage_v1(device).unwrap(),
                    before
                );
                assert_eq!(
                    context.allocation_admission_usage_v1(other).unwrap(),
                    other_before
                );
                drop(error);
                assert_eq!(drops.load(Ordering::SeqCst), 1);
            }
            let allocation = context
                .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 8, 8)
                .unwrap();
            assert_eq!(context.backend.attempts, attempts + 4);
            if configured {
                let usage = context
                    .allocation_admission_usage_v1(device)
                    .unwrap()
                    .unwrap();
                assert_eq!(
                    usage.used.get(K::RequestedAllocationBytes),
                    if neighbor { 16 } else { 8 }
                );
                assert_eq!(
                    usage.used.get(K::AllocationRecords),
                    if neighbor { 2 } else { 1 }
                );
                assert_eq!(usage.quarantined_records, 0);
            }
            context.release_allocation(allocation).unwrap();
            assert_eq!(
                context.allocation_admission_usage_v1(device).unwrap(),
                before
            );
            assert!(context.release_allocation(allocation).is_err());
            assert_eq!(context.backend.inner.cleanup_log.len(), 1);
            for allocation in retained {
                context.release_allocation(allocation).unwrap();
            }
            for device in [device, other] {
                if configured {
                    let usage = context
                        .allocation_admission_usage_v1(device)
                        .unwrap()
                        .unwrap();
                    assert_eq!(usage.used, RuntimeResourceVectorV1::ZERO);
                    assert_eq!(
                        (
                            usage.reserved_records,
                            usage.retained_records,
                            usage.quarantined_records
                        ),
                        (0, 0, 0)
                    );
                }
            }
            assert!(context.cleanup().is_complete());
            assert!(context.shutdown().is_ok());
        }
    }
}

#[test]
fn allocation_panic_before_settled_outcome_keeps_credit_and_original_payload() {
    let mut context = RuntimeContextV1::open(AllocationOnlyBackend::default()).unwrap();
    let device = context.devices()[0].id();
    context
        .configure_allocation_admission_v1(device, 8, 1)
        .unwrap();
    let drops = Arc::new(AtomicUsize::new(0));
    let error = Box::new(Diagnostic {
        drops: drops.clone(),
    });
    let address = &*error as *const Diagnostic;
    context.backend.diagnostic = Some(error);
    context.backend.panic_before_outcome = true;
    let payload = catch_unwind(AssertUnwindSafe(|| {
        context.allocate(device, RuntimeMemoryKindV1::DeviceLocal, 8, 8)
    }))
    .unwrap_err()
    .downcast::<Diagnostic>()
    .unwrap();
    assert_eq!(&*payload as *const Diagnostic, address);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert!(context.is_terminal());
    let usage = context
        .allocation_admission_usage_v1(device)
        .unwrap()
        .unwrap();
    assert_eq!(usage.used.get(K::RequestedAllocationBytes), 8);
    assert_eq!(usage.used.get(K::AllocationRecords), 1);
    assert_eq!(usage.quarantined_records, 1);
    assert!(context.allocations.is_empty() && context.backend.inner.memory.is_empty());
    assert!(matches!(
        context.allocate(device, RuntimeMemoryKindV1::DeviceLocal, 8, 8),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::ContextTerminal
        ))
    ));
    assert_eq!(context.backend.attempts, 1);
    assert_eq!(
        context
            .allocation_admission_usage_v1(device)
            .unwrap()
            .unwrap(),
        usage
    );
    assert!(context.backend_allocations.is_empty());
    assert_eq!(context.cleanup().allocation_credit_records_v1(), 1);
    drop(payload);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}
