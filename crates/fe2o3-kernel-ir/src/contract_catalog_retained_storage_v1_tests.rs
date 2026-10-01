//! Capacity/accounting controls only; no genuine compiler or timing claims.
use super::*;
use crate::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
    KernelIrContractCatalogStorageV1,
};

fn limits(bytes: Option<usize>, items: usize) -> Limits {
    Limits {
        max_bytes: bytes,
        max_items: items,
    }
}

fn definition() -> KernelIrPipelineContractDefinitionV1 {
    KernelIrPipelineContractDefinitionV1 {
        key: 0,
        semantic_pipeline_type: 9,
        semantic_payload_type: 3,
        buffers: 2,
        elements: 32,
        prefetch_distance: 1,
        packed_bits: 32,
        source_size_bytes: 4,
        source_alignment_bytes: 4,
    }
}

fn binding() -> KernelIrPipelineStorageBindingV1 {
    KernelIrPipelineStorageBindingV1 {
        function: 1,
        storage: 800,
        key: 0,
        block: 2,
        operation: 4,
    }
}

fn catalog(nonempty: bool) -> (Catalog, KernelIrContractCatalogStorageV1) {
    let mut work = Work::new(100_000);
    let mut budget = Budget::new(&mut work, 100_000);
    let definitions = [definition()];
    let bindings = [binding()];
    Catalog::from_rows_with_budget(
        [1; 32],
        if nonempty { &definitions } else { &[] },
        if nonempty { &bindings } else { &[] },
        &mut budget,
    )
    .unwrap()
}

fn expected_heap(catalog: &Catalog) -> usize {
    // Independent formula from the actual fields, not the new Capacities helper.
    catalog.canonical.capacity()
        + catalog.definitions.capacity() * size_of::<KernelIrPipelineContractDefinitionV1>()
        + catalog.bindings.capacity() * size_of::<KernelIrPipelineStorageBindingV1>()
}

#[test]
fn empty_catalog_measures_its_real_canonical_buffer_and_one_header() {
    let (catalog, _) = catalog(false);
    let observed = catalog
        .retained_logical_storage_v1(limits(None, 4))
        .unwrap();
    assert_eq!(observed.inline_bytes, size_of::<Catalog>());
    assert_eq!(observed.canonical_owned_bytes, catalog.canonical.capacity());
    assert_eq!(observed.definitions_owned_bytes, 0);
    assert_eq!(observed.bindings_owned_bytes, 0);
    assert_eq!(
        observed.total_bytes,
        size_of::<Catalog>() + expected_heap(&catalog)
    );
    assert_eq!(observed.visited_items, 4);
    assert!(!catalog.canonical_bytes().is_empty());
    assert!(!catalog.grants_authority());
}

#[test]
fn all_three_actual_capacities_include_spare_slots_without_changing_bytes() {
    let (mut catalog, receipt) = catalog(true);
    let before = (
        catalog.canonical.clone(),
        catalog.digest,
        catalog.semantic_source,
        catalog.definitions.clone(),
        catalog.bindings.clone(),
    );
    catalog.canonical.reserve_exact(127);
    catalog.definitions.reserve_exact(17);
    catalog.bindings.reserve_exact(31);
    assert!(catalog.canonical.capacity() > catalog.canonical.len());
    assert!(catalog.definitions.capacity() > catalog.definitions.len());
    assert!(catalog.bindings.capacity() > catalog.bindings.len());

    let observed = catalog
        .retained_logical_storage_v1(limits(None, 4))
        .unwrap();
    assert_eq!(observed.canonical_owned_bytes, catalog.canonical.capacity());
    assert_eq!(
        observed.definitions_owned_bytes,
        catalog.definitions.capacity() * size_of::<KernelIrPipelineContractDefinitionV1>()
    );
    assert_eq!(
        observed.bindings_owned_bytes,
        catalog.bindings.capacity() * size_of::<KernelIrPipelineStorageBindingV1>()
    );
    assert_eq!(
        observed.total_bytes,
        size_of::<Catalog>() + expected_heap(&catalog)
    );
    assert!(observed.total_bytes > receipt.retained_storage());
    assert_eq!(
        (
            catalog.canonical.clone(),
            catalog.digest,
            catalog.semantic_source,
            catalog.definitions.clone(),
            catalog.bindings.clone(),
        ),
        before
    );
}

#[test]
fn standalone_exact_byte_and_item_limits_and_one_short_refuse() {
    let (catalog, _) = catalog(true);
    let exact = size_of::<Catalog>() + expected_heap(&catalog);
    assert_eq!(
        catalog
            .retained_logical_storage_v1(limits(Some(exact), 4))
            .unwrap()
            .total_bytes,
        exact
    );
    assert_eq!(
        catalog.retained_logical_storage_v1(limits(Some(exact - 1), 4)),
        Err(Error::ByteLimit)
    );
    assert_eq!(
        catalog.retained_logical_storage_v1(limits(Some(exact), 3)),
        Err(Error::ItemLimit)
    );
    assert_eq!(
        catalog.retained_logical_storage_v1(limits(None, 0)),
        Err(Error::ItemLimit)
    );
}

#[test]
fn nested_owner_charges_enclosing_header_once_and_catalog_heap_only() {
    struct Enclosing {
        _tag: [u8; 13],
        catalog: Catalog,
    }
    let owner = Enclosing {
        _tag: [0; 13],
        catalog: catalog(true).0,
    };
    let heap = expected_heap(&owner.catalog);
    let exact = size_of::<Enclosing>() + heap;
    let mut counter = Counter::new(limits(Some(exact), 4));
    counter.charge(size_of::<Enclosing>(), 1).unwrap();
    owner
        .catalog
        .charge_retained_heap_storage_v1(&mut counter)
        .unwrap();
    assert_eq!((counter.bytes(), counter.items()), (exact, 4));
    let standalone = owner
        .catalog
        .retained_logical_storage_v1(limits(None, 4))
        .unwrap();
    assert_eq!(
        counter.bytes() - standalone.total_bytes,
        size_of::<Enclosing>() - size_of::<Catalog>()
    );
}

#[test]
fn heap_only_byte_and_item_refusals_preserve_the_existing_counter() {
    let (catalog, _) = catalog(true);
    let heap = expected_heap(&catalog);
    for (limit, expected) in [
        (limits(Some(19 + heap - 1), 8), Error::ByteLimit),
        (limits(Some(19 + heap), 7), Error::ItemLimit),
    ] {
        let mut counter = Counter::new(limit);
        counter.charge(19, 5).unwrap();
        assert_eq!(
            catalog.charge_retained_heap_storage_v1(&mut counter),
            Err(expected)
        );
        assert_eq!((counter.bytes(), counter.items()), (19, 5));
    }
    let mut exact = Counter::new(limits(Some(19 + heap), 8));
    exact.charge(19, 5).unwrap();
    catalog.charge_retained_heap_storage_v1(&mut exact).unwrap();
    assert_eq!((exact.bytes(), exact.items()), (19 + heap, 8));
}

#[test]
fn capacity_product_and_sum_overflows_refuse_without_fake_allocations() {
    for (canonical, definitions, bindings) in [
        (0, usize::MAX, 0),
        (0, 0, usize::MAX),
        (usize::MAX, 1, 0),
        (usize::MAX, 0, 1),
    ] {
        assert_eq!(
            Capacities::from_counts(canonical, definitions, bindings),
            Err(Error::Arithmetic)
        );
    }
    let maximum = Capacities::from_counts(usize::MAX, 0, 0).unwrap();
    assert_eq!(maximum.heap, usize::MAX);
    let mut counter = Counter::new(limits(None, usize::MAX));
    counter.charge(1, 1).unwrap();
    assert_eq!(maximum.charge_heap(&mut counter), Err(Error::Arithmetic));
    assert_eq!((counter.bytes(), counter.items()), (1, 1));
}

#[test]
fn actual_owner_heap_charge_refuses_counter_byte_and_item_overflow_atomically() {
    let (catalog, _) = catalog(false);
    let mut bytes = Counter::new(limits(None, usize::MAX));
    bytes.charge(usize::MAX, 0).unwrap();
    assert_eq!(
        catalog.charge_retained_heap_storage_v1(&mut bytes),
        Err(Error::Arithmetic)
    );
    assert_eq!((bytes.bytes(), bytes.items()), (usize::MAX, 0));

    let mut items = Counter::new(limits(None, usize::MAX));
    items.charge(0, usize::MAX - 2).unwrap();
    assert_eq!(
        catalog.charge_retained_heap_storage_v1(&mut items),
        Err(Error::Arithmetic)
    );
    assert_eq!((items.bytes(), items.items()), (0, usize::MAX - 2));
}

#[test]
fn equal_bytes_in_distinct_owners_count_two_payloads() {
    let (first, _) = catalog(true);
    let (second, _) = catalog(true);
    assert_eq!(first.canonical_bytes(), second.canonical_bytes());
    let owners = (first, second);
    let heap = expected_heap(&owners.0) + expected_heap(&owners.1);
    let mut counter = Counter::new(limits(None, 7));
    counter.charge(size_of::<(Catalog, Catalog)>(), 1).unwrap();
    owners
        .0
        .charge_retained_heap_storage_v1(&mut counter)
        .unwrap();
    owners
        .1
        .charge_retained_heap_storage_v1(&mut counter)
        .unwrap();
    assert_eq!(
        (counter.bytes(), counter.items()),
        (size_of::<(Catalog, Catalog)>() + heap, 7)
    );
}

#[test]
fn observing_does_not_change_original_canonical_bytes_or_admission_ledger() {
    let mut work = Work::new(100_000);
    let mut budget = Budget::new(&mut work, 100_000);
    budget.reserve_storage(23).unwrap();
    let (catalog, receipt) =
        Catalog::from_rows_with_budget([1; 32], &[definition()], &[binding()], &mut budget)
            .unwrap();
    let before = (
        budget.work(),
        budget.storage(),
        budget.peak_storage(),
        receipt.retained_storage(),
        catalog.canonical_bytes().to_vec(),
        *catalog.digest(),
        *catalog.semantic_source(),
    );
    catalog
        .retained_logical_storage_v1(limits(None, 4))
        .unwrap();
    let mut counter = Counter::new(limits(None, 3));
    catalog
        .charge_retained_heap_storage_v1(&mut counter)
        .unwrap();
    assert_eq!(
        (
            budget.work(),
            budget.storage(),
            budget.peak_storage(),
            receipt.retained_storage(),
            catalog.canonical_bytes().to_vec(),
            *catalog.digest(),
            *catalog.semantic_source(),
        ),
        before
    );
    assert_eq!(budget.storage(), 23);
    assert!(!catalog.grants_authority());
}
