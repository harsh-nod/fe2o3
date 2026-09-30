fn with_memory_inventory_v31(
    module: &Module,
    consume: impl FnOnce(&CanonicalKirAggregateMemoryInventoryV31<'_, '_>, &mut Budget<'_>),
) {
    let (owner, owner_credit) = admit(module);
    let mut work = Work::new(20_000_000);
    let mut budget = Budget::new(&mut work, 16 * 1024 * 1024);
    budget.reserve_storage(37 + owner_credit).unwrap();
    let (inventory, inventory_credit) = Inventory::derive_v18(&owner, &mut budget).unwrap();
    budget
        .reserve_storage(inventory_credit.retained_storage())
        .unwrap();
    let floor = budget.storage();
    let (memory, credit) =
        CanonicalKirAggregateMemoryInventoryV31::derive(&inventory, &mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    budget.reserve_storage(credit.retained_storage()).unwrap();
    assert!(std::ptr::eq(memory.inventory(), &inventory));
    assert!(!memory.grants_authority());
    consume(&memory, &mut budget);
    drop(memory);
    budget.release_storage(credit.retained_storage()).unwrap();
    assert_eq!(budget.storage(), floor);
    drop(inventory);
    budget
        .release_storage(inventory_credit.retained_storage())
        .unwrap();
    drop(owner);
    budget.release_storage(owner_credit).unwrap();
    assert_eq!(budget.storage(), 37);
}

#[test]
fn aggregate_memory_inventory_preserves_exact_owner_slot_and_every_event() {
    use CanonicalKirAggregateMemoryEventV31 as Event;
    with_memory_inventory_v31(&fixture(), |memory, budget| {
        assert_eq!(memory.allocation_count(), 1);
        let allocation = memory.allocation(0).unwrap();
        assert_eq!(allocation.operation(), 0);
        assert!(allocation.has_closed_supported_uses());
        assert_eq!(memory.leaf_count(), 1);
        assert_eq!(memory.leaf_alignment(0), Some(4));
        assert_eq!(memory.event_count(), memory.inventory().operations().len());
        assert_eq!(memory.event(0), Some(Event::Allocate { allocation: 0 }));
        assert_eq!(
            memory.event(1),
            Some(Event::Write {
                leaf: 0,
                value: ValueId(0)
            })
        );
        assert_eq!(
            memory.event(2),
            Some(Event::Read {
                leaf: 0,
                output: ValueId(20)
            })
        );
        assert_eq!(memory.event(3), None);
        assert_eq!(memory.allocation(1), None);
        assert_eq!(memory.leaf(1), None);
        assert_eq!(memory.leaf_alignment(1), None);
        let witness =
            derive_canonical_kir_aggregate_ssa_v18(memory.inventory().owner(), budget).unwrap();
        let witness_credit = witness.retained_storage();
        budget.reserve_storage(witness_credit).unwrap();
        assert_eq!(witness.memory_slots(), &[memory.leaf(0)]);
        assert_eq!(
            memory.leaf(0),
            Some(CanonicalKirAggregateSsaMemorySlotV18 {
                allocation: 0,
                layout: StorageLayoutIdV1(0),
                offset: 0,
                ty: CanonicalKirAggregateSsaLeafTypeV18::Scalar(ScalarType::U32),
            })
        );
        drop(witness);
        budget.release_storage(witness_credit).unwrap();
    });
}

#[test]
fn aggregate_memory_inventory_keeps_unselected_leaves_and_unknown_access_visible() {
    use CanonicalKirAggregateMemoryEventV31 as Event;
    let mut module = fixture();
    let Kind::Storage(Storage::ReadValue { access, .. }) =
        &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations[2].kind
    else {
        panic!("actual read")
    };
    access.volatile = true;
    with_memory_inventory_v31(&module, |memory, budget| {
        assert!(!memory.allocation(0).unwrap().has_closed_supported_uses());
        assert_eq!(memory.leaf_count(), 1);
        assert_eq!(memory.leaf(0).unwrap().allocation, 0);
        assert_eq!(memory.event(2), Some(Event::None));
        assert!(matches!(memory.inventory().operations()[2].operation.kind,
            Kind::Storage(Storage::ReadValue { access, .. }) if access.volatile));
        let witness =
            derive_canonical_kir_aggregate_ssa_v18(memory.inventory().owner(), budget).unwrap();
        let witness_credit = witness.retained_storage();
        budget.reserve_storage(witness_credit).unwrap();
        assert_eq!(witness.memory_slots(), &[None]);
        assert_eq!(witness.actions(), &[Action::Retain; 3]);
        drop(witness);
        budget.release_storage(witness_credit).unwrap();
    });
}

#[test]
fn aggregate_memory_inventory_closed_uses_do_not_claim_read_initialization() {
    use CanonicalKirAggregateMemoryEventV31 as Event;
    let mut module = fixture();
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .swap(1, 2);
    with_memory_inventory_v31(&module, |memory, budget| {
        assert!(memory.allocation(0).unwrap().has_closed_supported_uses());
        assert_eq!(
            memory.event(1),
            Some(Event::Read {
                leaf: 0,
                output: ValueId(20)
            })
        );
        assert_eq!(
            memory.event(2),
            Some(Event::Write {
                leaf: 0,
                value: ValueId(0)
            })
        );
        let witness =
            derive_canonical_kir_aggregate_ssa_v18(memory.inventory().owner(), budget).unwrap();
        let witness_credit = witness.retained_storage();
        budget.reserve_storage(witness_credit).unwrap();
        assert_eq!(witness.memory_slots(), &[None]);
        assert_eq!(witness.actions(), &[Action::Retain; 3]);
        drop(witness);
        budget.release_storage(witness_credit).unwrap();
    });
}

#[test]
fn aggregate_memory_inventory_derives_typed_field_offset_from_original_layout() {
    use CanonicalKirAggregateMemoryEventV31 as Event;
    let mut module = fixture();
    module.storage_layouts.push(StorageLayoutV1 {
        size: 8,
        alignment: 4,
        kind: LayoutKind::Record(
            vec![
                fe2o3_kernel_ir::StorageFieldV1 {
                    offset: 0,
                    layout: StorageLayoutIdV1(0),
                },
                fe2o3_kernel_ir::StorageFieldV1 {
                    offset: 4,
                    layout: StorageLayoutIdV1(0),
                },
            ]
            .into(),
        ),
    });
    let block = &mut module.functions[0].body.as_mut().unwrap().blocks[0];
    let leaf_pointer = block.operations[0].results[0].ty.clone();
    block.operations[0].results[0].ty = Type::pointer(
        Type::StorageObject(StorageLayoutIdV1(1)),
        AddressSpace::Private,
        AccessMode::ReadWrite,
    );
    let Kind::Alloca { element, .. } = &mut block.operations[0].kind else {
        panic!("actual allocation")
    };
    *element = Type::StorageObject(StorageLayoutIdV1(1));
    for operation in &mut block.operations[1..] {
        match &mut operation.kind {
            Kind::Storage(
                Storage::ReadValue { address, .. } | Storage::WriteValue { address, .. },
            ) => *address = ValueId(11),
            _ => panic!("actual access"),
        }
    }
    block.operations.insert(
        1,
        Operation::effect_free(
            ValueDef::new(ValueId(11), leaf_pointer),
            Kind::Storage(Storage::Project {
                base: ValueId(10),
                step: Projection::Field(1),
            }),
        ),
    );
    with_memory_inventory_v31(&module, |memory, _| {
        assert!(memory.allocation(0).unwrap().has_closed_supported_uses());
        assert_eq!(memory.event(1), Some(Event::Project { allocation: 0 }));
        assert_eq!(memory.leaf(0).unwrap().offset, 4);
        assert_eq!(memory.leaf(0).unwrap().layout, StorageLayoutIdV1(0));
        assert_eq!(memory.leaf_alignment(0), Some(4));
    });
}

#[test]
fn aggregate_memory_inventory_exact_and_one_short_resources_restore_original_floor() {
    with_memory_inventory_v31(&fixture(), |memory, outer| {
        let floor = outer.storage();
        let mut work = Work::new(20_000_000);
        let mut budget = Budget::new(&mut work, 16 * 1024 * 1024);
        budget.reserve_storage(floor).unwrap();
        let (view, _) =
            CanonicalKirAggregateMemoryInventoryV31::derive(memory.inventory(), &mut budget)
                .unwrap();
        let used = budget.work();
        let peak = budget.peak_storage();
        drop(view);
        assert_eq!(budget.storage(), floor);
        for (work_limit, storage_limit, expected) in
            [(used, peak, 0), (used - 1, peak, 1), (used, peak - 1, 2)]
        {
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(floor).unwrap();
            let result =
                CanonicalKirAggregateMemoryInventoryV31::derive(memory.inventory(), &mut budget);
            match (expected, result) {
                (0, Ok((view, _))) => drop(view),
                (1, Err(Error::Resource(Resource::Work(_)))) => {}
                (2, Err(Error::Resource(Resource::Storage(_)))) => {}
                _ => panic!("exact original memory inventory resource boundary"),
            }
            assert_eq!(budget.storage(), floor);
        }
    });
}

#[test]
fn aggregate_memory_inventory_headers_and_retained_capacity_match_field_oracles() {
    type CensusFields = (
        Vec<census::Allocation>,
        Vec<census::Cell>,
        Vec<census::Event>,
    );
    type OwnerFields<'a> = (&'a Inventory<'a>, CensusFields);
    type Output<'a> = (OwnerFields<'a>, usize);
    type Capture<'a> = (&'a Inventory<'a>, &'a mut Meter<'a, 'a>);
    type Scope<'a> = (
        &'a Inventory<'a>,
        Capture<'a>,
        std::panic::AssertUnwindSafe<Capture<'a>>,
        std::thread::Result<Result<Output<'a>>>,
        [Result<Output<'a>>; 2],
        std::panic::AssertUnwindSafe<Result<Output<'a>>>,
        std::thread::Result<()>,
        [Option<Box<dyn std::any::Any + Send>>; 2],
        Result<()>,
    );
    type Frame<'a> = (
        &'a Inventory<'a>,
        &'a mut Meter<'a, 'a>,
        Output<'a>,
        Result<Output<'a>>,
        Result<CensusFields>,
        [usize; 4],
        Scope<'a>,
    );
    assert_eq!(
        size_of::<OwnerFields<'_>>(),
        size_of::<CanonicalKirAggregateMemoryInventoryV31<'_, '_>>()
    );
    assert_eq!(
        std::mem::align_of::<OwnerFields<'_>>(),
        std::mem::align_of::<CanonicalKirAggregateMemoryInventoryV31<'_, '_>>()
    );
    assert_eq!(
        memory_inventory_v31::memory_inventory_headers_v31().unwrap(),
        size_of::<Frame<'_>>() + std::mem::align_of::<Frame<'_>>()
    );
    let (owner, owner_credit) = admit(&fixture());
    let mut work = Work::new(20_000_000);
    let mut budget = Budget::new(&mut work, 16 * 1024 * 1024);
    budget.reserve_storage(owner_credit + 37).unwrap();
    let (inventory, inventory_credit) = Inventory::derive_v18(&owner, &mut budget).unwrap();
    budget
        .reserve_storage(inventory_credit.retained_storage())
        .unwrap();
    let floor = budget.storage();
    let (independent, expected) = resources::scoped(&mut budget, |meter| {
        let census = census::derive(&inventory, meter)?;
        let expected = size_of::<OwnerFields<'_>>()
            + census.allocations.capacity() * size_of::<census::Allocation>()
            + census.cells.capacity() * size_of::<census::Cell>()
            + census.events.capacity() * size_of::<census::Event>();
        Ok::<_, Error>((census, expected))
    })
    .unwrap();
    drop(independent);
    assert_eq!(budget.storage(), floor);
    let (memory, credit) =
        CanonicalKirAggregateMemoryInventoryV31::derive(&inventory, &mut budget).unwrap();
    assert_eq!(credit.retained_storage(), expected);
    drop(memory);
    assert_eq!(budget.storage(), floor);
}
