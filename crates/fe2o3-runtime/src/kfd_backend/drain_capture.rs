//! Direct retained-host capture only. No synchronization or materialization fallback.
use super::*;
use crate::{BackendHostCaptureV1, RuntimeHostCaptureErrorV1};
use fe2o3_kfd::Gfx942SdmaHostReadIntoErrorV1;

fn capture_record_v1(
    record: &AllocationRecordV1,
    device: u64,
    offset: u64,
    destination_len: usize,
) -> Result<&SdmaBufferOwnerV1, RuntimeHostCaptureErrorV1> {
    use RuntimeHostCaptureErrorV1 as Error;
    if record.device != device {
        return Err(Error::ForeignContext);
    }
    if record.kind != RuntimeMemoryKindV1::HostVisible {
        return Err(Error::DeviceLocal);
    }
    let bytes = u64::try_from(destination_len).map_err(|_| Error::InvalidRange)?;
    if bytes == 0
        || offset
            .checked_add(bytes)
            .is_none_or(|end| end > record.bytes.len() as u64)
    {
        return Err(Error::InvalidRange);
    }
    if !record.sdma_backed || !record.sdma_initialized || !record.native_dirty.is_empty() {
        return Err(Error::SourceUnavailable);
    }
    let KfdRuntimeSdmaStorageV1::Host(buffer) = &record.sdma_storage else {
        return Err(Error::SourceUnavailable);
    };
    // A completed D2H can leave the Arc shadow stale. The native buffer, not
    // that shadow or its hash, is the capture source.
    Ok(buffer)
}

fn finish_capture_read_v1(
    terminal: &mut bool,
    result: Result<(), Gfx942SdmaHostReadIntoErrorV1>,
) -> Result<(), RuntimeBackendFailureV1<RuntimeHostCaptureErrorV1>> {
    use Gfx942SdmaHostReadIntoErrorV1 as Native;
    use RuntimeHostCaptureErrorV1 as Error;
    match result {
        Ok(()) => Ok(()),
        Err(Native::NativeUncertain) => {
            *terminal = true;
            Err(RuntimeBackendFailureV1::Terminal(Error::NativeUncertain))
        }
        Err(Native::InvalidRange) => Err(RuntimeBackendFailureV1::Rejected(Error::InvalidRange)),
        Err(Native::InvalidBuffer | Native::Unavailable) => {
            Err(RuntimeBackendFailureV1::Rejected(Error::NativeRejected))
        }
    }
}

fn read_capture_record_into_v1(
    record: &AllocationRecordV1,
    terminal: &mut bool,
    device: u64,
    offset: u64,
    destination: &mut [u8],
    read: impl FnOnce(&SdmaBufferOwnerV1, u64, &mut [u8]) -> Result<(), Gfx942SdmaHostReadIntoErrorV1>,
) -> Result<(), RuntimeBackendFailureV1<RuntimeHostCaptureErrorV1>> {
    let buffer = capture_record_v1(record, device, offset, destination.len())
        .map_err(RuntimeBackendFailureV1::Rejected)?;
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        read(buffer, offset, destination)
    })) {
        Ok(result) => finish_capture_read_v1(terminal, result),
        Err(payload) => {
            *terminal = true;
            std::panic::resume_unwind(payload)
        }
    }
}

impl KfdRuntimeBackendV1 {
    fn capture_has_pending_custody_v1(&self) -> bool {
        self.any_compute_active_v1()
            || !self.pending_compute.is_empty()
            || self
                .pending_compute_streams
                .values()
                .any(|queue| !queue.is_empty())
            || !self.allocation_custody.is_empty()
            || self.compute_completion_reservations != 0
            || self.sdma_completion_reservations != 0
            || !self.active_sdma.is_empty()
            || !self.published_sdma_submissions.is_empty()
            || self
                .active_sdma_streams
                .values()
                .any(|queue| !queue.is_empty())
            || self
                .submissions
                .values()
                .any(|submission| submission.status == BackendPollV1::Pending)
    }

    pub(super) fn capture_coherent_host_range_impl_v1(
        &mut self,
        mut request: BackendHostCaptureV1<'_>,
    ) -> Result<(), RuntimeBackendFailureV1<RuntimeHostCaptureErrorV1>> {
        let device = request.device();
        let allocation = request.allocation();
        let offset = request.byte_offset();
        self.capture_coherent_host_range_into_v1(
            device,
            allocation,
            offset,
            request.destination_mut(),
        )
    }

    fn capture_coherent_host_range_into_v1(
        &mut self,
        device: u64,
        allocation: u64,
        offset: u64,
        destination: &mut [u8],
    ) -> Result<(), RuntimeBackendFailureV1<RuntimeHostCaptureErrorV1>> {
        use RuntimeHostCaptureErrorV1 as Error;
        if self.terminal || self.terminal_memory.is_some() || self.terminal_sdma_custody.is_some() {
            self.terminal = true;
            return Err(RuntimeBackendFailureV1::Terminal(Error::ContextTerminal));
        }
        if device != self.description.backend_device {
            return Err(RuntimeBackendFailureV1::Rejected(Error::ForeignContext));
        }
        if !self.native_available {
            return Err(RuntimeBackendFailureV1::Rejected(Error::UnsupportedBackend));
        }
        if self.capture_has_pending_custody_v1() {
            return Err(RuntimeBackendFailureV1::Rejected(Error::Pending));
        }
        if self.queue_retired || !self.sdma_enabled {
            return Err(RuntimeBackendFailureV1::Rejected(Error::SourceUnavailable));
        }
        let record = self
            .allocations
            .get(&allocation)
            .ok_or(RuntimeBackendFailureV1::Rejected(Error::UnknownAllocation))?;
        let queue = self
            .queue
            .as_mut()
            .ok_or(RuntimeBackendFailureV1::Rejected(Error::SourceUnavailable))?;
        // The actual retained native token is resolved here, after Context's
        // drain gate. No earlier registration-time native generation is reused.
        read_capture_record_into_v1(
            record,
            &mut self.terminal,
            device,
            offset,
            destination,
            |buffer, offset, destination| {
                match buffer {
                    SdmaBufferOwnerV1::Native(buffer)
                        if buffer.requested_bytes() == record.bytes.len() as u64 => {}
                    _ => return Err(Gfx942SdmaHostReadIntoErrorV1::InvalidBuffer),
                }
                kfd_backend_sdma_seam::DirectionalSdmaOpsV1::Native(queue).read_host_into_v1(
                    buffer,
                    offset,
                    destination,
                )
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    std::thread_local! {
        static COUNT_ALLOCATIONS: Cell<bool> = const { Cell::new(false) };
        static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
    }

    struct CountingAllocator;

    fn note_allocation() {
        let _ = COUNT_ALLOCATIONS.try_with(|enabled| {
            if enabled.get() {
                let _ = ALLOCATIONS.try_with(|count| count.set(count.get().saturating_add(1)));
            }
        });
    }

    // Test-only transparent system allocator; counters use non-dropping TLS.
    #[allow(unsafe_code)]
    unsafe impl GlobalAlloc for CountingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            note_allocation();
            // SAFETY: forward the allocator caller's unchanged layout.
            unsafe { System.alloc(layout) }
        }
        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            note_allocation();
            // SAFETY: forward the allocator caller's unchanged layout.
            unsafe { System.alloc_zeroed(layout) }
        }
        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
            note_allocation();
            // SAFETY: the original pointer/layout and requested size are unchanged.
            unsafe { System.realloc(ptr, layout, size) }
        }
        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            // SAFETY: this pointer was allocated by the same System allocator.
            unsafe { System.dealloc(ptr, layout) }
        }
    }

    #[global_allocator]
    static ALLOCATOR: CountingAllocator = CountingAllocator;

    fn counted<R>(operation: impl FnOnce() -> R) -> (R, usize) {
        struct Stop;
        impl Drop for Stop {
            fn drop(&mut self) {
                COUNT_ALLOCATIONS.with(|enabled| enabled.set(false));
            }
        }
        ALLOCATIONS.with(|count| count.set(0));
        COUNT_ALLOCATIONS.with(|enabled| assert!(!enabled.replace(true)));
        let stop = Stop;
        let result = operation();
        drop(stop);
        (result, ALLOCATIONS.with(Cell::get))
    }

    fn host_record() -> AllocationRecordV1 {
        let driver = ScriptedSdmaDriverV1::new([]);
        AllocationRecordV1 {
            device: 7,
            kind: RuntimeMemoryKindV1::HostVisible,
            alignment: 8,
            bytes: Arc::from([0_u8; 32]),
            content_sha256: None,
            last_full_host_write: None,
            native_dirty: Vec::new(),
            sdma_storage: KfdRuntimeSdmaStorageV1::Host(driver.test_host_owner(32)),
            sdma_backed: true,
            sdma_initialized: true,
            sdma_shadow_dirty: true,
            scripted_three_binding_replay: false,
        }
    }

    #[test]
    fn coherent_capture_record_reads_native_bytes_not_stale_shadow_without_heap() {
        let record = host_record();
        let native_bytes = [0x5a_u8; 32];
        let mut terminal = false;
        let mut destination = [0xa5_u8; 12];
        // The reader is a CPU stand-in; this does not exercise Linux syscalls.
        let (result, allocations) = counted(|| {
            read_capture_record_into_v1(
                &record,
                &mut terminal,
                7,
                8,
                &mut destination[2..10],
                |owner, offset, destination| {
                    assert!(std::ptr::eq(
                        owner,
                        capture_record_v1(&record, 7, 8, 8).unwrap()
                    ));
                    destination.copy_from_slice(
                        &native_bytes[offset as usize..offset as usize + destination.len()],
                    );
                    Ok(())
                },
            )
        });
        assert_eq!(allocations, 0);
        assert!(result.is_ok());
        assert!(!terminal);
        assert_eq!(
            destination,
            [
                0xa5, 0xa5, 0x5a, 0x5a, 0x5a, 0x5a, 0x5a, 0x5a, 0x5a, 0x5a, 0xa5, 0xa5
            ]
        );
        assert!(record.sdma_shadow_dirty);
        assert!(record.bytes.iter().all(|byte| *byte == 0));
    }

    #[test]
    fn coherent_capture_record_rejections_do_not_read_write_or_allocate() {
        use RuntimeHostCaptureErrorV1 as Error;
        for case in 0..11 {
            let mut record = host_record();
            let (offset, length, expected) = match case {
                0 => {
                    record.device = 8;
                    (8, 8, Error::ForeignContext)
                }
                1 => {
                    record.kind = RuntimeMemoryKindV1::DeviceLocal;
                    (8, 8, Error::DeviceLocal)
                }
                2 => (0, 0, Error::InvalidRange),
                3 => (u64::MAX, 8, Error::InvalidRange),
                4 => (28, 8, Error::InvalidRange),
                5 => {
                    record.sdma_backed = false;
                    (8, 8, Error::SourceUnavailable)
                }
                6 => {
                    record.sdma_initialized = false;
                    (8, 8, Error::SourceUnavailable)
                }
                7 => {
                    record.native_dirty.push(NativeDirtyExtentV1 {
                        compute_lane: 0,
                        data_index: 0,
                        allocation_offset: 0,
                        data_offset: 0,
                        byte_len: 1,
                    });
                    (8, 8, Error::SourceUnavailable)
                }
                8 => {
                    record.sdma_storage = KfdRuntimeSdmaStorageV1::Synthetic;
                    (8, 8, Error::SourceUnavailable)
                }
                9 => {
                    record.sdma_storage =
                        KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Synchronous);
                    (8, 8, Error::SourceUnavailable)
                }
                _ => {
                    record.sdma_storage = KfdRuntimeSdmaStorageV1::ComputeInFlight(2);
                    (8, 8, Error::SourceUnavailable)
                }
            };
            let mut destination = [0xa5; 8];
            let mut terminal = false;
            let (result, allocations) = counted(|| {
                read_capture_record_into_v1(
                    &record,
                    &mut terminal,
                    7,
                    offset,
                    &mut destination[..length],
                    |_, _, _| panic!("rejected capture reached native read"),
                )
            });
            assert!(
                matches!(result, Err(RuntimeBackendFailureV1::Rejected(error)) if error == expected)
            );
            assert_eq!(allocations, 0);
            assert_eq!(destination, [0xa5; 8]);
            assert!(!terminal);
        }
    }

    #[test]
    fn coherent_capture_uncertain_read_returns_no_success_and_latches_terminal() {
        let record = host_record();
        let mut terminal = false;
        let mut destination = [0xa5; 8];
        let (result, allocations) = counted(|| {
            read_capture_record_into_v1(
                &record,
                &mut terminal,
                7,
                8,
                &mut destination,
                |_, _, destination| {
                    destination[..4].fill(0x5a);
                    Err(Gfx942SdmaHostReadIntoErrorV1::NativeUncertain)
                },
            )
        });
        assert!(matches!(
            result,
            Err(RuntimeBackendFailureV1::Terminal(
                RuntimeHostCaptureErrorV1::NativeUncertain
            ))
        ));
        assert!(terminal);
        assert_eq!(allocations, 0);
        assert_eq!(
            destination,
            [0x5a, 0x5a, 0x5a, 0x5a, 0xa5, 0xa5, 0xa5, 0xa5]
        );
    }

    #[test]
    fn coherent_capture_preexisting_native_terminal_is_not_a_retryable_rejection() {
        let record = host_record();
        let mut terminal = false;
        let mut destination = [0xa5; 8];
        let (result, allocations) = counted(|| {
            read_capture_record_into_v1(
                &record,
                &mut terminal,
                7,
                8,
                &mut destination,
                |_, _, _| Err(Gfx942SdmaHostReadIntoErrorV1::NativeUncertain),
            )
        });
        assert!(matches!(
            result,
            Err(RuntimeBackendFailureV1::Terminal(
                RuntimeHostCaptureErrorV1::NativeUncertain
            ))
        ));
        assert!(terminal);
        assert_eq!(destination, [0xa5; 8]);
        assert_eq!(allocations, 0);
    }

    #[test]
    fn coherent_capture_backend_rejects_pending_terminal_and_unsupported_without_heap() {
        use RuntimeHostCaptureErrorV1 as Error;
        for case in 0..12 {
            let mut backend = KfdRuntimeBackendV1::mock();
            backend.scripted_sdma = Some(ScriptedSdmaDriverV1::new([]));
            backend.scripted_drop_disarmed = true;
            backend.native_available = true;
            backend.sdma_enabled = true;
            let expected = match case {
                0 => {
                    backend.native_available = false;
                    Error::UnsupportedBackend
                }
                1 => {
                    backend.terminal = true;
                    Error::ContextTerminal
                }
                2 => {
                    backend.compute_completion_reservations = 1;
                    Error::Pending
                }
                3 => {
                    backend.sdma_completion_reservations = 1;
                    Error::Pending
                }
                4 => {
                    backend
                        .pending_compute_streams
                        .insert(1, VecDeque::from([2]));
                    Error::Pending
                }
                5 => {
                    backend.active_sdma_streams.insert(1, VecDeque::from([2]));
                    Error::Pending
                }
                6 => {
                    backend.published_sdma_submissions.push(2);
                    Error::Pending
                }
                7 => {
                    backend.submissions.insert(
                        2,
                        SubmissionRecordV1 {
                            stream: 1,
                            status: BackendPollV1::Pending,
                            profile_dispatch_published: false,
                        },
                    );
                    Error::Pending
                }
                8 => {
                    backend.queue_retired = true;
                    Error::SourceUnavailable
                }
                9 => {
                    backend.allocation_custody.insert(
                        1,
                        RuntimeAllocationCustodyV1 {
                            owners: VecDeque::new(),
                            sole_stream: None,
                            owner_counts: [0; 2],
                        },
                    );
                    Error::Pending
                }
                10 => Error::ForeignContext,
                _ => Error::UnknownAllocation,
            };
            let mut destination = [0xa5; 8];
            let (result, allocations) = counted(|| {
                backend.capture_coherent_host_range_into_v1(
                    if case == 10 { 8 } else { 7 },
                    1,
                    0,
                    &mut destination,
                )
            });
            assert!(
                matches!(result, Err(RuntimeBackendFailureV1::Rejected(error) | RuntimeBackendFailureV1::Terminal(error)) if error == expected)
            );
            assert_eq!(destination, [0xa5; 8]);
            assert_eq!(allocations, 0);
        }
    }

    #[test]
    fn coherent_capture_reader_panic_preserves_payload_and_terminal_custody() {
        let record = host_record();
        let mut terminal = false;
        let mut destination = [0xa5; 8];
        let payload = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            read_capture_record_into_v1(
                &record,
                &mut terminal,
                7,
                8,
                &mut destination,
                |_, _, destination| {
                    destination[0] = 0x5a;
                    std::panic::panic_any(0x4452_4e31_u32);
                },
            )
        }))
        .unwrap_err();
        assert_eq!(payload.downcast_ref::<u32>(), Some(&0x4452_4e31));
        assert!(terminal);
        assert_eq!(
            destination,
            [0x5a, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5]
        );
        assert!(matches!(
            record.sdma_storage,
            KfdRuntimeSdmaStorageV1::Host(_)
        ));
    }

    #[test]
    fn coherent_capture_native_seam_rejects_scripted_storage_without_fallback() {
        let mut driver = ScriptedSdmaDriverV1::new([]);
        let owner = driver.test_host_owner(32);
        let mut destination = [0xa5; 8];
        let (result, allocations) =
            counted(|| {
                kfd_backend_sdma_seam::DirectionalSdmaOpsV1::Scripted(&mut driver)
                    .read_host_into_v1(&owner, 8, &mut destination)
            });
        assert_eq!(result, Err(Gfx942SdmaHostReadIntoErrorV1::InvalidBuffer));
        assert_eq!(destination, [0xa5; 8]);
        assert_eq!(allocations, 0);
    }
}
