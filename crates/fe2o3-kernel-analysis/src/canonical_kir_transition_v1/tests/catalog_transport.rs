//! Admitted input/output graphs and independently authored transition records.
use super::*;
use crate::{
    CheckedKernelIrContractCatalogV1, KernelIrContractCatalogTransportErrorV1 as TransportError,
    KernelIrPipelineAllocationTransportV1, KernelIrPipelineMarkerTransportV1,
    TransportedKernelIrContractCatalogV1, check_kernel_ir_contract_catalog_v1 as check_catalog,
    transport_kernel_ir_contract_catalog_v1 as transport,
};
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, InertCanonicalKernelIrContractCatalogV1 as Catalog,
    KernelIrPipelineContractDefinitionV1 as Contract, KernelIrPipelineStorageBindingV1 as Binding,
    TargetCapability, VerificationContractKeyV12, VerificationContractOperationV12,
    WorkgroupMemory, WorkgroupMemoryExtent, WorkgroupPipelineEventKindV12 as Event,
};

fn pointer() -> Type {
    Type::pointer(U32, AddressSpace::Workgroup, AccessMode::ReadWrite)
}
fn allocation(id: u32) -> Operation {
    value(
        id,
        pointer(),
        OperationKind::WorkgroupMemory(WorkgroupMemory {
            element: U32,
            extent: WorkgroupMemoryExtent::Static(4),
            alignment: 4,
        }),
    )
}
fn marker(storage: u32, epoch: u32, key: u32) -> Operation {
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
fn pipeline_module(parameters: Vec<Type>, ids: Vec<u32>, blocks: Vec<BasicBlock>) -> Module {
    let mut result = module(parameters, vec![], ids, blocks);
    result
        .required_capabilities
        .insert(TargetCapability::WorkgroupMemory);
    result.functions[0]
        .required_capabilities
        .insert(TargetCapability::WorkgroupMemory);
    result
}
fn contract(key: u32) -> Contract {
    Contract {
        key,
        semantic_pipeline_type: 20 + key,
        semantic_payload_type: 3,
        buffers: 2,
        elements: 2,
        prefetch_distance: 1,
        packed_bits: 32,
        source_size_bytes: 4,
        source_alignment_bytes: 4,
    }
}
fn binding(storage: u32, key: u32, block: u32, operation: u32) -> Binding {
    Binding {
        function: 0,
        storage,
        key,
        block,
        operation,
    }
}

fn with_catalog(
    a: &Inventory<'_>,
    b: &Inventory<'_>,
    rows: &Rows,
    floor: usize,
    contracts: &[Contract],
    bindings: &[Binding],
    inspect: impl FnOnce(
        &CheckedCanonicalKirTransitionV1<'_, '_, '_, '_>,
        &CheckedKernelIrContractCatalogV1<'_, '_>,
        &mut Budget<'_>,
    ),
) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = Budget::new(&mut work, floor + LIMIT);
    budget.reserve_storage(floor).unwrap();
    let (catalog, catalog_storage) =
        Catalog::from_rows_with_budget([7; 32], contracts, bindings, &mut budget).unwrap();
    budget
        .reserve_storage(catalog_storage.retained_storage())
        .unwrap();
    let (transition_storage, binding_storage) = {
        let (checked, transition_storage) =
            check_canonical_kir_transition_v1(a, b, rows.candidate(), &mut budget).unwrap();
        budget
            .reserve_storage(transition_storage.retained_storage())
            .unwrap();
        let (view, binding_storage) = check_catalog(a, &catalog, &mut budget).unwrap();
        budget
            .reserve_storage(binding_storage.retained_storage())
            .unwrap();
        inspect(&checked, &view, &mut budget);
        (transition_storage, binding_storage)
    };
    budget
        .release_storage(binding_storage.retained_storage())
        .unwrap();
    budget
        .release_storage(transition_storage.retained_storage())
        .unwrap();
    drop(catalog);
    budget
        .release_storage(catalog_storage.retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn moved_allocation_and_marker_keep_keys_while_unreachable_occurrences_are_omitted() {
    let mut entry = BasicBlock::new(BlockId(40));
    entry.operations = vec![
        constant(1, Constant::Index(0)),
        constant(2, Constant::Bool(true)),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(50),
        then_arguments: vec![],
        else_target: BlockId(60),
        else_arguments: vec![],
    });
    let input = pipeline_module(
        vec![],
        vec![],
        vec![
            entry,
            returning(50, vec![allocation(10), marker(10, 1, 0)], &[]),
            returning(60, vec![allocation(20), marker(20, 1, 1)], &[]),
        ],
    );
    let output = pipeline_module(
        vec![],
        vec![],
        vec![returning(
            40,
            vec![
                constant(1, Constant::Index(0)),
                allocation(30),
                marker(30, 1, 0),
            ],
            &[],
        )],
    );
    inspect(
        input,
        output,
        |a, b| {
            Plan {
                chains: vec![vec![(0, Some(edge(0, 0))), (1, None)]],
                operations: vec![
                    Origin::Retained(op(0, 0)),
                    Origin::Retained(op(1, 0)),
                    Origin::Retained(op(1, 1)),
                ],
                relations: vec![(1, 1, R), (10, 30, R)],
                uses: vec![operand(1, 1, 0), operand(1, 1, 1)],
                ..Plan::default()
            }
            .rows(a, b)
        },
        |a, b, rows, floor| {
            with_catalog(
                a,
                b,
                rows,
                floor,
                &[contract(0), contract(1)],
                &[binding(10, 0, 1, 0), binding(20, 1, 2, 0)],
                |checked, catalog, budget| {
                    let before = budget.storage();
                    let (output, receipt) = transport(checked, catalog, budget).unwrap();
                    budget.reserve_storage(receipt.retained_storage()).unwrap();
                    assert_eq!(output.input_identity(), a.identity());
                    assert_eq!(output.output_identity(), b.identity());
                    assert_eq!(
                        output.catalog().definitions(),
                        catalog.catalog().definitions()
                    );
                    assert_eq!(output.catalog().bindings(), &[binding(30, 0, 0, 1)]);
                    assert_eq!(
                        output.allocations(),
                        &[
                            KernelIrPipelineAllocationTransportV1 {
                                input: binding(10, 0, 1, 0),
                                output: Some(binding(30, 0, 0, 1))
                            },
                            KernelIrPipelineAllocationTransportV1 {
                                input: binding(20, 1, 2, 0),
                                output: None
                            },
                        ]
                    );
                    assert_eq!(
                        output.markers(),
                        &[
                            KernelIrPipelineMarkerTransportV1 {
                                input: op(1, 1),
                                output: Some(op(0, 2))
                            },
                            KernelIrPipelineMarkerTransportV1 {
                                input: op(2, 1),
                                output: None
                            },
                        ]
                    );
                    assert!(!output.grants_authority());
                    drop(output);
                    budget.release_storage(receipt.retained_storage()).unwrap();
                    assert_eq!(budget.storage(), before);
                },
            );
        },
    );
}

#[test]
fn grounded_loop_marker_alias_retains_only_the_physical_allocation_binding() {
    let mut entry = BasicBlock::new(BlockId(100));
    entry.operations = vec![allocation(0), constant(1, Constant::Index(0))];
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(200),
        arguments: vec![ValueId(0)],
    });
    let mut header = BasicBlock::new(BlockId(200));
    header
        .parameters
        .push(ValueDef::new(ValueId(500), pointer()));
    header.operations.push(marker(500, 1, 0));
    header.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(77),
        then_target: BlockId(200),
        then_arguments: vec![ValueId(500)],
        else_target: BlockId(300),
        else_arguments: vec![],
    });
    let input = pipeline_module(
        vec![Type::BOOL],
        vec![77],
        vec![entry, header, returning(300, vec![], &[])],
    );
    inspect(
        input.clone(),
        input,
        |a, _| Rows::identity(a),
        |a, b, rows, floor| {
            with_catalog(
                a,
                b,
                rows,
                floor,
                &[contract(0)],
                &[binding(0, 0, 0, 0)],
                |checked, catalog, budget| {
                    let before = budget.storage();
                    let (output, receipt) = transport(checked, catalog, budget).unwrap();
                    budget.reserve_storage(receipt.retained_storage()).unwrap();
                    assert_eq!(output.catalog().bindings(), &[binding(0, 0, 0, 0)]);
                    assert_eq!(
                        output.markers(),
                        &[KernelIrPipelineMarkerTransportV1 {
                            input: op(1, 0),
                            output: Some(op(1, 0))
                        }]
                    );
                    assert!(
                        output
                            .catalog()
                            .bindings()
                            .iter()
                            .all(|row| row.storage != 500)
                    );
                    drop(output);
                    budget.release_storage(receipt.retained_storage()).unwrap();
                    assert_eq!(budget.storage(), before);
                },
            );
        },
    );
}

#[test]
fn equal_graph_bytes_do_not_authorize_a_foreign_catalog_inventory() {
    let input = pipeline_module(
        vec![],
        vec![],
        vec![returning(
            9,
            vec![
                allocation(0),
                constant(1, Constant::Index(0)),
                marker(0, 1, 0),
            ],
            &[],
        )],
    );
    inspect(
        input.clone(),
        input,
        |a, _| Rows::identity(a),
        |a, b, rows, floor| {
            with_catalog(
                a,
                b,
                rows,
                floor,
                &[contract(0)],
                &[binding(0, 0, 0, 0)],
                |checked, catalog, budget| {
                    assert_eq!(a.identity(), b.identity());
                    assert!(!std::ptr::eq(a, b));
                    let before = budget.storage();
                    let storage = {
                        let (foreign, storage) =
                            check_catalog(b, catalog.catalog(), budget).unwrap();
                        budget.reserve_storage(storage.retained_storage()).unwrap();
                        let prefix = budget.work();
                        let peak = budget.peak_storage();
                        assert_eq!(
                            transport(checked, &foreign, budget).unwrap_err(),
                            TransportError::InputInventory
                        );
                        assert_eq!(budget.work(), prefix + 1);
                        assert_eq!(budget.peak_storage(), peak);
                        storage
                    };
                    budget.release_storage(storage.retained_storage()).unwrap();
                    assert_eq!(budget.storage(), before);
                },
            );
        },
    );
}

fn codec_work(contracts: usize, bindings: usize) -> usize {
    let wire = 56 + 48 * contracts + 20 * bindings;
    9 + 2 * contracts
        + 2 * bindings
        + 2 * wire
        + b"FE2O3/NATIVE-KIR-IMPORT-CONTRACT-CATALOG/V1\0".len()
}

#[test]
fn empty_and_nonempty_catalog_transport_have_independent_exact_resource_boundaries() {
    for nonempty in [false, true] {
        let input = if nonempty {
            pipeline_module(
                vec![],
                vec![],
                vec![returning(
                    9,
                    vec![
                        allocation(0),
                        constant(1, Constant::Index(0)),
                        marker(0, 1, 0),
                    ],
                    &[],
                )],
            )
        } else {
            Module::new("x")
        };
        let contracts = if nonempty { vec![contract(0)] } else { vec![] };
        let bindings = if nonempty {
            vec![binding(0, 0, 0, 0)]
        } else {
            vec![]
        };
        inspect(
            input.clone(),
            input,
            |a, _| Rows::identity(a),
            |a, b, rows, floor| {
                with_catalog(
                    a,
                    b,
                    rows,
                    floor,
                    &contracts,
                    &bindings,
                    |checked, catalog, setup| {
                        let n = usize::from(nonempty);
                        assert_eq!(a.operations().len(), 3 * n);
                        assert_eq!(b.definitions().len(), 2 * n);
                        assert_eq!(catalog.marker_count(), n);
                        // Nonempty transport: five entry/reserve units, three inverse
                        // initializations, three*(row+two three-unit lookups), sixteen
                        // allocation-descendant/append units, five marker scan/append/
                        // coverage units = 50. Empty transport has only the five units.
                        let local_work = if nonempty { 50 } else { 5 };
                        // Sorted allocation0/epoch1: direct checker11, alias derivation
                        // 9+2*operations+10*definitions=35, one two-unit query: 48.
                        let checker_work = if nonempty { 48 } else { 0 };
                        let exact_work = local_work + codec_work(n, n) + checker_work;
                        let audit = size_of::<TransportedKernelIrContractCatalogV1>()
                            - size_of::<Catalog>()
                            + n * size_of::<KernelIrPipelineAllocationTransportV1>()
                            + n * size_of::<KernelIrPipelineMarkerTransportV1>();
                        let scratch = size_of::<Vec<usize>>()
                            + size_of::<Vec<Binding>>()
                            + 3 * n * size_of::<usize>()
                            + n * size_of::<Binding>();
                        let catalog_payload = size_of::<Catalog>()
                            + 56
                            + 48 * n
                            + 20 * n
                            + n * size_of::<Contract>()
                            + n * size_of::<Binding>();
                        let checked_header = size_of::<CheckedKernelIrContractCatalogV1<'_, '_>>();
                        let checker_peak = if nonempty {
                            let word = size_of::<usize>();
                            (23 * word + 2 * (4 * word + 2)).max(9 * word + checked_header)
                        } else {
                            checked_header
                        };
                        let peak = audit + scratch + catalog_payload + checker_peak;
                        let retained = audit + catalog_payload;
                        let floor = setup.storage() + 19;
                        for (work_under, storage_under) in [(0, 0), (1, 0), (0, 1)] {
                            let mut work =
                                CanonicalKernelIrWorkBudgetV1::new(7 + exact_work - work_under);
                            let mut budget = Budget::new(&mut work, floor + peak - storage_under);
                            budget.charge_work(7).unwrap();
                            budget.reserve_storage(floor).unwrap();
                            let outcome = transport(checked, catalog, &mut budget);
                            assert_eq!(budget.storage(), floor);
                            match (work_under, storage_under) {
                                (0, 0) => {
                                    let (transported, receipt) = outcome.unwrap();
                                    assert_eq!(receipt.retained_storage(), retained);
                                    assert_eq!(budget.work(), 7 + exact_work);
                                    assert_eq!(budget.peak_storage(), floor + peak);
                                    budget.reserve_storage(retained).unwrap();
                                    drop(transported);
                                    budget.release_storage(retained).unwrap();
                                }
                                (1, 0) => {
                                    use fe2o3_kernel_ir::KernelIrContractCatalogErrorV1 as CodecError;
                                    let error = match outcome.unwrap_err() {
                                        TransportError::Resource(Resource::Work(error))
                                        | TransportError::Catalog(CodecError::Resource(
                                            Resource::Work(error),
                                        )) => error,
                                        error => panic!("unexpected transport rejection: {error}"),
                                    };
                                    assert_eq!(error.actual(), 7 + exact_work);
                                    assert_eq!(error.limit(), 6 + exact_work);
                                    assert_eq!(
                                        budget.work(),
                                        if nonempty { 6 + exact_work } else { 13 }
                                    );
                                    assert_eq!(
                                        budget.peak_storage(),
                                        floor
                                            + audit
                                            + scratch
                                            + catalog_payload
                                            + if nonempty { checker_peak } else { 0 }
                                    );
                                }
                                (0, 1) => {
                                    assert!(matches!(
                                        outcome,
                                        Err(TransportError::Binding(
                                            crate::KernelIrContractCatalogBindingErrorV1::Resource(
                                                Resource::Storage(_)
                                            )
                                        ))
                                    ));
                                    assert_eq!(budget.failed_storage(), Some(floor + peak));
                                }
                                _ => unreachable!(),
                            }
                        }
                    },
                );
            },
        );
    }
}
