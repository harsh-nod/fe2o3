//! Capacity controls on ordinary formal derivation and private spare storage.
use super::*;
use crate::{
    AccessMode, AddressSpace, BasicBlock, BlockId, ExplicitLaunchExtent1d, FormalIndexWidth,
    FormalMemoryObligationAnalysis, Function, FunctionId, Kernel, KernelId, LaunchDomain,
    LaunchExtent, LogicalStorageLimitsV1 as Limits, MemoryAccess, Module, Operation, OperationKind,
    ScalarType, Signature, Terminator, Type, ValueId, derive_kernel_memory_obligations,
};
fn limits(bytes: Option<usize>, items: usize) -> Limits {
    Limits {
        max_bytes: bytes,
        max_items: items,
    }
}
fn derive(nonempty: bool) -> FormalMemoryObligations {
    let mut block = BasicBlock::new(BlockId(0));
    let (parameters, values) = if nonempty {
        block.operations.push(Operation::new(
            Vec::new(),
            OperationKind::Store {
                pointer: ValueId(1),
                value: ValueId(0),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ));
        (
            vec![
                Type::Scalar(ScalarType::U32),
                Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::ReadWrite,
                ),
            ],
            vec![ValueId(0), ValueId(1)],
        )
    } else {
        (Vec::new(), Vec::new())
    };
    block.terminator = Some(Terminator::Return { values: Vec::new() });
    let mut function = Function::kernel_entry(
        "formal_entry",
        Signature::new(parameters, Vec::new()),
        values,
        vec![block],
    );
    function.required_capabilities = function.derived_capabilities();
    let mut module = Module::new("formal-retained-storage");
    module.functions.push(function);
    let mut kernel = Kernel::new(
        "formal_kernel",
        "formal_entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(1),
        },
    );
    kernel.required_capabilities = module.functions[0].required_capabilities.clone();
    module.kernels.push(kernel);
    module.required_capabilities = module.derived_capabilities();
    match derive_kernel_memory_obligations(
        &module,
        &KernelId::new("formal_kernel"),
        ExplicitLaunchExtent1d::Exact(1),
        FormalIndexWidth::Bits64,
    )
    .unwrap()
    {
        FormalMemoryObligationAnalysis::Complete(owner) => owner,
        FormalMemoryObligationAnalysis::Incomplete { reasons, .. } => panic!("{reasons:?}"),
    }
}
fn expected(owner: &FormalMemoryObligations) -> usize {
    owner.kernel.retained_capacity_bytes()
        + owner.entry.retained_capacity_bytes()
        + owner.allocations.capacity() * size_of::<FormalAllocationParameter>()
        + owner.accesses.capacity() * size_of::<FormalMemoryAccess>()
        + owner.bounds_requirements.capacity() * size_of::<FormalBoundsRequirement>()
        + owner.runtime_alias_requirements.capacity() * size_of::<RuntimeAliasRequirement>()
        + owner.inter_invocation_conflicts.capacity()
            * size_of::<InterInvocationConflictRequirement>()
}
fn observe(owner: &FormalMemoryObligations) -> (usize, usize) {
    let mut counter = Counter::new(limits(None, 7));
    owner.charge_retained_heap_storage_v1(&mut counter).unwrap();
    (counter.bytes(), counter.items())
}

#[test]
fn ordinary_empty_and_nonempty_derivation_have_exact_heap_only_observations() {
    for nonempty in [false, true] {
        let owner = derive(nonempty);
        assert_eq!(!owner.allocations.is_empty(), nonempty);
        assert_eq!(!owner.accesses.is_empty(), nonempty);
        let before = owner.clone();
        assert_eq!(observe(&owner), (expected(&owner), 7));
        assert_eq!(owner, before);
    }
}

#[test]
fn every_vector_spare_capacity_is_counted_without_changing_rows() {
    let mut owner = derive(true);
    let before = owner.clone();
    let old = observe(&owner);
    owner
        .allocations
        .reserve_exact(owner.allocations.capacity() + 11);
    owner.accesses.reserve_exact(owner.accesses.capacity() + 13);
    owner
        .bounds_requirements
        .reserve_exact(owner.bounds_requirements.capacity() + 17);
    owner
        .runtime_alias_requirements
        .reserve_exact(owner.runtime_alias_requirements.capacity() + 19);
    owner
        .inter_invocation_conflicts
        .reserve_exact(owner.inter_invocation_conflicts.capacity() + 23);
    assert_eq!(owner, before);
    assert!(expected(&owner) > old.0);
    assert_eq!(observe(&owner), (expected(&owner), 7));
}

#[test]
fn both_name_allocations_use_capacity_not_text_length() {
    let mut owner = derive(false);
    let before = owner.clone();
    let old = observe(&owner);
    let mut kernel = String::with_capacity(211);
    kernel.push_str(owner.kernel.as_str());
    let kernel_capacity = kernel.capacity();
    let mut entry = String::with_capacity(307);
    entry.push_str(owner.entry.as_str());
    let entry_capacity = entry.capacity();
    let delta = kernel_capacity + entry_capacity
        - owner.kernel.retained_capacity_bytes()
        - owner.entry.retained_capacity_bytes();
    owner.kernel = KernelId::new(kernel);
    owner.entry = FunctionId::new(entry);
    assert_eq!(owner, before);
    assert_eq!(observe(&owner), (old.0 + delta, 7));
}

#[test]
fn exact_bounds_succeed_and_all_refusals_leave_caller_prefix_unchanged() {
    let owner = derive(true);
    let bytes = expected(&owner);
    let mut exact = Counter::new(limits(Some(bytes + 13), 10));
    exact.charge(13, 3).unwrap();
    owner.charge_retained_heap_storage_v1(&mut exact).unwrap();
    assert_eq!((exact.bytes(), exact.items()), (13 + bytes, 10));
    for (limits, error) in [
        (limits(Some(bytes + 12), 10), Error::ByteLimit),
        (limits(Some(bytes + 13), 9), Error::ItemLimit),
    ] {
        let mut counter = Counter::new(limits);
        counter.charge(13, 3).unwrap();
        assert_eq!(
            owner.charge_retained_heap_storage_v1(&mut counter),
            Err(error)
        );
        assert_eq!((counter.bytes(), counter.items()), (13, 3));
    }
    let mut exhausted = Counter::new(limits(None, 0));
    assert_eq!(
        owner.charge_retained_heap_storage_v1(&mut exhausted),
        Err(Error::ItemLimit)
    );
    assert_eq!((exhausted.bytes(), exhausted.items()), (0, 0));
}

#[test]
fn arithmetic_refuses_without_constructing_an_impossible_vector() {
    assert_eq!(
        vector_bytes::<FormalMemoryAccess>(usize::MAX),
        Err(Error::Arithmetic)
    );
    let owner = derive(false);
    let mut counter = Counter::new(limits(None, usize::MAX));
    counter.charge(usize::MAX, 0).unwrap();
    assert_eq!(
        owner.charge_retained_heap_storage_v1(&mut counter),
        Err(Error::Arithmetic)
    );
    assert_eq!((counter.bytes(), counter.items()), (usize::MAX, 0));
    let mut counter = Counter::new(limits(None, usize::MAX));
    counter.charge(0, usize::MAX).unwrap();
    assert_eq!(
        owner.charge_retained_heap_storage_v1(&mut counter),
        Err(Error::Arithmetic)
    );
    assert_eq!((counter.bytes(), counter.items()), (0, usize::MAX));
}

#[test]
fn two_equal_reports_in_separate_boxes_keep_both_headers_and_payloads() {
    let first = vec![derive(true)].into_boxed_slice();
    let second = vec![derive(true)].into_boxed_slice();
    assert_eq!(first, second);
    assert!(!std::ptr::eq(first.as_ptr(), second.as_ptr()));
    let expected_heap = expected(&first[0]) + expected(&second[0]);
    let mut counter = Counter::new(limits(None, 100));
    // The two box handles belong to the enclosing tuple header once.
    counter
        .charge(
            size_of::<(
                Box<[FormalMemoryObligations]>,
                Box<[FormalMemoryObligations]>,
            )>(),
            1,
        )
        .unwrap();
    for owner in [&first, &second] {
        counter
            .array::<FormalMemoryObligations>(owner.len())
            .unwrap();
        for report in owner.iter() {
            counter.charge(0, 1).unwrap();
            report
                .charge_retained_heap_storage_v1(&mut counter)
                .unwrap();
        }
    }
    assert_eq!(
        counter.bytes(),
        size_of::<(
            Box<[FormalMemoryObligations]>,
            Box<[FormalMemoryObligations]>
        )>() + 2 * size_of::<FormalMemoryObligations>()
            + expected_heap
    );
    assert_eq!(counter.items(), 1 + 2 * (1 + 1 + 7));
}
