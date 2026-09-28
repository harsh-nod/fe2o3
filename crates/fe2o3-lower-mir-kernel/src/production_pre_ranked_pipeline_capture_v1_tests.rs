use super::*;
use fe2o3_mir_model::{SsaResolvedEventV1, SsaValueV1};
use fe2o3_pliron::{
    ProductionSemanticSsaEventRoleV1 as Role, ProductionSemanticSsaOccurrenceSiteV1 as Site,
};

fn unused_pipeline_declaration_fixture() -> (
    ProductionSemanticSsaOwnerV1,
    crate::ProductionSourceLaunchRosterV1,
) {
    let (base, _) = pipeline_fixture(false);
    let semantic = base.source_semantic();
    let original = &semantic.functions()[0];
    let function = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        original.abi().clone(),
        original.locals().to_vec(),
        original.entry(),
        vec![block(180, vec![], SemanticTerminatorKindV1::Return)],
    )
    .unwrap()
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        vec![function],
        semantic.callables().to_vec(),
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    let ssa = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let launch = crate::ProductionSourceLaunchRosterV1::try_new(
        ssa.source_semantic(),
        &[crate::ProductionSourceLaunchRootInputV1::new(
            "native_pipeline",
            [226; 32],
            crate::ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
        )],
    )
    .unwrap();
    (ssa, launch)
}

fn materialize_pipeline(
    ssa: ProductionSemanticSsaOwnerV1,
    launch: crate::ProductionSourceLaunchRosterV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<ProductionPreRankedKirOwnerV1, ProductionPreRankedKirErrorV1> {
    // The actual production constructor, without the codegen cfg(test)/V29 pre-capture.
    ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        budget,
    )
}

#[test]
fn actual_nonhelper_pipeline_materialization_captures_original_use_site_ssa() {
    for loop_carried in [false, true] {
        let (ssa, launch) = pipeline_fixture(loop_carried);
        assert!(ssa.occurrence_storage().is_none());
        assert_eq!(ssa.source_semantic().functions().len(), 1);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let owner = materialize_pipeline(ssa, launch, &mut budget).unwrap();
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(
            owner.helper_source_policy_v1(),
            ProductionHelperSourcePolicyV1::RawEmpty
        );
        assert_eq!(owner.empty_effect_helpers().iter().count(), 0);
        let retained = owner.retained_analysis_storage_v1();
        budget.reserve_storage(retained).unwrap();
        let capture = owner.semantic_ssa().occurrence_storage().unwrap();
        assert!(capture.retained_storage() > 0);
        assert_eq!(
            owner.helper_memory.capture.transferred_storage(),
            capture.retained_storage()
        );
        let functions = owner.semantic_ssa().occurrences_v1().unwrap();
        let occurrences = functions
            .function(SemanticFunctionIdV1::from_index(0))
            .unwrap();
        assert!(std::ptr::eq(occurrences.owner(), owner.semantic_ssa()));
        let rows = occurrences
            .events()
            .iter()
            .filter(|row| {
                matches!(row.site(), Site::Terminator { block } if block.get() == 5)
                    && row.role() == Role::BaseUse
                    && row.event().variable().get() == 8
            })
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].is_reachable() && rows[0].is_promoted());
        let Some(SsaResolvedEventV1::Use { variable, value }) = rows[0].resolved() else {
            panic!("the real pipeline tile operand must keep its original SSA use");
        };
        assert_eq!(variable.get(), 8);
        assert_eq!(matches!(value, SsaValueV1::Definition(_)), !loop_carried);
        owner.semantic_ssa().verify_replay().unwrap();
        drop(owner);
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn unused_pipeline_declarations_keep_production_capture_absent() {
    let (ssa, launch) = unused_pipeline_declaration_fixture();
    assert!(ssa.source_semantic().callables().iter().any(|row| matches!(
        row,
        SemanticCallableDeclV1::CompilerIntrinsic {
            operation: SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineCreate { .. },
            ..
        }
    )));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let owner = materialize_pipeline(ssa, launch, &mut budget).unwrap();
    assert!(owner.semantic_ssa().occurrence_storage().is_none());
    assert!(matches!(
        owner.helper_memory.capture,
        HelperOccurrenceCaptureV1::Absent
    ));
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn pipeline_capture_demand_scan_pays_actual_sites_with_exact_and_one_short_work() {
    for limit in [11, 12] {
        let (ssa, _) = pipeline_fixture(false);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        work.charge_work(7).unwrap();
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let result = pre_ranked_pipeline_occurrences_required_v1(&ssa, &mut budget);
        // One function and two block/actual-call pairs before Create: 1 + 2 + 2.
        if limit == 12 {
            assert!(result.unwrap());
            assert_eq!(budget.work(), 12);
        } else {
            assert!(result.is_err());
            assert_eq!(budget.work(), 11);
        }
        assert_eq!(budget.storage(), FLOOR);
        assert!(ssa.occurrence_storage().is_none());
        drop(budget);
        assert_eq!(
            work.failed_work(),
            if limit == 12 { None } else { Some(12) }
        );
    }
    let (ssa, _) = unused_pipeline_declaration_fixture();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(2);
    let mut budget = AssertOriginBudgetV1::new(&mut work, FLOOR);
    budget.reserve_storage(FLOOR).unwrap();
    assert!(!pre_ranked_pipeline_occurrences_required_v1(&ssa, &mut budget).unwrap());
    assert_eq!((budget.work(), budget.peak_storage()), (2, FLOOR));
}

#[test]
fn pipeline_capture_transfer_is_counted_once_and_preexisting_capture_stays_caller_owned() {
    let mut measurements = Vec::new();
    for preexisting in [false, true] {
        let (mut ssa, launch) = pipeline_fixture(false);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let (inherited, capture_work) = if preexisting {
            let capture = ssa
                .try_capture_occurrences_with_budget_v1(&mut budget)
                .unwrap();
            assert_eq!(budget.storage(), FLOOR);
            budget.reserve_storage(capture.retained_storage()).unwrap();
            (capture.retained_storage(), budget.work())
        } else {
            (0, 0)
        };
        let owner = materialize_pipeline(ssa, launch, &mut budget).unwrap();
        assert_eq!(budget.storage(), FLOOR + inherited);
        let capture = owner
            .semantic_ssa()
            .occurrence_storage()
            .unwrap()
            .retained_storage();
        assert_eq!(
            owner.helper_memory.capture.transferred_storage(),
            if preexisting { 0 } else { capture }
        );
        let retained = owner.retained_analysis_storage_v1();
        budget.reserve_storage(retained).unwrap();
        assert_eq!(budget.storage(), FLOOR + inherited + retained);
        measurements.push((
            retained,
            capture,
            capture_work,
            budget.work(),
            owner.executable().canonical().canonical_bytes().to_vec(),
        ));
        drop(owner);
        budget.release_storage(retained + inherited).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
    let (fresh, existing) = (&measurements[0], &measurements[1]);
    assert_eq!(fresh.1, existing.1);
    assert_eq!(fresh.0, existing.0 + fresh.1);
    assert!(existing.2 > 0);
    assert_eq!(
        fresh.3,
        existing.3 + 5,
        "only the actual-site demand scan differs"
    );
    assert_eq!(fresh.4, existing.4, "capture does not alter emitted IR");
}

#[test]
fn actual_pipeline_capture_materialization_exact_and_one_short_resource_cuts_restore_floor() {
    let (ssa, launch) = pipeline_fixture(false);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let owner = materialize_pipeline(ssa, launch, &mut budget).unwrap();
    let required = (budget.work(), budget.peak_storage());
    assert!(owner.semantic_ssa().occurrence_storage().is_some());
    drop(owner);
    assert_eq!(budget.storage(), FLOOR);
    for (work_limit, storage_limit, succeeds) in [
        (required.0, required.1, true),
        (required.0 - 1, required.1, false),
        (required.0, required.1 - 1, false),
    ] {
        let (ssa, launch) = pipeline_fixture(false);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = AssertOriginBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR).unwrap();
        let result = materialize_pipeline(ssa, launch, &mut budget);
        assert_eq!(result.is_ok(), succeeds);
        assert_eq!(budget.storage(), FLOOR);
        if succeeds {
            let owner = result.unwrap();
            assert!(owner.semantic_ssa().occurrence_storage().is_some());
            assert_eq!((budget.work(), budget.peak_storage()), required);
        } else if work_limit < required.0 {
            drop(budget);
            assert!(work.failed_work().is_some());
        } else {
            assert!(budget.failed_storage().is_some());
        }
    }
}

#[test]
fn pipeline_preexisting_capture_missing_reservation_refuses_before_new_demand_scan() {
    let (mut ssa, launch) = pipeline_fixture(false);
    let mut capture_work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut capture_budget = AssertOriginBudgetV1::new(&mut capture_work, STORAGE);
    let capture = ssa
        .try_capture_occurrences_with_budget_v1(&mut capture_budget)
        .unwrap();
    assert!(capture.retained_storage() > FLOOR);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    assert!(materialize_pipeline(ssa, launch, &mut budget).is_err());
    assert_eq!((budget.work(), budget.storage()), (2, FLOOR));
    assert_eq!(budget.failed_storage(), None);
}
