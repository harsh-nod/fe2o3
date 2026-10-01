//! Private inert map ownership models; these do not authenticate pass execution.
use super::super::{
    Change, Coordinate, Endpoint, Event, Kind, KirOptimizationDispositionV12 as Disposition, Node,
    PassSpan, PlironOptimizationPassV1 as Pass,
};
use super::*;
use fe2o3_kernel_ir::{LogicalStorageLimitsV1 as Limits, Module, VerifiedCanonicalKernelIrV12};
fn limits(bytes: Option<usize>, items: usize) -> Limits {
    Limits {
        max_bytes: bytes,
        max_items: items,
    }
}
fn empty() -> MapData {
    let canonical = VerifiedCanonicalKernelIrV12::from_module(Module::new("map-storage")).unwrap();
    MapData {
        input: *canonical.identity(),
        output: *canonical.identity(),
        digest: [7; 32],
        nodes: Vec::new(),
        events: Vec::new(),
        terminal: Vec::new(),
        passes: Vec::new(),
        relations: Vec::new(),
        targets: Vec::new(),
        synthesized: Vec::new(),
    }
}
fn populated() -> MapData {
    let mut data = empty();
    let coordinate = Coordinate::Operation {
        function: 0,
        block: 0,
        operation: 0,
    };
    let endpoint = Endpoint::Operation(coordinate);
    data.nodes.reserve_exact(3);
    data.nodes.push(Node {
        kind: Kind::Operation,
        input: Some(endpoint),
        result_index: None,
    });
    data.events.reserve_exact(5);
    data.events.push(Event {
        pass: 0,
        change: Change::Move(0),
    });
    data.terminal.reserve_exact(7);
    data.terminal.push(Some(endpoint));
    data.passes.reserve_exact(11);
    data.passes.push(PassSpan {
        pass: Pass::DeadCodeElimination,
        input_epoch: 0,
        output_epoch: 1,
        start: 0,
        end: 1,
    });
    data.relations.reserve_exact(13);
    data.relations.push(KirOptimizationRelationV12 {
        source: coordinate,
        disposition: Disposition::Moved,
        identity_survived: true,
        moved: true,
        targets: 0..1,
    });
    data.targets.reserve_exact(17);
    data.targets.push(endpoint);
    data.synthesized.reserve_exact(19);
    data.synthesized.push(coordinate);
    data
}
fn expected(data: &MapData) -> usize {
    data.nodes.capacity() * size_of::<Node>()
        + data.events.capacity() * size_of::<Event>()
        + data.terminal.capacity() * size_of::<Option<Endpoint>>()
        + data.passes.capacity() * size_of::<PassSpan>()
        + data.relations.capacity() * size_of::<KirOptimizationRelationV12>()
        + data.targets.capacity() * size_of::<Endpoint>()
        + data.synthesized.capacity() * size_of::<Coordinate>()
}

#[test]
fn empty_and_all_seven_spare_capacities_match_the_unchanged_helper() {
    let empty = empty();
    let mut counter = Counter::new(limits(Some(0), 7));
    empty.charge_retained_heap_storage_v1(&mut counter).unwrap();
    assert_eq!((counter.bytes(), counter.items()), (0, 7));
    let data = populated();
    let before = (
        data.input,
        data.output,
        data.digest,
        data.nodes.clone(),
        data.events.clone(),
        data.terminal.clone(),
        data.passes.clone(),
        data.relations.clone(),
        data.targets.clone(),
        data.synthesized.clone(),
    );
    let heap = expected(&data);
    assert_eq!(
        data.retained_storage().unwrap(),
        size_of::<MapData>() + heap
    );
    let mut counter = Counter::new(limits(Some(heap), 7));
    data.charge_retained_heap_storage_v1(&mut counter).unwrap();
    assert_eq!((counter.bytes(), counter.items()), (heap, 7));
    assert_eq!(
        (
            data.input,
            data.output,
            data.digest,
            data.nodes,
            data.events,
            data.terminal,
            data.passes,
            data.relations,
            data.targets,
            data.synthesized
        ),
        before
    );
}

#[test]
fn all_three_policy_wrappers_share_only_the_accounting_not_authority() {
    macro_rules! check {
        ($owner:ident) => {{
            let owner = $owner { data: populated() };
            let expected = expected(&owner.data);
            let mut counter = Counter::new(limits(Some(expected), 7));
            owner.charge_retained_heap_storage_v1(&mut counter).unwrap();
            assert_eq!((counter.bytes(), counter.items()), (expected, 7));
        }};
    }
    check!(KirOptimizationMapV12);
    check!(KirOptimizationMapPolicy3V12);
    check!(KirOptimizationMapIntegerContinuationV12);
}

#[test]
fn map_short_bounds_and_counter_overflow_are_whole_charge_atomic() {
    let data = populated();
    let heap = expected(&data);
    for (limit, error) in [
        (limits(Some(23 + heap - 1), 10), Error::ByteLimit),
        (limits(None, 9), Error::ItemLimit),
    ] {
        let mut counter = Counter::new(limit);
        counter.charge(23, 3).unwrap();
        assert_eq!(
            data.charge_retained_heap_storage_v1(&mut counter),
            Err(error)
        );
        assert_eq!((counter.bytes(), counter.items()), (23, 3));
    }
    for (bytes, items) in [(usize::MAX, 0), (0, usize::MAX - 6)] {
        let mut counter = Counter::new(limits(None, usize::MAX));
        counter.charge(bytes, items).unwrap();
        assert_eq!(
            data.charge_retained_heap_storage_v1(&mut counter),
            Err(Error::Arithmetic)
        );
        assert_eq!((counter.bytes(), counter.items()), (bytes, items));
    }
}

#[test]
fn enclosing_header_is_once_and_equal_maps_keep_distinct_vector_allocations() {
    let owner = (
        KirOptimizationMapPolicy3V12 { data: populated() },
        KirOptimizationMapPolicy3V12 { data: populated() },
    );
    assert_eq!(owner.0, owner.1);
    let header = size_of::<(KirOptimizationMapPolicy3V12, KirOptimizationMapPolicy3V12)>();
    let heap = expected(&owner.0.data) + expected(&owner.1.data);
    let mut counter = Counter::new(limits(Some(header + heap), 15));
    counter.charge(header, 1).unwrap();
    owner
        .0
        .charge_retained_heap_storage_v1(&mut counter)
        .unwrap();
    owner
        .1
        .charge_retained_heap_storage_v1(&mut counter)
        .unwrap();
    assert_eq!((counter.bytes(), counter.items()), (header + heap, 15));
}
