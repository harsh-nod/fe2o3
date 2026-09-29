//! Public-entry cleanup and the independent nine-stage/domain accounting oracle.
use super::super::super::tests::{noop, with_checked};
use super::super::tests::{fixture, nine_oracle as oracle};
use super::*;
use oracle::numbers::{self, Oracle};
use std::panic::panic_any;

#[derive(Debug)]
struct Payload(usize);
impl Drop for Payload {
    fn drop(&mut self) {
        resources::cleanup_trace_record("payload", self.0);
        if self.0 != 0 {
            panic_any(Payload(self.0 - 1));
        }
    }
}

fn invoke<'w, T>(
    mode: usize,
    checked: &mut CheckedCanonicalRankedViewV1<'_, '_, '_, '_>,
    budget: &mut Budget<'w>,
    callback: impl FnOnce(&mut Budget<'w>) -> Result<T, Failure>,
) -> Result<T, Failure> {
    match mode {
        0 => with_canonical_private_memory_policy_checks_v1(
            checked,
            tests::limits(),
            budget,
            |_, budget| callback(budget),
        )
        .map_err(|error| error.failure),
        1 => with_canonical_private_policy_checks_v1(checked, budget, |_, budget| callback(budget))
            .map_err(|error| error.failure),
        2 => traps::with_canonical_trap_policy_checks_v1(checked, budget, |_, budget| {
            callback(budget)
        })
        .map_err(|error| error.failure),
        3 => with_canonical_ranked_policy_checks_v1(checked, budget, |_, budget| callback(budget))
            .map_err(|error| error.failure),
        4 => traps::with_canonical_trap_shape_v1(checked, budget, |_, budget| callback(budget)),
        _ => unreachable!(),
    }
}

fn assert_drain(depth: usize, held: usize, refund_floor: usize) {
    let events = resources::cleanup_trace_take();
    assert_eq!(events.first(), Some(&("callback discard", held)));
    assert_eq!(events.last(), Some(&("before refund", refund_floor)));
    assert_eq!(events.len(), depth + 3);
    for (position, expected) in (0..=depth).rev().enumerate() {
        assert_eq!(events[position + 1], ("payload", expected));
    }
}

#[test]
fn public_entries_contain_one_and_two_nested_payload_destructor_panics_before_refund() {
    for mode in 0..5 {
        for depth in 1..=2 {
            let module = if mode == 3 { noop() } else { fixture() };
            with_checked(&module, |checked, budget| {
                let entry = budget.storage();
                let held = Cell::new(0);
                resources::cleanup_trace_start();
                let error = invoke::<()>(mode, checked, budget, |budget| {
                    held.set(budget.storage());
                    panic_any(Payload(depth));
                })
                .unwrap_err();
                assert!(matches!(error, Failure::Panicked));
                assert!(held.get() > entry);
                assert_drain(depth, held.get(), held.get());
                assert_eq!(budget.storage(), entry);
                invoke(mode, checked, budget, |_| Ok(())).unwrap();
                assert_eq!(budget.storage(), entry);
            });
        }
    }
}

#[test]
fn public_entries_discard_rejected_callback_owners_with_nested_panics_before_refund() {
    for mode in 0..5 {
        for undercut in [false, true] {
            let module = if mode == 3 { noop() } else { fixture() };
            with_checked(&module, |checked, budget| {
                let entry = budget.storage();
                let held = Cell::new(0);
                resources::cleanup_trace_start();
                let error = invoke(mode, checked, budget, |budget| {
                    held.set(budget.storage());
                    if undercut {
                        budget.release_storage(1)?;
                    } else {
                        budget.reserve_storage(1)?;
                    }
                    Ok(Payload(2))
                })
                .unwrap_err();
                assert!(matches!(error, Failure::Resource(Resource::Accounting)));
                let before_refund = if undercut {
                    held.get() - 1
                } else {
                    held.get() + 1
                };
                assert!(before_refund > entry);
                assert_drain(2, held.get(), before_refund);
                assert_eq!(budget.storage(), entry);
            });
        }
    }
}

#[test]
fn physical_query_poison_keeps_first_error_while_rejected_owner_is_drained() {
    with_checked(&tests::split(), |checked, budget| {
        let entry = budget.storage();
        let held = Cell::new(0);
        resources::cleanup_trace_start();
        let error = with_canonical_private_memory_policy_checks_v1(
            checked,
            tests::limits(),
            budget,
            |view, budget| {
                held.set(budget.storage());
                assert!(view.trap_policies(budget)?.report(2, budget).is_err());
                assert!(view.physical_memory(budget).is_err());
                Ok(Payload(1))
            },
        )
        .unwrap_err();
        assert!(matches!(
            error.failure(),
            Failure::InvalidQuery { function: 2 }
        ));
        assert_drain(1, held.get(), held.get());
        assert_eq!(budget.storage(), entry);
    });
}

#[test]
fn successful_return_owner_is_not_discarded_as_a_rejected_callback() {
    with_checked(&fixture(), |checked, budget| {
        let entry = budget.storage();
        resources::cleanup_trace_start();
        let value = with_canonical_private_memory_policy_checks_v1(
            checked,
            tests::limits(),
            budget,
            |_, _| Ok(Payload(0)),
        )
        .unwrap();
        let before_drop = resources::cleanup_trace_take();
        assert_eq!(before_drop.len(), 1);
        assert_eq!(before_drop[0].0, "before refund");
        assert!(before_drop[0].1 > entry);
        assert_eq!(budget.storage(), entry);
        resources::cleanup_trace_start();
        drop(value);
        assert_eq!(resources::cleanup_trace_take(), [("payload", 0)]);
    });
}

#[test]
fn fixed_drain_header_is_distinct_and_existing_shape_double_slot_is_sufficient() {
    assert_eq!(size_of::<usize>(), 8, "guarded 64-bit accounting profile");
    assert_eq!(resources::drain_header(), 16);
    fn check<T>() {
        let result = size_of::<std::thread::Result<Result<T, Failure>>>();
        let with_drain = result.checked_add(resources::drain_header()).unwrap();
        assert_eq!(with_drain - result, 16);
        assert!(result.checked_mul(2).unwrap() >= with_drain);
    }
    check::<()>();
    check::<Payload>();
    check::<[u8; 4096]>();
}

#[test]
fn guarded_evidence_layout_accounts_for_the_private_graph_header_increment() {
    assert_eq!(size_of::<usize>(), 8);
    assert_eq!(size_of::<Vec<usize>>(), 24);
    // Both payloads contain one inventory reference and three Vec headers.
    assert_eq!(size_of::<CellCensus<'_, '_>>(), 8 + 3 * 24);
    assert_eq!(size_of::<PhysicalMemory<'_, '_>>(), 8 + 3 * 24);
    assert_eq!(size_of::<PrivateMemoryEvidenceV1<'_, '_>>(), 8 + 80);
    assert_eq!(size_of::<CallEffects<'_, '_>>(), 8 + 24);
    let old_header = 8 + 80 + 32 + 24 + 8;
    let new_header = 8 + 88 + 32 + 24 + 8;
    assert_eq!(old_header, 152);
    assert_eq!(
        size_of::<CanonicalPrivateGraphFactsV1<'_, '_>>(),
        new_header
    );
    assert_eq!(new_header - old_header, 8);
    // These are outer KIR reservations, not changes to a policy tuple.
    assert_eq!(resources::drain_header() + new_header - old_header, 24);
}

#[test]
fn failed_debit_does_not_preclude_an_accepted_smaller_cleanup_suffix() {
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    let mut work = Work::new(8);
    let mut budget = Budget::new(&mut work, 8);
    budget.charge_work(5).unwrap();
    assert!(budget.charge_work(4).is_err());
    budget.charge_work(3).unwrap();
    assert_eq!(budget.work(), 8);
    budget.reserve_storage(5).unwrap();
    assert!(budget.reserve_storage(4).is_err());
    budget.reserve_storage(3).unwrap();
    assert_eq!(budget.storage(), 8);
    budget.release_storage(8).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn physical_entry_retains_exact_existing_independent_nine_stage_numbers() {
    let full = numbers::module();
    // Independent caller/helper graph preparation adds74033/88673 work;
    // replacing their cyclic-capable continuation changes work by71662/85438
    // and retained credit by15 each. The second-function peak adds the first15.
    // Mixed coverage adds 1337 work and 21 retained units per function;
    // the helper's dominating peak holds the caller's additional 21 units.
    // CFG-domain setup, prepare, record, and finish costs add 271 work and
    // 11 retained units per definition. Only the caller's retained increment
    // is live at the helper's dominating trace peak.
    let domain_work = 11 + 4 + 9 * 20 + 9 * 4 + 9 * 4 + 4;
    let domain_retained = 9 + 1 + 1;
    assert_eq!(
        (full.w, full.r, full.p),
        (
            339644059 + 2 * domain_work,
            96824 + 2 * domain_retained,
            19995780 + domain_retained,
        )
    );
    with_checked(&fixture(), |checked, budget| {
        let entry = budget.storage();
        with_memory_checks(
            checked,
            tests::limits(),
            budget,
            oracle::exact_limits(full),
            |view, budget| {
                let traps = view.trap_policies(budget)?;
                assert_eq!(traps.definition_count(budget)?, 2);
                oracle::assert_observation(traps.observation(budget)?, full, None, false);
                for ordinal in 0..2 {
                    oracle::assert_report(traps.report(ordinal, budget)?, ordinal);
                    let history = traps.history(ordinal, budget)?;
                    assert_eq!(history.function(), ordinal);
                    oracle::assert_observation(
                        history.floor(),
                        oracle::floor(ordinal),
                        None,
                        false,
                    );
                    oracle::assert_observation(
                        history.invocation(),
                        Oracle::derive(ordinal).complete,
                        None,
                        false,
                    );
                }
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(budget.storage(), entry);
    });
}

// This predicts from the independent oracle, never from a measured invocation.
// A later masked storage cut is attributed to the actual earlier denial.
fn assert_cut(work: usize, peak: usize) -> usize {
    let mut prediction = None;
    'definitions: for ordinal in 0..2 {
        let oracle = Oracle::derive(ordinal);
        for gate in &oracle.gates {
            let total = oracle::floor(ordinal).then(gate.required);
            let resource = if total.w > work {
                if gate.name == "capture reservation" {
                    "remaining identity capture work upper bound"
                } else {
                    numbers::WORK
                }
            } else if total.p > peak {
                numbers::PEAK
            } else {
                continue;
            };
            prediction = Some((ordinal, gate.before, gate.phase, gate.cause, resource));
            break 'definitions;
        }
    }
    let (ordinal, local, phase, cause, resource) =
        prediction.expect("this is a genuine denied cut");
    with_checked(&fixture(), |checked, budget| {
        let entry = budget.storage();
        let error = with_memory_checks::<()>(
            checked,
            tests::limits(),
            budget,
            Limits::new(work, peak),
            |_, _| panic!("denied native invocation cannot reach callback"),
        )
        .unwrap_err();
        use crate::production_analysis::{
            pliron_pass_contract::PlironPassPreservationErrorV1 as Preservation,
            pliron_report_validation::ProductionAnalysisReportValidationErrorV1 as Validation,
        };
        let expected = match cause {
            numbers::Cause::Preservation => {
                ProductionPlironPreloweringErrorV2::Preservation(Preservation::ResourceLimit {
                    resource,
                })
            }
            numbers::Cause::Validation(producing_pass) => {
                ProductionPlironPreloweringErrorV2::ReportValidation(Validation::ResourceLimit {
                    producing_pass,
                    resource,
                })
            }
            numbers::Cause::Resource(producing_pass) => {
                ProductionPlironPreloweringErrorV2::ResourceLimit {
                    phase,
                    producing_pass,
                    resource,
                }
            }
        };
        match error.failure() {
            Failure::Analysis { function, cause } => {
                assert_eq!(*function, ordinal);
                assert_eq!(*cause, expected);
            }
            other => panic!("expected exact independent native quota cause: {other:?}"),
        }
        let history = error.last_invocation().unwrap();
        assert_eq!(history.function(), ordinal);
        oracle::assert_observation(history.floor(), oracle::floor(ordinal), None, false);
        oracle::assert_observation(history.invocation(), local, Some((phase, resource)), false);
        oracle::assert_observation(
            error.observation(),
            oracle::floor(ordinal).then(local),
            Some((phase, resource)),
            false,
        );
        assert_eq!(budget.storage(), entry);
    });
    ordinal
}

#[test]
fn physical_entry_all_nine_prepare_work_cuts_use_the_same_independent_prefixes() {
    let full = numbers::module();
    for ordinal in 0..2 {
        let oracle = Oracle::derive(ordinal);
        assert_eq!(oracle.stages.len(), 9);
        for stage in &oracle.stages {
            let work = oracle::floor(ordinal).w + stage.prepared.w;
            assert_cut(work - 1, full.p);
            assert_cut(work, full.p);
        }
    }
}

#[test]
fn physical_entry_global_storage_peaks_and_masked_late_cuts_are_not_retuned() {
    let full = numbers::module();
    let mut maximum = 0;
    let mut counts = [0usize; 2];
    for ordinal in 0..2 {
        for gate in &Oracle::derive(ordinal).gates {
            let required = oracle::floor(ordinal).then(gate.required).p;
            if required <= maximum {
                continue;
            }
            maximum = required;
            counts[ordinal] += 1;
            assert_cut(full.w, required - 1);
            if required < full.p {
                assert_cut(full.w, required);
            }
        }
    }
    assert_eq!(counts, [9, 1]);
    assert_eq!(maximum, full.p);
    assert_eq!(assert_cut(full.w, full.p - 1), 1);
    for stage in &Oracle::derive(1).stages {
        assert_eq!(
            assert_cut(full.w, oracle::floor(1).then(stage.checkpoint).p - 1),
            1
        );
    }
}

#[test]
fn callback_failure_releases_reports_without_rolling_back_the_analysis_prefix() {
    let full = numbers::module();
    with_checked(&fixture(), |checked, budget| {
        let entry = budget.storage();
        let error = with_memory_checks::<()>(
            checked,
            tests::limits(),
            budget,
            oracle::exact_limits(full),
            |_, _| Err(Failure::Callback("physical callback refusal")),
        )
        .unwrap_err();
        assert!(matches!(
            error.failure(),
            Failure::Callback("physical callback refusal")
        ));
        oracle::assert_observation(error.observation(), full.released(), None, false);
        let history = error.last_invocation().unwrap();
        oracle::assert_observation(history.floor(), oracle::floor(1), None, false);
        oracle::assert_observation(
            history.invocation(),
            Oracle::derive(1).complete,
            None,
            false,
        );
        assert_eq!(budget.storage(), entry);
    });
}

#[test]
fn private_call_whole_entry_pinned_private_facade_layout_equivalence_premises() {
    use std::mem::{align_of, size_of};

    // These actual-private vs public-field comparisons are required external
    // qualification gates, not ABI guarantees or successful-run calibration.
    type QueryFields = Result<Resource, usize>;
    type GuardFields = (usize, Ledger, usize, Cell<Option<QueryFields>>);
    type AnalysisFields = (
        ([usize; 2], [usize; 3]),
        [usize; 2],
        Option<(Phase, &'static str)>,
        bool,
        Option<CanonicalRankedPolicyHistoryV1>,
    );
    type OutcomeFields = (CanonicalPrivatePipelineReportV1, [usize; 3]);
    type ReportFields = (OutcomeFields, CanonicalRankedPolicyHistoryV1);
    type EvidenceFields<'i, 'g> = Result<CellCensus<'i, 'g>, PhysicalMemory<'i, 'g>>;
    type FactsFields<'i, 'g> = (
        &'i Inventory<'g>,
        EvidenceFields<'i, 'g>,
        CallEffects<'i, 'g>,
        Vec<usize>,
        Option<&'i ()>,
    );
    type TerminalFields<'i, 'g> = (
        &'i Inventory<'g>,
        Vec<traps::CanonicalTrapPairV1>,
        Vec<traps::CanonicalTrapIncomingEdgeV1<'i, 'g>>,
        Vec<fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1>,
        Option<fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1>,
    );
    fn check<A, E>(name: &str) {
        assert_eq!(
            (size_of::<A>(), align_of::<A>()),
            (size_of::<E>(), align_of::<E>()),
            "{name} pinned layout premise; do not retune entry totals"
        );
    }
    assert_eq!((size_of::<usize>(), align_of::<usize>()), (8, 8));
    check::<Guard, GuardFields>("private facade guard");
    check::<private_resources::PrivateAnalysisV1, AnalysisFields>("private analysis state");
    check::<PrivateReportRowV1, ReportFields>("private report row");
    check::<PrivateMemoryEvidenceV1<'_, '_>, EvidenceFields<'_, '_>>("owned private evidence");
    check::<CanonicalPrivateGraphFactsV1<'_, '_>, FactsFields<'_, '_>>("private graph facts");
    check::<traps::CanonicalTrapPairsGraphFactsV1<'_, '_>, TerminalFields<'_, '_>>(
        "terminal facts",
    );
}

#[test]
fn private_call_whole_entry_v909_native_layout_premises() {
    use crate::production_analysis::{
        pliron_analysis_manager as manager, pliron_function_inventory as inventory,
        pliron_invocation_trace as trace, pliron_memory_order as memory,
        pliron_presburger_adapter as presburger, pliron_provenance_alias as provenance,
        pliron_resource_envelope as envelope, pliron_simt_protocol as simt,
        pliron_tensor_layout as tensor,
    };
    use dialect_gpu::{AddressSpaceAttr, HierarchyAttr, MemoryOrderAttr, MemoryScopeAttr};
    use dialect_kernel::{AccessKindAttr, AtomicOrderingAttr, AtomicScopeAttr, MemorySpaceAttr};
    use fe2o3_kernel_ir::{AccessMode, AddressSpace, ValueId};
    use pliron::{context::Ptr, operation::Operation, value::Value};
    use std::{mem::align_of, sync::Arc};
    use trace::{native_events_v1 as event, native_input_v1 as input};

    // Complete V909 field rosters, including every nested cache failure. These
    // same-host equivalences are gates, not repr(Rust) ABI or observed retuning.
    type Key = (usize, usize, u64, usize, usize);
    type Address = (ValueId, Option<i128>, Option<u32>, AddressSpace, AccessMode);
    #[allow(dead_code)]
    enum TraceFailure {
        Native {
            block: usize,
            operation: usize,
            reason: input::NativeTraceRefusalV1,
        },
        NativeResource(Resource),
        Sparse(crate::SparseIndexFailureV1),
        DynamicLaunch {
            dimension: usize,
        },
        LaunchTooLarge {
            invocations: u64,
        },
        UnresolvedBranch {
            block: usize,
        },
        ForeignView {
            block: usize,
            operation: usize,
        },
        UnsupportedTerminator {
            block: usize,
        },
        CyclicControlFlow {
            block: usize,
        },
        MissingExecutionLayout,
        InvalidExecutionLayout,
        UnsupportedGridSynchronization {
            block: usize,
            operation: usize,
        },
        PartialBarrierParticipants {
            scope: HierarchyAttr,
            dimension: usize,
            global_extent: u64,
            workgroup_extent: u64,
        },
        ResourceLimit,
    }
    #[allow(dead_code)]
    enum ProvenanceFailure {
        NativeObligations,
        NativeGuardResource,
        ResourceLimit {
            limit: usize,
            actual: usize,
        },
        MissingViewDefinition {
            view: String,
        },
        ForeignViewDefinition {
            view: String,
        },
        MissingMemorySpace {
            view: String,
        },
        ClaimedNoAliasWithoutOrigin {
            subject: String,
            class: u64,
        },
        InconsistentClassForOrigin {
            origin: u64,
            first: u64,
            second: u64,
        },
        UnknownWritableAlias {
            memory_space: MemorySpaceAttr,
        },
        MissingRelativeOffset {
            memory_space: MemorySpaceAttr,
            class: u64,
            origins: Vec<u64>,
        },
        IncompatibleViewSignature {
            memory_space: MemorySpaceAttr,
            class: u64,
        },
    }
    #[allow(dead_code)]
    enum MemoryFailure {
        NativeObligations,
        UnresolvedAddress {
            location: (usize, usize),
        },
        MismatchedBarrierPhase {
            grid: u64,
            workgroup: u64,
            epoch: usize,
        },
        SubgroupPublicationUnsupported {
            location: (usize, usize),
        },
        FencePublicationUnsupported {
            location: (usize, usize),
        },
        VersionLimitExceeded,
        PublicationEdgeLimitExceeded,
        IssueLimitExceeded,
    }
    #[allow(dead_code)]
    enum MemoryAnalysisFailure {
        Trace(TraceFailure),
        Provenance(String),
        MemoryOrder(MemoryFailure),
    }
    #[allow(dead_code)]
    enum SimtFailure {
        Trace(TraceFailure),
    }
    type Manager = (
        Ptr<Operation>,
        Option<Key>,
        Option<[bool; 6]>,
        Option<envelope::ProductionAnalysisResourceLimitV1>,
        Option<envelope::ProductionAnalysisInputCensusV1>,
        envelope::ProductionAnalysisResourceContractV1,
        usize,
        Option<
            Result<
                Arc<inventory::BoundedPlironFunctionInventoryV1>,
                inventory::BoundedPlironFunctionInventoryFailureV1,
            >,
        >,
        Option<Result<crate::SparseIndexAnalysisV1, crate::SparseIndexFailureV1>>,
        Option<Result<presburger::PlironPresburgerAnalysisV1, crate::SparseIndexFailureV1>>,
        Option<Result<provenance::PlironProvenanceAliasAnalysisV1, ProvenanceFailure>>,
        Option<Result<Option<trace::PlironExecutionLayoutV1>, TraceFailure>>,
        Option<Result<Vec<trace::PlironInvocationTraceV1>, TraceFailure>>,
        Option<
            Result<
                tensor::PlironTensorLayoutDataflowAnalysisV1,
                tensor::PlironTensorLayoutDataflowFailureV1,
            >,
        >,
        Option<Result<memory::PlironMemoryOrderAnalysisV1, MemoryAnalysisFailure>>,
        Option<Result<simt::PlironSimtProtocolAnalysisV1, SimtFailure>>,
    );
    #[allow(dead_code)]
    enum Event {
        NativeSubject {
            location: (usize, usize),
            occurrence: usize,
            kind: event::NativeEventKindV1,
            address: Option<Address>,
        },
        NativeBarrier {
            location: (usize, usize),
            occurrence: usize,
            execution_scope: HierarchyAttr,
            address_spaces: u8,
        },
        NativeFence {
            location: (usize, usize),
            occurrence: usize,
            address_spaces: u8,
        },
        Barrier {
            location: (usize, usize),
            execution_scope: HierarchyAttr,
            memory_scope: MemoryScopeAttr,
            address_space: AddressSpaceAttr,
            order: MemoryOrderAttr,
        },
        Fence {
            location: (usize, usize),
            memory_scope: MemoryScopeAttr,
            address_space: AddressSpaceAttr,
            order: MemoryOrderAttr,
        },
        TensorInstruction {
            location: (usize, usize),
            subgroup_width: u16,
            claimed_active_lanes: u32,
        },
        Trap {
            location: (usize, usize),
        },
        Memory {
            location: (usize, usize),
            view: Value,
            memory_space: MemorySpaceAttr,
            access: AccessKindAttr,
            atomic_ordering: Option<AtomicOrderingAttr>,
            atomic_scope: Option<AtomicScopeAttr>,
            indices: Vec<Option<u64>>,
            allocation_origin: u64,
            noalias_class: u64,
            view_signature: (u32, Vec<u64>),
        },
        CollectiveAllocation {
            location: (usize, usize),
            access: AccessKindAttr,
            memory_space: MemorySpaceAttr,
            allocation_origin: u64,
            noalias_class: u64,
        },
    }
    fn check<A, E>(name: &str) {
        let actual = (size_of::<A>(), align_of::<A>());
        let expected = (size_of::<E>(), align_of::<E>());
        assert_eq!(
            actual, expected,
            "V909 {name}: invalidate, never retune resource totals"
        );
        eprintln!(
            "V885_V909_LAYOUT {name} size={} align={}",
            expected.0, expected.1
        );
    }
    assert_eq!((size_of::<usize>(), align_of::<usize>()), (8, 8));
    assert_eq!(
        (size_of::<Option<i128>>(), align_of::<Option<i128>>()),
        (32, 16)
    );
    check::<input::NativeTraceKeyV1, Key>("native key");
    check::<Option<input::NativeTraceKeyV1>, Option<Key>>("optional native key");
    check::<input::NativeTraceObligationsV1, [bool; 6]>("native obligations");
    check::<Option<input::NativeTraceObligationsV1>, Option<[bool; 6]>>("optional obligations");
    check::<event::NativeAddressV1, Address>("native address");
    check::<Option<event::NativeAddressV1>, Option<Address>>("optional native address");
    check::<trace::PlironTraceFailureV1, TraceFailure>("trace failure");
    check::<provenance::PlironProvenanceFailureV1, ProvenanceFailure>("provenance failure");
    check::<memory::PlironMemoryOrderFailureV1, MemoryFailure>("memory failure");
    check::<manager::PlironMemoryOrderAnalysisFailureV1, MemoryAnalysisFailure>(
        "memory cache failure",
    );
    check::<manager::PlironSimtProtocolAnalysisFailureV1, SimtFailure>("SIMT cache failure");
    check::<
        Option<Result<Vec<trace::PlironInvocationTraceV1>, trace::PlironTraceFailureV1>>,
        Option<Result<Vec<trace::PlironInvocationTraceV1>, TraceFailure>>,
    >("trace cache slot");
    check::<trace::PlironTraceEventV1, Event>("event");
    check::<trace::PlironInvocationTraceV1, (Vec<u64>, u64, u64, u64, u64, Vec<Event>)>(
        "trace header",
    );
    check::<manager::PlironAnalysisManagerV1, Manager>("complete manager");
    assert_eq!(manager::MAX_PLIRON_ANALYSIS_CACHE_SLOTS_V1, 9);
    // The distinct ordinary resource domain counts logical event/index items.
    // Neither this sizeof nor native folder APInt scratch enters that formula.
    assert_eq!(1 + 2 * dialect_kernel::MAX_RANKED_MEMORY_RANK, 17);
}

#[test]
fn private_call_whole_entry_real_facade_internal_panics_keep_native_prefix_and_kir_floor() {
    use crate::production_analysis::{
        pliron_analysis_manager as manager, pliron_pipeline as pipeline,
    };
    use numbers::Triple;
    let first = Oracle::derive(0);
    let full = numbers::module();
    let after_sparse = first
        .gates
        .iter()
        .find(|gate| gate.name == "sparse full")
        .unwrap()
        .after;
    let expected = [Triple::ZERO, after_sparse.held(), first.tensor_panic()];
    for (mode, prefix) in expected.into_iter().enumerate() {
        with_checked(&fixture(), |checked, budget| {
            let floor = budget.storage();
            let ledger = budget.work_ledger_identity_v1();
            let reset = match mode {
                0 => {
                    pipeline::panic_next_production_analysis_for_test_v1();
                    None
                }
                1 => {
                    manager::panic_next_analysis_manager_prepare_for_test_v1();
                    None
                }
                _ => Some(pipeline::panic_after_first_production_stage_for_test_v1()),
            };
            resources::cleanup_trace_start();
            let error = with_memory_checks::<()>(
                checked,
                tests::limits(),
                budget,
                oracle::exact_limits(full),
                |_, _| panic!("internal panic must precede callback"),
            )
            .unwrap_err();
            drop(reset);
            if mode == 2 {
                use crate::production_analysis::pliron_pass_contract::PlironPassPreservationErrorV1;
                assert!(matches!(
                    error.failure(),
                    Failure::Analysis {
                        function: 0,
                        cause: ProductionPlironPreloweringErrorV2::Preservation(
                            PlironPassPreservationErrorV1::AnalysisPanicked {
                                pass: crate::KernelCheckPassKindV1::TensorLayout
                            }
                        )
                    }
                ));
            } else {
                assert!(matches!(error.failure(), Failure::Panicked));
            }
            // Early failure does not execute release_reports; the diagnostic
            // envelope remains held even though the KIR scope restores its floor.
            oracle::assert_observation(error.observation(), prefix, None, true);
            let history = error.last_invocation().unwrap();
            assert_eq!(history.function(), 0);
            oracle::assert_observation(history.floor(), Triple::ZERO, None, false);
            oracle::assert_observation(history.invocation(), prefix, None, true);
            assert_eq!(budget.storage(), floor);
            assert!(budget.work_ledger_identity_v1() == ledger);
            let cleanup = resources::cleanup_trace_take();
            assert_eq!(
                cleanup.len(),
                1,
                "no callback-owned payload exists on an internal panic"
            );
            assert_eq!(cleanup[0].0, "before refund");
            assert!(cleanup[0].1 > floor);
            // A fresh complete invocation proves one-shot hooks have reset.
            with_memory_checks(
                checked,
                tests::limits(),
                budget,
                oracle::exact_limits(full),
                |view, budget| {
                    oracle::assert_observation(
                        view.trap_policies(budget)?.observation(budget)?,
                        full,
                        None,
                        false,
                    );
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(budget.storage(), floor);
        });
    }
}
