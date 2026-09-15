//! Actual seven-pass output, not a fabricated optimization relation.
use super::*;
use fe2o3_kernel_analysis::{
    CheckedCanonicalKirTransitionV1, CheckedKernelIrContractCatalogV1,
    KernelIrContractCatalogTransportErrorV1 as TransportError,
    KernelIrPipelineAllocationTransportV1, KernelIrPipelineMarkerTransportV1,
    TransportedKernelIrContractCatalogV1, check_kernel_ir_contract_catalog_v1 as check_catalog,
    transport_kernel_ir_contract_catalog_v1 as transport,
};
use fe2o3_kernel_ir::{
    InertCanonicalKernelIrContractCatalogV1 as Catalog,
    KernelIrPipelineContractDefinitionV1 as Contract, KernelIrPipelineStorageBindingV1 as Binding,
    TargetCapability, WorkgroupMemory, WorkgroupMemoryExtent,
};

fn allocation(id: u32) -> Operation {
    Operation::effect_free(
        ValueDef::new(
            ValueId(id),
            Type::pointer(u32_type(), AddressSpace::Workgroup, AccessMode::ReadWrite),
        ),
        OperationKind::WorkgroupMemory(WorkgroupMemory {
            element: u32_type(),
            extent: WorkgroupMemoryExtent::Static(4),
            alignment: 4,
        }),
    )
}

fn marker(storage: u32, epoch: u32, key: u32, kind: WorkgroupPipelineEventKindV12) -> Operation {
    Operation::new(
        vec![],
        OperationKind::VerificationContract(
            VerificationContractOperationV12::WorkgroupPipelineEvent {
                contract: VerificationContractKeyV12::new(key),
                kind,
                storage: ValueId(storage),
                epoch: ValueId(epoch),
            },
        ),
    )
}

fn fixture() -> Module {
    use WorkgroupPipelineEventKindV12 as Event;
    let mut entry = BasicBlock::new(BlockId(77));
    entry.operations = vec![
        constant(101, Constant::Index(0)),
        constant(102, Constant::Index(0)),
        constant(103, Constant::Bool(true)),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(103),
        then_target: BlockId(20),
        then_arguments: vec![],
        else_target: BlockId(30),
        else_arguments: vec![],
    });
    let mut live = BasicBlock::new(BlockId(20));
    live.operations = vec![
        allocation(100),
        marker(100, 101, 0, Event::Stage),
        marker(100, 102, 0, Event::Commit),
    ];
    live.terminator = Some(Terminator::Branch {
        target: BlockId(40),
        arguments: vec![],
    });
    let mut dead = returning(30, &[]);
    dead.operations = vec![allocation(500), marker(500, 101, 1, Event::Discard)];
    let mut tail = returning(40, &[]);
    tail.operations = vec![
        marker(100, 102, 0, Event::Wait),
        marker(100, 101, 0, Event::Consume),
        marker(100, 102, 0, Event::Release),
    ];
    let mut module = function_module(vec![], vec![], vec![], vec![entry, live, dead, tail]);
    module
        .required_capabilities
        .insert(TargetCapability::WorkgroupMemory);
    module.functions[0]
        .required_capabilities
        .insert(TargetCapability::WorkgroupMemory);
    module
}

fn contracts() -> [Contract; 2] {
    [0, 1].map(|key| Contract {
        key,
        semantic_pipeline_type: 20 + key,
        semantic_payload_type: 3,
        buffers: 2,
        elements: 2,
        prefetch_distance: 1,
        packed_bits: 32,
        source_size_bytes: 4,
        source_alignment_bytes: 4,
    })
}

fn bindings() -> [Binding; 2] {
    [
        Binding {
            function: 0,
            storage: 100,
            key: 0,
            block: 1,
            operation: 0,
        },
        Binding {
            function: 0,
            storage: 500,
            key: 1,
            block: 2,
            operation: 0,
        },
    ]
}

fn with_catalog(
    input: &Inventory<'_>,
    budget: &mut Budget<'_>,
    inspect: impl FnOnce(&CheckedKernelIrContractCatalogV1<'_, '_>, &mut Budget<'_>),
) {
    with_catalog_rows(input, &contracts(), &bindings(), budget, inspect);
}

fn with_catalog_rows(
    input: &Inventory<'_>,
    definitions: &[Contract],
    bindings: &[Binding],
    budget: &mut Budget<'_>,
    inspect: impl FnOnce(&CheckedKernelIrContractCatalogV1<'_, '_>, &mut Budget<'_>),
) {
    let floor = budget.storage();
    let (catalog, storage) =
        Catalog::from_rows_with_budget([7; 32], definitions, bindings, budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let checked_storage = {
        let (checked, checked_storage) = check_catalog(input, &catalog, budget).unwrap();
        budget
            .reserve_storage(checked_storage.retained_storage())
            .unwrap();
        inspect(&checked, budget);
        checked_storage
    };
    budget
        .release_storage(checked_storage.retained_storage())
        .unwrap();
    drop(catalog);
    budget.release_storage(storage.retained_storage()).unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn equal_geometry_allocations_keep_distinct_identities_and_output_binding_order() {
    let mut block = returning(81, &[]);
    block.operations = vec![
        allocation(900),
        allocation(100),
        constant(5, Constant::Index(0)),
        marker(900, 5, 0, WorkgroupPipelineEventKindV12::Stage),
        marker(100, 5, 1, WorkgroupPipelineEventKindV12::Stage),
    ];
    let mut module = function_module(vec![], vec![], vec![], vec![block]);
    module
        .required_capabilities
        .insert(TargetCapability::WorkgroupMemory);
    module.functions[0]
        .required_capabilities
        .insert(TargetCapability::WorkgroupMemory);
    let source_bindings = [
        Binding {
            function: 0,
            storage: 100,
            key: 1,
            block: 0,
            operation: 1,
        },
        Binding {
            function: 0,
            storage: 900,
            key: 0,
            block: 0,
            operation: 0,
        },
    ];
    with_transition(&module, |observed, input, output, budget| {
        with_catalog_rows(
            input,
            &contracts(),
            &source_bindings,
            budget,
            |catalog, budget| {
                let receipt = {
                    let (checked, receipt) =
                        check(input, output, observed.occurrences().candidate(), budget).unwrap();
                    budget.reserve_storage(receipt.retained_storage()).unwrap();
                    let (result, storage) = transport(&checked, catalog, budget).unwrap();
                    budget.reserve_storage(storage.retained_storage()).unwrap();
                    let rows = result.catalog().bindings();
                    assert_eq!(rows.len(), 2);
                    assert!(
                        (rows[0].function, rows[0].storage) < (rows[1].function, rows[1].storage)
                    );
                    assert_eq!(
                        output
                            .operations()
                            .iter()
                            .filter(|operation| matches!(
                                operation.operation.kind,
                                OperationKind::WorkgroupMemory(_)
                            ))
                            .count(),
                        2
                    );
                    assert!(result.allocations().iter().all(|row| row.output.is_some()));
                    assert_ne!(
                        result.allocations()[0].output,
                        result.allocations()[1].output
                    );
                    for row in result.allocations() {
                        let output = row.output.unwrap();
                        assert_eq!(output.key, row.input.key);
                        assert!(rows.contains(&output));
                    }
                    drop(result);
                    budget.release_storage(storage.retained_storage()).unwrap();
                    receipt
                };
                budget.release_storage(receipt.retained_storage()).unwrap();
            },
        );
    });
}

#[test]
fn actual_cse_dead_control_and_block_merges_transport_native_catalog() {
    with_transition(&fixture(), |observed, input, output, budget| {
        assert_eq!(input.blocks().len(), 4);
        assert_eq!(output.blocks().len(), 1);
        with_catalog(input, budget, |input_catalog, budget| {
            let receipt = {
                let (checked, receipt) =
                    check(input, output, observed.occurrences().candidate(), budget).unwrap();
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                let floor = budget.storage();
                let (result, storage) = transport(&checked, input_catalog, budget).unwrap();
                assert_eq!(budget.storage(), floor);
                budget.reserve_storage(storage.retained_storage()).unwrap();
                assert_eq!(result.input_identity(), input.identity());
                assert_eq!(result.output_identity(), output.identity());
                assert!(!result.grants_authority());
                assert_eq!(result.catalog().semantic_source(), &[7; 32]);
                assert_eq!(result.catalog().definitions(), &contracts());
                assert_eq!(result.catalog().bindings().len(), 1);
                assert_eq!(result.allocations().len(), 2);
                assert_eq!(result.allocations()[0].input, bindings()[0]);
                assert_eq!(
                    result.allocations()[0].output,
                    Some(result.catalog().bindings()[0])
                );
                assert_eq!(result.allocations()[1].input, bindings()[1]);
                assert_eq!(result.allocations()[1].output, None);
                assert_eq!(result.catalog().bindings()[0].block, 0);
                assert_ne!(result.catalog().bindings()[0], bindings()[0]);
                assert_eq!(result.markers().len(), 6);
                assert_eq!(
                    result
                        .markers()
                        .iter()
                        .filter(|row| row.output.is_some())
                        .count(),
                    5
                );
                assert_eq!(
                    result
                        .markers()
                        .iter()
                        .filter(|row| row.output.is_none())
                        .count(),
                    1
                );
                let bound_storage = {
                    let (bound, bound_storage) =
                        check_catalog(output, result.catalog(), budget).unwrap();
                    assert_eq!(bound.marker_count(), 5);
                    budget
                        .reserve_storage(bound_storage.retained_storage())
                        .unwrap();
                    bound_storage
                };
                budget
                    .release_storage(bound_storage.retained_storage())
                    .unwrap();
                let expected = size_of::<TransportedKernelIrContractCatalogV1>()
                    + 2 * size_of::<KernelIrPipelineAllocationTransportV1>()
                    + 6 * size_of::<KernelIrPipelineMarkerTransportV1>()
                    + result.catalog().canonical_bytes().len()
                    + 2 * size_of::<Contract>()
                    + size_of::<Binding>();
                assert_eq!(storage.retained_storage(), expected);
                drop(result);
                budget.release_storage(storage.retained_storage()).unwrap();
                receipt
            };
            budget.release_storage(receipt.retained_storage()).unwrap();
        });
    });
}

#[test]
fn identical_graph_bytes_do_not_substitute_another_inventory() {
    with_transition(&fixture(), |observed, input, output, budget| {
        let (other, other_storage) = Inventory::derive(input.owner(), budget).unwrap();
        budget
            .reserve_storage(other_storage.retained_storage())
            .unwrap();
        assert_eq!(other.identity(), input.identity());
        with_catalog(&other, budget, |wrong_catalog, budget| {
            let receipt = {
                let (checked, receipt) =
                    check(input, output, observed.occurrences().candidate(), budget).unwrap();
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                let floor = budget.storage();
                assert_eq!(
                    transport(&checked, wrong_catalog, budget).unwrap_err(),
                    TransportError::InputInventory
                );
                assert_eq!(budget.storage(), floor);
                receipt
            };
            budget.release_storage(receipt.retained_storage()).unwrap();
        });
        drop(other);
        budget
            .release_storage(other_storage.retained_storage())
            .unwrap();
    });
}

fn profile(
    checked: &CheckedCanonicalKirTransitionV1<'_, '_, '_, '_>,
    catalog: &CheckedKernelIrContractCatalogV1<'_, '_>,
    floor: usize,
    work_limit: usize,
    storage_limit: usize,
    succeeds: bool,
) -> (usize, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.charge_work(7).unwrap();
    budget.reserve_storage(floor).unwrap();
    let outcome = transport(checked, catalog, &mut budget);
    assert_eq!(budget.storage(), floor);
    assert_eq!(outcome.is_ok(), succeeds);
    if let Ok((result, storage)) = outcome {
        budget.reserve_storage(storage.retained_storage()).unwrap();
        drop(result);
        budget.release_storage(storage.retained_storage()).unwrap();
    }
    assert_eq!(budget.storage(), floor);
    (budget.work(), budget.peak_storage())
}

fn catalog_codec_work(contracts: usize, bindings: usize) -> usize {
    let wire = 56 + 48 * contracts + 20 * bindings;
    9 + 2 * contracts
        + 2 * bindings
        + 2 * wire
        + b"FE2O3/NATIVE-KIR-IMPORT-CONTRACT-CATALOG/V1\0".len()
}

fn transport_payloads(
    operations: usize,
    allocations: usize,
    markers: usize,
    contracts: usize,
    survivors: usize,
) -> (usize, usize, usize) {
    let audit = size_of::<TransportedKernelIrContractCatalogV1>() - size_of::<Catalog>()
        + allocations * size_of::<KernelIrPipelineAllocationTransportV1>()
        + markers * size_of::<KernelIrPipelineMarkerTransportV1>();
    let scratch = size_of::<Vec<usize>>()
        + size_of::<Vec<Binding>>()
        + operations * size_of::<usize>()
        + allocations * size_of::<Binding>();
    let catalog = size_of::<Catalog>()
        + 56
        + 48 * contracts
        + 20 * survivors
        + contracts * size_of::<Contract>()
        + survivors * size_of::<Binding>();
    (audit, scratch, catalog)
}

#[test]
fn nonempty_transport_exact_and_one_under_budgets_restore_borrowed_floor() {
    with_transition(&fixture(), |observed, input, output, budget| {
        with_catalog(input, budget, |catalog, budget| {
            let receipt = {
                let (checked, receipt) =
                    check(input, output, observed.occurrences().candidate(), budget).unwrap();
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                let floor = budget.storage();

                // Pin the actual pass-output census before applying independent
                // accounting equations. No successful run supplies a limit.
                assert_eq!(input.operations().len(), 11);
                assert_eq!(output.operations().len(), 7);
                assert_eq!(catalog.catalog().definitions().len(), 2);
                assert_eq!(catalog.catalog().bindings().len(), 2);
                assert_eq!(catalog.marker_count(), 6);
                assert_eq!(
                    checked
                        .rows()
                        .operations
                        .iter()
                        .filter(|row| matches!(row.origin, Origin::Retained(_)))
                        .count(),
                    7
                );
                for (storage, descendants) in [(100, 1), (500, 0)] {
                    let definition = input
                        .definitions()
                        .iter()
                        .position(|row| row.value == Some(ValueId(storage)))
                        .unwrap();
                    assert_eq!(
                        checked.rows().definitions[definition].outputs.len,
                        descendants
                    );
                }
                assert_eq!(output.definitions().len(), 2);
                assert_eq!(output.edge_arguments().len(), 0);
                let mut values = [
                    output.definitions()[0].value.unwrap(),
                    output.definitions()[1].value.unwrap(),
                ];
                values.sort_unstable();
                let allocation = output
                    .operations()
                    .iter()
                    .filter(|row| matches!(row.operation.kind, OperationKind::WorkgroupMemory(_)))
                    .collect::<Vec<_>>();
                assert_eq!(allocation.len(), 1);
                let storage = allocation[0].operation.results[0].id;
                let epoch = values
                    .iter()
                    .copied()
                    .find(|value| *value != storage)
                    .unwrap();
                let mut marker_count = 0;
                for row in output.operations() {
                    if let OperationKind::VerificationContract(
                        VerificationContractOperationV12::WorkgroupPipelineEvent {
                            storage: actual_storage,
                            epoch: actual_epoch,
                            ..
                        },
                    ) = &row.operation.kind
                    {
                        assert_eq!((*actual_storage, *actual_epoch), (storage, epoch));
                        marker_count += 1;
                    }
                }
                assert_eq!(marker_count, 5);
                // The sorted two-definition index probes the high entry first.
                let depth = |value| if value == values[0] { 2 } else { 1 };
                let direct_checker = 1 + 2 * depth(storage) + 7 + 5 * (1 + 2 * depth(epoch));
                // Alias derivation: fixed entry, operation census, five vector
                // allocations/initializations, definition/operation initialization,
                // seed visits, enqueue/dequeue, unresolved-cycle closure; then queries.
                let alias_work = 4 + 7 + 5 * (1 + 2) + 2 + 7 + 2 + 2 + 2 + 2;
                assert_eq!(alias_work, 43);
                let work = 7 + 114 + catalog_codec_work(2, 1) + direct_checker + alias_work + 5 * 2;
                let word = size_of::<usize>();
                let checked_header = size_of::<CheckedKernelIrContractCatalogV1<'_, '_>>();
                let alias_peak =
                    (23 * word + 2 * (4 * word + 2)).max(5 * word + 2 * 2 * word + checked_header);
                let (audit, scratch, catalog_payload) = transport_payloads(11, 2, 6, 2, 1);
                let peak = floor + audit + scratch + catalog_payload + alias_peak;
                assert_eq!(
                    profile(&checked, catalog, floor, work, peak, true),
                    (work, peak)
                );
                profile(&checked, catalog, floor, work - 1, peak, false);
                profile(&checked, catalog, floor, work, peak - 1, false);
                receipt
            };
            budget.release_storage(receipt.retained_storage()).unwrap();
        });
    });
}

#[test]
fn empty_transport_has_independent_exact_budgets_with_a_nonzero_borrowed_floor() {
    with_transition(
        &Module::new("empty-native-transport"),
        |observed, input, output, budget| {
            with_catalog_rows(input, &[], &[], budget, |catalog, budget| {
                let receipt = {
                    let (checked, receipt) =
                        check(input, output, observed.occurrences().candidate(), budget).unwrap();
                    budget.reserve_storage(receipt.retained_storage()).unwrap();
                    assert!(input.operations().is_empty());
                    assert!(output.operations().is_empty());
                    assert!(checked.rows().operations.is_empty());
                    assert_eq!(catalog.marker_count(), 0);
                    let floor = budget.storage();
                    assert!(floor > 0);
                    let work = 7 + 5 + catalog_codec_work(0, 0);
                    let (audit, scratch, catalog_payload) = transport_payloads(0, 0, 0, 0, 0);
                    let peak = floor
                        + audit
                        + scratch
                        + catalog_payload
                        + size_of::<CheckedKernelIrContractCatalogV1<'_, '_>>();
                    assert_eq!(
                        profile(&checked, catalog, floor, work, peak, true),
                        (work, peak)
                    );
                    profile(&checked, catalog, floor, work - 1, peak, false);
                    profile(&checked, catalog, floor, work, peak - 1, false);
                    receipt
                };
                budget.release_storage(receipt.retained_storage()).unwrap();
            });
        },
    );
}
