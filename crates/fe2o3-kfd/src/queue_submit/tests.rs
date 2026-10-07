use super::*;
use fe2o3_aql::{
    AqlBarrierAndPacketV1, AqlDependencySignalObservationV1, AqlDispatchGeometryV1,
    AqlKernelDispatchPacketV1, AqlPreparedDependencyDispatchV1, AqlPreparedKernelDispatchBatchV2,
    ObservedGpuAddressV1,
};

#[repr(align(64))]
struct AlignedRing([u8; 524_416]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FailureAfterV1 {
    FetchAdd,
    Body(usize),
    Header(usize),
    Doorbell,
}

struct FakeBackend {
    ring: Box<AlignedRing>,
    logical_bytes: usize,
    write: AtomicU64,
    read: AtomicU64,
    checks: usize,
    fail_check: Option<usize>,
    fail_check_error: Option<NativeAqlSubmissionErrorV1>,
    fail_after: Option<FailureAfterV1>,
    fail_error: Option<NativeAqlSubmissionErrorV1>,
    panic_check: Option<usize>,
    panic_after: Option<FailureAfterV1>,
    fetch_return_override: Option<u64>,
    body_calls: usize,
    header_calls: usize,
    trace: Vec<&'static str>,
    doorbells: Vec<u64>,
}

impl FakeBackend {
    fn new(write: u64, read: u64) -> Self {
        Self::with_ring_bytes(4_096, write, read)
    }

    fn with_ring_bytes(logical_bytes: usize, write: u64, read: u64) -> Self {
        let mut ring = Box::<AlignedRing>::new_uninit();
        // SAFETY: every byte pattern is valid for the wrapped byte array.
        // Initializing the allocation in place avoids a maximum-ring-sized
        // temporary on the test thread's stack.
        unsafe {
            ring.as_mut_ptr().cast::<u8>().write_bytes(0xa5, 524_416);
        }
        // SAFETY: the complete allocation was initialized above.
        let mut ring = unsafe { ring.assume_init() };
        initialize_invalid_ring(&mut ring.0[64..64 + logical_bytes]).unwrap();
        Self {
            ring,
            logical_bytes,
            write: AtomicU64::new(write),
            read: AtomicU64::new(read),
            checks: 0,
            fail_check: None,
            fail_check_error: None,
            fail_after: None,
            fail_error: None,
            panic_check: None,
            panic_after: None,
            fetch_return_override: None,
            body_calls: 0,
            header_calls: 0,
            trace: Vec::new(),
            doorbells: Vec::new(),
        }
    }

    fn logical_ring(&mut self) -> &mut [u8] {
        &mut self.ring.0[64..64 + self.logical_bytes]
    }

    fn slot_word(&mut self, slot: u32, byte_offset: usize) -> u32 {
        let start = slot as usize * AQL_KERNEL_DISPATCH_PACKET_BYTES_V1 + byte_offset;
        u32::from_le_bytes(self.logical_ring()[start..start + 4].try_into().unwrap())
    }
}

impl NativeAqlSubmissionBackendV1 for FakeBackend {
    fn check_currentness(&mut self) -> Result<(), NativeAqlSubmissionErrorV1> {
        self.checks += 1;
        self.trace.push("check");
        assert_ne!(
            self.panic_check,
            Some(self.checks),
            "injected currentness panic"
        );
        if self.fail_check == Some(self.checks) {
            Err(self
                .fail_check_error
                .take()
                .unwrap_or(NativeAqlSubmissionErrorV1::Currentness))
        } else {
            Ok(())
        }
    }

    fn observe_counters_acquire(&mut self) -> Result<(u64, u64), NativeAqlSubmissionErrorV1> {
        self.trace.push("observe");
        Ok((
            self.write.load(Ordering::Acquire),
            self.read.load(Ordering::Acquire),
        ))
    }

    fn fetch_add_write_acq_rel(
        &mut self,
        increment: u64,
    ) -> Result<u64, NativeAqlSubmissionErrorV1> {
        self.trace.push("fetch-add");
        assert_ne!(
            self.panic_after,
            Some(FailureAfterV1::FetchAdd),
            "injected fetch-add panic"
        );
        let observed = self.write.fetch_add(increment, Ordering::AcqRel);
        if self.fail_after == Some(FailureAfterV1::FetchAdd) {
            return Err(self
                .fail_error
                .take()
                .unwrap_or(NativeAqlSubmissionErrorV1::Currentness));
        }
        Ok(self.fetch_return_override.unwrap_or(observed))
    }

    fn write_unpublished(
        &mut self,
        slot: u32,
        packet: &[u8; AQL_KERNEL_DISPATCH_PACKET_BYTES_V1],
    ) -> Result<(), NativeAqlSubmissionErrorV1> {
        self.trace.push("body");
        let call = self.body_calls;
        self.body_calls += 1;
        assert_ne!(
            self.panic_after,
            Some(FailureAfterV1::Body(call)),
            "injected body panic"
        );
        write_unpublished_slot(self.logical_ring(), slot, packet)?;
        if self.fail_after == Some(FailureAfterV1::Body(call)) {
            return Err(self
                .fail_error
                .take()
                .unwrap_or(NativeAqlSubmissionErrorV1::PacketBody));
        }
        Ok(())
    }

    fn publish_release_header(
        &mut self,
        slot: u32,
        header: u16,
    ) -> Result<(), NativeAqlSubmissionErrorV1> {
        self.trace.push("header");
        let call = self.header_calls;
        self.header_calls += 1;
        assert_ne!(
            self.panic_after,
            Some(FailureAfterV1::Header(call)),
            "injected header panic"
        );
        publish_slot_header_release(self.logical_ring(), slot, header)?;
        if self.fail_after == Some(FailureAfterV1::Header(call)) {
            return Err(self
                .fail_error
                .take()
                .unwrap_or(NativeAqlSubmissionErrorV1::PacketHeader));
        }
        Ok(())
    }

    fn ring_doorbell_release(&mut self, packet_id: u64) -> Result<(), NativeAqlSubmissionErrorV1> {
        self.trace.push("doorbell");
        assert_ne!(
            self.panic_after,
            Some(FailureAfterV1::Doorbell),
            "injected doorbell panic"
        );
        release_fence_before_mmio();
        self.doorbells.push(packet_id);
        if self.fail_after == Some(FailureAfterV1::Doorbell) {
            return Err(self
                .fail_error
                .take()
                .unwrap_or(NativeAqlSubmissionErrorV1::Doorbell));
        }
        Ok(())
    }
}

fn packet() -> AqlPreparedKernelDispatchV1 {
    indexed_packet(0)
}

fn barrier() -> AqlPreparedBarrierAndV1 {
    AqlBarrierAndPacketV1::new_unpublished(ObservedGpuAddressV1::new(0x30_040).unwrap()).unwrap()
}

fn indexed_packet(index: u32) -> AqlPreparedKernelDispatchV1 {
    AqlKernelDispatchPacketV1::new_unpublished(
        AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
        0,
        index,
        ObservedGpuAddressV1::new(0x10_000).unwrap(),
        ObservedGpuAddressV1::new(0x20_000).unwrap(),
        16,
        ObservedGpuAddressV1::new(0x30_000).unwrap(),
    )
    .unwrap()
}

fn batch<const N: usize>() -> AqlPreparedKernelDispatchBatchV2<N> {
    let packets = (0..N)
        .map(|index| indexed_packet(index as u32))
        .collect::<Vec<_>>()
        .into_boxed_slice()
        .try_into()
        .unwrap();
    AqlPreparedKernelDispatchBatchV2::try_from_boxed_packets(packets).unwrap()
}

fn dependency_plan(count: usize) -> AqlPreparedDependencyDispatchV1 {
    let signals = (0..count)
        .map(|index| AqlDependencySignalObservationV1::new(0x40_000 + index as u64 * 64).unwrap())
        .collect::<Vec<_>>();
    AqlPreparedDependencyDispatchV1::new(&signals, indexed_packet(7)).unwrap()
}

#[test]
fn pristine_submission_requires_unpoisoned_zero_history() {
    let mut owner = NativeAqlSubmissionOwnerV1::new(4_096).unwrap();
    assert!(owner.is_pristine_v1());
    owner.poison();
    assert!(!owner.is_pristine_v1());
    for (write, read) in [(1, 0), (1, 1), (64, 0), (64, 64), (u64::MAX, u64::MAX)] {
        let owner = NativeAqlSubmissionOwnerV1::from_counters(4_096, write, read).unwrap();
        assert!(!owner.is_pristine_v1());
    }
}

#[test]
fn pristine_submission_is_lost_after_each_actual_publication_kind() {
    for kind in 0..3 {
        let mut owner = NativeAqlSubmissionOwnerV1::new(4_096).unwrap();
        let mut backend = FakeBackend::new(0, 0);
        assert!(owner.is_pristine_v1());
        match kind {
            0 => assert_eq!(owner.submit(packet(), &mut backend), Ok(0)),
            1 => assert_eq!(owner.submit_barrier_and(barrier(), &mut backend), Ok(0)),
            2 => {
                owner
                    .submit_dependency_dispatch_classified(dependency_plan(6), &mut backend)
                    .unwrap();
            }
            _ => unreachable!(),
        }
        assert!(!owner.is_pristine_v1());
        assert!(!backend.doorbells.is_empty());
        backend
            .read
            .store(backend.write.load(Ordering::Acquire), Ordering::Release);
        assert!(!owner.is_pristine_v1());
    }
}

#[test]
fn pristine_submission_rejects_prepublication_and_native_prefix_failures() {
    for prefix in 0..5 {
        let mut owner = NativeAqlSubmissionOwnerV1::new(4_096).unwrap();
        let mut backend = FakeBackend::new(0, 0);
        if prefix == 0 {
            backend.fail_check = Some(1);
        } else {
            backend.fail_after = Some(match prefix {
                1 => FailureAfterV1::FetchAdd,
                2 => FailureAfterV1::Body(0),
                3 => FailureAfterV1::Header(0),
                4 => FailureAfterV1::Doorbell,
                _ => unreachable!(),
            });
        }
        assert!(owner.submit(packet(), &mut backend).is_err());
        assert!(!owner.is_pristine_v1());
        assert!(owner.is_poisoned_for_test());
    }
}

#[test]
fn invalid_ring_initialization_covers_every_logical_slot() {
    let mut ring = [0xff; 4_096];
    initialize_invalid_ring(&mut ring).unwrap();
    for slot in ring.chunks_exact(64) {
        assert_eq!(&slot[..4], &1_u32.to_le_bytes());
        assert!(slot[4..].iter().all(|byte| *byte == 0));
    }
    assert!(initialize_invalid_ring(&mut [0; 4_095]).is_err());
}

#[test]
fn amd_aql_control_initialization_matches_the_reviewed_layout() {
    #[repr(align(64))]
    struct AlignedControl([u8; 4096]);

    let mut control = AlignedControl([0xff; 4096]);
    initialize_amd_aql_control(&mut control.0).unwrap();
    let mut expected = [0_u8; 4096];
    expected[0x88..0x8c].copy_from_slice(&0x80_u32.to_le_bytes());
    assert_eq!(control.0, expected);
    assert_eq!(
        u64::from_le_bytes(control.0[0x38..0x40].try_into().unwrap()),
        0
    );
    assert_eq!(
        u64::from_le_bytes(control.0[0x80..0x88].try_into().unwrap()),
        0
    );
    assert!(initialize_amd_aql_control(&mut [0; 4095]).is_err());
    assert!(initialize_amd_aql_control(&mut [0; 4097]).is_err());
}

#[test]
fn first_packet_uses_id_zero_and_exact_release_header() {
    let mut owner = NativeAqlSubmissionOwnerV1::new(4_096).unwrap();
    let mut backend = FakeBackend::new(0, 0);
    let untouched_slots = backend.logical_ring()[64..].to_vec();
    assert_eq!(owner.submit(packet(), &mut backend), Ok(0));
    assert_eq!(backend.write.load(Ordering::Relaxed), 1);
    assert_eq!(backend.doorbells, [0]);
    assert_eq!(
        backend.trace,
        [
            "check",
            "observe",
            "check",
            "fetch-add",
            "body",
            "header",
            "check",
            "doorbell"
        ]
    );
    assert_eq!(
        u32::from_le_bytes(backend.logical_ring()[..4].try_into().unwrap()),
        0x0001_1402
    );
    assert!(backend.ring.0[..64].iter().all(|byte| *byte == 0xa5));
    assert!(backend.ring.0[4_160..].iter().all(|byte| *byte == 0xa5));
    assert_eq!(
        &backend.logical_ring()[64..68],
        &u32::from(AQL_INVALID_PACKET_HEADER_V1).to_le_bytes()
    );
    assert_eq!(&backend.logical_ring()[64..], untouched_slots);
}

#[test]
fn zero_dependency_barrier_uses_one_slot_and_exact_release_header() {
    let mut owner = NativeAqlSubmissionOwnerV1::new(4_096).unwrap();
    let mut backend = FakeBackend::new(0, 0);
    let packet =
        AqlBarrierAndPacketV1::new_unpublished(ObservedGpuAddressV1::new(0x30_040).unwrap())
            .unwrap();

    assert_eq!(owner.submit_barrier_and(packet, &mut backend), Ok(0));
    assert_eq!(backend.write.load(Ordering::Relaxed), 1);
    assert_eq!(backend.doorbells, [0]);
    assert_eq!(backend.slot_word(0, 0), 0x0000_1403);
    assert!(backend.logical_ring()[8..48].iter().all(|byte| *byte == 0));
    assert_eq!(
        u64::from_le_bytes(backend.logical_ring()[56..64].try_into().unwrap()),
        0x30_040
    );
}

#[test]
fn dependency_dispatch_uses_one_claim_all_bodies_all_headers_and_one_doorbell() {
    for dependency_count in [0, 1, 5, 6, 256] {
        let mut owner = NativeAqlSubmissionOwnerV1::new(4_096).unwrap();
        let mut backend = FakeBackend::new(0, 0);
        let publication = owner
            .submit_dependency_dispatch_classified(dependency_plan(dependency_count), &mut backend)
            .unwrap();
        let barriers = dependency_count.div_ceil(5);
        assert_eq!(publication.dependency_count(), dependency_count as u16);
        assert_eq!(publication.barrier_count(), barriers as u16);
        assert_eq!(publication.packet_count(), barriers as u32 + 1);
        assert_eq!(
            backend.write.load(Ordering::Relaxed),
            u64::from(publication.packet_count())
        );
        assert_eq!(backend.body_calls, barriers + 1);
        assert_eq!(backend.header_calls, barriers + 1);
        assert_eq!(backend.doorbells, [publication.last_packet_id()]);
        assert_eq!(
            backend
                .trace
                .iter()
                .filter(|event| **event == "fetch-add")
                .count(),
            1
        );
        assert_eq!(
            backend
                .trace
                .iter()
                .filter(|event| **event == "doorbell")
                .count(),
            1
        );
        let first_header = backend.slot_word(0, 0) & 0xffff;
        if dependency_count == 0 {
            assert_eq!(first_header, 0x1402);
        } else {
            assert_eq!(first_header, 0x1403);
            let final_slot = barriers as u32;
            assert_eq!(backend.slot_word(final_slot, 0) & 0xffff, 0x1502);
        }
    }
}

#[test]
fn dependency_ring_pressure_is_retryable_and_preserves_the_plan() {
    let mut owner = NativeAqlSubmissionOwnerV1::from_counters(4_096, 64, 0).unwrap();
    let mut backend = FakeBackend::new(64, 0);
    let prepared = match owner
        .submit_dependency_dispatch_classified(dependency_plan(6), &mut backend)
        .unwrap_err()
    {
        NativeDependencyDispatchSubmissionFailureV1::RetryableBeforeSideEffect {
            error: NativeAqlSubmissionErrorV1::Ring(AqlRingReservationError::Full),
            prepared,
        } => prepared,
        failure => panic!("unexpected failure: {failure:?}"),
    };
    assert_eq!(backend.trace, ["check", "observe", "check"]);
    assert_eq!(backend.write.load(Ordering::Relaxed), 64);
    backend.read.store(3, Ordering::Release);
    let publication = owner
        .submit_dependency_dispatch_classified(prepared, &mut backend)
        .unwrap();
    assert_eq!(publication.first_packet_id(), 64);
    assert_eq!(publication.last_packet_id(), 66);
}

#[test]
fn dependency_callback_failures_report_exact_terminal_boundaries() {
    let cases = [
        (
            FailureAfterV1::FetchAdd,
            AqlDependencyDispatchPublicationBoundaryV1::ClaimWriteIndex,
        ),
        (
            FailureAfterV1::Body(0),
            AqlDependencyDispatchPublicationBoundaryV1::BarrierBody { barrier_index: 0 },
        ),
        (
            FailureAfterV1::Body(2),
            AqlDependencyDispatchPublicationBoundaryV1::FinalDispatchBody,
        ),
        (
            FailureAfterV1::Header(0),
            AqlDependencyDispatchPublicationBoundaryV1::BarrierHeader { barrier_index: 0 },
        ),
        (
            FailureAfterV1::Header(2),
            AqlDependencyDispatchPublicationBoundaryV1::FinalDispatchHeader,
        ),
        (
            FailureAfterV1::Doorbell,
            AqlDependencyDispatchPublicationBoundaryV1::Doorbell,
        ),
    ];
    for (stage, expected_boundary) in cases {
        let mut owner = NativeAqlSubmissionOwnerV1::new(4_096).unwrap();
        let mut backend = FakeBackend::new(0, 0);
        backend.fail_after = Some(stage);
        let failure = owner
            .submit_dependency_dispatch_classified(dependency_plan(6), &mut backend)
            .unwrap_err();
        assert!(matches!(
            failure,
            NativeDependencyDispatchSubmissionFailureV1::TerminalAmbiguous {
                boundary,
                ..
            } if boundary == expected_boundary
        ));
        assert!(matches!(
            owner.submit_dependency_dispatch_classified(dependency_plan(1), &mut backend),
            Err(
                NativeDependencyDispatchSubmissionFailureV1::TerminalBeforeClaim {
                    error: NativeAqlSubmissionErrorV1::Poisoned,
                    ..
                }
            )
        ));
    }
}

#[test]
fn dependency_callback_panics_become_terminal_custody() {
    let cases = [
        (
            None,
            Some(3),
            AqlDependencyDispatchPublicationBoundaryV1::ClaimWriteIndex,
        ),
        (
            Some(FailureAfterV1::FetchAdd),
            None,
            AqlDependencyDispatchPublicationBoundaryV1::ClaimWriteIndex,
        ),
        (
            Some(FailureAfterV1::Body(0)),
            None,
            AqlDependencyDispatchPublicationBoundaryV1::BarrierBody { barrier_index: 0 },
        ),
        (
            Some(FailureAfterV1::Header(2)),
            None,
            AqlDependencyDispatchPublicationBoundaryV1::FinalDispatchHeader,
        ),
        (
            Some(FailureAfterV1::Doorbell),
            None,
            AqlDependencyDispatchPublicationBoundaryV1::Doorbell,
        ),
    ];
    for (panic_after, panic_check, expected_boundary) in cases {
        let mut owner = NativeAqlSubmissionOwnerV1::new(4_096).unwrap();
        let mut backend = FakeBackend::new(0, 0);
        backend.panic_after = panic_after;
        backend.panic_check = panic_check;
        let failure = owner
            .submit_dependency_dispatch_classified(dependency_plan(6), &mut backend)
            .unwrap_err();
        assert!(matches!(
            failure,
            NativeDependencyDispatchSubmissionFailureV1::TerminalAmbiguous {
                error: NativeAqlSubmissionErrorV1::CallbackPanic,
                boundary,
                ..
            } if boundary == expected_boundary
        ));
    }
}

#[test]
fn dependency_currentness_rejection_before_claim_is_terminal_without_ring_effect() {
    for failed_check in [1, 2] {
        let mut owner = NativeAqlSubmissionOwnerV1::new(4_096).unwrap();
        let mut backend = FakeBackend::new(0, 0);
        backend.fail_check = Some(failed_check);
        let before = backend.logical_ring().to_vec();
        assert!(matches!(
            owner.submit_dependency_dispatch_classified(dependency_plan(6), &mut backend),
            Err(
                NativeDependencyDispatchSubmissionFailureV1::TerminalBeforeClaim {
                    error: NativeAqlSubmissionErrorV1::Currentness,
                    ..
                }
            )
        ));
        assert_eq!(backend.write.load(Ordering::Relaxed), 0);
        assert!(backend.doorbells.is_empty());
        assert_eq!(backend.logical_ring(), before);
    }
}

#[test]
fn dependency_preclaim_panics_are_terminal_with_exact_plan_custody() {
    for panic_check in [1, 2] {
        let mut owner = NativeAqlSubmissionOwnerV1::new(4_096).unwrap();
        let mut backend = FakeBackend::new(0, 0);
        backend.panic_check = Some(panic_check);
        let before = backend.logical_ring().to_vec();
        let failure = owner
            .submit_dependency_dispatch_classified(dependency_plan(6), &mut backend)
            .unwrap_err();
        let NativeDependencyDispatchSubmissionFailureV1::TerminalBeforeClaim { error, prepared } =
            failure
        else {
            panic!("preclaim panic returned nonterminal custody")
        };
        assert_eq!(error, NativeAqlSubmissionErrorV1::CallbackPanic);
        assert_eq!(prepared.dependency_count(), 6);
        assert_eq!(backend.write.load(Ordering::Relaxed), 0);
        assert!(backend.doorbells.is_empty());
        assert_eq!(backend.logical_ring(), before);
    }
}

#[test]
fn barrier_full_is_retryable_but_post_reservation_failure_is_terminal() {
    let mut full = NativeAqlSubmissionOwnerV1::from_counters(4_096, 64, 0).unwrap();
    let mut full_backend = FakeBackend::new(64, 0);
    assert_eq!(
        full.submit_barrier_and(barrier(), &mut full_backend),
        Err(
            NativeBarrierAndSubmissionFailureV1::RetryableBeforeSideEffect(
                NativeAqlSubmissionErrorV1::Ring(AqlRingReservationError::Full)
            )
        )
    );
    assert_eq!(full_backend.trace, ["check", "observe", "check"]);
    assert_eq!(full_backend.write.load(Ordering::Relaxed), 64);
    assert!(full_backend.doorbells.is_empty());
    full_backend.read.store(1, Ordering::Release);
    assert_eq!(
        full.submit_barrier_and(barrier(), &mut full_backend),
        Ok(64)
    );

    let mut ambiguous = NativeAqlSubmissionOwnerV1::new(4_096).unwrap();
    let mut ambiguous_backend = FakeBackend::new(0, 0);
    ambiguous_backend.fail_after = Some(FailureAfterV1::Body(0));
    assert_eq!(
        ambiguous.submit_barrier_and(barrier(), &mut ambiguous_backend),
        Err(NativeBarrierAndSubmissionFailureV1::Terminal(
            NativeAqlSubmissionErrorV1::PacketBody
        ))
    );
    assert_eq!(ambiguous_backend.write.load(Ordering::Relaxed), 1);
    assert_eq!(
        ambiguous.submit_barrier_and(barrier(), &mut ambiguous_backend),
        Err(NativeBarrierAndSubmissionFailureV1::Terminal(
            NativeAqlSubmissionErrorV1::Poisoned
        ))
    );
}

#[test]
fn barrier_backend_occupancy_shaped_errors_are_terminal_after_reservation() {
    let stages = [
        FailureAfterV1::FetchAdd,
        FailureAfterV1::Body(0),
        FailureAfterV1::Header(0),
        FailureAfterV1::Doorbell,
    ];
    for stage in stages {
        for insufficient in [false, true] {
            let mut owner = NativeAqlSubmissionOwnerV1::new(4_096).unwrap();
            let mut backend = FakeBackend::new(0, 0);
            backend.fail_after = Some(stage);
            backend.fail_error = Some(NativeAqlSubmissionErrorV1::Ring(if insufficient {
                AqlRingReservationError::InsufficientSpace {
                    requested: 1,
                    available: 0,
                }
            } else {
                AqlRingReservationError::Full
            }));

            let result = owner.submit_barrier_and(barrier(), &mut backend);
            assert!(
                matches!(
                    &result,
                    Err(NativeBarrierAndSubmissionFailureV1::Terminal(
                        NativeAqlSubmissionErrorV1::Ring(AqlRingReservationError::Full)
                            | NativeAqlSubmissionErrorV1::Ring(
                                AqlRingReservationError::InsufficientSpace { .. }
                            )
                    ))
                ),
                "{stage:?}: {result:?}"
            );
            let trace = backend.trace.clone();
            assert_eq!(
                owner.submit_barrier_and(barrier(), &mut backend),
                Err(NativeBarrierAndSubmissionFailureV1::Terminal(
                    NativeAqlSubmissionErrorV1::Poisoned
                )),
                "{stage:?}"
            );
            assert_eq!(backend.trace, trace, "{stage:?}");
        }
    }

    for insufficient in [false, true] {
        let mut owner = NativeAqlSubmissionOwnerV1::new(4_096).unwrap();
        let mut backend = FakeBackend::new(0, 0);
        backend.fail_check = Some(3);
        backend.fail_check_error = Some(NativeAqlSubmissionErrorV1::Ring(if insufficient {
            AqlRingReservationError::InsufficientSpace {
                requested: 1,
                available: 0,
            }
        } else {
            AqlRingReservationError::Full
        }));

        let result = owner.submit_barrier_and(barrier(), &mut backend);
        assert!(matches!(
            &result,
            Err(NativeBarrierAndSubmissionFailureV1::Terminal(
                NativeAqlSubmissionErrorV1::Ring(AqlRingReservationError::Full)
                    | NativeAqlSubmissionErrorV1::Ring(
                        AqlRingReservationError::InsufficientSpace { .. }
                    )
            ))
        ));
        assert_eq!(backend.write.load(Ordering::Relaxed), 1);
        assert!(backend.doorbells.is_empty());
        let trace = backend.trace.clone();
        assert_eq!(
            owner.submit_barrier_and(barrier(), &mut backend),
            Err(NativeBarrierAndSubmissionFailureV1::Terminal(
                NativeAqlSubmissionErrorV1::Poisoned
            ))
        );
        assert_eq!(backend.trace, trace);
    }
}

#[test]
fn full_regressed_and_replayed_counters_have_no_side_effect() {
    let mut full = NativeAqlSubmissionOwnerV1::from_counters(4_096, 64, 0).unwrap();
    let mut full_backend = FakeBackend::new(64, 0);
    assert_eq!(
        full.submit(packet(), &mut full_backend),
        Err(NativeAqlSubmissionErrorV1::Ring(
            AqlRingReservationError::Full
        ))
    );
    assert_eq!(full_backend.write.load(Ordering::Relaxed), 64);
    assert!(full_backend.doorbells.is_empty());
    full_backend.read.store(1, Ordering::Release);
    assert_eq!(full.submit(packet(), &mut full_backend), Ok(64));
    assert_eq!(full_backend.doorbells, [64]);

    let mut regressed = NativeAqlSubmissionOwnerV1::from_counters(4_096, 5, 5).unwrap();
    let mut regressed_backend = FakeBackend::new(5, 4);
    assert_eq!(
        regressed.submit(packet(), &mut regressed_backend),
        Err(NativeAqlSubmissionErrorV1::Ring(
            AqlRingReservationError::ReadRegressed
        ))
    );
    assert_eq!(regressed_backend.write.load(Ordering::Relaxed), 5);
    assert!(regressed_backend.doorbells.is_empty());
    assert_eq!(
        regressed.submit(packet(), &mut regressed_backend),
        Err(NativeAqlSubmissionErrorV1::Poisoned)
    );

    let mut replay = NativeAqlSubmissionOwnerV1::new(4_096).unwrap();
    let mut replay_backend = FakeBackend::new(1, 0);
    assert_eq!(
        replay.submit(packet(), &mut replay_backend),
        Err(NativeAqlSubmissionErrorV1::WriteCounterReplay {
            expected: 0,
            observed: 1,
        })
    );
    assert_eq!(replay_backend.write.load(Ordering::Relaxed), 1);
    assert!(replay_backend.doorbells.is_empty());
    assert_eq!(
        replay.submit(packet(), &mut replay_backend),
        Err(NativeAqlSubmissionErrorV1::Poisoned)
    );
}

#[test]
fn prepublication_currentness_failure_performs_no_store() {
    for failed_check in [1, 2] {
        let mut owner = NativeAqlSubmissionOwnerV1::new(4_096).unwrap();
        let mut backend = FakeBackend::new(0, 0);
        let before = backend.logical_ring().to_vec();
        backend.fail_check = Some(failed_check);
        assert_eq!(
            owner.submit(packet(), &mut backend),
            Err(NativeAqlSubmissionErrorV1::Currentness)
        );
        assert_eq!(backend.write.load(Ordering::Relaxed), 0);
        assert!(backend.doorbells.is_empty());
        assert_eq!(
            &backend.logical_ring()[..4],
            &u32::from(AQL_INVALID_PACKET_HEADER_V1).to_le_bytes()
        );
        assert_eq!(backend.logical_ring(), before);
        assert_eq!(
            owner.submit(packet(), &mut backend),
            Err(NativeAqlSubmissionErrorV1::Poisoned)
        );
    }
}

#[test]
fn postpublication_failure_poison_is_not_retryable() {
    let mut owner = NativeAqlSubmissionOwnerV1::new(4_096).unwrap();
    let mut backend = FakeBackend::new(0, 0);
    backend.fail_check = Some(3);
    assert_eq!(
        owner.submit(packet(), &mut backend),
        Err(NativeAqlSubmissionErrorV1::Currentness)
    );
    assert_eq!(backend.write.load(Ordering::Relaxed), 1);
    assert!(backend.doorbells.is_empty());
    assert_eq!(
        owner.submit(packet(), &mut backend),
        Err(NativeAqlSubmissionErrorV1::Poisoned)
    );
}

#[test]
fn batches_of_two_four_and_sixteen_use_one_reservation_and_one_doorbell() {
    assert_successful_batch::<2>();
    assert_successful_batch::<4>();
    assert_successful_batch::<16>();
}

#[test]
fn fixed_batch_of_8192_uses_one_fetch_add_and_one_final_doorbell() {
    let mut owner = NativeAqlSubmissionOwnerV1::new(524_288).unwrap();
    let mut backend = FakeBackend::with_ring_bytes(524_288, 0, 0);
    assert_eq!(owner.submit_batch(batch::<8192>(), &mut backend), Ok(8191));
    assert_eq!(backend.write.load(Ordering::Relaxed), 8192);
    assert_eq!(backend.body_calls, 8192);
    assert_eq!(backend.header_calls, 8192);
    assert_eq!(backend.doorbells, [8191]);
    assert_eq!(
        backend
            .trace
            .iter()
            .filter(|event| **event == "fetch-add")
            .count(),
        1
    );
    assert_eq!(
        backend
            .trace
            .iter()
            .filter(|event| **event == "doorbell")
            .count(),
        1
    );
    assert!(backend.trace[4..8196].iter().all(|event| *event == "body"));
    assert!(
        backend.trace[8196..16388]
            .iter()
            .all(|event| *event == "header")
    );
}

#[test]
fn batch_wrap_uses_exact_ordered_slots_and_last_packet_doorbell() {
    let mut owner = NativeAqlSubmissionOwnerV1::from_counters(4_096, 62, 62).unwrap();
    let mut backend = FakeBackend::new(62, 62);
    assert_eq!(owner.submit_batch(batch::<4>(), &mut backend), Ok(65));
    assert_eq!(backend.write.load(Ordering::Relaxed), 66);
    assert_eq!(backend.doorbells, [65]);

    for (batch_index, slot) in [62_u32, 63, 0, 1].into_iter().enumerate() {
        assert_eq!(backend.slot_word(slot, 0), 0x0001_1402);
        assert_eq!(backend.slot_word(slot, 28), batch_index as u32);
    }
    assert_eq!(backend.slot_word(2, 0), 1);
}

#[test]
fn full_and_insufficient_batch_space_are_retryable_before_side_effects() {
    let mut full = NativeAqlSubmissionOwnerV1::from_counters(4_096, 64, 0).unwrap();
    let mut full_backend = FakeBackend::new(64, 0);
    assert_eq!(
        full.submit_batch_classified(batch::<2>(), &mut full_backend),
        Err(NativeAqlSubmissionFailureV1::RetryableBeforeSideEffect(
            NativeAqlSubmissionErrorV1::Ring(AqlRingReservationError::Full)
        ))
    );
    assert_eq!(full_backend.trace, ["check", "observe", "check"]);
    assert_eq!(full_backend.write.load(Ordering::Relaxed), 64);
    assert!(full_backend.doorbells.is_empty());
    full_backend.read.store(2, Ordering::Release);
    assert_eq!(
        full.submit_batch_classified(batch::<2>(), &mut full_backend),
        Ok(65)
    );

    let mut insufficient = NativeAqlSubmissionOwnerV1::from_counters(4_096, 63, 0).unwrap();
    let mut insufficient_backend = FakeBackend::new(63, 0);
    assert_eq!(
        insufficient.submit_batch_classified(batch::<2>(), &mut insufficient_backend),
        Err(NativeAqlSubmissionFailureV1::RetryableBeforeSideEffect(
            NativeAqlSubmissionErrorV1::Ring(AqlRingReservationError::InsufficientSpace {
                requested: 2,
                available: 1,
            })
        ))
    );
    assert_eq!(insufficient_backend.trace, ["check", "observe", "check"]);
    assert_eq!(insufficient_backend.write.load(Ordering::Relaxed), 63);
    assert!(insufficient_backend.doorbells.is_empty());
    insufficient_backend.read.store(2, Ordering::Release);
    assert_eq!(
        insufficient.submit_batch_classified(batch::<2>(), &mut insufficient_backend),
        Ok(64)
    );
}

#[test]
fn occupancy_shaped_backend_errors_are_terminal_at_every_checkpoint() {
    for failed_check in [1, 2, 3] {
        for insufficient in [false, true] {
            let mut owner = NativeAqlSubmissionOwnerV1::new(4_096).unwrap();
            let mut backend = FakeBackend::new(0, 0);
            backend.fail_check = Some(failed_check);
            backend.fail_check_error = Some(NativeAqlSubmissionErrorV1::Ring(if insufficient {
                AqlRingReservationError::InsufficientSpace {
                    requested: 1,
                    available: 0,
                }
            } else {
                AqlRingReservationError::Full
            }));

            let result = owner.submit_batch_classified(batch::<1>(), &mut backend);
            assert!(matches!(
                result,
                Err(NativeAqlSubmissionFailureV1::Terminal(
                    NativeAqlSubmissionErrorV1::Ring(AqlRingReservationError::Full)
                        | NativeAqlSubmissionErrorV1::Ring(
                            AqlRingReservationError::InsufficientSpace { .. }
                        )
                ))
            ));
            let trace = backend.trace.clone();
            assert_eq!(
                owner.submit_batch_classified(batch::<1>(), &mut backend),
                Err(NativeAqlSubmissionFailureV1::Terminal(
                    NativeAqlSubmissionErrorV1::Poisoned
                ))
            );
            assert_eq!(backend.trace, trace);
        }
    }
}

#[test]
fn write_counter_divergence_after_reservation_is_terminal() {
    let mut owner = NativeAqlSubmissionOwnerV1::new(4_096).unwrap();
    let mut backend = FakeBackend::new(0, 0);
    backend.fetch_return_override = Some(9);
    assert_eq!(
        owner.submit_batch(batch::<4>(), &mut backend),
        Err(NativeAqlSubmissionErrorV1::WriteCounterRace {
            expected: 0,
            observed: 9,
        })
    );
    assert_eq!(backend.write.load(Ordering::Relaxed), 4);
    assert_eq!(backend.trace, ["check", "observe", "check", "fetch-add"]);
    assert!(backend.doorbells.is_empty());
    let trace = backend.trace.clone();
    assert_eq!(
        owner.submit_batch(batch::<4>(), &mut backend),
        Err(NativeAqlSubmissionErrorV1::Poisoned)
    );
    assert_eq!(backend.trace, trace);
}

#[test]
fn every_batch_side_effect_failure_is_terminal_without_cleanup_or_retry() {
    let mut cases = vec![(
        FailureAfterV1::FetchAdd,
        NativeAqlSubmissionErrorV1::Currentness,
    )];
    for index in 0..4 {
        cases.push((
            FailureAfterV1::Body(index),
            NativeAqlSubmissionErrorV1::PacketBody,
        ));
    }
    for index in 0..4 {
        cases.push((
            FailureAfterV1::Header(index),
            NativeAqlSubmissionErrorV1::PacketHeader,
        ));
    }
    cases.push((
        FailureAfterV1::Doorbell,
        NativeAqlSubmissionErrorV1::Doorbell,
    ));

    for (failure, expected) in cases {
        let mut owner = NativeAqlSubmissionOwnerV1::new(4_096).unwrap();
        let mut backend = FakeBackend::new(0, 0);
        backend.fail_after = Some(failure);
        assert_eq!(
            owner.submit_batch_classified(batch::<4>(), &mut backend),
            Err(NativeAqlSubmissionFailureV1::Terminal(expected))
        );
        assert_eq!(backend.write.load(Ordering::Relaxed), 4, "{failure:?}");
        assert!(backend.doorbells.len() <= 1, "{failure:?}");
        if failure == FailureAfterV1::Doorbell {
            assert_eq!(backend.doorbells, [3]);
        } else {
            assert!(backend.doorbells.is_empty(), "{failure:?}");
        }

        let trace = backend.trace.clone();
        assert_eq!(
            owner.submit_batch_classified(batch::<4>(), &mut backend),
            Err(NativeAqlSubmissionFailureV1::Terminal(
                NativeAqlSubmissionErrorV1::Poisoned
            )),
            "{failure:?}"
        );
        assert_eq!(backend.trace, trace, "{failure:?}");
        {
            let _terminal_owner = owner;
        }
        assert_eq!(backend.trace, trace, "Drop must not clean up {failure:?}");
    }
}

#[test]
fn occupancy_shaped_batch_side_effect_errors_are_terminal_and_sticky() {
    for failure in [
        FailureAfterV1::FetchAdd,
        FailureAfterV1::Body(0),
        FailureAfterV1::Header(0),
        FailureAfterV1::Doorbell,
    ] {
        for insufficient in [false, true] {
            let mut owner = NativeAqlSubmissionOwnerV1::new(4_096).unwrap();
            let mut backend = FakeBackend::new(0, 0);
            backend.fail_after = Some(failure);
            backend.fail_error = Some(NativeAqlSubmissionErrorV1::Ring(if insufficient {
                AqlRingReservationError::InsufficientSpace {
                    requested: 1,
                    available: 0,
                }
            } else {
                AqlRingReservationError::Full
            }));

            let result = owner.submit_batch_classified(batch::<1>(), &mut backend);
            assert!(matches!(
                result,
                Err(NativeAqlSubmissionFailureV1::Terminal(
                    NativeAqlSubmissionErrorV1::Ring(AqlRingReservationError::Full)
                        | NativeAqlSubmissionErrorV1::Ring(
                            AqlRingReservationError::InsufficientSpace { .. }
                        )
                ))
            ));
            let trace = backend.trace.clone();
            assert_eq!(
                owner.submit_batch_classified(batch::<1>(), &mut backend),
                Err(NativeAqlSubmissionFailureV1::Terminal(
                    NativeAqlSubmissionErrorV1::Poisoned
                ))
            );
            assert_eq!(backend.trace, trace);
        }
    }
}

#[test]
fn cwsr_headers_match_pinned_rocr_layout() {
    let mut bytes = vec![0xff; GFX942_CWSR_TOTAL_BYTES_V1];
    let payload = KfdQueueExceptionPayloadAddressV1::new(0x1000).unwrap();
    let event = KfdSignalEventIdV1::new(7).unwrap();
    initialize_gfx942_cwsr_headers(&mut bytes, payload, event).unwrap();
    for xcc in 0..GFX942_CWSR_XCC_COUNT_V1 {
        let offset = xcc * GFX942_CWSR_CONTEXT_BYTES_PER_XCC_V1;
        let header = &bytes[offset..offset + CWSR_HEADER_BYTES];
        assert_eq!(&header[..16], &[0; 16]);
        assert_eq!(
            u32::from_le_bytes(header[16..20].try_into().unwrap()),
            ((GFX942_CWSR_XCC_COUNT_V1 - xcc) * GFX942_CWSR_CONTEXT_BYTES_PER_XCC_V1) as u32
        );
        assert_eq!(
            u32::from_le_bytes(header[20..24].try_into().unwrap()),
            GFX942_CWSR_DEBUG_BYTES_TOTAL_V1
        );
        assert_eq!(
            u64::from_le_bytes(header[24..32].try_into().unwrap()),
            0x1000
        );
        assert_eq!(u32::from_le_bytes(header[32..36].try_into().unwrap()), 7);
        assert_eq!(&header[36..40], &[0; 4]);
    }
    assert!(
        bytes[CWSR_HEADER_BYTES..GFX942_CWSR_CONTEXT_BYTES_PER_XCC_V1]
            .iter()
            .all(|byte| *byte == 0xff)
    );
    assert!(initialize_gfx942_cwsr_headers(&mut [0; 4096], payload, event).is_err());
    assert!(gfx942_cwsr_header_bytes(8, payload, event).is_err());
}

fn assert_successful_batch<const N: usize>() {
    let mut owner = NativeAqlSubmissionOwnerV1::new(4_096).unwrap();
    let mut backend = FakeBackend::new(0, 0);
    assert_eq!(
        owner.submit_batch(batch::<N>(), &mut backend),
        Ok(N as u64 - 1)
    );
    assert_eq!(backend.write.load(Ordering::Relaxed), N as u64);
    assert_eq!(backend.doorbells, [N as u64 - 1]);
    assert_eq!(
        backend.trace[..4],
        ["check", "observe", "check", "fetch-add"]
    );
    assert!(backend.trace[4..4 + N].iter().all(|event| *event == "body"));
    assert!(
        backend.trace[4 + N..4 + 2 * N]
            .iter()
            .all(|event| *event == "header")
    );
    assert_eq!(backend.trace[4 + 2 * N..], ["check", "doorbell"]);
    assert_eq!(
        backend
            .trace
            .iter()
            .filter(|event| **event == "fetch-add")
            .count(),
        1
    );
    assert_eq!(
        backend
            .trace
            .iter()
            .filter(|event| **event == "doorbell")
            .count(),
        1
    );
    for index in 0..N {
        assert_eq!(backend.slot_word(index as u32, 0), 0x0001_1402);
        assert_eq!(backend.slot_word(index as u32, 28), index as u32);
    }
}
