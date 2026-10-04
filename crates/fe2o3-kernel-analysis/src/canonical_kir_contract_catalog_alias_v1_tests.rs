use super::*;

fn carried(conflicting_backedge: bool) -> Module {
    let mut source = module(1, 0);
    let function = &mut source.functions[0];
    function.signature.parameters.push(Type::BOOL);
    let body = function.body.as_mut().unwrap();
    body.parameters.push(ValueId(555));
    let mut entry = body.blocks.remove(0);
    let mut markers = entry.operations.split_off(1);
    for marker in &mut markers {
        let OperationKind::VerificationContract(
            VerificationContractOperationV12::WorkgroupPipelineEvent { storage, .. },
        ) = &mut marker.kind
        else {
            panic!("actual marker");
        };
        *storage = ValueId(800);
    }
    if conflicting_backedge {
        let mut other = entry.operations[0].clone();
        other.results[0].id = ValueId(901);
        entry.operations.push(other);
    }
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(7),
        arguments: vec![ValueId(900)],
    });
    let mut header = BasicBlock::new(BlockId(7));
    header.parameters.push(ValueDef::new(
        ValueId(800),
        entry.operations[0].results[0].ty.clone(),
    ));
    header.operations = markers;
    header.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(555),
        then_target: BlockId(7),
        then_arguments: vec![ValueId(if conflicting_backedge { 901 } else { 800 })],
        else_target: BlockId(8),
        else_arguments: vec![],
    });
    let mut exit = BasicBlock::new(BlockId(8));
    exit.terminator = Some(Terminator::Return { values: vec![] });
    body.blocks = vec![entry, header, exit];
    source
}

#[test]
fn grounded_marker_alias_binds_the_direct_allocation_without_an_alias_catalog_row() {
    let owner = admit(&carried(false));
    let catalog = catalog(&[contract()], &[binding(0)]);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(71).unwrap();
    let (inventory, inventory_storage) = Inventory::derive(&owner, &mut budget).unwrap();
    budget
        .reserve_storage(inventory_storage.retained_storage())
        .unwrap();
    let floor = budget.storage();
    let (checked, storage) =
        check_kernel_ir_contract_catalog_v1(&inventory, &catalog, &mut budget).unwrap();
    assert_eq!(checked.marker_count(), 6);
    assert_eq!(checked.catalog().bindings(), &[binding(0)]);
    assert_eq!(
        storage.retained_storage(),
        size_of::<CheckedKernelIrContractCatalogV1<'_, '_>>()
    );
    assert_eq!(budget.storage(), floor);
}

#[test]
fn conflicting_loop_origins_do_not_choose_one_available_catalog_binding() {
    let owner = admit(&carried(true));
    let catalog = catalog(&[contract()], &[binding(0)]);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(71).unwrap();
    let (inventory, storage) = Inventory::derive(&owner, &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let floor = budget.storage();
    assert_eq!(
        check_kernel_ir_contract_catalog_v1(&inventory, &catalog, &mut budget).unwrap_err(),
        BindingError::Invalid("marker storage has no unique allocation")
    );
    assert_eq!(budget.storage(), floor);
}

#[test]
fn block_parameter_cannot_be_forged_as_the_catalog_allocation() {
    let owner = admit(&carried(false));
    let catalog = catalog(
        &[contract()],
        &[Binding {
            storage: 800,
            block: 1,
            ..binding(0)
        }],
    );
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(71).unwrap();
    let (inventory, storage) = Inventory::derive(&owner, &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let floor = budget.storage();
    assert_eq!(
        check_kernel_ir_contract_catalog_v1(&inventory, &catalog, &mut budget).unwrap_err(),
        BindingError::Invalid("storage is not an allocation result")
    );
    assert_eq!(budget.storage(), floor);
}

#[test]
fn resolved_alias_must_still_match_the_exact_contract_key() {
    let mut source = carried(false);
    let OperationKind::VerificationContract(
        VerificationContractOperationV12::WorkgroupPipelineEvent { contract, .. },
    ) = &mut source.functions[0].body.as_mut().unwrap().blocks[1].operations[0].kind
    else {
        panic!("actual marker");
    };
    *contract = VerificationContractKeyV12::new(1);
    let owner = admit(&source);
    let catalog = catalog(&[super::contract()], &[binding(0)]);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(71).unwrap();
    let (inventory, storage) = Inventory::derive(&owner, &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let floor = budget.storage();
    assert_eq!(
        check_kernel_ir_contract_catalog_v1(&inventory, &catalog, &mut budget).unwrap_err(),
        BindingError::Invalid("marker contract differs from storage")
    );
    assert_eq!(budget.storage(), floor);
}
