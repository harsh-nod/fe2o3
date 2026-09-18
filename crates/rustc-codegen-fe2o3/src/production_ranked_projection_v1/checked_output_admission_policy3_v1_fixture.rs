// Real production roster authentication for exact-output consumer tests.
pub(crate) fn with_backend_checked_output_policy3_v1(
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    next: impl FnOnce(
        &fe2o3_lower_mir_kernel::ProductionCheckedOutputOwnerPolicy3V1,
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ),
) {
    with_backend_checked_output_policy3_owned_v1(profile, |owner, budget| next(&owner, budget));
}

pub(crate) fn with_backend_checked_output_policy3_owned_v1(
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    next: impl FnOnce(
        fe2o3_lower_mir_kernel::ProductionCheckedOutputOwnerPolicy3V1,
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ),
) {
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work, VerifiedCanonicalKernelIrModuleV12,
    };
    let original = source_launch_test_semantic_v1(70, 0xa1);
    let unit = SemanticTypeIdV1::from_index(0);
    let scalar = SemanticTypeIdV1::from_index(1);
    let types = vec![
        original.types()[0].clone(),
        neutral_semantic_types_v1()[3].clone(),
    ];
    let functions = original
        .functions()
        .iter()
        .enumerate()
        .map(|(i, source)| {
            let abi = SemanticFunctionAbiV1::from_rustc(
                SemanticAbiIdentityV1::from_sha256(bytes(100 + i as u8)),
                SemanticLayoutIdentityV1::from_sha256(bytes(250)),
                SemanticCanonAbiV1::GpuKernel,
                SemanticExternAbiV1::GpuKernel,
                false,
                false,
                1,
                vec![SemanticAbiArgumentV1::source(
                    neutral_plain_direct_abi_value_v1(scalar),
                )],
                SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
            )
            .unwrap()
            .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue])
            .unwrap();
            SemanticFunctionDeclV1::new(
                source.identity(),
                source.role(),
                source.item_definition_identity(),
                source.monomorphization_identity(),
                source.generic_type_arguments_identity(),
                source.const_generic_arguments_identity(),
                source.source(),
                abi,
                vec![
                    local(160 + i as u8, unit, SemanticLocalRoleV1::Return),
                    local(170 + i as u8, scalar, SemanticLocalRoleV1::Argument(0)),
                    local(180 + i as u8, scalar, SemanticLocalRoleV1::Temporary),
                ],
                SemanticBlockIdV1::from_index(0),
                vec![block(
                    90 + i as u8,
                    vec![typed_assignment(
                        2,
                        scalar,
                        SemanticRvalueKindV1::Use(typed_constant(scalar, 7 + i as u128, 4)),
                    )],
                    SemanticTerminatorKindV1::Return,
                )],
            )
            .unwrap()
            .with_kernel_entry(source.kernel_entry().unwrap().clone())
        })
        .collect();
    let semantic = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(250))),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        original.roots().to_vec(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let semantic = ProductionSemanticMirOwnerV1::try_new(
        semantic,
        fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap();
    let ssa = ProductionSemanticSsaOwnerV1::try_new(
        semantic,
        fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let inputs = source_launch_test_inputs_v1();
    let materialized = materialize_ranked_fixture_v1(ssa, &inputs).unwrap();
    let program = project_and_verify_ranked_materialized_semantic_mir_v1(
        materialized,
        &inputs,
        &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
    )
    .unwrap();
    assert!(program.all_kernel_checks_are_clean());
    assert_eq!(program.root_count(), 2);
    let retained = program.materialized.retained_analysis_storage_v1();
    let binding = dialect_amdgcn::bind_production_target_v1(
        program.materialized.executable().module(),
        profile,
    )
    .unwrap();
    let mut work = Work::new(
        usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT).unwrap(),
    );
    let mut budget = Budget::new(
        &mut work,
        crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
    );
    const PREFIX: usize = 29;
    budget.reserve_storage(PREFIX + retained).unwrap();
    let (bound, bound_storage) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            binding.module(),
            &mut budget,
        )
        .unwrap();
    budget
        .reserve_storage(bound_storage.retained_storage())
        .unwrap();
    drop(binding);
    let checked =
        fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy3_v1(&bound, &mut budget)
            .unwrap();
    assert_eq!(checked.report().passes().len(), 8);
    let checked_storage = checked.storage().retained_storage();
    budget.reserve_storage(checked_storage).unwrap();
    let verified = program.into_verified_roster_receipt().unwrap();
    let (receipt, _verification) = verified.into_module_verified_receipt().unwrap();
    let floor = budget.storage();
    let admitted =
        fe2o3_lower_mir_kernel::ProductionCheckedOutputOwnerPolicy3V1::try_admit_general_v1(
            receipt,
            bound,
            checked,
            &mut budget,
        )
        .unwrap();
    next(admitted, &mut budget);
    assert_eq!(budget.storage(), floor);
    budget
        .release_storage(checked_storage + bound_storage.retained_storage() + retained)
        .unwrap();
    assert_eq!(budget.storage(), PREFIX);
}
