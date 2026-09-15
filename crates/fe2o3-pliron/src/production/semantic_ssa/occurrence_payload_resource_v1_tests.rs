use super::*;
use crate::production::semantic_ssa::occurrences_v1::capture_resource_row_sizes_for_test;
use crate::{
    ProductionSemanticSsaConstantOccurrenceV1, ProductionSemanticSsaEdgeDefinitionOccurrenceV1,
    ProductionSemanticSsaEntryDefinitionOccurrenceV1, ProductionSemanticSsaEventOccurrenceV1,
    ProductionSemanticSsaOccurrenceStorageV1, ProductionSemanticSsaSuccessorOccurrenceV1,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
use std::{
    mem::{align_of, size_of},
    ops::Range,
};

const PREFIX: usize = 7;
const FLOOR: usize = 11;
const START: usize = 1 + 1;
const INPUT: usize = 2 + 1;
const FINISH: usize = 3;
const JOIN_FIXED: usize = 1 + 1 + 1 + 3 + 1 + 1 + 1;
const JOIN_BLOCK: usize = 1 + 7 + 2 + 1 + 1;
const JOIN_EDGE: usize = 1 + 7 + 1 + 1 + 1;
const JOIN_EVENT: usize = 1 + 4 + 1 + 3 + 1;
const JOIN_UNRESOLVED_EVENT: usize = 1 + 4 + 1 + 1;
const JOIN_DEFINITION: usize = 1 + 4 + 1 + 1 + 1;
const JOIN_UNRESOLVED_DEFINITION: usize = 1 + 4 + 1 + 1;
const JOIN_ENTRY: usize = 1 + 2 + 1 + 1 + 1;

// sparse_owner's seven statements have grammar visits [5,4,5,6,5,1;5]:
// aggregate; scalar; indexed destination; indexed Copy; Move; StorageDead;
// unreachable Copy. Three Block/Terminator pairs add six. Each block pass
// also pays prepass3, lookup2 per statement, event2, constant2, successor3,
// terminal4+row1 per block, and observer finish1. The five event rows
// without resolution include three unpromoted rows and two unreachable rows.
const SPARSE_GRAMMAR: usize = 5 + 4 + 5 + 6 + 5 + 1 + 5 + 3 * 2;
const SPARSE_COUNT: usize = 3 + SPARSE_GRAMMAR + 7 * 2 + 13 * 2 + 4 * 2 + 2 * 3 + 3 * 5 + 1;
const SPARSE_ARRAYS: usize = 4;
const SPARSE_FILL: usize = SPARSE_COUNT + 7;
const SPARSE_ENTRY_COUNT: usize = 3 + 5 * 2 + 1 + 1;
const SPARSE_ENTRY_FILL: usize = 3 + 5 * 2 + 2 + 1 + 7;
const SPARSE_PREJOIN: usize = START
    + INPUT
    + SPARSE_COUNT
    + SPARSE_ARRAYS
    + SPARSE_FILL
    + SPARSE_ENTRY_COUNT
    + SPARSE_ENTRY_FILL;
const SPARSE_JOIN: usize =
    JOIN_FIXED + 3 * JOIN_BLOCK + 2 * JOIN_EDGE + 8 * JOIN_EVENT + 5 * JOIN_UNRESOLVED_EVENT;
const SPARSE_WORK: usize = SPARSE_PREJOIN + SPARSE_JOIN + FINISH;

// edge_owner's root has two constant assignments (8 visits), the Switch
// operand/place (2), two calls (5 and 3), and four Block/Terminator pairs.
// Its seven distinct successor ordinals include two normal-return definitions;
// only the reachable one resolves. No entry is defined for the root.
const ROOT_GRAMMAR: usize = 8 + 2 + 5 + 3 + 4 * 2;
const ROOT_COUNT: usize = 3 + ROOT_GRAMMAR + 2 * 2 + 5 * 2 + 6 * 2 + 7 * 3 + 2 + 4 * 5 + 1;
const ROOT_ARRAYS: usize = 5;
const ROOT_FILL: usize = ROOT_COUNT + 2 * 5 + 7;
const ROOT_ENTRY_COUNT: usize = 3 + 3 * 2 + 1 + 1;
const ROOT_ENTRY_FILL: usize = 3 + 3 * 2 + 2 + 1 + 7;
const ROOT_JOIN: usize = JOIN_FIXED
    + 4 * JOIN_BLOCK
    + 7 * JOIN_EDGE
    + 5 * JOIN_EVENT
    + JOIN_DEFINITION
    + JOIN_UNRESOLVED_DEFINITION;
const ROOT_END: usize = START
    + INPUT
    + ROOT_COUNT
    + ROOT_ARRAYS
    + ROOT_FILL
    + ROOT_ENTRY_COUNT
    + ROOT_ENTRY_FILL
    + ROOT_JOIN;
// The actual helper has one Return block, four locals and three argument
// entries, despite its ignored Unit ABI slots. Each entry is promoted.
const HELPER_COUNT: usize = 3 + 2 + 4 + 1 + 1;
const HELPER_FILL: usize = HELPER_COUNT + 7;
const HELPER_ENTRY_COUNT: usize = 3 + 4 * 2 + 3 * 2 + 1 + 1;
const HELPER_ENTRY_FILL: usize = 3 + 4 * 2 + 3 * 2 + 2 + 1 + 7;
const HELPER_JOIN: usize = JOIN_FIXED + JOIN_BLOCK + 3 * JOIN_ENTRY;
const EDGE_WORK: usize = ROOT_END
    + INPUT
    + HELPER_COUNT
    + 1
    + HELPER_FILL
    + HELPER_ENTRY_COUNT
    + 1
    + HELPER_ENTRY_FILL
    + HELPER_JOIN
    + FINISH;

#[derive(Clone, Copy)]
struct Layout {
    header: usize,
    function: usize,
    block: usize,
    event: usize,
    constant: usize,
    successor: usize,
    edge_definition: usize,
    entry: usize,
}

impl Layout {
    fn sparse(self) -> usize {
        self.header
            + self.function
            + 3 * self.block
            + 13 * self.event
            + 4 * self.constant
            + 2 * self.successor
    }

    fn edges(self) -> usize {
        self.header
            + 2 * self.function
            + 5 * self.block
            + 5 * self.event
            + 6 * self.constant
            + 7 * self.successor
            + 2 * self.edge_definition
            + 3 * self.entry
    }
}

fn layout() -> Layout {
    let vector = size_of::<Vec<u8>>();
    let word = size_of::<usize>();
    assert_eq!(vector, 3 * word);
    assert_eq!(size_of::<ProductionSemanticSsaOccurrenceStorageV1>(), word);
    let aligned = |bytes: usize, alignment: usize| bytes.div_ceil(alignment) * alignment;
    // Independently check the three private layouts against their actual
    // fields, not a production storage formula. Other rows are public types.
    let header = vector + word;
    let function = aligned(
        size_of::<SemanticFunctionIdV1>() + 7 * vector,
        align_of::<Vec<u8>>(),
    );
    let block = aligned(
        size_of::<SsaBlockIdV1>() + 2 * size_of::<Range<usize>>(),
        align_of::<Range<usize>>(),
    );
    assert_eq!(
        capture_resource_row_sizes_for_test(),
        [header, function, block]
    );
    Layout {
        header,
        function,
        block,
        event: size_of::<ProductionSemanticSsaEventOccurrenceV1>(),
        constant: size_of::<ProductionSemanticSsaConstantOccurrenceV1>(),
        successor: size_of::<ProductionSemanticSsaSuccessorOccurrenceV1>(),
        edge_definition: size_of::<ProductionSemanticSsaEdgeDefinitionOccurrenceV1>(),
        entry: size_of::<ProductionSemanticSsaEntryDefinitionOccurrenceV1>(),
    }
}

fn fixture(edges: bool) -> ProductionSemanticSsaOwnerV1 {
    let owner = if edges { edge_owner() } else { sparse_owner() };
    let expected: &[(usize, usize, usize)] = if edges {
        &[(3, 4, 2), (4, 1, 0)]
    } else {
        &[(5, 3, 7)]
    };
    assert_eq!(owner.source_semantic().functions().len(), expected.len());
    for (function, &(locals, blocks, statements)) in
        owner.source_semantic().functions().iter().zip(expected)
    {
        assert_eq!(function.locals().len(), locals);
        assert_eq!(function.blocks().len(), blocks);
        assert_eq!(
            function
                .blocks()
                .iter()
                .map(|block| block.statements().len())
                .sum::<usize>(),
            statements
        );
    }
    assert_eq!(owner.summary().input_blocks(), if edges { 5 } else { 3 });
    assert_eq!(owner.summary().input_events(), if edges { 5 } else { 13 });
    assert_eq!(owner.summary().input_edges(), if edges { 7 } else { 2 });
    assert_eq!(
        owner.summary().input_edge_definitions(),
        if edges { 2 } else { 0 }
    );
    assert!(owner.occurrences_v1().is_none());
    assert_eq!(owner.occurrence_storage(), None);
    owner
}

fn initial_budget(work: &mut Work, storage_limit: usize) -> Budget<'_> {
    let mut budget = Budget::new(work, storage_limit);
    budget.charge_work(PREFIX).unwrap();
    budget.reserve_storage(FLOOR).unwrap();
    budget
}

#[test]
fn payload_capture_exact_work_and_storage_follow_actual_source_phases() {
    let sizes = layout();
    assert_eq!((SPARSE_GRAMMAR, ROOT_GRAMMAR), (37, 26));
    assert_eq!((SPARSE_COUNT, SPARSE_FILL, SPARSE_JOIN), (110, 117, 182));
    assert_eq!((ROOT_COUNT, ROOT_FILL, ROOT_JOIN), (99, 116, 199));
    assert_eq!((ROOT_END, HELPER_JOIN), (454, 39));
    assert_eq!((SPARSE_WORK, EDGE_WORK), (459, 576));
    for edges in [false, true] {
        // Source, classification, replay and planner allocations retain their
        // inherited scope. This ledger covers only the new capture storage/work.
        let mut owner = fixture(edges);
        let identity = owner.identity();
        let total = if edges { EDGE_WORK } else { SPARSE_WORK };
        let retained = if edges { sizes.edges() } else { sizes.sparse() };
        let mut work = Work::new(PREFIX + total);
        {
            let mut budget = initial_budget(&mut work, FLOOR + retained);
            let receipt = owner
                .try_capture_occurrences_with_budget_v1(&mut budget)
                .unwrap();
            assert_eq!(receipt.retained_storage(), retained);
            assert_eq!(budget.work(), PREFIX + total);
            assert_eq!(budget.storage(), FLOOR);
            assert_eq!(budget.peak_storage(), FLOOR + retained);
            assert_eq!(budget.failed_storage(), None);
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            let view = owner.occurrences_v1().unwrap();
            let root = view.function(SemanticFunctionIdV1::from_index(0)).unwrap();
            assert_eq!(root.events().len(), if edges { 5 } else { 13 });
            assert_eq!(root.constants().len(), if edges { 6 } else { 4 });
            assert_eq!(root.edge_definitions().len(), if edges { 2 } else { 0 });
            if edges {
                assert_eq!(
                    view.function(SemanticFunctionIdV1::from_index(1))
                        .unwrap()
                        .entry_definitions()
                        .len(),
                    3
                );
            }
            assert_eq!(owner.occurrence_storage(), Some(receipt));
            assert_eq!(owner.identity(), identity);
            assert!(!owner.grants_proof_or_artifact_authority());
            drop(owner);
            budget.release_storage(receipt.retained_storage()).unwrap();
            assert_eq!(budget.storage(), FLOOR);
        }
        assert_eq!(work.failed_work(), None);
    }
}

#[test]
fn payload_capture_one_under_final_work_publishes_no_partial_rows() {
    let sizes = layout();
    for edges in [false, true] {
        let mut owner = fixture(edges);
        let identity = owner.identity();
        let total = if edges { EDGE_WORK } else { SPARSE_WORK };
        let retained = if edges { sizes.edges() } else { sizes.sparse() };
        let mut work = Work::new(PREFIX + total - 1);
        {
            let mut budget = initial_budget(&mut work, FLOOR + retained);
            let Err(CaptureError::Resource(Resource::Work(error))) =
                owner.try_capture_occurrences_with_budget_v1(&mut budget)
            else {
                panic!("the final three-unit charge must be denied")
            };
            assert_eq!(error.actual(), PREFIX + total);
            assert_eq!(error.limit(), PREFIX + total - 1);
            assert_eq!(budget.work(), PREFIX + total - FINISH);
            assert_eq!(budget.storage(), FLOOR);
            assert_eq!(budget.peak_storage(), FLOOR + retained);
            assert_eq!(budget.failed_storage(), None);
            assert!(owner.occurrences_v1().is_none());
            assert_eq!(owner.occurrence_storage(), None);
            assert_eq!(owner.identity(), identity);
        }
        assert_eq!(work.failed_work(), Some(PREFIX + total));
        owner.verify_replay().unwrap();
    }
}

#[test]
fn payload_capture_denies_each_new_nonempty_row_allocation_before_fill() {
    let s = layout();
    let sparse_before_events = s.header + s.function + 3 * s.block;
    let sparse_events = sparse_before_events + 13 * s.event;
    let sparse_constants = sparse_events + 4 * s.constant;
    let root_before_definitions =
        s.header + 2 * s.function + 4 * s.block + 5 * s.event + 6 * s.constant + 7 * s.successor;
    let root_definitions = root_before_definitions + 2 * s.edge_definition;
    let sparse_event_work = START + INPUT + SPARSE_COUNT + 2;
    let sparse_constant_work = sparse_event_work + 1;
    let edge_definition_work = START + INPUT + ROOT_COUNT + ROOT_ARRAYS;
    let helper_entry_work =
        ROOT_END + INPUT + HELPER_COUNT + 1 + HELPER_FILL + HELPER_ENTRY_COUNT + 1;
    assert_eq!((sparse_event_work, sparse_constant_work), (117, 118));
    assert_eq!((edge_definition_work, helper_entry_work), (109, 507));
    for (edges, accepted, prior, attempted) in [
        (
            false,
            sparse_event_work,
            sparse_before_events,
            sparse_events,
        ),
        (false, sparse_constant_work, sparse_events, sparse_constants),
        (
            true,
            edge_definition_work,
            root_before_definitions,
            root_definitions,
        ),
        (true, helper_entry_work, s.edges() - 3 * s.entry, s.edges()),
    ] {
        let mut owner = fixture(edges);
        let identity = owner.identity();
        let total = if edges { EDGE_WORK } else { SPARSE_WORK };
        let mut work = Work::new(PREFIX + total);
        {
            let mut budget = initial_budget(&mut work, FLOOR + attempted - 1);
            let Err(CaptureError::Resource(Resource::Storage(error))) =
                owner.try_capture_occurrences_with_budget_v1(&mut budget)
            else {
                panic!("the selected fixed row payload must be denied")
            };
            assert_eq!(error.actual(), FLOOR + attempted);
            assert_eq!(error.limit(), FLOOR + attempted - 1);
            assert_eq!(budget.work(), PREFIX + accepted);
            assert_eq!(budget.storage(), FLOOR);
            assert_eq!(budget.peak_storage(), FLOOR + prior);
            assert_eq!(budget.failed_storage(), Some(FLOOR + attempted));
            assert!(owner.occurrences_v1().is_none());
            assert_eq!(owner.occurrence_storage(), None);
            assert_eq!(owner.identity(), identity);
        }
        assert_eq!(work.failed_work(), None);
        owner.verify_replay().unwrap();
    }
}

#[test]
fn payload_capture_join_denial_restores_floor_and_preserves_first_failures() {
    let retained = layout().sparse();
    let mut owner = fixture(false);
    let identity = owner.identity();
    // After count/fill274, take/function/block-count3 and the first block's
    // visit/ranges/lengths/reachability11 precede D1's unpromoted join7.
    // D2's visit/input/advance/resolved-key checks9 end at304; its one-unit
    // resolution write is denied at305. The earlier D1 row was already joined.
    let accepted =
        SPARSE_PREJOIN + (1 + 1 + 1) + (1 + 7 + 2 + 1) + JOIN_UNRESOLVED_EVENT + (1 + 4 + 1 + 3);
    let attempted = accepted + 1;
    assert_eq!((SPARSE_PREJOIN, accepted, attempted), (274, 304, 305));
    let work_limit = PREFIX + accepted;
    let storage_limit = FLOOR + retained;
    let mut work = Work::new(work_limit);
    let first_work;
    {
        let mut budget = initial_budget(&mut work, storage_limit);
        let Err(Resource::Work(error)) = budget.charge_work(work_limit + 1) else {
            panic!("the seed work request must exceed the fixed limit")
        };
        first_work = error.actual();
        assert_eq!(first_work, PREFIX + work_limit + 1);
        let Err(Resource::Storage(error)) = budget.reserve_storage(storage_limit + 1) else {
            panic!("the seed storage request must exceed the fixed limit")
        };
        let first_storage = error.actual();
        assert_eq!(first_storage, FLOOR + storage_limit + 1);
        let Err(CaptureError::Resource(Resource::Work(error))) =
            owner.try_capture_occurrences_with_budget_v1(&mut budget)
        else {
            panic!("the first promoted resolution write must be denied")
        };
        assert_eq!(error.actual(), PREFIX + attempted);
        assert_eq!(error.limit(), work_limit);
        assert_eq!(budget.work(), PREFIX + accepted);
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.peak_storage(), FLOOR + retained);
        assert_eq!(budget.failed_storage(), Some(first_storage));
        assert!(owner.occurrences_v1().is_none());
        assert_eq!(owner.occurrence_storage(), None);
        assert_eq!(owner.identity(), identity);
    }
    assert_eq!(work.failed_work(), Some(first_work));
    owner.verify_replay().unwrap();
}
