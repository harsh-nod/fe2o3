mod source_contract_v26 {
    use super::*;
    include!("production_source_mixed_contract_v26_tests.rs");
}

include!("production_source_synthetic_traps_v26_tests.rs");

thread_local! {
    static MIXED_FIXTURE_SOURCE_TRAPS_V26: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[test]
fn mixed_private_spill_walker_leaves_genuine_global_reads_and_writes_to_global_completion() {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            scoped_raw_admission_v29::SPILL_GLOBAL_VISITS_V26.with(|visits| visits.set(None));
        }
    }
    let _reset = Reset;
    for exclusive in [false, true] {
        scoped_raw_admission_v29::SPILL_GLOBAL_VISITS_V26.with(|visits| visits.set(Some([0, 0])));
        let (result, _, _, completed) =
            run_mixed_handoff_v26(exclusive, 0, OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
        assert!(result.is_ok(), "exclusive={exclusive}: {result:?}");
        assert!(completed);
        let [reads, writes] =
            scoped_raw_admission_v29::SPILL_GLOBAL_VISITS_V26.with(|visits| visits.get().unwrap());
        assert!(reads > 0);
        assert_eq!(writes > 0, exclusive);
    }
}

fn run_mixed_handoff_v26(
    exclusive: bool,
    fault: usize,
    work_limit: usize,
    storage_limit: usize,
) -> (
    Result<(), ProductionMixedSourceHandoffErrorV26>,
    usize,
    usize,
    bool,
) {
    MIXED_FIXTURE_SOURCE_TRAPS_V26.set(0);
    let owner = if exclusive {
        issued_descriptor_role_owner_v18(DescriptorRoleSourceV18::Arithmetic)
    } else {
        descriptor_source_owner(DescriptorCase::READ)
    };
    let abi = if exclusive {
        issued_descriptor_role_abi_v18(&owner)
    } else {
        kernel_argument_abi_v18::tests::FixtureKernelAbiV18::new(&owner)
    };
    let semantic = owner.source_semantic();
    let launch_roots: Vec<_> = semantic
        .roots()
        .iter()
        .map(|root| {
            let entry = semantic.functions()[root.index() as usize]
                .kernel_entry()
                .unwrap();
            ProductionSourceLaunchRootInputV1::new(
                std::str::from_utf8(entry.export_symbol().as_bytes()).unwrap(),
                *entry.kernel_binding_identity().as_bytes(),
                ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
            )
        })
        .collect();
    let launch = ProductionSourceLaunchRosterV1::try_new(semantic, &launch_roots).unwrap();
    let sha = *owner.source_semantic_sha256();
    let classes = vec![ProductionScopeCallableCandidateV29::Ordinary; semantic.callables().len()];
    let roots = abi.roots();
    let input = ProductionExecutionSourceInputV29 {
        semantic_sha256: &sha,
        roots: &[],
        classes: &classes,
        events: &[],
    };
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let mut completed = false;
    let result = (|| -> Result<(), ProductionMixedSourceHandoffErrorV26> {
        let prepared =
            ProductionPendingScopedSourceOwnerV29::prepare_source_with_kernel_abi_budget_v18(
                owner,
                launch,
                input,
                ProductionKernelArgumentAbiInputV18 { roots: &roots },
                ProductionSemanticKirLimitsV1::default(),
                &mut budget,
            )?;
        prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            let floor = budget.storage();
            let mut launches = vec![
                fe2o3_kernel_ir::ExplicitLaunchExtent::Exact {
                    rank: 1,
                    extents: [64, 1, 1]
                };
                source.root_count(budget)?
            ];
            // Independent test census over actual original source spans. Not
            // every slice-access source contains a lowered Rust assertion.
            let mut source_traps = 0;
            for root in 0..launches.len() {
                source_traps += source.root_row(root)?.coordinates.spans.rows.iter()
                    .filter(|span| matches!(span.source, InstanceSpanSourceV1::Synthetic(source)
                        if source.rule == SemanticKirSyntheticOperationRuleV1::RuntimeAssertFailureTrap))
                    .count();
            }
            MIXED_FIXTURE_SOURCE_TRAPS_V26.set(source_traps);
            let width = if fault == 1 {
                fe2o3_kernel_ir::FormalIndexWidth::Unknown
            } else {
                fe2o3_kernel_ir::FormalIndexWidth::Bits64
            };
            if fault == 2 {
                launches.clear();
            }
            if fault == 3 {
                launches[0] = fe2o3_kernel_ir::ExplicitLaunchExtent::Exact {
                    rank: 2,
                    extents: [64, 2, 1],
                };
            }
            let handoff = source.conditional_mixed_worklist_output_v26(
                ProductionKernelArgumentAbiInputV18 { roots: &roots },
                &launches,
                width,
                budget,
            )?;
            let inspected = (|| -> Result<(), ProductionMixedSourceHandoffErrorV26> {
            if fault == 4 {
                budget.charge_work(work_limit.checked_sub(budget.work()).unwrap())?;
            }
            handoff.check_original_source(source.source_ssa(budget)?, budget)?;
            handoff.check_original_argument_abi_v26(
                ProductionKernelArgumentAbiInputV18 { roots: &roots },
                budget,
            )?;
            let output = handoff.output(budget)?;
            assert_eq!(output.execution().policy_version(), 9);
            assert_eq!(
                output.input_audit_bytes(),
                source.canonical(budget)?.canonical_bytes()
            );
            let premises = handoff.runtime_premises(budget)?;
            let occurrences = handoff.runtime_occurrences(budget)?;
            assert!(!premises.is_empty());
            assert!(!occurrences.is_empty());
            let mut reads = 0usize;
            let mut writes = 0usize;
            for occurrence in occurrences {
                let premise = &premises[occurrence.premise_index()];
                assert_eq!(premise.launch(), launches[premise.root()]);
                assert_eq!(premise.index_width(), width);
                assert!(premise.requires_valid_aligned_extent());
                assert!(occurrence.requires_address_formation_domain());
                assert!(!occurrence.grants_artifact_or_launch_authority());
                reads += usize::from(!occurrence.domain().writing());
                writes += usize::from(occurrence.domain().writing());
                if occurrence.domain().writing() {
                    assert!(premise.source_exclusive_contract());
                    assert!(premise.requires_exact_launch_binding());
                    assert!(premise.requires_exclusive_nonoverlapping_runtime_binding());
                    assert!(occurrence.invocation_projection().is_some());
                } else {
                    assert!(premise.requires_initialized_extent());
                }
            }
            assert!(reads > 0);
            assert_eq!(writes > 0, exclusive);
            assert_eq!(
                premises.iter().map(|p| p.access_counts()[0]).sum::<usize>(),
                reads
            );
            assert_eq!(
                premises.iter().map(|p| p.access_counts()[1]).sum::<usize>(),
                writes
            );
            assert!(!handoff.runtime_requirements_are_discharged());
            assert!(!handoff.ranked_verification_is_complete());
            assert!(!handoff.grants_artifact_or_launch_authority());
            assert_eq!(budget.storage(), floor + handoff.retained_storage(budget)?);
            Ok(())
            })();
            // A paid query can refuse after successful owning adoption. Always
            // destroy/refund that exact owner before propagating the first error.
            let cleanup = handoff.discard(budget);
            assert_eq!(budget.storage(), floor);
            inspected?;
            cleanup?;
            completed = true;
            Ok::<_, ProductionMixedSourceHandoffErrorV26>(())
        })
    })();
    assert_eq!(budget.storage(), MODULE_FLOOR, "{result:?}");
    (result, budget.work(), budget.peak_storage(), completed)
}

#[test]
fn mixed_handoff_v26_late_query_work_refusal_disposes_owned_output_before_return() {
    for exclusive in [false, true] {
        let (result, _, _, completed) =
            run_mixed_handoff_v26(exclusive, 4, OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
        assert!(!completed);
        assert!(
            matches!(
                result,
                Err(ProductionMixedSourceHandoffErrorV26::Check(
                    ProductionMixedSourceCheckErrorV26::Source(
                        ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(_))
                    )
                ))
            ),
            "exclusive={exclusive}: {result:?}"
        );
    }
}

#[test]
fn mixed_handoff_v26_runs_actual_policy9_for_shared_and_read_modify_write_sources() {
    for exclusive in [false, true] {
        let (result, _, _, reached) =
            run_mixed_handoff_v26(exclusive, 0, OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
        assert!(result.is_ok(), "exclusive={exclusive}: {result:?}");
        assert!(reached);
    }
}

#[test]
fn mixed_handoff_v26_refuses_missing_launch_unbound_width_and_conflicting_launch() {
    for fault in [1, 2, 3] {
        let (result, _, _, reached) =
            run_mixed_handoff_v26(true, fault, OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
        assert!(result.is_err(), "fault={fault}");
        assert!(!reached);
    }
}

#[test]
fn mixed_handoff_v26_exact_and_one_short_whole_transaction_limits() {
    let (result, work, peak, reached) =
        run_mixed_handoff_v26(true, 0, OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
    assert!(result.is_ok(), "{result:?}");
    assert!(reached);
    let (exact, exact_work, exact_peak, reached) = run_mixed_handoff_v26(true, 0, work, peak);
    assert!(exact.is_ok(), "{exact:?}");
    assert!(reached);
    assert_eq!((exact_work, exact_peak), (work, peak));
    for (work_limit, storage_limit) in [(work - 1, peak), (work, peak - 1)] {
        let (short, _, _, _) = run_mixed_handoff_v26(true, 0, work_limit, storage_limit);
        assert!(short.is_err(), "one-short transaction was admitted");
    }
}

#[derive(Debug)]
enum MixedRootRefusalV26<'a> {
    Sentinel,
    Source(ProductionSourceOwnedViewErrorV18),
    Payload(MixedPanicCaptureV26<'a>),
}
impl From<ProductionSourceOwnedViewErrorV18> for MixedRootRefusalV26<'_> {
    fn from(error: ProductionSourceOwnedViewErrorV18) -> Self {
        Self::Source(error)
    }
}
#[derive(Debug)]
struct MixedPanicCaptureV26<'a>(&'a std::cell::Cell<bool>);
impl MixedPanicCaptureV26<'_> {
    fn touch(&self) {
        assert!(!self.0.get());
    }
}
impl Drop for MixedPanicCaptureV26<'_> {
    fn drop(&mut self) {
        self.0.set(true);
        panic!("mixed uncalled consumer destructor");
    }
}

#[test]
fn mixed_native_public_and_preparation_refusals_protect_uncalled_capture_destructors() {
    for fault in [0, 1, 2, 3] {
        let dropped = std::cell::Cell::new(false);
        let error_dropped = std::cell::Cell::new(false);
        let root_called = std::cell::Cell::new(false);
        let checked_result = std::cell::Cell::new(false);
        let owner = issued_descriptor_role_owner_v18(DescriptorRoleSourceV18::Arithmetic);
        let abi = issued_descriptor_role_abi_v18(&owner);
        let (result, _, _) = run_descriptor_role_owner_result_with_abi_v18(
            owner,
            abi,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |original, optimized, budget| {
                use fe2o3_kernel_analysis::{
                    CanonicalRankedMetadataV18, CanonicalRankedViewErrorV1,
                    build_canonical_ranked_candidate_v18, with_checked_canonical_ranked_view_v18,
                };
                let floor = budget.storage();
                let output = optimized.output_inventory(budget)?;
                let metadata = CanonicalRankedMetadataV18::new(output.owner(), &[]);
                let metadata_storage = metadata.storage_extent(budget).unwrap();
                budget.reserve_storage(metadata_storage)?;
                let (candidate, receipt) =
                    build_canonical_ranked_candidate_v18(output, &metadata, budget).unwrap();
                budget.reserve_storage(receipt.retained_storage())?;
                let layouts = original.source.limits(budget)?.storage_layout_limits();
                let limits = fe2o3_kernel_analysis::CanonicalKirPrivateMemoryLimitsV1 {
                    max_cells: output.definitions().len(),
                };
                let launches = vec![
                    fe2o3_kernel_ir::ExplicitLaunchExtent::Exact {
                        rank: 1,
                        extents: [64, 1, 1]
                    };
                    if fault == 1 {
                        0
                    } else {
                        original.source.root_count(budget)?
                    }
                ];
                let capture = MixedPanicCaptureV26(&dropped);
                let result = with_checked_canonical_ranked_view_v18(
                    output,
                    &metadata,
                    &candidate,
                    budget,
                    |checked, budget| {
                        if fault == 2 {
                            let _ = original
                                .source
                                .missing::<()>("mixed public sticky preflight fixture");
                        }
                        let before = budget.work();
                        let checked_floor = budget.storage();
                        let result = optimized.with_mixed_memory_native_policies_v26(
                            checked,
                            layouts,
                            limits,
                            &launches,
                            fe2o3_kernel_ir::FormalIndexWidth::Bits64,
                            budget,
                            |_, _| {
                                root_called.set(true);
                                if fault == 3 {
                                    let _ = original
                                        .source
                                        .missing::<()>("mixed first root query refusal");
                                    return Err(MixedRootRefusalV26::Payload(
                                        MixedPanicCaptureV26(&error_dropped),
                                    ));
                                }
                                Err(MixedRootRefusalV26::Sentinel)
                            },
                            move |_, _| {
                                capture.touch();
                                panic!("refused root reached the consumer");
                            },
                        );
                        if fault == 2 {
                            assert_eq!(budget.work(), before);
                        }
                        assert_eq!(
                            budget.storage(),
                            checked_floor,
                            "mixed wrapper must settle inside the pending callback before returning to the ranked view"
                        );
                        assert!(
                            std::ptr::eq(checked.inventory(budget).unwrap(), output),
                            "typed source refusal must leave exact ranked custody queryable"
                        );
                        Ok::<_, CanonicalRankedViewErrorV1>(result)
                    },
                )
                .unwrap();
                assert!(dropped.get());
                if fault == 0 {
                    assert!(
                        matches!(result, Ok(Err(MixedRootRefusalV26::Sentinel))),
                        "{result:?}"
                    );
                } else if fault == 3 {
                    assert!(result.is_err(), "{result:?}");
                    assert!(format!("{result:?}").contains("mixed first root query refusal"));
                    assert!(error_dropped.get());
                } else {
                    assert!(result.is_err(), "{result:?}");
                }
                assert_eq!(root_called.get(), fault == 0 || fault == 3);
                checked_result.set(true);
                drop(candidate);
                drop(metadata);
                budget.release_storage(metadata_storage + receipt.retained_storage())?;
                assert_eq!(budget.storage(), floor);
                Ok(())
            },
        );
        assert!(checked_result.get(), "fault={fault}: {result:?}");
        assert!(dropped.get());
    }
}
