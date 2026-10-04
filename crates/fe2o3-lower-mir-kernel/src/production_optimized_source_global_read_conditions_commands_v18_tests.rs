// White-box commands remain beside the private proof implementation. Genuine
// source factories live in consumer_tests; no proof type crosses that boundary.
#[test]
fn pending_global_read_conditions_fixed_frames_have_independent_exact_and_one_short_limits() {
    use fe2o3_kernel_analysis::{
        CanonicalKirBlockRefV1, CanonicalKirEdgeRefV1, CanonicalKirInventoryV18,
        CanonicalKirOperationRefV1,
    };
    type Frame<'a> = (
        &'a PendingGlobalSourceAccessesV18<'a>,
        &'a fe2o3_pliron::PendingCanonicalGlobalAccessesV18<'a, 'a>,
        &'a GlobalReadFactsV18<'a, 'a>,
        SliceOperation,
        &'a mut ArgumentBudgetV1<'a>,
    );
    type Capture<'a> = (
        &'a PendingGlobalSourceAccessesV18<'a>,
        &'a fe2o3_pliron::PendingCanonicalGlobalAccessesV18<'a, 'a>,
        &'a GlobalReadFactsV18<'a, 'a>,
        &'a CanonicalKirInventoryV18<'a>,
        &'a SliceOperation,
        &'a mut ArgumentBudgetV1<'a>,
        &'a mut (),
    );
    type DomainFrame<'a> = (
        &'a PendingGlobalSourceAccessesV18<'a>,
        &'a CanonicalKirInventoryV18<'a>,
        &'a GlobalReadFactsV18<'a, 'a>,
        &'a GlobalSourceAccessPairV18,
        &'a GlobalReadFactV18<'a, 'a>,
        &'a mut ArgumentBudgetV1<'a>,
    );
    type Join<'a> = (
        &'a CanonicalKirInventoryV18<'a>,
        &'a GlobalSourceAccessPairV18,
        &'a GlobalSourceAccessEndpointV18,
        &'a GlobalReadFactV18<'a, 'a>,
        &'a CanonicalKirDefinitionRefV1<'a>,
        &'a CanonicalKirDefinitionRefV1<'a>,
        &'a CanonicalKirOperationRefV1<'a>,
        &'a CanonicalKirBlockRefV1<'a>,
        &'a CanonicalKirEdgeRefV1<'a>,
        Option<&'a CanonicalKirEdgeRefV1<'a>>,
        &'a fe2o3_kernel_ir::Operation,
        &'a [fe2o3_kernel_ir::ValueDef],
        &'a fe2o3_kernel_ir::ValueDef,
        &'a Type,
        &'a fe2o3_kernel_ir::SliceType,
        &'a [CanonicalKirEdgeRefV1<'a>],
        &'a std::ops::Range<usize>,
        fe2o3_kernel_ir::FormalRuntimeSliceReadDomainV1,
        fe2o3_kernel_ir::FormalRuntimeSliceReadDomainV1,
        fe2o3_kernel_ir::FormalAllocationIdentity,
        Type,
        fe2o3_kernel_ir::FormalGuardedPathV1,
        (fe2o3_kernel_ir::BlockId, usize, fe2o3_kernel_ir::BlockId),
        [ValueId; 4],
        [usize; 3],
        [u32; 2],
        Option<u16>,
        u16,
        u64,
        bool,
        (
            &'a CanonicalKirInventoryV18<'a>,
            SliceDefinition,
            &'a mut ArgumentBudgetV1<'a>,
        ),
        SliceDefinition,
        fe2o3_kernel_ir::CanonicalKirEdgeCoordinateV1,
        Result<&'a CanonicalKirDefinitionRefV1<'a>, ProductionSourceOwnedViewErrorV18>,
        Result<&'a CanonicalKirDefinitionRefV1<'a>, ProductionSourceOwnedViewErrorV18>,
        Result<&'a CanonicalKirOperationRefV1<'a>, ProductionSourceOwnedViewErrorV18>,
        Result<&'a CanonicalKirBlockRefV1<'a>, ProductionSourceOwnedViewErrorV18>,
        Option<usize>,
        &'a usize,
        &'a std::ops::Range<usize>,
    );
    type NormalizedJoin<'a> = (
        [&'a CanonicalKirDefinitionRefV1<'a>; 2],
        Result<&'a CanonicalKirDefinitionRefV1<'a>, ProductionSourceOwnedViewErrorV18>,
        Result<&'a CanonicalKirDefinitionRefV1<'a>, ProductionSourceOwnedViewErrorV18>,
        SliceOperation,
        &'a CanonicalKirOperationRefV1<'a>,
        Result<&'a CanonicalKirOperationRefV1<'a>, ProductionSourceOwnedViewErrorV18>,
        &'a fe2o3_kernel_ir::Operation,
        &'a fe2o3_kernel_ir::OperationKind,
        &'a [fe2o3_kernel_ir::ValueDef],
        &'a fe2o3_kernel_ir::ValueDef,
        [&'a ValueId; 2],
        [&'a Type; 2],
        fe2o3_kernel_ir::CanonicalGuardedReadIndexOriginV1,
        (ValueId, ValueId),
        (ValueId, ValueId),
        [ValueId; 4],
        [Option<ValueId>; 2],
        [bool; 2],
        [Type; 3],
        SliceDefinition,
        fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1,
        [fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1; 3],
    );
    type ControlJoin<'a> = (
        &'a CanonicalKirDefinitionRefV1<'a>,
        SliceOperation,
        SliceDefinition,
        fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
        &'a CanonicalKirOperationRefV1<'a>,
        &'a fe2o3_kernel_ir::Operation,
        &'a fe2o3_kernel_ir::OperationKind,
        &'a [fe2o3_kernel_ir::ValueDef],
        &'a fe2o3_kernel_ir::ValueDef,
        &'a Option<Terminator>,
        &'a Terminator,
        Option<&'a Terminator>,
        [&'a ValueId; 3],
        [&'a Type; 3],
        [Type; 2],
        [Option<ValueId>; 2],
        bool,
        Option<&'a CanonicalKirDefinitionRefV1<'a>>,
        Result<
            Option<&'a CanonicalKirDefinitionRefV1<'a>>,
            fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1,
        >,
        SourceOwnedResultV18<Option<&'a CanonicalKirDefinitionRefV1<'a>>>,
        SourceOwnedResultV18<&'a CanonicalKirDefinitionRefV1<'a>>,
        SourceOwnedResultV18<&'a CanonicalKirOperationRefV1<'a>>,
        Result<(), ArgumentResourceV1>,
        (
            &'a CanonicalKirInventoryV18<'a>,
            SliceOperation,
            &'a mut ArgumentBudgetV1<'a>,
        ),
        (
            &'a CanonicalKirInventoryV18<'a>,
            fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
            ValueId,
            &'a mut ArgumentBudgetV1<'a>,
        ),
    );
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T, PendingGlobalReadConditionErrorV18>>()
    }
    let capture = 17;
    let alignment = 8;
    let expected = [
        capture,
        alignment,
        global_native_pair_headers_v18().unwrap(),
        source_domain_join_header_oracle_v30(),
        h::<Frame<'_>>(),
        h::<Capture<'_>>(),
        h::<Capture<'_>>(),
        h::<DomainFrame<'_>>(),
        h::<DomainFrame<'_>>(),
        h::<Join<'_>>(),
        h::<NormalizedJoin<'_>>(),
        h::<ControlJoin<'_>>(),
        h::<PendingGlobalReadConditionsV18<'_, '_>>(),
        h::<&PendingGlobalReadConditionsV18<'_, '_>>(),
        h::<Option<&PendingGlobalReadConditionsV18<'_, '_>>>(),
        h::<GlobalReadFactV18<'_, '_>>(),
        h::<fe2o3_kernel_ir::CanonicalGuardedGlobalReadOutcomeV18<'_, '_>>(),
        h::<fe2o3_kernel_ir::CanonicalGuardedPredicateFactV18<'_, '_>>(),
        h::<Option<fe2o3_kernel_ir::CanonicalGuardedPredicateFactV18<'_, '_>>>(),
        h::<Option<&fe2o3_kernel_ir::CanonicalGuardedPredicateFactV18<'_, '_>>>(),
        h::<(
            &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
            &GlobalReadFactV18<'_, '_>,
            &fe2o3_kernel_ir::FormalRuntimeSliceReadDomainV1,
            &fe2o3_kernel_ir::CanonicalGuardedPredicateFactV18<'_, '_>,
        )>(),
        h::<fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1>(),
        h::<fe2o3_kernel_ir::CanonicalGuardedGlobalReadReasonV1>(),
        h::<&fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18>(),
        h::<&GlobalSourceAccessPairV18>(),
        h::<Option<&GlobalSourceAccessPairV18>>(),
        h::<ProductionSourceOwnedViewErrorV18>(),
        h::<PendingGlobalReadConditionErrorV18>(),
        h::<Option<SourceOwnedQueryFailureV18>>(),
        h::<SourceOwnedQueryFailureV18>(),
        h::<(
            &ProductionSourceOwnedViewErrorV18,
            SourceOwnedQueryFailureV18,
        )>(),
        h::<(&&'static str, &'static str)>(),
        h::<ArgumentResourceV1>(),
        h::<bool>(),
        h::<&Option<fe2o3_kernel_ir::CanonicalGuardedPredicateFactV18<'_, '_>>>(),
        h::<Result<(), fe2o3_pliron::CanonicalRankedPolicyFailureV1>>(),
        h::<Result<(), PendingGlobalNativeErrorV18>>(),
        h::<Option<ArgumentResourceV1>>(),
        h::<&ArgumentResourceV1>(),
        h::<Option<&GlobalSourceAccessPairV18>>(),
        h::<&&GlobalSourceAccessPairV18>(),
        h::<&Result<(), PendingGlobalReadConditionErrorV18>>(),
        h::<SourceOwnedResultV18<()>>(),
        h::<Result<(), ArgumentResourceV1>>(),
        h::<
            Result<
                &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18,
                fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1,
            >,
        >(),
        h::<
            Result<
                fe2o3_kernel_ir::CanonicalGuardedGlobalReadOutcomeV18<'_, '_>,
                fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1,
            >,
        >(),
        h::<
            Result<
                Option<fe2o3_kernel_ir::CanonicalGuardedPredicateFactV18<'_, '_>>,
                fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1,
            >,
        >(),
        h::<Result<(), PendingGlobalReadConditionErrorV18>>(),
        h::<std::thread::Result<Result<(), PendingGlobalReadConditionErrorV18>>>(),
        h::<std::panic::AssertUnwindSafe<Capture<'_>>>(),
        h::<(
            fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
            [usize; 3],
        )>(),
        h::<(
            &PendingGlobalReadConditionsV18<'_, '_>,
            &mut ArgumentBudgetV1<'_>,
        )>(),
        h::<(
            Option<&PendingGlobalReadConditionsV18<'_, '_>>,
            &mut ArgumentBudgetV1<'_>,
        )>(),
        h::<(
            &PendingGlobalSourceAccessesV18<'_>,
            &fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1,
        )>(),
        h::<(
            &PendingGlobalSourceAccessesV18<'_>,
            SourceOwnedQueryFailureV18,
        )>(),
        h::<(&ScopedSourceCleanupV29, &ArgumentResourceV1)>(),
    ]
    .into_iter()
    .sum::<usize>();
    assert_eq!(
        global_read_condition_headers_v18(capture, alignment).unwrap(),
        expected
    );
    for short in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1);
        let mut budget = ArgumentBudgetV1::new(&mut work, expected - usize::from(short));
        let result =
            budget.reserve_storage(global_read_condition_headers_v18(capture, alignment).unwrap());
        if short {
            assert!(matches!(result, Err(ArgumentResourceV1::Storage(error))
                if error.actual() == expected && error.limit() == expected - 1));
            assert_eq!(budget.storage(), 0);
        } else {
            result.unwrap();
            assert_eq!(budget.storage(), expected);
            budget.release_storage(expected).unwrap();
        }
    }
    assert_eq!(
        global_read_condition_headers_v18(usize::MAX, 1),
        Err(ArgumentResourceV1::Arithmetic)
    );
}

#[derive(Clone, Copy, Debug)]
pub(super) enum GlobalReadConditionTestV18 {
    Positive,
    IssuedGuardShape,
    ControlTransport(u8),
    NormalizedPositive,
    NormalizedDomain(u8),
    OriginalLengthOccurrence,
    ChangedDomain(u8),
    ForeignFacts,
    ForeignQuery(usize),
    HeaderCut,
    WorkCut,
    SelectedError,
    Unwind,
    ErrorThenDropPanic,
    QueryThenDropPanic,
    SwallowedQueryThenError,
    HigherFloor(u8),
    ForeignLedger { extra: usize, disposition: u8 },
}

fn local_read_test_error_v18(
    error: PendingGlobalReadConditionErrorV18,
) -> ProductionSourceOwnedViewErrorV18 {
    match error {
        PendingGlobalReadConditionErrorV18::Source(error)
        | PendingGlobalReadConditionErrorV18::Formal {
            source_refusal: error,
            ..
        }
        | PendingGlobalReadConditionErrorV18::Unproved {
            source_refusal: error,
            ..
        } => error,
        PendingGlobalReadConditionErrorV18::Native(
            fe2o3_pliron::CanonicalRankedPolicyFailureV1::Resource(error),
        ) => error.into(),
        other => panic!("unexpected native local-condition refusal: {other:?}"),
    }
}

// Same authentic metadata/candidate/import route as the Global13 test helper,
// but constructor and teardown resource refusals remain typed for exact cuts.
fn with_local_read_native_test_view_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
    run: impl FnOnce(
        &fe2o3_pliron::PendingCanonicalGlobalAccessesV18<'_, '_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()>,
) -> SourceOwnedResultV18<()> {
    use fe2o3_kernel_analysis::{
        CanonicalRankedMetadataV18, CanonicalRankedViewErrorV1,
        build_canonical_ranked_candidate_v18, with_checked_canonical_ranked_view_v18,
    };
    let view_error = |error| match error {
        CanonicalRankedViewErrorV1::Resource(error) => {
            ProductionSourceOwnedViewErrorV18::Resource(error)
        }
        other => panic!("actual canonical candidate fixture: {other:?}"),
    };
    let result =
        source_scalar_normalization_scratch_v18(original.source.cleanup, budget, 0, |budget| {
            type CallbackResult = Option<SourceOwnedResultV18<()>>;
            budget.reserve_storage(
                size_of::<CallbackResult>()
                    + size_of::<&mut CallbackResult>()
                    + size_of::<SourceOwnedResultV18<()>>()
                    + size_of::<Result<(), fe2o3_pliron::CanonicalRankedPolicyFailureV1>>(),
            )?;
            let mut callback_result = None;
            let output = optimized.output_inventory(budget)?;
            let metadata = CanonicalRankedMetadataV18::new(output.owner(), &[]);
            let metadata_storage = metadata.storage_extent(budget).map_err(view_error)?;
            budget.reserve_storage(metadata_storage)?;
            let (candidate, receipt) =
                build_canonical_ranked_candidate_v18(output, &metadata, budget)
                    .map_err(view_error)?;
            budget.reserve_storage(receipt.retained_storage())?;
            let layouts = fe2o3_kernel_ir::StorageLayoutLimitsV1 {
                rows: 4096,
                edges: 16384,
                containment_depth: 128,
                object_bytes: 1 << 24,
            };
            let result = with_checked_canonical_ranked_view_v18(
                output,
                &metadata,
                &candidate,
                budget,
                |checked, budget| {
                    Ok::<_, CanonicalRankedViewErrorV1>(
                        fe2o3_pliron::with_pending_canonical_ranked_source_roles_v18(
                            checked,
                            layouts,
                            budget,
                            |pending, budget| {
                                pending.with_pending_global_accesses_v18(
                                    output.owner(),
                                    budget,
                                    |native, budget| {
                                        callback_result =
                                            Some(original.retain_query(run(native, budget)));
                                        Ok(())
                                    },
                                )
                            },
                        ),
                    )
                },
            );
            // The native callback is unit-only. Preserve its earlier typed
            // source refusal before interpreting any later native teardown.
            if let Some(Err(error)) = callback_result {
                return Err(error);
            }
            result.map_err(view_error)?.map_err(|error| match error {
                fe2o3_pliron::CanonicalRankedPolicyFailureV1::Resource(error) => {
                    ProductionSourceOwnedViewErrorV18::Resource(error)
                }
                other => panic!("actual native fixture: {other:?}"),
            })?;
            assert!(
                callback_result.is_some(),
                "successful native scope must invoke its callback"
            );
            drop(candidate);
            drop(metadata);
            budget.release_storage(metadata_storage + receipt.retained_storage())?;
            Ok(())
        });
    original.retain_query(result)
}

pub(super) fn test_pending_global_read_conditions_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
    work_limit: usize,
    mode: GlobalReadConditionTestV18,
    observed: &std::cell::Cell<[usize; 3]>,
) -> SourceOwnedResultV18<()> {
    original.with_pending_global_accesses_v18(optimized, 0, budget, |source, budget| {
        let formal = original.with_optimized_guarded_reads_v18(
            optimized,
            fe2o3_kernel_ir::CanonicalGuardedGlobalReadLimitsV1::default(),
            budget,
            |facts, budget| {
                with_local_read_native_test_view_v18(original, optimized, budget, |native, budget| {
                    let inventory = optimized.output_inventory(budget).unwrap();
                    let pair = source.roles.rows.iter()
                        .find_map(|row| row.global.as_ref().filter(|pair| !pair.output.writing))
                        .expect("genuine source read, not an empty operation census");
                    let operation = pair.output.logical.access.operation;
                    let floor = budget.storage();
                    match mode {
                        GlobalReadConditionTestV18::IssuedGuardShape => {
                            source.with_native_access_v18(native, operation, budget, |view, _| {
                                let view = view.expect("issued read must first join the actual native access");
                                assert!(std::ptr::eq(view.pair, pair));
                                assert!(!view.grants_memory_or_launch_authority());
                                assert!(!view.native.memory_safety_is_complete());
                                Ok(())
                            }).map_err(PendingGlobalReadConditionErrorV18::from)
                                .map_err(local_read_test_error_v18)?;
                            assert_eq!(budget.storage(), floor);
                            let guard = source_block_row_v18(inventory, pair.output.logical.guard_edge.source, budget)?;
                            let Some(Terminator::Switch { selector, cases, default_target, .. }) = guard.block.terminator.as_ref()
                                else { panic!("actual issued guard must retain the expected Switch shape"); };
                            let [case] = cases.as_slice() else { panic!("exact one-case issued discriminant switch"); };
                            assert_eq!(case.value, 1);
                            assert_ne!(case.target, *default_target);
                            assert_eq!(pair.output.logical.guard_edge.successor, 0);
                            let edge = &inventory.edges()[guard.edges.start];
                            assert_eq!(edge.coordinate, pair.output.logical.guard_edge);
                            assert_eq!(edge.target_id, case.target);
                            let selector_definition = optimized_source_definition_row_v18(inventory, pair.output.logical.guard_condition, budget)?;
                            assert_eq!(selector_definition.value, Some(*selector));
                            assert_eq!(selector_definition.ty, &Type::Scalar(ScalarType::U32));
                            let SliceDefinition::Result { operation: cast, result: 0 } = selector_definition.coordinate
                                else { panic!("actual discriminant must be a result"); };
                            let cast = source_operation_row_v18(inventory, cast, budget)?.operation;
                            let OperationKind::Cast { kind: fe2o3_kernel_ir::CastKind::ZeroExtend, value: predicate, to } = &cast.kind
                                else { panic!("actual discriminant must extend Bool"); };
                            assert_eq!(to, &Type::Scalar(ScalarType::U32));
                            assert!(matches!(cast.results.as_slice(), [result] if result.id == *selector && &result.ty == to));
                            let predicate_definition = inventory.definition_for_value(operation.block.function, *predicate, budget)
                                .map_err(source_pointer_inventory_error_v18)?.expect("actual Bool producer");
                            assert_eq!(predicate_definition.ty, &Type::BOOL);
                            let SliceDefinition::Result { operation: comparison, result: 0 } = predicate_definition.coordinate
                                else { panic!("actual issued predicate must be a result"); };
                            let comparison = source_operation_row_v18(inventory, comparison, budget)?.operation;
                            let OperationKind::Compare { predicate: fe2o3_kernel_ir::ComparePredicate::LessThan, lhs, rhs } = &comparison.kind
                                else { panic!("actual issued predicate must be LessThan"); };
                            assert!(matches!(comparison.results.as_slice(), [result] if result.id == *predicate && result.ty == Type::BOOL));
                            let index = optimized_source_definition_row_v18(inventory, pair.output.logical.index, budget)?;
                            let address_index = optimized_source_definition_row_v18(inventory, pair.output.address_index, budget)?;
                            let length = source_operation_row_v18(inventory, pair.output.logical.length, budget)?.operation;
                            let [length] = length.results.as_slice() else { panic!("actual source length result"); };
                            assert_eq!(index.ty, &Type::INDEX);
                            assert_eq!(address_index.ty, &Type::INDEX);
                            assert_eq!(length.ty, Type::INDEX);
                            assert_eq!(index.value, Some(*lhs));
                            assert_eq!(address_index.value, Some(*lhs));
                            assert_eq!(length.id, *rhs);
                            assert_ne!(*predicate, *selector, "Bool truth and integer transport are distinct values");
                            assert_eq!(budget.storage(), floor);
                            observed.set([1, 1, 1]);
                        }
                        GlobalReadConditionTestV18::ControlTransport(fault) => {
                            let other = source.roles.rows.iter().find_map(|row| row.global.as_ref().filter(|other|
                                other.output.writing && other.output.logical.guard_condition != pair.output.logical.guard_condition))
                                .expect("second actual guarded descriptor with a distinct selector");
                            source.with_native_access_v18(native, other.output.logical.access.operation, budget, |view, _| {
                                let view = view.expect("genuine cross-descriptor Store native pair");
                                assert!(std::ptr::eq(view.pair, other));
                                assert!(!view.grants_memory_or_launch_authority());
                                Ok(())
                            }).unwrap();
                            source.with_local_read_conditions_v18(native, facts, operation, budget, |view, _| {
                                let view = view.expect("actual one-case issued read must complete its local join");
                                assert!(std::ptr::eq(view.pair, pair));
                                assert!(!view.grants_memory_or_launch_authority());
                                Ok(())
                            }).unwrap();
                            assert_eq!(budget.storage(), floor);
                            let fe2o3_kernel_ir::CanonicalGuardedGlobalReadOutcomeV18::ProvedLocalConditions(fact)
                                = facts.read_at(operation, budget).unwrap() else { panic!("genuine formal issued read"); };
                            let guard = source_block_row_v18(inventory, pair.output.logical.guard_edge.source, budget).unwrap();
                            let Some(Terminator::Switch { cases, .. }) = guard.block.terminator.as_ref()
                                else { panic!("actual issued Switch"); };
                            let [case] = cases.as_slice() else { panic!("one actual case"); };
                            assert_eq!(case.value, 1);
                            assert_eq!(fact.domain().path(), fe2o3_kernel_ir::FormalGuardedPathV1::TrueEdge {
                                source: guard.block.id, ordinal: 0, target: case.target,
                            });
                            let first = optimized_source_definition_row_v18(inventory, pair.output.logical.guard_condition, budget).unwrap();
                            let second = optimized_source_definition_row_v18(inventory, other.output.logical.guard_condition, budget).unwrap();
                            assert_eq!(first.ty, &Type::Scalar(ScalarType::U32));
                            assert_eq!(first.ty, second.ty);
                            assert_ne!(first.value, second.value);
                            let predicate = inventory.definition_for_value(operation.block.function, fact.domain().predicate(), budget)
                                .unwrap().expect("actual Bool comparison definition");
                            assert_eq!(predicate.ty, &Type::BOOL);
                            let SliceDefinition::Result { operation: second_cast, result: 0 } = second.coordinate
                                else { panic!("second actual discriminant result"); };
                            let second_cast = source_operation_row_v18(inventory, second_cast, budget).unwrap().operation;
                            let OperationKind::Cast { kind: fe2o3_kernel_ir::CastKind::ZeroExtend, value: second_predicate, to } = &second_cast.kind
                                else { panic!("second actual Boolean extension"); };
                            assert_eq!(to, &Type::Scalar(ScalarType::U32));
                            assert_ne!(*second_predicate, fact.domain().predicate());
                            let mut changed = *pair;
                            match fault {
                                0 => changed.output.logical.guard_condition = second.coordinate,
                                1 => changed.output.logical.guard_condition = predicate.coordinate,
                                2 => changed.output.logical.guard_edge = other.output.logical.guard_edge,
                                3 => changed.output.logical.guard_edge.successor = 1,
                                4 => {
                                    changed.output.logical.guard_condition = second.coordinate;
                                    changed.output.logical.guard_edge = other.output.logical.guard_edge;
                                }
                                _ => unreachable!(),
                            }
                            let headers = global_read_condition_headers_v18(0, 1).unwrap();
                            budget.reserve_storage(headers).unwrap();
                            // Copied source coordinates carry no authority. Both actual selector
                            // producers and the positive source/native/formal join precede this fault.
                            let result = source.check_local_read_domain_v18(inventory, facts, &changed, &fact, budget);
                            let expected = if fault == 3 { "pending global read changed exact local domain" }
                                else { "pending global read changed exact control transport" };
                            assert!(matches!(result, Err(PendingGlobalReadConditionErrorV18::Source(
                                ProductionSourceOwnedViewErrorV18::Binding(detail))) if detail == expected));
                            drop(fact);
                            budget.release_storage(headers).unwrap();
                            assert_eq!(budget.storage(), floor);
                            observed.set([1, 1, 1]);
                        }
                        GlobalReadConditionTestV18::Positive => {
                            let mut previous = None;
                            for _ in 0..4 {
                                let before = budget.work();
                                source.with_local_read_conditions_v18(native, facts, operation, budget, |view, budget| {
                                    let view = view.expect("genuine read must have local conditions");
                                    assert!(view.requires_runtime_allocation_binding());
                                    assert!(!view.initialized_readable_region_is_proved());
                                    assert!(!view.grants_memory_or_launch_authority());
                                    assert!(!view.native.memory_safety_is_complete());
                                    assert!(std::ptr::eq(view.fact.owner(), inventory.owner()));
                                    assert_eq!(view.fact.operation(), operation);
                                    assert!(std::ptr::eq(view.pair, pair));
                                    check_source_read_endpoint_prepaid_v30(
                                        source.roles.original, inventory, facts, view.pair, &view.fact, budget,
                                    )?;
                                    let mut count = observed.get(); count[0] += 1; observed.set(count);
                                    Ok(())
                                }).map_err(local_read_test_error_v18)?;
                                assert_eq!(budget.storage(), floor);
                                let work = budget.work() - before;
                                if let Some(previous) = previous { assert_eq!(work, previous); }
                                previous = Some(work);
                            }
                            for row in source.roles.rows {
                                if row.global.as_ref().is_some_and(|pair| pair.output.writing) {
                                    source.with_local_read_conditions_v18(native, facts, row.output, budget, |view, _| {
                                        assert!(view.is_none(), "Store cannot consume a read fact");
                                        let mut count = observed.get(); count[1] += 1; observed.set(count);
                                        Ok(())
                                    }).map_err(local_read_test_error_v18)?;
                                    assert_eq!(budget.storage(), floor);
                                }
                            }
                        }
                        GlobalReadConditionTestV18::NormalizedPositive => {
                            let mut native_entered = false;
                            source.with_native_access_v18(native, operation, budget, |view, _| {
                                let view = view.expect("assertion-backed read must join its actual native access");
                                assert!(std::ptr::eq(view.pair, pair));
                                assert!(!view.pair.output.writing);
                                assert!(!view.grants_memory_or_launch_authority());
                                assert!(!view.native.memory_safety_is_complete());
                                native_entered = true;
                                Ok(())
                            }).map_err(PendingGlobalReadConditionErrorV18::from)
                                .map_err(local_read_test_error_v18)?;
                            assert!(native_entered);
                            assert_eq!(budget.storage(), floor);
                            assert_ne!(pair.output.logical.index, pair.output.address_index,
                                "the normalized source index and actual GEP index remain distinct");
                            let fe2o3_kernel_ir::CanonicalGuardedGlobalReadOutcomeV18::ProvedLocalConditions(fact)
                                = facts.read_at(operation, budget).unwrap()
                                else { panic!("original assertion must independently prove a local read"); };
                            let index = optimized_source_definition_row_v18(inventory, pair.output.logical.index, budget).unwrap();
                            let length = source_operation_row_v18(inventory, pair.output.logical.length, budget).unwrap();
                            let [length_result] = length.operation.results.as_slice() else { panic!("length result"); };
                            assert!(index.value != Some(fact.domain().guard_index())
                                || length_result.id != fact.domain().length(), "actual representation chain required");
                            let mut entered = false;
                            source.with_local_read_conditions_v18(native, facts, operation, budget, |view, _| {
                                let view = view.expect("real normalized read must complete local-condition join");
                                assert!(std::ptr::eq(view.pair, pair));
                                assert_eq!(view.fact.normalized_index_origin(),
                                    fe2o3_kernel_ir::CanonicalGuardedReadIndexOriginV1::ProvenOrigin(index.value.unwrap()));
                                assert_eq!(view.fact.normalized_length_origin(), length_result.id);
                                assert!(view.requires_runtime_allocation_binding());
                                assert!(!view.initialized_readable_region_is_proved());
                                assert!(!view.grants_memory_or_launch_authority());
                                entered = true;
                                Ok(())
                            }).map_err(local_read_test_error_v18)?;
                            assert!(entered);
                            assert_eq!(budget.storage(), floor);
                            observed.set([1, 0, 0]);
                        }
                        GlobalReadConditionTestV18::NormalizedDomain(fault) => {
                            let other = source.roles.rows.iter().find_map(|row| row.global.as_ref().filter(|other|
                                !other.output.writing && other.output.logical.access.operation != operation
                                    && other.output.logical.root == pair.output.logical.root))
                                .expect("second authentic same-root guarded read");
                            for candidate in [pair, other] {
                                source.with_local_read_conditions_v18(native, facts, candidate.output.logical.access.operation,
                                    budget, |view, _| {
                                        let view = view.expect("both real normalized reads require positive joins");
                                        assert!(std::ptr::eq(view.pair, candidate));
                                        assert!(!view.grants_memory_or_launch_authority());
                                        Ok(())
                                    }).unwrap();
                                assert_eq!(budget.storage(), floor);
                            }
                            let fe2o3_kernel_ir::CanonicalGuardedGlobalReadOutcomeV18::ProvedLocalConditions(fact)
                                = facts.read_at(operation, budget).unwrap() else { panic!("authentic normalized fact"); };
                            let mut changed = *pair;
                            match fault {
                                0 => {
                                    let exact = optimized_source_definition_row_v18(inventory, pair.output.logical.index, budget).unwrap();
                                    changed.output.logical.index = inventory.definitions().iter().find(|row|
                                        row.coordinate != exact.coordinate && row.ty == exact.ty && row.value.is_some()
                                            && match row.coordinate {
                                                SliceDefinition::FunctionArgument { function, .. } => function == operation.block.function,
                                                SliceDefinition::BlockArgument { block, .. } => block.function == operation.block.function,
                                                SliceDefinition::Result { operation: site, .. } => site.block.function == operation.block.function,
                                            }).expect("genuine same-type different normalized index").coordinate;
                                }
                                1 => {
                                    let exact = optimized_source_definition_row_v18(inventory, pair.output.address_index, budget).unwrap();
                                    let second = optimized_source_definition_row_v18(inventory, other.output.address_index, budget).unwrap();
                                    assert_eq!(exact.ty, second.ty);
                                    assert_ne!(exact.value, second.value);
                                    changed.output.address_index = second.coordinate;
                                }
                                2 => {
                                    let exact = optimized_source_definition_row_v18(inventory, pair.output.logical.guard_condition, budget).unwrap();
                                    let second = optimized_source_definition_row_v18(inventory, other.output.logical.guard_condition, budget).unwrap();
                                    assert_eq!(exact.ty, second.ty);
                                    assert_ne!(exact.value, second.value);
                                    changed.output.logical.guard_condition = second.coordinate;
                                }
                                3 => {
                                    assert_ne!(pair.output.logical.guard_edge, other.output.logical.guard_edge);
                                    changed.output.logical.guard_edge = other.output.logical.guard_edge;
                                }
                                4 => {
                                    let exact = source_operation_row_v18(inventory, pair.output.logical.address, budget).unwrap();
                                    let second = source_operation_row_v18(inventory, other.output.logical.address, budget).unwrap();
                                    assert_eq!(exact.operation.results[0].ty, second.operation.results[0].ty);
                                    assert_ne!(pair.output.pointer, other.output.pointer);
                                    changed.output.pointer = other.output.pointer;
                                }
                                _ => unreachable!(),
                            }
                            let headers = global_read_condition_headers_v18(0, 1).unwrap();
                            budget.reserve_storage(headers).unwrap();
                            // Copied-row internal join control. Both immutable owner facts above
                            // were genuine; the copied row itself conveys no source authority.
                            let result = source.check_local_read_domain_v18(inventory, facts, &changed, &fact, budget);
                            let expected = if matches!(fault, 2 | 3) {
                                "pending global read changed exact control transport"
                            } else {
                                "pending global read changed exact local domain"
                            };
                            assert!(matches!(result, Err(PendingGlobalReadConditionErrorV18::Source(
                                ProductionSourceOwnedViewErrorV18::Binding(detail))) if detail == expected),
                                "normalized fault {fault}: {result:?}");
                            drop(fact);
                            budget.release_storage(headers).unwrap();
                            assert_eq!(budget.storage(), floor);
                            observed.set([2, 1, 0]);
                        }
                        GlobalReadConditionTestV18::OriginalLengthOccurrence => {
                            // Duplicate SliceLength can be legitimately CSE'd in output. Use
                            // two actual original-input occurrences, never invented IDs, for
                            // this private same-owner domain-check control.
                            source.with_local_read_conditions_v18(native, facts, operation, budget, |view, _| {
                                assert!(view.is_some()); Ok(())
                            }).unwrap();
                            let other = source.roles.rows.iter().find_map(|row| row.global.as_ref().filter(|other|
                                !other.input.writing && other.input.logical.root == pair.input.logical.root
                                    && other.input.logical.length != pair.input.logical.length))
                                .expect("two distinct authenticated original SliceLength occurrences");
                            let mut original_pair = *pair;
                            original_pair.output = pair.input;
                            fe2o3_kernel_ir::with_canonical_guarded_global_reads_v18(original.inventory.owner(),
                                fe2o3_kernel_ir::CanonicalGuardedGlobalReadLimitsV1::default(), budget,
                                |input_facts, budget| {
                                    let before = budget.storage();
                                    let fe2o3_kernel_ir::CanonicalGuardedGlobalReadOutcomeV18::ProvedLocalConditions(fact)
                                        = input_facts.read_at(pair.input.logical.access.operation, budget).unwrap()
                                        else { panic!("original input has its own genuine local read fact"); };
                                    let exact = source_operation_row_v18(original.inventory, pair.input.logical.length, budget).unwrap();
                                    let second = source_operation_row_v18(original.inventory, other.input.logical.length, budget).unwrap();
                                    let (OperationKind::SliceLength { slice: first }, OperationKind::SliceLength { slice: second_slice })
                                        = (&exact.operation.kind, &second.operation.kind) else { panic!("actual length operations"); };
                                    assert_eq!(first, second_slice, "same actual root, not merely same scalar type");
                                    let ([first], [second]) = (exact.operation.results.as_slice(), second.operation.results.as_slice())
                                        else { panic!("one result per actual length occurrence"); };
                                    assert_eq!(first.ty, Type::INDEX);
                                    assert_eq!(first.ty, second.ty);
                                    assert_ne!(first.id, second.id);
                                    assert_eq!(fact.normalized_length_origin(), first.id);
                                    let headers = global_read_condition_headers_v18(0, 1).unwrap();
                                    budget.reserve_storage(headers).unwrap();
                                    source.check_local_read_domain_v18(original.inventory, input_facts, &original_pair, &fact, budget).unwrap();
                                    original_pair.output.logical.length = other.input.logical.length;
                                    let result = source.check_local_read_domain_v18(original.inventory, input_facts, &original_pair, &fact, budget);
                                    assert!(matches!(result, Err(PendingGlobalReadConditionErrorV18::Source(
                                        ProductionSourceOwnedViewErrorV18::Binding("pending global read changed exact local domain")))));
                                    drop(fact);
                                    budget.release_storage(headers).unwrap();
                                    assert_eq!(budget.storage(), before);
                                    observed.set([1, 1, 1]);
                                    Ok(())
                                }).unwrap();
                            assert_eq!(budget.storage(), floor);
                        }
                        GlobalReadConditionTestV18::ChangedDomain(fault) => {
                            source.with_local_read_conditions_v18(native, facts, operation, budget, |view, _| {
                                assert!(view.is_some()); Ok(())
                            }).unwrap();
                            let fe2o3_kernel_ir::CanonicalGuardedGlobalReadOutcomeV18::ProvedLocalConditions(fact)
                                = facts.read_at(operation, budget).unwrap() else { panic!("positive read fact"); };
                            // Internal copied-row control, not a constructor of source authority.
                            let mut changed = *pair;
                            match fault {
                                0 => {
                                    let root = optimized_source_definition_row_v18(inventory, pair.output.logical.root, budget).unwrap();
                                    changed.output.logical.root = inventory.definitions().iter()
                                        .find(|row| row.coordinate != root.coordinate && row.ty == root.ty
                                            && matches!(row.coordinate, SliceDefinition::FunctionArgument { .. }))
                                        .expect("second authentic same-type source slice parameter").coordinate;
                                }
                                1 => changed.output.logical.index = SliceDefinition::Result {
                                    operation: pair.output.logical.length, result: 0 },
                                2 => {
                                    let data = source_operation_row_v18(inventory, pair.output.logical.data, budget).unwrap();
                                    let [data] = data.operation.results.as_slice() else { panic!("data result"); };
                                    let address = source_operation_row_v18(inventory, pair.output.logical.address, budget).unwrap();
                                    assert_eq!(data.ty, address.operation.results[0].ty);
                                    assert_ne!(data.id, pair.output.pointer);
                                    changed.output.pointer = data.id;
                                }
                                3 => changed.output.logical.access.operation = pair.output.logical.length,
                                4 => changed.output.memory.volatile = true,
                                5 => changed.output.scalar = ScalarType::U64,
                                6 => changed.output.memory.alignment = 8,
                                7 => changed.output.logical.guard_edge.successor ^= 1,
                                8 => changed.output.writing = true,
                                9 => changed.output.memory.address_space = AddressSpace::Private,
                                10 => changed.output.value = inventory.definitions().iter()
                                    .find(|row| row.ty == &Type::Scalar(pair.output.scalar)
                                        && row.value.is_some_and(|value| value != pair.output.value))
                                    .expect("authentic same-scalar arithmetic input/result").value.unwrap(),
                                11 => {
                                    let SliceDefinition::Result { operation, .. } = pair.output.logical.index
                                        else { panic!("issuer's original ThreadIndex result"); };
                                    let length = source_operation_row_v18(inventory, operation, budget).unwrap();
                                    assert_eq!(length.operation.results[0].ty, Type::INDEX);
                                    assert_ne!(operation, pair.output.logical.length);
                                    changed.output.logical.length = operation;
                                }
                                _ => unreachable!(),
                            }
                            let headers = global_read_condition_headers_v18(0, 1).unwrap();
                            budget.reserve_storage(headers).unwrap();
                            let result = if fault == 10 {
                                source.check_native_pair_v18(native, &changed, budget).map_err(Into::into)
                            } else {
                                source.check_local_read_domain_v18(inventory, facts, &changed, &fact, budget)
                            };
                            assert!(matches!(result, Err(PendingGlobalReadConditionErrorV18::Source(
                                ProductionSourceOwnedViewErrorV18::Binding(_)))));
                            drop(fact);
                            budget.release_storage(headers).unwrap();
                            assert_eq!(budget.storage(), floor);
                            observed.set([1, usize::from(fault < 3), 0]);
                        }
                        GlobalReadConditionTestV18::ForeignFacts => {
                            assert!(!std::ptr::eq(original.inventory.owner(), inventory.owner()));
                            let result = fe2o3_kernel_ir::with_canonical_guarded_global_reads_v18(
                                original.inventory.owner(), fe2o3_kernel_ir::CanonicalGuardedGlobalReadLimitsV1::default(),
                                budget, |foreign, budget| {
                                    let before = budget.storage();
                                    let result = source.with_local_read_conditions_v18(native, foreign, operation, budget,
                                        |_, _| panic!("foreign formal owner reached callback"));
                                    assert!(matches!(result, Err(PendingGlobalReadConditionErrorV18::Source(
                                        ProductionSourceOwnedViewErrorV18::Binding("pending global read changed formal owner")))));
                                    assert_eq!(budget.storage(), before);
                                    observed.set([1, 0, 0]);
                                    Ok(())
                                });
                            result.unwrap();
                        }
                        GlobalReadConditionTestV18::ForeignQuery(extra) => {
                            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
                            let mut foreign = ArgumentBudgetV1::new(&mut work, budget.storage_limit());
                            foreign.reserve_storage(floor + extra).unwrap();
                            let before = (foreign.work(), foreign.storage());
                            let result = source.with_local_read_conditions_v18(native, facts, operation, &mut foreign,
                                |_, _| panic!("foreign query ledger cannot enter callback"));
                            assert!(matches!(result, Err(PendingGlobalReadConditionErrorV18::Source(
                                ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)))));
                            assert_eq!((foreign.work(), foreign.storage()), before);
                            assert!(original.source.cleanup.is_denied());
                            observed.set([1, 0, 0]);
                        }
                        GlobalReadConditionTestV18::HeaderCut => {
                            let callback = |_: Option<&PendingGlobalReadConditionsV18<'_, '_>>, _: &mut ArgumentBudgetV1<'_>| {
                                panic!("one-short frame must refuse before callback")
                            };
                            let required = global_read_condition_headers_v18(std::mem::size_of_val(&callback),
                                std::mem::align_of_val(&callback)).unwrap();
                            let padding = budget.storage_limit() - floor - required + 1;
                            budget.reserve_storage(padding).unwrap();
                            let first = source.with_local_read_conditions_v18(native, facts, operation, budget, callback);
                            let Err(PendingGlobalReadConditionErrorV18::Source(ProductionSourceOwnedViewErrorV18::Resource(
                                ArgumentResourceV1::Storage(first)))) = first else { panic!("exact frame cut"); };
                            assert_eq!(first.actual(), first.limit() + 1);
                            assert_eq!(budget.failed_storage(), Some(first.actual()));
                            budget.release_storage(padding).unwrap();
                            let before = (budget.work(), budget.storage());
                            let retry = source.with_local_read_conditions_v18(native, facts, operation, budget, |_, _| unreachable!());
                            assert!(matches!(retry, Err(PendingGlobalReadConditionErrorV18::Source(
                                ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(error)))) if error == first));
                            assert_eq!((budget.work(), budget.storage()), before);
                            observed.set([1, 0, 0]);
                        }
                        GlobalReadConditionTestV18::WorkCut => {
                            budget.charge_work(work_limit - budget.work()).unwrap();
                            let first = source.with_local_read_conditions_v18(native, facts, operation, budget,
                                |_, _| panic!("first formal owner query must refuse before callback"));
                            let Err(PendingGlobalReadConditionErrorV18::Formal { error:
                                fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1::Resource(
                                    fe2o3_kernel_ir::FormalGuardedMemoryResourceErrorV1::Work(first)), source_refusal:
                                ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(retained)) })
                                = first else { panic!("first formal query work refusal must remain typed"); };
                            assert_eq!(first, retained);
                            assert_eq!(first.actual(), first.limit() + 1);
                            assert_eq!(budget.failed_work(), Some(first.actual()));
                            let before = (budget.work(), budget.storage());
                            assert!(matches!(source.with_local_read_conditions_v18(native, facts, operation, budget, |_, _| unreachable!()),
                                Err(PendingGlobalReadConditionErrorV18::Source(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(error)))) if error == first));
                            assert_eq!((budget.work(), budget.storage()), before);
                            assert_eq!(budget.storage(), floor);
                            observed.set([1, 0, 0]);
                        }
                        GlobalReadConditionTestV18::QueryThenDropPanic => {
                            struct QueryDropPanic;
                            impl Drop for QueryDropPanic {
                                fn drop(&mut self) { std::panic::resume_unwind(Box::new(0x1648_u64)); }
                            }
                            source.with_local_read_conditions_v18(native, facts, operation, budget, |view, _| {
                                assert!(view.is_some()); Ok(())
                            }).unwrap();
                            budget.reserve_storage(size_of::<u64>()).unwrap();
                            let before = budget.storage();
                            budget.charge_work(work_limit - budget.work()).unwrap();
                            let dropper = QueryDropPanic;
                            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                source.with_local_read_conditions_v18(native, facts, operation, budget, move |_, _| {
                                    let _owned = &dropper;
                                    panic!("genuine formal query denial must precede callback entry");
                                })
                            }));
                            assert_eq!(*result.unwrap_err().downcast::<u64>().unwrap(), 0x1648);
                            assert_eq!(budget.storage(), before);
                            budget.release_storage(size_of::<u64>()).unwrap();
                            let before = (budget.work(), budget.storage());
                            let Err(fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1::Resource(
                                fe2o3_kernel_ir::FormalGuardedMemoryResourceErrorV1::Work(first)))
                                = facts.owner(budget) else { panic!("canonical query must retain its earlier work refusal"); };
                            assert_eq!(first.actual(), first.limit() + 1);
                            assert!(matches!(original.check(budget), Err(ProductionSourceOwnedViewErrorV18::Resource(
                                ArgumentResourceV1::Work(error))) if error == first));
                            assert_eq!((budget.work(), budget.storage()), before);
                            observed.set([1, 0, 0]);
                        }
                        GlobalReadConditionTestV18::SwallowedQueryThenError => {
                            let first = std::cell::Cell::new(None);
                            let result = source.with_local_read_conditions_v18(native, facts, operation, budget, |view, budget| {
                                assert!(view.is_some());
                                budget.charge_work(work_limit - budget.work())?;
                                let Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(error)))
                                    = source.operation_count(budget) else { panic!("authentic source query work refusal"); };
                                first.set(Some(error));
                                Err(ProductionSourceOwnedViewErrorV18::Binding("later swallowed-query sentinel").into())
                            });
                            assert!(matches!(result, Err(PendingGlobalReadConditionErrorV18::Source(
                                ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(error))))
                                if Some(error) == first.get()));
                            assert_eq!(budget.storage(), floor);
                            let before = (budget.work(), budget.storage());
                            assert!(matches!(original.check(budget), Err(ProductionSourceOwnedViewErrorV18::Resource(
                                ArgumentResourceV1::Work(error))) if Some(error) == first.get()));
                            assert_eq!((budget.work(), budget.storage()), before);
                            observed.set([1, 0, 0]);
                        }
                        GlobalReadConditionTestV18::SelectedError => {
                            let result = source.with_local_read_conditions_v18(native, facts, operation, budget, |view, _| {
                                assert!(view.is_some()); Err(ProductionSourceOwnedViewErrorV18::Binding("local read callback sentinel").into())
                            });
                            assert!(matches!(result, Err(PendingGlobalReadConditionErrorV18::Source(
                                ProductionSourceOwnedViewErrorV18::Binding("local read callback sentinel")))));
                            assert_eq!(budget.storage(), floor);
                            original.check(budget).unwrap();
                            source.with_local_read_conditions_v18(native, facts, operation, budget, |view, _| {
                                assert!(view.is_some()); Ok(())
                            }).unwrap();
                            observed.set([1, 0, 0]);
                        }
                        GlobalReadConditionTestV18::Unwind | GlobalReadConditionTestV18::ErrorThenDropPanic => {
                            struct DropPanic;
                            impl Drop for DropPanic { fn drop(&mut self) { std::panic::resume_unwind(Box::new(0x1647_u64)); } }
                            budget.reserve_storage(size_of::<u64>()).unwrap();
                            let before = budget.storage();
                            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                if matches!(mode, GlobalReadConditionTestV18::ErrorThenDropPanic) {
                                    let dropper = DropPanic;
                                    source.with_local_read_conditions_v18(native, facts, operation, budget, move |view, _| {
                                        let _keep_owned = &dropper; assert!(view.is_some());
                                        Err(ArgumentResourceV1::Arithmetic.into())
                                    })
                                } else {
                                    source.with_local_read_conditions_v18(native, facts, operation, budget, |view, _| {
                                        assert!(view.is_some()); std::panic::resume_unwind(Box::new(0x1647_u64))
                                    })
                                }
                            }));
                            assert_eq!(*result.unwrap_err().downcast::<u64>().unwrap(), 0x1647);
                            assert_eq!(budget.storage(), before);
                            budget.release_storage(size_of::<u64>()).unwrap();
                            if matches!(mode, GlobalReadConditionTestV18::ErrorThenDropPanic) {
                                let before = (budget.work(), budget.storage());
                                assert!(matches!(original.check(budget), Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Arithmetic))));
                                assert_eq!((budget.work(), budget.storage()), before);
                            } else { original.check(budget).unwrap(); }
                            observed.set([1, 0, 0]);
                        }
                        GlobalReadConditionTestV18::HigherFloor(disposition)
                        | GlobalReadConditionTestV18::ForeignLedger { disposition, .. } => {
                            let expected = std::cell::Cell::new(0usize);
                            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                source.with_local_read_conditions_v18(native, facts, operation, budget, |view, budget| {
                                    assert!(view.is_some());
                                    match mode {
                                        GlobalReadConditionTestV18::HigherFloor(_) => {
                                            budget.release_storage(1)?;
                                            expected.set(budget.storage());
                                        }
                                        GlobalReadConditionTestV18::ForeignLedger { extra, .. } => {
                                            let meter = Box::leak(Box::new(CanonicalKernelIrWorkBudgetV1::new(100_000_000)));
                                            let mut foreign = ArgumentBudgetV1::new(meter, 100_000_000);
                                            expected.set(budget.storage() + extra);
                                            foreign.reserve_storage(expected.get())?;
                                            drop(std::mem::replace(budget, foreign));
                                        }
                                        _ => unreachable!(),
                                    }
                                    match disposition {
                                        0 => Ok(()),
                                        1 => Err(ProductionSourceOwnedViewErrorV18::Binding("local custody sentinel").into()),
                                        _ => std::panic::resume_unwind(Box::new(0x1647_u64)),
                                    }
                                })
                            }));
                            assert_eq!(budget.storage(), expected.get(), "no refund from foreign or undercut credits");
                            assert!(original.source.cleanup.is_denied());
                            if matches!(mode, GlobalReadConditionTestV18::ForeignLedger { .. }) { assert_eq!(budget.work(), 0); }
                            match disposition {
                                0 => assert!(matches!(result.unwrap(),
                                    Err(PendingGlobalReadConditionErrorV18::Source(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)))
                                    | Err(PendingGlobalReadConditionErrorV18::Native(fe2o3_pliron::CanonicalRankedPolicyFailureV1::Resource(ArgumentResourceV1::Accounting))))),
                                1 => assert!(matches!(result.unwrap(), Err(PendingGlobalReadConditionErrorV18::Source(
                                    ProductionSourceOwnedViewErrorV18::Binding("local custody sentinel"))))),
                                _ => assert_eq!(*result.unwrap_err().downcast::<u64>().unwrap(), 0x1647),
                            }
                            observed.set([1, 0, 0]);
                        }
                    }
                    Ok(())
                })
            },
        );
        formal.map_err(|error| match error {
            ProductionOptimizedSourceFormalErrorV18::Source(error)
            | ProductionOptimizedSourceFormalErrorV18::Formal { source_refusal: error, .. }
            | ProductionOptimizedSourceFormalErrorV18::Consumer(error) => error,
        })
    })
}
