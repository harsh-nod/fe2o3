fn packet_j_source_bytes_v1(source: &ProductionPreRankedKirOwnerV1) -> usize {
    source.executable_storage().retained_storage()
        + source.assert_origin_storage().payload_storage()
}

fn packet_j_two_root_source_v1() -> (
    ProductionPreRankedKirOwnerV1,
    Vec<ProductionRankedRootInputV1>,
) {
    let seed = canonical_private_constant_store_source_v1();
    let semantic = seed.semantic_ssa().source_semantic();
    let original = &semantic.functions()[0];
    let contract = original.kernel_entry().unwrap().source_contract();
    let names = ["z_module_root", "a_module_root"];
    let functions = names
        .iter()
        .enumerate()
        .map(|(index, name)| {
            SemanticFunctionDeclV1::new(
                SemanticFunctionIdentityV1::from_sha256(bytes(237 + index as u8)),
                original.role(),
                SemanticItemDefinitionIdentityV1::from_sha256(bytes(242 + index as u8)),
                SemanticMonomorphizationIdentityV1::from_sha256(bytes(244 + index as u8)),
                original.generic_type_arguments_identity(),
                original.const_generic_arguments_identity(),
                original.source(),
                original.abi().clone(),
                original.locals().to_vec(),
                original.entry(),
                original.blocks().to_vec(),
            )
            .unwrap()
            .with_kernel_entry(SemanticKernelEntryV1::new(
                SemanticLinkSymbolV1::new(name.as_bytes().to_vec()).unwrap(),
                SemanticKernelBindingIdentityV1::from_sha256(bytes(247 + index as u8)),
                contract,
            ))
        })
        .collect();
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(250))),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        functions,
        (0..2)
            .map(|index| SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(index)))
            .collect(),
        (0..2).map(SemanticFunctionIdV1::from_index).collect(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let source = ProductionSemanticMirOwnerV1::try_new(
        admitted,
        fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap();
    let ssa = ProductionSemanticSsaOwnerV1::try_new(
        source,
        fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let inputs = names
        .iter()
        .enumerate()
        .map(|(index, name)| ranked_root_input_1d(name, 247 + index as u8, 64))
        .collect::<Vec<_>>();
    (materialize_ranked_fixture_v1(ssa, &inputs).unwrap(), inputs)
}

#[test]
fn packet_j_same_actual_policy3_output_reaches_complete_module_then_native_l() {
    use crate::production_pipeline::{
        lower_checked_output_native_text_v1 as lower, with_checked_output_memory_target_v1 as run,
    };
    let source = canonical_private_constant_store_source_v1();
    let inputs = [ranked_root_input_1d(A_NAME, 247, 64)];
    let references = crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default();
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
        budget.charge_work(7).unwrap();
        budget
            .reserve_storage(PREFIX + packet_j_source_bytes_v1(&source))
            .unwrap();
        let floor = budget.storage();
        let text = run(
            &source,
            &inputs,
            &references,
            profile,
            &mut budget,
            |bound, checked, formals, budget| {
                assert_eq!(checked.execution().policy_version(), 3);
                assert_eq!(
                    bound.canonical().canonical_bytes(),
                    checked.native_input_audit_bytes()
                );
                assert_ne!(
                    bound.canonical().canonical_bytes(),
                    checked.owner().canonical().canonical_bytes()
                );
                assert_eq!(formals.len(), 1);
                assert!(std::ptr::eq(formals[0].output(), checked.owner()));
                assert!(std::ptr::eq(
                    formals[0].kernel(),
                    &checked.owner().module().kernels[0]
                ));
                let before_lower = budget.storage();
                let text = lower(checked, profile)?;
                assert_eq!(budget.storage(), before_lower);
                let identity = checked.owner().canonical().identity();
                let digest = identity
                    .digest()
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>();
                let old_digest = bound
                    .canonical()
                    .identity()
                    .digest()
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>();
                let target = match profile {
                    Profile::Gfx942 => "gfx942:xnack-",
                    Profile::Gfx950 => "gfx950:xnack-",
                };
                let exact = format!(
                    "!\"sha256:{digest}\", !\"kir-version:12\", i64 {}, !\"target:{target}\"",
                    identity.canonical_length()
                );
                assert_eq!(text.matches(&exact).count(), 1);
                assert!(!text.contains(&format!("!\"sha256:{old_digest}\"")));
                Ok(text)
            },
        )
        .unwrap();
        assert!(text.contains("define"));
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.failed_storage(), None);
        assert!(budget.work() > 7);
        assert_eq!(work.failed_work(), None);
    }
}

#[test]
fn packet_j_complete_module_keeps_nonlexical_roots_and_rejects_incomplete_rosters() {
    use crate::production_pipeline::with_checked_output_memory_target_v1 as run;
    let (source, inputs) = packet_j_two_root_source_v1();
    let references = crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default();
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
        budget.charge_work(7).unwrap();
        budget
            .reserve_storage(PREFIX + packet_j_source_bytes_v1(&source))
            .unwrap();
        let floor = budget.storage();
        run(
            &source,
            &inputs,
            &references,
            profile,
            &mut budget,
            |_, checked, formals, _| {
                assert_eq!(formals.len(), 2);
                for (ordinal, formal) in formals.iter().enumerate() {
                    assert_eq!(
                        formal.selected_root(),
                        SemanticFunctionIdV1::from_index(ordinal as u32)
                    );
                    assert!(std::ptr::eq(formal.output(), checked.owner()));
                    assert_eq!(
                        formal.kernel().id.as_str(),
                        ["z_module_root", "a_module_root"][ordinal]
                    );
                    assert_eq!(formal.obligations().kernel(), &formal.kernel().id);
                }
                let text = crate::production_pipeline::lower_checked_output_native_text_v1(
                    checked, profile,
                )?;
                assert!(text.contains("!fe2o3.semantic_anchor.absence.v1"));
                assert!(!text.contains("!fe2o3.semantic_anchor.v1 ="));
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(budget.storage(), floor);
        for wrong in [
            vec![ranked_root_input_1d("z_module_root", 247, 64)],
            vec![
                ranked_root_input_1d("a_module_root", 248, 64),
                ranked_root_input_1d("z_module_root", 247, 64),
            ],
            vec![
                ranked_root_input_1d("z_module_root", 247, 64),
                ranked_root_input_1d("z_module_root", 247, 64),
            ],
        ] {
            let mut called = false;
            assert!(
                run(
                    &source,
                    &wrong,
                    &references,
                    profile,
                    &mut budget,
                    |_, _, _, _| {
                        called = true;
                        Ok(())
                    }
                )
                .is_err()
            );
            assert!(!called);
            assert_eq!(budget.storage(), floor);
        }
    }
}

#[test]
fn packet_j_binder_denial_and_callback_exits_preserve_original_ledger() {
    use crate::production_pipeline::{
        CheckedOutputMemoryTargetErrorV1 as CheckedError, ProductionPipelineError as Error,
        with_checked_output_memory_target_v1 as run,
    };
    use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
    let source = canonical_private_constant_store_source_v1();
    let inputs = [ranked_root_input_1d(A_NAME, 247, 64)];
    let references = crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default();
    let source_bytes = packet_j_source_bytes_v1(&source);
    // The shared production core's first charge is one, before binder invocation.
    {
        let mut work = Work::new(7);
        let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(PREFIX + source_bytes).unwrap();
        let floor = budget.storage();
        let mut called = false;
        let error = run(
            &source,
            &inputs,
            &references,
            Profile::Gfx942,
            &mut budget,
            |_, _, _, _| {
                called = true;
                Ok(())
            },
        )
        .unwrap_err();
        assert!(matches!(
            error,
            Error::CheckedOutputMemoryTarget(CheckedError::Resource(Resource::Work(_)))
        ));
        assert!(!called);
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.peak_storage(), floor);
        assert_eq!(budget.work(), 7);
        assert_eq!(work.failed_work(), Some(8));
    }
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(PREFIX + source_bytes).unwrap();
        let floor = budget.storage();
        let error = run(
            &source,
            &inputs,
            &references,
            profile,
            &mut budget,
            |_, _, _, _| Err::<(), _>(Error::EmptyCollectedDeviceClosure),
        )
        .unwrap_err();
        assert!(matches!(error, Error::EmptyCollectedDeviceClosure));
        assert_eq!(budget.storage(), floor);
        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run(
                &source,
                &inputs,
                &references,
                profile,
                &mut budget,
                |_, _, _, _| -> Result<(), Error> { std::panic::panic_any(271_330_u32) },
            )
        }))
        .unwrap_err();
        assert_eq!(unwind.downcast_ref::<u32>(), Some(&271_330));
        assert_eq!(budget.storage(), floor);
        budget.charge_work(1).unwrap();
        budget.reserve_storage(1).unwrap();
        budget.release_storage(1).unwrap();
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.failed_storage(), None);
        assert_eq!(work.failed_work(), None);
    }
}

#[test]
fn packet_j_dynamic_indices_cannot_be_relabelled_complete() {
    use crate::production_pipeline::with_checked_output_memory_target_v1 as run;
    let source = canonical_same_typed_tuple_store_source_v1(1, BodyShape::Straight);
    let inputs = [ranked_root_input_1d(A_NAME, 247, 1)];
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
    budget
        .reserve_storage(PREFIX + packet_j_source_bytes_v1(&source))
        .unwrap();
    let floor = budget.storage();
    let mut called = false;
    let result = run(
        &source,
        &inputs,
        &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
        Profile::Gfx942,
        &mut budget,
        |_, _, _, _| {
            called = true;
            Ok(())
        },
    );
    assert!(matches!(
        result,
        Err(
            crate::production_pipeline::ProductionPipelineError::CheckedOutputMemoryTarget(
                crate::production_pipeline::CheckedOutputMemoryTargetErrorV1::Join(
                    CheckedOutputModuleJoinErrorV1::Formal(
                        fe2o3_lower_mir_kernel::ProductionScopedFormalMemoryErrorV1::Formal(_)
                    )
                )
            )
        )
    ));
    assert!(!called);
    assert_eq!(budget.storage(), floor);
}

fn packet_j_later_incomplete_source_v1() -> (
    ProductionPreRankedKirOwnerV1,
    Vec<ProductionRankedRootInputV1>,
) {
    let seed = canonical_same_typed_tuple_store_source_v1(0, BodyShape::Straight);
    let semantic = seed.semantic_ssa().source_semantic();
    let dynamic = &semantic.functions()[0];
    let private = canonical_private_constant_store_source_v1();
    let private_semantic = private.semantic_ssa().source_semantic();
    assert_eq!(
        &semantic.types()[..private_semantic.types().len()],
        private_semantic.types(),
    );
    let private_root = &private_semantic.functions()[0];
    let complete = private_root
        .clone()
        .with_kernel_entry(SemanticKernelEntryV1::new(
            SemanticLinkSymbolV1::new(b"complete_module_root".to_vec()).unwrap(),
            SemanticKernelBindingIdentityV1::from_sha256(bytes(245)),
            private_root.kernel_entry().unwrap().source_contract(),
        ));
    let contract = dynamic.kernel_entry().unwrap().source_contract();
    let incomplete = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(254)),
        dynamic.role(),
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(253)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(252)),
        dynamic.generic_type_arguments_identity(),
        dynamic.const_generic_arguments_identity(),
        dynamic.source(),
        dynamic.abi().clone(),
        dynamic.locals().to_vec(),
        dynamic.entry(),
        dynamic.blocks().to_vec(),
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"incomplete_module_root".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256(bytes(246)),
        contract,
    ));
    let mut functions = semantic.functions().to_vec();
    functions[0] = complete;
    let later = functions.len() as u32;
    functions.push(incomplete);
    let callables = (0..functions.len())
        .map(|ordinal| {
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(ordinal as u32))
        })
        .collect();
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(250))),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        functions,
        callables,
        vec![
            SemanticFunctionIdV1::from_index(0),
            SemanticFunctionIdV1::from_index(later),
        ],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let source = ProductionSemanticMirOwnerV1::try_new(
        admitted,
        fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap();
    let ssa = ProductionSemanticSsaOwnerV1::try_new(
        source,
        fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let inputs = vec![
        ranked_root_input_1d("complete_module_root", 245, 64),
        ranked_root_input_1d("incomplete_module_root", 246, 1),
    ];
    (materialize_ranked_fixture_v1(ssa, &inputs).unwrap(), inputs)
}

#[test]
fn packet_j_later_incomplete_root_prevents_whole_module_callback() {
    use fe2o3_lower_mir_kernel::{
        ProductionFormalMemoryErrorV1 as Formal, ProductionScopedFormalMemoryErrorV1 as Error,
    };
    let (source, inputs) = packet_j_later_incomplete_source_v1();
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        with_formal_completed_roots_v1(&source, profile, true, &inputs, |roots, budget| {
            assert_eq!(roots.len(), 2);
            roots[0].with_complete_formal_memory_v1(budget, |formal, _| {
                // The first root is genuinely Complete because its actual
                // ordinary Store is Private C, not a claimed Global literal.
                let function = formal.output().module().function(&formal.kernel().entry).unwrap();
                let body = function.body.as_ref().unwrap();
                let mut private_stores = 0;
                for operation in body.blocks.iter().flat_map(|block| &block.operations) {
                    if let fe2o3_kernel_ir::OperationKind::Store { access, .. } = &operation.kind {
                        assert_eq!(access.address_space, fe2o3_kernel_ir::AddressSpace::Private);
                        private_stores += 1;
                    }
                }
                assert_eq!(private_stores, 1);
                assert!(formal.obligations().accesses().is_empty());
                Ok(())
            }).unwrap();
            let floor = budget.storage();
            let mut called = false;
            let result = fe2o3_lower_mir_kernel::with_complete_formal_memory_module_v1(
                roots, budget, |_, _| { called = true; Ok(()) });
            assert!(matches!(result, Err(Error::Formal(Formal::Incomplete { ref reasons }))
                if reasons.iter().any(|reason| matches!(reason,
                    fe2o3_kernel_ir::FormalMemoryIncompleteReason::UnsupportedIndexExpression { .. }))));
            assert!(!called);
            assert_eq!(budget.storage(), floor);
            Ok(())
        }).unwrap();
    }
}

#[test]
fn packet_j_genuine_helper_keeps_source_and_kernel_domains_distinct() {
    let source = scoped_formal_helper_before_entry_source_v1();
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
        budget
            .reserve_storage(PREFIX + packet_j_source_bytes_v1(&source))
            .unwrap();
        let floor = budget.storage();
        crate::production_pipeline::with_checked_output_memory_target_v1(
            &source,
            &[ranked_root_input_1d(A_NAME, 247, 64)],
            &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
            profile,
            &mut budget,
            |_, checked, formals, _| {
                let [formal] = formals else {
                    panic!("one complete root")
                };
                assert_eq!(formal.selected_root(), SemanticFunctionIdV1::from_index(1));
                assert!(std::ptr::eq(
                    formal.kernel(),
                    &checked.owner().module().kernels[0]
                ));
                assert_eq!(checked.owner().module().functions.len(), 2);
                let llvm = crate::production_pipeline::lower_checked_output_native_text_v1(
                    checked, profile,
                )?;
                assert!(llvm.contains("!fe2o3.semantic_anchor.absence.v1"));
                assert!(!llvm.contains("!fe2o3.semantic_anchor.v1 ="));
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn packet_j_whole_module_requires_all_scoped_rows_and_original_ledger() {
    use fe2o3_lower_mir_kernel::{
        ProductionScopedFormalMemoryErrorV1 as Error, ProductionSourceOutputErrorV1 as Output,
    };
    let (source, inputs) = packet_j_two_root_source_v1();
    with_formal_completed_roots_v1(&source, Profile::Gfx942, true, &inputs, |roots, budget| {
        let floor = budget.storage();
        for partial in [&roots[..0], &roots[..1]] {
            let mut called = false;
            let result = fe2o3_lower_mir_kernel::with_complete_formal_memory_module_v1(
                partial,
                budget,
                |_, _| {
                    called = true;
                    Ok(())
                },
            );
            assert!(matches!(
                result,
                Err(Error::SourceOutput(Output::Invalid(_)))
            ));
            assert!(!called);
            assert_eq!(budget.storage(), floor);
        }
        let mut work = Work::new(LIMIT);
        let mut foreign = Budget::new(&mut work, STORAGE_LIMIT);
        foreign.reserve_storage(floor).unwrap();
        let mut called = false;
        let result = fe2o3_lower_mir_kernel::with_complete_formal_memory_module_v1(
            roots,
            &mut foreign,
            |_, _| {
                called = true;
                Ok(())
            },
        );
        assert!(matches!(
            result,
            Err(Error::SourceOutput(Output::Resource(
                fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting
            )))
        ));
        assert!(!called);
        assert_eq!(foreign.storage(), floor);
        assert_eq!(budget.storage(), floor);
        Ok(())
    })
    .unwrap();
}

#[test]
fn packet_j_core_has_one_binder_and_one_fixed_optimizer_callsite() {
    let source = include_str!("../production_checked_output_pipeline_v1.rs");
    assert_eq!(source.matches("bind_production_target_v1(").count(), 1);
    assert_eq!(
        source
            .matches("optimize_native_neutral_kernel_ir_policy3_v1(")
            .count(),
        1
    );
    assert_eq!(source.matches("try_check_and_finish_v1(").count(), 1);
    assert!(!source.contains("optimize_production_kernel_ir_module_v2("));
    assert!(!source.contains("optimize_production_kernel_ir_module_v3("));
    assert!(!source.contains("target_module.clone()"));
}
