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
    with_backend_checked_output_policy3_roster_v1(profile, |owner, _, budget| next(owner, budget));
}

pub(crate) fn with_backend_checked_output_policy3_roster_v1(
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    next: impl FnOnce(
        fe2o3_lower_mir_kernel::ProductionCheckedOutputOwnerPolicy3V1,
        AuthenticatedRankedVerificationRosterV1,
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ),
) {
    with_backend_checked_ranked_bound_v1(profile, |receipt, bound, verification, budget| {
        let checked =
            fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy3_v1(&bound, budget)
                .unwrap();
        assert_eq!(checked.report().passes().len(), 8);
        let storage = checked.storage().retained_storage();
        budget.reserve_storage(storage).unwrap();
        let floor = budget.storage();
        let admitted =
            fe2o3_lower_mir_kernel::ProductionCheckedOutputOwnerPolicy3V1::try_admit_general_v1(
                receipt, bound, checked, budget,
            )
            .unwrap();
        next(admitted, verification, budget);
        assert_eq!(budget.storage(), floor);
        budget.release_storage(storage).unwrap();
    });
}

pub(crate) fn with_backend_checked_output_policy4_owned_v1(
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    next: impl FnOnce(
        fe2o3_lower_mir_kernel::ProductionCheckedOutputOwnerPolicy4V1,
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ),
) {
    with_backend_checked_ranked_bound_v1(profile, |receipt, bound, _, budget| {
        let checked =
            fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy4_v1(&bound, budget)
                .unwrap();
        let storage = checked.retained_storage();
        budget.reserve_storage(storage).unwrap();
        let floor = budget.storage();
        let admitted = fe2o3_lower_mir_kernel::ProductionCheckedOutputOwnerPolicy4V1::try_admit_v1(
            receipt, bound, checked, budget,
        )
        .unwrap();
        next(admitted, budget);
        assert_eq!(budget.storage(), floor);
        budget.release_storage(storage).unwrap();
    });
}

pub(crate) fn with_backend_nominal_policy4_owned_v3(
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    kind: SemanticRustTypeKindV1,
    signed: bool,
    next: impl FnOnce(
        fe2o3_lower_mir_kernel::ProductionCheckedOutputOwnerPolicy4V1,
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ),
) {
    let case = match (kind, signed) {
        (SemanticRustTypeKindV1::Usize, false) => 0u8,
        (SemanticRustTypeKindV1::Isize, true) => 1,
        (SemanticRustTypeKindV1::Ordinary, false) => 2,
        (SemanticRustTypeKindV1::Ordinary, true) => 3,
        // fe2o3-hygiene: allow-panic - this include is inside the cfg(test) module.
        _ => panic!("inconsistent nominal scalar fixture"),
    };
    with_backend_checked_ranked_bound_types_functions_v1(
        profile,
        None,
        |types, functions| {
            let unit = SemanticTypeIdV1::from_index(0);
            let scalar = SemanticTypeIdV1::from_index(1);
            types[1] = SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256(bytes(230 + case)),
                SemanticLayoutIdentityV1::from_sha256(bytes(240 + case)),
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(8),
                    8,
                    neutral_scalar_backend_v1(
                        SemanticBackendPrimitiveV1::integer(signed, 64, 8),
                        u64::MAX.into(),
                    ),
                    false,
                )
                .unwrap(),
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed, bits: 64 }),
            )
            .with_rust_type_kind(kind)
            .with_rustc_abi_properties(
                SemanticTypeAbiPropertiesV1::new(false, false).with_rustc_layout_is_noundef(true),
            );
            for (i, source) in functions.iter_mut().enumerate() {
                let stamp = 40 + 2 * case + i as u8;
                let abi = SemanticFunctionAbiV1::from_rustc(
                    SemanticAbiIdentityV1::from_sha256(bytes(stamp)),
                    SemanticLayoutIdentityV1::from_sha256(bytes(stamp + 16)),
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
                *source = SemanticFunctionDeclV1::new(
                    source.identity(),
                    source.role(),
                    source.item_definition_identity(),
                    source.monomorphization_identity(),
                    source.generic_type_arguments_identity(),
                    source.const_generic_arguments_identity(),
                    source.source(),
                    abi,
                    source.locals().to_vec(),
                    source.entry(),
                    vec![block(
                        90 + i as u8,
                        vec![typed_assignment(
                            2,
                            scalar,
                            SemanticRvalueKindV1::Use(typed_constant(scalar, 7 + i as u128, 8)),
                        )],
                        SemanticTerminatorKindV1::Return,
                    )],
                )
                .unwrap()
                .with_kernel_entry(source.kernel_entry().unwrap().clone());
            }
        },
        |receipt, bound, _, budget| {
            let checked =
                fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy4_v1(&bound, budget)
                    .unwrap();
            let storage = checked.retained_storage();
            budget.reserve_storage(storage).unwrap();
            let floor = budget.storage();
            let owner =
                fe2o3_lower_mir_kernel::ProductionCheckedOutputOwnerPolicy4V1::try_admit_v1(
                    receipt, bound, checked, budget,
                )
                .unwrap();
            next(owner, budget);
            assert_eq!(budget.storage(), floor);
            budget.release_storage(storage).unwrap();
        },
    );
}

pub(crate) fn with_backend_checked_output_policy4_v1(
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    next: impl FnOnce(
        &fe2o3_lower_mir_kernel::ProductionCheckedOutputOwnerPolicy4V1,
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ),
) {
    with_backend_checked_output_policy4_owned_v1(profile, |owner, budget| next(&owner, budget));
}

fn with_backend_checked_ranked_bound_v1(
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    next: impl FnOnce(
        fe2o3_lower_mir_kernel::ProductionMaterializedRankedModuleReceiptV1,
        fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
        AuthenticatedRankedVerificationRosterV1,
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ),
) {
    with_backend_checked_ranked_bound_stores_v1(profile, None, next)
}

fn with_backend_checked_ranked_bound_stores_v1(
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    stores: Option<usize>,
    next: impl FnOnce(
        fe2o3_lower_mir_kernel::ProductionMaterializedRankedModuleReceiptV1,
        fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
        AuthenticatedRankedVerificationRosterV1,
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ),
) {
    with_backend_checked_ranked_bound_functions_v1(profile, stores, |_| {}, next)
}

fn with_backend_checked_ranked_bound_functions_v1(
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    stores: Option<usize>,
    transform: impl FnOnce(&mut Vec<SemanticFunctionDeclV1>),
    next: impl FnOnce(
        fe2o3_lower_mir_kernel::ProductionMaterializedRankedModuleReceiptV1,
        fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
        AuthenticatedRankedVerificationRosterV1,
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ),
) {
    with_backend_checked_ranked_bound_types_functions_v1(
        profile,
        stores,
        |_, functions| transform(functions),
        next,
    )
}

fn with_backend_checked_ranked_bound_types_functions_v1(
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    stores: Option<usize>,
    transform: impl FnOnce(&mut Vec<SemanticTypeDeclV1>, &mut Vec<SemanticFunctionDeclV1>),
    next: impl FnOnce(
        fe2o3_lower_mir_kernel::ProductionMaterializedRankedModuleReceiptV1,
        fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
        AuthenticatedRankedVerificationRosterV1,
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ),
) {
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    let mut work = Work::new(
        usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT).unwrap(),
    );
    let mut budget = Budget::new(
        &mut work,
        crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
    );
    let (receipt, bound, verification, retained) =
        prepare_backend_checked_ranked_bound_types_functions_v1(
            profile,
            stores,
            transform,
            &mut budget,
        );
    let floor = budget.storage();
    next(receipt, bound, verification, &mut budget);
    assert_eq!(budget.storage(), floor);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), 29);
}

fn prepare_backend_checked_ranked_bound_types_functions_v1(
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    stores: Option<usize>,
    transform: impl FnOnce(&mut Vec<SemanticTypeDeclV1>, &mut Vec<SemanticFunctionDeclV1>),
    budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> (
    fe2o3_lower_mir_kernel::ProductionMaterializedRankedModuleReceiptV1,
    fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    AuthenticatedRankedVerificationRosterV1,
    usize,
) {
    use fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12;
    assert_eq!(budget.storage(), 0);
    let program = backend_checked_ranked_source_program_v1(stores, transform);
    let retained = program.materialized.retained_analysis_storage_v1();
    let binding = dialect_amdgcn::bind_production_target_v1(
        program.materialized.executable().module(),
        profile,
    )
    .unwrap();
    const PREFIX: usize = 29;
    budget.reserve_storage(PREFIX + retained).unwrap();
    let (bound, bound_storage) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            binding.module(),
            budget,
        )
        .unwrap();
    budget
        .reserve_storage(bound_storage.retained_storage())
        .unwrap();
    drop(binding);
    let verified = program.into_verified_roster_receipt().unwrap();
    let (receipt, verification) = verified.into_module_verified_receipt().unwrap();
    (
        receipt,
        bound,
        verification,
        bound_storage.retained_storage() + retained,
    )
}

fn backend_checked_ranked_source_program_v1(
    stores: Option<usize>,
    transform: impl FnOnce(&mut Vec<SemanticTypeDeclV1>, &mut Vec<SemanticFunctionDeclV1>),
) -> ProductionRankedSemanticProgramV1 {
    let original = source_launch_test_semantic_v1(70, 0xa1);
    let unit = SemanticTypeIdV1::from_index(0);
    let scalar = SemanticTypeIdV1::from_index(1);
    let mut types = vec![
        original.types()[0].clone(),
        neutral_semantic_types_v1()[3].clone(),
    ];
    let mut functions = original
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
                    if let Some(count) = stores {
                        assert!((1..=2).contains(&count));
                        (0..count)
                            .map(|_| {
                                statement(SemanticStatementKindV1::Store(
                                    SemanticMemoryStoreV1::new(
                                        typed_place(2, scalar),
                                        typed_constant(scalar, 7 + i as u128, 4),
                                        SemanticVolatilityV1::NonVolatile,
                                        None,
                                    ),
                                ))
                            })
                            .collect()
                    } else {
                        vec![typed_assignment(
                            2,
                            scalar,
                            SemanticRvalueKindV1::Use(typed_constant(scalar, 7 + i as u128, 4)),
                        )]
                    },
                    SemanticTerminatorKindV1::Return,
                )],
            )
            .unwrap()
            .with_kernel_entry(source.kernel_entry().unwrap().clone())
        })
        .collect();
    transform(&mut types, &mut functions);
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
    program
}

#[test]
fn canonical_assertion_multi_entry_source_projection_precedes_target_binding() {
    for looped in [false, true] {
        let program = backend_checked_ranked_source_program_v1(None, |types, functions| {
            if looped {
                let boolean = SemanticTypeIdV1::from_index(types.len() as u32);
                let wide = SemanticTypeIdV1::from_index(types.len() as u32 + 1);
                types.push(native_licm_scalar_type_v1(239, true));
                types.push(native_licm_scalar_type_v1(240, false));
                native_licm_source_v1(functions, boolean, wide, false);
            }
        });
        // The genuine materialized-source/canonical assertion path ran, but
        // no B, optimizer, native finalizer or signed proof was fabricated.
        assert!(!program.has_conditional_roots_v1());
        assert!(program.all_kernel_checks_are_clean());
        assert_eq!(program.root_count(), 2);
    }
}
