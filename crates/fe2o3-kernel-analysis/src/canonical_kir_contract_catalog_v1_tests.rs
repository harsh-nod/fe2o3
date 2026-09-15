use super::*;
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1, Function,
    KernelIrPipelineContractDefinitionV1 as Contract, Module, Operation, Signature,
    TargetCapability, Terminator, ValueDef, VerificationContractKeyV12,
    VerifiedCanonicalKernelIrModuleV12, WorkgroupMemory, WorkgroupPipelineEventKindV12 as Event,
};

const LIMIT: usize = 1_000_000;

#[path = "canonical_kir_contract_catalog_alias_v1_tests.rs"]
mod aliases;

fn contract() -> Contract {
    Contract {
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
fn binding(function: u32) -> Binding {
    Binding {
        function,
        storage: 900,
        key: 0,
        block: 0,
        operation: 0,
    }
}

fn module(function_count: usize, marker_key: u32) -> Module {
    let mut module = Module::new("pipeline-catalog");
    module
        .required_capabilities
        .insert(TargetCapability::WorkgroupMemory);
    for ordinal in 0..function_count {
        let mut block = BasicBlock::new(BlockId(812));
        block.operations.push(Operation::effect_free(
            ValueDef::new(
                ValueId(900),
                Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Workgroup,
                    AccessMode::ReadWrite,
                ),
            ),
            OperationKind::WorkgroupMemory(WorkgroupMemory {
                element: Type::Scalar(ScalarType::U32),
                extent: WorkgroupMemoryExtent::Static(64),
                alignment: 4,
            }),
        ));
        for kind in [
            Event::Stage,
            Event::Commit,
            Event::Wait,
            Event::Consume,
            Event::Discard,
            Event::Release,
        ] {
            block.operations.push(Operation::new(
                vec![],
                OperationKind::VerificationContract(
                    VerificationContractOperationV12::WorkgroupPipelineEvent {
                        contract: VerificationContractKeyV12::new(marker_key),
                        kind,
                        storage: ValueId(900),
                        epoch: ValueId(444),
                    },
                ),
            ));
        }
        block.terminator = Some(Terminator::Return { values: vec![] });
        let mut function = Function::internal_helper(
            format!("f{ordinal}"),
            Signature::new(vec![Type::INDEX], vec![]),
            vec![ValueId(444)],
            vec![block],
        );
        function
            .required_capabilities
            .insert(TargetCapability::WorkgroupMemory);
        module.functions.push(function);
    }
    module
}

fn admit(module: &Module) -> VerifiedCanonicalKernelIrModuleV12 {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
        module,
        &mut budget,
    )
    .unwrap()
    .0
}

fn catalog(definitions: &[Contract], bindings: &[Binding]) -> Catalog {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    Catalog::from_rows_with_budget([1; 32], definitions, bindings, &mut budget)
        .unwrap()
        .0
}

#[test]
fn all_six_markers_bind_sparse_allocations_in_each_actual_function() {
    let owner = admit(&module(2, 0));
    let other = admit(&module(2, 0));
    let catalog = catalog(&[contract()], &[binding(0), binding(1)]);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let (inventory, storage) = Inventory::derive(&owner, &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let floor = budget.storage();
    let (checked, receipt) =
        check_kernel_ir_contract_catalog_v1(&inventory, &catalog, &mut budget).unwrap();
    assert_eq!(checked.marker_count(), 12);
    assert!(checked.inventory().belongs_to(&owner));
    assert!(!checked.inventory().belongs_to(&other));
    assert!(std::ptr::eq(checked.catalog(), &catalog));
    assert!(!checked.grants_authority());
    assert_eq!(budget.storage(), floor);
    assert_eq!(
        receipt.retained_storage(),
        size_of::<CheckedKernelIrContractCatalogV1<'_, '_>>()
    );
}

#[test]
fn rejects_actual_marker_key_and_missing_function_binding() {
    for (owner, catalog) in [
        (admit(&module(1, 1)), catalog(&[contract()], &[binding(0)])),
        (admit(&module(2, 0)), catalog(&[contract()], &[binding(0)])),
        (admit(&module(1, 0)), catalog(&[contract()], &[])),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let (inventory, storage) = Inventory::derive(&owner, &mut budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let floor = budget.storage();
        assert!(check_kernel_ir_contract_catalog_v1(&inventory, &catalog, &mut budget).is_err());
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn rejects_each_mismatched_allocation_coordinate() {
    let owner = admit(&module(1, 0));
    let mut rows = [binding(0); 4];
    rows[0].function = 2;
    rows[1].storage = 444;
    rows[2].block = 812;
    rows[3].operation = 1;
    for row in rows {
        let catalog = catalog(&[contract()], &[row]);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let (inventory, storage) = Inventory::derive(&owner, &mut budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        assert!(check_kernel_ir_contract_catalog_v1(&inventory, &catalog, &mut budget).is_err());
    }
}

#[test]
fn rejects_valid_catalog_with_different_actual_geometry_or_payload() {
    let owner = admit(&module(1, 0));
    let mut rows = [contract(); 3];
    rows[0].elements = 16;
    rows[1].source_alignment_bytes = 8;
    rows[2].packed_bits = 64;
    rows[2].source_size_bytes = 8;
    for row in rows {
        let catalog = catalog(&[row], &[binding(0)]);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let (inventory, storage) = Inventory::derive(&owner, &mut budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        assert!(check_kernel_ir_contract_catalog_v1(&inventory, &catalog, &mut budget).is_err());
    }
}

#[test]
fn empty_catalog_checks_empty_graph_with_exact_inline_storage() {
    let owner = admit(&Module::new("empty"));
    let catalog = catalog(&[], &[]);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let (inventory, _) = Inventory::derive(&owner, &mut budget).unwrap();
    let retained = size_of::<CheckedKernelIrContractCatalogV1<'_, '_>>();
    for (storage, accepts) in [(retained, true), (retained - 1, false)] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
        let mut budget = Budget::new(&mut work, storage);
        let result = check_kernel_ir_contract_catalog_v1(&inventory, &catalog, &mut budget);
        assert_eq!(result.is_ok(), accepts);
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn nonempty_binding_work_and_storage_are_exact_without_peak_calibration() {
    for (function_count, exact_work) in [(1, 95), (2, 191)] {
        let source = module(function_count, 0);
        let bindings = (0..function_count as u32).map(binding).collect::<Vec<_>>();
        let mut setup_work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut setup = Budget::new(&mut setup_work, LIMIT);
        let (owner, owner_storage) =
            VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
                &source, &mut setup,
            )
            .unwrap();
        setup
            .reserve_storage(owner_storage.retained_storage())
            .unwrap();
        let (catalog, catalog_storage) =
            Catalog::from_rows_with_budget([1; 32], &[contract()], &bindings, &mut setup).unwrap();
        setup
            .reserve_storage(catalog_storage.retained_storage())
            .unwrap();
        let (inventory, inventory_storage) = Inventory::derive(&owner, &mut setup).unwrap();
        setup
            .reserve_storage(inventory_storage.retained_storage())
            .unwrap();
        assert_eq!(inventory.definitions().len(), 2 * function_count);
        assert_eq!(inventory.operations().len(), 7 * function_count);
        let owned = owner_storage.retained_storage()
            + catalog_storage.retained_storage()
            + inventory_storage.retained_storage();
        setup.release_storage(owned).unwrap();
        // Transfer the same caller-owned inputs between isolated boundary ledgers.
        // Direct binding/marker checks cost 40/90 from the exact sorted keys.
        // With V=2F, O=7F and no dependencies, origin analysis adds
        // 4 + 2O + 5*(1+V) + 3V + V enqueues + V dequeues = 9+34F.
        // Each of 6F marker queries adds two fixed units: totals 95 and 191.
        // Engine header: 23 words. Each definition: four words + two flags.
        // The report plus checked-view headers coexist below this engine peak.
        let word = size_of::<usize>();
        let peak = 23 * word + 2 * function_count * (4 * word + 2);
        let retained = size_of::<CheckedKernelIrContractCatalogV1<'_, '_>>();
        for (work_under, storage_under) in [(0, 0), (1, 0), (0, 1)] {
            let floor = 61 + owned;
            let mut work = CanonicalKernelIrWorkBudgetV1::new(11 + exact_work - work_under);
            let mut budget = Budget::new(&mut work, floor + peak - storage_under);
            budget.reserve_storage(floor).unwrap();
            budget.charge_work(11).unwrap();
            let result = check_kernel_ir_contract_catalog_v1(&inventory, &catalog, &mut budget);
            let returned_storage = match result {
                Ok((checked, receipt)) => {
                    assert_eq!((work_under, storage_under), (0, 0));
                    assert_eq!(checked.marker_count(), 6 * function_count);
                    assert_eq!(receipt.retained_storage(), retained);
                    assert_eq!(budget.work(), 11 + exact_work);
                    assert_eq!(budget.peak_storage(), floor + peak);
                    budget.reserve_storage(receipt.retained_storage()).unwrap();
                    Some(receipt.retained_storage())
                }
                Err(BindingError::Inventory(CanonicalKirInventoryErrorV1::Resource(
                    Resource::Work(_),
                ))) => {
                    assert_eq!((work_under, storage_under), (1, 0));
                    None
                }
                Err(BindingError::Resource(Resource::Storage(_))) => {
                    assert_eq!((work_under, storage_under), (0, 1));
                    None
                }
                Err(error) => panic!("unexpected exact-boundary rejection: {error}"),
            };
            if let Some(retained) = returned_storage {
                budget.release_storage(retained).unwrap();
            }
            assert_eq!(budget.storage(), floor);
            budget.release_storage(owned).unwrap();
            assert_eq!(budget.storage(), 61);
        }
        drop(inventory);
        drop(catalog);
        drop(owner);
    }
}

#[test]
fn malformed_epoch_and_marker_results_reject_at_real_owner_admission() {
    let mut wrong_epoch = module(1, 0);
    wrong_epoch.functions[0].signature.parameters[0] = Type::Scalar(ScalarType::U32);
    let mut marker_result = module(1, 0);
    marker_result.functions[0].body.as_mut().unwrap().blocks[0].operations[1]
        .results
        .push(ValueDef::new(ValueId(901), Type::Scalar(ScalarType::U32)));
    for source in [wrong_epoch, marker_result] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(67).unwrap();
        let result =
            VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
                &source,
                &mut budget,
            );
        assert!(matches!(
            result,
            Err(fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV12::Verification(_))
        ));
        assert_eq!(budget.storage(), 67);
    }
}

#[test]
fn otherwise_admitted_workgroup_pointer_result_must_be_an_allocation() {
    let mut source = module(1, 0);
    let pointer = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Workgroup,
        AccessMode::ReadWrite,
    );
    source.functions[0]
        .signature
        .parameters
        .extend([pointer, Type::BOOL]);
    let body = source.functions[0].body.as_mut().unwrap();
    body.parameters.extend([ValueId(333), ValueId(555)]);
    body.blocks[0].operations[0].kind = OperationKind::Select {
        condition: ValueId(555),
        true_value: ValueId(333),
        false_value: ValueId(333),
    };
    let owner = admit(&source);
    let catalog = catalog(&[contract()], &[binding(0)]);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(71).unwrap();
    let (inventory, storage) = Inventory::derive(&owner, &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let floor = budget.storage();
    assert!(matches!(
        check_kernel_ir_contract_catalog_v1(&inventory, &catalog, &mut budget),
        Err(BindingError::Invalid("allocation producer kind"))
    ));
    assert_eq!(budget.storage(), floor);
    drop(inventory);
    budget.release_storage(storage.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 71);
}
