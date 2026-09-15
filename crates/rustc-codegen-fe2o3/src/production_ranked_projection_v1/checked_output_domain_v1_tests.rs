// Included in the existing execution_domain_v1_tests.rs module. Its admitted
// source factory and original three tests remain byte-for-byte unchanged.
mod checked_output_domain_tests {
    use super::*;
    use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work, VerifiedCanonicalKernelIrModuleV12 as Owner,
    };
    use fe2o3_lower_mir_kernel::{
        ProductionPreRankedKirOwnerV1, ProductionSourceOutputGlobalAccessV1,
    };

    fn with_actual_output(
        source: &ProductionPreRankedKirOwnerV1,
        profile: Profile,
        body: impl FnOnce(&Owner, &fe2o3_pliron::CheckedNeutralKernelIrOwnerV1, &mut Budget<'_>),
    ) {
        let bound =
            dialect_amdgcn::bind_production_target_v1(source.executable().module(), profile)
                .unwrap();
        let mut work = Work::new(1_000_000_000_000);
        let mut budget = Budget::new(&mut work, 1_000_000_000);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(23).unwrap();
        let source_bytes = source.executable_storage().retained_storage()
            + source.assert_origin_storage().payload_storage();
        budget.reserve_storage(source_bytes).unwrap();
        let (input, receipt) =
            Owner::from_module_ref_with_verification_budget_v12(bound.module(), &mut budget)
                .unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        // Exactly one fixed observed execution supplies the actual O owner.
        let checked =
            fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_v1(&input, &mut budget).unwrap();
        let output_bytes = checked.storage().retained_storage();
        budget.reserve_storage(output_bytes).unwrap();
        let floor = budget.storage();
        body(&input, &checked, &mut budget);
        assert_eq!(budget.storage(), floor);
        assert!(!checked.grants_authority());
        drop(checked);
        budget.release_storage(output_bytes).unwrap();
        drop(input);
        budget.release_storage(receipt.retained_storage()).unwrap();
        budget.release_storage(source_bytes).unwrap();
        assert_eq!(budget.storage(), 23);
    }

    #[test]
    fn output_read_projection_preserves_exact_domain_and_actual_global_occurrence() {
        let (ssa, inputs) = source(false, u32::MAX);
        let expected = source_launch_roster_for_ranked_inputs_v1(&ssa, &inputs)
            .unwrap()
            .roots()[0]
            .layout();
        assert_eq!(expected.global_extents(), [0, 1, 1]);
        let source = materialize_ranked_fixture_v1(ssa, &inputs).unwrap();
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_actual_output(&source, profile, |bound, checked, budget| {
                with_projected_checked_output_roots_v1(
                    &source,
                    bound,
                    checked,
                    profile,
                    &inputs,
                    &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
                    budget,
                    |roots, view, budget| {
                        let [root] = roots else {
                            panic!("one source-qualified global root");
                        };
                        assert!(root.all_kernel_checks_are_clean());
                        assert!(projected_execution_domain_matches_v1(
                            expected,
                            root.lowering.kernel().blocks()[0].operations()
                        ));
                        assert!(
                            !root
                                .lowering
                                .kernel()
                                .blocks()
                                .iter()
                                .flat_map(|block| block.operations())
                                .any(|operation| matches!(
                                    operation,
                                    ProductionRankedOperationV1::InvocationIndex { .. }
                                ))
                        );
                        let [access] = root.access_sources.as_slice() else {
                            panic!("the unchanged source has one ordinary read");
                        };
                        assert_eq!(access.semantic_block(), 1);
                        assert_eq!(access.semantic_statement(), Some(0));
                        assert_eq!(access.semantic_access_ordinal(), 0);
                        assert!(matches!(
                            view.global_access(
                                root.semantic_root(),
                                root.semantic_root(),
                                access.semantic_block(),
                                access.semantic_statement(),
                                access.semantic_access_ordinal(),
                                budget,
                            )
                            .unwrap(),
                            ProductionSourceOutputGlobalAccessV1::Retained {
                                source_argument: 0,
                                value: None,
                                result: Some(_),
                                executable: true,
                                ..
                            }
                        ));
                        // The dynamic source bounds guard retains the exact
                        // synthetic failure Call through the real optimizer.
                        for module in [source.executable().module(), view.output().module()] {
                            let traps = module.functions.iter()
                                .filter_map(|function| function.body.as_ref())
                                .flat_map(|body| &body.blocks)
                                .flat_map(|block| &block.operations)
                                .filter(|operation| matches!(&operation.kind,
                                    fe2o3_kernel_ir::OperationKind::Call { callee, arguments }
                                    if fe2o3_kernel_ir::AmdGpuDiagnosticOperation::from_intrinsic_call(callee, arguments)
                                        == Some(fe2o3_kernel_ir::AmdGpuDiagnosticOperation::Trap)))
                                .count();
                            assert_eq!(traps, 1);
                        }
                        let census = view.check_ranked_output_effect_census(
                            root.semantic_root(),
                            root.semantic_root(),
                            &root.lowering,
                            &root.access_sources,
                            &root.executable_effect_sources,
                            budget,
                        ).unwrap();
                        assert_eq!((census.accesses(), census.private_allocations()), (1, 0));
                        assert!(std::ptr::eq(view.output(), checked.owner()));
                        assert!(!view.grants_authority());
                        Ok(())
                    },
                )
                .unwrap();
            });
        }
    }

    #[test]
    fn output_projection_keeps_mandatory_nonexclusive_write_refusal() {
        let (ssa, inputs) = source(true, 2);
        let expected = source_launch_roster_for_ranked_inputs_v1(&ssa, &inputs)
            .unwrap()
            .roots()[0]
            .layout();
        assert_eq!(expected.global_extents(), [2, 1, 1]);
        let source = materialize_ranked_fixture_v1(ssa, &inputs).unwrap();
        with_actual_output(&source, Profile::Gfx942, |bound, checked, budget| {
            let called = std::cell::Cell::new(false);
            let result = with_projected_checked_output_roots_v1(
                &source,
                bound,
                checked,
                Profile::Gfx942,
                &inputs,
                &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
                budget,
                |_, _, _| {
                    called.set(true);
                    Ok(())
                },
            );
            assert!(!called.get());
            let Err(ProductionRankedProjectionErrorV1::Compile {
                error,
                ranked_ir,
                access_sources,
            }) = result
            else {
                panic!("the unchanged write must reach mandatory compilation");
            };
            let ProductionRankedCompileErrorV1::Session(
                fe2o3_pliron::ProductionSessionErrorV1::RankedRace(race),
            ) = *error
            else {
                panic!("the write must be refused by mandatory race analysis");
            };
            assert!(!ranked_ir.contains("kernel.invocation_index"));
            assert_eq!(access_sources.len(), 1);
            assert_eq!(access_sources[0].access, AccessKindAttr::Write);
            assert_eq!(access_sources[0].memory_space, MemorySpaceAttr::Global);
            // Preserve the existing exact limitation, not a fabricated
            // enumerated conflict or a singleton execution domain.
            let [
                fe2o3_pliron::RankedRaceFindingV1::UnresolvedIndex {
                    block,
                    operation,
                    dimension: 0,
                    ..
                },
            ] = race.report().findings()
            else {
                panic!("the unchanged slice-index limitation must remain explicit");
            };
            assert_eq!(*block, access_sources[0].block);
            assert_eq!(ranked_ir.matches("kernel.semantic_expression").count(), 1);
            // The one constant-valued ranked expression becomes a typed
            // constant plus its root in Pliron. Race findings use Pliron
            // operation coordinates; source rows use ranked-builder coordinates.
            assert_eq!(access_sources[0].operation, 1);
            assert_eq!(*operation, 2);
        });
    }
}
