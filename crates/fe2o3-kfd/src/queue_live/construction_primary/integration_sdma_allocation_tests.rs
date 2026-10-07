//! The fresh allocation driver runs on the original constructed primary and SDMA owners.

use super::*;
#[path = "integration_sdma_promotion_tests.rs"]
mod promotion;
#[path = "integration_sdma_public_allocation_tests.rs"]
mod public_allocation;
#[path = "integration_sdma_recycle_tests.rs"]
mod recycle;
use crate::queue::live::sdma_allocation::{
    Gfx942SdmaAllocationDispositionV1, Gfx942SdmaAllocationFailureV1, SdmaAllocationContextV1,
    SdmaAllocationCustodyV1, SdmaAllocationMemoryV1, SdmaAllocationPartsV1,
    SdmaAllocationRequestV1, allocate_classified_in_place, allocate_in_place,
};
use crate::shared_memory::{
    CoherentAllocationCustodyV1, CoherentInsertionFaultV1, CoherentInsertionPrefixV1,
    CoherentPreparationTraceV1, DataCleanupCustodyV1, DeviceAllocationCustodyV1,
    DeviceInsertionPrefixV1, DispatchDataReleaseV1, device_memory_layout,
};

thread_local! {
    static HOST_TRACE: RefCell<CoherentPreparationTraceV1> = RefCell::default();
}

impl SdmaAllocationMemoryV1 for Memory {
    fn prepare_host(
        &mut self,
        root: &mut CoherentAllocationCustodyV1,
        bytes: usize,
    ) -> Result<(), MemorySessionError> {
        HOST_TRACE.with(|trace| {
            self.primary_prepare_coherent_allocation_v1(
                root,
                bytes,
                CoherentInsertionFaultV1::None,
                &mut trace.borrow_mut(),
            )
        })
    }
    fn prepare_device(
        &mut self,
        root: &mut DeviceAllocationCustodyV1,
        bytes: u64,
        alignment: u64,
    ) -> Result<(), MemorySessionError> {
        self.primary_prepare_device_allocation_v1(root, bytes, alignment)
    }
    fn is_quarantined(&self) -> bool {
        self.primary_is_quarantined_v1()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Fault {
    None,
    Error,
    Panic,
    AfterError,
    AfterPanic,
    Regression,
}

struct AllocationParent {
    parent: Parent,
    custody: Option<SdmaAllocationCustodyV1>,
    outstanding: usize,
    opening: Fault,
    closing: Fault,
    capacity_retry_settled: bool,
    poison_panics: bool,
    calls: Vec<&'static str>,
}

impl Drop for AllocationParent {
    fn drop(&mut self) {
        if let Some(sdma) = &mut self.parent.sdma {
            // Release only fixture-owned anonymous mappings, not native queue resources.
            crate::sdma::retained_release::fixture::cleanup_set(sdma);
        }
    }
}

impl AllocationParent {
    fn new() -> Self {
        HOST_TRACE.with(|trace| *trace.borrow_mut() = CoherentPreparationTraceV1::default());
        Self {
            parent: sdma_cases::with_sdma(false).0,
            custody: None,
            outstanding: 0,
            opening: Fault::None,
            closing: Fault::None,
            capacity_retry_settled: true,
            poison_panics: false,
            calls: Vec::new(),
        }
    }
    fn allocate(
        &mut self,
        host: bool,
        bytes: usize,
    ) -> Result<Gfx942SdmaBufferV1, ComputeAqlQueueSessionErrorV1> {
        allocate_in_place(
            self,
            if host {
                SdmaAllocationRequestV1::Host(bytes)
            } else {
                SdmaAllocationRequestV1::Device {
                    bytes: bytes as u64,
                    alignment: 4096,
                }
            },
        )
    }
    fn no_retry(&mut self) {
        let e = &self.parent.engine;
        let before = e.backend.session.data_release_snapshot_v1(&e.foundation);
        let host_before = e.backend.session.coherent_insertion_snapshot_v1();
        let device_before = e.backend.session.insertion_memory_snapshot_v1();
        let loan = e.backend.session.primary_loan_state_v1(&e.foundation);
        let address = self.custody.as_ref().map(|r| r as *const _ as usize);
        let calls = self.calls.clone();
        let outstanding = self.outstanding;
        for host in [false, true] {
            assert!(matches!(
                self.allocate(host, 17),
                Err(ComputeAqlQueueSessionErrorV1::Contract(
                    "unfinished SDMA allocation"
                ))
            ));
        }
        let e = &self.parent.engine;
        assert!(
            e.backend.session.data_release_snapshot_v1(&e.foundation) == before,
            "terminal retry changed native/model state"
        );
        assert!(
            e.backend.session.coherent_insertion_snapshot_v1() == host_before,
            "terminal retry changed host allocation state"
        );
        assert!(
            e.backend.session.insertion_memory_snapshot_v1() == device_before,
            "terminal retry changed device allocation state"
        );
        assert_eq!(e.backend.session.primary_loan_state_v1(&e.foundation), loan);
        assert_eq!(
            self.custody.as_ref().map(|r| r as *const _ as usize),
            address
        );
        assert_eq!(self.calls, calls);
        assert_eq!(self.outstanding, outstanding);
    }
    fn release(&mut self, buffer: Gfx942SdmaBufferV1) {
        let mut root = DataCleanupCustodyV1::from_sdma(buffer);
        let loan = self.loan().unwrap();
        self.parent
            .engine
            .backend
            .session
            .release_data(&mut root)
            .unwrap();
        self.retake(loan).unwrap();
        assert!(root.is_complete());
        self.outstanding -= 1;
    }
    fn shutdown(mut self) {
        assert!(self.custody.is_none() && self.outstanding == 0);
        let mut root = PrimaryReleaseStateV1::<Fixture>::new();
        root.release_in_place(&mut self.parent).unwrap();
        self.parent
            .engine
            .backend
            .session
            .primary_assert_all_released_v1();
        root.sdma.as_mut().unwrap().cleanup_local_mappings();
    }
}

impl SdmaAllocationContextV1 for AllocationParent {
    type Memory = Memory;
    fn preflight(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if self.custody.is_some() {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "unfinished SDMA allocation",
            ));
        }
        if self.parent.poisoned || self.parent.sdma.is_none() {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "unavailable SDMA fixture",
            ));
        }
        Ok(())
    }
    fn parts(
        &mut self,
    ) -> Result<SdmaAllocationPartsV1<'_, Memory>, ComputeAqlQueueSessionErrorV1> {
        Ok(SdmaAllocationPartsV1 {
            memory: &mut self.parent.engine.backend.session,
            custody: &mut self.custody,
            outstanding: &mut self.outstanding,
            owner: self.parent.key,
        })
    }
    fn loan(&mut self) -> Result<LiveQueueModelFoundationLoanV1, ComputeAqlQueueSessionErrorV1> {
        self.calls.push("loan");
        match self.opening {
            Fault::Error => return Err(ComputeAqlQueueSessionErrorV1::Contract("allocation loan")),
            Fault::Panic => std::panic::panic_any("allocation loan"),
            _ => (),
        }
        let e = &mut self.parent.engine;
        assert!(e.backend.foundation_in_engine);
        let loan = e.backend.session.primary_loan(&mut e.foundation)?;
        e.backend.foundation_in_engine = false;
        Ok(loan)
    }
    fn retake(
        &mut self,
        loan: LiveQueueModelFoundationLoanV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.calls.push("retake");
        match self.closing {
            Fault::Error => {
                return Err(ComputeAqlQueueSessionErrorV1::Contract("allocation retake"));
            }
            Fault::Panic => std::panic::panic_any("allocation retake"),
            Fault::Regression => self
                .parent
                .engine
                .backend
                .session
                .primary_regress_loan_revision_v1(&loan),
            _ => (),
        }
        let e = &mut self.parent.engine;
        assert!(!e.backend.foundation_in_engine);
        e.backend.session.primary_reclaim(&mut e.foundation, loan)?;
        e.backend.foundation_in_engine = true;
        match self.closing {
            Fault::AfterError => Err(ComputeAqlQueueSessionErrorV1::Contract(
                "allocation retake after",
            )),
            Fault::AfterPanic => std::panic::panic_any("allocation retake after"),
            _ => Ok(()),
        }
    }
    fn is_terminal(&self) -> bool {
        self.parent.poisoned
    }
    fn capacity_retry_is_settled(&self) -> bool {
        self.capacity_retry_settled
            && !self.parent.poisoned
            && self.custody.is_none()
            && self.parent.sdma.is_some()
            && self.parent.engine.backend.foundation_in_engine
            && !self
                .parent
                .engine
                .backend
                .session
                .primary_is_quarantined_v1()
    }
    fn poison(&mut self) {
        self.parent.poison_release();
        if self.poison_panics {
            std::panic::panic_any("allocation poison");
        }
    }
}

fn device_layout(bytes: usize) -> crate::Gfx942DeviceMemoryLayoutV1 {
    device_memory_layout(
        (bytes as u64).next_multiple_of(4096),
        4096,
        fe2o3_kfd_uapi::KfdAllocMemoryFlags::DEVICE_LOCAL,
    )
    .unwrap()
}

fn classified_allocate(
    parent: &mut AllocationParent,
    host: bool,
    bytes: usize,
) -> Result<Gfx942SdmaBufferV1, Gfx942SdmaAllocationFailureV1> {
    allocate_classified_in_place(
        parent,
        if host {
            SdmaAllocationRequestV1::Host(bytes)
        } else {
            SdmaAllocationRequestV1::Device {
                bytes: bytes as u64,
                alignment: 4096,
            }
        },
    )
}

#[derive(Clone, Copy, Debug)]
enum LowerFault {
    Native(&'static str, bool),
    Currentness(usize, bool),
    Map(u32, bool),
}

fn lower_failure(host: bool, fault: LowerFault) {
    let mut f = AllocationParent::new();
    let e = &f.parent.engine;
    let host_before = e.backend.session.coherent_insertion_snapshot_v1();
    let device_before = e.backend.session.insertion_memory_snapshot_v1();
    let accounting = e.backend.session.observation();
    let loan = e.backend.session.primary_loan_state_v1(&e.foundation);
    let resources = original_resource_ids(&f.parent);
    let signal = Memory::primary_token_identity(f.parent.signals.as_ref().unwrap());
    let memory = &mut f.parent.engine.backend.session;
    match fault {
        LowerFault::Native(op, panic) => memory.primary_arm_native(op, panic),
        LowerFault::Currentness(check, panic) => memory.insertion_arm_currentness_v1(check, panic),
        LowerFault::Map(prefix, errno) => memory.insertion_arm_map_v1(prefix, errno),
    }
    let result = catch_unwind(AssertUnwindSafe(|| f.allocate(host, 17)));
    match fault {
        LowerFault::Native(op, true) => assert_eq!(
            result.unwrap_err().downcast_ref::<(&str, &str)>(),
            Some(&("N2 native panic", op))
        ),
        LowerFault::Currentness(_, true) => assert_eq!(
            result.unwrap_err().downcast_ref::<(&str, &str)>(),
            Some(&("N2 native panic", "currentness"))
        ),
        LowerFault::Native(op, false) => assert!(
            matches!(result.unwrap(), Err(ComputeAqlQueueSessionErrorV1::Sdma(crate::sdma::Gfx942SdmaErrorV1::Memory(MemorySessionError::Injected(actual)))) if actual == op)
        ),
        LowerFault::Currentness(_, false) => assert!(matches!(
            result.unwrap(),
            Err(ComputeAqlQueueSessionErrorV1::Sdma(
                crate::sdma::Gfx942SdmaErrorV1::Memory(MemorySessionError::Injected("currentness"))
            ))
        )),
        LowerFault::Map(n, errno) => {
            let Err(ComputeAqlQueueSessionErrorV1::Sdma(crate::sdma::Gfx942SdmaErrorV1::Memory(
                error,
            ))) = result.unwrap()
            else {
                panic!("expected SDMA memory error");
            };
            if errno && n <= 1 {
                assert!(matches!(error, MemorySessionError::Injected("map_gpu")));
            } else {
                let expected = match (host, n) {
                    (true, 2) => "shared MAP_MEMORY_TO_GPU cumulative n_success",
                    (true, _) => "shared MAP_MEMORY_TO_GPU full prefix",
                    (false, 2) => "device-memory MAP_MEMORY_TO_GPU cumulative n_success",
                    (false, _) => "device-memory MAP_MEMORY_TO_GPU full prefix",
                };
                assert!(
                    matches!(error, MemorySessionError::KernelResultMalformed(actual) if actual == expected)
                );
            }
        }
    }
    assert!(f.parent.poisoned && f.custody.is_some(), "{fault:?}");
    assert_eq!(f.outstanding, 0);
    assert_eq!(f.calls, ["loan", "retake"]);
    let e = &f.parent.engine;
    let memory = &e.backend.session;
    assert!(memory.primary_is_quarantined_v1());
    assert!(e.backend.foundation_in_engine);
    assert_eq!(
        memory.primary_loan_state_v1(&e.foundation),
        (loan.0, None, loan.2 + 1)
    );
    assert_eq!(original_resource_ids(&f.parent), resources);
    assert_eq!(
        Memory::primary_token_identity(f.parent.signals.as_ref().unwrap()),
        signal
    );
    memory.assert_original_records_unchanged(&accounting);
    memory.primary_assert_accounts_and_records(memory.primary_session_id());
    if host {
        let (calls, pending, mapped, progress, operations) = match fault {
            LowerFault::Currentness(1, _) => ([1, 0, 0, 0, 0], None, false, None, vec![]),
            LowerFault::Currentness(2, _) => (
                [2, 1, 1, 0, 0],
                Some(("CheckAllocation", true, true, None)),
                false,
                None,
                vec![],
            ),
            LowerFault::Currentness(3, _) => (
                [3, 1, 1, 1, 0],
                Some(("CheckMapping", true, true, Some(true))),
                false,
                None,
                vec!["map_cpu", "prepare_cpu_mapping"],
            ),
            LowerFault::Currentness(n, _) => (
                [n, 1, 1, 1, usize::from(n == 5)],
                None,
                true,
                Some(if n == 5 {
                    (true, Some(true), Some(1))
                } else {
                    (false, None, None)
                }),
                if n == 5 {
                    vec!["map_cpu", "prepare_cpu_mapping", "map_gpu"]
                } else {
                    vec!["map_cpu", "prepare_cpu_mapping"]
                },
            ),
            LowerFault::Native("reserve_va", _) => (
                [1, 1, 0, 0, 0],
                Some(("ReserveVa", false, false, None)),
                false,
                None,
                vec![],
            ),
            LowerFault::Native("alloc", panic) => (
                [1, 1, 1, 0, 0],
                Some(("Allocate", true, !panic, None)),
                false,
                None,
                vec![],
            ),
            LowerFault::Native("map_cpu", _) => (
                [2, 1, 1, 1, 0],
                Some(("MapCpu", true, true, None)),
                false,
                None,
                vec!["map_cpu"],
            ),
            LowerFault::Native("prepare_cpu_mapping", _) => (
                [2, 1, 1, 1, 0],
                Some(("PrepareCpuMapping", true, true, Some(false))),
                false,
                None,
                vec!["map_cpu", "prepare_cpu_mapping"],
            ),
            LowerFault::Native("map_gpu", panic) => (
                [4, 1, 1, 1, 1],
                None,
                true,
                Some(if panic {
                    (true, None, None)
                } else {
                    (true, Some(false), Some(1))
                }),
                vec!["map_cpu", "prepare_cpu_mapping", "map_gpu"],
            ),
            LowerFault::Map(n, errno) => (
                [4, 1, 1, 1, 1],
                None,
                true,
                Some((true, Some(!errno), Some(n))),
                vec!["map_cpu", "prepare_cpu_mapping", "map_gpu"],
            ),
            _ => unreachable!(),
        };
        memory.coherent_assert_prefix_for_length_v1(
            &host_before,
            17,
            None,
            CoherentInsertionPrefixV1 {
                calls,
                operations,
                copied: 0,
                record_phase: mapped.then_some("CpuWritable"),
                pending,
            },
        );
        let SdmaAllocationCustodyV1::Host { allocation, .. } = f.custody.as_ref().unwrap() else {
            unreachable!()
        };
        assert!(allocation.completed().is_err());
        HOST_TRACE.with(|t| {
            let mut t = t.borrow_mut();
            assert!(t.source.is_none());
            if let Some(progress) = progress {
                let token = t.allocated.as_ref().unwrap();
                token.assert_id(host_before.next_id);
                memory.coherent_assert_terminal_v1(token, "Map", false, progress);
            }
            memory.coherent_assert_model_for_length_v1(&e.foundation, &mut t, 17, u8::from(mapped));
        });
    } else {
        let (calls, phase, handle, progress) = match fault {
            LowerFault::Currentness(n, _) => (
                [
                    n,
                    usize::from(n > 1),
                    usize::from(n > 1),
                    0,
                    usize::from(n == 4),
                ],
                match n {
                    1 => None,
                    3 => Some("Unmapped"),
                    _ => Some("Ambiguous"),
                },
                n > 1,
                (n == 4, (n == 4).then_some(true), (n == 4).then_some(1)),
            ),
            LowerFault::Native(op, panic) => (
                [
                    if op == "map_gpu" { 3 } else { 1 },
                    1,
                    usize::from(op != "reserve_va"),
                    0,
                    usize::from(op == "map_gpu"),
                ],
                (op != "reserve_va").then_some("Ambiguous"),
                !(op == "reserve_va" || op == "alloc" && panic),
                (
                    op == "map_gpu",
                    (op == "map_gpu" && !panic).then_some(false),
                    (op == "map_gpu" && !panic).then_some(1),
                ),
            ),
            LowerFault::Map(n, errno) => (
                [3, 1, 1, 0, 1],
                Some("Ambiguous"),
                true,
                (true, Some(!errno), Some(n)),
            ),
        };
        memory.insertion_assert_native_prefix_with_layout_v1(
            &device_before,
            DeviceInsertionPrefixV1 {
                calls,
                phase,
                handle,
                cpu_writable: None,
                written: false,
                operations: if calls[4] == 0 { &[] } else { &["map_gpu"] },
            },
            device_layout(17),
            None,
        );
        let SdmaAllocationCustodyV1::Device { allocation, .. } = f.custody.as_ref().unwrap() else {
            unreachable!()
        };
        let state = allocation.insertion_snapshot_for_test();
        assert!(state.started() && state.failed());
        assert_eq!(state.progress(), progress);
        assert!(allocation.completed().is_err());
        memory.insertion_assert_allocation_partition_v1(
            &[],
            &[],
            Some(allocation),
            device_before.next_id,
            device_layout(17),
            &[],
        );
        assert!(!memory.insertion_memory_snapshot_v1().terminal_occupied);
    }
    f.no_retry();
}

#[path = "integration_sdma_allocation_tests/allocation_tests.rs"]
mod allocation_tests;
