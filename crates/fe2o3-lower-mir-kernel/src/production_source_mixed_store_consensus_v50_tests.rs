#[test]
fn genuine_policy11_store_consensus_retains_distinct_final_owner_and_native_chain() {
    for looping in [false, true] {
        with_mixed_fixedpoint_prefix_v28(looping, |prefix, source, budget| {
            let floor = budget.storage();
            let relocated = prefix.prepare_mixed_fixedpoint_licm_v29(budget).unwrap();
            let relocation_floor = budget.storage();
            let forwarded = relocated.prepare_store_consensus_v46(budget).unwrap();
            let forwarding_floor = budget.storage();
            assert!(std::ptr::eq(
                forwarded.relocation(budget).unwrap(),
                &relocated
            ));
            let (checked, credit) = forwarded.replay(budget).unwrap();
            budget.reserve_storage(credit.retained_storage()).unwrap();
            assert!(std::ptr::eq(
                checked.input(),
                relocated.tail(budget).unwrap().output()
            ));
            assert!(std::ptr::eq(
                checked.output(),
                forwarded.output(budget).unwrap()
            ));
            assert!(!std::ptr::eq(checked.input(), checked.output()));
            // This owner deliberately has no direct-private forwarding sites.
            // A fresh no-op output still cannot impersonate the LICM owner.
            assert!(checked.origins().iter().all(|row| row.store.is_none()));
            assert_eq!(
                checked.input().canonical_bytes(),
                checked.output().canonical_bytes()
            );
            drop(checked);
            budget.release_storage(credit.retained_storage()).unwrap();
            assert_eq!(budget.storage(), forwarding_floor);
            let native = forwarded.complete_native_v46(budget).unwrap();
            native.check_original_source(source, budget).unwrap();
            assert!(std::ptr::eq(
                native.store_consensus_v46(budget).unwrap().unwrap(),
                &forwarded
            ));
            assert!(std::ptr::eq(native.relocation(budget).unwrap(), &relocated));
            assert!(std::ptr::eq(
                native.output(budget).unwrap(),
                forwarded.output(budget).unwrap()
            ));
            assert_eq!(
                native.launch_context(budget).unwrap(),
                prefix.launch_context(budget).unwrap()
            );
            assert_eq!(
                native.runtime_premises(budget).unwrap(),
                prefix.runtime_premises(budget).unwrap()
            );
            assert!(native.source_roles_are_complete());
            assert!(native.final_native_completion_is_complete());
            assert!(!native.runtime_requirements_are_discharged());
            assert!(!native.ranked_verification_is_complete());
            assert!(!native.grants_artifact_or_launch_authority());
            native.discard(budget).unwrap();
            assert_eq!(budget.storage(), forwarding_floor);
            forwarded.discard(budget).unwrap();
            assert_eq!(budget.storage(), relocation_floor);
            relocated.discard(budget).unwrap();
            assert_eq!(budget.storage(), floor);
        })
        .unwrap();
    }
}

#[test]
fn genuine_policy11_store_consensus_native_has_exact_and_one_short_tail_limits() {
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
                let forwarded = relocated.prepare_store_consensus_v46(budget)?;
                let selected = (|| -> Result<(), ProductionMixedLicmCompletionErrorV28> {
                    let native = forwarded.complete_native_v46(budget)?;
                    let observed = native
                        .store_consensus_v46(budget)
                        .map(|owner| assert!(owner.is_some()));
                    let released = native.discard(budget);
                    observed?;
                    released?;
                    Ok(())
                })();
                let released = forwarded.discard(budget);
                selected?;
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
        let observed = observation.expect("genuine Policy11 reached the post-LICM tail");
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
        panic!("typed consensus native Work required");
    };
    assert_eq!(error.limit(), WORK);
    assert!(error.actual() > WORK);
    let ArgumentResourceV1::Storage(error) = run(full.1, full.2 - 1).0.unwrap() else {
        panic!("typed consensus native Storage required");
    };
    assert_eq!(error.limit(), SPACE);
    assert!(error.actual() > SPACE);
}

#[test]
fn genuine_policy11_store_consensus_retains_foreign_and_restored_floor_refusals() {
    for foreign in [false, true] {
        let mut reached = false;
        let result = with_mixed_fixedpoint_prefix_custody_v28(false, true, |prefix, _, budget| {
            let relocated = prefix.prepare_mixed_fixedpoint_licm_v29(budget).unwrap();
            let forwarded = relocated.prepare_store_consensus_v46(budget).unwrap();
            let paid = budget.storage();
            if foreign {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000_000);
                let mut other = ArgumentBudgetV1::new(&mut work, 256 << 20);
                other.reserve_storage(paid).unwrap();
                let counters = (
                    other.work(),
                    other.storage(),
                    budget.work(),
                    budget.storage(),
                );
                assert!(matches!(
                    forwarded.output(&other),
                    Err(ProductionSourceOwnedViewErrorV18::Resource(
                        ArgumentResourceV1::Accounting
                    ))
                ));
                assert_eq!(
                    (
                        other.work(),
                        other.storage(),
                        budget.work(),
                        budget.storage()
                    ),
                    counters
                );
            } else {
                budget.release_storage(1).unwrap();
                assert!(matches!(
                    forwarded.output(budget),
                    Err(ProductionSourceOwnedViewErrorV18::Resource(
                        ArgumentResourceV1::Accounting
                    ))
                ));
                budget.reserve_storage(1).unwrap();
            }
            let before = (budget.work(), budget.storage());
            assert!(matches!(
                forwarded.output(budget),
                Err(ProductionSourceOwnedViewErrorV18::Resource(
                    ArgumentResourceV1::Accounting
                ))
            ));
            assert_eq!((budget.work(), budget.storage()), before);
            assert!(forwarded.discard(budget).is_err());
            assert_eq!(budget.storage(), paid);
            assert!(relocated.discard(budget).is_err());
            assert_eq!(budget.storage(), paid);
            reached = true;
        });
        assert!(reached);
        assert!(result.is_err());
    }
}
