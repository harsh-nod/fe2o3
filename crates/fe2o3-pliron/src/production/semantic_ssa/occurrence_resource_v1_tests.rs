use super::*;
use crate::production::semantic_ssa::occurrences_v1::{
    capture_pending_with_cleanup, capture_resource_row_sizes_for_test,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_mir_model::SsaEdgeIdV1;
use std::{
    cell::Cell,
    mem::{align_of, size_of},
    ops::Range,
};

type CaptureError = ProductionSemanticSsaOccurrenceErrorV1;
const PREFIX: usize = 7;
const FLOOR: usize = 11;
const PRIOR_SCRATCH: usize = 13;
const FINISH: usize = 3;

// Independent phase census of the two existing admitted source fixtures.
// Input: two shape checks plus Function. Empty block pass: prepass3, two
// grammar visits, terminal4, block row1, observer finish1. Real fill adds
// seven count comparisons. Entry count/fill include prepass3, local visit2,
// input checks1/2, observer finish1 and (fill only) seven count comparisons.
const START: usize = 1 + 1;
const INPUT: usize = 2 + 1;
const ONE_COUNT: usize = 3 + 2 + 4 + 1 + 1;
const ONE_FILL: usize = ONE_COUNT + 7;
const ENTRY_COUNT: usize = 3 + 2 + 1 + 1;
const ENTRY_FILL: usize = 3 + 2 + 2 + 1 + 7;
// Join: take/function/initial blocks checks, per-block visit/ranges/lengths/
// reachability/cursor, three final ranges, entry length/cursor, outer push.
const ONE_JOIN: usize = 1 + 1 + 1 + (1 + 7 + 2 + 1 + 1) + 3 + 1 + 1 + 1;
const ONE_FUNCTION: usize = INPUT + ONE_COUNT + 1 + ONE_FILL + ENTRY_COUNT + ENTRY_FILL + ONE_JOIN;
const ONE_WORK: usize = START + ONE_FUNCTION + FINISH;
// The second root has two blocks and one Goto edge. Each pass adds two
// grammar visits, terminal4+row1, and successor checks2+row1. Joining adds
// one block plus edge visit1/keys7/length1/reachability1/cursor1.
const TWO_COUNT: usize = ONE_COUNT + 2 + 4 + 1 + 2 + 1;
const TWO_FILL: usize = TWO_COUNT + 7;
const TWO_JOIN: usize = ONE_JOIN + (1 + 7 + 2 + 1 + 1) + (1 + 7 + 1 + 1 + 1);
const SECOND_FUNCTION: usize =
    INPUT + TWO_COUNT + 2 + TWO_FILL + ENTRY_COUNT + ENTRY_FILL + TWO_JOIN;
const TWO_WORK: usize = START + ONE_FUNCTION + SECOND_FUNCTION + FINISH;

#[derive(Clone, Copy)]
struct Layout {
    header: usize,
    function: usize,
    block: usize,
    successor: usize,
}

impl Layout {
    fn receipt(self, two: bool) -> usize {
        if two {
            self.header + 2 * self.function + 3 * self.block + self.successor
        } else {
            self.header + self.function + self.block
        }
    }
}

fn aligned(bytes: usize, alignment: usize) -> usize {
    bytes.div_ceil(alignment) * alignment
}

fn independent_layout() -> Layout {
    let vector = size_of::<Vec<u8>>();
    let vector_alignment = align_of::<Vec<u8>>();
    assert_eq!(vector, 3 * size_of::<usize>());
    assert_eq!(
        size_of::<ProductionSemanticSsaOccurrenceStorageV1>(),
        size_of::<usize>()
    );
    // Attachment is Vec + one-word receipt; its Option uses the Vec niche.
    // FunctionRows is its exact function ID + seven complete Vec headers.
    // BlockRows is its exact block ID + two usize ranges, including padding.
    let header = vector + size_of::<ProductionSemanticSsaOccurrenceStorageV1>();
    let function = aligned(
        size_of::<SemanticFunctionIdV1>() + 7 * vector,
        vector_alignment,
    );
    let block = aligned(
        size_of::<SsaBlockIdV1>() + 2 * size_of::<Range<usize>>(),
        align_of::<Range<usize>>(),
    );
    assert_eq!(
        capture_resource_row_sizes_for_test(),
        [header, function, block]
    );
    let successor = aligned(
        size_of::<SsaEdgeIdV1>()
            + size_of::<SemanticControlFlowEdgeV1>()
            + size_of::<Range<usize>>(),
        align_of::<Range<usize>>(),
    );
    assert_eq!(
        size_of::<ProductionSemanticSsaSuccessorOccurrenceV1>(),
        successor
    );
    assert!(header > PRIOR_SCRATCH);
    Layout {
        header,
        function,
        block,
        successor,
    }
}

fn source_owner(two: bool) -> ProductionSemanticSsaOwnerV1 {
    let owner = if two {
        two_root_owner()
    } else {
        let source = ProductionSemanticMirOwnerV1::try_new(
            admitted_single_function_semantic(),
            ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap();
        ProductionSemanticSsaOwnerV1::try_new(source, ProductionSemanticSsaLimitsV1::default())
            .unwrap()
    };
    let expected_blocks: &[usize] = if two { &[1, 2] } else { &[1] };
    assert_eq!(
        owner.source_semantic().functions().len(),
        expected_blocks.len()
    );
    for (function, &blocks) in owner
        .source_semantic()
        .functions()
        .iter()
        .zip(expected_blocks)
    {
        assert_eq!(function.locals().len(), 1);
        assert!(matches!(
            function.locals()[0].role(),
            SemanticLocalRoleV1::Return
        ));
        assert!(matches!(
            function.abi().return_value().mode(),
            SemanticAbiPassModeV1::Ignore
        ));
        assert_eq!(function.blocks().len(), blocks);
        for (ordinal, block) in function.blocks().iter().enumerate() {
            assert!(block.statements().is_empty());
            if ordinal + 1 == blocks {
                assert!(matches!(
                    block.terminator().kind(),
                    SemanticTerminatorKindV1::Return
                ));
            } else {
                let SemanticTerminatorKindV1::Goto(edge) = block.terminator().kind() else {
                    panic!("the second root must contain its one actual Goto")
                };
                assert_eq!(edge.role(), SemanticEdgeRoleV1::Goto);
                assert_eq!(edge.target(), SemanticBlockIdV1::from_index(1));
            }
        }
    }
    assert_eq!(owner.summary().input_blocks(), if two { 3 } else { 1 });
    assert_eq!(owner.summary().input_edges(), usize::from(two));
    assert_eq!(owner.summary().input_events(), 0);
    assert_eq!(owner.summary().input_edge_definitions(), 0);
    assert!(
        owner
            .plans()
            .iter()
            .all(|row| row.plan().entry_definitions().is_empty())
    );
    assert!(owner.occurrences_v1().is_none());
    owner
}

fn initial_budget(work: &mut Work, storage: usize) -> Budget<'_> {
    let mut budget = Budget::new(work, storage);
    budget.charge_work(PREFIX).unwrap();
    budget.reserve_storage(FLOOR).unwrap();
    budget
}

fn seed_history(budget: &mut Budget<'_>, work_limit: usize, scratch: usize) -> (usize, usize) {
    budget.reserve_storage(scratch).unwrap();
    budget.release_storage(scratch).unwrap();
    let Err(Resource::Work(work)) = budget.charge_work(work_limit + 1) else {
        panic!("the independently oversized work request must fail")
    };
    let storage_limit = budget.storage_limit();
    let Err(Resource::Storage(storage)) = budget.reserve_storage(storage_limit + 1) else {
        panic!("the independently oversized storage request must fail")
    };
    assert_eq!(work.actual(), PREFIX + work_limit + 1);
    assert_eq!(storage.actual(), FLOOR + storage_limit + 1);
    (work.actual(), storage.actual())
}

#[test]
fn capture_exact_work_and_logical_storage_are_independently_derived() {
    let layout = independent_layout();
    assert_eq!((ONE_WORK, TWO_WORK), (81, 201));
    for two in [false, true] {
        // Existing MIR, planner and fixture allocations precede this ledger.
        let mut owner = source_owner(two);
        let identity = owner.identity();
        let required = layout.receipt(two);
        let expected_work = if two { TWO_WORK } else { ONE_WORK };
        let mut work = Work::new(PREFIX + expected_work);
        {
            let mut budget = initial_budget(&mut work, FLOOR + required);
            let receipt = owner
                .try_capture_occurrences_with_budget_v1(&mut budget)
                .unwrap();
            assert_eq!(receipt.retained_storage(), required);
            assert_eq!(budget.storage(), FLOOR);
            assert_eq!(budget.peak_storage(), FLOOR + required);
            assert_eq!(budget.work(), PREFIX + expected_work);
            assert_eq!(budget.failed_storage(), None);
            // The next controlled allocation first re-reserves the transfer.
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            assert_eq!(owner.occurrence_storage(), Some(receipt));
            assert_eq!(
                owner.occurrences_v1().unwrap().function_count(),
                if two { 2 } else { 1 }
            );
            owner.verify_replay().unwrap();
            assert_eq!(owner.identity(), identity);
            assert_eq!(budget.work(), PREFIX + expected_work);
            drop(owner);
            budget.release_storage(receipt.retained_storage()).unwrap();
            assert_eq!(budget.storage(), FLOOR);
        }
        assert_eq!(work.failed_work(), None);
    }
}

#[test]
fn capture_one_under_final_work_drops_all_pending_rows() {
    let layout = independent_layout();
    for two in [false, true] {
        let mut owner = source_owner(two);
        let total = if two { TWO_WORK } else { ONE_WORK };
        let required = layout.receipt(two);
        let mut work = Work::new(PREFIX + total - 1);
        {
            let mut budget = initial_budget(&mut work, FLOOR + required);
            let Err(CaptureError::Resource(Resource::Work(error))) =
                owner.try_capture_occurrences_with_budget_v1(&mut budget)
            else {
                panic!("the final three-unit admission must fail")
            };
            assert_eq!(error.actual(), PREFIX + total);
            assert_eq!(error.limit(), PREFIX + total - 1);
            assert_eq!(budget.work(), PREFIX + total - FINISH);
            assert_eq!(budget.storage(), FLOOR);
            assert_eq!(budget.peak_storage(), FLOOR + required);
            assert_eq!(budget.failed_storage(), None);
            assert!(owner.occurrences_v1().is_none());
            assert_eq!(owner.occurrence_storage(), None);
            owner.verify_replay().unwrap();
        }
        assert_eq!(work.failed_work(), Some(PREFIX + total));
    }
}

#[test]
fn capture_each_known_allocation_is_denied_before_its_buffer() {
    let l = independent_layout();
    let outer = l.header + 2 * l.function;
    let first_blocks = outer + l.block;
    let second_blocks = outer + 3 * l.block;
    // Attempted payload, prior admitted payload, accepted work at reservation.
    let phases = [
        (l.header, 0, 1),
        (outer, l.header, START),
        (first_blocks, outer, START + INPUT + ONE_COUNT + 1),
        (
            second_blocks,
            first_blocks,
            START + ONE_FUNCTION + INPUT + TWO_COUNT + 1,
        ),
        (
            l.receipt(true),
            second_blocks,
            START + ONE_FUNCTION + INPUT + TWO_COUNT + 2,
        ),
    ];
    assert_eq!(phases.map(|(_, _, work)| work), [1, 2, 17, 103, 104]);
    for (attempt, previous, accepted_work) in phases {
        let mut owner = source_owner(true);
        let mut work = Work::new(PREFIX + TWO_WORK);
        {
            let mut budget = initial_budget(&mut work, FLOOR + attempt - 1);
            budget.reserve_storage(PRIOR_SCRATCH).unwrap();
            budget.release_storage(PRIOR_SCRATCH).unwrap();
            let Err(CaptureError::Resource(Resource::Storage(error))) =
                owner.try_capture_occurrences_with_budget_v1(&mut budget)
            else {
                panic!("the independently selected storage phase must fail")
            };
            assert_eq!(error.actual(), FLOOR + attempt);
            assert_eq!(error.limit(), FLOOR + attempt - 1);
            assert_eq!(budget.work(), PREFIX + accepted_work);
            assert_eq!(budget.storage(), FLOOR);
            assert_eq!(budget.peak_storage(), FLOOR + previous.max(PRIOR_SCRATCH));
            assert_eq!(budget.failed_storage(), Some(FLOOR + attempt));
            assert!(owner.occurrences_v1().is_none());
            assert_eq!(owner.occurrence_storage(), None);
            owner.verify_replay().unwrap();
        }
        assert_eq!(work.failed_work(), None);
    }
}

#[test]
fn capture_transfer_and_already_captured_preserve_first_history() {
    let required = independent_layout().receipt(true);
    let mut owner = source_owner(true);
    let work_limit = PREFIX + TWO_WORK;
    let mut work = Work::new(work_limit);
    let first_work;
    {
        let scratch = required + PRIOR_SCRATCH;
        let mut budget = initial_budget(&mut work, FLOOR + scratch);
        let (failed_work, failed_storage) = seed_history(&mut budget, work_limit, scratch);
        first_work = failed_work;
        let receipt = owner
            .try_capture_occurrences_with_budget_v1(&mut budget)
            .unwrap();
        assert_eq!(receipt.retained_storage(), required);
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.peak_storage(), FLOOR + scratch);
        budget.reserve_storage(required).unwrap();
        let before = (
            budget.work(),
            budget.storage(),
            budget.peak_storage(),
            budget.failed_storage(),
        );
        assert!(matches!(
            owner.try_capture_occurrences_with_budget_v1(&mut budget),
            Err(CaptureError::AlreadyCaptured)
        ));
        assert_eq!(
            (
                budget.work(),
                budget.storage(),
                budget.peak_storage(),
                budget.failed_storage()
            ),
            before
        );
        assert_eq!(budget.work(), work_limit);
        assert_eq!(budget.failed_storage(), Some(failed_storage));
        // Actual consuming plain replay drops the capture, not the source owner.
        let source = owner.into_source_owner().unwrap();
        budget.release_storage(required).unwrap();
        assert_eq!(source.semantic().functions().len(), 2);
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.peak_storage(), FLOOR + scratch);
        drop(source);
    }
    assert_eq!(work.failed_work(), Some(first_work));
}

#[test]
fn capture_late_old_error_keeps_history_and_restores_the_valid_floor() {
    let required = independent_layout().receipt(true);
    for earlier_mismatch in [false, true] {
        let mut owner = source_owner(true);
        owner.limits = block_limits(2);
        if earlier_mismatch {
            owner.plans[0].auxiliary_resources.work_units += 1;
        }
        let work_limit = PREFIX + TWO_WORK;
        let mut work = Work::new(work_limit);
        let first_work;
        {
            let mut budget = initial_budget(&mut work, FLOOR + required);
            let (failed_work, failed_storage) =
                seed_history(&mut budget, work_limit, PRIOR_SCRATCH);
            first_work = failed_work;
            let Err(CaptureError::Replay(error)) =
                owner.try_capture_occurrences_with_budget_v1(&mut budget)
            else {
                panic!("the actual late aggregate limit must win")
            };
            assert_eq!(error, late_block_error());
            // Both joins completed before summary rejects block3; finish is not run.
            assert_eq!(budget.work(), PREFIX + TWO_WORK - FINISH);
            assert_eq!(budget.storage(), FLOOR);
            assert_eq!(budget.peak_storage(), FLOOR + required);
            assert_eq!(budget.failed_storage(), Some(failed_storage));
            assert!(owner.occurrences_v1().is_none());
            assert_eq!(owner.occurrence_storage(), None);
            assert_eq!(owner.verify_replay(), Err(late_block_error()));
        }
        assert_eq!(work.failed_work(), Some(first_work));
    }
}

// These probes exercise the same private cleanup helper used by the fixed API,
// not a fabricated capture or source fixture. Each owns a prepaid nine-byte
// allocation. Drop releases it before recording the still-reserved ledger,
// without risking a second panic during unwinding.
struct DropProbe<'a, 'w> {
    budget: &'a Budget<'w>,
    observed: &'a Cell<usize>,
    payload: Option<Box<[u8; 9]>>,
}

impl Drop for DropProbe<'_, '_> {
    fn drop(&mut self) {
        drop(self.payload.take());
        self.observed.set(self.budget.storage());
    }
}

#[test]
fn capture_cleanup_helper_drops_before_returned_error_floor_release() {
    const PAYLOAD: usize = 9;
    const BODY_WORK: usize = 5;
    let work_limit = PREFIX + BODY_WORK;
    let mut work = Work::new(work_limit);
    let observed = Cell::new(usize::MAX);
    let first_work;
    {
        let mut budget = initial_budget(&mut work, FLOOR + PRIOR_SCRATCH);
        let (failed_work, failed_storage) = seed_history(&mut budget, work_limit, PRIOR_SCRATCH);
        first_work = failed_work;
        let result: Result<(), CaptureError> =
            capture_pending_with_cleanup(&mut budget, |budget| {
                budget.charge_work(BODY_WORK)?;
                budget.reserve_storage(PAYLOAD)?;
                let _probe = DropProbe {
                    budget,
                    observed: &observed,
                    payload: Some(Box::new([0; PAYLOAD])),
                };
                Err(Resource::Allocation.into())
            });
        assert!(matches!(
            result,
            Err(CaptureError::Resource(Resource::Allocation))
        ));
        assert_eq!(observed.get(), FLOOR + PAYLOAD);
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.peak_storage(), FLOOR + PRIOR_SCRATCH);
        assert_eq!(budget.failed_storage(), Some(failed_storage));
        assert_eq!(budget.work(), work_limit);
    }
    assert_eq!(work.failed_work(), Some(first_work));
}

#[test]
fn capture_cleanup_helper_restores_before_resuming_the_original_panic() {
    const PAYLOAD: usize = 9;
    const BODY_WORK: usize = 5;
    const PANIC_TOKEN: u64 = 0x5343_4150_5455_5245;
    let work_limit = PREFIX + BODY_WORK;
    let mut work = Work::new(work_limit);
    let observed = Cell::new(usize::MAX);
    let first_work;
    {
        let mut budget = initial_budget(&mut work, FLOOR + PRIOR_SCRATCH);
        let (failed_work, failed_storage) = seed_history(&mut budget, work_limit, PRIOR_SCRATCH);
        first_work = failed_work;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            capture_pending_with_cleanup::<()>(&mut budget, |budget| {
                budget.charge_work(BODY_WORK)?;
                budget.reserve_storage(PAYLOAD)?;
                let _probe = DropProbe {
                    budget,
                    observed: &observed,
                    payload: Some(Box::new([0; PAYLOAD])),
                };
                std::panic::panic_any(PANIC_TOKEN)
            })
        }));
        let payload = result.expect_err("the cleanup helper must resume the panic");
        assert_eq!(*payload.downcast::<u64>().unwrap(), PANIC_TOKEN);
        assert_eq!(observed.get(), FLOOR + PAYLOAD);
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.peak_storage(), FLOOR + PRIOR_SCRATCH);
        assert_eq!(budget.failed_storage(), Some(failed_storage));
        assert_eq!(budget.work(), work_limit);
    }
    assert_eq!(work.failed_work(), Some(first_work));
}

#[test]
fn capture_cleanup_helper_does_not_invent_an_invalidated_floor() {
    let work_limit = PREFIX;
    let mut work = Work::new(work_limit);
    let first_work;
    {
        let mut budget = initial_budget(&mut work, FLOOR + PRIOR_SCRATCH);
        let (failed_work, failed_storage) = seed_history(&mut budget, work_limit, PRIOR_SCRATCH);
        first_work = failed_work;
        let result: Result<(), CaptureError> =
            capture_pending_with_cleanup(&mut budget, |budget| {
                // Deliberate private fault: normal fixed capture never releases caller storage.
                budget.release_storage(FLOOR)?;
                Err(Resource::Allocation.into())
            });
        assert!(matches!(
            result,
            Err(CaptureError::Resource(Resource::Accounting))
        ));
        assert_eq!(budget.storage(), 0);
        assert_eq!(budget.peak_storage(), FLOOR + PRIOR_SCRATCH);
        assert_eq!(budget.failed_storage(), Some(failed_storage));
        assert_eq!(budget.work(), PREFIX);
    }
    assert_eq!(work.failed_work(), Some(first_work));
}
