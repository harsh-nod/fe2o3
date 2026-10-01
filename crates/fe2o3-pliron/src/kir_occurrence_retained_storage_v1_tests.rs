//! Private occurrence ownership models; no transition or execution authority.
use super::super::{
    BlockRow, DefinitionRow, Descendant, EdgeArgumentRow, EdgeRow, Function, FunctionRow,
    OperationRow, Segment, UseRow,
};
use super::*;
use fe2o3_kernel_ir::LogicalStorageLimitsV1 as Limits;
fn limits(bytes: Option<usize>, items: usize) -> Limits {
    Limits {
        max_bytes: bytes,
        max_items: items,
    }
}
fn empty() -> Rows {
    Rows {
        functions: Vec::new(),
        blocks: Vec::new(),
        segments: Vec::new(),
        operations: Vec::new(),
        definitions: Vec::new(),
        definition_outputs: Vec::new(),
        uses: Vec::new(),
        edges: Vec::new(),
        edge_arguments: Vec::new(),
    }
}
fn populated() -> Rows {
    let mut rows = empty();
    rows.functions.reserve_exact(1);
    rows.functions.push(FunctionRow {
        input: Function(0),
        output: Function(0),
    });
    rows.blocks.reserve_exact(2);
    rows.segments.reserve_exact(3);
    rows.operations.reserve_exact(4);
    rows.definitions.reserve_exact(5);
    rows.definition_outputs.reserve_exact(6);
    rows.uses.reserve_exact(7);
    rows.edges.reserve_exact(8);
    rows.edge_arguments.reserve_exact(9);
    rows
}
fn expected(rows: &Rows) -> usize {
    rows.functions.capacity() * size_of::<FunctionRow>()
        + rows.blocks.capacity() * size_of::<BlockRow>()
        + rows.segments.capacity() * size_of::<Segment>()
        + rows.operations.capacity() * size_of::<OperationRow>()
        + rows.definitions.capacity() * size_of::<DefinitionRow>()
        + rows.definition_outputs.capacity() * size_of::<Descendant>()
        + rows.uses.capacity() * size_of::<UseRow>()
        + rows.edges.capacity() * size_of::<EdgeRow>()
        + rows.edge_arguments.capacity() * size_of::<EdgeArgumentRow>()
}

#[test]
fn empty_and_all_nine_spare_capacities_match_the_unchanged_helper() {
    let empty = empty();
    let mut counter = Counter::new(limits(Some(0), 9));
    empty.charge_retained_heap_storage_v1(&mut counter).unwrap();
    assert_eq!((counter.bytes(), counter.items()), (0, 9));
    let rows = populated();
    let before = (
        rows.functions.clone(),
        rows.blocks.clone(),
        rows.segments.clone(),
        rows.operations.clone(),
        rows.definitions.clone(),
        rows.definition_outputs.clone(),
        rows.uses.clone(),
        rows.edges.clone(),
        rows.edge_arguments.clone(),
    );
    let heap = expected(&rows);
    assert_eq!(rows.retained_storage().unwrap(), size_of::<Rows>() + heap);
    let mut counter = Counter::new(limits(Some(heap), 9));
    rows.charge_retained_heap_storage_v1(&mut counter).unwrap();
    assert_eq!((counter.bytes(), counter.items()), (heap, 9));
    assert_eq!(
        (
            rows.functions,
            rows.blocks,
            rows.segments,
            rows.operations,
            rows.definitions,
            rows.definition_outputs,
            rows.uses,
            rows.edges,
            rows.edge_arguments
        ),
        before
    );
}

#[test]
fn occurrence_exact_and_one_short_limits_preserve_nonzero_caller_state() {
    let rows = populated();
    let heap = expected(&rows);
    let mut exact = Counter::new(limits(Some(29 + heap), 13));
    exact.charge(29, 4).unwrap();
    rows.charge_retained_heap_storage_v1(&mut exact).unwrap();
    assert_eq!((exact.bytes(), exact.items()), (29 + heap, 13));
    for (limit, error) in [
        (limits(Some(29 + heap - 1), 13), Error::ByteLimit),
        (limits(None, 12), Error::ItemLimit),
    ] {
        let mut counter = Counter::new(limit);
        counter.charge(29, 4).unwrap();
        assert_eq!(
            rows.charge_retained_heap_storage_v1(&mut counter),
            Err(error)
        );
        assert_eq!((counter.bytes(), counter.items()), (29, 4));
    }
}

#[test]
fn occurrence_counter_arithmetic_refuses_without_any_partial_charge() {
    let rows = populated();
    for (bytes, items) in [(usize::MAX, 0), (0, usize::MAX - 8)] {
        let mut counter = Counter::new(limits(None, usize::MAX));
        counter.charge(bytes, items).unwrap();
        assert_eq!(
            rows.charge_retained_heap_storage_v1(&mut counter),
            Err(Error::Arithmetic)
        );
        assert_eq!((counter.bytes(), counter.items()), (bytes, items));
    }
}

#[test]
fn root_visit_and_headers_are_external_and_identical_row_owners_are_distinct() {
    let owner = (populated(), populated());
    assert_eq!(owner.0.candidate().functions, owner.1.candidate().functions);
    let header = size_of::<(Rows, Rows)>();
    let heap = expected(&owner.0) + expected(&owner.1);
    let mut counter = Counter::new(limits(Some(header + heap), 19));
    counter.charge(header, 0).unwrap();
    owner
        .0
        .charge_retained_heap_storage_v1(&mut counter)
        .unwrap();
    owner
        .1
        .charge_retained_heap_storage_v1(&mut counter)
        .unwrap();
    assert_eq!((counter.bytes(), counter.items()), (header + heap, 18));
    counter.charge(0, 1).unwrap();
    assert_eq!(counter.items(), 19);
}
