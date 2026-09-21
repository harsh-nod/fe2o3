pub(super) mod source259_execution {
    use super::*;
    use crate::production_analysis::{self as pa, conditional_execution_v1 as ce};
    use ce::{
        FailureV1 as F, SelectedResultV1 as SR, SelectionErrorV1 as SE, TestTraceFailureV1 as TF,
    };
    type Census = pa::ProductionAnalysisInputCensusV1;
    #[derive(Clone, Copy)]
    enum Case {
        Positive,
        Residual,
        RefusedResidual,
        Race,
        EffectDomainRace,
    }
    fn owner(case: Case) -> ProductionConditionalRankedAnalysisV1 {
        owner_with(
            case,
            |_, _| {},
            |kernel| {
                ProductionConstructionV1::ranked_kernel("source259_execution", kernel).unwrap()
            },
        )
    }
    fn owner_with(
        case: Case,
        edit: impl FnOnce(&mut Vec<ProductionRankedOperationV1>, &mut Vec<ProductionRankedOperationV1>),
        finish: impl FnOnce(ProductionRankedKernelV1) -> ProductionConstructionV1,
    ) -> ProductionConditionalRankedAnalysisV1 {
        use ProductionRankedOperationV1 as O;
        use ProductionRankedTerminatorV1 as T;
        let (base, _) = recipe(true, true);
        let racing = matches!(case, Case::Race | Case::EffectDomainRace);
        let mut entry: Vec<_> = base.blocks()[0]
            .operations()
            .iter()
            .filter(|op| !matches!(op, O::Dimension { .. } | O::OwnershipContract { .. }))
            .cloned()
            .collect();
        for op in &mut entry {
            if let O::SemanticExpression { result, .. } = op {
                *result = ProductionRankedValueIdV1::new(3);
            }
        }
        if racing {
            for op in &mut entry {
                match op {
                    O::ExecutionLayout { global_extents, .. } => global_extents[0] = 4,
                    O::View {
                        result,
                        shape,
                        dynamic_extents,
                        ..
                    }
                    | O::ViewInSpace {
                        result,
                        shape,
                        dynamic_extents,
                        ..
                    } if *result == ProductionRankedValueIdV1::new(2) => {
                        shape[0] = 1;
                        dynamic_extents.clear();
                    }
                    _ => {}
                }
            }
        }
        entry.push(O::OwnershipContract {
            view: local(1),
            coverage: OwnershipCoverageAttr::TotalView,
            partition: OwnershipPartitionAttr::ExactSets,
        });
        if !matches!(case, Case::Positive) {
            entry.push(O::OwnershipContract {
                view: local(2),
                coverage: match case {
                    Case::Race => OwnershipCoverageAttr::ExactView,
                    Case::EffectDomainRace => OwnershipCoverageAttr::ExactEffectDomain,
                    _ => OwnershipCoverageAttr::TotalView,
                },
                partition: OwnershipPartitionAttr::ExactSets,
            });
        }
        if racing {
            entry.push(O::IndexConstant {
                result: ProductionRankedValueIdV1::new(4),
                value: 0,
            });
            entry.push(O::Access {
                kind: AccessKindAttr::Write,
                view: local(2),
                indices: vec![local(4)],
            });
        }
        let write = O::Access {
            kind: AccessKindAttr::Write,
            view: local(1),
            indices: vec![local(0)],
        };
        let mut body = if matches!(case, Case::RefusedResidual) {
            vec![write.clone(), write]
        } else {
            vec![write]
        };
        edit(&mut entry, &mut body);
        let kernel = ProductionRankedKernelV1::new(
            "source259_execution",
            1,
            vec![
                ProductionRankedBlockV1::new(
                    entry,
                    T::IndexLessThan {
                        lhs: local(0),
                        rhs: ProductionRankedValueV1::Argument(0),
                        true_block: 1,
                        false_block: 2,
                    },
                ),
                ProductionRankedBlockV1::new(body, T::Branch { target: 2 }),
                ProductionRankedBlockV1::new(vec![], T::Return),
            ],
        )
        .unwrap();
        let site = kernel.blocks()[0]
            .operations()
            .iter()
            .enumerate()
            .find_map(|(operation, op)| match op {
                O::OwnershipContract { view, .. } if *view == local(1) => Some(Site {
                    block: 0,
                    operation: operation as u32,
                    view: *view,
                }),
                _ => None,
            })
            .unwrap();
        let mut session = session();
        let registered = session.register_construction(finish(kernel)).unwrap();
        let (stage, root) = session.construct_registered(registered).unwrap();
        session
            .prepare_conditional_ranked_analysis_v1(stage, root, &[site])
            .unwrap()
    }
    fn occurrences(p: &ProductionConditionalRankedAnalysisV1) -> Vec<ce::OccurrenceV1> {
        p._session.constructed_roots[&p._stage.identity]
            .ownership_occurrences
            .iter()
            .map(|o| (o.site, o.operation, o.view))
            .collect()
    }
    struct Ready {
        function: FuncOp,
        manager: PlironAnalysisManagerV1,
        census: Census,
        epoch: u64,
        rows: Vec<ce::RowV1>,
        race: crate::RankedRaceReportV1,
        #[cfg(feature = "internal-proof-staging")]
        ownership_stage: Bound,
        _identity: BuiltIdentityV1,
    }
    fn ready(p: &ProductionConditionalRankedAnalysisV1, selections: usize) -> Ready {
        ready_from_prefix(
            p,
            selections,
            p.analysis._analyses.resource_upper_bound(),
            ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
        )
    }
    fn ready_from_prefix(
        p: &ProductionConditionalRankedAnalysisV1,
        selections: usize,
        prefix: Bound,
        limits: ProductionAnalysisResourceLimitsV1,
    ) -> Ready {
        let ctx = &p._session.inner.context;
        let function = FuncOp::from_operation(p.analysis.payload.function());
        let epoch = p.analysis._mutation_epoch;
        let mut provider = LivePlironStructuralIdentityProviderV1::new(ctx, &function);
        assert_eq!(provider.mutation_epoch().ok().unwrap(), epoch);
        let capture = provider
            .capture_with_resource_limits_v1(
                limits
                    .remaining_after_retained(Phase::StructuralIdentity, prefix)
                    .unwrap(),
            )
            .ok()
            .unwrap();
        let comparison_work = prefix
            .retained_storage_upper_bound()
            .checked_add(capture.resource_upper_bound.retained_storage_upper_bound())
            .and_then(|n| n.checked_add(1))
            .unwrap();
        let comparison = Bound::checked_phase(
            Phase::StructuralIdentity,
            comparison_work,
            0,
            comparison_work.checked_add(1024).unwrap(),
        )
        .unwrap();
        let initial = prefix
            .checked_then_retain(capture.resource_upper_bound, Phase::StructuralIdentity)
            .unwrap()
            .checked_then_retain(comparison, Phase::StructuralIdentity)
            .unwrap();
        limits.require(Phase::StructuralIdentity, initial).unwrap();
        assert!(
            provider
                .require_exact_identity(&p.analysis._identity, &capture.snapshot)
                .is_ok()
        );
        assert_eq!(provider.mutation_epoch().ok().unwrap(), epoch);
        let census = capture.input_census;
        let mut m = PlironAnalysisManagerV1::new_with_resource_contract(
            &function,
            census,
            initial,
            capture.resource_upper_bound.retained_storage_upper_bound(),
            limits,
        )
        .unwrap();
        m.prepare_function_inventory(ctx, &function);
        macro_rules! cache {
            ($phase:ident, $bound:expr, $prepare:ident) => {{
                let b = $bound.unwrap();
                m.admit_retained_resource_upper_bound(Phase::$phase, b)
                    .unwrap();
                m.$prepare(ctx, &function);
            }};
        }
        cache!(
            SparseIndex,
            pa::preflight_sparse_index_resource_upper_bound_v1(
                ctx,
                &function,
                census,
                m.remaining_resource_limits(Phase::SparseIndex).unwrap()
            ),
            prepare_sparse_indices
        );
        cache!(
            Presburger,
            pa::preflight_presburger_resource_upper_bound_v1(
                m.sparse_indices().ok(),
                m.remaining_resource_limits(Phase::Presburger).unwrap()
            ),
            prepare_presburger
        );
        cache!(
            LaunchContract,
            ce::test_layout_bound_v1(
                census,
                m.remaining_resource_limits(Phase::LaunchContract).unwrap()
            ),
            prepare_execution_layout
        );
        let trace = ce::test_trace_bound_v1(
            ctx,
            m.function_inventory().ok(),
            census,
            m.sparse_indices().ok(),
            m.execution_layout().unwrap(),
            m.remaining_resource_limits(Phase::InvocationTrace).unwrap(),
        )
        .unwrap();
        m.admit_retained_resource_upper_bound(Phase::InvocationTrace, trace.attempt_upper_bound())
            .unwrap();
        m.prepare_exact_trace(ctx, &function);
        assert!(
            matches!(
                m.exact_trace(),
                Err(TF::DynamicLaunch { dimension: 0 }) | Err(TF::UnresolvedBranch { block: 0 })
            ),
            "{:?}",
            m.exact_trace()
        );
        let trace_admission = trace.exact_admission(m.exact_trace()).unwrap();
        assert!(trace_admission.is_none());
        cache!(
            ProvenanceAlias,
            pa::preflight_provenance_alias_resource_upper_bound_v1(
                census,
                m.remaining_resource_limits(Phase::ProvenanceAlias).unwrap()
            ),
            prepare_provenance_alias
        );
        let bounds = pa::preflight_ranked_bounds_resource_upper_bound_v1(
            census,
            m.remaining_resource_limits(Phase::MemoryBounds).unwrap(),
        )
        .unwrap();
        m.admit_retained_resource_upper_bound(Phase::MemoryBounds, bounds)
            .unwrap();
        let checked_bounds =
            pa::run_pliron_ranked_bounds_check_with_analyses_v1(ctx, &function, &mut m);
        assert!(checked_bounds.is_clean(), "{checked_bounds:?}");
        let race_bound = pa::preflight_race_resource_upper_bound_v1(
            ctx,
            &function,
            m.function_inventory().ok(),
            census,
            m.sparse_indices().unwrap(),
            m.execution_layout().unwrap(),
            m.remaining_resource_limits(Phase::RaceFreedom).unwrap(),
        )
        .unwrap();
        m.admit_retained_resource_upper_bound(Phase::RaceFreedom, race_bound)
            .unwrap();
        let race = pa::run_pliron_ranked_race_check_with_analyses_v1(ctx, &function, &mut m);
        let rows = ce::prepare_rows_v1(ctx, &function, &mut m, census, epoch, selections).unwrap();
        let local = pa::preflight_hierarchical_ownership_resource_upper_bound_v1(
            rows.named_census,
            trace_admission,
            m.remaining_resource_limits(Phase::HierarchicalOwnership)
                .unwrap(),
        )
        .unwrap();
        let stage = local
            .checked_with_nested_sequence_discard(
                &[bounds, race_bound],
                Phase::HierarchicalOwnership,
            )
            .unwrap();
        let retained_stage = Bound::checked_phase(
            Phase::HierarchicalOwnership,
            stage.work_upper_bound(),
            stage.peak_storage_upper_bound(),
            0,
        )
        .unwrap();
        m.admit_retained_resource_upper_bound(Phase::HierarchicalOwnership, retained_stage)
            .unwrap();
        Ready {
            function,
            manager: m,
            census,
            epoch,
            rows: rows.rows,
            race,
            #[cfg(feature = "internal-proof-staging")]
            ownership_stage: retained_stage,
            _identity: capture.snapshot,
        }
    }

    #[cfg(feature = "internal-proof-staging")]
    pub(super) mod conditional_effect {
        use super::*;
        use crate::PlironEffectRefinementFindingV1 as Finding;
        use crate::production_analysis::source259_effect_tests as fx;
        use ProductionRankedOperationV1 as O;
        use ed25519_dalek::{Signer, SigningKey};
        use fe2o3_functional_proof::*;
        use fe2o3_proof_contracts::DigestV1;

        fn digest(value: u8) -> DigestV1 {
            DigestV1::from_untrusted_bytes([value; 32])
        }

        fn subjects() -> FunctionalRefinementSubjectsV2 {
            FunctionalRefinementSubjectsV2::new(
                SafeReferenceKindV2::Mir,
                digest(1),
                DigestV1::ZERO,
                digest(2),
                digest(3),
                digest(4),
            )
            .unwrap()
        }

        // This test key supplies internal-policy metadata, not verifier authority.
        fn stage(mut kernel: ProductionRankedKernelV1) -> ProductionConstructionV1 {
            let O::RequestEffectRefinement { contract, subjects } =
                &kernel.blocks()[1].operations()[1]
            else {
                panic!()
            };
            let obligation =
                normalized_effect_refinement_hash_for_kernel_v2(&kernel, 1, 1, contract, *subjects)
                    .unwrap();
            let binding =
                FunctionalRefinementBindingV2::from_subjects(*subjects, obligation).unwrap();
            let signing = SigningKey::from_bytes(&[91; 32]);
            let toolchain = VerusToolchainIdentityV2::new(
                digest(10),
                digest(11),
                digest(12),
                digest(13),
                digest(14),
            )
            .unwrap();
            let boundary = FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir;
            let policy = FunctionalRefinementImportPolicyV2::new(
                signing.verifying_key().to_bytes(),
                toolchain,
                boundary,
            )
            .unwrap();
            let signer_identity = policy.signer_identity();
            let unsigned = UnsignedFunctionalRefinementReceiptV2::from_verified_execution_join(
                signer_identity,
                binding,
                toolchain,
                digest(20),
                FunctionalRefinementResultV2::Proved,
                boundary,
            )
            .unwrap();
            let signature = signing.sign(unsigned.signing_bytes()).to_bytes();
            let wire = unsigned.attach_signature(signature);
            let mut importer = FunctionalRefinementReceiptImporterV2::new(policy, 1).unwrap();
            let imported = importer
                .import(FunctionalRefinementImportExpectationV2::new(binding), &wire)
                .unwrap();
            kernel = kernel
                .bind_functional_refinement_request_v2(
                    1,
                    1,
                    ProductionReferenceProofV2::request_exact(imported.receipt_identity(), binding),
                )
                .unwrap();
            stage_ranked_kernel_with_policy_checked_refinement_v2(
                ProductionConstructionV1::ranked_kernel("source259_execution", kernel).unwrap(),
                vec![imported],
                ProductionRefinementStagingPolicyV2::new([signer_identity], toolchain).unwrap(),
            )
            .unwrap()
        }

        pub(crate) fn effect_owner(case: u8) -> ProductionConditionalRankedAnalysisV1 {
            owner_with(
                Case::Positive,
                |entry, body| {
                    entry.extend([
                        O::SemanticConstant {
                            result: ProductionRankedValueIdV1::new(4),
                            value: 7,
                        },
                        O::SemanticConstant {
                            result: ProductionRankedValueIdV1::new(5),
                            value: 8,
                        },
                    ]);
                    body[0] = O::ValueAccess {
                        kind: AccessKindAttr::Write,
                        view: local(1),
                        indices: vec![local(0)],
                        value: local(4),
                    };
                    if case == 5 {
                        return;
                    }
                    let contract = ProductionEffectRefinementContractV2::new(
                        1,
                        ProductionGpuWriteSiteV2::new(1, 0),
                        ProductionReferenceOutputSiteV2::new(0, 0, 0),
                        local(1),
                        vec![local(0)],
                        vec![local(4)],
                        vec![local(if case == 2 { 5 } else { 4 })],
                        local(4),
                        local(if case == 3 { 5 } else { 4 }),
                        local(4),
                        local(if case == 4 { 5 } else { 4 }),
                        local(4),
                        local(if case == 1 { 5 } else { 4 }),
                    )
                    .unwrap();
                    body.push(O::RequestEffectRefinement {
                        contract,
                        subjects: subjects(),
                    });
                },
                |mut kernel| {
                    if case == 5 {
                        ProductionConstructionV1::ranked_kernel("source259_execution", kernel)
                            .unwrap()
                    } else {
                        if case == 6 {
                            let mut blocks = kernel.blocks().to_vec();
                            blocks[2] = ProductionRankedBlockV1::new(
                                vec![],
                                ProductionRankedTerminatorV1::Branch { target: 2 },
                            );
                            kernel = ProductionRankedKernelV1::new(
                                kernel.function_name(),
                                kernel.argument_count(),
                                blocks,
                            )
                            .unwrap();
                        }
                        stage(kernel)
                    }
                },
            )
        }

        fn with_effect(
            p: &ProductionConditionalRankedAnalysisV1,
            consume: impl FnOnce(fx::ReportV1, &ConditionalPipelineSubjectV1<'_>, &mut Ready),
        ) {
            with_prepared_effect(p, true, |prepared, subject, ready| {
                let report = fx::run_preadmitted_v1(
                    subject.context(),
                    &ready.function,
                    &mut ready.manager,
                    prepared,
                );
                consume(report, subject, ready);
            });
        }

        fn with_prepared_effect(
            p: &ProductionConditionalRankedAnalysisV1,
            clean_pass5: bool,
            consume: impl FnOnce(fx::PreparedV1<'_>, &ConditionalPipelineSubjectV1<'_>, &mut Ready),
        ) {
            let limits = ProductionAnalysisResourceLimitsV1::production_hard_ceiling();
            let mut resources = ProductionAnalysisResourceContractV1::new(limits);
            resources
                .admit_retained(
                    Phase::PipelineVerification,
                    p.analysis._analyses.resource_upper_bound(),
                )
                .unwrap();
            let subject = p.prepare_pipeline_subject_v1(&mut resources).unwrap();
            let mut h = ready_from_prefix(p, 1, resources.cumulative(), limits);
            let pass5 = ce::run_preadmitted_v1(
                subject.context(),
                &h.function,
                &mut h.manager,
                h.census,
                h.epoch,
                subject.recipe(),
                subject.occurrences(),
                ce::RaceSourceV1::SameInvocation(&h.race),
                std::mem::take(&mut h.rows),
            );
            assert_eq!(pass5.is_clean(), clean_pass5, "{pass5:?}");
            let common = pa::preflight_effect_refinement_resource_upper_bound_v1(
                h.census,
                h.manager
                    .remaining_resource_limits(Phase::EffectRefinement)
                    .unwrap(),
            )
            .unwrap();
            let held = Bound::checked_phase(
                Phase::EffectRefinement,
                common.work_upper_bound(),
                common.peak_storage_upper_bound(),
                0,
            )
            .unwrap();
            h.manager
                .admit_retained_resource_upper_bound(Phase::EffectRefinement, held)
                .unwrap();
            h.manager
                .admit_retained_resource_upper_bound(
                    Phase::EffectRefinement,
                    fx::preflight_v1(h.census, 1).unwrap(),
                )
                .unwrap();
            let nested = ce::prepare_rows_v1(
                subject.context(),
                &h.function,
                &mut h.manager,
                h.census,
                h.epoch,
                1,
            )
            .unwrap();
            h.manager
                .admit_retained_resource_upper_bound(
                    Phase::HierarchicalOwnership,
                    h.ownership_stage,
                )
                .unwrap();
            let prepared = fx::prepare_preadmitted_v1(&subject, nested.rows).unwrap();
            consume(prepared, &subject, &mut h);
            drop(pass5);
        }

        fn run_effect(p: &ProductionConditionalRankedAnalysisV1) -> fx::ReportV1 {
            let mut result = None;
            with_effect(p, |report, _, _| result = Some(report));
            result.unwrap()
        }

        #[test]
        fn conditional_effect_missing_or_wrong_private_binding_has_no_credit() {
            with_effect(&effect_owner(0), |report, subject, h| {
                let common = pa::preflight_effect_refinement_resource_upper_bound_v1(
                    h.census,
                    h.manager
                        .remaining_resource_limits(Phase::EffectRefinement)
                        .unwrap(),
                )
                .unwrap();
                h.manager
                    .admit_retained_resource_upper_bound(
                        Phase::EffectRefinement,
                        Bound::checked_phase(
                            Phase::EffectRefinement,
                            common.work_upper_bound(),
                            common.peak_storage_upper_bound(),
                            0,
                        )
                        .unwrap(),
                    )
                    .unwrap();
                h.manager
                    .admit_retained_resource_upper_bound(
                        Phase::EffectRefinement,
                        fx::preflight_v1(h.census, 1).unwrap(),
                    )
                    .unwrap();
                report.test_binding_refusals(subject, &h.manager);
            });
        }

        #[test]
        fn conditional_effect_binds_without_ordinary_credit() {
            let pending = effect_owner(0);
            let report = run_effect(&pending);
            assert!(report.is_clean(), "{report:?}");
            let (counts, findings) = report.test_parts();
            assert_eq!(counts, (1, 0, 1));
            assert!(findings.is_empty());
            assert_eq!(pending.pending_pipeline_checks().len(), 9);
            assert_eq!(
                pending
                    .legacy_report()
                    .coverage_summary()
                    .total_view_proved(),
                0
            );
        }

        #[test]
        fn conditional_effect_rejects_value_coordinate_domain_and_precondition_mismatches() {
            for case in 1..5 {
                let report = run_effect(&effect_owner(case));
                assert!(!report.is_clean(), "case {case}: {report:?}");
                let (counts, findings) = report.test_parts();
                assert_eq!(counts, (1, 0, 0));
                assert!(
                    findings.iter().any(|f| match case {
                        1 | 2 => matches!(f, Finding::ValueMismatch { .. }),
                        3 => matches!(f, Finding::DomainMismatch { .. }),
                        4 => matches!(f, Finding::PreconditionMismatch { .. }),
                        _ => unreachable!(),
                    }),
                    "case {case}: {report:?}"
                );
            }
        }

        #[test]
        fn conditional_effect_without_contracts_does_not_claim_completion() {
            let report = run_effect(&effect_owner(5));
            assert!(!report.is_clean());
            assert_eq!(report.test_parts().0, (0, 0, 0));
            assert!(report.test_no_contracts());
        }

        #[test]
        fn conditional_semantic_failed_progress_keeps_effect_not_run() {
            use pa::conditional_semantic_v1 as semantic;

            let pending = effect_owner(6);
            with_prepared_effect(&pending, false, |prepared, subject, ready| {
                let manager = &mut ready.manager;
                for bound in [
                    pa::preflight_semantic_refinement_resource_upper_bound_v1(
                        ready.census,
                        manager
                            .remaining_resource_limits(Phase::SemanticRefinement)
                            .unwrap(),
                    )
                    .unwrap(),
                    semantic::preflight_v1(ready.census, 1).unwrap(),
                ] {
                    manager
                        .admit_retained_resource_upper_bound(
                            Phase::SemanticRefinement,
                            Bound::checked_phase(
                                Phase::SemanticRefinement,
                                bound.work_upper_bound(),
                                bound.peak_storage_upper_bound(),
                                0,
                            )
                            .unwrap(),
                        )
                        .unwrap();
                }
                // Two actual progress runs retain separate reports. This exercises
                // the post-progress helper, not a successful nine-pass pipeline.
                for _ in 0..2 {
                    let bound = pa::preflight_progress_resource_upper_bound_v1(
                        ready.census,
                        manager.remaining_resource_limits(Phase::Progress).unwrap(),
                    )
                    .unwrap();
                    manager
                        .admit_retained_resource_upper_bound(
                            Phase::Progress,
                            Bound::checked_phase(
                                Phase::Progress,
                                bound.work_upper_bound(),
                                bound.peak_storage_upper_bound(),
                                0,
                            )
                            .unwrap(),
                        )
                        .unwrap();
                }
                let expected = pa::run_pliron_progress_check_v1(subject.context(), &ready.function);
                let progress = pa::run_pliron_progress_check_v1(subject.context(), &ready.function);
                let report = semantic::run_after_progress_preadmitted_v1(
                    subject.context(),
                    &ready.function,
                    manager,
                    progress,
                    prepared,
                );
                report.test_failed_progress(&expected);
            });
        }

        #[test]
        fn conditional_shared_pipeline_runs_all_nine_passes_and_replays_exactly() {
            let pending = effect_owner(0);
            let old = pending.legacy_report().clone();
            let mut resources = ProductionAnalysisResourceContractV1::new(
                ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
            );
            resources
                .admit_retained(
                    Phase::PipelineVerification,
                    pending.analysis._analyses.resource_upper_bound(),
                )
                .unwrap();
            let subject = pending.prepare_pipeline_subject_v1(&mut resources).unwrap();
            let first = pa::run_conditional_production_checks_v1(
                &subject,
                None,
                None,
                resources.remaining(Phase::PipelineVerification).unwrap(),
            )
            .unwrap();
            resources
                .admit_retained(Phase::PipelineVerification, first.resource_upper_bound)
                .unwrap();
            assert_eq!(first.report.test_pass_count(), 9);
            assert!(first.report.ownership().is_clean());
            assert!(first.report.semantics().is_clean());
            assert!(!first.report.independently_validated());
            assert_eq!(
                first
                    .report
                    .ownership()
                    .residual
                    .as_ref()
                    .unwrap()
                    .coverage_summary()
                    .total_view_proved(),
                0
            );
            let second = pa::run_conditional_production_checks_v1(
                &subject,
                None,
                None,
                resources.remaining(Phase::PipelineVerification).unwrap(),
            )
            .unwrap();
            assert_eq!(second.resource_upper_bound, first.resource_upper_bound);
            resources
                .admit_retained(Phase::PipelineVerification, second.resource_upper_bound)
                .unwrap();
            let compare = first
                .resource_upper_bound
                .retained_storage_upper_bound()
                .checked_add(second.resource_upper_bound.retained_storage_upper_bound())
                .unwrap();
            resources
                .admit_retained(
                    Phase::PipelineVerification,
                    Bound::checked_phase(Phase::PipelineVerification, compare, 0, compare).unwrap(),
                )
                .unwrap();
            assert_eq!(first.report, second.report);
            assert_eq!(pending.legacy_report(), &old);
            assert_eq!(pending.pending_pipeline_checks().len(), 9);
            first.report.test_payload_inequality_v1(&second.report);
        }

        #[test]
        fn conditional_shared_pipeline_keeps_semantic_refusals() {
            for case in 1..=5 {
                let pending = effect_owner(case);
                let mut resources = ProductionAnalysisResourceContractV1::new(
                    ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
                );
                resources
                    .admit_retained(
                        Phase::PipelineVerification,
                        pending.analysis._analyses.resource_upper_bound(),
                    )
                    .unwrap();
                let subject = pending.prepare_pipeline_subject_v1(&mut resources).unwrap();
                let error = pa::run_conditional_production_checks_v1(
                    &subject,
                    None,
                    None,
                    resources.remaining(Phase::PipelineVerification).unwrap(),
                )
                .err()
                .unwrap();
                let pa::PipelineErrorV1::ConditionalSemantic(error) = error else {
                    panic!("case {case}: {error:?}")
                };
                assert!(!error.report.is_clean());
                assert_eq!(pending.pending_pipeline_checks().len(), 9);
            }
        }

        #[test]
        fn conditional_shared_pipeline_rejects_identical_foreign_endpoints() {
            let first = effect_owner(0);
            let other = effect_owner(0);
            let mut resources = ProductionAnalysisResourceContractV1::new(
                ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
            );
            for pending in [&first, &other] {
                resources
                    .admit_retained(
                        Phase::PipelineVerification,
                        pending.analysis._analyses.resource_upper_bound(),
                    )
                    .unwrap();
            }
            let input = first.prepare_pipeline_subject_v1(&mut resources).unwrap();
            let foreign = other.prepare_pipeline_subject_v1(&mut resources).unwrap();
            assert_eq!(input.recipe(), foreign.recipe());
            assert_eq!(input.census(), foreign.census());
            assert_eq!(input.epoch(), foreign.epoch());
            assert_ne!(input.pending_subject(), foreign.pending_subject());
            assert!(pa::test_conditional_foreign_endpoint_v1(
                &input,
                &foreign,
                resources.remaining(Phase::PipelineVerification).unwrap()
            ));
            assert_eq!(first.pending_pipeline_checks().len(), 9);
            assert_eq!(other.pending_pipeline_checks().len(), 9);
        }

        #[test]
        fn conditional_shared_pipeline_rejects_capture_substitutions() {
            use pa::{CaptureFaultV1 as Fault, conditional_validation_v1::ErrorV1 as Error};

            for case in 0..6 {
                let first = effect_owner(0);
                let other = effect_owner(0);
                let mut resources = ProductionAnalysisResourceContractV1::new(
                    ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
                );
                for pending in [&first, &other] {
                    resources
                        .admit_retained(
                            Phase::PipelineVerification,
                            pending.analysis._analyses.resource_upper_bound(),
                        )
                        .unwrap();
                }
                let input = first.prepare_pipeline_subject_v1(&mut resources).unwrap();
                let foreign = other.prepare_pipeline_subject_v1(&mut resources).unwrap();
                let (fault, expected) = match case {
                    0 => (
                        Fault::Duplicate,
                        Error::Order {
                            expected: 5,
                            observed: 4,
                        },
                    ),
                    1 => (
                        Fault::OutOfOrder,
                        Error::Order {
                            expected: 4,
                            observed: 5,
                        },
                    ),
                    2 => (Fault::WrongFamily, Error::Family { position: 8 }),
                    3 => (Fault::Partial, Error::Manifest),
                    4 => (
                        Fault::ForeignManager(&other.analysis._analyses as *const _ as usize),
                        Error::Ledger,
                    ),
                    5 => (
                        Fault::ForeignSubject(foreign.pending_subject()),
                        Error::Subject,
                    ),
                    _ => unreachable!(),
                };
                let error = pa::test_capture_rejection_v1(
                    &input,
                    resources.remaining(Phase::PipelineVerification).unwrap(),
                    fault,
                );
                let pa::PipelineErrorV1::ConditionalValidation(actual) = error else {
                    panic!("case {case}: {error:?}")
                };
                assert_eq!(actual, expected, "case {case}");
                assert_eq!(first.pending_pipeline_checks().len(), 9);
                assert_eq!(other.pending_pipeline_checks().len(), 9);
            }
        }

        #[test]
        fn conditional_shared_pipeline_rejects_same_text_stale_epoch() {
            let pending = effect_owner(0);
            let mut resources = ProductionAnalysisResourceContractV1::new(
                ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
            );
            resources
                .admit_retained(
                    Phase::PipelineVerification,
                    pending.analysis._analyses.resource_upper_bound(),
                )
                .unwrap();
            let input = pending.prepare_pipeline_subject_v1(&mut resources).unwrap();
            drop(input.occurrences()[0].1.deref_mut(input.context()));
            let result = pa::run_conditional_production_checks_v1(
                &input,
                None,
                None,
                resources.remaining(Phase::PipelineVerification).unwrap(),
            );
            assert!(matches!(result, Err(pa::PipelineErrorV1::ConditionalInput)));
        }
    }
    fn execute(
        p: &ProductionConditionalRankedAnalysisV1,
        mut h: Ready,
        occurrences: &[ce::OccurrenceV1],
        fresh: bool,
    ) -> ce::ReportV1 {
        let race = if fresh {
            ce::RaceSourceV1::FreshNested
        } else {
            assert!(h.race.is_clean());
            ce::RaceSourceV1::SameInvocation(&h.race)
        };
        let report = ce::run_preadmitted_v1(
            &p._session.inner.context,
            &h.function,
            &mut h.manager,
            h.census,
            h.epoch,
            p.kernel().unwrap(),
            occurrences,
            race,
            h.rows,
        );
        assert_eq!(report.admitted, h.manager.resource_upper_bound());
        report
    }
    #[test]
    fn expanded_selected_positive_has_no_ordinary_credit_and_preserves_pending() {
        let p = owner(Case::Positive);
        let old = p.legacy_report().clone();
        assert!(old.findings().iter().any(|f| matches!(
            f,
            crate::HierarchicalOwnershipFindingV1::TraceIncomplete { .. }
        )));
        let diagnostics = || {
            p.rows()
                .iter()
                .map(|r| {
                    (
                        r.operation(),
                        r.location(),
                        r.view(),
                        r.selected(),
                        r.effect_site(),
                        r.extent(),
                        r.trace(),
                        r.coverage(),
                        r.findings().to_vec(),
                        r.regions().to_vec(),
                    )
                })
                .collect::<Vec<_>>()
        };
        let before = diagnostics();
        let occurrence = occurrences(&p);
        assert_eq!(occurrence.len(), 1);
        assert!(p.rows()[0].location().operation() > occurrence[0].0.operation as usize);
        for fresh in [false, true] {
            let h = ready(&p, 1);
            assert!(h.race.is_clean(), "{:?}", h.race);
            let prefix = h.manager.resource_upper_bound();
            let report = execute(&p, h, &occurrence, fresh);
            assert!(report.is_clean(), "{report:?}");
            assert_eq!(report.selected.len(), 1);
            assert_eq!(report.selected[0].ownership, p.rows()[0].location());
            assert_eq!(report.selected[0].recipe, occurrence[0].0);
            let SR::Checked(facts) = report.selected[0].result else {
                panic!("{report:?}")
            };
            assert_eq!(facts.view, report.selected[0].view);
            assert_eq!(facts.write.block, 1);
            assert_eq!(facts.write.operation, 0);
            assert_eq!(facts.normal_exits, [2, 2]);
            let residual = report.residual.as_ref().unwrap();
            assert!(residual.is_clean());
            assert!(residual.regions().is_empty());
            assert_eq!(residual.coverage_summary().total_view_declared(), 0);
            assert_eq!(residual.coverage_summary().total_view_proved(), 0);
            assert!(report.admitted.work_upper_bound() > prefix.work_upper_bound());
        }
        assert_eq!(p.legacy_report(), &old);
        assert_eq!(diagnostics(), before);
        assert_eq!(p.legacy_report().coverage_summary().total_view_proved(), 0);
        assert_eq!(
            p.pending_pipeline_checks(),
            &crate::PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2
        );
    }
    #[test]
    fn duplicate_and_wrong_live_bindings_refuse_before_rule_execution() {
        for case in 0..5 {
            let p = owner(Case::Residual);
            let all = occurrences(&p);
            let mut selected = if case < 3 { all.clone() } else { vec![all[0]] };
            let (index, reason) = match case {
                0 => {
                    selected[1] = selected[0];
                    (1, SE::DuplicateSite)
                }
                1 => {
                    selected[1].1 = selected[0].1;
                    (1, SE::DuplicateOperation)
                }
                2 => {
                    selected[1].2 = selected[0].2;
                    (1, SE::DuplicateView)
                }
                3 => {
                    selected[0].1 = p.analysis.payload.function();
                    (0, SE::MissingOperation)
                }
                _ => {
                    selected[0].2 = all[1].2;
                    (0, SE::WrongView)
                }
            };
            let h = ready(&p, selected.len());
            assert!(h.race.is_clean());
            let report = execute(&p, h, &selected, true);
            assert_eq!(report.failure, Some(F::Selection { index, reason }));
            assert!(!report.is_clean());
            assert!(report.residual.is_none());
            assert_eq!(report.selected.len(), index);
            assert!(report.selected.iter().all(|r| r.result == SR::NotRun));
        }
    }
    #[test]
    fn retained_epoch_rejects_even_a_noop_mutation_attempt() {
        let p = owner(Case::Positive);
        let selected = occurrences(&p);
        let mut h = ready(&p, 1);
        let before = h.manager.resource_upper_bound();
        let expected = h.epoch;
        let ctx = &p._session.inner.context;
        drop(selected[0].1.deref_mut(ctx));
        let actual = ctx.ir_mutation_attempt_epoch().unwrap().value();
        assert_ne!(actual, expected);
        assert!(
            matches!(ce::prepare_rows_v1(ctx, &h.function, &mut h.manager, h.census, expected, 1), Err(F::Epoch { expected: e, actual: Some(a) }) if e == expected && a == actual)
        );
        assert_eq!(h.manager.resource_upper_bound(), before);
        let report = execute(&p, h, &selected, true);
        assert_eq!(
            report.failure,
            Some(F::Epoch {
                expected,
                actual: Some(actual)
            })
        );
        assert_eq!(report.admitted, before);
        assert!(report.selected.is_empty());
        assert!(report.residual.is_none());
    }
    #[test]
    fn unavailable_residual_trace_survives_selected_success_or_refusal() {
        for case in [Case::Residual, Case::RefusedResidual] {
            let p = owner(case);
            let old = p.legacy_report().clone();
            let selected = occurrences(&p);
            let h = ready(&p, 1);
            assert!(h.race.is_clean(), "{:?}", h.race);
            let report = execute(&p, h, &selected[..1], true);
            assert!(report.failure.is_none(), "{report:?}");
            assert_eq!(report.selected.len(), 1);
            assert!(match case {
                Case::Residual => matches!(report.selected[0].result, SR::Checked(_)),
                _ => matches!(report.selected[0].result, SR::Refused(_)),
            });
            let residual = report.residual.as_ref().unwrap();
            assert!(residual.findings().iter().any(|f| matches!(
                f,
                crate::HierarchicalOwnershipFindingV1::TraceIncomplete { .. }
            )));
            assert_eq!(residual.coverage_summary().total_view_declared(), 1);
            assert_eq!(residual.coverage_summary().total_view_proved(), 0);
            assert!(!report.is_clean());
            assert_eq!(p.legacy_report(), &old);
        }
    }
    #[test]
    fn actual_race_failure_rejects_fresh_and_original_effect_domain_prerequisites() {
        for case in [Case::Race, Case::EffectDomainRace] {
            let p = owner(case);
            let selected = occurrences(&p);
            let h = ready(&p, 1);
            assert!(
                h.race
                    .findings()
                    .iter()
                    .any(|f| matches!(f, crate::RankedRaceFindingV1::ConflictingEffects { .. })),
                "{:?}",
                h.race
            );
            let report = execute(&p, h, &selected[..1], true);
            let Some(F::Prerequisite(prerequisite)) = &report.failure else {
                panic!("{report:?}")
            };
            assert!(prerequisite.findings().iter().any(|f| matches!(f, crate::HierarchicalOwnershipFindingV1::EffectDomainIncomplete { detail } if !detail.is_empty())));
            if matches!(case, Case::Race) {
                assert!(prerequisite.findings().iter().any(|f| matches!(f, crate::HierarchicalOwnershipFindingV1::EffectDomainIncomplete { detail } if detail.starts_with("mandatory ranked race failed: "))));
            }
            assert!(report.selected.is_empty());
            assert!(report.residual.is_none());
            assert!(!report.is_clean());
        }
    }
    #[test]
    fn census_identity_mismatch_is_not_a_resource_denial() {
        let p = owner(Case::Positive);
        let selected = occurrences(&p);
        let mut h = ready(&p, 1);
        h.census.operations += 1;
        let before = h.manager.resource_upper_bound();
        let report = execute(&p, h, &selected, true);
        assert_eq!(report.failure, Some(F::CensusIdentity));
        assert_eq!(report.admitted, before);
        assert!(report.selected.is_empty());
        assert!(report.residual.is_none());
    }
}
