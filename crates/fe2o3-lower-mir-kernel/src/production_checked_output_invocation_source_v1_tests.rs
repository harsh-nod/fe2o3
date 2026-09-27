use super::super::super::source_roles::check_source;
use super::super::super::*;
use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1, IndexKind, IntrinsicKind, OperationKind};

include!("production_checked_output_source_roles_fixtures_v1_tests.rs");

fn launch_source(kind: IndexKind, rank: u8) -> ProductionPreRankedKirOwnerV1 {
    let original = request(Fixture {
        intrinsic: true,
        ..Fixture::default()
    });
    let mut callables = original.callables().to_vec();
    let mut count = 0;
    for callable in &mut callables {
        if let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = callable {
            assert_eq!(
                *operation,
                SemanticCompilerIntrinsicOperationV1::ThreadIndex(SemanticAxisV1::X)
            );
            *operation = match kind {
                IndexKind::Local => {
                    SemanticCompilerIntrinsicOperationV1::ThreadIndex(SemanticAxisV1::X)
                }
                IndexKind::Workgroup => {
                    SemanticCompilerIntrinsicOperationV1::WorkgroupIndex(SemanticAxisV1::X)
                }
                _ => unreachable!(),
            };
            count += 1;
        }
    }
    assert_eq!(count, 1);
    let request = InertSemanticMirRequestV1::new_with_callables(
        original.target(),
        original.types().to_vec(),
        original.allocations().to_vec(),
        original.statics().to_vec(),
        original.vtables().to_vec(),
        original.functions().to_vec(),
        callables,
        original.roots().to_vec(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let semantic =
        ProductionSemanticMirOwnerV1::try_new(request, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    let ssa =
        ProductionSemanticSsaOwnerV1::try_new(semantic, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    let inputs = [crate::ProductionSourceLaunchRootInputV1::new(
        "logical_0",
        [30; 32],
        crate::ProductionSourceLaunchInputV1::new(rank, Some([64, 1, 1]), [1, 1, 1]),
    )];
    let launch =
        crate::ProductionSourceLaunchRosterV1::try_new(ssa.source_semantic(), &inputs).unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let source = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap();
    assert_eq!(budget.storage(), FLOOR);
    source
}

fn launch_receipt(kind: IndexKind) -> ProductionMaterializedRankedModuleReceiptV1 {
    launch_receipt_with_rank(kind, 1)
}

fn launch_receipt_with_rank(
    kind: IndexKind,
    rank: u8,
) -> ProductionMaterializedRankedModuleReceiptV1 {
    use fe2o3_pliron::{
        ProductionConstructionV1, ProductionRankedBlockV1, ProductionRankedKernelV1,
        ProductionRankedTerminatorV1, ProductionSessionLimitsV1,
        compile_ranked_kernel_for_lowering_v1,
    };
    let source = launch_source(kind, rank);
    assert_eq!(source.source_launch().roots().len(), 1);
    let layout = source.source_launch().roots()[0].layout();
    let name = "selected_wrapper_0";
    let kernel = ProductionRankedKernelV1::new(
        name,
        0,
        vec![ProductionRankedBlockV1::new(
            vec![ProductionRankedOperationV1::ExecutionLayout {
                grid_identity: layout.grid_identity(),
                global_extents: layout.global_extents(),
                workgroup_extents: layout.workgroup_extents(),
                subgroup_size: layout.subgroup_size(),
                full_physical_workgroups: layout.full_physical_workgroups(),
            }],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    let lowering = compile_ranked_kernel_for_lowering_v1(
        ProductionConstructionV1::ranked_kernel(name, kernel).unwrap(),
        ProductionSessionLimitsV1::default(),
    )
    .unwrap();
    assert!(lowering.all_mandatory_reports_are_clean());
    let root = ProductionRankedSemanticProjectionRootV1::new(
        SemanticFunctionIdV1::from_index(0), rank, lowering,
        "genuine original intrinsic source/N with compiled no-global-effect ranked component; not backend projection".to_owned(),
        vec![], vec![],
    );
    ProductionMaterializedRankedModuleReceiptV1::from_unvalidated_projection_roster_candidate(
        source,
        vec![root],
    )
    .unwrap()
}

fn launch_module_candidate(
    receipt: &ProductionMaterializedRankedModuleReceiptV1,
    mutation: Option<usize>,
) -> fe2o3_kernel_ir::Module {
    let mut module = receipt.materialized.executable().module().clone();
    let mut sites = 0;
    for function in &mut module.functions {
        for block in &mut function.body.as_mut().unwrap().blocks {
            for operation in &mut block.operations {
                if let OperationKind::Intrinsic(intrinsic) = &mut operation.kind {
                    if let IntrinsicKind::InvocationIndex { kind, axis } = &mut intrinsic.kind {
                        sites += 1;
                        match mutation {
                            Some(0) => {
                                *kind = if *kind == IndexKind::Local {
                                    IndexKind::Workgroup
                                } else {
                                    IndexKind::Local
                                }
                            }
                            Some(1) => *axis = fe2o3_kernel_ir::Axis::Y,
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    assert_eq!(sites, 1);
    if mutation == Some(2) {
        module.kernels[0].domain = fe2o3_kernel_ir::LaunchDomain::D1 {
            x: fe2o3_kernel_ir::LaunchExtent::Static(2),
        };
    }
    module
}

fn launch_bound(
    receipt: &ProductionMaterializedRankedModuleReceiptV1,
    profile: Profile,
    mutation: Option<usize>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12 {
    let module = launch_module_candidate(receipt, mutation);
    let binding = dialect_amdgcn::bind_production_target_v1(&module, profile).unwrap();
    let (bound, storage) = fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(binding.module(), budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    bound
}

#[test]
fn invocation_index_census_genuine_original_source_composes_all_policy_prefixes_both_targets() {
    for kind in [IndexKind::Local, IndexKind::Workgroup] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            for policy in 3..=6 {
                let receipt = launch_receipt(kind);
                let original = receipt.materialized.executable().module().clone();
                let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
                let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
                budget
                    .reserve_storage(
                        FLOOR
                            + receipt
                                .materialized
                                .unit_local_source_storage_floor_v1()
                                .unwrap(),
                    )
                    .unwrap();
                let bound = launch_bound(&receipt, profile, None, &mut budget);
                macro_rules! finish {
                    ($checked:expr, $storage:expr, $owner:ty, $admit:ident) => {{
                        let checked = $checked;
                        budget.reserve_storage(($storage)(&checked)).unwrap();
                        let floor = budget.storage();
                        let output = checked.owner().canonical().canonical_bytes().to_vec();
                        let owner = <$owner>::$admit(receipt, bound, checked, &mut budget).unwrap();
                        assert_eq!(owner.source_semantic_kir().module(), &original);
                        assert_eq!(owner.output().canonical().canonical_bytes(), output);
                        assert!(!owner.grants_artifact_or_launch_authority());
                        owner.verify_equivalence(&mut budget).unwrap();
                        assert_eq!(budget.storage(), floor);
                    }};
                }
                match policy {
                    3 => finish!(
                        fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy3_v1(
                            &bound,
                            &mut budget
                        )
                        .unwrap(),
                        |checked: &fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1| checked
                            .storage()
                            .retained_storage(),
                        crate::ProductionCheckedOutputOwnerPolicy3V1,
                        try_admit_general_v1
                    ),
                    4 => finish!(
                        fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy4_v1(
                            &bound,
                            &mut budget
                        )
                        .unwrap(),
                        |checked: &fe2o3_kernel_opt::CheckedCanonicalKernelIrOwnerPolicy4V1| {
                            checked.retained_storage()
                        },
                        crate::ProductionCheckedOutputOwnerPolicy4V1,
                        try_admit_v1
                    ),
                    5 => finish!(
                        fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy5_v1(
                            &bound,
                            &mut budget
                        )
                        .unwrap(),
                        |checked: &fe2o3_kernel_opt::CheckedCanonicalKernelIrOwnerPolicy5V1| {
                            checked.retained_storage()
                        },
                        crate::ProductionCheckedOutputOwnerPolicy5V1,
                        try_admit_v1
                    ),
                    6 => {
                        let prefix =
                            fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy5_v1(
                                &bound,
                                &mut budget,
                            )
                            .unwrap();
                        budget.reserve_storage(prefix.retained_storage()).unwrap();
                        finish!(
                            fe2o3_kernel_opt::continue_checked_canonical_kernel_ir_policy6_v1(
                                &bound,
                                prefix,
                                &mut budget
                            )
                            .unwrap(),
                            |checked: &fe2o3_kernel_opt::CheckedCanonicalKernelIrOwnerPolicy6V1| {
                                checked.retained_storage()
                            },
                            crate::ProductionCheckedOutputOwnerPolicy6V1,
                            try_admit_v1
                        );
                    }
                    _ => unreachable!(),
                }
            }
        }
    }
}

#[test]
fn invocation_index_census_one_dimensional_y_is_rejected_before_later_censuses() {
    for kind in [IndexKind::Local, IndexKind::Workgroup] {
        let receipt = launch_receipt(kind);
        let original = launch_module_candidate(&receipt, None);
        let changed = launch_module_candidate(&receipt, Some(1));
        assert!(matches!(
            original.kernels[0].domain,
            fe2o3_kernel_ir::LaunchDomain::D1 { .. }
        ));
        let check_diagnostic = |errors: fe2o3_kernel_ir::VerificationErrors| {
            let diagnostics = errors.diagnostics();
            assert_eq!(diagnostics.len(), 1, "{errors:?}");
            let diagnostic = &diagnostics[0];
            assert_eq!(
                diagnostic.code,
                fe2o3_kernel_ir::DiagnosticCode::InvalidLaunchDomain
            );
            assert_eq!(diagnostic.location.module, original.id);
            assert_eq!(
                diagnostic.location.function,
                Some("selected_wrapper_0".into())
            );
            assert_eq!(
                diagnostic.location.kernel,
                Some("selected_wrapper_0".into())
            );
            assert_eq!(diagnostic.location.block, Some(fe2o3_kernel_ir::BlockId(0)));
            assert_eq!(diagnostic.location.operation, Some(1));
            assert_eq!(
                diagnostic.message,
                "axis Y is outside the 1D launch domain of kernel selected_wrapper_0"
            );
        };
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            dialect_amdgcn::bind_production_target_v1(&original, profile).unwrap();
            let error = dialect_amdgcn::bind_production_target_v1(&changed, profile).unwrap_err();
            let dialect_amdgcn::ProductionTargetBindingErrorV1::InvalidTargetBoundModule(errors) =
                error
            else {
                panic!("expected exact launch-domain target refusal, got {error:?}");
            };
            check_diagnostic(errors);
        }
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let (owner, storage) = fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(&original, &mut budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        drop(owner);
        budget.release_storage(storage.retained_storage()).unwrap();
        assert_eq!(budget.storage(), FLOOR);
        let error = fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(&changed, &mut budget).unwrap_err();
        let fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV12::Verification(errors) = error
        else {
            panic!("expected exact launch-domain canonical refusal, got {error:?}");
        };
        check_diagnostic(errors);
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn invocation_index_census_original_target_binding_rejects_kind_axis_and_domain_substitutions() {
    for kind in [IndexKind::Local, IndexKind::Workgroup] {
        for mutation in 0..3 {
            // Keep the Y mutant well-formed so this tests N/B identity, not
            // the earlier canonical launch-domain verifier.
            let receipt = launch_receipt_with_rank(kind, 2);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
            budget
                .reserve_storage(
                    FLOOR
                        + receipt
                            .materialized
                            .unit_local_source_storage_floor_v1()
                            .unwrap(),
                )
                .unwrap();
            let bound = launch_bound(&receipt, Profile::Gfx942, Some(mutation), &mut budget);
            let checked = fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy3_v1(
                &bound,
                &mut budget,
            )
            .unwrap();
            budget
                .reserve_storage(checked.storage().retained_storage())
                .unwrap();
            let floor = budget.storage();
            let error = crate::ProductionCheckedOutputOwnerPolicy3V1::try_admit_general_v1(
                receipt,
                bound,
                checked,
                &mut budget,
            )
            .unwrap_err();
            assert!(
                matches!(error, E::Coordinates(_)),
                "{kind:?}/{mutation}: {error:?}"
            );
            assert_eq!(budget.storage(), floor);
        }
    }
}

fn launch_admit_with_limits(
    kind: IndexKind,
    work_limit: usize,
    storage_limit: usize,
) -> (
    R<crate::ProductionCheckedOutputOwnerPolicy3V1>,
    usize,
    usize,
    usize,
) {
    let receipt = launch_receipt(kind);
    let mut preparation_work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut preparation = AssertOriginBudgetV1::new(&mut preparation_work, STORAGE);
    preparation
        .reserve_storage(
            FLOOR
                + receipt
                    .materialized
                    .unit_local_source_storage_floor_v1()
                    .unwrap(),
        )
        .unwrap();
    let bound = launch_bound(&receipt, Profile::Gfx942, None, &mut preparation);
    let checked =
        fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy3_v1(&bound, &mut preparation)
            .unwrap();
    preparation
        .reserve_storage(checked.storage().retained_storage())
        .unwrap();
    let floor = preparation.storage();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = AssertOriginBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(floor).unwrap();
    let result = crate::ProductionCheckedOutputOwnerPolicy3V1::try_admit_general_v1(
        receipt,
        bound,
        checked,
        &mut budget,
    );
    assert_eq!(budget.storage(), floor);
    (result, budget.work(), budget.peak_storage(), floor)
}

#[test]
fn invocation_index_census_full_source_exact_and_one_short_admission_keeps_caller_floor() {
    for kind in [IndexKind::Local, IndexKind::Workgroup] {
        let (owner, work, peak, floor) = launch_admit_with_limits(kind, WORK, STORAGE);
        drop(owner.unwrap());
        assert!(peak > floor && work > 0);
        for (work_limit, storage_limit, success) in [
            (work, peak, true),
            (work - 1, peak, false),
            (work, peak - 1, false),
        ] {
            let (result, accepted, actual_peak, same_floor) =
                launch_admit_with_limits(kind, work_limit, storage_limit);
            assert_eq!(same_floor, floor);
            assert_eq!(result.is_ok(), success, "{kind:?}: {result:?}");
            assert!(accepted <= work_limit && actual_peak <= storage_limit);
        }
    }
}

#[test]
fn invocation_index_census_checked_transition_rejects_changed_launch_payload() {
    for kind in [IndexKind::Local, IndexKind::Workgroup] {
        // Both axes are structurally legal; the transition must still retain
        // the exact X payload from its authenticated input.
        let receipt = launch_receipt_with_rank(kind, 2);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget
            .reserve_storage(
                FLOOR
                    + receipt
                        .materialized
                        .unit_local_source_storage_floor_v1()
                        .unwrap(),
            )
            .unwrap();
        let bound = launch_bound(&receipt, Profile::Gfx942, None, &mut budget);
        let checked =
            fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy3_v1(&bound, &mut budget)
                .unwrap();
        budget
            .reserve_storage(checked.storage().retained_storage())
            .unwrap();
        let (input, storage) = CanonicalKirInventoryV1::derive(&bound, &mut budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        for fault in 0..3 {
            let mut output = checked.owner().module().clone();
            let mut changed = 0;
            for function in &mut output.functions {
                for block in &mut function.body.as_mut().unwrap().blocks {
                    for operation in &mut block.operations {
                        if let OperationKind::Intrinsic(intrinsic) = &mut operation.kind {
                            if let IntrinsicKind::InvocationIndex { kind: actual, axis } =
                                &mut intrinsic.kind
                            {
                                assert_eq!(*actual, kind);
                                match fault {
                                    0 => {}
                                    1 => {
                                        *actual = if kind == IndexKind::Local {
                                            IndexKind::Workgroup
                                        } else {
                                            IndexKind::Local
                                        }
                                    }
                                    2 => *axis = fe2o3_kernel_ir::Axis::Y,
                                    _ => unreachable!(),
                                }
                                changed += 1;
                            }
                        }
                    }
                }
            }
            assert_eq!(changed, 1);
            let (owner, storage) = fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(&output, &mut budget).unwrap();
            budget.reserve_storage(storage.retained_storage()).unwrap();
            let (output, storage) = CanonicalKirInventoryV1::derive(&owner, &mut budget).unwrap();
            budget.reserve_storage(storage.retained_storage()).unwrap();
            let floor = budget.storage();
            let result = fe2o3_kernel_analysis::check_canonical_kir_transition_v1(
                &input,
                &output,
                checked.occurrences().candidate(),
                &mut budget,
            );
            if fault == 0 {
                let (view, storage) = result.unwrap();
                budget.reserve_storage(storage.retained_storage()).unwrap();
                assert!(!view.grants_authority());
                drop(view);
                budget.release_storage(storage.retained_storage()).unwrap();
            } else {
                assert!(
                    matches!(
                        result,
                        Err(fe2o3_kernel_analysis::CanonicalKirTransitionErrorV1::Rule(
                            _
                        ))
                    ),
                    "{kind:?}/{fault}: {result:?}"
                );
            }
            assert_eq!(budget.storage(), floor);
        }
    }
}
