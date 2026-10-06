use super::*;

#[test]
fn source_descriptor_loan_archive_has_an_independent_nominal_field_layout() {
    type Fields = (
        usize,
        SemanticBorrowKindV1,
        SourceReferenceSiteV29,
        usize,
        ProductionCallInstanceIdV1,
        SemanticLocalIdV1,
        u32,
        SemanticFunctionIdV1,
        SemanticTypeIdV1,
        ProductionSourceReferenceCarrierV38,
    );
    assert_eq!(size_of::<Fields>(), size_of::<SourceSsaLoanV36>());
    assert_eq!(align_of::<Fields>(), align_of::<SourceSsaLoanV36>());
}

#[path = "production_source_ssa_results_v30_tests.rs"]
mod ssa_tests;

fn probe(
    allowance: Option<(usize, usize)>,
    bad_site: bool,
) -> (SourceOwnedResultV18<()>, usize, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    let prepared = prepared_source_fixture(ModuleFixture::Ordinary, false, &mut budget);
    let adopted = prepared.adopted_storage();
    budget
        .reserve_storage(budget.peak_storage() + 1 - budget.storage())
        .unwrap();
    if let Some((work, storage)) = allowance {
        budget
            .charge_work(MODULE_LIMIT - budget.work() - work)
            .unwrap();
        budget
            .reserve_storage(MODULE_LIMIT - budget.storage() - storage)
            .unwrap();
    }
    let floor = budget.storage();
    let before = budget.work();
    let result: SourceOwnedResultV18<()> = prepared.with_checked_source_v18(&mut budget, |source, budget| {
        source.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget| {
            source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                for root in 0..source.root_count(budget)? {
                    let owner = source.root_row(root)?;
                    let retained = owner.rvalue_results.as_ref().expect("mandatory retained source roster");
                    assert_eq!(retained.rows.len(), 1);
                    assert!(retained.storage >= size_of::<SourceRvalueRowV30>());
                    let row = retained.rows[0];
                    assert_eq!((row.instance, row.block, row.statement), (0, 0, 0));
                    let definition = relation.assignment_scalar_definition_v30(
                        root, row.instance, SemanticBlockIdV1::from_index(row.block),
                        if bad_site { u32::MAX } else { row.statement }, budget,
                    )?.expect("the original assignment copies its scalar argument");
                    let actual = &inventory.definitions()[definition];
                    assert!(matches!(actual.coordinate,
                        fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::FunctionArgument { function, argument: 0 }
                        if function.0 as usize == owner.function_ordinal));
                    assert_eq!(actual.ty, &Type::Scalar(ScalarType::U32));
                }
                Ok(())
            })
        }))
    });
    assert_eq!(budget.storage(), floor - adopted);
    (
        result,
        budget.work() - before,
        budget.peak_storage() - floor,
    )
}

#[test]
fn retained_source_rvalues_join_every_actual_scalar_assignment_to_original_inventory() {
    probe(None, false).0.unwrap();
}

#[test]
fn retained_source_rvalues_missing_original_site_is_a_sticky_refusal() {
    assert!(matches!(
        probe(None, true).0,
        Err(ProductionSourceOwnedViewErrorV18::Binding(
            "original assignment result site differs"
        ))
    ));
}

#[test]
fn retained_source_rvalue_capture_and_query_have_exact_and_one_short_resources() {
    let (result, work, storage) = probe(None, false);
    result.unwrap();
    let exact = probe(Some((work, storage)), false);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2), (work, storage));
    let short_work = probe(Some((work - 1, storage)), false).0.unwrap_err();
    assert!(matches!(
        entrance_resource(short_work),
        ArgumentResourceV1::Work(_)
    ));
    let short_storage = probe(Some((work, storage - 1)), false).0.unwrap_err();
    assert!(matches!(
        entrance_resource(short_storage),
        ArgumentResourceV1::Storage(_)
    ));
}

#[test]
fn retained_source_rvalue_replay_detects_type_endpoint_omission_and_source_changes() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    let prepared = prepared_source_fixture(ModuleFixture::Ordinary, false, &mut budget);
    prepared
        .with_checked_source_v18(&mut budget, |source, budget| {
            let original = source.root_row(0)?.rvalue_results.as_ref().unwrap();
            for fault in 0..6 {
                let mut altered = OwnedSourceRvaluesV30 {
                    source: original.source,
                    ledger: original.ledger,
                    rows: original.rows.clone(),
                    values: original.values.clone(),
                    carriers: original.carriers.clone(),
                    index_readers: original.index_readers.clone(),
                    enum_spills: original.enum_spills.clone(),
                    storage: original.storage,
                };
                match fault {
                    0 => {}
                    1 => altered.rows[0].ty = SemanticTypeIdV1::from_index(u32::MAX),
                    2 => altered.rows[0].endpoint = SourceRvalueEndpointV30::Unmodeled,
                    3 => {
                        altered.rows.pop();
                    }
                    4 => altered.source.semantic[0] ^= 1,
                    5 => {
                        altered.rows[0].descriptor = Some(SourceDescriptorOperandV30 {
                            event: 0,
                            original: SsaValueV1::Definition(
                                fe2o3_mir_model::SsaDefinitionIdV1::new(0),
                            ),
                            receiver: ValueId(0),
                        })
                    }
                    _ => unreachable!(),
                }
                assert_eq!(
                    original.matches_replay_v30(&altered, budget).map_err(|_| {
                        ProductionSourceOwnedViewErrorV18::Binding("test replay failed")
                    })?,
                    fault == 0
                );
            }
            Ok(())
        })
        .unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn retained_rvalue_projection_never_flattens_an_unmodeled_whole_binding() {
    let scalar = SemanticValueBindingV1::Value {
        id: ValueId(7),
        ty: Type::Scalar(ScalarType::U32),
    };
    assert_eq!(
        source_rvalue_endpoint_v30(&scalar),
        SourceRvalueEndpointV30::Scalar {
            value: ValueId(7),
            scalar: ScalarType::U32,
        }
    );
    assert_eq!(
        source_rvalue_endpoint_v30(&SemanticValueBindingV1::Unit),
        SourceRvalueEndpointV30::Unit
    );
    assert_eq!(
        source_rvalue_endpoint_v30(&SemanticValueBindingV1::Aggregate(vec![scalar])),
        SourceRvalueEndpointV30::Unmodeled
    );
    assert_eq!(
        source_rvalue_endpoint_v30(&SemanticValueBindingV1::Unmaterialized),
        SourceRvalueEndpointV30::Unmodeled
    );
}

#[test]
fn retained_rvalue_header_oracle_covers_capture_and_query_envelopes() {
    fn h<T>() -> usize {
        size_of::<T>()
            + 2 * size_of::<Result<T, ProductionSemanticKirErrorV1>>()
            + 2 * size_of::<SourceOwnedResultV18<T>>()
    }
    let expected = h::<OwnedSourceRvaluesV30>()
        + h::<Vec<SourceRvalueRowV30>>()
        + h::<SourceRvalueRowV30>()
        + h::<SourceRvalueEndpointV30>()
        + h::<Vec<SourceSsaRowV30>>()
        + h::<SourceSsaRowV30>()
        + h::<&SourceSsaRowV30>()
        + h::<SsaValueV1>()
        + h::<(usize, SsaValueV1)>()
        + h::<&fe2o3_pliron::ProductionSemanticSsaFunctionPlanV1>()
        + h::<std::collections::btree_map::Iter<'_, SsaValueV1, Box<SemanticValueBindingV1>>>()
        + h::<&ExecutionArchiveV29>()
        + h::<&ExecutionInstancesV29<'_>>()
        + h::<&SourceReferencePlanV29<'_, '_>>()
        + h::<&mut PendingScopedRootEmissionV29>()
        + h::<&mut ArgumentBudgetV1<'_>>()
        + h::<&ProductionSourceCorrespondenceV18<'_>>()
        + h::<&SemanticValueBindingV1>()
        + h::<&ScopedModuleRootV29>()
        + h::<&OwnedSourceRvaluesV30>()
        + h::<&SourceRvalueRowV30>()
        + h::<&SemanticFunctionDeclV1>()
        + h::<&fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1>()
        + h::<(
            &SourceRvalueRowV30,
            &fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1,
        )>()
        + h::<&fe2o3_kernel_analysis::CanonicalKirDefinitionRefV1<'_>>()
        + h::<Option<usize>>()
        + h::<Type>()
        + h::<(usize, u32, u32)>()
        + h::<ExecutionCallSourceV29>()
        + h::<ExecutionSiteV29>()
        + size_of::<Option<SourceCheckedResultV44>>()
        + 2 * size_of::<Result<Option<SourceCheckedResultV44>, ProductionSemanticKirErrorV1>>()
        + size_of::<[usize; 2]>()
        + 2 * size_of::<SourceOwnedResultV18<[usize; 2]>>()
        + size_of::<[&SemanticValueBindingV1; 2]>()
        + size_of::<[SemanticTypeIdV1; 2]>()
        + 6 * size_of::<Type>()
        + 12 * size_of::<usize>()
        + descriptor_operand_header_oracle_v30()
        + index_reader_header_oracle_v35()
        + typed_endpoint_header_oracle_v36()
        + enum_spill_header_oracle_v48()
        + h::<std::slice::Iter<'_, SourceRvalueRowV30>>()
        + 8 * h::<usize>()
        + h::<()>();
    assert_eq!(source_rvalue_headers_v30().unwrap(), expected);
}

fn enum_spill_header_oracle_v48() -> usize {
    fn h<T>() -> usize {
        size_of::<T>()
            + 2 * size_of::<Result<T, ProductionSemanticKirErrorV1>>()
            + 2 * size_of::<SourceOwnedResultV18<T>>()
    }
    h::<Vec<SourceEnumSpillRowV48>>()
        + h::<SourceEnumSpillRowV48>()
        + h::<ExecutionEnumSpillV48>()
        + h::<ProductionSourceEnumSpillOriginV48>()
        + h::<ProductionSourceEnumSpillV48<'_, '_>>()
        + h::<&[SourceEnumSpillRowV48]>()
        + h::<std::slice::Iter<'_, SourceEnumSpillRowV48>>()
        + h::<(&ExecutionEnumSpillV48, &ExecutionEnumSpillV48)>()
        + h::<(&Type, &Type)>()
        + h::<&fe2o3_kernel_analysis::CanonicalKirOperationRefV1<'_>>()
        + h::<&fe2o3_kernel_analysis::CanonicalKirDefinitionRefV1<'_>>()
        + h::<[usize; 6]>()
}

fn typed_endpoint_header_oracle_v36() -> usize {
    fn h<T>() -> usize {
        size_of::<T>()
            + 2 * size_of::<Result<T, ProductionSemanticKirErrorV1>>()
            + 2 * size_of::<SourceOwnedResultV18<T>>()
    }
    h::<SourceSsaElementV36>()
        + h::<SourceSsaCarrierTypeV36>()
        + h::<SourceSsaLoanV36>()
        + execution_borrow_header_oracle_v163()
        + execution_owner_header_oracle_v199()
        + reference_endpoint_header_oracle_v38()
        + h::<SourceSsaWitnessV50>()
        + h::<(ValueId, SourceSsaCarrierTypeV36)>()
        + h::<SourceSsaPhysicalV36>()
        + h::<SourceSsaEndpointRowV36>()
        + carrier_tree_header_oracle_v37()
        + h::<ProductionSourceSsaEndpointV36<'_, '_>>()
        + h::<Vec<Option<SemanticLocalIdV1>>>()
        + h::<&mut [Option<SemanticLocalIdV1>]>()
        + h::<&[Option<SemanticLocalIdV1>]>()
        + h::<Option<SemanticLocalIdV1>>()
        + h::<&SourceReferenceEmissionV29<'_, '_>>()
        + h::<&SemanticSourceReferenceBindingV29>()
        + h::<&SourceReferenceLoanV29>()
        + h::<&SourceReferenceOriginV29>()
        + h::<&fe2o3_mir_model::SsaConstructionPlanV1>()
        + h::<fe2o3_pliron::ProductionSemanticSsaFunctionOccurrencesV1<'_>>()
        + h::<std::slice::Iter<'_, fe2o3_pliron::ProductionSemanticSsaEntryDefinitionOccurrenceV1>>(
        )
        + h::<std::slice::Iter<'_, fe2o3_pliron::ProductionSemanticSsaEventOccurrenceV1>>()
        + h::<std::slice::Iter<'_, fe2o3_pliron::ProductionSemanticSsaEdgeDefinitionOccurrenceV1>>()
        + h::<std::slice::Iter<'_, Option<SemanticLocalIdV1>>>()
        + h::<&SemanticTypeShapeV1>()
        + h::<Option<SourceSsaCarrierTypeV36>>()
        + h::<(
            SourceReferenceBindingOriginV29,
            Option<SourceSsaCarrierTypeV36>,
        )>()
        + h::<(Option<SourceSsaCarrierTypeV36>, &SemanticTypeShapeV1)>()
        + h::<(bool, Option<SourceSsaCarrierTypeV36>)>()
        + h::<(&SemanticValueBindingV1, &SemanticTypeShapeV1)>()
        + h::<SemanticFunctionIdV1>()
        + h::<SemanticLocalIdV1>()
        + h::<SemanticTypeIdV1>()
        + 12 * h::<usize>()
}

fn execution_borrow_header_oracle_v163() -> usize {
    fn h<T>() -> usize {
        size_of::<T>()
            + 2 * size_of::<Result<T, ProductionSemanticKirErrorV1>>()
            + 2 * size_of::<SourceOwnedResultV18<T>>()
    }
    let expected = h::<SourceSsaExecutionBorrowV163>()
        + h::<ProductionSourceExecutionBorrowSiteV163>()
        + h::<ProductionSourceExecutionBorrowCoordinatesV163>()
        + h::<Option<ProductionSourceExecutionBorrowCoordinatesV163>>()
        + h::<&SemanticExecutionBorrowBindingV29>()
        + 8 * h::<&()>();
    assert_eq!(source_execution_borrow_headers_v163().unwrap(), expected);
    expected
}

fn execution_owner_header_oracle_v199() -> usize {
    fn h<T>() -> usize {
        size_of::<T>()
            + 2 * size_of::<Result<T, ProductionSemanticKirErrorV1>>()
            + 2 * size_of::<SourceOwnedResultV18<T>>()
    }
    let expected = h::<ProductionSourceExecutionIdentityV199>()
        + h::<ProductionSourceExecutionOwnerV199>()
        + h::<Option<ProductionSourceExecutionIdentityV199>>()
        + h::<Option<ProductionSourceExecutionOwnerV199>>()
        + 8 * h::<&()>();
    assert_eq!(source_execution_owner_headers_v199().unwrap(), expected);
    expected
}

fn reference_endpoint_header_oracle_v38() -> usize {
    fn h<T>() -> usize {
        size_of::<T>()
            + 2 * size_of::<Result<T, ProductionSemanticKirErrorV1>>()
            + 2 * size_of::<SourceOwnedResultV18<T>>()
    }
    h::<ProductionSourceReferenceCarrierV38>()
        + h::<ProductionSourceReferenceEndpointV38<'_, '_>>()
        + h::<Option<ProductionSourceReferenceEndpointV38<'_, '_>>>()
        + h::<(usize, SemanticBlockIdV1, Option<usize>)>()
        + h::<SourceReferenceRepresentationV29>()
        + h::<SemanticBorrowKindV1>()
        + h::<u32>()
        + h::<&SourceSsaLoanV36>()
}

fn carrier_tree_header_oracle_v37() -> usize {
    fn h<T>() -> usize {
        size_of::<T>()
            + 2 * size_of::<Result<T, ProductionSemanticKirErrorV1>>()
            + 2 * size_of::<SourceOwnedResultV18<T>>()
    }
    h::<SourceSsaComponentV37>()
        + enum_carrier_header_oracle_v47()
        + h::<SourceCarrierFrameV37<'_>>()
        + h::<ProductionSourceSsaCarrierShapeV37>()
        + h::<Vec<SourceSsaComponentV37>>()
        + h::<&mut Vec<SourceSsaComponentV37>>()
        + h::<&[SourceSsaComponentV37]>()
        + h::<Vec<SourceCarrierFrameV37<'_>>>()
        + h::<Option<SourceCarrierFrameV37<'_>>>()
        + h::<&SemanticTypeShapeV1>()
        + h::<&Vec<SemanticValueBindingV1>>()
        + h::<std::ops::Range<usize>>()
        + h::<std::iter::Rev<std::iter::Enumerate<std::slice::Iter<'_, SemanticValueBindingV1>>>>()
        + h::<fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1>()
        + h::<fe2o3_kernel_ir::FixedVectorTypeV12>()
        + h::<(
            &ExecutionInstancesV29<'_>,
            &SourceReferenceEmissionV29<'_, '_>,
            &mut Vec<SourceCarrierFrameV37<'_>>,
            &mut Vec<SourceSsaComponentV37>,
            &mut ArgumentBudgetV1<'_>,
        )>()
        + 10 * h::<usize>()
}

fn enum_carrier_header_oracle_v47() -> usize {
    fn h<T>() -> usize {
        size_of::<T>()
            + 2 * size_of::<Result<T, ProductionSemanticKirErrorV1>>()
            + 2 * size_of::<SourceOwnedResultV18<T>>()
    }
    h::<Option<SourceSsaPhysicalV36>>()
        + h::<Option<(usize, usize)>>()
        + h::<Option<SemanticOptionAvailabilityV1>>()
        + h::<(
            &ExecutionInstancesV29<'_>,
            SemanticTypeIdV1,
            &SemanticValueBindingV1,
            &mut Vec<SourceSsaComponentV37>,
            &mut ArgumentBudgetV1<'_>,
        )>()
        + h::<Option<ProductionSourceSsaEndpointV36<'_, '_>>>()
        + h::<(SemanticTypeIdV1, usize)>()
        + h::<std::cmp::Ordering>()
        + h::<std::collections::btree_map::Iter<'_, u32, Vec<SemanticValueBindingV1>>>()
        + h::<
            std::iter::Enumerate<
                std::collections::btree_map::Iter<'_, u32, Vec<SemanticValueBindingV1>>,
            >,
        >()
        + h::<std::slice::Iter<'_, SemanticTypeIdV1>>()
        + h::<(
            &ExecutionInstancesV29<'_>,
            SemanticTypeIdV1,
            &SemanticValueBindingV1,
            &mut Vec<SourceSsaComponentV37>,
            &mut Vec<SourceCarrierFrameV37<'_>>,
            &mut ArgumentBudgetV1<'_>,
        )>()
        + 16 * h::<usize>()
}

fn index_reader_header_oracle_v35() -> usize {
    fn h<T>() -> usize {
        size_of::<T>()
            + 2 * size_of::<Result<T, ProductionSemanticKirErrorV1>>()
            + 2 * size_of::<SourceOwnedResultV18<T>>()
    }
    #[allow(dead_code)]
    struct Reader {
        instance: usize,
        block: u32,
        function: SemanticFunctionIdV1,
        callee: fe2o3_mir_model::semantic_mir_v1::SemanticCallableIdV1,
        argument: SsaValueV1,
        result: SsaValueV1,
        reference_type: SemanticTypeIdV1,
        witness_type: SemanticTypeIdV1,
        index_space: SemanticDisjointIndexSpaceV1,
        disjoint: bool,
        loan: usize,
        loan_site: SourceReferenceSiteV29,
        origin: usize,
        origin_instance: ProductionCallInstanceIdV1,
        origin_local: SemanticLocalIdV1,
        origin_generation: u32,
        emitted_index: ValueId,
    }
    assert_eq!(size_of::<Reader>(), size_of::<SourceIndexReaderRowV35>());
    assert_eq!(
        std::mem::align_of::<Reader>(),
        std::mem::align_of::<SourceIndexReaderRowV35>()
    );
    let expected = h::<Vec<SourceIndexReaderRowV35>>()
        + h::<Reader>()
        + h::<&SourceIndexReaderRowV35>()
        + h::<&SourceReferenceEmissionV29<'_, '_>>()
        + h::<SourceIndexWitnessBorrowV29>()
        + h::<&SourceReferenceLoanV29>()
        + h::<&SourceReferenceOriginV29>()
        + h::<&SemanticDirectCallV1>()
        + h::<&SemanticOperandV1>()
        + h::<&SemanticSourceReferenceBindingV29>()
        + h::<(SemanticTypeIdV1, SemanticDisjointIndexSpaceV1, bool)>()
        + h::<Option<SsaValueV1>>()
        + h::<SsaValueV1>()
        + h::<ValueId>()
        + h::<[u32; 2]>()
        + h::<(usize, u32)>()
        + h::<ProductionSourceIndexReadV35<'_, '_>>()
        + h::<Option<ProductionSourceIndexReadV35<'_, '_>>>()
        + h::<&ProductionSourceIndexReadV35<'_, '_>>()
        + h::<fe2o3_mir_model::SsaEdgeIdV1>()
        + h::<&SsaArgumentV1>()
        + h::<ProductionSemanticExpressionV2>()
        + h::<ProductionSemanticScalarTypeV2>()
        + h::<&AdmittedInertSemanticMirV1>()
        + h::<ProductionCallInstanceIdV1>()
        + h::<&production_call_instances_v1::ProductionCallInstanceV1<'_>>()
        + h::<SemanticFunctionIdV1>()
        + h::<SemanticBlockIdV1>()
        + h::<&fe2o3_mir_model::semantic_mir_v1::SemanticCallDestinationV1>()
        + h::<&SemanticPlaceV1>()
        + h::<bool>()
        + h::<fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1>()
        + h::<&[SsaArgumentV1]>()
        + h::<fe2o3_pliron::ProductionSemanticSsaFunctionOccurrencesV1<'_>>()
        + h::<
            std::iter::Enumerate<
                std::slice::Iter<'_, fe2o3_mir_model::semantic_mir_v1::SemanticBasicBlockV1>,
            >,
        >()
        + h::<std::iter::Enumerate<std::slice::Iter<'_, Option<usize>>>>()
        + h::<std::slice::Iter<'_, fe2o3_pliron::ProductionSemanticSsaEventOccurrenceV1>>()
        + 8 * h::<usize>();
    assert_eq!(source_index_reader_headers_v35().unwrap(), expected);
    expected
}

fn descriptor_operand_header_oracle_v30() -> usize {
    #[allow(dead_code)]
    struct Operand {
        event: usize,
        original: SsaValueV1,
        receiver: ValueId,
    }
    assert_eq!(
        size_of::<Operand>(),
        size_of::<SourceDescriptorOperandV30>()
    );
    type Frame<'a> = (
        [&'a (); 8],
        [usize; 4],
        Operand,
        Option<Operand>,
        Option<(&'a SemanticPlaceV1, usize, ExecutionOperandV29)>,
        Option<(usize, SsaValueV1)>,
        Option<&'a fe2o3_pliron::ProductionSemanticSsaEventOccurrenceV1>,
        fe2o3_pliron::ProductionSemanticSsaFunctionOccurrencesV1<'a>,
        Result<Option<Operand>, ProductionSemanticKirErrorV1>,
        SourceOwnedResultV18<()>,
        Result<&'a SemanticValueBindingV1, ProductionSemanticKirErrorV1>,
        &'a SemanticValueBindingV1,
        std::iter::Enumerate<
            std::slice::Iter<'a, fe2o3_pliron::ProductionSemanticSsaEventOccurrenceV1>,
        >,
        std::slice::Iter<'a, SemanticProjectionV1>,
    );
    let expected = size_of::<Frame<'_>>() + std::mem::align_of::<Frame<'_>>();
    assert_eq!(source_descriptor_operand_headers_v30().unwrap(), expected);
    expected
}

#[test]
fn retained_rvalue_query_rejects_funded_foreign_ledger_before_any_charge() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    let prepared = prepared_source_fixture(ModuleFixture::Ordinary, false, &mut budget);
    let at_refusal = std::cell::Cell::new(None);
    let result: SourceOwnedResultV18<()> =
        prepared.with_checked_source_v18(&mut budget, |source, budget| {
            source.with_analysis_v18(budget, |scope| {
                scope.with_inventory_v1(|inventory, budget| {
                    source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                        let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
                        let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, MODULE_LIMIT);
                        foreign.reserve_storage(budget.storage())?;
                        let before = (foreign.work(), foreign.storage(), foreign.peak_storage());
                        assert!(matches!(
                            relation.assignment_scalar_definition_v30(
                                0,
                                0,
                                SemanticBlockIdV1::from_index(0),
                                0,
                                &mut foreign
                            ),
                            Err(ProductionSourceOwnedViewErrorV18::Resource(
                                ArgumentResourceV1::Accounting
                            ))
                        ));
                        assert_eq!(
                            (foreign.work(), foreign.storage(), foreign.peak_storage()),
                            before
                        );
                        at_refusal.set(Some(budget.storage()));
                        Err(ProductionSourceOwnedViewErrorV18::Resource(
                            ArgumentResourceV1::Accounting,
                        ))
                    })
                })
            })
        });
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Resource(
            ArgumentResourceV1::Accounting
        ))
    ));
    assert_eq!(Some(budget.storage()), at_refusal.get());
}
