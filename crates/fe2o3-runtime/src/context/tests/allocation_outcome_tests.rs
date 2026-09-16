use super::*;
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
        self.inner.release_allocation_v1(allocation).unwrap();
        Ok(())
    }

    fn create_stream_v1(&mut self, _: u64) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        unreachable!()
    }
    fn destroy_stream_v1(&mut self, _: u64) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        unreachable!()
    }
    fn write_allocation_v1(
        &mut self,
        _: u64,
        _: u64,
        _: &[u8],
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        unreachable!()
    }
    fn read_allocation_v1(
        &mut self,
        _: u64,
        _: u64,
        _: &mut [u8],
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        unreachable!()
    }
    fn load_module_v1(
        &mut self,
        _: u64,
        _: &[u8],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        unreachable!()
    }
    fn unload_module_v1(&mut self, _: u64) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        unreachable!()
    }
    fn resolve_kernel_v1(
        &mut self,
        _: u64,
        _: &str,
        _: [u8; 32],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        unreachable!()
    }
    fn submit_v1(
        &mut self,
        _: BackendLaunchV1<'_>,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        unreachable!()
    }
    fn poll_v1(&mut self, _: u64) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>> {
        unreachable!()
    }
    fn wait_v1(
        &mut self,
        _: u64,
        _: Instant,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>> {
        unreachable!()
    }
    fn release_submission_v1(
        &mut self,
        _: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        unreachable!()
    }
    fn record_event_v1(
        &mut self,
        _: u64,
        _: u64,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        unreachable!()
    }
    fn release_event_v1(&mut self, _: u64) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        unreachable!()
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
