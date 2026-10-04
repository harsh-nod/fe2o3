use super::*;
use crate::production_analysis::{
    LivePlironStructuralIdentityProviderV1, PlironStructuralIdentityProviderV1,
};
use dialect_kernel::AccessKindAttr;
use fe2o3_kernel_ir::{
    CanonicalKernelIrWorkBudgetV1 as Work, ConditionalTotalViewAddressDomainV1 as Domain,
};

fn local(id: u32) -> ProductionRankedValueV1 {
    ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(id))
}

fn fixture() -> (
    ProductionPlironSessionV1,
    StageIdentityV1,
    crate::production_analysis::ProductionAnalysisInputCensusV1,
    [RecipeReadBoundV1; 2],
) {
    use ProductionRankedOperationV1 as O;
    let recipe = ProductionRankedKernelV1::new(
        "conditional_read_join",
        2,
        vec![ProductionRankedBlockV1::new(
            vec![
                O::View {
                    result: ProductionRankedValueIdV1::new(0),
                    element_width: 32,
                    writable: false,
                    shape: vec![DYNAMIC_EXTENT],
                    dynamic_extents: vec![ProductionRankedValueV1::Argument(0)],
                    allocation_origin: 1,
                    noalias_class: 1,
                },
                O::View {
                    result: ProductionRankedValueIdV1::new(1),
                    element_width: 32,
                    writable: false,
                    shape: vec![DYNAMIC_EXTENT],
                    dynamic_extents: vec![ProductionRankedValueV1::Argument(1)],
                    allocation_origin: 2,
                    noalias_class: 2,
                },
                O::IndexConstant {
                    result: ProductionRankedValueIdV1::new(2),
                    value: 0,
                },
                O::SemanticExpression {
                    result: ProductionRankedValueIdV1::new(3),
                    expression: ProductionSemanticExpressionV2::Constant {
                        scalar: ProductionSemanticScalarTypeV2::Bool,
                        bits: 1,
                    },
                    numerical_contract: ProductionNumericalContractV2::exact_for(
                        ProductionSemanticScalarTypeV2::Bool,
                    ),
                },
                O::Access {
                    kind: AccessKindAttr::Read,
                    view: local(0),
                    indices: vec![local(2)],
                },
                O::Access {
                    kind: AccessKindAttr::Read,
                    view: local(1),
                    indices: vec![local(2)],
                },
            ],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    let mut session =
        ProductionPlironSessionV1::new_ranked_v1(ProductionSessionLimitsV1::default()).unwrap();
    let registered = session
        .register_construction(
            ProductionConstructionV1::ranked_kernel("read_join", recipe).unwrap(),
        )
        .unwrap();
    let stage = session.construct_registered(registered).unwrap().0.identity;
    let context = &session.inner.context;
    let record = &session.constructed_roots[&stage];
    let function = FuncOp::from_operation(record.ranked_function.unwrap());
    let mut provider = LivePlironStructuralIdentityProviderV1::new(context, &function);
    let capture = provider
        .capture_with_resource_limits_v1(session.analysis_resource_limits())
        .ok()
        .unwrap();
    let census = capture.input_census;
    super::super::super::conditional_read_occurrences_v1::replay_read_occurrences_v1(
        context,
        &function,
        record.ranked_kernel.as_ref().unwrap(),
        &record.read_occurrences,
        context.ir_mutation_attempt_epoch().unwrap().value(),
        census,
    )
    .unwrap();
    let reads = [0, 1].map(|index| RecipeReadBoundV1 {
        block: 0,
        operation: index + 4,
        view: local(index),
        index: local(2),
        extent: ProductionRankedValueV1::Argument(index),
        domain: if index == 0 {
            Domain::GuardedOutput
        } else {
            Domain::GlobalLaunch
        },
    });
    (session, stage, census, reads)
}

#[test]
fn source_read_join_uses_captured_operations_and_preserves_domain_order() {
    let (session, stage, census, mut reads) = fixture();
    let record = &session.constructed_roots[&stage];
    let recipe = record.ranked_kernel.as_ref().unwrap();
    let mut resources =
        ProductionAnalysisResourceContractV1::new(session.analysis_resource_limits());
    let mapped = bind_live_reads_v1(
        recipe,
        &record.read_occurrences,
        census,
        &reads,
        &mut resources,
        None,
    )
    .unwrap();
    for (index, row) in mapped.iter().enumerate() {
        let original = &record.read_occurrences[index];
        assert_eq!(row.operation, original.operation);
        assert_eq!(row.view, original.view);
        assert_eq!(row.index, original.coordinates()[0].index);
        assert_eq!(
            ReadExtentV1::Dynamic(row.extent),
            original.coordinates()[0].extent
        );
        assert_eq!(row.domain, reads[index].domain);
    }
    reads.swap(0, 1);
    let reordered = bind_live_reads_v1(
        recipe,
        &record.read_occurrences,
        census,
        &reads,
        &mut resources,
        None,
    )
    .unwrap();
    assert_eq!(mapped, reordered);
}

#[test]
fn source_read_join_rejects_incomplete_duplicate_or_substituted_coordinates() {
    let (session, stage, census, original) = fixture();
    let record = &session.constructed_roots[&stage];
    for case in 0..7 {
        let mut reads = original.to_vec();
        match case {
            0 => {
                reads.pop();
            }
            1 => reads[1] = reads[0],
            2 => reads[0].block += 1,
            3 => reads[0].operation += 1,
            4 => reads[0].view = reads[1].view,
            5 => reads[0].index = ProductionRankedValueV1::Argument(0),
            6 => reads[0].extent = reads[1].extent,
            _ => unreachable!(),
        }
        let mut resources =
            ProductionAnalysisResourceContractV1::new(session.analysis_resource_limits());
        assert!(
            matches!(
                bind_live_reads_v1(
                    record.ranked_kernel.as_ref().unwrap(),
                    &record.read_occurrences,
                    census,
                    &reads,
                    &mut resources,
                    None,
                ),
                Err(ProductionSessionErrorV1::RankedGraphChanged)
            ),
            "case {case}"
        );
    }
}

#[test]
fn source_read_join_charges_original_canonical_account_at_exact_limits() {
    let (session, stage, census, reads) = fixture();
    let record = &session.constructed_roots[&stage];
    let run = |work_limit, storage_limit| {
        let mut work = Work::new(work_limit);
        let mut budget = CanonicalBudget::new(&mut work, storage_limit);
        let mut resources =
            ProductionAnalysisResourceContractV1::new(session.analysis_resource_limits());
        let account = RefCell::new(&mut budget);
        let result =
            retain_source_reads_v1(&reads, &mut resources, Some(&account)).and_then(|retained| {
                bind_live_reads_v1(
                    record.ranked_kernel.as_ref().unwrap(),
                    &record.read_occurrences,
                    census,
                    &retained,
                    &mut resources,
                    Some(&account),
                )
            });
        let bound = resources.cumulative();
        drop(account);
        let storage = budget.storage();
        drop(budget);
        (result, bound, work.work(), storage)
    };
    let (mapped, bound, work, storage) = run(usize::MAX, usize::MAX);
    assert_eq!(mapped.unwrap().len(), 2);
    assert_eq!(work, bound.work_upper_bound());
    assert_eq!(storage, bound.retained_storage_upper_bound());
    assert_eq!(
        storage,
        2 * (std::mem::size_of::<RecipeReadBoundV1>() + std::mem::size_of::<LiveReadBoundV1>())
    );
    assert!(run(work, storage).0.is_ok());
    assert!(run(work - 1, storage).0.is_err());
    assert!(run(work, storage - 1).0.is_err());
}
