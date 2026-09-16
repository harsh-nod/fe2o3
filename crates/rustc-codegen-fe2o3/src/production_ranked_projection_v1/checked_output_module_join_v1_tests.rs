fn checked_output_descriptor_argument_source_v1(
    source: ProductionPreRankedKirOwnerV1,
    inputs: &[ProductionRankedRootInputV1],
) -> ProductionPreRankedKirOwnerV1 {
    // The typed compiler descriptor contract requires a nonempty argument
    // list. Re-admit a real unused scalar formal; do not relax that contract.
    let semantic = source.semantic_ssa().source_semantic();
    let mut functions = semantic.functions().to_vec();
    for root in semantic.roots() {
        let function = &functions[root.index() as usize];
        let abi = function.abi();
        assert!(abi.source_input_types().is_empty());
        let abi = SemanticFunctionAbiV1::from_rustc(
            abi.identity(),
            abi.layout_identity(),
            abi.canon_abi(),
            abi.extern_abi(),
            abi.can_unwind(),
            abi.c_variadic(),
            1,
            vec![SemanticAbiArgumentV1::source(
                neutral_plain_direct_abi_value_v1(A_U32),
            )],
            abi.return_value().clone(),
        )
        .unwrap()
        .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue])
        .unwrap();
        let mut locals = function.locals().to_vec();
        locals.push(local(250, A_U32, SemanticLocalRoleV1::Argument(0)));
        functions[root.index() as usize] =
            ordinary_rebuild_v1(function, abi, locals, function.blocks().to_vec());
    }
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        functions,
        semantic.callables().to_vec(),
        semantic.roots().to_vec(),
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
    materialize_ranked_fixture_v1(ssa, inputs).unwrap()
}

fn checked_output_descriptor_fixture_roots_v1(
    source: &ProductionPreRankedKirOwnerV1,
    inputs: &[ProductionRankedRootInputV1],
) -> Vec<crate::compiler_descriptor::TypedDescriptorRootV1> {
    let semantic = source.semantic_ssa().source_semantic();
    assert_eq!(semantic.roots().len(), inputs.len());
    semantic
        .roots()
        .iter()
        .zip(inputs)
        .map(|(id, input)| {
            crate::compiler_descriptor::checked_output_scalar_descriptor_fixture_v1(
                &semantic.functions()[id.index() as usize],
                &semantic.types()[A_U32.index() as usize],
                &input.logical_name,
                &input.source_launch,
            )
        })
        .collect()
}

#[test]
fn source_descriptor_actual_r1_policy3_formals_and_native_text_stay_on_one_owner() {
    use crate::production_pipeline::{
        with_checked_output_descriptor_text_v1 as descriptor,
        with_checked_output_memory_target_v1 as run,
    };
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for shape in 0..3 {
            let (source, inputs) = match shape {
                0 => (
                    canonical_private_constant_store_source_v1(),
                    vec![ranked_root_input_1d(A_NAME, 247, 64)],
                ),
                1 => packet_j_two_root_source_v1(),
                _ => (
                    scoped_formal_helper_before_entry_source_v1(),
                    vec![ranked_root_input_1d(A_NAME, 247, 64)],
                ),
            };
            let source = checked_output_descriptor_argument_source_v1(source, &inputs);
            let typed = checked_output_descriptor_fixture_roots_v1(&source, &inputs);
            let references =
                crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default();
            let ProductionRankedSemanticProgramV1 {
                materialized,
                roots,
            } = project_and_verify_ranked_materialized_semantic_mir_v1(
                source,
                &inputs,
                &references,
            )
            .unwrap();
            let mut work = Work::new(LIMIT);
            let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
            budget
                .reserve_storage(PREFIX + packet_j_source_bytes_v1(&materialized))
                .unwrap();
            let floor = budget.storage();
            with_authenticated_borrowed_ranked_source_roster_v1(
                &materialized,
                roots,
                &mut budget,
                |correspondence, verification, budget| {
                    assert!(verification.roots().iter().all(|root| {
                        !root
                            .verification()
                            .has_authenticated_functional_verification()
                    }));
                    Ok(run(
                        &materialized,
                        &inputs,
                        &references,
                        profile,
                        budget,
                        |_, checked, formals, budget| {
                            let before = budget.storage();
                            let identity = checked.owner().canonical().identity();
                            descriptor(
                                correspondence,
                                checked,
                                formals,
                                &typed,
                                profile,
                                fe2o3_compiler_ffi::DeviceTargetV1::parse(profile.device_target())
                                    .unwrap(),
                                None,
                                budget,
                                |text, source, budget| {
                                    assert_eq!(
                                        text.descriptor_source_identity(),
                                        Some(source.identity())
                                    );
                                    assert_eq!(source.table().kernels().len(), inputs.len());
                                    assert!(!source.authenticates_compiler_origin());
                                    assert!(!source.grants_link_authority());
                                    assert!(!source.grants_launch_authority());
                                    let decoded =
                                        fe2o3_compiler_ffi::CompilerDescriptorSourceV1::decode(
                                            source.canonical_bytes(),
                                        )
                                        .unwrap();
                                    assert_eq!(decoded.canonical_bytes(), source.canonical_bytes());
                                    for formal in formals {
                                        let descriptor = source.table().kernels().iter().find(|kernel|
                                            kernel.entry_name().as_str() == formal.kernel().id.as_str()).unwrap();
                                        assert_eq!(
                                            descriptor.entry_name().as_str(),
                                            formal.kernel().id.as_str()
                                        );
                                        assert_eq!(
                                            descriptor
                                                .descriptor_symbol()
                                                .as_str()
                                                .strip_suffix(".kd"),
                                            Some(descriptor.entry_name().as_str())
                                        );
                                        assert!(std::ptr::eq(formal.output(), checked.owner()));
                                        let query_start = budget.work();
                                        formal
                                            .require_borrowed_source_v1(correspondence, budget)
                                            .unwrap();
                                        // Store floor4 + analysis1 + control14 + N pointer1.
                                        assert_eq!(budget.work() - query_start, 20);
                                        let mut alien_work = Work::new(20);
                                        let mut alien = Budget::new(&mut alien_work, budget.storage());
                                        alien.reserve_storage(budget.storage()).unwrap();
                                        assert!(matches!(formal.require_borrowed_source_v1(correspondence, &mut alien),
                                            Err(fe2o3_lower_mir_kernel::ProductionSourceOutputErrorV1::Resource(
                                                fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting
                                            ))));
                                        assert_eq!(alien.work(), 19);
                                        assert_eq!(budget.work() - query_start, 20);
                                    }
                                    let digest = identity
                                        .digest()
                                        .iter()
                                        .map(|byte| format!("{byte:02x}"))
                                        .collect::<String>();
                                    assert!(text.llvm_ir().contains(&format!("sha256:{digest}")));
                                    assert!(text.llvm_ir().contains("module asm \".byte "));
                                    if shape != 0 {
                                        assert!(
                                            text.llvm_ir()
                                                .contains("!fe2o3.semantic_anchor.absence.v1")
                                        );
                                    }
                                    Ok(())
                                },
                            )?;
                            assert_eq!(budget.storage(), before);
                            Ok(())
                        },
                    ))
                },
            )
            .unwrap()
            .unwrap();
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.failed_storage(), None);
        }
    }
}

#[test]
fn source_descriptor_rejects_foreign_r1_owner_even_when_bytes_and_root_ids_match() {
    use crate::production_pipeline::{
        with_checked_output_descriptor_text_v1 as descriptor,
        with_checked_output_memory_target_v1 as run,
    };
    let source = canonical_private_constant_store_source_v1();
    let inputs = [ranked_root_input_1d(A_NAME, 247, 64)];
    let source = checked_output_descriptor_argument_source_v1(source, &inputs);
    let typed = checked_output_descriptor_fixture_roots_v1(&source, &inputs);
    let references = crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default();
    let ProductionRankedSemanticProgramV1 {
        materialized: other,
        roots,
    } = project_and_verify_ranked_materialized_semantic_mir_v1(
        checked_output_descriptor_argument_source_v1(
            canonical_private_constant_store_source_v1(),
            &inputs,
        ),
        &inputs,
        &references,
    )
    .unwrap();
    assert_eq!(
        source.executable().canonical().canonical_bytes(),
        other.executable().canonical().canonical_bytes()
    );
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
    budget
        .reserve_storage(
            PREFIX + packet_j_source_bytes_v1(&source) + packet_j_source_bytes_v1(&other),
        )
        .unwrap();
    let floor = budget.storage();
    with_authenticated_borrowed_ranked_source_roster_v1(&other, roots, &mut budget, |correspondence, _, budget| {
        Ok(run(&source, &inputs, &references, Profile::Gfx942, budget, |_, checked, formals, budget| {
            let before = budget.storage();
            let mut called = false;
            let error = descriptor::<()>(correspondence, checked, formals, &typed, Profile::Gfx942,
                fe2o3_compiler_ffi::DeviceTargetV1::parse("gfx942:xnack-").unwrap(), None, budget,
                |_, _, _| { called = true; Ok(()) }).unwrap_err();
            assert!(matches!(error, crate::production_pipeline::ProductionPipelineError::DescriptorEvidence(
                crate::compiler_descriptor::CompilerDescriptorError::CheckedOutputSource(
                    fe2o3_lower_mir_kernel::ProductionSourceOutputErrorV1::Invalid(
                        "formal analysis and borrowed correspondence have different source owners"
                    )
                )
            )));
            assert!(!called);
            assert_eq!(budget.storage(), before);
            Ok(())
        }))
    }).unwrap().unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn source_descriptor_rejects_missing_roots_foreign_output_and_target() {
    use crate::production_pipeline::{
        with_checked_output_descriptor_text_v1 as descriptor,
        with_checked_output_memory_target_v1 as run,
    };
    let (source, inputs) = packet_j_two_root_source_v1();
    let source = checked_output_descriptor_argument_source_v1(source, &inputs);
    let typed = checked_output_descriptor_fixture_roots_v1(&source, &inputs);
    let references = crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default();
    let ProductionRankedSemanticProgramV1 {
        materialized,
        roots,
    } = project_and_verify_ranked_materialized_semantic_mir_v1(source, &inputs, &references)
        .unwrap();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
    budget
        .reserve_storage(PREFIX + packet_j_source_bytes_v1(&materialized))
        .unwrap();
    let floor = budget.storage();
    with_authenticated_borrowed_ranked_source_roster_v1(&materialized, roots, &mut budget, |correspondence, _, budget| {
        Ok(run(&materialized, &inputs, &references, Profile::Gfx942, budget, |_, checked, formals, budget| {
            let before = budget.storage();
            let target = fe2o3_compiler_ffi::DeviceTargetV1::parse("gfx942:xnack-").unwrap();
            let mut reversed = typed.clone();
            reversed.reverse();
            for (rows, roots) in [(&formals[..1], &typed[..]), (formals, &typed[..1]), (formals, &reversed[..])] {
                let mut called = false;
                let error = descriptor(correspondence, checked, rows, roots, Profile::Gfx942, target, None, budget,
                    |_, _, _| { called = true; Ok(()) }).unwrap_err();
                assert!(matches!(error, crate::production_pipeline::ProductionPipelineError::DescriptorEvidence(
                    crate::compiler_descriptor::CompilerDescriptorError::ProductionDescriptorMismatch(_)
                )));
                assert!(!called);
                assert_eq!(budget.storage(), before);
            }
            use crate::compiler_descriptor::{CheckedOutputDescriptorFixtureMutationV1 as Mutation, checked_output_descriptor_mutated_fixture_v1 as mutate};
            for mutation in [Mutation::TypeIdentity, Mutation::Ownership, Mutation::Abi, Mutation::Layout] {
                let mut invalid = typed.clone();
                invalid[0] = mutate(&invalid[0], mutation);
                let error = descriptor::<()>(correspondence, checked, formals, &invalid, Profile::Gfx942, target, None, budget,
                    |_, _, _| panic!("invalid source ABI metadata reached callback")).unwrap_err();
                assert!(matches!(error, crate::production_pipeline::ProductionPipelineError::DescriptorEvidence(
                    crate::compiler_descriptor::CompilerDescriptorError::ProductionDescriptorMismatch(
                        "rustc semantic argument type identity" | "rustc semantic argument layout/ownership"
                    )
                )));
                assert_eq!(budget.storage(), before);
            }
            let error = descriptor::<()>(correspondence, checked, formals, &typed, Profile::Gfx942,
                fe2o3_compiler_ffi::DeviceTargetV1::parse("gfx950:xnack-").unwrap(), None, budget,
                |_, _, _| panic!("foreign target reached descriptor callback")).unwrap_err();
            assert!(matches!(error, crate::production_pipeline::ProductionPipelineError::WorkerHandoff(
                crate::production_worker_handoff::ProductionWorkerHandoffError::TargetBindingMismatch { .. }
            )));
            run(&materialized, &inputs, &references, Profile::Gfx942, budget, |_, other_checked, _, budget| {
                assert_eq!(other_checked.owner().canonical().canonical_bytes(), checked.owner().canonical().canonical_bytes());
                let error = descriptor::<()>(correspondence, other_checked, formals, &typed, Profile::Gfx942, target, None, budget,
                    |_, _, _| panic!("foreign O reached descriptor callback")).unwrap_err();
                assert!(matches!(error, crate::production_pipeline::ProductionPipelineError::DescriptorEvidence(
                    crate::compiler_descriptor::CompilerDescriptorError::ProductionDescriptorMismatch("checked-output source/owner/root identity")
                )));
                Ok(())
            })?;
            assert_eq!(budget.storage(), before);
            Ok(())
        }))
    }).unwrap().unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn source_descriptor_callback_error_unwind_and_accounting_keep_the_original_scope() {
    use crate::production_pipeline::{
        ProductionPipelineError as Error, with_checked_output_descriptor_text_v1 as descriptor,
        with_checked_output_memory_target_v1 as run,
    };
    for exit in 0..4 {
        let source = canonical_private_constant_store_source_v1();
        let inputs = [ranked_root_input_1d(A_NAME, 247, 64)];
        let source = checked_output_descriptor_argument_source_v1(source, &inputs);
        let typed = checked_output_descriptor_fixture_roots_v1(&source, &inputs);
        let references =
            crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default();
        let ProductionRankedSemanticProgramV1 {
            materialized,
            roots,
        } = project_and_verify_ranked_materialized_semantic_mir_v1(source, &inputs, &references)
            .unwrap();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
        budget
            .reserve_storage(PREFIX + packet_j_source_bytes_v1(&materialized))
            .unwrap();
        with_authenticated_borrowed_ranked_source_roster_v1(&materialized, roots, &mut budget, |correspondence, _, budget| {
            Ok(run(&materialized, &inputs, &references, Profile::Gfx942, budget, |_, checked, formals, budget| {
                let before = budget.storage();
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    descriptor::<()>(correspondence, checked, formals, &typed, Profile::Gfx942,
                        fe2o3_compiler_ffi::DeviceTargetV1::parse("gfx942:xnack-").unwrap(), None, budget,
                        |_, _, budget| {
                            if exit >= 2 { budget.release_storage(1).unwrap(); }
                            if exit % 2 == 0 { Err(Error::ExtractionCannotPublish) }
                            else { std::panic::panic_any(173_u32) }
                        })
                }));
                match exit {
                    0 => assert!(matches!(outcome.unwrap(), Err(Error::ExtractionCannotPublish))),
                    1 => assert_eq!(*outcome.unwrap_err().downcast::<u32>().unwrap(), 173),
                    _ => {
                        assert!(matches!(outcome.unwrap(), Err(Error::CheckedOutputMemoryTarget(
                            crate::production_pipeline::CheckedOutputMemoryTargetErrorV1::Resource(
                                fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting
                            )
                        ))));
                        // The hostile callback is not allowed to corrupt the
                        // enclosing genuine scope after this refusal is checked.
                        assert_eq!(budget.storage(), before - 1);
                        budget.reserve_storage(1).unwrap();
                    }
                }
                assert_eq!(budget.storage(), before);
                Ok(())
            }))
        }).unwrap().unwrap();
    }
}

#[test]
fn source_descriptor_postflight_work_preserves_mandatory_failure_precedence() {
    use crate::production_pipeline::{
        ProductionPipelineError as Error, with_checked_output_descriptor_text_v1 as descriptor,
        with_checked_output_memory_target_v1 as run,
    };
    // This is the isolated live postflight query cost, not a calibrated bound
    // for materialization, R1, optimization, descriptor construction or LLVM.
    const QUERY: usize = 4 + 1 + 14 + 1;
    for exact in [true, false] {
        for panic in [false, true] {
            let inputs = [ranked_root_input_1d(A_NAME, 247, 64)];
            let source = checked_output_descriptor_argument_source_v1(
                canonical_private_constant_store_source_v1(),
                &inputs,
            );
            let typed = checked_output_descriptor_fixture_roots_v1(&source, &inputs);
            let references =
                crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default();
            let ProductionRankedSemanticProgramV1 {
                materialized,
                roots,
            } = project_and_verify_ranked_materialized_semantic_mir_v1(
                source,
                &inputs,
                &references,
            )
            .unwrap();
            let mut work = Work::new(LIMIT);
            let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
            budget
                .reserve_storage(PREFIX + packet_j_source_bytes_v1(&materialized))
                .unwrap();
            let floor = budget.storage();
            let mut reached = false;
            let result = with_authenticated_borrowed_ranked_source_roster_v1(&materialized, roots, &mut budget, |correspondence, _, budget| {
                Ok(run(&materialized, &inputs, &references, Profile::Gfx942, budget, |_, checked, formals, budget| {
                    let before = budget.storage();
                    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        descriptor::<()>(correspondence, checked, formals, &typed, Profile::Gfx942,
                            fe2o3_compiler_ffi::DeviceTargetV1::parse("gfx942:xnack-").unwrap(), None, budget,
                            |_, _, budget| {
                                reached = true;
                                // Exhaust only the unused allowance, on the
                                // same ledger without reset or replacement.
                                let margin = QUERY - usize::from(!exact);
                                budget.charge_work(LIMIT.checked_sub(budget.work()).unwrap().checked_sub(margin).unwrap()).unwrap();
                                if panic { std::panic::panic_any(177_u32) }
                                else { Err(Error::ExtractionCannotPublish) }
                            })
                    }));
                    if !exact {
                        assert!(matches!(outcome.unwrap(), Err(Error::DescriptorEvidence(
                            crate::compiler_descriptor::CompilerDescriptorError::CheckedOutputSource(
                                fe2o3_lower_mir_kernel::ProductionSourceOutputErrorV1::Resource(
                                    fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Work(_)
                                )
                            )
                        ))));
                    } else if panic {
                        assert_eq!(*outcome.unwrap_err().downcast::<u32>().unwrap(), 177);
                    } else {
                        assert!(matches!(outcome.unwrap(), Err(Error::ExtractionCannotPublish)));
                    }
                    assert_eq!(budget.work(), LIMIT);
                    assert_eq!(budget.storage(), before);
                    Ok(())
                }))
            }).unwrap();
            // The outer genuine scope also performs mandatory live queries;
            // it cannot return success after the component consumed the cap.
            assert!(result.is_err());
            assert!(reached);
            assert_eq!(budget.storage(), floor);
        }
    }
}

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

fn formal_literal_tuple_store_source_v1(
    bits: u64,
    invocations: u32,
) -> ProductionPreRankedKirOwnerV1 {
    let seed = canonical_same_typed_tuple_store_source_v1(0, BodyShape::Straight);
    let semantic = seed.semantic_ssa().source_semantic();
    let root = &semantic.functions()[0];
    let mut blocks = root.blocks().to_vec();
    let guard = &blocks[2];
    let mut statements = vec![typed_assignment(
        8,
        A_U64,
        SemanticRvalueKindV1::Use(typed_constant(A_U64, u128::from(bits), 8)),
    )];
    statements.extend_from_slice(guard.statements());
    blocks[2] = SemanticBasicBlockV1::new(
        guard.identity(),
        guard.source(),
        statements,
        guard.terminator().clone(),
    )
    .unwrap();
    let mut locals = root.locals().to_vec();
    assert_eq!(locals[8].role(), SemanticLocalRoleV1::Argument(3));
    locals[8] = SemanticLocalDeclV1::new(
        locals[8].identity(),
        A_U64,
        SemanticLocalRoleV1::Temporary,
        locals[8].source(),
    );
    let old = root.abi();
    assert_eq!(old.fixed_count(), 4);
    let abi = SemanticFunctionAbiV1::from_rustc(
        old.identity(),
        old.layout_identity(),
        old.canon_abi(),
        old.extern_abi(),
        old.can_unwind(),
        old.c_variadic(),
        3,
        old.arguments()[..3].to_vec(),
        old.return_value().clone(),
    )
    .unwrap()
    .with_source_argument_ownership(old.source_argument_ownership()[..3].to_vec())
    .unwrap();
    let dimensions = SemanticWorkgroupDimensionsV1::new([invocations, 1, 1]).unwrap();
    let contract = SemanticKernelSourceContractV1::new(
        Some(SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None).unwrap()),
        None,
        None,
    )
    .unwrap();
    let mut functions = semantic.functions().to_vec();
    functions[0] = ordinary_rebuild_v1(root, abi, locals, blocks).with_kernel_entry(
        SemanticKernelEntryV1::new(
            SemanticLinkSymbolV1::new(A_NAME.as_bytes().to_vec()).unwrap(),
            SemanticKernelBindingIdentityV1::from_sha256(bytes(247)),
            contract,
        ),
    );
    materialize_ranked_fixture_v1(
        assertion_ssa_functions(semantic.types().to_vec(), functions),
        &[ranked_root_input_1d(A_NAME, 247, invocations)],
    )
    .unwrap()
}

// Fixture oracle on the actual O own-use chain, independent of the affine cache.
// The original tuple helper result and the source assertion remain executable.
fn require_actual_literal_global_offset_v1(
    output: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    bits: u64,
) -> fe2o3_kernel_ir::FunctionOperationLocation {
    use fe2o3_kernel_ir::{
        AddressSpace, CastKind, Constant, OperationKind, ScalarType, Terminator, Type,
    };
    let module = output.module();
    assert_eq!(module.kernels.len(), 1);
    let function = module.function(&module.kernels[0].entry).unwrap();
    let body = function.body.as_ref().unwrap();
    let definition = |value| {
        body.blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find(|operation| operation.results.iter().any(|result| result.id == value))
            .unwrap()
    };
    let stores = body
        .blocks
        .iter()
        .flat_map(|block| {
            block.operations.iter().enumerate().filter_map(
                move |(index, operation)| match operation.kind {
                    OperationKind::Store {
                        pointer, access, ..
                    } if access.address_space == AddressSpace::Global => {
                        Some((block.id, index, pointer))
                    }
                    _ => None,
                },
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(stores.len(), 1);
    let &(block, operation, mut pointer) = stores.first().unwrap();
    let location = fe2o3_kernel_ir::FunctionOperationLocation::new(block, operation);
    let mut offset = None;
    for _ in 0..8 {
        match definition(pointer).kind {
            OperationKind::Cast {
                kind: CastKind::RestrictPointerAccess,
                value,
                ..
            } => pointer = value,
            OperationKind::GetElementPointer { offset: value, .. } => {
                offset = Some(value);
                break;
            }
            _ => panic!("unexpected literal fixture pointer chain"),
        }
    }
    let cast = definition(offset.expect("actual GEP"));
    let OperationKind::Cast {
        kind: CastKind::Bitcast,
        mut value,
        ref to,
    } = cast.kind
    else {
        panic!("source U64 index must remain the actual INDEX bitcast");
    };
    assert_eq!(to, &Type::INDEX);
    assert_eq!(cast.results.len(), 1);
    assert_eq!(cast.results[0].ty, Type::INDEX);
    for _ in 0..128 {
        if let Some(operation) = body
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find(|operation| operation.results.iter().any(|result| result.id == value))
        {
            assert_eq!(operation.results.len(), 1);
            assert_eq!(operation.results[0].ty, Type::Scalar(ScalarType::U64));
            assert!(
                matches!(operation.kind, OperationKind::Constant(Constant::U64(actual)) if actual == bits)
            );
            return location;
        }
        let (block, ordinal) = body
            .blocks
            .iter()
            .find_map(|block| {
                block
                    .parameters
                    .iter()
                    .position(|parameter| parameter.id == value)
                    .map(|ordinal| (block, ordinal))
            })
            .expect("literal value must have an actual definition or block parameter");
        assert_eq!(block.parameters[ordinal].ty, Type::Scalar(ScalarType::U64));
        let mut incoming = Vec::new();
        for predecessor in &body.blocks {
            match predecessor.terminator.as_ref().unwrap() {
                Terminator::Branch { target, arguments } if *target == block.id => {
                    incoming.push(arguments[ordinal])
                }
                Terminator::ConditionalBranch {
                    then_target,
                    then_arguments,
                    else_target,
                    else_arguments,
                    ..
                } => {
                    if *then_target == block.id {
                        incoming.push(then_arguments[ordinal]);
                    }
                    if *else_target == block.id {
                        incoming.push(else_arguments[ordinal]);
                    }
                }
                _ => {}
            }
        }
        let first = *incoming.first().expect("constant forwarding predecessor");
        assert!(incoming.iter().all(|value| *value == first));
        value = first;
    }
    panic!("literal fixture forwarding depth exceeded");
}

#[test]
fn formal_actual_tuple_literal_global_store_is_complete_at_one_invocation() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for bits in [0, 3, 17] {
            let source = formal_literal_tuple_store_source_v1(bits, 1);
            with_formal_completed_roots_v1(
                &source,
                profile,
                true,
                &[ranked_root_input_1d(A_NAME, 247, 1)],
                |roots, budget| {
                    assert_eq!(roots.len(), 1);
                    let actual = require_actual_literal_global_offset_v1(roots[0].output(), bits);
                    roots[0].with_physical_address_relation_v1(budget, |relation, _| {
                        assert_eq!(
                            (
                                relation.global_access_count(),
                                relation.private_access_count()
                            ),
                            (1, 0)
                        );
                        Ok(())
                    })?;
                    fe2o3_lower_mir_kernel::with_complete_formal_memory_module_v1(
                        roots,
                        budget,
                        |formal, _| {
                            assert_eq!(formal.len(), 1);
                            assert!(std::ptr::eq(formal[0].output(), roots[0].output()));
                            let obligations = formal[0].obligations();
                            let [access] = obligations.accesses() else {
                                panic!("one formal Global access");
                            };
                            assert_eq!(access.location(), actual);
                            assert_eq!(
                                access.byte_offset(),
                                fe2o3_kernel_ir::ByteExpression::invocation_affine(bits * 4, 0)
                            );
                            assert_eq!(access.byte_width(), 4);
                            assert_eq!(
                                obligations.bounds_requirements()[0].minimum_byte_len(),
                                Some((bits + 1) * 4)
                            );
                            assert!(obligations.inter_invocation_conflicts().is_empty());
                            Ok(())
                        },
                    )
                    .unwrap();
                    Ok(())
                },
            )
            .unwrap();
        }
    }
}

#[test]
fn formal_actual_tuple_literal_global_store_keeps_multi_invocation_conflict() {
    use fe2o3_kernel_ir::{
        ExplicitLaunchExtent, FormalIndexWidth, derive_kernel_memory_obligations_for_launch,
    };
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for bits in [0, 17] {
            let source = formal_literal_tuple_store_source_v1(bits, 64);
            // D is not forged past its independent ranked race gate. This is
            // genuine checked-O extraction, not admissible F completion.
            with_actual_policy3_canonical_view_v1(&source, profile, |checked, view, _| {
                assert!(std::ptr::eq(checked.owner(), view.output()));
                let actual = require_actual_literal_global_offset_v1(checked.owner(), bits);
                let module = checked.owner().module();
                let analysis = derive_kernel_memory_obligations_for_launch(
                    module,
                    &module.kernels[0].id,
                    ExplicitLaunchExtent::Exact {
                        rank: 1,
                        extents: [64, 1, 1],
                    },
                    FormalIndexWidth::Bits64,
                )
                .unwrap();
                assert!(analysis.is_complete(), "{analysis:?}");
                let obligations = analysis.obligations();
                assert_eq!(obligations.accesses().len(), 1);
                assert_eq!(obligations.accesses()[0].location(), actual);
                assert_eq!(obligations.inter_invocation_conflicts().len(), 1);
                assert_eq!(obligations.inter_invocation_conflicts()[0].left(), actual);
                assert_eq!(obligations.inter_invocation_conflicts()[0].right(), actual);
            });
        }
    }
}
