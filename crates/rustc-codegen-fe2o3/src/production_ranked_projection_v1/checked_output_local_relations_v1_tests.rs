mod checked_output_local_relations_tests {
    use super::*;
    use crate::production_pipeline::ProductionPipelineError as Pipeline;
    use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrVerificationResourceErrorV1 as Resource,
        CanonicalKernelIrWorkBudgetV1 as Work, InertCanonicalKernelIrContractCatalogV1 as Catalog,
        VerifiedCanonicalKernelIrModuleV12 as Owner,
    };
    use fe2o3_lower_mir_kernel::{
        ProductionBorrowedRankedCorrespondenceV1 as R1, ProductionPreRankedKirOwnerV1 as Source,
        ProductionScopedCompleteFormalMemoryV1 as Complete,
    };
    use fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1 as Checked;

    const WORK: usize = 1_000_000_000_000;
    const STORAGE: usize = 1_000_000_000;
    const PREFIX: usize = 97;

    // Fresh admission keeps the established exact type catalog/target and adds
    // a normal U32 formal plus a retained eight-element private array. The
    // unused original locals keep that catalog's reachable type closure exact.
    fn source_ssa(
        count: usize,
    ) -> (
        ProductionSemanticSsaOwnerV1,
        Vec<ProductionRankedRootInputV1>,
    ) {
        assert!((1..=2).contains(&count));
        let seed = neutral_ranked_source_for_operation_v1(
            SemanticCompilerIntrinsicOperationV1::NeutralWorkgroupReduceSum {
                context: NEUTRAL_CONTEXT_TYPE,
                dynamic_lds: NEUTRAL_DYNAMIC_LDS_TYPE,
                element_storage: NEUTRAL_ELEMENT_TYPE,
                element: NEUTRAL_ELEMENT_TYPE,
            },
            64,
        );
        let semantic = seed.source_semantic();
        let original = &semantic.functions()[0];
        assert_eq!(original.locals().len(), 7);
        assert!(original.abi().source_input_types().is_empty());
        let mut types = semantic.types().to_vec();
        let array = SemanticTypeIdV1::from_index(u32::try_from(types.len()).unwrap());
        // Preserve positional type IDs; the seed's identity range ends at 228.
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(229)),
            SemanticLayoutIdentityV1::from_sha256(bytes(181)),
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                32,
                4,
                SemanticFieldsShapeV1::array(4, 8),
                SemanticRustcVariantsV1::Single { index: 0 },
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                4,
                0,
                SemanticTypeLayoutDetailsV1::None,
            )
            .unwrap(),
            SemanticTypeShapeV1::Array {
                element: NEUTRAL_ELEMENT_TYPE,
                length: 8,
            },
        ));
        let scalar = |bits| {
            SemanticOperandV1::Constant(SemanticConstantV1::new(
                NEUTRAL_ELEMENT_TYPE,
                SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(bits, 4).unwrap()),
            ))
        };
        let abi = original.abi();
        let abi = SemanticFunctionAbiV1::from_rustc(
            abi.identity(),
            abi.layout_identity(),
            abi.canon_abi(),
            abi.extern_abi(),
            abi.can_unwind(),
            abi.c_variadic(),
            1,
            vec![SemanticAbiArgumentV1::source(
                neutral_plain_direct_abi_value_v1(NEUTRAL_ELEMENT_TYPE),
            )],
            abi.return_value().clone(),
        )
        .unwrap()
        .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue])
        .unwrap();
        let mut locals = original.locals().to_vec();
        // The seven seed locals have identities 235 through 241.
        locals.push(local(
            242,
            NEUTRAL_ELEMENT_TYPE,
            SemanticLocalRoleV1::Argument(0),
        ));
        locals.push(local(243, array, SemanticLocalRoleV1::Temporary));
        let statements = vec![
            statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                neutral_test_place_v1(6, NEUTRAL_ELEMENT_TYPE),
                SemanticRvalueV1::new(NEUTRAL_ELEMENT_TYPE, SemanticRvalueKindV1::Use(scalar(0))),
            ))),
            statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(8),
                    vec![
                        SemanticProjectionV1::new(
                            SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(6)),
                            NEUTRAL_ELEMENT_TYPE,
                        )
                        .unwrap(),
                    ],
                    NEUTRAL_ELEMENT_TYPE,
                )
                .unwrap(),
                SemanticRvalueV1::new(NEUTRAL_ELEMENT_TYPE, SemanticRvalueKindV1::Use(scalar(7))),
            ))),
        ];
        let blocks = vec![
            block(
                184,
                vec![],
                SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
            ),
            block(185, statements, SemanticTerminatorKindV1::Return),
        ];
        let names = ["stage_a_z", "stage_a_a"];
        let mut functions = Vec::new();
        let mut inputs = Vec::new();
        for (ordinal, name) in names.iter().take(count).enumerate() {
            let tag = u8::try_from(190 + ordinal * 4).unwrap();
            functions.push(
                SemanticFunctionDeclV1::new(
                    SemanticFunctionIdentityV1::from_sha256(bytes(tag)),
                    SemanticFunctionRoleV1::KernelRoot,
                    SemanticItemDefinitionIdentityV1::from_sha256(bytes(tag + 1)),
                    SemanticMonomorphizationIdentityV1::from_sha256(bytes(tag + 2)),
                    original.generic_type_arguments_identity(),
                    original.const_generic_arguments_identity(),
                    original.source(),
                    abi.clone(),
                    locals.clone(),
                    SemanticBlockIdV1::from_index(0),
                    blocks.clone(),
                )
                .unwrap()
                .with_kernel_entry(SemanticKernelEntryV1::new(
                    SemanticLinkSymbolV1::new(name.as_bytes().to_vec()).unwrap(),
                    SemanticKernelBindingIdentityV1::from_sha256(bytes(tag + 3)),
                    original.kernel_entry().unwrap().source_contract(),
                )),
            );
            inputs.push(ranked_root_input_1d(name, tag + 3, 64));
        }
        let roots = (0..count)
            .map(|n| SemanticFunctionIdV1::from_index(u32::try_from(n).unwrap()))
            .collect::<Vec<_>>();
        let callables = roots
            .iter()
            .copied()
            .map(SemanticCallableDeclV1::defined)
            .collect();
        let admitted = InertSemanticMirRequestV1::new_with_callables(
            semantic.target(),
            types,
            vec![],
            vec![],
            vec![],
            functions,
            callables,
            roots,
        )
        .unwrap()
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
        let owner = ProductionSemanticMirOwnerV1::try_new(
            admitted,
            fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap();
        (
            ProductionSemanticSsaOwnerV1::try_new(
                owner,
                fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
            )
            .unwrap(),
            inputs,
        )
    }

    fn source_with_budget(
        count: usize,
        budget: &mut Budget<'_>,
    ) -> (Source, Vec<ProductionRankedRootInputV1>, usize) {
        let (mut ssa, inputs) = source_ssa(count);
        let capture = ssa.try_capture_occurrences_with_budget_v1(budget).unwrap();
        budget.reserve_storage(capture.retained_storage()).unwrap();
        let launch = source_launch_roster_for_ranked_inputs_v1(&ssa, &inputs).unwrap();
        let source = Source::try_materialize_with_budget(
            ssa,
            launch,
            fe2o3_lower_mir_kernel::ProductionSemanticKirLimitsV1::default(),
            budget,
        )
        .unwrap();
        let owner = source.executable_storage().retained_storage()
            + source.assert_origin_storage().payload_storage();
        budget.reserve_storage(owner).unwrap();
        let storage = owner + capture.retained_storage();
        (source, inputs, storage)
    }

    fn with_fixture(
        profile: Profile,
        count: usize,
        body: impl FnOnce(
            &R1<'_>,
            &Owner,
            &Checked,
            &Catalog,
            &[Complete<'_, '_, '_, '_>],
            &[crate::compiler_descriptor::TypedDescriptorRootV1],
            &mut Budget<'_>,
        ),
    ) {
        with_fixture_expected_tail(profile, count, false, body)
    }

    fn with_fixture_expected_tail(
        profile: Profile,
        count: usize,
        exhausted: bool,
        body: impl FnOnce(
            &R1<'_>,
            &Owner,
            &Checked,
            &Catalog,
            &[Complete<'_, '_, '_, '_>],
            &[crate::compiler_descriptor::TypedDescriptorRootV1],
            &mut Budget<'_>,
        ),
    ) {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(PREFIX).unwrap();
        let (source, inputs, source_storage) = source_with_budget(count, &mut budget);
        let semantic = source.semantic_ssa().source_semantic();
        let typed = semantic
            .roots()
            .iter()
            .zip(&inputs)
            .map(|(id, input)| {
                crate::compiler_descriptor::checked_output_scalar_descriptor_fixture_v1(
                    &semantic.functions()[id.index() as usize],
                    &semantic.types()[NEUTRAL_ELEMENT_TYPE.index() as usize],
                    &input.logical_name,
                    &input.source_launch,
                )
            })
            .collect::<Vec<_>>();
        let references =
            crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default();
        let ProductionRankedSemanticProgramV1 {
            materialized,
            roots,
        } = project_and_verify_ranked_materialized_semantic_mir_v1(source, &inputs, &references)
            .unwrap();
        let floor = budget.storage();
        with_authenticated_borrowed_ranked_source_roster_v1(
            &materialized,
            roots,
            &mut budget,
            |r1, verification, budget| {
                assert!(verification.roots().iter().all(|root| {
                    !root
                        .verification()
                        .has_authenticated_functional_verification()
                }));
                let (bound, bound_storage) = {
                    let raw = dialect_amdgcn::bind_production_target_v1(
                        materialized.executable().module(),
                        profile,
                    )
                    .unwrap();
                    Owner::from_module_ref_with_verification_budget_v12(raw.module(), budget)
                        .unwrap()
                };
                budget
                    .reserve_storage(bound_storage.retained_storage())
                    .unwrap();
                let checked =
                    fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy3_v1(&bound, budget)
                        .unwrap();
                let checked_storage = checked.storage().retained_storage();
                budget.reserve_storage(checked_storage).unwrap();
                assert_ne!(
                    bound.canonical().canonical_bytes(),
                    checked.owner().canonical().canonical_bytes()
                );
                let source = RankedProjectionSourceV1::from_legacy(&materialized).unwrap();
                let before = budget.storage();
                let completed = with_prepared_checked_output_module_v1(
                    &source,
                    &materialized,
                    &bound,
                    &checked,
                    profile,
                    &inputs,
                    &references,
                    budget,
                    None,
                    |view, formals, budget| {
                        assert_eq!(formals.len(), count);
                        assert!(std::ptr::eq(view.source(), r1.materialized()));
                        assert!(std::ptr::eq(view.bound(), &bound));
                        assert!(std::ptr::eq(view.output(), checked.owner()));
                        let start = budget.work();
                        let catalog = view.output_pipeline_catalog(budget).unwrap();
                        assert_eq!(budget.work() - start, 1);
                        body(r1, &bound, &checked, catalog, formals, &typed, budget);
                        Ok(())
                    },
                );
                if exhausted {
                    assert!(matches!(completed, Err(CheckedOutputModuleJoinErrorV1::Formal(
                        fe2o3_lower_mir_kernel::ProductionScopedFormalMemoryErrorV1::SourceOutput(
                            fe2o3_lower_mir_kernel::ProductionSourceOutputErrorV1::Resource(Resource::Work(_))
                        )
                    ))));
                } else {
                    completed.unwrap();
                }
                assert_eq!(budget.storage(), before);
                drop(checked);
                budget.release_storage(checked_storage).unwrap();
                drop(bound);
                budget
                    .release_storage(bound_storage.retained_storage())
                    .unwrap();
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(budget.storage(), floor);
        drop(materialized);
        budget.release_storage(source_storage).unwrap();
        assert_eq!(budget.storage(), PREFIX);
        budget.release_storage(PREFIX).unwrap();
    }

    #[allow(clippy::too_many_arguments)]
    fn run(
        r1: &R1<'_>,
        bound: &Owner,
        checked: &Checked,
        catalog: &Catalog,
        formals: &[Complete<'_, '_, '_, '_>],
        typed: &[crate::compiler_descriptor::TypedDescriptorRootV1],
        profile: Profile,
        budget: &mut Budget<'_>,
        next: impl FnOnce(
            &fe2o3_kernel_opt::CheckedCanonicalPolicy3ExecutionReceiptV1<'_, '_>,
            &dialect_amdgcn::ReplayedNativeV12TextDescriptorRelationV1<'_, '_, '_, '_>,
            &mut Budget<'_>,
        ) -> Result<(), Pipeline>,
    ) -> Result<(), Pipeline> {
        with_checked_output_local_relations_v1(
            r1,
            bound,
            checked,
            catalog,
            formals,
            typed,
            profile,
            fe2o3_compiler_ffi::DeviceTargetV1::parse(profile.device_target()).unwrap(),
            None,
            budget,
            next,
        )
    }

    #[test]
    fn actual_source_local_relations_keep_one_b_o_catalog_and_descriptor_scope() {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            for count in [1, 2] {
                with_fixture(
                    profile,
                    count,
                    |r1, bound, checked, catalog, formals, typed, budget| {
                        let floor = budget.storage();
                        let mut prior = budget.work();
                        for _ in 0..2 {
                            run(
                                r1,
                                bound,
                                checked,
                                catalog,
                                formals,
                                typed,
                                profile,
                                budget,
                                |local, native, budget| {
                                    assert!(std::ptr::eq(local.input(), bound));
                                    assert!(std::ptr::eq(local.output(), checked.owner()));
                                    assert!(std::ptr::eq(local.execution_owner(), checked));
                                    assert!(std::ptr::eq(native.output(), checked.owner()));
                                    assert!(std::ptr::eq(native.catalog(), catalog));
                                    assert_eq!(native.descriptors().kernels().len(), count);
                                    assert!(
                                        native
                                            .final_llvm()
                                            .starts_with(native.pre_descriptor_llvm())
                                    );
                                    assert!(native.final_llvm().contains("module asm \".byte "));
                                    assert_eq!(native.profile(), profile);
                                    for root in native.descriptors().kernels() {
                                        assert_eq!(
                                            root.descriptor_symbol().as_str().strip_suffix(".kd"),
                                            Some(root.entry_name().as_str())
                                        );
                                    }
                                    assert!(!local.grants_authority());
                                    assert!(!native.grants_authority());
                                    assert!(
                                        budget.storage()
                                            >= floor
                                                + local.storage().retained_storage()
                                                + native.storage().retained_storage()
                                    );
                                    Ok(())
                                },
                            )
                            .unwrap();
                            assert_eq!(budget.storage(), floor);
                            assert!(budget.work() > prior);
                            prior = budget.work();
                        }
                    },
                );
            }
        }
    }

    #[test]
    fn wrapper_bookkeeping_has_source_derived_prefix_and_cleanup_boundaries() {
        for prior in [0, 7] {
            for allowed in [7, 8] {
                let mut work = Work::new(prior + allowed);
                let mut budget = Budget::new(&mut work, PREFIX + 3);
                budget.charge_work(prior).unwrap();
                budget.reserve_storage(PREFIX).unwrap();
                let mut entered = false;
                let result = with_local_relation_scope_v1(&mut budget, |budget| {
                    entered = true;
                    budget
                        .reserve_storage(3)
                        .map_err(local_relation_resource_v1)?;
                    Ok(())
                });
                assert_eq!(entered, allowed == 8);
                assert_eq!(result.is_ok(), allowed == 8);
                assert_eq!(budget.work(), prior + if allowed == 8 { 8 } else { 0 });
                assert_eq!(budget.storage(), PREFIX);
            }
        }
    }

    #[test]
    fn local_relations_storage_denial_preserves_live_fixture_and_allows_reentry() {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_fixture(
                profile,
                1,
                |r1, bound, checked, catalog, formals, typed, budget| {
                    let floor = budget.storage();
                    let held = budget.storage_limit() - floor - 1;
                    budget.reserve_storage(held).unwrap();
                    let mut entered = false;
                    let failed = run(
                        r1,
                        bound,
                        checked,
                        catalog,
                        formals,
                        typed,
                        profile,
                        budget,
                        |_, _, _| {
                            entered = true;
                            Ok(())
                        },
                    );
                    assert!(matches!(failed, Err(Pipeline::CheckedOutputMemoryTarget(
                    crate::production_pipeline::CheckedOutputMemoryTargetErrorV1::LocalRelation(
                        CheckedOutputLocalRelationErrorV1::Policy3(fe2o3_kernel_opt::CanonicalPolicy3ExecutionReceiptErrorV1::Codec(
                            fe2o3_kernel_ir::CanonicalKirTransitionReceiptErrorV1::Resource(Resource::Storage(_))
                        ))
                    )
                ))));
                    assert!(!entered);
                    assert_eq!(budget.storage(), floor + held);
                    let denial = budget.failed_storage();
                    assert!(denial.is_some());
                    budget.release_storage(held).unwrap();
                    run(
                        r1,
                        bound,
                        checked,
                        catalog,
                        formals,
                        typed,
                        profile,
                        budget,
                        |_, _, _| Ok(()),
                    )
                    .unwrap();
                    assert_eq!(budget.storage(), floor);
                    assert_eq!(budget.failed_storage(), denial);
                },
            );
        }
    }

    fn marker() -> Pipeline {
        Pipeline::RankedVerification(ProductionRankedVerificationErrorV1::RosterMetadata(
            "stage A callback marker",
        ))
    }

    #[test]
    fn actual_local_relations_error_panic_and_broken_floor_preserve_precedence() {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_fixture(
                profile,
                1,
                |r1, bound, checked, catalog, formals, typed, budget| {
                    let floor = budget.storage();
                    let failed = run(
                        r1,
                        bound,
                        checked,
                        catalog,
                        formals,
                        typed,
                        profile,
                        budget,
                        |_, _, _| Err(marker()),
                    );
                    assert!(matches!(
                        failed,
                        Err(Pipeline::RankedVerification(
                            ProductionRankedVerificationErrorV1::RosterMetadata(
                                "stage A callback marker"
                            )
                        ))
                    ));
                    assert_eq!(budget.storage(), floor);
                    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        run(
                            r1,
                            bound,
                            checked,
                            catalog,
                            formals,
                            typed,
                            profile,
                            budget,
                            |_, _, _| panic!("stage A panic marker"),
                        )
                    }))
                    .unwrap_err();
                    assert_eq!(panic.downcast_ref::<&str>(), Some(&"stage A panic marker"));
                    assert_eq!(budget.storage(), floor);
                    for unwind in [false, true] {
                        let outcome =
                            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                run(
                                    r1,
                                    bound,
                                    checked,
                                    catalog,
                                    formals,
                                    typed,
                                    profile,
                                    budget,
                                    |_, _, budget| {
                                        budget
                                            .release_storage(budget.storage() - floor + 1)
                                            .unwrap();
                                        if unwind {
                                            panic!("broken local floor");
                                        }
                                        Err(marker())
                                    },
                                )
                            }))
                            .unwrap();
                        assert!(matches!(outcome, Err(Pipeline::CheckedOutputMemoryTarget(
                        crate::production_pipeline::CheckedOutputMemoryTargetErrorV1::LocalRelation(CheckedOutputLocalRelationErrorV1::Resource(Resource::Accounting))
                    ))));
                        assert_eq!(budget.storage(), floor - 1);
                        budget.reserve_storage(1).unwrap();
                    }
                    run(
                        r1,
                        bound,
                        checked,
                        catalog,
                        formals,
                        typed,
                        profile,
                        budget,
                        |_, _, _| Ok(()),
                    )
                    .unwrap();
                    assert_eq!(budget.storage(), floor);
                },
            );
        }
    }

    #[test]
    fn descriptor_postflight_work_failure_overrides_local_callback_error_or_panic() {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            for unwind in [false, true] {
                with_fixture_expected_tail(
                    profile,
                    1,
                    true,
                    |r1, bound, checked, catalog, formals, typed, budget| {
                        let floor = budget.storage();
                        let outcome =
                            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                run(
                                    r1,
                                    bound,
                                    checked,
                                    catalog,
                                    formals,
                                    typed,
                                    profile,
                                    budget,
                                    |_, _, budget| {
                                        budget.charge_work(WORK - budget.work()).unwrap();
                                        if unwind {
                                            panic!("exhausted callback marker");
                                        }
                                        Err(marker())
                                    },
                                )
                            }))
                            .unwrap();
                        assert!(matches!(outcome, Err(Pipeline::DescriptorEvidence(
                        crate::compiler_descriptor::CompilerDescriptorError::CheckedOutputSource(
                            fe2o3_lower_mir_kernel::ProductionSourceOutputErrorV1::Resource(Resource::Work(_))
                        )
                    ))));
                        assert_eq!(budget.storage(), floor);
                        assert_eq!(budget.work(), WORK);
                    },
                );
            }
        }
    }

    fn copied_bytes(bytes: &[u8], budget: &mut Budget<'_>) -> (Vec<u8>, usize) {
        budget.charge_work(bytes.len() + 2).unwrap();
        let header = std::mem::size_of::<Vec<u8>>();
        budget.reserve_storage(header + bytes.len()).unwrap();
        let mut out = Vec::new();
        out.try_reserve_exact(bytes.len()).unwrap();
        budget
            .reserve_storage(out.capacity() - bytes.len())
            .unwrap();
        out.extend_from_slice(bytes);
        let retained = header + out.capacity();
        (out, retained)
    }

    #[test]
    fn valid_semantics_with_mutated_execution_claim_is_not_local_execution() {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_fixture(
                profile,
                1,
                |r1, bound, checked, catalog, formals, typed, budget| {
                    run(r1, bound, checked, catalog, formals, typed, profile, budget, |local, _, budget| {
                    let floor = budget.storage();
                    let (mut altered, retained) = copied_bytes(local.receipt().canonical_bytes(), budget);
                    // Closed framing32 + fixed-record profile-work offset96.
                    let old = u64::from_le_bytes(altered[128..136].try_into().unwrap());
                    altered[128..136].copy_from_slice(&(old ^ 1).to_le_bytes());
                    let replay = fe2o3_kernel_opt::decode_and_check_published_policy3_semantic_relation_v1(bound, checked.owner(), &altered, budget).unwrap();
                    let replay_storage = replay.storage().retained_storage();
                    budget.reserve_storage(replay_storage).unwrap();
                    assert_eq!(replay.unauthenticated_execution_claim().declared_profile_work(), old ^ 1);
                    assert!(!replay.unauthenticated_execution_claim().grants_authority());
                    assert!(!replay.grants_authority());
                    drop(replay);
                    budget.release_storage(replay_storage).unwrap();
                    assert!(matches!(fe2o3_kernel_opt::decode_and_check_canonical_policy3_execution_receipt_v1(bound, checked, &altered, budget),
                        Err(fe2o3_kernel_opt::CanonicalPolicy3ExecutionReceiptErrorV1::ExecutionWitness)));
                    drop(altered);
                    budget.release_storage(retained).unwrap();
                    // A separately encoded, canonical semantic body for another
                    // endpoint cannot be transplanted under a valid execution record.
                    let (foreign_body, foreign_storage) = fe2o3_kernel_ir::InertCanonicalKirTransitionReceiptV1::from_candidate_with_budget(
                        checked.owner().canonical().identity(), checked.owner().canonical().identity(),
                        checked.occurrences().candidate(), budget,
                    ).unwrap();
                    budget.reserve_storage(foreign_storage.retained_storage()).unwrap();
                    let (mut transplanted, retained) = copied_bytes(local.receipt().canonical_bytes(), budget);
                    let header = fe2o3_kernel_opt::CANONICAL_POLICY3_EXECUTION_RECEIPT_HEADER_V1;
                    assert_eq!(transplanted.len() - header, foreign_body.canonical_bytes().len());
                    transplanted[header..].copy_from_slice(foreign_body.canonical_bytes());
                    assert!(matches!(fe2o3_kernel_opt::decode_and_check_canonical_policy3_execution_receipt_v1(bound, checked, &transplanted, budget),
                        Err(fe2o3_kernel_opt::CanonicalPolicy3ExecutionReceiptErrorV1::Semantic(
                            fe2o3_kernel_opt::KernelIrCheckedOptimizationReceiptErrorV1::Transition(
                                fe2o3_kernel_analysis::CanonicalKirTransitionErrorV1::Rule("transition receipt endpoint or policy")
                            )
                        ))));
                    drop(transplanted);
                    budget.release_storage(retained).unwrap();
                    drop(foreign_body);
                    budget.release_storage(foreign_storage.retained_storage()).unwrap();
                    assert_eq!(budget.storage(), floor);
                    Ok(())
                }).unwrap();
                    let mut entered = false;
                    let failed = run(
                        r1,
                        checked.owner(),
                        checked,
                        catalog,
                        formals,
                        typed,
                        profile,
                        budget,
                        |_, _, _| {
                            entered = true;
                            Ok(())
                        },
                    );
                    assert!(matches!(failed, Err(Pipeline::CheckedOutputMemoryTarget(
                    crate::production_pipeline::CheckedOutputMemoryTargetErrorV1::LocalRelation(CheckedOutputLocalRelationErrorV1::Policy3(fe2o3_kernel_opt::CanonicalPolicy3ExecutionReceiptErrorV1::InputHistory))
                ))));
                    assert!(!entered);
                },
            );
        }
    }

    #[test]
    fn actual_source_native_relation_rejects_changed_bytes_and_catalog_binding() {
        use dialect_amdgcn::{
            NativeV12TextDescriptorReplayErrorV1 as E,
            check_native_v12_text_descriptor_relation_v1 as check,
        };
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_fixture(
                profile,
                2,
                |r1, bound, checked, catalog, formals, typed, budget| {
                    run(r1, bound, checked, catalog, formals, typed, profile, budget, |_, native, budget| {
                    let floor = budget.storage();
                    let (mut bytes, storage) = copied_bytes(checked.owner().canonical().canonical_bytes(), budget);
                    bytes[0] ^= 1;
                    assert!(matches!(check(checked.owner(), catalog, &bytes, profile, native.descriptors(), native.final_llvm(), budget), Err(E::OutputBytes)));
                    drop(bytes);
                    budget.release_storage(storage).unwrap();
                    let suffix_byte = native.final_llvm().find("module asm \".byte 0x").unwrap()
                        + "module asm \".byte 0x".len();
                    for offset in [0, suffix_byte] {
                        let (mut llvm, storage) = copied_bytes(native.final_llvm().as_bytes(), budget);
                        llvm[offset] = if llvm[offset] == b'0' { b'1' } else { b'0' };
                        assert!(matches!(check(checked.owner(), catalog, checked.owner().canonical().canonical_bytes(), profile, native.descriptors(), std::str::from_utf8(&llvm).unwrap(), budget), Err(E::Invalid("exact native LLVM/descriptor text"))));
                        drop(llvm);
                        budget.release_storage(storage).unwrap();
                    }
                    // This well-framed catalog claims a nonexistent actual O allocation.
                    let definition = fe2o3_kernel_ir::KernelIrPipelineContractDefinitionV1 {
                        key: 0, semantic_pipeline_type: 20, semantic_payload_type: 3,
                        buffers: 2, elements: 2, prefetch_distance: 1, packed_bits: 32,
                        source_size_bytes: 4, source_alignment_bytes: 4,
                    };
                    let binding = fe2o3_kernel_ir::KernelIrPipelineStorageBindingV1 {
                        function: 0, storage: u32::MAX, key: 0, block: 0, operation: 0,
                    };
                    let (bad, bad_storage) = Catalog::from_rows_with_budget(*catalog.semantic_source(), &[definition], &[binding], budget).unwrap();
                    budget.reserve_storage(bad_storage.retained_storage()).unwrap();
                    assert!(matches!(check(checked.owner(), &bad, checked.owner().canonical().canonical_bytes(), profile, native.descriptors(), native.final_llvm(), budget),
                        Err(E::Catalog(fe2o3_kernel_analysis::KernelIrContractCatalogBindingErrorV1::Invalid("allocation value")))));
                    drop(bad);
                    budget.release_storage(bad_storage.retained_storage()).unwrap();
                    assert_eq!(budget.storage(), floor);
                    Ok(())
                }).unwrap();
                },
            );
        }
    }

    #[test]
    fn new_held_source_gate_still_refuses_functional_none_before_b_o() {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(PREFIX).unwrap();
        let (source, inputs, retained) = source_with_budget(2, &mut budget);
        let floor = budget.storage();
        let mut entered = false;
        let result = with_source_ranked_custody_v1(
            &source,
            &inputs,
            &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
            &mut budget,
            |_, _| {
                entered = true;
                Ok(())
            },
        );
        assert!(matches!(
            result,
            Err(Pipeline::RankedVerification(
                ProductionRankedVerificationErrorV1::RosterMetadata(
                    "every source-first root requires retained functional and aggregate custody"
                )
            ))
        ));
        assert!(!entered);
        assert_eq!(budget.storage(), floor);
        drop(source);
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.storage(), PREFIX);
    }
}
