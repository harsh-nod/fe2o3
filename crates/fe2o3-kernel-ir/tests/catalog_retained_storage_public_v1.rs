//! External API control: no private module/type inference can mask visibility.
//! Byte charging and logical item visits are independent: standalone observes
//! one root plus three capacities; heap-only observes three capacities and does
//! not include a root visit, even when the enclosing header contains the catalog.
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
    InertCanonicalKernelIrContractCatalogV1 as Catalog, KernelIrContractCatalogRetainedStorageV1,
    LogicalStorageCounterV1 as Counter, LogicalStorageLimitsV1 as Limits,
};

#[test]
fn public_named_report_and_heap_only_methods_compose_without_a_second_header() {
    let mut work = Work::new(100_000);
    let mut budget = Budget::new(&mut work, 100_000);
    let (catalog, _) = Catalog::from_rows_with_budget([1; 32], &[], &[], &mut budget).unwrap();
    let limits = Limits {
        max_bytes: None,
        max_items: 4,
    };
    let report: KernelIrContractCatalogRetainedStorageV1 =
        catalog.retained_logical_storage_v1(limits).unwrap();
    assert_eq!(report.inline_bytes, std::mem::size_of::<Catalog>());
    assert_eq!(report.visited_items, 4);

    let mut counter = Counter::new(limits);
    // Header bytes alone do not imply a root item visit.
    counter.charge(report.inline_bytes, 0).unwrap();
    catalog
        .charge_retained_heap_storage_v1(&mut counter)
        .unwrap();
    assert_eq!(counter.bytes(), report.total_bytes);
    assert_eq!(counter.items(), 3);
    counter.charge(0, 1).unwrap();
    assert_eq!(counter.items(), report.visited_items);
}
