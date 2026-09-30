fn mixed_licm_native_resource_v28(
    error: ProductionMixedLicmCompletionErrorV28,
) -> ArgumentResourceV1 {
    if let ProductionMixedLicmCompletionErrorV28::Relocation(error) = error {
        return mixed_licm_resource_v28(error);
    }
    let mut current: Option<&(dyn std::error::Error + 'static)> = Some(&error);
    while let Some(cause) = current {
        if let Some(resource) = cause.downcast_ref::<ArgumentResourceV1>() {
            return *resource;
        }
        if let Some(work) = cause.downcast_ref::<fe2o3_kernel_ir::CanonicalKernelIrWorkLimitV1>() {
            return ArgumentResourceV1::Work(*work);
        }
        current = cause.source();
    }
    panic!("typed final native resource refusal required: {error:?}");
}

#[test]
fn genuine_mixed_licm_final_owner_rechecks_all_native_stages_and_relocated_occurrences() {
    for looping in [false, true] {
        with_mixed_licm_prefix_v28(looping, |prefix, source, budget| {
            let floor = budget.storage();
            let relocated = prefix.prepare_mixed_licm_v28(budget).unwrap();
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
fn genuine_mixed_licm_final_native_has_exact_and_one_short_post_relocation_limits() {
    const WORK: usize = 1_000_000_000;
    const SPACE: usize = 256 << 20;
    let run = |work_headroom: usize, storage_headroom: usize| {
        let mut observation = None;
        let outer = with_mixed_licm_prefix_v28(true, |prefix, _, budget| {
            let floor = budget.storage();
            let relocated = prefix.prepare_mixed_licm_v28(budget).unwrap();
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
fn genuine_mixed_licm_final_native_custody_loss_cannot_refund_restored_owner_credit() {
    for foreign in [false, true] {
        let mut completed = false;
        let error = with_mixed_licm_prefix_custody_v28(true, true, |prefix, _, budget| {
            let relocated = prefix.prepare_mixed_licm_v28(budget).unwrap();
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
fn genuine_mixed_licm_final_native_refuses_omitted_and_same_count_substituted_join_rows() {
    for fault in 1..=10 {
        let mut completed = false;
        with_mixed_licm_prefix_v28(true, |prefix, _, budget| {
            let floor = budget.storage();
            let relocated = prefix.prepare_mixed_licm_v28(budget).unwrap();
            let relocation_floor = budget.storage();
            let error = relocated.complete_native_fault_v28(fault, budget).err().expect("malformed private join scratch must refuse");
            let expected = match fault {
                1 | 2 => "LICM final source obligation changed or duplicated",
                3 => "LICM final conditional access changed or duplicated",
                4 => "LICM final conditional access absent",
                5 => "LICM native parameter omitted or invented a source premise",
                6 => "LICM parameter premise index",
                7 => "LICM final conditional global access census",
                8 | 9 => "LICM final native owner or complete function census",
                10 => "LICM final source obligation count",
                _ => unreachable!(),
            };
            assert!(matches!(error, ProductionMixedLicmCompletionErrorV28::Relocation(ProductionMixedLicmRelocationErrorV28::Binding(message)) if message == expected), "fault={fault}: {error:?}");
            assert_eq!(budget.storage(), relocation_floor, "fault={fault}");
            relocated.discard(budget).unwrap();
            assert_eq!(budget.storage(), floor);
            completed = true;
        }).unwrap();
        assert!(completed, "fault={fault}");
    }
}
