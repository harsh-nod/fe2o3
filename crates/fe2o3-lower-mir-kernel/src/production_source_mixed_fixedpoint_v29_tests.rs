include!("production_source_checked_licm_v44_tests.rs");
include!("production_source_mixed_store_consensus_v50_tests.rs");

fn with_mixed_fixedpoint_prefix_v28(
    looping: bool,
    run: impl FnMut(
        &ProductionConditionalMixedFixedpointOutputHandoffV29<'_, '_>,
        &ProductionSemanticSsaOwnerV1,
        &mut ArgumentBudgetV1<'_>,
    ),
) -> Result<(), ProductionMixedSourceHandoffErrorV26> {
    with_mixed_fixedpoint_prefix_custody_v28(looping, false, run)
}

fn with_mixed_fixedpoint_prefix_custody_v28(
    looping: bool,
    custody_denied: bool,
    run: impl FnMut(
        &ProductionConditionalMixedFixedpointOutputHandoffV29<'_, '_>,
        &ProductionSemanticSsaOwnerV1,
        &mut ArgumentBudgetV1<'_>,
    ),
) -> Result<(), ProductionMixedSourceHandoffErrorV26> {
    let owner = mixed_licm_source_v28(looping);
    with_mixed_fixedpoint_prefix_owner_v28(owner, custody_denied, run)
}

fn with_mixed_fixedpoint_prefix_owner_v28(
    owner: ProductionSemanticSsaOwnerV1,
    custody_denied: bool,
    mut run: impl FnMut(
        &ProductionConditionalMixedFixedpointOutputHandoffV29<'_, '_>,
        &ProductionSemanticSsaOwnerV1,
        &mut ArgumentBudgetV1<'_>,
    ),
) -> Result<(), ProductionMixedSourceHandoffErrorV26> {
    let abi = issued_descriptor_role_abi_v18(&owner);
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
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 256 << 20);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared =
        ProductionPendingScopedSourceOwnerV29::prepare_source_with_kernel_abi_budget_v18(
            owner,
            launch,
            input,
            ProductionKernelArgumentAbiInputV18 { roots: &roots },
            ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        )
        .unwrap();
    let result: Result<(), ProductionMixedSourceHandoffErrorV26> = prepared
        .with_source_consumer_v18(&mut budget, |source, budget| {
            let floor = budget.storage();
            let launches = vec![
                fe2o3_kernel_ir::ExplicitLaunchExtent::Exact {
                    rank: 1,
                    extents: [64, 1, 1],
                };
                source.root_count(budget)?
            ];
            let original = source.source_ssa(budget)?;
            let prefix = source.conditional_mixed_fixedpoint_output_v29(
                ProductionKernelArgumentAbiInputV18 { roots: &roots },
                &launches,
                fe2o3_kernel_ir::FormalIndexWidth::Bits64,
                budget,
            )?;
            run(&prefix, original, budget);
            let released = prefix.discard(budget);
            released?;
            assert_eq!(budget.storage(), floor);
            Ok(())
        });
    if custody_denied {
        assert!(budget.storage() > MODULE_FLOOR);
    } else {
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
    result
}

#[test]
fn genuine_mixed_fixedpoint_licm_final_owner_rechecks_all_native_stages_and_relocated_occurrences()
{
    for looping in [false, true] {
        with_mixed_fixedpoint_prefix_v28(looping, |prefix, source, budget| {
            let checked = prefix.output(budget).unwrap();
            assert_eq!(checked.execution().policy_version(), 11);
            assert!((1..=32).contains(&checked.execution().rounds()));
            let witnessed = prefix.checked_prefix_v29(budget).unwrap();
            assert!(std::ptr::eq(witnessed.owner(), checked.owner()));
            assert_eq!(
                witnessed.execution().canonical_bytes(),
                checked.execution().canonical_bytes()
            );
            assert_eq!(witnessed.input_audit_bytes(), checked.input_audit_bytes());
            let floor = budget.storage();
            let relocated = prefix.prepare_mixed_fixedpoint_licm_v29(budget).unwrap();
            let observed = (|| -> Result<(), ProductionMixedLicmCompletionErrorV28> {
                let relocation_floor = budget.storage();
                let native = relocated.complete_native_v28(budget)?;
                let observed = (|| -> Result<(), ProductionMixedLicmCompletionErrorV28> {
                    native.check_original_source(source, budget)?;
                    let abi = issued_descriptor_role_abi_v18(source);
                    let roots = abi.roots();
                    native.check_original_argument_abi_v26(
                        ProductionKernelArgumentAbiInputV18 { roots: &roots },
                        budget,
                    )?;
                    assert!(std::ptr::eq(native.relocation(budget)?, &relocated));
                    let tail = relocated.tail(budget)?;
                    assert!(std::ptr::eq(native.output(budget)?, tail.output()));
                    assert_eq!(
                        tail.origins().iter().any(|row| row.hoist.is_some()),
                        looping
                    );
                    assert_eq!(
                        native.runtime_premises(budget)?,
                        prefix.runtime_premises(budget)?
                    );
                    assert_eq!(
                        native.launch_context(budget)?,
                        prefix.launch_context(budget)?
                    );
                    let occurrences = native.runtime_occurrences(budget)?;
                    let original = prefix.runtime_occurrences(budget)?;
                    assert!(!occurrences.is_empty());
                    assert_eq!(occurrences.len(), original.len());
                    for (after, before) in occurrences.iter().zip(original) {
                        let row = tail
                            .origins()
                            .iter()
                            .find(|row| row.input == before.output_operation())
                            .unwrap();
                        let formation = tail
                            .origins()
                            .iter()
                            .find(|row| row.input == before.output_address_formation())
                            .unwrap();
                        assert_eq!(after.original_instance(), before.original_instance());
                        assert_eq!(after.original_operation(), before.original_operation());
                        assert_eq!(after.output_operation(), row.output);
                        assert_eq!(after.output_address_formation(), formation.output);
                        assert_eq!(after.domain(), before.domain());
                        assert_eq!(after.memory_access(), before.memory_access());
                        assert_eq!(after.output_guard_edge(), before.output_guard_edge());
                        assert!(after.requires_address_formation_domain());
                        assert!(!after.grants_artifact_or_launch_authority());
                    }
                    let histories = native.native_histories(budget)?;
                    assert_eq!(histories.len(), tail.output().module().functions.len());
                    for (function, history) in
                        tail.output().module().functions.iter().zip(histories)
                    {
                        assert_eq!(history.is_some(), function.body.is_some());
                    }
                    assert!(native.source_roles_are_complete());
                    assert!(native.final_native_completion_is_complete());
                    assert!(!native.runtime_requirements_are_discharged());
                    assert!(!native.ranked_verification_is_complete());
                    assert!(!native.grants_artifact_or_launch_authority());
                    Ok(())
                })();
                let released = native.discard(budget);
                observed?;
                released?;
                assert_eq!(budget.storage(), relocation_floor);
                Ok(())
            })();
            let released = relocated.discard(budget);
            observed.unwrap();
            released.unwrap();
            assert_eq!(budget.storage(), floor);
        })
        .unwrap();
    }
}

#[test]
fn genuine_mixed_fixedpoint_licm_final_native_has_exact_and_one_short_post_relocation_limits() {
    const WORK: usize = 1_000_000_000;
    const SPACE: usize = 256 << 20;
    let run = |work_headroom: usize, storage_headroom: usize| {
        let mut observation = None;
        let outer = with_mixed_fixedpoint_prefix_v28(true, |prefix, _, budget| {
            let floor = budget.storage();
            let relocated = prefix.prepare_mixed_fixedpoint_licm_v29(budget).unwrap();
            let relocation_floor = budget.storage();
            budget
                .charge_work(WORK - budget.work() - work_headroom)
                .unwrap();
            let padding = SPACE - relocation_floor - storage_headroom;
            let old_peak = budget.peak_storage();
            budget.reserve_storage(padding).unwrap();
            assert!(budget.storage() > old_peak);
            let start = (budget.work(), budget.storage());
            let result = (|| -> Result<(), ProductionMixedLicmCompletionErrorV28> {
                let native = relocated.complete_native_v28(budget)?;
                let observed = native
                    .runtime_occurrences(budget)
                    .map(|rows| assert!(!rows.is_empty()));
                let released = native.discard(budget);
                observed?;
                released?;
                Ok(())
            })();
            assert_eq!(budget.storage(), start.1);
            observation = Some((
                result.err().map(mixed_licm_native_resource_v28),
                budget.work() - start.0,
                budget.peak_storage() - start.1,
            ));
            budget.release_storage(padding).unwrap();
            assert_eq!(budget.storage(), relocation_floor);
            let released = relocated.discard(budget);
            assert_eq!(budget.storage(), floor);
            if observation.as_ref().unwrap().0.is_none() {
                released.unwrap();
            }
        });
        let observed = observation.expect("actual relocation reached final native continuation");
        if observed.0.is_none() {
            outer.unwrap();
        }
        observed
    };
    let full = run(WORK / 2, SPACE / 2);
    assert!(full.0.is_none());
    let exact = run(full.1, full.2);
    assert!(exact.0.is_none());
    assert_eq!((exact.1, exact.2), (full.1, full.2));
    let ArgumentResourceV1::Work(error) = run(full.1 - 1, full.2).0.unwrap() else {
        panic!("typed native Work required");
    };
    assert_eq!(error.limit(), WORK);
    assert!(error.actual() > WORK);
    let ArgumentResourceV1::Storage(error) = run(full.1, full.2 - 1).0.unwrap() else {
        panic!("typed native Storage required");
    };
    assert_eq!(error.limit(), SPACE);
    assert!(error.actual() > SPACE);
}

#[test]
fn genuine_mixed_fixedpoint_licm_final_native_custody_loss_cannot_refund_restored_owner_credit() {
    for foreign in [false, true] {
        let mut completed = false;
        let error = with_mixed_fixedpoint_prefix_custody_v28(true, true, |prefix, _, budget| {
            let relocated = prefix.prepare_mixed_fixedpoint_licm_v29(budget).unwrap();
            let native = relocated.complete_native_v28(budget).unwrap();
            let paid = budget.storage();
            let selected = if foreign {
                let mut other_work = CanonicalKernelIrWorkBudgetV1::new(1_000_000_000);
                let mut other = ArgumentBudgetV1::new(&mut other_work, 256 << 20);
                other.reserve_storage(paid).unwrap();
                let before = (
                    other.work(),
                    other.storage(),
                    budget.work(),
                    budget.storage(),
                );
                let error = native
                    .output(&other)
                    .err()
                    .expect("foreign slot and ledger must refuse");
                assert_eq!(
                    (
                        other.work(),
                        other.storage(),
                        budget.work(),
                        budget.storage()
                    ),
                    before
                );
                error
            } else {
                budget.release_storage(1).unwrap();
                let error = native
                    .output(budget)
                    .err()
                    .expect("one-byte retained loss must refuse");
                budget.reserve_storage(1).unwrap();
                error
            };
            assert!(matches!(
                selected,
                ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
            ));
            assert!(matches!(
                native.output(budget),
                Err(ProductionSourceOwnedViewErrorV18::Resource(
                    ArgumentResourceV1::Accounting
                ))
            ));
            assert!(matches!(
                native.discard(budget),
                Err(ProductionSourceOwnedViewErrorV18::Resource(
                    ArgumentResourceV1::Accounting
                ))
            ));
            assert_eq!(
                budget.storage(),
                paid,
                "observed loss must retain final-owner credit"
            );
            assert!(matches!(
                relocated.discard(budget),
                Err(ProductionSourceOwnedViewErrorV18::Resource(
                    ArgumentResourceV1::Accounting
                ))
            ));
            assert_eq!(
                budget.storage(),
                paid,
                "the enclosing relocation must also retain its credit"
            );
            completed = true;
        })
        .unwrap_err();
        assert!(
            completed,
            "actual final-native owner must reach the custody control: {error:?}"
        );
        assert!(matches!(
            error,
            ProductionMixedSourceHandoffErrorV26::Check(
                ProductionMixedSourceCheckErrorV26::Source(
                    ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
                )
            )
        ));
    }
}

#[test]
fn genuine_mixed_fixedpoint_prefix_is_nominal_and_preserves_unsupported_progress_refusal() {
    use std::any::TypeId;
    assert_ne!(
        TypeId::of::<ProductionConditionalMixedPureCseOutputHandoffV26<'static, 'static>>(),
        TypeId::of::<ProductionConditionalMixedFixedpointOutputHandoffV29<'static, 'static>>(),
    );
    assert_ne!(
        TypeId::of::<ProductionMixedLicmRelocationV28<'static, 'static, 'static>>(),
        TypeId::of::<ProductionMixedFixedpointLicmRelocationV29<'static, 'static, 'static>>(),
    );
    assert_ne!(
        TypeId::of::<
            ProductionConditionalMixedLicmOutputHandoffV28<'static, 'static, 'static, 'static>,
        >(),
        TypeId::of::<
            ProductionConditionalMixedFixedpointLicmOutputHandoffV29<
                'static,
                'static,
                'static,
                'static,
            >,
        >(),
    );
    let mut calls = 0;
    let error = with_mixed_fixedpoint_prefix_owner_v28(
        mixed_licm_source_with_guard_v28(true, false),
        false,
        |_, _, _| {
            calls += 1;
        },
    )
    .unwrap_err();
    assert_eq!(calls, 0);
    assert!(matches!(
        error,
        ProductionMixedSourceHandoffErrorV26::Optimization(
            ProductionSourceOptimizationErrorV18::Adoption(
                fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Origin(
                    ProductionMixedSourceCheckErrorV26::Native(_)
                )
            )
        )
    ));
}
