use super::*;
use crate::check_kernel_ir_contract_catalog_v1;
use fe2o3_kernel_ir::{
    AccessMode, BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1, Constant, Function,
    KernelIrPipelineContractDefinitionV1 as Contract, KernelIrPipelineStorageBindingV1 as Binding,
    Module, Operation, ScalarType, Signature, TargetCapability, Terminator, ValueDef, ValueId,
    VerificationContractKeyV12, VerifiedCanonicalKernelIrModuleV12, WorkgroupMemory,
    WorkgroupMemoryExtent, WorkgroupPipelineEventKindV12 as Event,
};

const LIMIT: usize = 10_000_000;
const PREFIX: usize = 7;

#[path = "physical_cases.rs"]
mod physical_cases;

fn pointer() -> Type {
    Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Workgroup,
        AccessMode::ReadWrite,
    )
}
fn memory(id: u32) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), pointer()),
        OperationKind::WorkgroupMemory(WorkgroupMemory {
            element: Type::Scalar(ScalarType::U32),
            extent: WorkgroupMemoryExtent::Static(64),
            alignment: 4,
        }),
    )
}
fn index(id: u32) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), Type::INDEX),
        OperationKind::Constant(Constant::Index(0)),
    )
}
fn marker(key: u32, storage: u32, epoch: u32) -> Operation {
    Operation::new(
        vec![],
        OperationKind::VerificationContract(
            VerificationContractOperationV12::WorkgroupPipelineEvent {
                contract: VerificationContractKeyV12::new(key),
                kind: Event::Stage,
                storage: ValueId(storage),
                epoch: ValueId(epoch),
            },
        ),
    )
}
fn block(id: u32, parameter: Option<u32>) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(id));
    if let Some(parameter) = parameter {
        block
            .parameters
            .push(ValueDef::new(ValueId(parameter), pointer()));
    }
    block.terminator = Some(Terminator::Return { values: vec![] });
    block
}
fn branch(target: u32, value: u32) -> Option<Terminator> {
    Some(Terminator::Branch {
        target: BlockId(target),
        arguments: vec![ValueId(value)],
    })
}
fn conditional(a: u32, av: u32, b: u32, bv: u32) -> Option<Terminator> {
    Some(Terminator::ConditionalBranch {
        condition: ValueId(99),
        then_target: BlockId(a),
        then_arguments: vec![ValueId(av)],
        else_target: BlockId(b),
        else_arguments: vec![ValueId(bv)],
    })
}
fn module(blocks: Vec<BasicBlock>, parameters: bool) -> Module {
    let mut module = Module::new("physical-occurrences");
    let mut function = Function::internal_helper(
        "storage",
        Signature::new(
            if parameters {
                vec![Type::BOOL, pointer(), Type::INDEX]
            } else {
                vec![]
            },
            vec![],
        ),
        if parameters {
            vec![ValueId(99), ValueId(98), ValueId(97)]
        } else {
            vec![]
        },
        blocks,
    );
    function
        .required_capabilities
        .insert(TargetCapability::WorkgroupMemory);
    module
        .required_capabilities
        .insert(TargetCapability::WorkgroupMemory);
    module.functions.push(function);
    module
}
fn contract(key: u32) -> Contract {
    Contract {
        key,
        semantic_pipeline_type: 9 + key,
        semantic_payload_type: 3 + key,
        buffers: 2,
        elements: 32,
        prefetch_distance: 1,
        packed_bits: 32,
        source_size_bytes: 4,
        source_alignment_bytes: 4,
    }
}
fn binding(storage: u32, key: u32, operation: u32) -> Binding {
    Binding {
        function: 0,
        storage,
        key,
        block: 0,
        operation,
    }
}
fn direct() -> Module {
    let mut entry = block(700, None);
    entry.operations = vec![index(4), memory(900), marker(0, 900, 4)];
    module(vec![entry], false)
}

fn with_checked(
    source: Module,
    definitions: &[Contract],
    bindings: &[Binding],
    inspect: impl for<'i, 'g> FnOnce(
        &'i Inventory<'g>,
        &CheckedKernelIrContractCatalogV1<'i, 'g>,
        usize,
    ),
) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let (owner, owner_storage) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            &source,
            &mut budget,
        )
        .unwrap();
    budget
        .reserve_storage(owner_storage.retained_storage())
        .unwrap();
    let (catalog, catalog_storage) =
        Catalog::from_rows_with_budget([1; 32], definitions, bindings, &mut budget).unwrap();
    budget
        .reserve_storage(catalog_storage.retained_storage())
        .unwrap();
    let (inventory, inventory_storage) = Inventory::derive(&owner, &mut budget).unwrap();
    budget
        .reserve_storage(inventory_storage.retained_storage())
        .unwrap();
    let checked_storage = {
        let (checked, storage) =
            check_kernel_ir_contract_catalog_v1(&inventory, &catalog, &mut budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        inspect(&inventory, &checked, budget.storage() + 19);
        storage
    };
    budget
        .release_storage(checked_storage.retained_storage())
        .unwrap();
    drop(inventory);
    budget
        .release_storage(inventory_storage.retained_storage())
        .unwrap();
    drop(catalog);
    budget
        .release_storage(catalog_storage.retained_storage())
        .unwrap();
    drop(owner);
    budget
        .release_storage(owner_storage.retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), 0);
}

fn definition(inventory: &Inventory<'_>, value: u32) -> usize {
    inventory
        .definitions()
        .iter()
        .position(|row| row.value == Some(ValueId(value)))
        .unwrap()
}

#[test]
fn literal_allocation_marker_and_pointer_rows_belong_to_actual_source_inventory() {
    with_checked(
        direct(),
        &[contract(0)],
        &[binding(900, 0, 1)],
        |inventory, checked, floor| {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            budget.reserve_storage(floor).unwrap();
            let (report, storage) =
                CanonicalKirPhysicalOccurrencesV1::derive(inventory, checked, &mut budget).unwrap();
            assert_eq!(budget.storage(), floor);
            budget.reserve_storage(storage.retained_storage()).unwrap();
            assert!(std::ptr::eq(report.inventory(), inventory));
            assert!(std::ptr::eq(report.catalog(), checked.catalog()));
            assert!(!report.grants_authority());
            assert_eq!(report.occurrences().len(), 2);
            let allocation = definition(inventory, 900);
            assert_eq!(
                report.occurrences()[0].kind(),
                CanonicalKirPhysicalOccurrenceKindV1::Allocation {
                    effect: 0,
                    allocation: Some(allocation),
                    binding: Some(0)
                }
            );
            assert_eq!(
                report.occurrences()[1].kind(),
                CanonicalKirPhysicalOccurrenceKindV1::Contract {
                    storage_use: 0,
                    epoch_use: 1,
                    allocation,
                    binding: 0
                }
            );
            assert_eq!(report.pointer_uses().len(), 1);
            let pointer = &report.pointer_uses()[0];
            assert_eq!(
                pointer.address(),
                CanonicalKirPhysicalAddressV1::Allocation(allocation)
            );
            assert_eq!(
                pointer.role(),
                CanonicalKirPhysicalPointerRoleV1::ContractStorage
            );
            assert_eq!(
                report.use_for(pointer, &mut budget).unwrap().value,
                ValueId(900)
            );
            assert!(std::ptr::eq(
                report
                    .operation_for(&report.occurrences()[1], &mut budget)
                    .unwrap()
                    .operation,
                inventory.operations()[2].operation
            ));
            let view = report.function(0, &mut budget).unwrap();
            assert_eq!(view.function_ordinal(), 0);
            assert_eq!(view.occurrences().len(), 2);
            assert_eq!(view.pointer_uses().len(), 1);
            assert!(matches!(
                report.function(1, &mut budget),
                Err(Error::InvalidCoordinate)
            ));
            drop(report);
            budget.release_storage(storage.retained_storage()).unwrap();
            assert_eq!(budget.storage(), floor);
        },
    );
}

#[test]
fn independent_empty_and_declaration_work_storage_boundaries() {
    for declaration in [false, true] {
        let mut source = Module::new("empty-physical");
        if declaration {
            source.functions.push(Function::external_import(
                "external",
                Signature::new(vec![], vec![]),
            ));
        }
        with_checked(source, &[], &[], |inventory, checked, floor| {
            let functions = usize::from(declaration);
            assert_eq!(inventory.functions().len(), functions);
            assert!(inventory.blocks().is_empty());
            assert!(inventory.uses().is_empty());
            let retained = size_of::<CanonicalKirPhysicalOccurrencesV1<'_, '_>>()
                + functions * size_of::<FunctionRanges>();
            // Fixed 16+1+4, plus one reservation and one visit for a declaration.
            let exact_work = 21 + 2 * functions;
            for (work_under, storage_under) in [(0, 0), (1, 0), (0, 1)] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(PREFIX + exact_work - work_under);
                let mut budget = Budget::new(&mut work, floor + retained - storage_under);
                budget.charge_work(PREFIX).unwrap();
                budget.reserve_storage(floor).unwrap();
                match CanonicalKirPhysicalOccurrencesV1::derive(inventory, checked, &mut budget) {
                    Ok((report, storage)) => {
                        assert_eq!((work_under, storage_under), (0, 0));
                        assert_eq!(storage.retained_storage(), retained);
                        assert_eq!(budget.work(), PREFIX + exact_work);
                        assert_eq!(budget.peak_storage(), floor + retained);
                        assert!(report.occurrences().is_empty());
                        budget.reserve_storage(retained).unwrap();
                        drop(report);
                        budget.release_storage(retained).unwrap();
                    }
                    Err(Error::Resource(Resource::Work(error))) => {
                        assert_eq!(work_under, 1);
                        assert_eq!(error.actual(), PREFIX + exact_work);
                        assert_eq!(budget.work(), PREFIX + exact_work - 4);
                    }
                    Err(Error::Resource(Resource::Storage(error))) => {
                        assert_eq!(storage_under, 1);
                        assert_eq!(error.actual(), floor + retained);
                    }
                    other => panic!("unexpected empty boundary: {other:?}"),
                }
                assert_eq!(budget.storage(), floor);
            }
        });
    }
}

#[test]
fn independently_derived_alias_and_report_peaks_and_late_terminal_work() {
    with_checked(
        direct(),
        &[contract(0)],
        &[binding(900, 0, 1)],
        |inventory, checked, floor| {
            assert_eq!(
                (
                    inventory.functions().len(),
                    inventory.blocks().len(),
                    inventory.operations().len(),
                    inventory.definitions().len(),
                    inventory.uses().len(),
                    inventory.effects().len()
                ),
                (1, 1, 3, 2, 2, 1)
            );
            assert!(inventory.edge_arguments().is_empty());
            let (alias_peak, alias_retained) =
                crate::canonical_kir_must_alias_v1::tests::exact_payloads(2, 0);
            let report_retained = size_of::<CanonicalKirPhysicalOccurrencesV1<'_, '_>>()
                + size_of::<FunctionRanges>()
                + 2 * size_of::<CanonicalKirPhysicalOccurrenceV1>()
                + size_of::<CanonicalKirPhysicalPointerUseV1>();
            let peak = alias_peak.max(alias_retained + report_retained);
            assert!(alias_retained + report_retained > alias_peak);
            // Alias:4+2*3+5*(2+1)+3*2+2 enqueues+2 dequeues=35.
            // Report:21+2*3+2*2+1+1+8+4*2+3 reservations+2 query+2 searches+35=91.
            for (work_under, storage_under) in [(0, 0), (1, 0), (0, 1)] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(PREFIX + 91 - work_under);
                let mut budget = Budget::new(&mut work, floor + peak - storage_under);
                budget.charge_work(PREFIX).unwrap();
                budget.reserve_storage(floor).unwrap();
                match CanonicalKirPhysicalOccurrencesV1::derive(inventory, checked, &mut budget) {
                    Ok((report, storage)) => {
                        assert_eq!((work_under, storage_under), (0, 0));
                        assert_eq!(storage.retained_storage(), report_retained);
                        assert_eq!(budget.work(), PREFIX + 91);
                        assert_eq!(budget.peak_storage(), floor + peak);
                        budget.reserve_storage(report_retained).unwrap();
                        drop(report);
                        budget.release_storage(report_retained).unwrap();
                    }
                    Err(Error::Resource(Resource::Work(error))) => {
                        assert_eq!(work_under, 1);
                        assert_eq!(error.actual(), PREFIX + 91);
                        assert_eq!(budget.work(), PREFIX + 87);
                        assert_eq!(budget.peak_storage(), floor + peak);
                        assert_eq!(budget.failed_storage(), None);
                    }
                    Err(Error::Resource(Resource::Storage(error))) => {
                        assert_eq!(storage_under, 1);
                        assert_eq!(error.actual(), floor + peak);
                        assert_eq!(budget.work(), PREFIX + 59);
                        assert_eq!(
                            budget.peak_storage(),
                            (floor + alias_peak)
                                .max(floor + peak - size_of::<CanonicalKirPhysicalPointerUseV1>())
                        );
                    }
                    other => panic!("unexpected literal boundary: {other:?}"),
                }
                assert_eq!(budget.storage(), floor);
            }
            // The fifth two-element alias vector is the final alias scratch charge.
            let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
            let mut budget = Budget::new(&mut work, floor + alias_peak - 1);
            budget.charge_work(PREFIX).unwrap();
            budget.reserve_storage(floor).unwrap();
            let Err(Error::Resource(Resource::Storage(error))) =
                CanonicalKirPhysicalOccurrencesV1::derive(inventory, checked, &mut budget)
            else {
                panic!("missing alias scratch denial");
            };
            assert_eq!(error.actual(), floor + alias_peak);
            assert_eq!(budget.work(), PREFIX + 43);
            assert_eq!(
                budget.peak_storage(),
                floor + alias_peak - 2 * size_of::<usize>()
            );
            assert_eq!(budget.storage(), floor);
        },
    );
}

#[test]
fn exact_inventory_and_foreign_member_checks_do_not_use_structural_equality() {
    with_checked(
        direct(),
        &[contract(0)],
        &[binding(900, 0, 1)],
        |inventory, checked, floor| {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            budget.reserve_storage(floor).unwrap();
            let (other_inventory, other_storage) =
                Inventory::derive(inventory.owner(), &mut budget).unwrap();
            budget
                .reserve_storage(other_storage.retained_storage())
                .unwrap();
            let before = budget.work();
            assert!(matches!(
                CanonicalKirPhysicalOccurrencesV1::derive(&other_inventory, checked, &mut budget),
                Err(Error::InventoryMismatch)
            ));
            assert_eq!(budget.work(), before + 16);
            let (first, first_storage) =
                CanonicalKirPhysicalOccurrencesV1::derive(inventory, checked, &mut budget).unwrap();
            budget
                .reserve_storage(first_storage.retained_storage())
                .unwrap();
            let (second, second_storage) =
                CanonicalKirPhysicalOccurrencesV1::derive(inventory, checked, &mut budget).unwrap();
            budget
                .reserve_storage(second_storage.retained_storage())
                .unwrap();
            let before = budget.work();
            assert!(matches!(
                first.operation_for(&second.occurrences()[0], &mut budget),
                Err(Error::ForeignOccurrence)
            ));
            assert!(matches!(
                first.use_for(&second.pointer_uses()[0], &mut budget),
                Err(Error::ForeignPointerUse)
            ));
            assert_eq!(budget.work(), before + 6);
            drop(second);
            budget
                .release_storage(second_storage.retained_storage())
                .unwrap();
            drop(first);
            budget
                .release_storage(first_storage.retained_storage())
                .unwrap();
            drop(other_inventory);
            budget
                .release_storage(other_storage.retained_storage())
                .unwrap();
            assert_eq!(budget.storage(), floor);
        },
    );
}

fn inspect_addresses(source: Module, expected: &[(u32, Option<u32>)]) {
    with_checked(source, &[], &[], |inventory, checked, floor| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(floor).unwrap();
        let (report, storage) =
            CanonicalKirPhysicalOccurrencesV1::derive(inventory, checked, &mut budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        for &(value, expected) in expected {
            let rows = report
                .pointer_uses()
                .iter()
                .filter(|row| inventory.uses()[row.use_index()].value == ValueId(value))
                .collect::<Vec<_>>();
            assert!(!rows.is_empty(), "fixture must actually use %{value}");
            for row in rows {
                let actual = match row.address() {
                    CanonicalKirPhysicalAddressV1::Allocation(index) => {
                        inventory.definitions()[index].value.map(|v| v.0)
                    }
                    CanonicalKirPhysicalAddressV1::Unresolved(_) => None,
                    other => panic!("unexpected address: {other:?}"),
                };
                assert_eq!(actual, expected, "value %{value}");
            }
        }
        let edge_rows = report
            .pointer_uses()
            .iter()
            .filter_map(|row| match row.role() {
                CanonicalKirPhysicalPointerRoleV1::Forwarding {
                    edge_argument: Some(edge),
                    ..
                } => Some(edge),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            edge_rows,
            (0..inventory.edge_arguments().len()).collect::<Vec<_>>()
        );
        drop(report);
        budget.release_storage(storage.retained_storage()).unwrap();
    });
}

#[test]
fn grounded_loops_and_irreducible_joins_reuse_exact_edge_occurrences() {
    let mut entry = block(77, None);
    entry.operations = vec![memory(0)];
    entry.terminator = branch(18, 0);
    let mut header = block(18, Some(400));
    header.terminator = conditional(18, 400, 19, 400);
    let mut exit = block(19, Some(500));
    exit.terminator = branch(19, 500);
    inspect_addresses(
        module(vec![entry, header, exit], true),
        &[(0, Some(0)), (400, Some(0)), (500, Some(0))],
    );
    for different in [false, true] {
        let mut entry = block(77, None);
        entry.operations = vec![memory(0), memory(1)];
        entry.terminator = conditional(18, 0, 19, u32::from(different));
        let mut left = block(18, Some(400));
        left.terminator = conditional(19, 400, 20, 400);
        let mut right = block(19, Some(500));
        right.terminator = conditional(18, 500, 20, 500);
        let mut exit = block(20, Some(600));
        exit.terminator = branch(20, 600);
        let expected = (!different).then_some(0);
        inspect_addresses(
            module(vec![entry, left, right, exit], true),
            &[(400, expected), (500, expected), (600, expected)],
        );
    }
}

#[test]
fn repeated_edges_unknown_entries_and_ungrounded_cycles_remain_explicit() {
    for different in [false, true] {
        let mut entry = block(77, None);
        entry.operations = vec![memory(0), memory(1)];
        entry.terminator = conditional(18, 0, 18, u32::from(different));
        let mut exit = block(18, Some(400));
        exit.terminator = branch(18, 400);
        inspect_addresses(
            module(vec![entry, exit], true),
            &[(400, (!different).then_some(0))],
        );
    }
    let mut entry = block(77, None);
    entry.terminator = branch(20, 98);
    let mut left = block(18, Some(400));
    left.terminator = branch(19, 400);
    let mut right = block(19, Some(500));
    right.terminator = branch(18, 500);
    let mut exit = block(20, Some(600));
    exit.terminator = branch(20, 600);
    inspect_addresses(
        module(vec![entry, left, right, exit], true),
        &[(98, None), (400, None), (500, None), (600, None)],
    );
}

#[test]
fn two_typed_catalog_keys_bind_distinct_physical_allocations() {
    let mut entry = block(700, None);
    entry.operations = vec![
        index(4),
        memory(900),
        memory(901),
        marker(0, 900, 4),
        marker(1, 901, 4),
    ];
    with_checked(
        module(vec![entry], false),
        &[contract(0), contract(1)],
        &[binding(900, 0, 1), binding(901, 1, 2)],
        |inventory, checked, floor| {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            budget.reserve_storage(floor).unwrap();
            let (report, storage) =
                CanonicalKirPhysicalOccurrencesV1::derive(inventory, checked, &mut budget).unwrap();
            budget.reserve_storage(storage.retained_storage()).unwrap();
            let bindings = report
                .occurrences()
                .iter()
                .filter_map(|row| match row.kind() {
                    CanonicalKirPhysicalOccurrenceKindV1::Contract {
                        allocation,
                        binding,
                        ..
                    } => Some((
                        inventory.definitions()[allocation].value.unwrap().0,
                        report.catalog().bindings()[binding].key,
                    )),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(bindings, vec![(900, 0), (901, 1)]);
            assert_eq!(report.catalog().definitions()[1].semantic_pipeline_type, 10);
            drop(report);
            budget.release_storage(storage.retained_storage()).unwrap();
        },
    );
}
