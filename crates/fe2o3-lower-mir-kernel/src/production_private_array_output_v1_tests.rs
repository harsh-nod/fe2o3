use super::*;
use fe2o3_kernel_analysis::check_canonical_kir_coordinate_preservation_v1;
use fe2o3_pliron::{CheckedNeutralKernelIrOwnerV1, KirPlironGraphV12};

include!("production_private_array_initializer_output_v1_tests.rs");

fn optimize(
    input: &VerifiedCanonicalKernelIrModuleV12,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> CheckedNeutralKernelIrOwnerV1 {
    let (mut graph, storage) = KirPlironGraphV12::import(input, budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let observed = graph
        .execute_production_neutral_optimization_v1(budget)
        .unwrap()
        .extract()
        .unwrap();
    assert_eq!(observed.report().passes().len(), 7);
    budget
        .reserve_storage(observed.storage().retained_storage())
        .unwrap();
    let graph_storage = graph.retained_storage();
    drop(graph);
    budget.release_storage(graph_storage).unwrap();
    let checked = observed.try_check_and_finish_v1(budget).unwrap();
    budget
        .reserve_storage(checked.storage().retained_storage())
        .unwrap();
    checked
}

// N and B are the same borrowed neutral endpoint in these lowerer tests. The
// codegen companion separately exercises the actual target binder on both GPUs.
fn with_output(
    source: ProductionPreRankedKirOwnerV1,
    body: impl FnOnce(&mut ProductionSourceOutputOccurrencesV1<'_, '_>, &mut AssertOriginBudgetV1<'_>),
) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let source_storage = retained(&source);
    budget.reserve_storage(source_storage).unwrap();
    let checked = optimize(source.executable(), &mut budget);
    let (coordinates, coordinate_storage) = check_canonical_kir_coordinate_preservation_v1(
        source.executable(),
        source.executable(),
        &mut budget,
    )
    .unwrap();
    budget
        .reserve_storage(coordinate_storage.retained_storage())
        .unwrap();
    let incoming = budget.storage();
    let (mut view, storage) =
        derive_source_output_occurrences_v1(&source, &coordinates, &checked, &mut budget).unwrap();
    assert_eq!(budget.storage(), incoming);
    budget.reserve_storage(storage.retained_storage()).unwrap();
    assert_eq!(view.storage, storage);
    let live = budget.storage();
    body(&mut view, &mut budget);
    assert_eq!(budget.storage(), live);
    drop(view);
    budget.release_storage(storage.retained_storage()).unwrap();
    #[allow(
        clippy::drop_non_drop,
        reason = "End the borrowed witness before releasing its ledger reservation"
    )]
    drop(coordinates);
    budget
        .release_storage(coordinate_storage.retained_storage())
        .unwrap();
    let checked_storage = checked.storage().retained_storage();
    drop(checked);
    budget.release_storage(checked_storage).unwrap();
    drop(source);
    budget.release_storage(source_storage).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

fn site(block: u32, statement: u32) -> Site {
    Site::Statement {
        block: SsaBlockIdV1::new(block),
        statement,
    }
}

fn operations(owner: &VerifiedCanonicalKernelIrModuleV12) -> usize {
    owner
        .module()
        .functions
        .iter()
        .filter_map(|function| function.body.as_ref())
        .flat_map(|body| &body.blocks)
        .map(|block| block.operations.len())
        .sum()
}

fn with_control(
    view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
    budget: &mut AssertOriginBudgetV1<'_>,
    body: impl FnOnce(
        &fe2o3_kernel_analysis::CheckedCanonicalKirControlIndexV1<'_, '_, '_>,
        &fe2o3_kernel_analysis::CheckedCanonicalKirTransitionV1<'_, '_, '_, '_>,
        usize,
    ),
) {
    let floor = budget.storage();
    let (input, input_storage) =
        fe2o3_kernel_analysis::CanonicalKirInventoryV1::derive(view.bound(), budget).unwrap();
    budget
        .reserve_storage(input_storage.retained_storage())
        .unwrap();
    let (output, output_storage) =
        fe2o3_kernel_analysis::CanonicalKirInventoryV1::derive(view.output(), budget).unwrap();
    budget
        .reserve_storage(output_storage.retained_storage())
        .unwrap();
    let (transition, transition_storage) =
        fe2o3_kernel_analysis::check_canonical_kir_transition_v1(
            &input,
            &output,
            view.checked_output.occurrences().candidate(),
            budget,
        )
        .unwrap();
    budget
        .reserve_storage(transition_storage.retained_storage())
        .unwrap();
    let (control, control_storage) =
        fe2o3_kernel_analysis::CheckedCanonicalKirControlIndexV1::derive(&transition, budget)
            .unwrap();
    budget
        .reserve_storage(control_storage.retained_storage())
        .unwrap();
    body(&control, &transition, budget.storage());
    drop(control);
    budget
        .release_storage(control_storage.retained_storage())
        .unwrap();
    #[allow(
        clippy::drop_non_drop,
        reason = "End the borrowed witness before releasing its ledger reservation"
    )]
    drop(transition);
    budget
        .release_storage(transition_storage.retained_storage())
        .unwrap();
    drop(output);
    budget
        .release_storage(output_storage.retained_storage())
        .unwrap();
    drop(input);
    budget
        .release_storage(input_storage.retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn genuine_checked_use_capture_has_exact_typed_payload_and_failure_cleanup() {
    with_output(
        array_owner(ArrayCase::Write { sparse: false }),
        |view, live_budget| {
            with_control(view, live_budget, |control, _, floor| {
                // This second capture has its own Vec header, in addition to the
                // already-live view header. Production prepays it before capture.
                let floor = floor + std::mem::size_of::<Vec<SourceOutputArrayRowV1>>();
                let payload = std::mem::size_of::<SourceOutputArrayRowV1>();
                // Component ledgers isolate the added capture, over real checked
                // inventories. Header is charged separately by the view constructor.
                // Entry/reserve3 + instance4 + source/slot17 + coordinates6 +
                // block3 + five use queries30 + outcomes6 + anchors21 + push/census2.
                for (limit, accepted, attempted) in [(92, 92, None), (91, 91, Some(92))] {
                    let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
                    let mut budget = AssertOriginBudgetV1::new(&mut work, floor + payload);
                    budget.reserve_storage(floor).unwrap();
                    let result = source_output_array_rows_v1(view.source(), control, &mut budget);
                    if let Some(attempted) = attempted {
                        assert!(
                            matches!(result, Err(ProductionSourceOutputErrorV1::Resource(AssertOriginResourceV1::Work(error)))
                        if error.actual() == attempted && error.limit() == limit)
                        );
                    } else {
                        let (rows, bytes) = result.unwrap();
                        assert_eq!((rows.len(), bytes), (1, payload));
                        assert!(matches!(
                            rows[0].placement,
                            SourceOutputArrayPlacementV1::Retained(_)
                        ));
                        drop(rows);
                    }
                    // The constructor normally performs this unwind/return cleanup;
                    // in the isolated call all returned or rejected rows are dropped.
                    assert_eq!(
                        (budget.work(), budget.storage(), budget.peak_storage()),
                        (accepted, floor + payload, floor + payload)
                    );
                    budget.release_storage(payload).unwrap();
                    assert_eq!(budget.storage(), floor);
                }
                let mut work = CanonicalKernelIrWorkBudgetV1::new(92);
                let mut budget = AssertOriginBudgetV1::new(&mut work, floor + payload - 1);
                budget.reserve_storage(floor).unwrap();
                assert!(
                    matches!(source_output_array_rows_v1(view.source(), control, &mut budget),
                Err(ProductionSourceOutputErrorV1::Resource(AssertOriginResourceV1::Storage(error)))
                    if error.actual() == floor + payload && error.limit() == floor + payload - 1)
                );
                assert_eq!(
                    (budget.work(), budget.storage(), budget.peak_storage()),
                    (2, floor, floor)
                );
            });
        },
    );
}

#[test]
fn genuine_sparse_write_uses_checked_output_operands_not_original_spans() {
    with_output(
        array_owner(ArrayCase::Write { sparse: true }),
        |view, budget| {
            assert_eq!(
                (
                    operations(view.source().executable()),
                    operations(view.output())
                ),
                (7, 6)
            );
            assert_ne!(
                view.source().executable().canonical().identity(),
                view.output().canonical().identity()
            );
            assert!(!view.grants_authority());
            let before = view
                .source()
                .executable()
                .canonical()
                .canonical_bytes()
                .to_vec();
            let original = &view.source.correspondence.private_arrays.effects[0];
            assert_eq!(
                (
                    original.semantic_block,
                    original.memory_location.block_ordinal,
                    original.memory_location.operation
                ),
                (2, 1, 4)
            );
            let PrivateArrayIndexV1::Local {
                direct_definition: Some(original_index),
                ..
            } = original.original_index
            else {
                panic!("actual direct unsigned source definition required")
            };
            let old_body = view.source().executable().module().functions[0]
                .body
                .as_ref()
                .unwrap();
            assert!(matches!(
                old_body.blocks[original_index.block_ordinal].operations[original_index.operation]
                    .kind,
                OperationKind::Constant(Constant::U32(0))
            ));
            assert!(
                !view.output().module().functions[0]
                    .body
                    .as_ref()
                    .unwrap()
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .any(|operation| matches!(
                        operation.kind,
                        OperationKind::Constant(Constant::U32(0))
                    ))
            );
            let outcome = view
                .private_array_write(
                    ARRAY_ROOT,
                    ARRAY_ROOT,
                    site(2, 1),
                    Role::Destination,
                    budget,
                )
                .unwrap();
            let ProductionSourceOutputPrivateArrayAccessV1::Retained {
                index,
                coordinate,
                executable,
            } = outcome
            else {
                panic!("actual output write placement required")
            };
            assert_eq!((index, executable, coordinate.effect), (0, true, 0));
            assert_eq!(
                (
                    coordinate.operation.block.function.0,
                    coordinate.operation.block.block,
                    coordinate.operation.operation
                ),
                (0, 0, 5)
            );
            let operation =
                source_output_operation_v1(view.output(), coordinate.operation, budget).unwrap();
            assert!(matches!(operation.kind, OperationKind::Store { access, .. }
            if access.address_space == AddressSpace::Private && access.alignment == 4 && !access.volatile));
            assert_eq!(
                view.source().executable().canonical().canonical_bytes(),
                before
            );
        },
    );
}

#[test]
fn actual_output_query_has_literal_work_and_live_storage_boundaries() {
    with_output(
        array_owner(ArrayCase::Write { sparse: false }),
        |view, live_budget| {
            // These are query-only component ledgers over genuine prebuilt owners,
            // not a production reset. The preceding test uses one live ledger end to end.
            // N facade280 + floor4 + source/row29 + placement/ancestry15 +
            // two Index definitions20 + three result lookups18 + physical45 =411.
            let floor = live_budget.storage();
            for (limit, accepted, attempted) in [(411, 411, None), (410, 410, Some(411))] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
                let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
                budget.reserve_storage(floor).unwrap();
                let result = view.private_array_write(
                    ARRAY_ROOT,
                    ARRAY_ROOT,
                    site(0, 1),
                    Role::Destination,
                    &mut budget,
                );
                if let Some(attempted) = attempted {
                    assert!(
                        matches!(result, Err(ProductionSourceOutputErrorV1::PrivateArray(
                    SemanticKirPrivateArrayQueryErrorV1::Resource(AssertOriginResourceV1::Work(error))))
                    if error.actual() == attempted && error.limit() == limit)
                    );
                } else {
                    assert!(matches!(
                        result,
                        Ok(ProductionSourceOutputPrivateArrayAccessV1::Retained {
                            index: 0,
                            executable: true,
                            ..
                        })
                    ));
                }
                assert_eq!(
                    (budget.work(), budget.storage(), budget.peak_storage()),
                    (accepted, floor, floor)
                );
            }
            let required = retained(view.source())
                + view.checked_output.storage().retained_storage()
                + view.storage.retained_storage();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(4);
            let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
            budget.reserve_storage(required - 1).unwrap();
            assert!(matches!(
                view.private_array_write(
                    ARRAY_ROOT,
                    ARRAY_ROOT,
                    site(0, 1),
                    Role::Destination,
                    &mut budget
                ),
                Err(ProductionSourceOutputErrorV1::Resource(
                    AssertOriginResourceV1::Accounting
                ))
            ));
            assert_eq!((budget.work(), budget.storage()), (4, required - 1));
        },
    );
}

#[test]
fn original_source_controls_and_required_output_rows_do_not_fall_back() {
    with_output(
        array_owner(ArrayCase::Write { sparse: false }),
        |view, budget| {
            for bad_site in [site(0, 0), site(0, 2), site(7, 1)] {
                assert!(
                    view.private_array_write(
                        ARRAY_ROOT,
                        ARRAY_ROOT,
                        bad_site,
                        Role::Destination,
                        budget
                    )
                    .is_err()
                );
            }
            assert!(
                view.private_array_write(
                    ARRAY_ROOT,
                    ARRAY_ROOT,
                    site(0, 1),
                    Role::RvalueOperand(0),
                    budget
                )
                .is_err()
            );
            // Private corruption controls do not construct source or transition authority.
            let rows = std::mem::take(&mut view.private_arrays);
            assert!(matches!(
                view.private_array_write(
                    ARRAY_ROOT,
                    ARRAY_ROOT,
                    site(0, 1),
                    Role::Destination,
                    budget
                ),
                Err(ProductionSourceOutputErrorV1::Invalid(
                    "required source-qualified array output row is absent"
                ))
            ));
            view.private_arrays = rows;
            let saved = view.private_arrays[0];
            view.private_arrays[0].placement = SourceOutputArrayPlacementV1::Unsupported;
            assert!(matches!(
                view.private_array_write(
                    ARRAY_ROOT,
                    ARRAY_ROOT,
                    site(0, 1),
                    Role::Destination,
                    budget
                ),
                Err(ProductionSourceOutputErrorV1::PrivateArray(
                    SemanticKirPrivateArrayQueryErrorV1::Incomplete(_)
                ))
            ));
            view.private_arrays[0] = saved;
            let SourceOutputArrayPlacementV1::Retained(mut anchors) = saved.placement else {
                panic!("retained")
            };
            anchors.uses[1].definition = anchors.uses[2].definition;
            view.private_arrays[0].placement = SourceOutputArrayPlacementV1::Retained(anchors);
            assert!(matches!(
                view.private_array_write(
                    ARRAY_ROOT,
                    ARRAY_ROOT,
                    site(0, 1),
                    Role::Destination,
                    budget
                ),
                Err(ProductionSourceOutputErrorV1::Invalid(
                    "array output pointer ancestry or adjacency changed"
                ))
            ));
            view.private_arrays[0] = saved;
            assert!(matches!(
                view.private_array_write(
                    ARRAY_ROOT,
                    ARRAY_ROOT,
                    site(0, 1),
                    Role::Destination,
                    budget
                ),
                Ok(ProductionSourceOutputPrivateArrayAccessV1::Retained { .. })
            ));
        },
    );
}

fn omitted_write_owner() -> ProductionPreRankedKirOwnerV1 {
    let seed = array_owner(ArrayCase::Write { sparse: false });
    let semantic = seed.semantic_ssa().source_semantic();
    let mut source_types = semantic.types().to_vec();
    let boolean = SemanticTypeIdV1::from_index(u32::try_from(source_types.len()).unwrap());
    let boolean_type = &types()[BOOL.index() as usize];
    source_types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([251; 32]),
        SemanticLayoutIdentityV1::from_sha256([251; 32]),
        boolean_type.layout().clone(),
        boolean_type.shape().clone(),
    ));
    let source = &semantic.functions()[0];
    let statements = source.blocks()[0].statements();
    let function = SemanticFunctionDeclV1::new(
        source.identity(),
        source.role(),
        source.item_definition_identity(),
        source.monomorphization_identity(),
        source.generic_type_arguments_identity(),
        source.const_generic_arguments_identity(),
        source.source(),
        source.abi().clone(),
        source.locals().to_vec(),
        source.entry(),
        vec![
            block(
                211,
                vec![],
                SemanticTerminatorKindV1::SwitchInt {
                    // Boolean switches lower to foldable conditional branches;
                    // integer switches remain preserved by the fixed optimizer.
                    discriminant: constant(boolean, 0, 1),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            edge(SemanticEdgeRoleV1::SwitchValue, 2),
                        )],
                        edge(SemanticEdgeRoleV1::SwitchOtherwise, 1),
                    )
                    .unwrap(),
                },
            ),
            block(212, statements.to_vec(), SemanticTerminatorKindV1::Return),
            block(213, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
    .unwrap()
    .with_kernel_entry(source.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        source_types,
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![ARRAY_ROOT],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let semantic =
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    let ssa =
        ProductionSemanticSsaOwnerV1::try_new(semantic, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    let launch = crate::ProductionSourceLaunchRosterV1::try_new(
        ssa.source_semantic(),
        &[crate::ProductionSourceLaunchRootInputV1::new(
            "array_relation",
            [202; 32],
            crate::ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
        )],
    )
    .unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap()
}

#[test]
fn checked_unreachable_write_is_distinct_from_proven_unretained_source() {
    with_output(omitted_write_owner(), |view, budget| {
        assert!(
            view.source()
                .has_materialized_private_array_access(
                    ARRAY_ROOT,
                    ARRAY_ROOT,
                    site(1, 1),
                    Role::Destination,
                    budget
                )
                .unwrap()
        );
        assert_eq!(
            view.private_array_write(
                ARRAY_ROOT,
                ARRAY_ROOT,
                site(1, 1),
                Role::Destination,
                budget
            )
            .unwrap(),
            ProductionSourceOutputPrivateArrayAccessV1::OmittedUnreachable
        );
        assert!(
            !view
                .output()
                .module()
                .functions
                .iter()
                .filter_map(|f| f.body.as_ref())
                .flat_map(|body| &body.blocks)
                .flat_map(|block| &block.operations)
                .any(|operation| matches!(operation.kind, OperationKind::Store { .. }))
        );
    });
    with_output(
        array_owner(ArrayCase::ValueRead { local_index: false }),
        |view, budget| {
            assert!(view.private_arrays.is_empty());
            assert_eq!(
                view.private_array_write(
                    ARRAY_ROOT,
                    ARRAY_ROOT,
                    site(0, 2),
                    Role::RvalueOperand(0),
                    budget
                )
                .unwrap(),
                ProductionSourceOutputPrivateArrayAccessV1::ProvenUnretained
            );
        },
    );
}

#[test]
fn array_header_and_rows_remain_reserved_across_real_catalog_success_and_failure() {
    // Catalog HEADER:8 magic +2+2 versions +4 length +32 source +4+4 counts.
    const EMPTY_CATALOG_CANONICAL_BYTES: usize = 56;
    with_output(
        array_owner(ArrayCase::Write { sparse: false }),
        |view, live_budget| {
            // This source has no assertions or pipelines. In addition to the
            // block/C rows, two empty catalog canonical encodings remain owned.
            let expected = std::mem::size_of::<ProductionSourceOutputOccurrencesV1<'_, '_>>()
                + std::mem::size_of::<SourceOutputBlockRowV1>()
                + std::mem::size_of::<SourceOutputArrayRowV1>()
                + 2 * EMPTY_CATALOG_CANONICAL_BYTES;
            assert_eq!(view.storage.retained_storage(), expected);
            with_control(view, live_budget, |control, transition, floor| {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
                let mut setup = AssertOriginBudgetV1::new(&mut work, STORAGE);
                setup.reserve_storage(floor).unwrap();
                let (coordinates, receipt) = check_canonical_kir_coordinate_preservation_v1(
                    view.source().executable(),
                    view.bound(),
                    &mut setup,
                )
                .unwrap();
                setup.reserve_storage(receipt.retained_storage()).unwrap();
                let floor = setup.storage();
                for limit in [97, WORK] {
                    let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
                    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
                    budget.reserve_storage(floor).unwrap();
                    let (rows, payload, header) = source_output_array_rows_with_header_v1(
                        view.source(),
                        control,
                        &mut budget,
                    )
                    .unwrap();
                    assert_eq!(header, std::mem::size_of::<Vec<SourceOutputArrayRowV1>>());
                    assert_eq!(
                        (rows.len(), payload),
                        (1, std::mem::size_of::<SourceOutputArrayRowV1>())
                    );
                    assert_eq!(
                        (budget.work(), budget.storage()),
                        (93, floor + header + payload)
                    );
                    let result = source_output_catalogs_after_replay_v1(
                        view.source(),
                        &coordinates,
                        transition,
                        &mut budget,
                    );
                    assert_eq!(budget.storage(), floor + header + payload);
                    if limit == 97 {
                        assert!(
                            matches!(result, Err(ProductionSourceOutputCatalogErrorV1::Resource(AssertOriginResourceV1::Work(error)))
                        if error.actual() == 98 && error.limit() == 97)
                        );
                        assert_eq!(budget.work(), 93);
                        assert_eq!(budget.peak_storage(), floor + header + payload);
                    } else {
                        let (catalogs, retained) = result.unwrap();
                        assert_eq!(
                            catalogs.source.canonical_bytes().len(),
                            EMPTY_CATALOG_CANONICAL_BYTES
                        );
                        assert_eq!(
                            catalogs.transported.catalog().canonical_bytes().len(),
                            EMPTY_CATALOG_CANONICAL_BYTES
                        );
                        assert_eq!(
                            retained,
                            std::mem::size_of::<SourceOutputCatalogsV1>()
                                + 2 * EMPTY_CATALOG_CANONICAL_BYTES
                        );
                        budget.reserve_storage(retained).unwrap();
                        assert_eq!(budget.storage(), floor + header + payload + retained);
                        drop(catalogs);
                        budget.release_storage(retained).unwrap();
                    }
                    drop(rows);
                    budget.release_storage(payload + header).unwrap();
                    assert_eq!(budget.storage(), floor);
                }
                #[allow(
                    clippy::drop_non_drop,
                    reason = "End the borrowed witness before releasing its ledger reservation"
                )]
                drop(coordinates);
                setup.release_storage(receipt.retained_storage()).unwrap();
            });
        },
    );
}

#[test]
fn initializer_components_survive_the_real_seven_pass_output_constructor() {
    for (values, float) in [
        ([0, 1, 2, 3, 7, 31, 255, u32::MAX], false),
        ([11; 8], false),
        (
            [
                0,
                0x8000_0000,
                0x3f80_0000,
                0x7f80_0000,
                0xff80_0000,
                0x7fc0_0001,
                0x7fc0_0002,
                1,
            ],
            true,
        ),
    ] {
        for repetitions in [1, 2] {
            with_output(
                array_owner(ArrayCase::Initializer {
                    values,
                    repetitions,
                    float,
                }),
                |view, budget| {
                    assert_eq!(view.private_arrays.len(), 8 * repetitions + 1);
                    for statement in 1..=repetitions {
                        assert_eq!(
                            view.source()
                                .materialized_private_array_initializer_count(
                                    ARRAY_ROOT,
                                    ARRAY_ROOT,
                                    site(0, statement as u32),
                                    budget,
                                )
                                .unwrap(),
                            Some(8),
                        );
                        assert!(matches!(
                            view.private_array_write(
                                ARRAY_ROOT,
                                ARRAY_ROOT,
                                site(0, statement as u32),
                                Role::Destination,
                                budget,
                            ),
                            Err(ProductionSourceOutputErrorV1::PrivateArray(
                                SemanticKirPrivateArrayQueryErrorV1::Incomplete(
                                    "private array requires one exact index projection"
                                )
                            ))
                        ));
                        let rows: Vec<_> = view
                            .private_arrays
                            .iter()
                            .filter(|row| row.key[3] == statement as u32)
                            .collect();
                        assert_eq!(rows.len(), 8);
                        for (component, row) in rows.iter().enumerate() {
                            assert_eq!(
                                row.key,
                                [0, 0, 0, statement as u32, 2, 0, component as u32]
                            );
                            let original = &view.source().correspondence.private_arrays;
                            let effect = &original.effects[row.original_effect];
                            let slot = &original.slots[row.original_slot];
                            assert_eq!(effect.original_index.component(), component as u32);
                            assert_eq!(effect.semantic_statement, statement as u32);
                            assert_eq!(
                                (slot.owner, slot.function, slot.local),
                                (effect.owner, effect.function, effect.local)
                            );
                            let SourceOutputArrayPlacementV1::Retained(anchors) = row.placement
                            else {
                                panic!("required initializer Store placement");
                            };
                            assert!(anchors.executable);
                            let memory =
                                source_output_operation_v1(view.output(), anchors.memory, budget)
                                    .unwrap();
                            assert!(matches!(memory.kind, OperationKind::Store { .. }));
                        }
                    }
                    assert!(matches!(
                        view.private_array_write(
                            ARRAY_ROOT,
                            ARRAY_ROOT,
                            site(0, repetitions as u32 + 1),
                            Role::Destination,
                            budget,
                        ),
                        Ok(ProductionSourceOutputPrivateArrayAccessV1::Retained {
                            index: 0,
                            executable: true,
                            ..
                        })
                    ));
                },
            );
        }
    }
}

#[test]
fn complete_component_keys_reject_duplicates_and_ordinary_row_substitution() {
    with_output(
        array_owner(ArrayCase::Initializer {
            values: [11; 8],
            repetitions: 1,
            float: false,
        }),
        |view, budget| {
            // Private key-check components over real rows, not forged source owners.
            let mut distinct = [view.private_arrays[1], view.private_arrays[0]];
            assert_eq!(&distinct[0].key[..6], &distinct[1].key[..6]);
            assert_ne!(distinct[0].key[6], distinct[1].key[6]);
            source_output_array_check_keys_v1(&mut distinct, budget).unwrap();
            assert!(distinct[0].key < distinct[1].key);
            let mut duplicate = distinct;
            duplicate[1].key = duplicate[0].key;
            assert!(matches!(
                source_output_array_check_keys_v1(&mut duplicate, budget),
                Err(ProductionSourceOutputErrorV1::Invalid(
                    "duplicate array source occurrence"
                ))
            ));

            let ordinary = view
                .private_arrays
                .iter()
                .position(|row| row.key[3] == 2)
                .unwrap();
            let saved = view.private_arrays[ordinary];
            for mutation in 0..2 {
                let expected = if mutation == 0 {
                    view.private_arrays[ordinary].key[6] = 1;
                    "required source-qualified array output row is absent"
                } else {
                    view.private_arrays[ordinary].original_effect =
                        view.private_arrays[0].original_effect;
                    "array original occurrence identity changed"
                };
                assert!(matches!(
                    view.private_array_write(ARRAY_ROOT, ARRAY_ROOT, site(0, 2), Role::Destination, budget),
                    Err(ProductionSourceOutputErrorV1::Invalid(actual)) if actual == expected
                ));
                view.private_arrays[ordinary] = saved;
                assert!(matches!(
                    view.private_array_write(
                        ARRAY_ROOT,
                        ARRAY_ROOT,
                        site(0, 2),
                        Role::Destination,
                        budget
                    ),
                    Ok(ProductionSourceOutputPrivateArrayAccessV1::Retained { index: 0, .. })
                ));
            }
        },
    );
}

#[test]
fn component_qualified_initialization_does_not_turn_a_read_into_a_write_proof() {
    let owner = array_owner(ArrayCase::RetainedValueRead);
    let semantic = owner.semantic_ssa().source_semantic();
    assert_eq!(semantic.functions().len(), 1);
    assert_eq!(
        semantic.functions()[0].role(),
        SemanticFunctionRoleV1::KernelRoot
    );
    assert_eq!(semantic.functions()[0].blocks().len(), 1);
    assert_eq!(semantic.functions()[0].blocks()[0].statements().len(), 4);
    assert!(
        !owner
            .semantic_ssa()
            .plan_for_function(ARRAY_ROOT)
            .unwrap()
            .plan()
            .promoted_variables()
            .iter()
            .any(|variable| variable.get() == 1)
    );
    let rows = &owner.correspondence.private_arrays;
    assert!(rows.active);
    assert_eq!(
        (rows.instances.len(), rows.slots.len(), rows.effects.len()),
        (1, 1, 10)
    );
    assert_eq!(owner.executable().module().functions.len(), 1);
    let function = &owner.executable().module().functions[0];
    assert_eq!(function.role, fe2o3_kernel_ir::FunctionRole::KernelEntry);
    let body = function.body.as_ref().unwrap();
    let operations: Vec<_> = body
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .collect();
    assert_eq!(
        operations
            .iter()
            .filter(|op| matches!(op.kind, OperationKind::Store { .. }))
            .count(),
        9
    );
    assert_eq!(
        operations
            .iter()
            .filter(|op| matches!(op.kind, OperationKind::Load { .. }))
            .count(),
        1
    );
    for (ordinal, effect) in rows.effects.iter().enumerate() {
        let (statement, role, access) = match ordinal {
            0..8 => (1, Role::Destination, PrivateArrayAccessV1::Write),
            8 => (2, Role::Destination, PrivateArrayAccessV1::Write),
            9 => (3, Role::RvalueOperand(0), PrivateArrayAccessV1::Read),
            _ => unreachable!(),
        };
        assert_eq!(
            (effect.owner, effect.function, effect.local),
            (ARRAY_ROOT, ARRAY_ROOT, 1)
        );
        assert_eq!(
            (
                effect.semantic_block,
                effect.semantic_statement,
                effect.role,
                effect.access
            ),
            (0, statement, role, access)
        );
        if ordinal < 8 {
            assert!(matches!(effect.original_index,
                PrivateArrayIndexV1::InitializerElement { component, .. }
                if component == ordinal as u32));
        } else {
            assert!(matches!(
                effect.original_index,
                PrivateArrayIndexV1::Local { local: 2, .. }
            ));
        }
        let block = &body.blocks[effect.memory_location.block_ordinal];
        assert_eq!(block.id, effect.memory_location.block);
        let memory = &block.operations[effect.memory_location.operation];
        assert!(match access {
            PrivateArrayAccessV1::Write => matches!(memory.kind, OperationKind::Store { .. }),
            PrivateArrayAccessV1::Read => matches!(memory.kind, OperationKind::Load { .. }),
        });
    }
    with_output(owner, |view, budget| {
        assert_eq!(view.private_arrays.len(), 10);
        assert_eq!(
            view.private_arrays
                .iter()
                .filter(|row| matches!(row.placement, SourceOutputArrayPlacementV1::Unsupported))
                .count(),
            1
        );
        let read = view
            .private_arrays
            .iter()
            .find(|row| row.key[3] == 3)
            .unwrap();
        assert_eq!(read.original_effect, 9);
        assert!(matches!(
            read.placement,
            SourceOutputArrayPlacementV1::Unsupported
        ));
        assert!(matches!(
            view.private_array_write(
                ARRAY_ROOT,
                ARRAY_ROOT,
                site(0, 2),
                Role::Destination,
                budget
            ),
            Ok(ProductionSourceOutputPrivateArrayAccessV1::Retained { index: 0, .. })
        ));
        assert_eq!(
            view.source()
                .materialized_private_array_initializer_count(
                    ARRAY_ROOT,
                    ARRAY_ROOT,
                    site(0, 1),
                    budget,
                )
                .unwrap(),
            Some(8)
        );
        assert_eq!(
            view.source()
                .materialized_private_array_constant_index(
                    ARRAY_ROOT,
                    ARRAY_ROOT,
                    site(0, 3),
                    Role::RvalueOperand(0),
                    budget,
                )
                .unwrap(),
            Some(0)
        );
        assert!(matches!(
            view.private_array_write(
                ARRAY_ROOT,
                ARRAY_ROOT,
                site(0, 3),
                Role::RvalueOperand(0),
                budget
            ),
            Err(ProductionSourceOutputErrorV1::PrivateArray(
                SemanticKirPrivateArrayQueryErrorV1::Incomplete(
                    "checked output currently requires an ordinary private-array write"
                )
            ))
        ));
    });
}

#[test]
fn component_binding_and_seventh_comparison_have_literal_work_boundaries() {
    for limit in [1, 2] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = AssertOriginBudgetV1::new(&mut work, 0);
        let result = source_output_array_key_v1(
            ARRAY_ROOT,
            ARRAY_ROOT,
            0,
            1,
            Role::Destination,
            u32::MAX,
            &mut budget,
        );
        if limit == 2 {
            assert_eq!(result.unwrap(), [0, 0, 0, 1, 2, 0, u32::MAX]);
            assert_eq!(budget.work(), 2);
        } else {
            assert!(
                matches!(result, Err(ProductionSourceOutputErrorV1::Resource(
                AssertOriginResourceV1::Work(error))) if error.actual() == 2 && error.limit() == 1)
            );
            assert_eq!(budget.work(), 0);
        }
        assert_eq!(budget.storage(), 0);
    }
    with_output(
        array_owner(ArrayCase::Write { sparse: false }),
        |view, live_budget| {
            // Isolated key-search ledgers over one genuine row, not production resets.
            // One loop + midpoint/lookup3 + all seven equal fields = 11.
            let floor = live_budget.storage();
            let rows = [view.private_arrays[0]];
            for limit in [10, 11] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
                let mut budget = AssertOriginBudgetV1::new(&mut work, floor);
                budget.reserve_storage(floor).unwrap();
                let result = private_array_binary_search_v1(
                    &rows,
                    |row| row.key.map(|value| value as usize),
                    rows[0].key.map(|value| value as usize),
                    &mut PrivateArrayQueryWorkV1 {
                        budget: &mut budget,
                    },
                );
                if limit == 11 {
                    assert_eq!(result.unwrap(), Ok(0));
                } else {
                    assert!(
                        matches!(result, Err(SemanticKirPrivateArrayQueryErrorV1::Resource(
                    AssertOriginResourceV1::Work(error))) if error.actual() == 11 && error.limit() == 10)
                    );
                }
                assert_eq!(
                    (budget.work(), budget.storage(), budget.peak_storage()),
                    (limit, floor, floor)
                );
            }
        },
    );
}

include!("production_private_array_ranked_output_v1_tests.rs");
