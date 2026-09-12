//! The coherent path reuses the constructed primary, auxiliary and original engine.

use super::*;
use crate::shared_memory::{
    CoherentInitializationCustodyV1, CoherentInsertionFaultV1 as Fault,
    CoherentInsertionPrefixV1 as Prefix, CoherentPreparationTraceV1, CoherentTokenSnapshotV1,
    PrimaryProjectionCaseV1,
};

const SOURCE: [u8; 17] = [0x5a; 17];
const OPERATIONS: [&str; 3] = ["map_cpu", "prepare_cpu_mapping", "map_gpu"];

#[derive(Default)]
struct CoherentTrace {
    memory: CoherentPreparationTraceV1,
    fault: Fault,
    before_retake: Option<CoherentTokenSnapshotV1>,
    before_failure: Option<CoherentTokenSnapshotV1>,
}

struct CoherentContext<'a> {
    base: Context<'a>,
    trace: &'a mut CoherentTrace,
}

impl DataInsertionContextV1<CoherentInitializationCustodyV1, &[u8]> for CoherentContext<'_> {
    fn require_unbound(&self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.base.require_unbound()
    }
    fn ledger(&mut self) -> DetachedInsertionLedgerV1<'_> {
        self.base.ledger()
    }
    fn reserve(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.base.trace.reserve_calls += 1;
        outcome("insertion-reserve", self.base.trace.reserve)?;
        let ledger = self.base.ledger();
        reserve_identity_capacity_v1(
            ledger.identities,
            "detached initialized-coherent identity ledger",
        )?;
        self.base.trace.reserved_storage = Some((
            ledger.identities.as_ptr() as usize,
            ledger.identities.capacity(),
        ));
        Ok(())
    }
    fn prepare(
        &mut self,
        root: &mut CoherentInitializationCustodyV1,
        source: &[u8],
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.base.trace.prepare_calls += 1;
        let before = self.base.ledger_snapshot();
        let ledger = self.base.ledger();
        assert!(
            ledger.identities.capacity() >= 16,
            "coherent identity capacity reserved before effects"
        );
        let storage = (ledger.identities.as_ptr(), ledger.identities.capacity());
        let skip = self.base.trace.skip_prepare;
        let trace = &mut self.trace;
        let result = self.base.parent.with_preparation_custody(|memory| {
            if skip {
                return Ok(());
            }
            let result = catch_unwind(AssertUnwindSafe(|| {
                memory.primary_prepare_coherent_initialization_v1(
                    root,
                    source,
                    trace.fault,
                    &mut trace.memory,
                )
            }));
            trace.before_retake = root.coherent_snapshot_for_test();
            match result {
                Ok(result) => result.map_err(Into::into),
                Err(payload) => std::panic::resume_unwind(payload),
            }
        });
        assert_eq!(
            self.base.ledger_snapshot(),
            before,
            "coherent metadata cannot commit before retake"
        );
        let ledger = self.base.ledger();
        assert_eq!(
            (ledger.identities.as_ptr(), ledger.identities.capacity()),
            storage
        );
        let (operation, retake) = result?;
        retake?;
        operation?;
        self.base.trace.commit_ready = true;
        Ok(())
    }
    fn fail(&mut self, root: CoherentInitializationCustodyV1, panicked: bool) {
        self.base.trace.fail_calls += 1;
        self.base.trace.panic_fail |= panicked;
        self.trace.before_failure = root.coherent_snapshot_for_test();
        if let Some(lane) = self.base.lane.as_mut() {
            lane.completion_owner.poison_owner();
            lane.submission.as_mut().unwrap().poison();
            lane.unpublished_dispatch.continuation = None;
        } else {
            self.base.primary.continuation = None;
        }
        self.base.parent.poison();
        if panicked {
            Fixture::poison();
        }
        self.base
            .parent
            .original
            .as_mut()
            .unwrap()
            .primary
            .completed
            .as_mut()
            .unwrap()
            .engine
            .backend
            .session
            .primary_retain_coherent_initialization_v1(root);
    }
}

struct CoherentFixture {
    base: InsertionFixture,
    trace: CoherentTrace,
}

impl CoherentFixture {
    fn assert_model(&mut self, committed: u8) {
        let engine = &self.base.scope.primary.completed.as_ref().unwrap().engine;
        engine.backend.session.coherent_assert_model_v1(
            &engine.foundation,
            &mut self.trace.memory,
            &SOURCE,
            committed,
        );
    }
    fn new(ordinal: usize) -> Self {
        let mut result = Self {
            base: InsertionFixture::new(ordinal),
            trace: CoherentTrace::default(),
        };
        let engine = &result.base.scope.primary.completed.as_ref().unwrap().engine;
        engine
            .backend
            .session
            .coherent_expect_unchanged_model_v1(&engine.foundation, &mut result.trace.memory);
        result
    }
    fn insert(&mut self, index: Option<usize>, source: &[u8]) -> SettledDataInsertionV1 {
        settle_data_insertion_v1(
            &mut CoherentContext {
                base: self.base.context(),
                trace: &mut self.trace,
            },
            CoherentInitializationCustodyV1::new(),
            index.map_or(
                DataInsertionIndexV1::RequiredHole,
                DataInsertionIndexV1::Explicit,
            ),
            source,
        )
    }
    fn assert_retry(&mut self) {
        assert!(self.base.scope.parent.poisoned);
        assert!(
            self.base
                .scope
                .primary
                .completed
                .as_ref()
                .unwrap()
                .completion_owner
                .is_poisoned_for_test()
        );
        if let Some(lane) = &self.base.lane {
            assert!(lane.completion_owner.is_poisoned_for_test());
            assert!(lane.submission.as_ref().unwrap().is_poisoned_for_test());
        }
        self.base.memory_mut().insertion_clear_faults_v1();
        self.base.scope.parent.faults = Faults::default();
        let memory = self.base.memory().coherent_insertion_snapshot_v1();
        let accounting = self.base.memory().observation();
        let ledger = self.base.context().ledger_snapshot();
        let calls = (
            self.base.trace.reserve_calls,
            self.base.trace.prepare_calls,
            self.base.trace.fail_calls,
        );
        let retry = self.insert(Some(0), &SOURCE);
        assert!(!retry.transport);
        assert!(matches!(
            retry.result,
            Ok(Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                Gfx942DispatchBindingErrorV1::Poisoned
            )))
        ));
        assert_eq!(self.base.memory().coherent_insertion_snapshot_v1(), memory);
        assert_eq!(self.base.memory().observation(), accounting);
        assert_eq!(self.base.context().ledger_snapshot(), ledger);
        assert_eq!(
            (
                self.base.trace.reserve_calls,
                self.base.trace.prepare_calls,
                self.base.trace.fail_calls
            ),
            calls
        );
    }
    fn finish(
        &mut self,
        transport: bool,
        marker: Option<crate::shared_memory::SharedGttAllocationIdentityV1>,
    ) {
        let native = self.base.memory().coherent_insertion_snapshot_v1();
        self.base.restore_and_transport(transport);
        assert_eq!(self.base.memory().coherent_insertion_snapshot_v1(), native);
        let engine = &self.base.scope.primary.completed.as_ref().unwrap().engine;
        self.base
            .memory()
            .coherent_assert_model_retained_v1(&engine.foundation, &self.trace.memory);
        let mut refs = PreparationOwnerRefsV1::default();
        if let Some(dispatch) = &self.base.scope.primary.completed.as_ref().unwrap().dispatch {
            refs.dispatch(dispatch);
        }
        let mut owners = Vec::new();
        for slot in &self.base.scope.lanes {
            if let Some(lane) = &slot.state {
                if let Some(dispatch) = &lane.dispatch {
                    refs.dispatch(dispatch);
                }
                owners.push(Memory::primary_token_identity(
                    lane.completion_signals.as_ref().unwrap(),
                ));
            }
        }
        refs.data(&self.base.data);
        owners.extend(refs.shared.iter().map(|(id, _)| *id));
        assert_partition_with_markers(
            &self.base.scope.primary,
            owners,
            marker.into_iter().collect(),
        );
        let devices = self.base.memory().insertion_memory_snapshot_v1();
        self.base.assert_partition(devices.next_id, SOURCE.len());
    }
    fn assert_account_delta(
        &self,
        before: &crate::shared_memory::PreparationMemoryObservationV1,
        admitted: bool,
        pending: bool,
    ) {
        self.base.memory().assert_original_records_unchanged(before);
        let after = self.base.memory().observation();
        assert_eq!(after.device, before.device);
        let before = before.host.unwrap();
        let after = after.host.unwrap();
        assert_eq!(
            after.used_backing_bytes,
            before.used_backing_bytes + if admitted { 4096 } else { 0 }
        );
        assert_eq!(
            after.used_allocation_records,
            before.used_allocation_records + u64::from(admitted)
        );
        assert_eq!(
            after.quarantined_records,
            before.quarantined_records + usize::from(pending)
        );
        assert_eq!(
            after.retained_records,
            before.retained_records + usize::from(admitted && !pending)
        );
        assert_eq!(after.reserved_records, 0);
    }
}

fn success_prefix() -> Prefix {
    Prefix {
        calls: [7, 1, 1, 1, 1],
        operations: OPERATIONS.to_vec(),
        copied: SOURCE.len(),
        record_phase: Some("GpuAccessibleMutable"),
        pending: None,
    }
}

fn failed_prefix(
    calls: [usize; 5],
    copied: usize,
    record: bool,
    pending: Option<(&'static str, bool, bool, Option<bool>)>,
) -> Prefix {
    let operations = if calls[4] != 0 {
        3
    } else if calls[3] != 0 {
        2
    } else {
        0
    };
    Prefix {
        calls,
        operations: OPERATIONS[..operations].to_vec(),
        copied,
        record_phase: record.then_some("CpuWritable"),
        pending,
    }
}

type NativeProgress = (bool, Option<bool>, Option<u32>);

fn check_native_failure(
    ordinal: usize,
    fault: Fault,
    prefix: Prefix,
    terminal: Option<(&str, NativeProgress)>,
) {
    let mut f = CoherentFixture::new(ordinal);
    let before = f.base.memory().coherent_insertion_snapshot_v1();
    let accounting = f.base.memory().observation();
    let ledger = f.base.context().ledger_snapshot();
    let admitted = prefix.calls[1] != 0;
    let pending = prefix.pending.is_some();
    let copied = prefix.copied;
    f.trace.fault = fault;
    let source = SOURCE;
    let settled = f.insert(Some(2), &source);
    assert!(settled.transport);
    assert_eq!(
        f.base.memory().observation().phase,
        SharedMemorySessionPhaseV1::Quarantined
    );
    match (fault, &settled.result) {
        (Fault::Currentness(_, true), Err(p)) => assert_eq!(
            p.downcast_ref::<(&str, &str)>(),
            Some(&("N2 native panic", "currentness"))
        ),
        (Fault::Native(op, true), Err(p)) => assert_eq!(
            p.downcast_ref::<(&str, &str)>(),
            Some(&("N2 native panic", op))
        ),
        (Fault::CopyPanic(n), Err(p)) => assert_eq!(
            p.downcast_ref::<(&str, usize)>(),
            Some(&("coherent insertion copy", n))
        ),
        (
            Fault::Currentness(_, false),
            Ok(Err(ComputeAqlQueueSessionErrorV1::Memory(MemorySessionError::Injected(
                "currentness",
            )))),
        ) => {}
        (
            Fault::Native(op, false),
            Ok(Err(ComputeAqlQueueSessionErrorV1::Memory(MemorySessionError::Injected(actual)))),
        ) => assert_eq!(op, *actual),
        (Fault::Map(n, errno), Ok(Err(ComputeAqlQueueSessionErrorV1::Memory(error)))) => {
            match error {
                MemorySessionError::KernelResultMalformed(
                    "shared MAP_MEMORY_TO_GPU cumulative n_success",
                ) => assert_eq!(n, 2),
                MemorySessionError::KernelResultMalformed(
                    "shared MAP_MEMORY_TO_GPU full prefix",
                ) => assert_eq!((n, errno), (0, false)),
                MemorySessionError::Injected("map_gpu") => assert!(errno && n <= 1),
                _ => panic!("unexpected map failure: {error:?}"),
            }
        }
        _ => panic!("exact native failure required"),
    }
    assert_eq!(f.base.context().ledger_snapshot(), ledger);
    assert!(f.trace.before_retake.is_none() && f.trace.before_failure.is_none());
    let expected_stages = match terminal {
        None => [1, 0, 0],
        Some(("Copy", _)) => [1, 1, 0],
        Some(_) => [1; 3],
    };
    assert_eq!(f.trace.memory.stages, expected_stages);
    if expected_stages[1] == 1 {
        assert_eq!(
            f.trace.memory.source,
            Some((source.as_ptr() as usize, source.len()))
        );
    }
    assert_eq!(source, SOURCE);
    let marker = terminal.map(|(stage, progress)| {
        let token = f.trace.memory.allocated.as_ref().unwrap();
        token.assert_id(before.next_id);
        f.base
            .memory()
            .coherent_assert_terminal_v1(token, stage, false, progress);
        token.identity
    });
    assert!(copied <= SOURCE.len());
    f.base
        .memory()
        .coherent_assert_prefix_v1(&before, &SOURCE, prefix);
    f.assert_account_delta(&accounting, admitted, pending);
    f.assert_model(u8::from(terminal.is_some()));
    f.assert_retry();
    f.finish(true, marker);
}

#[test]
fn coherent_insertion_currentness_matrix_retains_exact_prefix_and_model() {
    for ordinal in 0..3 {
        for panic in [false, true] {
            for check in 1..=7 {
                let prefix = match check {
                    1 => failed_prefix([1, 0, 0, 0, 0], 0, false, None),
                    2 => failed_prefix(
                        [2, 1, 1, 0, 0],
                        0,
                        false,
                        Some(("CheckAllocation", true, true, None)),
                    ),
                    3 => failed_prefix(
                        [3, 1, 1, 1, 0],
                        0,
                        false,
                        Some(("CheckMapping", true, true, Some(true))),
                    ),
                    _ => failed_prefix(
                        [check, 1, 1, 1, usize::from(check == 7)],
                        if check == 4 { 0 } else { SOURCE.len() },
                        true,
                        None,
                    ),
                };
                let terminal = (check >= 4).then_some((
                    if check <= 5 { "Copy" } else { "Map" },
                    if check == 7 {
                        (true, Some(true), Some(1))
                    } else {
                        (false, None, None)
                    },
                ));
                check_native_failure(ordinal, Fault::Currentness(check, panic), prefix, terminal);
            }
        }
    }
}

#[test]
fn coherent_insertion_native_allocation_matrix_preserves_pending_owner() {
    for ordinal in 0..3 {
        for panic in [false, true] {
            for operation in ["reserve_va", "alloc", "map_cpu", "prepare_cpu_mapping"] {
                let mut prefix = match operation {
                    "reserve_va" => failed_prefix(
                        [1, 1, 0, 0, 0],
                        0,
                        false,
                        Some(("ReserveVa", false, false, None)),
                    ),
                    "alloc" => failed_prefix(
                        [1, 1, 1, 0, 0],
                        0,
                        false,
                        Some(("Allocate", true, !panic, None)),
                    ),
                    "map_cpu" => failed_prefix(
                        [2, 1, 1, 1, 0],
                        0,
                        false,
                        Some(("MapCpu", true, true, None)),
                    ),
                    _ => failed_prefix(
                        [2, 1, 1, 1, 0],
                        0,
                        false,
                        Some(("PrepareCpuMapping", true, true, Some(false))),
                    ),
                };
                if operation == "map_cpu" {
                    prefix.operations.truncate(1);
                }
                check_native_failure(ordinal, Fault::Native(operation, panic), prefix, None);
            }
        }
    }
}

#[test]
fn coherent_insertion_copy_and_map_matrix_retains_cpu_authority() {
    for ordinal in 0..3 {
        for length in [0, 1, SOURCE.len()] {
            check_native_failure(
                ordinal,
                Fault::CopyPanic(length),
                failed_prefix([4, 1, 1, 1, 0], length, true, None),
                Some(("Copy", (false, None, None))),
            );
        }
        for (n, errno) in [(0, false), (0, true), (1, true), (2, false), (2, true)] {
            check_native_failure(
                ordinal,
                Fault::Map(n, errno),
                failed_prefix([6, 1, 1, 1, 1], SOURCE.len(), true, None),
                Some(("Map", (true, Some(!errno), Some(n)))),
            );
        }
        for panic in [false, true] {
            check_native_failure(
                ordinal,
                Fault::Native("map_gpu", panic),
                failed_prefix([6, 1, 1, 1, 1], SOURCE.len(), true, None),
                Some((
                    "Map",
                    if panic {
                        (true, None, None)
                    } else {
                        (true, Some(false), Some(1))
                    },
                )),
            );
        }
    }
}

#[test]
fn coherent_insertion_projection_matrix_preserves_checkpoint_and_exact_successor() {
    for ordinal in 0..3 {
        for allocation in [true, false] {
            for (index, case) in PrimaryProjectionCaseV1::cases(allocation)
                .into_iter()
                .enumerate()
            {
                let mut f = CoherentFixture::new(ordinal);
                let before = f.base.memory().coherent_insertion_snapshot_v1();
                let accounting = f.base.memory().observation();
                let ledger = f.base.context().ledger_snapshot();
                f.trace.fault = Fault::Projection(allocation, case);
                let settled = f.insert(Some(2), &SOURCE);
                assert!(settled.transport);
                if case.panic {
                    case.assert_panic(&*settled.result.err().unwrap());
                } else {
                    assert!(matches!(
                        settled.result,
                        Ok(Err(ComputeAqlQueueSessionErrorV1::Memory(
                            MemorySessionError::Injected("session projection")
                        )))
                    ));
                }
                assert_eq!(f.base.context().ledger_snapshot(), ledger);
                assert!(f.trace.before_retake.is_none() && f.trace.before_failure.is_none());
                let mapped = !allocation && index >= 4;
                let mut prefix = if allocation {
                    failed_prefix([3, 1, 1, 1, 0], 0, true, None)
                } else {
                    failed_prefix(
                        [if mapped { 7 } else { 5 }, 1, 1, 1, usize::from(mapped)],
                        SOURCE.len(),
                        true,
                        None,
                    )
                };
                if mapped {
                    prefix.record_phase = Some("GpuAccessibleMutable");
                }
                f.base
                    .memory()
                    .coherent_assert_prefix_v1(&before, &SOURCE, prefix);
                assert_eq!(
                    f.trace.memory.stages,
                    if allocation { [1, 0, 0] } else { [1; 3] }
                );
                let engine = &f.base.scope.primary.completed.as_ref().unwrap().engine;
                f.base
                    .memory()
                    .coherent_assert_projection_v1(&engine.foundation);
                f.assert_model(u8::from(!allocation));
                f.assert_account_delta(&accounting, true, false);
                let marker = f.trace.memory.allocated.as_ref().map(|t| {
                    t.assert_id(before.next_id);
                    t.identity
                });
                f.assert_retry();
                f.finish(true, marker);
            }
        }
    }
}

#[test]
fn coherent_insertion_constructed_success_preserves_exact_order_source_and_accounts() {
    for ordinal in 0..3 {
        for index in [0, 2, 4] {
            let mut f = CoherentFixture::new(ordinal);
            let before = f.base.memory().coherent_insertion_snapshot_v1();
            let accounting = f.base.memory().observation();
            let ledger = f.base.context().ledger_snapshot();
            let loan = f.base.loan_state();
            let source = Box::new(SOURCE);
            let pointer = source.as_ptr() as usize;
            let settled = f.insert(Some(index), source.as_slice());
            assert!(!settled.transport);
            let data = settled.into_result().unwrap();
            let expected = f.trace.memory.allocated.as_ref().unwrap().mapped();
            assert_eq!(f.trace.before_retake.as_ref(), Some(&expected));
            assert_eq!(f.trace.memory.source, Some((pointer, SOURCE.len())));
            assert_eq!(*source, SOURCE);
            assert_eq!(f.trace.memory.stages, [1; 3]);
            assert_eq!(
                data.storage_identity(),
                Gfx942FixedDispatchStorageIdentityV1::HostVisibleInitialized(expected.identity)
            );
            let mut identities = ledger.0;
            identities.insert(index, data.storage_identity());
            assert_eq!(
                f.base.context().ledger_snapshot(),
                (identities, ledger.1 + 1, None)
            );
            assert_eq!(f.base.loan_state(), (loan.0, None, loan.2 + 1));
            f.base
                .memory()
                .coherent_assert_prefix_v1(&before, &SOURCE, success_prefix());
            f.assert_account_delta(&accounting, true, false);
            f.assert_model(2);
            f.base.data.insert(index, data);
            f.finish(false, None);
        }
    }
}

#[test]
fn coherent_insertion_completed_owner_survives_retake_and_commit_failure() {
    for ordinal in 0..3 {
        for closing in 0..6 {
            let mut f = CoherentFixture::new(ordinal);
            let before = f.base.memory().coherent_insertion_snapshot_v1();
            let accounting = f.base.memory().observation();
            let ledger = f.base.context().ledger_snapshot();
            let offset = trace().borrow().calls.len();
            let pre_occurrence = trace()
                .borrow()
                .calls
                .iter()
                .filter(|&&s| s == "auxiliary-retake")
                .count()
                + 1;
            let post_occurrence = trace()
                .borrow()
                .calls
                .iter()
                .filter(|&&s| s == "auxiliary-retake-complete")
                .count()
                + 1;
            match closing {
                0 => f.base.scope.parent.faults.reclaim_before = Outcome::Error,
                1 => f.base.scope.parent.faults.reclaim_before = Outcome::Panic,
                2 => f.base.scope.parent.faults.reclaim_after = Outcome::Error,
                3 => f.base.scope.parent.faults.reclaim_after = Outcome::Panic,
                4 => f.base.scope.parent.faults.regress_revision = true,
                _ => f.base.trace.commit_panic = true,
            }
            let settled = f.insert(Some(2), &SOURCE);
            assert!(settled.transport);
            assert_eq!(settled.result.is_err(), matches!(closing, 1 | 3 | 5));
            match &settled.result {
                Err(payload) if closing == 5 => assert_eq!(
                    payload.downcast_ref::<&str>(),
                    Some(&"insertion commit ledger access")
                ),
                Err(payload) => assert_eq!(
                    payload.downcast_ref::<(&str, usize)>(),
                    Some(&(
                        if closing == 1 {
                            "auxiliary-retake"
                        } else {
                            "auxiliary-retake-complete"
                        },
                        if closing == 1 {
                            pre_occurrence
                        } else {
                            post_occurrence
                        }
                    ))
                ),
                Ok(Err(ComputeAqlQueueSessionErrorV1::Contract(name))) => assert_eq!(
                    *name,
                    if closing == 0 {
                        "auxiliary-retake"
                    } else {
                        "auxiliary-retake-complete"
                    }
                ),
                Ok(Err(ComputeAqlQueueSessionErrorV1::Memory(MemorySessionError::Model(name))))
                    if closing == 4 =>
                {
                    assert_eq!(*name, "fixture live foundation reclaim")
                }
                _ => panic!("exact coherent closing failure required"),
            }
            for call in ["auxiliary-loan", "auxiliary-retake"] {
                assert_eq!(
                    trace().borrow().calls[offset..]
                        .iter()
                        .filter(|&&name| name == call)
                        .count(),
                    1
                );
            }
            assert_eq!(f.base.context().ledger_snapshot(), ledger);
            let expected = f.trace.memory.allocated.as_ref().unwrap().mapped();
            assert_eq!(f.trace.before_retake.as_ref(), Some(&expected));
            assert_eq!(
                f.trace.before_failure.as_ref(),
                Some(&expected),
                "Complete retained until the full ledger commit"
            );
            f.base.memory().coherent_assert_terminal_v1(
                &expected,
                "LiveInsertion",
                true,
                (false, None, None),
            );
            f.base
                .memory()
                .coherent_assert_prefix_v1(&before, &SOURCE, success_prefix());
            f.assert_account_delta(&accounting, true, false);
            f.assert_model(2);
            f.assert_retry();
            f.finish(true, Some(expected.identity));
        }
    }
}

#[test]
fn coherent_insertion_required_hole_and_validation_reject_before_reservation() {
    for ordinal in 0..3 {
        for case in 0..5 {
            let mut f = CoherentFixture::new(ordinal);
            let before = f.base.memory().coherent_insertion_snapshot_v1();
            let accounting = f.base.memory().observation();
            let ledger = f.base.context().ledger_snapshot();
            if case == 2 {
                f.base.trace.reserve = Outcome::Error;
            }
            if case == 3 {
                f.base.trace.reserve = Outcome::Panic;
            }
            let settled = f.insert(
                if case == 0 {
                    None
                } else if case == 1 {
                    Some(5)
                } else {
                    Some(0)
                },
                if case == 4 { &[] } else { &SOURCE },
            );
            assert_eq!(settled.transport, case >= 3);
            assert!(!matches!(settled.result, Ok(Ok(_))));
            match &settled.result {
                Ok(Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::ResourcePhase,
                ))) => assert_eq!(case, 0),
                Ok(Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::InvalidData { .. },
                ))) => assert_eq!(case, 1),
                Ok(Err(ComputeAqlQueueSessionErrorV1::Contract("insertion-reserve"))) => {
                    assert_eq!(case, 2)
                }
                Err(payload) => {
                    assert_eq!(case, 3);
                    assert_eq!(
                        payload.downcast_ref::<(&str, usize)>().unwrap().0,
                        "insertion-reserve"
                    );
                }
                Ok(Err(ComputeAqlQueueSessionErrorV1::Memory(
                    MemorySessionError::InvalidRequestedSize,
                ))) => assert_eq!(case, 4),
                _ => panic!("exact coherent rejection required"),
            }
            assert_eq!(f.base.trace.reserve_calls, usize::from(case >= 2));
            assert_eq!(f.base.trace.prepare_calls, usize::from(case == 4));
            assert_eq!(f.trace.memory.stages, [0; 3]);
            assert_eq!(f.base.memory().coherent_insertion_snapshot_v1(), before);
            assert_eq!(f.base.memory().observation(), accounting);
            assert_eq!(f.base.context().ledger_snapshot(), ledger);
            if settled.transport {
                f.assert_retry();
            }
            f.finish(settled.transport, None);
        }
    }
}

#[test]
fn coherent_insertion_opening_failure_and_missing_complete_never_commit() {
    for ordinal in 0..3 {
        for case in 0..3 {
            let mut f = CoherentFixture::new(ordinal);
            let before = f.base.memory().coherent_insertion_snapshot_v1();
            let ledger = f.base.context().ledger_snapshot();
            let offset = trace().borrow().calls.len();
            let occurrence = trace()
                .borrow()
                .calls
                .iter()
                .filter(|&&s| s == "auxiliary-loan")
                .count()
                + 1;
            if case == 2 {
                f.base.trace.skip_prepare = true;
            } else {
                f.base.scope.parent.faults.loan = if case == 0 {
                    Outcome::Error
                } else {
                    Outcome::Panic
                };
            }
            let settled = f.insert(Some(0), &SOURCE);
            assert!(settled.transport);
            assert!(!matches!(settled.result, Ok(Ok(_))));
            assert_eq!(settled.result.is_err(), case == 1);
            match &settled.result {
                Err(payload) => assert_eq!(
                    payload.downcast_ref::<(&str, usize)>(),
                    Some(&("auxiliary-loan", occurrence))
                ),
                Ok(Err(ComputeAqlQueueSessionErrorV1::Contract("auxiliary-loan"))) => {
                    assert_eq!(case, 0)
                }
                Ok(Err(ComputeAqlQueueSessionErrorV1::Memory(
                    MemorySessionError::InvalidAllocationAuthority,
                ))) => assert_eq!(case, 2),
                _ => panic!("exact opening or absent-Complete failure required"),
            }
            assert_eq!(
                trace().borrow().calls[offset..]
                    .iter()
                    .filter(|&&call| call == "auxiliary-retake")
                    .count(),
                usize::from(case == 2)
            );
            assert!(f.trace.before_retake.is_none() && f.trace.before_failure.is_none());
            assert_eq!(f.base.memory().coherent_insertion_snapshot_v1(), before);
            assert_eq!(f.base.context().ledger_snapshot(), ledger);
            f.assert_retry();
            f.finish(true, None);
        }
    }
}

#[test]
fn coherent_insertion_admits_sixteenth_then_rejects_full_before_index_or_reservation() {
    for ordinal in 0..3 {
        let mut f = CoherentFixture::new(ordinal);
        for count in 4..16 {
            f.trace = CoherentTrace::default();
            let before = f.base.memory().coherent_insertion_snapshot_v1();
            let accounting = f.base.memory().observation();
            let (mut identities, n, hole) = f.base.context().ledger_snapshot();
            assert_eq!((n, hole), (count, None));
            let settled = f.insert(Some(count), &SOURCE);
            assert!(!settled.transport);
            let data = settled.into_result().unwrap();
            identities.push(data.storage_identity());
            f.base.data.push(data);
            assert_eq!(
                f.base.context().ledger_snapshot(),
                (identities, count + 1, None)
            );
            f.base
                .memory()
                .coherent_assert_prefix_v1(&before, &SOURCE, success_prefix());
            f.assert_account_delta(&accounting, true, false);
            f.assert_model(2);
        }
        let before = f.base.memory().coherent_insertion_snapshot_v1();
        let ledger = f.base.context().ledger_snapshot();
        let calls = (f.base.trace.reserve_calls, f.base.trace.prepare_calls);
        f.base.trace.reserve = Outcome::Panic;
        for index in [Some(0), Some(17), None] {
            let settled = f.insert(index, &[]);
            assert!(!settled.transport);
            assert!(matches!(
                settled.result,
                Ok(Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::DataLeaseCount {
                        requested: 17,
                        maximum: 16
                    }
                )))
            ));
            assert_eq!(f.base.memory().coherent_insertion_snapshot_v1(), before);
            assert_eq!(f.base.context().ledger_snapshot(), ledger);
            assert_eq!(
                (f.base.trace.reserve_calls, f.base.trace.prepare_calls),
                calls
            );
        }
        f.finish(false, None);
    }
}

#[test]
fn coherent_insertion_uses_real_release_hole_and_explicit_override() {
    for ordinal in 0..3 {
        for index in [None, Some(1)] {
            let mut f = CoherentFixture::new(ordinal);
            let old = f.base.data[2].storage_identity();
            let mut data = Some(f.base.data.remove(2));
            let mut released = None;
            let (operation, retake) = f
                .base
                .scope
                .parent
                .with_preparation_custody(|memory| {
                    released = Some(memory.insertion_release_device_v1(data.take().unwrap())?);
                    Ok(())
                })
                .unwrap();
            retake.unwrap();
            operation.unwrap();
            f.base.released.push(released.unwrap());
            {
                let mut context = f.base.context();
                let ledger = context.ledger();
                assert_eq!(ledger.identities.remove(2), old);
                *ledger.count -= 1;
                *ledger.next = Some(2);
            }
            let before = f.base.memory().coherent_insertion_snapshot_v1();
            let accounting = f.base.memory().observation();
            let (mut identities, count, hole) = f.base.context().ledger_snapshot();
            assert_eq!((count, hole), (3, Some(2)));
            let settled = f.insert(index, &SOURCE);
            assert!(!settled.transport);
            let output = settled.into_result().unwrap();
            identities.insert(index.unwrap_or(2), output.storage_identity());
            f.base.data.insert(index.unwrap_or(2), output);
            assert_eq!(
                f.base.context().ledger_snapshot(),
                (identities.clone(), 4, None)
            );
            f.base
                .memory()
                .coherent_assert_prefix_v1(&before, &SOURCE, success_prefix());
            f.assert_account_delta(&accounting, true, false);
            f.assert_model(2);
            let native = f.base.memory().coherent_insertion_snapshot_v1();
            let calls = (f.base.trace.reserve_calls, f.base.trace.prepare_calls);
            let rejected = f.insert(None, &SOURCE);
            assert!(!rejected.transport);
            assert!(matches!(
                rejected.result,
                Ok(Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                )))
            ));
            assert_eq!(f.base.memory().coherent_insertion_snapshot_v1(), native);
            assert_eq!(
                (f.base.trace.reserve_calls, f.base.trace.prepare_calls),
                calls
            );
            assert_eq!(f.base.context().ledger_snapshot(), (identities, 4, None));
            f.finish(false, None);
        }
    }
}

#[test]
fn coherent_insertion_operation_panic_wins_secondary_retake_error_or_panic() {
    for ordinal in 0..3 {
        for closing in [Outcome::Success, Outcome::Error, Outcome::Panic] {
            let mut f = CoherentFixture::new(ordinal);
            let before = f.base.memory().coherent_insertion_snapshot_v1();
            let accounting = f.base.memory().observation();
            let ledger = f.base.context().ledger_snapshot();
            let calls = trace().borrow().calls.len();
            f.trace.fault = Fault::CopyPanic(1);
            f.base.scope.parent.faults.reclaim_before = closing;
            let settled = f.insert(Some(2), &SOURCE);
            assert!(settled.transport);
            assert_eq!(
                settled
                    .result
                    .err()
                    .unwrap()
                    .downcast_ref::<(&str, usize)>(),
                Some(&("coherent insertion copy", 1))
            );
            assert_eq!(
                trace().borrow().calls[calls..]
                    .iter()
                    .filter(|&&c| c == "auxiliary-retake")
                    .count(),
                1
            );
            assert_eq!(f.base.context().ledger_snapshot(), ledger);
            let token = f.trace.memory.allocated.as_ref().unwrap();
            f.base
                .memory()
                .coherent_assert_terminal_v1(token, "Copy", false, (false, None, None));
            let marker = token.identity;
            f.base.memory().coherent_assert_prefix_v1(
                &before,
                &SOURCE,
                failed_prefix([4, 1, 1, 1, 0], 1, true, None),
            );
            f.assert_account_delta(&accounting, true, false);
            f.assert_model(1);
            f.assert_retry();
            f.finish(true, Some(marker));
        }
    }
}

#[test]
fn coherent_insertion_retake_failure_precedes_ordinary_operation_error() {
    for ordinal in 0..3 {
        for after in [false, true] {
            for closing in [Outcome::Error, Outcome::Panic] {
                let mut f = CoherentFixture::new(ordinal);
                let before = f.base.memory().coherent_insertion_snapshot_v1();
                let accounting = f.base.memory().observation();
                let ledger = f.base.context().ledger_snapshot();
                let name = if after {
                    "auxiliary-retake-complete"
                } else {
                    "auxiliary-retake"
                };
                let occurrence = trace()
                    .borrow()
                    .calls
                    .iter()
                    .filter(|&&c| c == name)
                    .count()
                    + 1;
                f.trace.fault = Fault::Currentness(5, false);
                if after {
                    f.base.scope.parent.faults.reclaim_after = closing;
                } else {
                    f.base.scope.parent.faults.reclaim_before = closing;
                }
                let settled = f.insert(Some(2), &SOURCE);
                assert!(settled.transport);
                match settled.result {
                    Err(payload) => {
                        assert_eq!(closing, Outcome::Panic);
                        assert_eq!(
                            payload.downcast_ref::<(&str, usize)>(),
                            Some(&(name, occurrence))
                        );
                    }
                    Ok(Err(ComputeAqlQueueSessionErrorV1::Contract(actual))) => {
                        assert_eq!(closing, Outcome::Error);
                        assert_eq!(actual, name);
                    }
                    _ => panic!("retake must precede the ordinary operation error"),
                }
                assert_eq!(f.base.context().ledger_snapshot(), ledger);
                let token = f.trace.memory.allocated.as_ref().unwrap();
                let marker = token.identity;
                f.base.memory().coherent_assert_terminal_v1(
                    token,
                    "Copy",
                    false,
                    (false, None, None),
                );
                f.base.memory().coherent_assert_prefix_v1(
                    &before,
                    &SOURCE,
                    failed_prefix([5, 1, 1, 1, 0], SOURCE.len(), true, None),
                );
                f.assert_account_delta(&accounting, true, false);
                f.assert_model(1);
                f.assert_retry();
                f.finish(true, Some(marker));
            }
        }
    }
}
