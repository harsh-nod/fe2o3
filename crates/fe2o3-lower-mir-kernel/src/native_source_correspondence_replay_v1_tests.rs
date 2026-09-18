// Registered beneath the existing checked-output source fixture module.
use super::*;
use crate::{NativeSourceReplayErrorV1 as E, replay_native_source_correspondence_v1 as replay};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    InertCanonicalKernelIrContractCatalogV1 as Catalog,
};

fn current_scalar_source() -> ProductionPreRankedKirOwnerV1 {
    use fe2o3_mir_model::semantic_mir_v1::{InertSemanticMirRequestV1, SemanticMirLimitsV1};
    let original = scalar_source(false, false);
    let semantic = original.semantic_ssa().source_semantic();
    let current = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
        semantic.allocations().to_vec(),
        semantic.statics().to_vec(),
        semantic.vtables().to_vec(),
        semantic.functions().to_vec(),
        semantic.callables().to_vec(),
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let row = original.source_launch().roots()[0];
    let inputs = [crate::ProductionSourceLaunchRootInputV1::new(
        "replay_root",
        row.kernel_binding(),
        row.source_launch(),
    )];
    let launch = crate::ProductionSourceLaunchRosterV1::try_new(&current, &inputs).unwrap();
    let source = fe2o3_pliron::ProductionSemanticMirOwnerV1::try_new(
        current,
        fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap();
    let ssa = fe2o3_pliron::ProductionSemanticSsaOwnerV1::try_new(
        source,
        fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
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

fn with_input(
    next: impl FnOnce(
        &ProductionPreRankedKirOwnerV1,
        &Catalog,
        &[crate::ProductionSourceLaunchRootInputV1<'_>],
        &mut AssertOriginBudgetV1<'_>,
    ),
) {
    let source = current_scalar_source();
    let semantic = source.semantic_ssa().source_semantic();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    const PREFIX: usize = 31;
    budget
        .reserve_storage(PREFIX + source.retained_analysis_storage_v1())
        .unwrap();
    let (catalog, receipt) = Catalog::from_rows_with_budget(
        *semantic.semantic_sha256().as_bytes(),
        &[],
        &[],
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let row = source.source_launch().roots()[0];
    let inputs = [crate::ProductionSourceLaunchRootInputV1::new(
        "replay_root",
        row.kernel_binding(),
        row.source_launch(),
    )];
    let floor = budget.storage();
    next(&source, &catalog, &inputs, &mut budget);
    assert_eq!(budget.storage(), floor);
}

#[test]
fn native_source_replay_explicitly_refuses_legacy_v2_semantic_input() {
    use fe2o3_mir_model::semantic_mir_v1::{SemanticMirDecodeErrorV1, SemanticMirWireVersionV1};
    let source = scalar_source(false, false);
    let semantic = source.semantic_ssa().source_semantic();
    assert_eq!(semantic.wire_version(), SemanticMirWireVersionV1::V2);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget
        .reserve_storage(source.retained_analysis_storage_v1())
        .unwrap();
    let (catalog, receipt) = Catalog::from_rows_with_budget(
        *semantic.semantic_sha256().as_bytes(),
        &[],
        &[],
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let row = source.source_launch().roots()[0];
    let inputs = [crate::ProductionSourceLaunchRootInputV1::new(
        "replay_root",
        row.kernel_binding(),
        row.source_launch(),
    )];
    let floor = budget.storage();
    assert!(matches!(
        replay(
            semantic.canonical_encoding(),
            source.executable().canonical().canonical_bytes(),
            catalog.canonical_bytes(),
            &inputs,
            &mut budget
        ),
        Err(E::Semantic(
            SemanticMirDecodeErrorV1::UnsupportedProductionWireVersion(
                SemanticMirWireVersionV1::V2
            )
        ))
    ));
    assert_eq!(budget.storage(), floor);
}

#[test]
fn native_source_replay_rebuilds_exact_normal_source_n_and_catalog() {
    with_input(|source, catalog, inputs, budget| {
        let floor = budget.storage();
        let (checked, receipt) = replay(
            source.semantic_ssa().source_semantic().canonical_encoding(),
            source.executable().canonical().canonical_bytes(),
            catalog.canonical_bytes(),
            inputs,
            budget,
        )
        .unwrap();
        assert_eq!(budget.storage(), floor);
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        assert!(!std::ptr::eq(
            checked.source().executable(),
            source.executable()
        ));
        assert_eq!(
            checked.source().executable().canonical().canonical_bytes(),
            source.executable().canonical().canonical_bytes()
        );
        assert_eq!(checked.source().source_launch(), source.source_launch());
        assert_eq!(
            checked.catalog().canonical_bytes(),
            catalog.canonical_bytes()
        );
        assert!(!checked.authenticates_launch_origin());
        assert!(!checked.grants_artifact_or_launch_authority());
        drop(checked);
        budget.release_storage(receipt.retained_storage()).unwrap();
    });
}

#[test]
fn native_source_replay_refuses_changed_n_launch_root_and_catalog_subject() {
    with_input(|source, catalog, inputs, budget| {
        let semantic = source.semantic_ssa().source_semantic().canonical_encoding();
        let native = source.executable().canonical().canonical_bytes();
        let floor = budget.storage();
        assert!(matches!(
            replay(
                semantic,
                &native[..native.len() - 1],
                catalog.canonical_bytes(),
                inputs,
                budget
            ),
            Err(E::Mismatch("complete normal-materialized N bytes"))
        ));
        assert_eq!(budget.storage(), floor);
        let row = source.source_launch().roots()[0];
        let original = row.source_launch();
        let changed_launch = [crate::ProductionSourceLaunchRootInputV1::new(
            "replay_root",
            row.kernel_binding(),
            crate::ProductionSourceLaunchInputV1::new(
                original.rank(),
                Some([32, 1, 1]),
                original.max_grid(),
            ),
        )];
        assert!(matches!(
            replay(
                semantic,
                native,
                catalog.canonical_bytes(),
                &changed_launch,
                budget
            ),
            Err(E::Launch(
                crate::ProductionSourceLaunchErrorV1::Unsupported(
                    "authenticated LaunchContract workgroup disagrees with semantic source workgroup"
                )
            ))
        ));
        let wrong_binding = [crate::ProductionSourceLaunchRootInputV1::new(
            "replay_root",
            [0xa5; 32],
            original,
        )];
        assert!(matches!(
            replay(
                semantic,
                native,
                catalog.canonical_bytes(),
                &wrong_binding,
                budget
            ),
            Err(E::Launch(_))
        ));
        let (foreign, receipt) =
            Catalog::from_rows_with_budget([0xa5; 32], &[], &[], budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        assert!(matches!(
            replay(semantic, native, foreign.canonical_bytes(), inputs, budget),
            Err(E::Mismatch("catalog semantic source"))
        ));
        drop(foreign);
        budget.release_storage(receipt.retained_storage()).unwrap();
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn native_source_replay_entry_and_partial_header_denials_restore_floor() {
    with_input(|source, catalog, inputs, _| {
        let semantic = source.semantic_ssa().source_semantic().canonical_encoding();
        let native = source.executable().canonical().canonical_bytes();
        const FLOOR: usize = 19;
        let mut work = CanonicalKernelIrWorkBudgetV1::new(3);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let error = replay(
            semantic,
            native,
            catalog.canonical_bytes(),
            inputs,
            &mut budget,
        )
        .err()
        .unwrap();
        let E::Resource(Resource::Work(error)) = error else {
            panic!("exact entry work denial");
        };
        assert_eq!((error.actual(), error.limit()), (4, 3));
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.work(), 0);

        let header = std::mem::size_of::<crate::ReplayedNativeSourceV1>() + semantic.len();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, FLOOR + header - 1);
        budget.reserve_storage(FLOOR).unwrap();
        let error = replay(
            semantic,
            native,
            catalog.canonical_bytes(),
            inputs,
            &mut budget,
        )
        .err()
        .unwrap();
        let E::Resource(Resource::Storage(error)) = error else {
            panic!("exact wrapper storage denial");
        };
        assert_eq!(
            (error.actual(), error.limit()),
            (FLOOR + header, FLOOR + header - 1)
        );
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.work(), 4);
        assert_eq!(budget.peak_storage(), FLOOR);
    });
}
