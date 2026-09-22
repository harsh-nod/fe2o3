//! Genuine CPU signature-consistency fixture; test keys are not runtime evidence.
use super::*;
use fe2o3_kernel_ir::{Constant, OperationKind, VerifiedCanonicalKernelIrModuleV12};
use fe2o3_lower_mir_kernel::{
    ProductionHelperSourcePolicyV1, ProductionRankedSemanticProjectionRootV1,
    ProductionUnitLocalErasedSourceOwnerV1,
};

fn unit_source_with_value(private_value: u32) -> ProductionPreRankedKirOwnerV1 {
    unit_source_for_stores(private_value, &STORES[..1])
}

fn unit_source_for_stores(
    private_value: u32,
    specs: &[StoreSpec],
) -> ProductionPreRankedKirOwnerV1 {
    let base = source_for_stores(specs);
    let semantic = base.semantic_ssa().source_semantic();
    let original = &semantic.functions()[0];
    let unit = SemanticTypeIdV1::from_index(0);
    let u32_ty = SemanticTypeIdV1::from_index(1);
    let provenance = SemanticSourceProvenanceV1::unavailable();
    let place =
        |local, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    let local = |tag, ty, role| {
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([tag; 32]),
            ty,
            role,
            provenance,
        )
    };
    let block = |tag, statements, terminator| {
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([tag; 32]),
            provenance,
            statements,
            SemanticTerminatorV1::new(provenance, terminator),
        )
        .unwrap()
    };
    let mut locals = original.locals().to_vec();
    locals.push(local(150, unit, SemanticLocalRoleV1::Temporary));
    let root = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        provenance,
        original.abi().clone(),
        locals,
        original.entry(),
        vec![
            block(
                151,
                original.blocks()[0].statements().to_vec(),
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new_callable(
                        SemanticCallableIdV1::from_index(semantic.functions().len() as u32),
                        vec![],
                        Some(SemanticCallDestinationV1::new(
                            place(3, unit),
                            SemanticControlFlowEdgeV1::new(
                                SemanticEdgeRoleV1::CallReturn,
                                SemanticBlockIdV1::from_index(1),
                            ),
                        )),
                        SemanticUnwindActionV1::Unreachable,
                    )
                    .unwrap(),
                ),
            ),
            block(152, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
    .unwrap()
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let helper_abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([153; 32]),
        semantic.target_layout_identity(),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let helper = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([154; 32]),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256([155; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([156; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([157; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([158; 32]),
        provenance,
        helper_abi,
        vec![
            local(159, unit, SemanticLocalRoleV1::Return),
            local(160, u32_ty, SemanticLocalRoleV1::Temporary),
            local(161, u32_ty, SemanticLocalRoleV1::Temporary),
        ],
        SemanticBlockIdV1::from_index(0),
        vec![block(
            162,
            vec![
                SemanticStatementV1::new(
                    provenance,
                    SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                        place(1, u32_ty),
                        SemanticOperandV1::Constant(SemanticConstantV1::new(
                            u32_ty,
                            SemanticConstantValueV1::Scalar(
                                SemanticScalarValueV1::new(u128::from(private_value), 4).unwrap(),
                            ),
                        )),
                        SemanticVolatilityV1::NonVolatile,
                        None,
                    )),
                ),
                SemanticStatementV1::new(
                    provenance,
                    SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                        place(2, u32_ty),
                        SemanticRvalueV1::new(
                            u32_ty,
                            SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                                place(1, u32_ty),
                                SemanticVolatilityV1::NonVolatile,
                                None,
                            )),
                        ),
                    )),
                ),
            ],
            SemanticTerminatorKindV1::Return,
        )],
    )
    .unwrap();
    let mut functions = semantic.functions().to_vec();
    functions[0] = root;
    functions.push(helper);
    let semantic = InertSemanticMirRequestV1::new(
        semantic.target(),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        functions,
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let inputs: Vec<_> = specs
        .iter()
        .map(|spec| {
            ProductionSourceLaunchRootInputV1::new(
                spec.name,
                spec.binding,
                ProductionSourceLaunchInputV1::new(1, Some([1, 1, 1]), [1, 1, 1]),
            )
        })
        .collect();
    let launch = ProductionSourceLaunchRosterV1::try_new(&semantic, &inputs).unwrap();
    let semantic =
        ProductionSemanticMirOwnerV1::try_new(semantic, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    let ssa =
        ProductionSemanticSsaOwnerV1::try_new(semantic, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let source = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap();
    assert_eq!(
        source.helper_source_policy_v1(),
        ProductionHelperSourcePolicyV1::UnitLocal
    );
    source
}

pub(super) fn unit_fixture() -> (Fixture, ProductionUnitLocalErasedSourceOwnerV1) {
    unit_fixture_for_stores(&STORES[..1])
}

pub(super) fn unit_fixture_for_stores(
    specs: &[StoreSpec],
) -> (Fixture, ProductionUnitLocalErasedSourceOwnerV1) {
    let source = unit_source_for_stores(11, specs);
    let fixture = fixture_for_stores(&source, specs);
    let toolchain = VerusToolchainIdentityV2::new(d(72), d(73), d(74), d(75), d(76)).unwrap();
    let roots: Vec<_> = specs
        .iter()
        .enumerate()
        .map(|(ordinal, &spec)| {
            let (lowering, _, _) = ranked_for_root(&source, ordinal, spec, toolchain);
            ProductionRankedSemanticProjectionRootV1::new(
                SemanticFunctionIdV1::from_index(ordinal as u32),
                1,
                lowering,
                TEXT.into(),
                vec![ProductionRankedAccessSourceV1::new(0, Some(1), 0, 0, 8)],
                vec![],
            )
        })
        .collect();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let floor = ProductionUnitLocalErasedSourceOwnerV1::input_storage_floor_v1(
        &source,
        &roots,
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(floor).unwrap();
    let (owner, _) =
        ProductionUnitLocalErasedSourceOwnerV1::try_produce_v1(source, roots, &mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    (fixture, owner)
}

fn validate_unit(
    fixture: &Fixture,
    erased: &VerifiedCanonicalKernelIrModuleV12,
    receipts: &[InertFunctionalRefinementReceiptSignatureV2],
    access: &[ProductionRankedAccessSourceV1],
    budget: &mut Budget<'_>,
) -> Result<
    (
        ValidatedNativeCompilerUnitLocalErasedSourceProofV1,
        NativeCompilerUnitLocalErasedSourceProofStorageV1,
    ),
    E,
> {
    let launch = [ProductionSourceLaunchRootInputV1::new(
        NAME,
        BINDING,
        ProductionSourceLaunchInputV1::new(1, Some([1, 1, 1]), [1, 1, 1]),
    )];
    let commitments = [fixture.roots[0].staging];
    let staging = [NativeCompilerRootStagingV1 {
        semantic_root: 0,
        commitments: &commitments,
    }];
    let roots = [NativeCompilerRankedRootV1 {
        candidate: NativeRankedSourceCandidateV1::from_untrusted_parts(
            0,
            1,
            &fixture.roots[0].kernel,
            access,
            &[],
            TEXT,
        ),
        effect_receipts: receipts,
    }];
    validate_native_compiler_unit_local_erased_source_proof_v1(
        NativeCompilerUnitLocalErasedSourceProofInputsV1 {
            original: NativeCompilerSourceProofInputsV1 {
                semantic_mir: &fixture.semantic,
                native_module: &fixture.native,
                middle_end_roster: fixture.middle.canonical_bytes(),
                correspondence_roster: fixture.correspondence.canonical_bytes(),
                verus_roster: fixture.verus.canonical_bytes(),
                launch_inputs: &launch,
                staging_roots: &staging,
            },
            ranked_roots: &roots,
            erased,
        },
        budget,
    )
}

fn effect_counts(module: &fe2o3_kernel_ir::Module) -> (usize, usize) {
    module
        .functions
        .iter()
        .filter_map(|function| function.body.as_ref())
        .flat_map(|body| &body.blocks)
        .flat_map(|block| &block.operations)
        .fold((0, 0), |(calls, stores), operation| {
            (
                calls + usize::from(matches!(operation.kind, OperationKind::Call { .. })),
                stores + usize::from(matches!(operation.kind, OperationKind::Store { .. })),
            )
        })
}

#[test]
fn typed_unit_replay_retains_original_n_and_exact_e_with_signed_global_effect() {
    let (fixture, owner) = unit_fixture();
    assert!(effect_counts(owner.original_source().executable().module()).0 > 0);
    assert!(effect_counts(owner.original_source().executable().module()).1 > 1);
    assert_eq!(effect_counts(owner.erased().module()), (0, 1));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let floor = owner.retained_storage_floor_v1() + 73;
    budget.reserve_storage(floor).unwrap();
    let (checked, storage) = validate_unit(
        &fixture,
        owner.erased(),
        &[fixture.roots[0].signature],
        &[ProductionRankedAccessSourceV1::new(0, Some(1), 0, 0, 8)],
        &mut budget,
    )
    .unwrap();
    assert_eq!(budget.storage(), floor);
    assert!(storage.retained_storage() > 0);
    assert_eq!(checked.root_count(), 1);
    assert_eq!(
        checked
            .source()
            .source()
            .original_source()
            .executable()
            .canonical()
            .canonical_bytes(),
        fixture.native_graph
    );
    assert_eq!(
        checked
            .source()
            .source()
            .erased()
            .canonical()
            .canonical_bytes(),
        owner.erased().canonical().canonical_bytes()
    );
    assert!(!checked.grants_artifact_or_launch_authority());
    assert!(!checked.authenticates_compiler_or_launch_origin());
    assert!(!checked.proves_whole_operational_or_indexed_address_equivalence());
}

pub(super) fn changed_erased_module(
    erased: &VerifiedCanonicalKernelIrModuleV12,
) -> fe2o3_kernel_ir::Module {
    let mut changed = erased.module().clone();
    let operation = changed
        .functions
        .iter_mut()
        .filter_map(|function| function.body.as_mut())
        .flat_map(|body| &mut body.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| operation.kind == OperationKind::Constant(Constant::U32(7)))
        .unwrap();
    operation.kind = OperationKind::Constant(Constant::U32(8));
    changed
}

#[test]
fn typed_unit_replay_rejects_n_as_e_and_freshly_verified_changed_e() {
    let (fixture, owner) = unit_fixture();
    let changed = changed_erased_module(owner.erased());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget
        .reserve_storage(owner.retained_storage_floor_v1())
        .unwrap();
    let (changed, storage) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            &changed,
            &mut budget,
        )
        .unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let floor = budget.storage();
    for erased in [owner.original_source().executable(), &changed] {
        assert!(
            validate_unit(
                &fixture,
                erased,
                &[fixture.roots[0].signature],
                &[ProductionRankedAccessSourceV1::new(0, Some(1), 0, 0, 8)],
                &mut budget
            )
            .is_err()
        );
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn typed_unit_replay_refuses_missing_duplicate_receipts_and_changed_source_map() {
    let (fixture, owner) = unit_fixture();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let floor = owner.retained_storage_floor_v1();
    budget.reserve_storage(floor).unwrap();
    for signatures in [
        vec![],
        vec![fixture.roots[0].signature, fixture.roots[0].signature],
    ] {
        assert!(
            validate_unit(
                &fixture,
                owner.erased(),
                &signatures,
                &[ProductionRankedAccessSourceV1::new(0, Some(1), 0, 0, 8)],
                &mut budget
            )
            .is_err()
        );
        assert_eq!(budget.storage(), floor);
    }
    assert!(
        validate_unit(
            &fixture,
            owner.erased(),
            &[fixture.roots[0].signature],
            &[ProductionRankedAccessSourceV1::new(0, Some(0), 0, 0, 8)],
            &mut budget
        )
        .is_err()
    );
    assert_eq!(budget.storage(), floor);
}

#[test]
fn typed_unit_replay_rejects_resigned_false_aggregate_and_legacy_route() {
    let (mut fixture, owner) = unit_fixture();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let floor = owner.retained_storage_floor_v1();
    budget.reserve_storage(floor).unwrap();
    let access = [ProductionRankedAccessSourceV1::new(0, Some(1), 0, 0, 8)];
    assert!(
        fixture
            .validate(&fixture.roots[0].kernel, &access, &mut budget)
            .is_err()
    );
    replace_and_resign_aggregate_claim(&mut fixture, 0);
    assert!(matches!(
        validate_unit(
            &fixture,
            owner.erased(),
            &[fixture.roots[0].signature],
            &access,
            &mut budget
        ),
        Err(E::Mismatch("fresh source/ranked aggregate subjects"))
    ));
    assert_eq!(budget.storage(), floor);
}

#[test]
fn typed_unit_replay_exact_and_one_short_work_and_peak_storage_restore_floor() {
    let (fixture, owner) = unit_fixture();
    let access = [ProductionRankedAccessSourceV1::new(0, Some(1), 0, 0, 8)];
    let signatures = [fixture.roots[0].signature];
    let floor = owner.retained_storage_floor_v1();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(floor).unwrap();
    let checked =
        validate_unit(&fixture, owner.erased(), &signatures, &access, &mut budget).unwrap();
    drop(checked);
    let exact_work = budget.work();
    let exact_storage = budget.peak_storage();
    assert_eq!(budget.storage(), floor);
    for (work_limit, storage_limit, success) in [
        (exact_work, exact_storage, true),
        (exact_work - 1, exact_storage, false),
        (exact_work, exact_storage - 1, false),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        assert_eq!(
            validate_unit(&fixture, owner.erased(), &signatures, &access, &mut budget).is_ok(),
            success
        );
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn typed_unit_replay_compares_complete_original_n_even_for_silent_helper_changes() {
    let (mut fixture, owner) = unit_fixture();
    let changed_source = unit_source_with_value(12);
    fixture.semantic = changed_source
        .semantic_ssa()
        .source_semantic()
        .canonical_encoding()
        .to_vec();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let floor = owner.retained_storage_floor_v1()
        + changed_source.unit_local_source_storage_floor_v1().unwrap();
    budget.reserve_storage(floor).unwrap();
    assert!(matches!(
        validate_unit(
            &fixture,
            owner.erased(),
            &[fixture.roots[0].signature],
            &[ProductionRankedAccessSourceV1::new(0, Some(1), 0, 0, 8)],
            &mut budget
        ),
        Err(E::Source(
            fe2o3_lower_mir_kernel::NativeSourceReplayErrorV1::Mismatch(
                "complete normal-materialized N bytes"
            )
        ))
    ));
    assert_eq!(budget.storage(), floor);
}
