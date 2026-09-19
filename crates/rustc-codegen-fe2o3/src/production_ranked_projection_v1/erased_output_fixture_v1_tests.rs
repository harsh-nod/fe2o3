// Constructed semantic MIR, real materialization and normal ranked projection.
// This is not collected Rust, an approved runtime helper, or a signed artifact.
fn erased_backend_materialized_fixture_v1(
    expected: bool,
    root_count: usize,
) -> (
    fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
    Vec<ProductionRankedRootInputV1>,
) {
    erased_backend_materialized_mode_v1(expected, root_count, false)
}

fn erased_backend_materialized_mode_v1(
    expected: bool,
    root_count: usize,
    load_forwarding: bool,
) -> (
    fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
    Vec<ProductionRankedRootInputV1>,
) {
    erased_backend_materialized_stores_v1(expected, root_count, load_forwarding, false)
}

fn erased_backend_materialized_stores_v1(
    expected: bool,
    root_count: usize,
    load_forwarding: bool,
    duplicate_store: bool,
) -> (
    fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
    Vec<ProductionRankedRootInputV1>,
) {
    assert!((1..=2).contains(&root_count));
    let seed = materialized_unit_local_helper_v1();
    let semantic = seed.semantic_ssa().source_semantic();
    let mut types = semantic.types().to_vec();
    let pointer = SemanticTypeIdV1::from_index(types.len() as u32);
    let carrier = SemanticTypeIdV1::from_index(types.len() as u32 + 1);
    let backend = SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(1, 8, 8),
        SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
    ));
    let properties = SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
        Some(SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap()),
        None,
    );
    types.push(
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(237)),
            SemanticLayoutIdentityV1::from_sha256(bytes(237)),
            SemanticTypeLayoutV1::new_with_backend_repr(Some(8), 8, backend, false).unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    A_U32,
                    SemanticPointerKindV1::Raw,
                    SemanticMutabilityV1::Mutable,
                    1,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        )
        .with_rustc_abi_properties(properties),
    );
    types.push(
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(238)),
            SemanticLayoutIdentityV1::from_sha256(bytes(238)),
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                Some(8),
                8,
                backend,
                false,
                SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![pointer]).unwrap()),
        )
        .with_rustc_abi_properties(properties),
    );
    let dimensions = SemanticWorkgroupDimensionsV1::new([1, 1, 1]).unwrap();
    let mut functions = Vec::new();
    let mut inputs = Vec::new();
    for ordinal in 0..root_count {
        let name = if ordinal == 0 {
            A_NAME
        } else {
            "erased_second_root"
        };
        let tag = 247 - ordinal as u8;
        let pointer_assignment = typed_assignment(
            2,
            pointer,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
                SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(1),
                    vec![
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), pointer)
                            .unwrap(),
                    ],
                    pointer,
                )
                .unwrap(),
            )),
        );
        let store = statement(SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            typed_place(3, A_U32),
            typed_constant(A_U32, 7, 4),
            SemanticVolatilityV1::NonVolatile,
            None,
        )));
        let load = typed_assignment(
            4,
            A_U32,
            SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                typed_place(3, A_U32),
                SemanticVolatilityV1::NonVolatile,
                None,
            )),
        );
        let index = typed_assignment(
            6,
            A_U64,
            SemanticRvalueKindV1::Cast {
                kind: SemanticCastKindV1::Integer,
                operand: typed_constant(A_U32, 7, 4),
            },
        );
        let predicate = typed_assignment(
            5,
            A_BOOL,
            SemanticRvalueKindV1::Binary {
                operation: if expected {
                    SemanticBinaryOpV1::NotEqual
                } else {
                    SemanticBinaryOpV1::Equal
                },
                left: typed_operand(6, A_U64),
                right: typed_constant(A_U64, 0, 8),
            },
        );
        // Keep the Load live independently of the constant divisor assertion.
        let store_loaded = statement(SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            typed_place(7, A_U32),
            typed_operand(4, A_U32),
            SemanticVolatilityV1::NonVolatile,
            None,
        )));
        let global = statement(SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(2),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, A_U32)
                        .unwrap(),
                ],
                A_U32,
            )
            .unwrap(),
            typed_constant(A_U32, 7, 4),
            SemanticVolatilityV1::NonVolatile,
            None,
        )));
        let duplicate = store.clone();
        let (mut private_statements, final_statements) = if load_forwarding {
            (
                vec![
                    store,
                    global,
                    load.clone(),
                    load,
                    store_loaded,
                    index,
                    predicate,
                ],
                vec![],
            )
        } else {
            (
                vec![store, load, store_loaded, index, predicate],
                vec![global],
            )
        };
        if duplicate_store {
            private_statements.insert(1, duplicate);
        }
        let root = assertion_root_with_access(
            vec![
                (A_UNIT, SemanticLocalRoleV1::Return),
                (carrier, SemanticLocalRoleV1::Argument(0)),
                (pointer, SemanticLocalRoleV1::Temporary),
                (A_U32, SemanticLocalRoleV1::Temporary),
                (A_U32, SemanticLocalRoleV1::Temporary),
                (A_BOOL, SemanticLocalRoleV1::Temporary),
                (A_U64, SemanticLocalRoleV1::Temporary),
                (A_U32, SemanticLocalRoleV1::Temporary),
            ],
            vec![carrier],
            vec![
                block(
                    170 + ordinal as u8 * 3,
                    vec![pointer_assignment],
                    neutral_test_call_v1(root_count as u32, vec![], 0, A_UNIT, 1),
                ),
                block(
                    171 + ordinal as u8 * 3,
                    private_statements,
                    SemanticTerminatorKindV1::Assert {
                        condition: typed_operand(5, A_BOOL),
                        expected,
                        message: SemanticAssertMessageV1::DivisionByZero(typed_operand(6, A_U64)),
                        target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, 2),
                        unwind: SemanticUnwindActionV1::Unreachable,
                    },
                ),
                block(
                    172 + ordinal as u8 * 3,
                    final_statements,
                    SemanticTerminatorKindV1::Return,
                ),
            ],
            false,
        );
        functions.push(
            SemanticFunctionDeclV1::new(
                SemanticFunctionIdentityV1::from_sha256(bytes(240 + ordinal as u8)),
                root.role(),
                root.item_definition_identity(),
                root.monomorphization_identity(),
                root.generic_type_arguments_identity(),
                root.const_generic_arguments_identity(),
                root.source(),
                root.abi()
                    .clone()
                    .with_source_argument_ownership(vec![
                        SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
                    ])
                    .unwrap(),
                root.locals().to_vec(),
                root.entry(),
                root.blocks().to_vec(),
            )
            .unwrap()
            .with_kernel_entry(SemanticKernelEntryV1::new(
                SemanticLinkSymbolV1::new(name.as_bytes().to_vec()).unwrap(),
                SemanticKernelBindingIdentityV1::from_sha256(bytes(tag)),
                SemanticKernelSourceContractV1::new(
                    Some(
                        SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None)
                            .unwrap(),
                    ),
                    None,
                    None,
                )
                .unwrap(),
            )),
        );
        inputs.push(ranked_root_input_1d(name, tag, 1));
    }
    functions.push(semantic.functions()[1].clone());
    let admitted = InertSemanticMirRequestV1::new(
        semantic.target(),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        (0..root_count)
            .map(|i| SemanticFunctionIdV1::from_index(i as u32))
            .collect(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let ssa = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(
            admitted,
            fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap(),
        fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    (materialize_ranked_fixture_v1(ssa, &inputs).unwrap(), inputs)
}

pub(crate) fn with_backend_erased_bound_v1(
    expected: bool,
    roots: usize,
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    next: impl FnOnce(
        fe2o3_lower_mir_kernel::ProductionUnitLocalErasedSourceOwnerV1,
        fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ),
) {
    with_backend_erased_roster_v1(
        expected,
        roots,
        profile,
        |source, bound, verification, budget| {
            next(source, bound, budget);
            drop(verification);
        },
    );
}

/// Transfers only the genuine normal-projector roster, which this fixture has
/// never signed. It is suitable for custody/refusal tests, not proof success.
pub(crate) fn with_backend_erased_roster_v1(
    expected: bool,
    roots: usize,
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    next: impl FnOnce(
        fe2o3_lower_mir_kernel::ProductionUnitLocalErasedSourceOwnerV1,
        fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
        AuthenticatedRankedVerificationRosterV1,
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ),
) {
    with_backend_erased_roster_mode_v1(expected, roots, profile, false, next)
}

pub(crate) fn with_backend_erased_load_roster_v1(
    expected: bool,
    roots: usize,
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    next: impl FnOnce(
        fe2o3_lower_mir_kernel::ProductionUnitLocalErasedSourceOwnerV1,
        fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
        AuthenticatedRankedVerificationRosterV1,
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ),
) {
    with_backend_erased_roster_mode_v1(expected, roots, profile, true, next)
}

fn with_backend_erased_roster_mode_v1(
    expected: bool,
    roots: usize,
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    load_forwarding: bool,
    next: impl FnOnce(
        fe2o3_lower_mir_kernel::ProductionUnitLocalErasedSourceOwnerV1,
        fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
        AuthenticatedRankedVerificationRosterV1,
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ),
) {
    with_backend_erased_store_roster_v1(expected, roots, profile, load_forwarding, false, next)
}

pub(crate) fn with_backend_erased_store_roster_v1(
    expected: bool,
    roots: usize,
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    load_forwarding: bool,
    duplicate_store: bool,
    next: impl FnOnce(
        fe2o3_lower_mir_kernel::ProductionUnitLocalErasedSourceOwnerV1,
        fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
        AuthenticatedRankedVerificationRosterV1,
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ),
) {
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as B, CanonicalKernelIrWorkBudgetV1 as W,
        VerifiedCanonicalKernelIrModuleV12 as V,
    };
    let (source, inputs) = if duplicate_store {
        erased_backend_materialized_stores_v1(expected, roots, load_forwarding, true)
    } else if load_forwarding {
        erased_backend_materialized_mode_v1(expected, roots, true)
    } else {
        erased_backend_materialized_fixture_v1(expected, roots)
    };
    let original = *source.executable().canonical().identity();
    let original_storage = source.unit_local_source_storage_floor_v1().unwrap();
    let program = project_and_verify_ranked_materialized_semantic_mir_v1(
        source,
        &inputs,
        &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
    )
    .unwrap();
    assert!(program.all_kernel_checks_are_clean());
    assert_eq!(program.root_count(), roots);
    assert!(
        program
            .roots
            .iter()
            .all(|root| !root.access_sources.is_empty())
    );
    let verified = program.into_verified_roster_receipt().unwrap();
    let mut work =
        W::new(usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT).unwrap());
    let mut budget = B::new(
        &mut work,
        crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
    );
    const PREFIX: usize = 29;
    budget.reserve_storage(PREFIX + original_storage).unwrap();
    let (source, verification, source_receipt) = verified
        .into_silent_unit_erased_source_v1(&mut budget)
        .unwrap();
    budget
        .reserve_storage(source_receipt.retained_storage())
        .unwrap();
    assert_eq!(
        *source.original_source().executable().canonical().identity(),
        original
    );
    assert_ne!(source.erased().canonical().identity(), &original);
    assert_eq!(source.deleted_call_count(), roots);
    assert_eq!(source.deleted_function_count(), 1);
    assert_eq!(source.ranked_root_count(), roots);
    let binding =
        dialect_amdgcn::bind_production_target_v1(source.erased().module(), profile).unwrap();
    let (bound, bound_receipt) =
        V::from_module_ref_with_verification_budget_v12(binding.module(), &mut budget).unwrap();
    budget
        .reserve_storage(bound_receipt.retained_storage())
        .unwrap();
    drop(binding);
    let floor = budget.storage();
    next(source, bound, verification, &mut budget);
    assert_eq!(budget.storage(), floor);
    budget
        .release_storage(
            original_storage + source_receipt.retained_storage() + bound_receipt.retained_storage(),
        )
        .unwrap();
    assert_eq!(budget.storage(), PREFIX);
}
