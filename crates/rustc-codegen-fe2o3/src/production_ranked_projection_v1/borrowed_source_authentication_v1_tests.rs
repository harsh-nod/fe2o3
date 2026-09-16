mod borrowed_source_authentication_tests {
    use super::*;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };

    const WORK_LIMIT: usize = 16_000_000;
    const STORAGE_LIMIT: usize = 64 * 1024 * 1024;
    const PREFIX: usize = 97;

    fn source_floor(source: &fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1) -> usize {
        PREFIX
            + source.executable_storage().retained_storage()
            + source.assert_origin_storage().payload_storage()
    }

    fn two_root_program() -> ProductionRankedSemanticProgramV1 {
        let seed = neutral_ranked_program_v1();
        let semantic = seed.materialized.semantic_ssa().source_semantic();
        let original = &semantic.functions()[0];
        let names = ["z_source_root", "a_source_root"];
        assert_eq!(semantic.functions().len(), 1);
        assert_eq!(semantic.callables().len(), 4);
        assert_eq!(
            semantic.callables()[0],
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0))
        );
        let mut callables = semantic.callables().to_vec();
        callables.insert(
            1,
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
        );
        let mut remapped_calls = 0;
        let blocks = original
            .blocks()
            .iter()
            .map(|block| {
                // This seed has only borrow statements and intrinsic Call terminators.
                for statement in block.statements() {
                    assert!(matches!(
                        statement.kind(),
                        SemanticStatementKindV1::Assign(assignment)
                            if matches!(assignment.value().kind(), SemanticRvalueKindV1::Borrow { .. })
                    ));
                }
                let kind = match block.terminator().kind() {
                    SemanticTerminatorKindV1::Call(call) => {
                        let old = call.callee().index() as usize;
                        assert!((1..semantic.callables().len()).contains(&old));
                        assert!(matches!(
                            &semantic.callables()[old],
                            SemanticCallableDeclV1::CompilerIntrinsic { .. }
                        ));
                        assert_eq!(&callables[old + 1], &semantic.callables()[old]);
                        remapped_calls += 1;
                        SemanticTerminatorKindV1::Call(
                            SemanticDirectCallV1::new_callable_with_variadic_argument_abis(
                                SemanticCallableIdV1::from_index(call.callee().index() + 1),
                                call.arguments().to_vec(),
                                call.variadic_argument_abis().to_vec(),
                                call.destination().cloned(),
                                call.unwind(),
                            )
                            .unwrap(),
                        )
                    }
                    SemanticTerminatorKindV1::Return => SemanticTerminatorKindV1::Return,
                    other => panic!("unexpected neutral fixture terminator: {other:?}"),
                };
                SemanticBasicBlockV1::new(
                    block.identity(),
                    block.source(),
                    block.statements().to_vec(),
                    SemanticTerminatorV1::new(block.terminator().source(), kind),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(remapped_calls, 3);
        let functions = names
            .iter()
            .enumerate()
            .map(|(index, name)| {
                SemanticFunctionDeclV1::new(
                    SemanticFunctionIdentityV1::from_sha256(bytes(231 + index as u8)),
                    original.role(),
                    SemanticItemDefinitionIdentityV1::from_sha256(bytes(233 + index as u8)),
                    SemanticMonomorphizationIdentityV1::from_sha256(bytes(235 + index as u8)),
                    original.generic_type_arguments_identity(),
                    original.const_generic_arguments_identity(),
                    original.source(),
                    original.abi().clone(),
                    original.locals().to_vec(),
                    original.entry(),
                    blocks.clone(),
                )
                .unwrap()
                .with_kernel_entry(SemanticKernelEntryV1::new(
                    SemanticLinkSymbolV1::new(name.as_bytes().to_vec()).unwrap(),
                    SemanticKernelBindingIdentityV1::from_sha256(bytes(247 - index as u8)),
                    original.kernel_entry().unwrap().source_contract(),
                ))
            })
            .collect();
        let admitted = InertSemanticMirRequestV1::new_with_callables(
            semantic.target(),
            semantic.types().to_vec(),
            vec![],
            vec![],
            vec![],
            functions,
            callables,
            vec![
                SemanticFunctionIdV1::from_index(0),
                SemanticFunctionIdV1::from_index(1),
            ],
        )
        .unwrap()
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
        let owner = ProductionSemanticMirOwnerV1::try_new(
            admitted,
            fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap();
        let ssa = ProductionSemanticSsaOwnerV1::try_new(
            owner,
            fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap();
        let inputs = [
            ranked_root_input_1d(names[0], 247, 64),
            ranked_root_input_1d(names[1], 246, 64),
        ];
        let materialized = materialize_ranked_fixture_v1(ssa, &inputs).unwrap();
        project_and_verify_ranked_materialized_semantic_mir_v1(
            materialized,
            &inputs,
            &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
        )
        .unwrap()
    }

    #[test]
    fn borrowed_source_scope_matches_historical_full_bytes_and_optional_custody() {
        for multiple in [false, true] {
            let fixture = || {
                if multiple {
                    two_root_program()
                } else {
                    neutral_ranked_program_v1()
                }
            };
            let legacy_receipt = fixture().into_verified_roster_receipt().unwrap();
            legacy_receipt.verify_equivalence().unwrap();
            let (legacy_receipt, legacy_verification) =
                legacy_receipt.into_module_verified_receipt().unwrap();
            let legacy = fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1::
                try_attach_materialized_ranked_checks(legacy_receipt).unwrap();
            let ProductionRankedSemanticProgramV1 {
                materialized,
                roots,
            } = fixture();
            let expected_roots = roots.len();
            let mut work = Work::new(WORK_LIMIT);
            let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
            budget.charge_work(7).unwrap();
            let floor = source_floor(&materialized);
            budget.reserve_storage(floor).unwrap();
            let ledger = std::ptr::from_ref(&budget);
            with_authenticated_borrowed_ranked_source_roster_v1(
                &materialized,
                roots,
                &mut budget,
                |correspondence, verification, budget| {
                    assert!(std::ptr::eq(correspondence.materialized(), &materialized));
                    assert!(std::ptr::eq(
                        correspondence.executable(),
                        materialized.executable()
                    ));
                    assert_eq!(std::ptr::from_ref(budget), ledger);
                    assert!(budget.storage() > floor);
                    assert_eq!(correspondence.root_count(), expected_roots);
                    assert_eq!(verification.root_count(), expected_roots);
                    assert_eq!(
                        verification.canonical_roster_identity(),
                        legacy_verification.canonical_roster_identity()
                    );
                    assert_eq!(
                        verification.canonical_kernel_order(),
                        legacy_verification.canonical_kernel_order()
                    );
                    if multiple {
                        assert_eq!(verification.canonical_kernel_order(), &[1, 0]);
                    }
                    for (index, ((name, report), expected)) in legacy
                        .mir_pliron_translation_validations()
                        .zip(legacy_verification.roots())
                        .enumerate()
                    {
                        let actual = &verification.roots()[index];
                        let root = &correspondence.roots()[index];
                        assert_eq!(root.selected_root(), actual.semantic_root());
                        assert_eq!(root.function_name(), name);
                        assert_eq!(root.launch_rank(), actual.source_rank());
                        assert_eq!(actual.semantic_root(), expected.semantic_root());
                        assert_eq!(actual.export_symbol(), expected.export_symbol());
                        assert_eq!(actual.kernel_binding(), expected.kernel_binding());
                        assert_eq!(correspondence.report(index), Some(report));
                        assert!(!report.claims_indexed_address_equivalence());
                        assert!(!report.claims_complete_operational_equivalence());
                        assert_eq!(
                            actual
                                .verification()
                                .middle_end_evidence()
                                .as_inert()
                                .canonical_bytes(),
                            expected
                                .verification()
                                .middle_end_evidence()
                                .as_inert()
                                .canonical_bytes()
                        );
                        assert_eq!(
                            actual.verification().semantic_u32_induction(),
                            expected.verification().semantic_u32_induction()
                        );
                        assert!(
                            !actual
                                .verification()
                                .has_authenticated_functional_verification()
                        );
                        assert!(
                            actual
                                .verification()
                                .retained_functional_verification_is_coherent()
                        );
                        assert!(actual.verification().aggregate_verus_execution().is_none());
                    }
                    assert!(correspondence.report(expected_roots).is_none());
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.failed_storage(), None);
        }
    }

    #[test]
    fn borrowed_source_scope_rejects_wrong_complete_root_rosters_before_callback() {
        for mutation in 0..7 {
            let ProductionRankedSemanticProgramV1 {
                materialized,
                roots,
            } = two_root_program();
            let mut roots = roots.into_vec();
            match mutation {
                0 => {
                    roots.pop();
                }
                1 => roots.swap(0, 1),
                2 => roots[1].semantic_root = roots[0].semantic_root,
                3 => roots[1].semantic_root_identity = roots[0].semantic_root_identity,
                4 => roots[1].kernel_binding = roots[0].kernel_binding,
                5 => roots[1].export_symbol[0] ^= 1,
                6 => roots[1].source_rank = 0,
                _ => unreachable!(),
            }
            let mut work = Work::new(WORK_LIMIT);
            let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
            budget.charge_work(7).unwrap();
            let floor = source_floor(&materialized);
            budget.reserve_storage(floor).unwrap();
            let mut called = false;
            let result = with_authenticated_borrowed_ranked_source_roster_v1(
                &materialized,
                roots.into_boxed_slice(),
                &mut budget,
                |_, _, _| {
                    called = true;
                    Ok(())
                },
            );
            assert!(matches!(
                result,
                Err(ProductionRankedVerificationErrorV1::RosterMetadata(_))
            ));
            assert!(!called);
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.work(), 7);
            assert_eq!(budget.failed_storage(), None);
        }
    }

    fn changed_generated_effect_program() -> ProductionRankedSemanticProgramV1 {
        let mut program = neutral_ranked_program_v1();
        let sources = &mut program.roots[0].executable_effect_sources;
        let old = sources[0];
        let mut recipe = old.recipe_identity();
        recipe[0] ^= 0x80;
        sources[0] = ProductionRankedExecutableEffectSourceV1::new(
            old.semantic_block(),
            old.semantic_effect_ordinal(),
            old.ranked_block(),
            old.ranked_operation(),
            old.origin(),
            recipe,
        );
        program
    }

    #[test]
    fn borrowed_shared_replay_rejects_real_cross_root_evidence_and_induction() {
        for induction_only in [false, true] {
            let ProductionRankedSemanticProgramV1 {
                materialized,
                roots,
            } = two_root_program();
            let (mut verified, order, identity) =
                authenticate_ranked_source_parts_v1(&materialized, roots).unwrap();
            verify_authenticated_ranked_source_parts_v1(&materialized, &verified, identity, &order)
                .unwrap();
            let (first, second) = verified.split_at_mut(1);
            if induction_only {
                std::mem::swap(
                    &mut first[0].verification.semantic_u32_induction,
                    &mut second[0].verification.semantic_u32_induction,
                );
            } else {
                std::mem::swap(&mut first[0].verification, &mut second[0].verification);
            }
            let error = verify_authenticated_ranked_source_parts_v1(
                &materialized,
                &verified,
                identity,
                &order,
            )
            .unwrap_err();
            let expected = if induction_only {
                "changed per-root semantic induction custody"
            } else {
                "changed per-root ranked verification custody"
            };
            assert!(
                matches!(error, ProductionRankedVerificationErrorV1::RosterMetadata(detail) if detail == expected)
            );
        }
    }

    #[test]
    fn borrowed_source_scope_preserves_generated_effect_rejection() {
        let historical = changed_generated_effect_program()
            .into_verified_roster_receipt()
            .and_then(ProductionRankedSemanticProjectionRosterReceiptV1::into_module_verified_receipt)
            .and_then(|(receipt, _)| {
                fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1::try_attach_materialized_ranked_checks(receipt)
                    .map_err(ProductionRankedVerificationErrorV1::Custody)
            })
            .err().expect("changed recipe must be rejected");
        let ProductionRankedSemanticProgramV1 {
            materialized,
            roots,
        } = changed_generated_effect_program();
        let mut work = Work::new(WORK_LIMIT);
        let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
        let floor = source_floor(&materialized);
        budget.reserve_storage(floor).unwrap();
        let mut called = false;
        let actual = with_authenticated_borrowed_ranked_source_roster_v1(
            &materialized,
            roots,
            &mut budget,
            |_, _, _| {
                called = true;
                Ok(())
            },
        )
        .unwrap_err();
        assert_eq!(actual.to_string(), historical.to_string());
        assert!(!called);
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.failed_storage(), None);
    }

    #[test]
    fn borrowed_source_scope_has_exact_shared_report_work_and_storage_boundaries() {
        use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
        use fe2o3_lower_mir_kernel::{
            ProductionBorrowedRankedCorrespondenceV1 as Scope,
            ProductionMirPlironTranslationValidationV1 as Report,
            ProductionSemanticKirErrorV1 as KirError,
            SemanticKirAssertOriginErrorV1 as OriginError,
        };
        // Wrapper-only work: entry 2, initial reserve 1, one visit 1, push 1.
        // Source authentication/translation retain their existing separate domain.
        const QUERY_WORK: usize = 2 + 1 + 1 + 1;
        for exact in [false, true] {
            let ProductionRankedSemanticProgramV1 {
                materialized,
                roots,
            } = neutral_ranked_program_v1();
            let floor = source_floor(&materialized);
            let mut work = Work::new(7 + QUERY_WORK - usize::from(!exact));
            let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
            budget.charge_work(7).unwrap();
            budget.reserve_storage(floor).unwrap();
            let mut called = false;
            let result = with_authenticated_borrowed_ranked_source_roster_v1(
                &materialized,
                roots,
                &mut budget,
                |_, _, budget| {
                    called = true;
                    assert_eq!(budget.work(), 7 + QUERY_WORK);
                    Ok(())
                },
            );
            assert_eq!(called, exact);
            if exact {
                result.unwrap();
            } else {
                assert!(matches!(
                    result,
                    Err(ProductionRankedVerificationErrorV1::Custody(
                        KirError::AssertOrigin(OriginError::Resource(Resource::Work(_)))
                    ))
                ));
            }
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.work(), 7 + QUERY_WORK - usize::from(!exact));
            assert_eq!(budget.failed_storage(), None);
            assert_eq!(work.failed_work(), (!exact).then_some(7 + QUERY_WORK));
        }

        let ProductionRankedSemanticProgramV1 {
            materialized,
            roots,
        } = neutral_ranked_program_v1();
        let floor = source_floor(&materialized);
        let header = std::mem::size_of::<Scope<'_>>() + std::mem::size_of::<Vec<Report>>();
        // The existing growth helper prepays its minimum capacity of four.
        let attempted = floor + header + 4 * std::mem::size_of::<Report>();
        let mut work = Work::new(WORK_LIMIT);
        let mut budget = Budget::new(&mut work, attempted - 1);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(floor).unwrap();
        let mut called = false;
        let result = with_authenticated_borrowed_ranked_source_roster_v1(
            &materialized,
            roots,
            &mut budget,
            |_, _, _| {
                called = true;
                Ok(())
            },
        );
        assert!(matches!(
            result,
            Err(ProductionRankedVerificationErrorV1::Custody(
                KirError::AssertOrigin(OriginError::Resource(Resource::Storage(_)))
            ))
        ));
        assert!(!called);
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.work(), 7 + 2 + 1);
        assert_eq!(budget.failed_storage(), Some(attempted));
    }

    #[test]
    fn borrowed_source_scope_retains_original_callback_error_panic_and_ledger() {
        for panic in [false, true] {
            let ProductionRankedSemanticProgramV1 {
                materialized,
                roots,
            } = neutral_ranked_program_v1();
            let mut work = Work::new(WORK_LIMIT);
            let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
            budget.charge_work(7).unwrap();
            let floor = source_floor(&materialized);
            budget.reserve_storage(floor).unwrap();
            let accepted = std::cell::Cell::new(0);
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                with_authenticated_borrowed_ranked_source_roster_v1::<()>(
                    &materialized,
                    roots,
                    &mut budget,
                    |_, _, budget| {
                        budget.charge_work(13).unwrap();
                        accepted.set(budget.work());
                        if panic {
                            std::panic::panic_any(271331_u32);
                        }
                        Err(ProductionRankedVerificationErrorV1::RosterMetadata(
                            "original R1 callback",
                        ))
                    },
                )
            }));
            if panic {
                assert_eq!(result.unwrap_err().downcast_ref::<u32>(), Some(&271331));
            } else {
                assert!(matches!(
                    result.unwrap(),
                    Err(ProductionRankedVerificationErrorV1::RosterMetadata(
                        "original R1 callback"
                    ))
                ));
            }
            assert!(accepted.get() > 20);
            assert_eq!(budget.work(), accepted.get());
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.failed_storage(), None);
            budget.charge_work(1).unwrap();
            assert_eq!(budget.work(), accepted.get() + 1);
        }
    }
}
