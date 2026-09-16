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
        with_fixture_catalogs_expected_tail(
            profile,
            count,
            exhausted,
            false,
            |r1, bound, checked, input, catalog, formals, typed, budget| {
                assert!(input.is_none());
                body(r1, bound, checked, catalog, formals, typed, budget);
            },
        )
    }

    fn with_fixture_catalogs_expected_tail(
        profile: Profile,
        count: usize,
        exhausted: bool,
        borrow_input: bool,
        body: impl FnOnce(
            &R1<'_>,
            &Owner,
            &Checked,
            Option<&Catalog>,
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
                        let input = if borrow_input {
                            let start = budget.work();
                            let input = view.input_pipeline_catalog(budget).unwrap();
                            assert_eq!(budget.work() - start, 1);
                            Some(input)
                        } else {
                            None
                        };
                        body(r1, &bound, &checked, input, catalog, formals, &typed, budget);
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

    #[test]
    fn native_input_chain_keeps_original_borrows_through_t_and_l_on_all_component_roots() {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            for count in [1, 2] {
                with_fixture_catalogs_expected_tail(
                    profile,
                    count,
                    false,
                    true,
                    |r1, bound, checked, input, output, formals, typed, budget| {
                        let input = input.unwrap();
                        let source = r1.materialized();
                        let floor = budget.storage();
                        let mut previous = budget.work();
                        for _ in 0..2 {
                            with_native_input_relations_v1(
                                source,
                                bound,
                                input,
                                profile,
                                budget,
                                |materialization, target, budget| {
                                    assert!(std::ptr::eq(
                                        materialization.source(),
                                        source.semantic_ssa()
                                    ));
                                    assert!(std::ptr::eq(
                                        materialization.launch(),
                                        source.source_launch()
                                    ));
                                    assert!(std::ptr::eq(
                                        materialization.native(),
                                        source.executable()
                                    ));
                                    assert!(std::ptr::eq(materialization.catalog(), input));
                                    assert!(std::ptr::eq(target.neutral(), source.executable()));
                                    assert!(std::ptr::eq(target.bound(), bound));
                                    assert!(std::ptr::eq(target.neutral_catalog(), input));
                                    assert!(std::ptr::eq(target.bound_catalog(), input));
                                    assert_eq!(target.profile(), profile);
                                    assert!(!materialization.grants_authority());
                                    assert!(!target.grants_authority());
                                    let input_floor = budget.storage();
                                    assert!(
                                        input_floor
                                            >= floor
                                                + std::mem::size_of_val(materialization)
                                                + std::mem::size_of_val(target)
                                    );
                                    run(
                                        r1,
                                        bound,
                                        checked,
                                        output,
                                        formals,
                                        typed,
                                        profile,
                                        budget,
                                        |local, native, budget| {
                                            assert!(std::ptr::eq(local.input(), target.bound()));
                                            assert!(std::ptr::eq(local.execution_owner(), checked));
                                            assert!(std::ptr::eq(native.output(), checked.owner()));
                                            assert!(std::ptr::eq(native.catalog(), output));
                                            assert_eq!(native.descriptors().kernels().len(), count);
                                            assert!(!local.grants_authority());
                                            assert!(!native.grants_authority());
                                            assert!(budget.storage() >= input_floor);
                                            Ok(())
                                        },
                                    )?;
                                    assert_eq!(budget.storage(), input_floor);
                                    Ok(())
                                },
                            )
                            .unwrap();
                            assert_eq!(budget.storage(), floor);
                            assert!(budget.work() > previous);
                            previous = budget.work();
                        }
                    },
                );
            }
        }
    }

    #[test]
    fn native_input_scope_has_independent_nine_unit_prefix_and_cumulative_cleanup() {
        for prior in [0, 7] {
            for allowance in [8, 9, 18] {
                let mut work = Work::new(prior + allowance);
                let mut budget = Budget::new(&mut work, PREFIX + 3);
                budget.charge_work(prior).unwrap();
                budget.reserve_storage(PREFIX).unwrap();
                let mut entered = 0;
                for call in 0..2 {
                    let result = with_native_input_relation_scope_v1(&mut budget, |budget| {
                        entered += 1;
                        budget
                            .reserve_storage(3)
                            .map_err(local_relation_resource_v1)?;
                        Ok(())
                    });
                    let accepted = allowance >= (call + 1) * 9;
                    assert_eq!(result.is_ok(), accepted);
                    assert_eq!(budget.storage(), PREFIX);
                }
                assert_eq!(entered, allowance / 9);
                assert_eq!(budget.work(), prior + (allowance / 9) * 9);
                budget.release_storage(PREFIX).unwrap();
                assert_eq!(work.failed_work().is_some(), allowance < 18);
            }
        }
    }

    #[test]
    fn native_input_inventory_header_denial_preserves_floor_and_reentry() {
        use fe2o3_kernel_analysis::{CanonicalKirInventoryErrorV1, CanonicalKirInventoryV1};
        with_fixture_catalogs_expected_tail(
            Profile::Gfx942,
            1,
            false,
            true,
            |r1, bound, _, input, _, _, _, budget| {
                let input = input.unwrap();
                let floor = budget.storage();
                // Inventory census precedes its fixed header reservation. This is
                // one short of that minimum, not a calibrated whole-query cap.
                let minimum = std::mem::size_of::<CanonicalKirInventoryV1<'_>>();
                let held = budget.storage_limit() - floor - (minimum - 1);
                budget.reserve_storage(held).unwrap();
                let mut entered = false;
                let result = with_native_input_relations_v1(
                    r1.materialized(),
                    bound,
                    input,
                    Profile::Gfx942,
                    budget,
                    |_, _, _| {
                        entered = true;
                        Ok(())
                    },
                );
                assert!(matches!(
                    result,
                    Err(Pipeline::CheckedOutputMemoryTarget(
                        crate::production_pipeline::CheckedOutputMemoryTargetErrorV1::LocalRelation(
                            CheckedOutputLocalRelationErrorV1::Inventory(
                                CanonicalKirInventoryErrorV1::Resource(Resource::Storage(_))
                            )
                        )
                    ))
                ));
                assert!(!entered);
                assert_eq!(budget.storage(), floor + held);
                let denial = budget.failed_storage();
                assert!(denial.is_some());
                budget.release_storage(held).unwrap();
                with_native_input_relations_v1(
                    r1.materialized(),
                    bound,
                    input,
                    Profile::Gfx942,
                    budget,
                    |_, _, _| Ok(()),
                )
                .unwrap();
                assert_eq!(budget.storage(), floor);
                assert_eq!(budget.failed_storage(), denial);
            },
        );
    }

    #[test]
    fn native_input_prefix_retains_callback_errors_panic_and_accounting_precedence() {
        with_fixture_catalogs_expected_tail(
            Profile::Gfx942,
            1,
            false,
            true,
            |r1, bound, _, input, _, _, _, budget| {
                let input = input.unwrap();
                let floor = budget.storage();
                let result = with_native_input_relations_v1(
                    r1.materialized(),
                    bound,
                    input,
                    Profile::Gfx942,
                    budget,
                    |_, _, _| Err(marker()),
                );
                assert!(matches!(
                    result,
                    Err(Pipeline::RankedVerification(
                        ProductionRankedVerificationErrorV1::RosterMetadata(
                            "stage A callback marker"
                        )
                    ))
                ));
                assert_eq!(budget.storage(), floor);
                let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    with_native_input_relations_v1(
                        r1.materialized(),
                        bound,
                        input,
                        Profile::Gfx942,
                        budget,
                        |_, _, _| panic!("native input panic marker"),
                    )
                }))
                .unwrap_err();
                assert_eq!(
                    panic.downcast_ref::<&str>(),
                    Some(&"native input panic marker")
                );
                assert_eq!(budget.storage(), floor);
                for unwind in [false, true] {
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        with_native_input_relations_v1(
                            r1.materialized(),
                            bound,
                            input,
                            Profile::Gfx942,
                            budget,
                            |_, _, budget| {
                                budget
                                    .release_storage(budget.storage() - floor + 1)
                                    .unwrap();
                                if unwind {
                                    panic!("native input broken floor");
                                }
                                Err(marker())
                            },
                        )
                    }))
                    .unwrap();
                    assert!(matches!(result, Err(Pipeline::CheckedOutputMemoryTarget(
                        crate::production_pipeline::CheckedOutputMemoryTargetErrorV1::LocalRelation(
                            CheckedOutputLocalRelationErrorV1::Resource(Resource::Accounting)
                        )
                    ))));
                    assert_eq!(budget.storage(), floor - 1);
                    // Repair only the deliberate hostile byte after all query
                    // borrows have ended, so the original fixture can tear down.
                    budget.reserve_storage(1).unwrap();
                }
                with_native_input_relations_v1(
                    r1.materialized(),
                    bound,
                    input,
                    Profile::Gfx942,
                    budget,
                    |_, _, _| Ok(()),
                )
                .unwrap();
                assert_eq!(budget.storage(), floor);
            },
        );
    }

    #[test]
    fn native_input_chain_preserves_later_descriptor_work_failure_over_err_and_panic() {
        for unwind in [false, true] {
            with_fixture_catalogs_expected_tail(
                Profile::Gfx942,
                1,
                true,
                true,
                |r1, bound, checked, input, output, formals, typed, budget| {
                    let floor = budget.storage();
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        with_native_input_relations_v1(
                            r1.materialized(),
                            bound,
                            input.unwrap(),
                            Profile::Gfx942,
                            budget,
                            |_, _, budget| {
                                run(
                                    r1,
                                    bound,
                                    checked,
                                    output,
                                    formals,
                                    typed,
                                    Profile::Gfx942,
                                    budget,
                                    |_, _, budget| {
                                        budget.charge_work(WORK - budget.work()).unwrap();
                                        if unwind {
                                            panic!("native input later postflight");
                                        }
                                        Err(marker())
                                    },
                                )
                            },
                        )
                    }))
                    .unwrap();
                    assert!(matches!(result, Err(Pipeline::DescriptorEvidence(
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

    #[test]
    fn native_input_chain_source_catalog_and_bound_mismatches_do_not_enter_t_or_l() {
        with_fixture_catalogs_expected_tail(
            Profile::Gfx942,
            1,
            false,
            true,
            |r1, bound, _, input, _, _, _, budget| {
                let input = input.unwrap();
                let floor = budget.storage();
                let mut semantic = *input.semantic_source();
                semantic[0] ^= 1;
                let (foreign, receipt) = Catalog::from_rows_with_budget(
                    semantic,
                    input.definitions(),
                    input.bindings(),
                    budget,
                )
                .unwrap();
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                let with_foreign = budget.storage();
                let mut entered = false;
                let result = with_native_input_relations_v1(
                    r1.materialized(),
                    bound,
                    &foreign,
                    Profile::Gfx942,
                    budget,
                    |_, _, _| {
                        entered = true;
                        Ok(())
                    },
                );
                assert!(matches!(result, Err(Pipeline::CheckedOutputMemoryTarget(
                    crate::production_pipeline::CheckedOutputMemoryTargetErrorV1::LocalRelation(
                        CheckedOutputLocalRelationErrorV1::Materialization(
                            fe2o3_lower_mir_kernel::SuppliedNativeMaterializationErrorV1::Invalid("complete source catalog bytes")
                        )
                    )
                ))));
                assert!(!entered);
                assert_eq!(budget.storage(), with_foreign);
                drop(foreign);
                budget.release_storage(receipt.retained_storage()).unwrap();

                // Prefix-only adversary: real J would reject this B before
                // this later chain. A valid different workgroup reaches B2.
                let (changed, receipt) = {
                    let mut module = bound.module().clone();
                    module.kernels[0].workgroup_size =
                        Some(fe2o3_kernel_ir::WorkgroupSize::new(128, 1, 1));
                    Owner::from_module_ref_with_verification_budget_v12(&module, budget).unwrap()
                };
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                let changed_floor = budget.storage();
                let result = with_native_input_relations_v1(
                    r1.materialized(),
                    &changed,
                    input,
                    Profile::Gfx942,
                    budget,
                    |_, _, _| {
                        entered = true;
                        Ok(())
                    },
                );
                assert!(matches!(
                    result,
                    Err(Pipeline::CheckedOutputMemoryTarget(
                        crate::production_pipeline::CheckedOutputMemoryTargetErrorV1::LocalRelation(
                            CheckedOutputLocalRelationErrorV1::Target(
                                dialect_amdgcn::NativeV12TargetBindingRelationErrorV1::Target(_)
                            )
                        )
                    ))
                ));
                assert!(!entered);
                assert_eq!(budget.storage(), changed_floor);
                drop(changed);
                budget.release_storage(receipt.retained_storage()).unwrap();
                assert_eq!(budget.storage(), floor);
            },
        );
    }

    #[test]
    fn native_input_chain_production_order_keeps_source_gates_and_owned_stage_cleanup() {
        let local = include_str!("checked_output_local_relations_v1.rs");
        let adapter = local
            .split("pub(crate) fn with_source_checked_output_local_relations_v1(")
            .nth(1)
            .unwrap()
            .split("fn with_native_input_relation_scope_v1(")
            .next()
            .unwrap();
        assert!(
            adapter.find("with_native_input_relations_v1(").unwrap()
                < adapter
                    .find("with_checked_output_local_relations_v1(")
                    .unwrap()
        );
        let prefix = local
            .split("fn with_native_input_relations_v1(")
            .nth(1)
            .unwrap()
            .split("// Shared inert core;")
            .next()
            .unwrap();
        assert_eq!(
            prefix.matches("CanonicalKirInventoryV1::derive(").count(),
            2
        );
        assert_eq!(
            prefix
                .matches("check_kernel_ir_contract_catalog_v1(")
                .count(),
            2
        );
        assert_eq!(prefix.matches(".reserve_storage(").count(), 6);
        assert!(
            prefix
                .find("check_supplied_native_materialization_consistency_v1(")
                .unwrap()
                < prefix
                    .find("check_native_v12_target_binding_relation_v1(")
                    .unwrap()
        );
        assert!(
            prefix
                .find("check_native_v12_target_binding_relation_v1(")
                .unwrap()
                < prefix
                    .find("next(&materialization, &target, budget)")
                    .unwrap()
        );
        assert!(!prefix.contains("optimize_"));
        assert!(!prefix.contains("bind_production_target_v1("));
        assert!(!prefix.contains("try_materialize_with_budget("));

        let join = include_str!("checked_output_module_join_v1.rs");
        let catalog = join
            .split("pub(crate) fn with_source_checked_output_module_catalog_v1<T>(")
            .nth(1)
            .unwrap()
            .split("fn with_source_checked_output_module_view_v1<T>(")
            .next()
            .unwrap();
        assert!(
            catalog
                .find("view.output_pipeline_catalog(budget)")
                .unwrap()
                < catalog.find("view.input_pipeline_catalog(budget)").unwrap()
        );
        assert!(catalog.contains("next(formals, input_catalog, catalog, budget)"));
        assert!(
            join.find(".check_borrowed_ranked_addresses_v1(").unwrap()
                < join
                    .find("::with_complete_formal_memory_module_v1(")
                    .unwrap()
        );

        let source = include_str!("checked_output_source_join_v1.rs");
        for gate in [
            "has_authenticated_functional_verification()",
            "retained_functional_verification_is_coherent()",
            "aggregate_verus_execution().is_none()",
        ] {
            assert!(source.contains(gate));
        }
        let source_gate: String = source
            .split("pub(crate) fn with_source_ranked_custody_v1<T>(")
            .nth(1)
            .unwrap()
            .split("fn with_source_ranked_prefix_v1<T>(")
            .next()
            .unwrap()
            .split_whitespace()
            .collect();
        assert!(
            source_gate
                .find("require_source_functional_roster_v1(")
                .unwrap()
                < source_gate.find("next(&SourceRankedCustodyV1").unwrap()
        );

        let pipeline = include_str!("../production_checked_output_pipeline_v1.rs");
        let setup = pipeline
            .split("fn with_source_checked_output_transaction_v1<T>(")
            .nth(1)
            .unwrap()
            .split("pub(crate) fn lower_checked_output_memory_target_v1(")
            .next()
            .unwrap();
        assert!(
            setup.find("::with_source_ranked_custody_v1(").unwrap()
                < setup
                    .find("with_checked_output_target_endpoint_v1(")
                    .unwrap()
        );
        let stage_a = pipeline
            .split("pub(crate) fn with_checked_output_local_relations_v1(")
            .nth(1)
            .unwrap();
        let export = include_str!("../production_checked_output_export_buffer_v1.rs");
        let stage_b1 = export
            .split("fn prepare_inert_checked_output_buffer_v1(")
            .nth(1)
            .unwrap();
        for route in [stage_a, stage_b1] {
            assert!(route.contains("|formals, input_catalog, catalog, budget|"));
            assert!(route.contains("source, bound, checked, input_catalog, catalog, formals,"));
            assert!(
                route
                    .find("::with_source_checked_output_module_catalog_v1(")
                    .unwrap()
                    < route
                        .find(".validate_inert_target_geometry_v1(budget)")
                        .unwrap()
            );
            assert!(
                route
                    .find(".validate_inert_target_geometry_v1(budget)")
                    .unwrap()
                    < route
                        .find("::with_source_checked_output_local_relations_v1(")
                        .unwrap()
            );
            assert!(!route.contains("AuthenticatedReferenceEffectBindingsV1::default"));
        }
        assert!(
            stage_b1.find("::with_source_ranked_custody_v1(").unwrap()
                < stage_b1
                    .find("with_checked_output_target_endpoint_v1(")
                    .unwrap()
        );
        let split = stage_b1.split("|stage| {").last().unwrap();
        assert!(
            split.find("drop((materialized, ranked_roots));").unwrap()
                < split.rfind("bindings").unwrap()
        );
        assert_eq!(
            pipeline
                .matches("dialect_amdgcn::bind_production_target_v1(")
                .count(),
            1
        );
        assert_eq!(
            pipeline
                .matches("optimize_native_neutral_kernel_ir_policy3_v1(")
                .count(),
            1
        );
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

mod native_input_pipeline_catalog_tests {
    use super::*;
    use crate::production_pipeline::{CheckedOutputMemoryTargetErrorV1, ProductionPipelineError};
    use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
    use fe2o3_kernel_analysis::{CanonicalKirInventoryV1, check_kernel_ir_contract_catalog_v1};
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work, InertCanonicalKernelIrContractCatalogV1 as Catalog,
        VerifiedCanonicalKernelIrModuleV12 as Owner,
    };

    include!("source_output_pipeline_fixture_v1_tests.rs");

    fn graph_binding_without_markers(owner: &Owner, catalog: &Catalog, budget: &mut Budget<'_>) {
        let floor = budget.storage();
        {
            let (inventory, inventory_storage) =
                CanonicalKirInventoryV1::derive(owner, budget).unwrap();
            budget
                .reserve_storage(inventory_storage.retained_storage())
                .unwrap();
            let (checked, storage) =
                check_kernel_ir_contract_catalog_v1(&inventory, catalog, budget).unwrap();
            budget.reserve_storage(storage.retained_storage()).unwrap();
            assert_eq!(checked.marker_count(), 0);
            assert!(!checked.grants_authority());
        }
        // Both the catalog view and the inventory are lexically gone.
        budget
            .release_storage(budget.storage().checked_sub(floor).unwrap())
            .unwrap();
        assert_eq!(budget.storage(), floor);
    }

    #[test]
    fn native_input_prefix_uses_real_nonempty_source_catalog_not_relocated_output_rows() {
        const WORK: usize = 1_000_000_000_000;
        const STORAGE: usize = 1_000_000_000;
        const PREFIX: usize = 97;
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(PREFIX).unwrap();
        let (mut ssa, launch) = pipeline_fixture(false);
        let capture = ssa
            .try_capture_occurrences_with_budget_v1(&mut budget)
            .unwrap();
        budget.reserve_storage(capture.retained_storage()).unwrap();
        let source =
            fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
                ssa,
                launch,
                fe2o3_lower_mir_kernel::ProductionSemanticKirLimitsV1::default(),
                &mut budget,
            )
            .unwrap();
        let owner_storage = source.executable_storage().retained_storage()
            + source.assert_origin_storage().payload_storage();
        budget.reserve_storage(owner_storage).unwrap();
        let source_floor = budget.storage();
        let mut previous = budget.work();
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            {
                let (bound, bound_storage) = {
                    let raw = dialect_amdgcn::bind_production_target_v1(
                        source.executable().module(),
                        profile,
                    )
                    .unwrap();
                    Owner::from_module_ref_with_verification_budget_v12(raw.module(), &mut budget)
                        .unwrap()
                };
                budget
                    .reserve_storage(bound_storage.retained_storage())
                    .unwrap();
                let checked = fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy3_v1(
                    &bound,
                    &mut budget,
                )
                .unwrap();
                budget
                    .reserve_storage(checked.storage().retained_storage())
                    .unwrap();
                let (coordinates, coordinate_storage) =
                    dialect_amdgcn::check_production_target_coordinate_preservation_v1(
                        source.executable(),
                        &bound,
                        profile,
                        &mut budget,
                    )
                    .unwrap();
                budget
                    .reserve_storage(coordinate_storage.retained_storage())
                    .unwrap();
                let (view, view_storage) =
                    fe2o3_lower_mir_kernel::derive_source_output_occurrences_policy3_v1(
                        &source,
                        &coordinates,
                        &checked,
                        &mut budget,
                    )
                    .unwrap();
                budget
                    .reserve_storage(view_storage.retained_storage())
                    .unwrap();
                let input = view.input_pipeline_catalog(&mut budget).unwrap();
                let output = view.output_pipeline_catalog(&mut budget).unwrap();
                assert_eq!((input.definitions().len(), input.bindings().len()), (1, 1));
                assert_eq!(output.bindings().len(), 1);
                assert_ne!(input.bindings()[0], output.bindings()[0]);
                assert!(
                    view.pipeline_marker_placements(&mut budget)
                        .unwrap()
                        .is_empty()
                );
                graph_binding_without_markers(source.executable(), input, &mut budget);
                graph_binding_without_markers(&bound, input, &mut budget);
                let floor = budget.storage();
                let mut entered = false;
                with_native_input_relations_v1(
                    &source,
                    &bound,
                    input,
                    profile,
                    &mut budget,
                    |materialization, target, _| {
                        entered = true;
                        assert!(std::ptr::eq(
                            materialization.source(),
                            source.semantic_ssa()
                        ));
                        assert!(std::ptr::eq(
                            materialization.launch(),
                            source.source_launch()
                        ));
                        assert!(std::ptr::eq(materialization.native(), source.executable()));
                        assert!(std::ptr::eq(materialization.catalog(), input));
                        assert!(std::ptr::eq(target.neutral_catalog(), input));
                        assert!(std::ptr::eq(target.bound_catalog(), input));
                        assert_eq!(target.bound_catalog().bindings().len(), 1);
                        assert!(!materialization.grants_authority());
                        assert!(!target.grants_authority());
                        Ok(())
                    },
                )
                .unwrap();
                assert!(entered);
                assert_eq!(budget.storage(), floor);

                // Missing allocation metadata is graph-valid for this ordinary
                // marker-free emission, but not the source-derived full catalog.
                {
                    let (omitted, receipt) = Catalog::from_rows_with_budget(
                        *input.semantic_source(),
                        input.definitions(),
                        &[],
                        &mut budget,
                    )
                    .unwrap();
                    budget.reserve_storage(receipt.retained_storage()).unwrap();
                    graph_binding_without_markers(source.executable(), &omitted, &mut budget);
                    graph_binding_without_markers(&bound, &omitted, &mut budget);
                    let retained = budget.storage();
                    let rejected = with_native_input_relations_v1(
                        &source,
                        &bound,
                        &omitted,
                        profile,
                        &mut budget,
                        |_, _, _| panic!("incomplete source catalog entered local continuation"),
                    );
                    assert!(matches!(rejected, Err(ProductionPipelineError::CheckedOutputMemoryTarget(
                        CheckedOutputMemoryTargetErrorV1::LocalRelation(
                            CheckedOutputLocalRelationErrorV1::Materialization(
                                fe2o3_lower_mir_kernel::SuppliedNativeMaterializationErrorV1::Invalid("complete source catalog bytes")
                            )
                        )
                    ))));
                    assert_eq!(budget.storage(), retained);
                }
                budget
                    .release_storage(budget.storage().checked_sub(floor).unwrap())
                    .unwrap();

                // This is a proved relocated catalog, not merely a different
                // pointer to byte-identical N/B/O metadata.
                let rejected = with_native_input_relations_v1(
                    &source,
                    &bound,
                    output,
                    profile,
                    &mut budget,
                    |_, _, _| panic!("relocated output rows entered input continuation"),
                );
                assert!(matches!(
                    rejected,
                    Err(ProductionPipelineError::CheckedOutputMemoryTarget(
                        CheckedOutputMemoryTargetErrorV1::LocalRelation(
                            CheckedOutputLocalRelationErrorV1::Catalog(_)
                        )
                    ))
                ));
                assert_eq!(budget.storage(), floor);
            }
            // View, coordinates, checked O, and B all end before release.
            budget
                .release_storage(budget.storage().checked_sub(source_floor).unwrap())
                .unwrap();
            assert_eq!(budget.storage(), source_floor);
            assert!(budget.work() > previous);
            previous = budget.work();
        }
        drop(source);
        budget
            .release_storage(owner_storage + capture.retained_storage())
            .unwrap();
        assert_eq!(budget.storage(), PREFIX);
        budget.release_storage(PREFIX).unwrap();
    }
}
