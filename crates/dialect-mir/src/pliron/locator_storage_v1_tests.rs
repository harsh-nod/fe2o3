use super::super::{
    MirProductionSemanticSha256V1, SemanticBlockIdV1, SemanticEdgeRoleV1, SemanticFunctionIdV1,
};
use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    Arithmetic,
    ByteLimit,
    ItemLimit,
}

#[derive(Debug)]
struct Ledger {
    bytes: usize,
    items: usize,
    max_bytes: usize,
    max_items: usize,
}

impl Ledger {
    fn new(max_bytes: usize, max_items: usize) -> Self {
        Self {
            bytes: 0,
            items: 0,
            max_bytes,
            max_items,
        }
    }

    fn charge(&mut self, count: usize, width: usize) -> Result<(), Refusal> {
        let term = count.checked_mul(width).ok_or(Refusal::Arithmetic)?;
        let bytes = self.bytes.checked_add(term).ok_or(Refusal::Arithmetic)?;
        let items = self.items.checked_add(1).ok_or(Refusal::Arithmetic)?;
        if bytes > self.max_bytes {
            return Err(Refusal::ByteLimit);
        }
        if items > self.max_items {
            return Err(Refusal::ItemLimit);
        }
        self.bytes = bytes;
        self.items = items;
        Ok(())
    }
}

fn fixture() -> MirProductionModuleLocatorV1 {
    let mut first_statements = Vec::with_capacity(11);
    first_statements.extend((0..3).map(MirProductionStatementLocatorV1::new));
    let mut first_arcs = Vec::with_capacity(17);
    first_arcs.push(MirProductionSuccessorArcV1::new(
        SemanticEdgeRoleV1::SwitchOtherwise,
        SemanticBlockIdV1::from_index(1),
    ));
    let first = MirProductionBlockLocatorV1::try_new(
        SemanticBlockIdV1::from_index(0),
        first_statements,
        MirProductionTerminatorLocatorV1::try_new(first_arcs).unwrap(),
    )
    .unwrap();
    let mut second_statements = Vec::with_capacity(13);
    second_statements.push(MirProductionStatementLocatorV1::new(0));
    let second = MirProductionBlockLocatorV1::try_new(
        SemanticBlockIdV1::from_index(1),
        second_statements,
        MirProductionTerminatorLocatorV1::try_new(Vec::with_capacity(19)).unwrap(),
    )
    .unwrap();
    let mut blocks = Vec::with_capacity(7);
    blocks.extend([first, second]);
    let function = MirProductionFunctionLocatorV1::try_new(
        SemanticFunctionIdV1::from_index(0),
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap();
    let mut functions = Vec::with_capacity(5);
    functions.push(function);
    MirProductionModuleLocatorV1::try_new(
        MirProductionSemanticSha256V1::from_sha256([0x49; 32]),
        functions,
    )
    .unwrap()
}

fn expected_heap(module: &MirProductionModuleLocatorV1) -> usize {
    let blocks = &module.functions[0].blocks;
    module.functions.capacity() * size_of::<MirProductionFunctionLocatorV1>()
        + blocks.capacity() * size_of::<MirProductionBlockLocatorV1>()
        + (blocks[0].statements.capacity() + blocks[1].statements.capacity())
            * size_of::<MirProductionStatementLocatorV1>()
        + (blocks[0].terminator.successors.capacity() + blocks[1].terminator.successors.capacity())
            * size_of::<MirProductionSuccessorArcV1>()
}

fn capacities(module: &MirProductionModuleLocatorV1) -> [usize; 6] {
    let blocks = &module.functions[0].blocks;
    [
        module.functions.capacity(),
        blocks.capacity(),
        blocks[0].statements.capacity(),
        blocks[0].terminator.successors.capacity(),
        blocks[1].statements.capacity(),
        blocks[1].terminator.successors.capacity(),
    ]
}

#[test]
fn locator_storage_actual_spare_capacities_match_independent_formula() {
    let module = fixture();
    let before = module.clone();
    let before_capacities = capacities(&module);
    assert!(module.functions.capacity() > module.functions.len());
    let mut ledger = Ledger::new(usize::MAX, usize::MAX);
    module
        .visit_logical_retained_storage_v1(&mut |count, width| ledger.charge(count, width))
        .unwrap();
    assert_eq!(
        ledger.bytes,
        size_of::<MirProductionModuleLocatorV1>() + expected_heap(&module)
    );
    assert_eq!(ledger.items, 7);
    assert_eq!(module, before);
    assert_eq!(capacities(&module), before_capacities);
}

#[test]
fn locator_storage_full_minus_heap_is_one_header_with_same_visits() {
    let module = fixture();
    let mut full = Ledger::new(usize::MAX, usize::MAX);
    let mut heap = Ledger::new(usize::MAX, usize::MAX);
    module
        .visit_logical_retained_storage_v1(&mut |n, w| full.charge(n, w))
        .unwrap();
    module
        .visit_logical_retained_heap_v1(&mut |n, w| heap.charge(n, w))
        .unwrap();
    assert_eq!(
        full.bytes - heap.bytes,
        size_of::<MirProductionModuleLocatorV1>()
    );
    assert_eq!(heap.bytes, expected_heap(&module));
    assert_eq!(full.items, heap.items);
}

#[test]
fn locator_storage_exact_and_one_short_limits() {
    let module = fixture();
    let bytes = size_of::<MirProductionModuleLocatorV1>() + expected_heap(&module);
    let mut exact = Ledger::new(bytes, 7);
    module
        .visit_logical_retained_storage_v1(&mut |n, w| exact.charge(n, w))
        .unwrap();
    let mut short_bytes = Ledger::new(bytes - 1, 7);
    assert_eq!(
        module.visit_logical_retained_storage_v1(&mut |n, w| short_bytes.charge(n, w)),
        Err(Refusal::ByteLimit),
    );
    let mut short_items = Ledger::new(bytes, 6);
    assert_eq!(
        module.visit_logical_retained_storage_v1(&mut |n, w| short_items.charge(n, w)),
        Err(Refusal::ItemLimit),
    );
}

#[test]
fn locator_storage_first_refusal_stops_each_full_and_heap_prefix() {
    let module = fixture();
    for heap_only in [false, true] {
        for stop in 0..7 {
            let mut calls = 0;
            let mut visit = |_: usize, _: usize| {
                let ordinal = calls;
                calls += 1;
                if ordinal == stop { Err(stop) } else { Ok(()) }
            };
            let result = if heap_only {
                module.visit_logical_retained_heap_v1(&mut visit)
            } else {
                module.visit_logical_retained_storage_v1(&mut visit)
            };
            assert_eq!(result, Err(stop));
            assert_eq!(calls, stop + 1);
        }
    }
}

#[test]
fn locator_storage_empty_buffers_still_charge_bounded_visits() {
    // Private ownership-only fixture: this is not constructor admission or authority.
    let module = MirProductionModuleLocatorV1 {
        semantic_sha256: MirProductionSemanticSha256V1::from_sha256([0; 32]),
        functions: Vec::new(),
    };
    let mut ledger = Ledger::new(0, 2);
    module
        .visit_logical_retained_heap_v1(&mut |n, w| ledger.charge(n, w))
        .unwrap();
    assert_eq!((ledger.bytes, ledger.items), (0, 2));
    let mut no_work = Ledger::new(usize::MAX, 0);
    assert_eq!(
        module.visit_logical_retained_heap_v1(&mut |n, w| no_work.charge(n, w)),
        Err(Refusal::ItemLimit),
    );
    assert_eq!((no_work.bytes, no_work.items), (0, 0));
}

#[test]
fn locator_storage_caller_arithmetic_overflow_is_propagated_without_later_work() {
    let module = fixture();
    let mut ledger = Ledger::new(usize::MAX, usize::MAX);
    ledger.bytes = usize::MAX;
    assert_eq!(
        module.visit_logical_retained_storage_v1(&mut |n, w| ledger.charge(n, w)),
        Err(Refusal::Arithmetic),
    );
    assert_eq!((ledger.bytes, ledger.items), (usize::MAX, 0));
    ledger.bytes = 0;
    ledger.items = usize::MAX;
    assert_eq!(
        module.visit_logical_retained_heap_v1(&mut |n, w| ledger.charge(n, w)),
        Err(Refusal::Arithmetic),
    );
    assert_eq!((ledger.bytes, ledger.items), (0, usize::MAX));
    let mut multiplication = Ledger::new(usize::MAX, usize::MAX);
    assert_eq!(
        multiplication.charge(usize::MAX, 2),
        Err(Refusal::Arithmetic)
    );
    assert_eq!((multiplication.bytes, multiplication.items), (0, 0));
}

#[test]
fn locator_storage_distinct_clones_are_separate_owned_trees() {
    let original = fixture();
    let clone = original.clone();
    let mut ledger = Ledger::new(usize::MAX, usize::MAX);
    original
        .visit_logical_retained_storage_v1(&mut |n, w| ledger.charge(n, w))
        .unwrap();
    clone
        .visit_logical_retained_storage_v1(&mut |n, w| ledger.charge(n, w))
        .unwrap();
    assert_eq!(
        ledger.bytes,
        2 * size_of::<MirProductionModuleLocatorV1>()
            + expected_heap(&original)
            + expected_heap(&clone),
    );
    assert_eq!(ledger.items, 14);
}

#[test]
fn locator_storage_all_functions_are_visited_in_source_order() {
    let first = fixture();
    let second = MirProductionFunctionLocatorV1::try_new(
        SemanticFunctionIdV1::from_index(1),
        SemanticBlockIdV1::from_index(0),
        first.functions[0].blocks.clone(),
    )
    .unwrap();
    let mut functions = Vec::with_capacity(23);
    functions.push(first.functions[0].clone());
    functions.push(second);
    let module = MirProductionModuleLocatorV1::try_new(first.semantic_sha256, functions).unwrap();
    let mut expected = vec![
        (0, 1),
        (
            module.functions.capacity(),
            size_of::<MirProductionFunctionLocatorV1>(),
        ),
    ];
    for function in &module.functions {
        expected.push((
            function.blocks.capacity(),
            size_of::<MirProductionBlockLocatorV1>(),
        ));
        for block in &function.blocks {
            expected.push((
                block.statements.capacity(),
                size_of::<MirProductionStatementLocatorV1>(),
            ));
            expected.push((
                block.terminator.successors.capacity(),
                size_of::<MirProductionSuccessorArcV1>(),
            ));
        }
    }
    let mut actual = Vec::new();
    module
        .visit_logical_retained_heap_v1(&mut |n, w| {
            actual.push((n, w));
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_eq!(actual, expected);
    assert_eq!(actual.len(), 12);
}
