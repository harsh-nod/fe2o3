//! Private capacity controls; malformed owner mutation is only a refusal probe.
use super::*;
use crate::{
    BasicBlock, BlockId, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrWorkBudgetV1 as Work, Function, LogicalStorageLimitsV1 as Limits, Module,
    Signature, StorageLayoutIdV1, Terminator, Type,
};
fn limits(bytes: Option<usize>, items: usize) -> Limits {
    Limits {
        max_bytes: bytes,
        max_items: items,
    }
}
fn owner() -> VerifiedCanonicalKernelIrModuleV12 {
    let mut module = Module::new("v12-storage");
    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return { values: Vec::new() });
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(Vec::new(), Vec::new()),
        Vec::new(),
        vec![block],
    ));
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
        &module,
        &mut budget,
    )
    .unwrap()
    .0
}

#[test]
fn canonical_bytes_use_actual_spare_capacity_and_one_visit_without_a_header() {
    let mut owner = owner();
    let before = owner.canonical.canonical_bytes().to_vec();
    let identity = *owner.canonical.identity();
    owner.canonical.canonical_bytes.reserve_exact(173);
    let expected = owner.canonical.canonical_bytes.capacity();
    assert!(expected > before.len());
    let mut counter = Counter::new(limits(Some(expected), 1));
    owner
        .canonical
        .charge_retained_heap_storage_v1(&mut counter)
        .unwrap();
    assert_eq!((counter.bytes(), counter.items()), (expected, 1));
    assert_eq!(owner.canonical.canonical_bytes(), before);
    assert_eq!(*owner.canonical.identity(), identity);
}

#[test]
fn canonical_capacity_charge_refuses_atomically_at_all_counter_boundaries() {
    let owner = owner();
    let capacity = owner.canonical.canonical_bytes.capacity();
    for (limit, expected) in [
        (limits(Some(capacity - 1), 1), Error::ByteLimit),
        (limits(None, 0), Error::ItemLimit),
    ] {
        let mut counter = Counter::new(limit);
        assert_eq!(
            owner
                .canonical
                .charge_retained_heap_storage_v1(&mut counter),
            Err(expected)
        );
        assert_eq!((counter.bytes(), counter.items()), (0, 0));
    }
    let mut counter = Counter::new(limits(None, usize::MAX));
    counter.charge(usize::MAX, 0).unwrap();
    assert_eq!(
        owner
            .canonical
            .charge_retained_heap_storage_v1(&mut counter),
        Err(Error::Arithmetic),
    );
    assert_eq!((counter.bytes(), counter.items()), (usize::MAX, 0));
}

#[test]
fn decoded_module_and_byte_allocation_are_composed_without_duplicate_headers() {
    let mut owner = owner();
    owner.canonical.canonical_bytes.reserve_exact(37);
    owner.module.functions.reserve_exact(3);
    let before = owner.module.clone();
    let mut module = Counter::new(limits(None, 10_000));
    owner.module.charge_retained_heap_v11(&mut module).unwrap();
    let expected = owner.canonical.canonical_bytes.capacity() + module.bytes();
    let mut composed = Counter::new(limits(Some(expected), module.items() + 1));
    owner.charge_retained_heap_v11(&mut composed).unwrap();
    assert_eq!(
        (composed.bytes(), composed.items()),
        (expected, module.items() + 1)
    );
    assert_eq!(owner.module, before);
    let mut short = Counter::new(limits(None, module.items()));
    assert_eq!(
        owner.charge_retained_heap_v11(&mut short),
        Err(Error::ItemLimit)
    );
}

#[test]
fn unsupported_module_payload_is_refused_and_exhausted_limits_do_not_hide_a_walk() {
    let mut owner = owner();
    // Deliberately invalid private test mutation, not a newly admitted V12 owner.
    owner.module.functions[0]
        .signature
        .parameters
        .push(Type::StorageObject(StorageLayoutIdV1(0)));
    let mut counter = Counter::new(limits(None, 10_000));
    assert_eq!(
        owner.charge_retained_heap_v11(&mut counter),
        Err(Error::UnsupportedV11Owner)
    );
    // This failed partial ledger must never be published as a total.
    assert!(counter.items() > 0);
    let mut exhausted = Counter::new(limits(None, 0));
    assert_eq!(
        owner.charge_retained_heap_v11(&mut exhausted),
        Err(Error::ItemLimit)
    );
    assert_eq!((exhausted.bytes(), exhausted.items()), (0, 0));
}
