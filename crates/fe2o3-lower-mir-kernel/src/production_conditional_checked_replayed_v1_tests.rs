//! COMPONENT checks on actual owners and checked relations, not a genuine
//! Request, CPU binding, proof, native consumer, or source-to-native receipt.
#![cfg(test)]
use super::*;
use fe2o3_kernel_analysis::check_canonical_kir_coordinate_preservation_v1 as coordinates;

fn with_replayed<T>(
    p: &Complete,
    budget: &mut Budget<'_>,
    run: impl FnOnce(&History<'_>, &Coordinates<'_, '_>, &[Premise], &mut Budget<'_>) -> T,
) -> T {
    scoped(budget, |budget| {
        let premises = original_premises(p, budget);
        budget.reserve_storage(
            premises.capacity() * size_of::<Premise>() + size_of::<Vec<Premise>>(),
        )?;
        let (coordinates, storage) = coordinates(&p.n, &p.b, budget).unwrap();
        budget.reserve_storage(storage.retained_storage())?;
        let history = check_canonical_refined_forwarding_history_v1(p.inputs(), budget)
            .map_err(Error::History)?;
        budget.reserve_storage(history.storage().retained_storage())?;
        let floor = budget.storage();
        let account = budget.work_ledger_identity_v1();
        let result = run(&history, &coordinates, &premises, budget);
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == account);
        drop(history);
        drop(coordinates);
        drop(premises);
        Ok(result)
    })
    .unwrap()
}

// Exercise the exact production shared algorithms, but deliberately do NOT
// impersonate the public genuine-Request binding/argument/current-graph checks.
fn inspect_replayed(
    p: &Complete,
    history: &History<'_>,
    coordinates: &Coordinates<'_, '_>,
    premises: &[Premise],
    budget: &mut Budget<'_>,
) -> Result<()> {
    scoped(budget, |budget| {
        require_replayed_subjects(&p.n, coordinates, history, p.inputs().limits, budget)?;
        if budget.storage() < replayed_backing_floor(history, budget)? {
            return Err(Resource::Accounting.into());
        }
        budget.reserve_storage(SCRATCH)?;
        let kernel = KernelId::new("entry");
        let prefix = history.prefix().policy7_relation().policy6_relation();
        {
            let before = facts(&p.n, &kernel, budget)?;
            let after = facts(prefix.continuation().output(), &kernel, budget)?;
            let original = occurrences::reads(&before, budget)?;
            let output = occurrences::reads(&after, budget)?;
            occurrences::coverage(&before, &after, prefix, &original, &output, budget)?;
            occurrences::premises(&after, &output, premises, budget)?;
        }
        check_replayed_final(history, &kernel, premises, budget)
    })
}

fn parity(p: &Complete, history: &History<'_>, budget: &mut Budget<'_>) -> Result<()> {
    scoped(budget, |budget| {
        budget.reserve_storage(SCRATCH)?;
        let kernel = KernelId::new("entry");
        let before = facts(&p.n, &kernel, budget)?;
        let after = facts(p.checked.owner(), &kernel, budget)?;
        let original = occurrences::reads(&before, budget)?;
        let output = occurrences::reads(&after, budget)?;
        occurrences::coverage(&before, &after, &p.checked, &original, &output, budget)?;
        let replayed = history.prefix().policy7_relation().policy6_relation();
        occurrences::coverage(&before, &after, replayed, &original, &output, budget)?;
        let final_f = facts(history.output(), &kernel, budget)?;
        let final_reads = occurrences::reads(&final_f, budget)?;
        let mut live_order = occurrences::reads(&after, budget)?;
        let mut replayed_order = occurrences::reads(&after, budget)?;
        coverage(
            &p.checked,
            history,
            &after,
            &final_f,
            &mut live_order,
            &final_reads,
            budget,
        )?;
        coverage(
            replayed,
            history,
            &after,
            &final_f,
            &mut replayed_order,
            &final_reads,
            budget,
        )?;
        assert_eq!(live_order, replayed_order);
        Ok(())
    })
}

#[test]
fn conditional_replayed_both_targets_live_and_checked_relation_parity() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for changed in [false, true] {
            with_complete(profile, changed, |p, budget| {
                with_replayed(p, budget, |h, c, premises, budget| {
                    parity(p, h, budget).unwrap();
                    inspect_replayed(p, h, c, premises, budget).unwrap();
                    assert!(std::ptr::eq(h.inputs().output, p.f.output()));
                    assert!(!h.authenticates_execution() && !h.grants_authority());
                    if changed {
                        scoped(budget, |budget| {
                            let kernel = KernelId::new("entry");
                            let i = facts(p.checked.owner(), &kernel, budget)?;
                            let f = facts(h.output(), &kernel, budget)?;
                            let initial = coordinate(&i, i.store_location(), budget)?;
                            let final_site = coordinate(&f, f.store_location(), budget)?;
                            assert_ne!(initial, final_site);
                            assert_eq!(follow(h, initial, budget)?, final_site);
                            Ok(())
                        })
                        .unwrap();
                    }
                })
            });
        }
    }
}

#[test]
fn conditional_replayed_helper_motion_uses_same_checked_history() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        fixture::with_helper_motion(profile, |p, budget| {
            with_replayed(p, budget, |h, c, premises, budget| {
                assert!(!p.h.preheaders().is_empty());
                assert!(p.l.origins().iter().any(|r| r.hoist.is_some()));
                parity(p, h, budget).unwrap();
                inspect_replayed(p, h, c, premises, budget).unwrap();
            })
        });
    }
}

#[test]
fn conditional_replayed_exact_n_b_borrows_and_external_limits_are_required() {
    with_complete(Profile::Gfx942, true, |p, budget| {
        with_replayed(p, budget, |h, c, _, budget| {
            assert!(matches!(
                require_replayed_subjects(&p.b, c, h, h.limits(), budget),
                Err(Error::Mismatch("exact source N and checked history B"))
            ));
            let mut changed = h.limits();
            changed.refinement.operations += 1;
            assert!(matches!(
                require_replayed_subjects(&p.n, c, h, changed, budget),
                Err(Error::LimitsMismatch)
            ));
            scoped(budget, |budget| {
                let donor_b = fixture::graph(p.b.module(), budget);
                assert_eq!(
                    donor_b.canonical().canonical_bytes(),
                    p.b.canonical().canonical_bytes()
                );
                let (donor_c, storage) = coordinates(&p.n, &donor_b, budget).unwrap();
                budget.reserve_storage(storage.retained_storage())?;
                assert!(matches!(
                    require_replayed_subjects(&p.n, &donor_c, h, h.limits(), budget),
                    Err(Error::Mismatch("exact source N and checked history B"))
                ));
                drop(donor_c);
                drop(donor_b);
                Ok(())
            })
            .unwrap();
        })
    });
}

#[test]
fn conditional_replayed_missing_reordered_repeated_reads_and_premises_refuse() {
    with_complete(Profile::Gfx950, true, |p, budget| {
        with_replayed(p, budget, |h, c, premises, budget| {
            assert!(inspect_replayed(p, h, c, &premises[..premises.len() - 1], budget).is_err());
            scoped(budget, |budget| {
                budget.reserve_storage(SCRATCH)?;
                let prefix = h.prefix().policy7_relation().policy6_relation();
                let kernel = KernelId::new("entry");
                let n = facts(&p.n, &kernel, budget)?;
                let i = facts(p.checked.owner(), &kernel, budget)?;
                let f = facts(h.output(), &kernel, budget)?;
                let original = occurrences::reads(&n, budget)?;
                let mut middle = occurrences::reads(&i, budget)?;
                let mut final_reads = occurrences::reads(&f, budget)?;
                assert_eq!(middle.len(), 2);
                assert!(
                    occurrences::coverage(&n, &i, prefix, &original, &middle[..1], budget).is_err()
                );
                middle.swap(0, 1);
                assert!(occurrences::coverage(&n, &i, prefix, &original, &middle, budget).is_err());
                middle.swap(0, 1);
                final_reads[1] = final_reads[0];
                assert!(coverage(prefix, h, &i, &f, &mut middle, &final_reads, budget).is_err());
                Ok(())
            })
            .unwrap();
        })
    });
}

#[test]
fn conditional_replayed_same_parameter_reads_require_exact_ordered_occurrences() {
    use crate::ProductionConditionalCheckedOutputErrorV1 as OutputError;

    for profile in [Profile::Gfx942, Profile::Gfx950] {
        fixture::with_same_parameter_reads(profile, |p, budget| {
            with_replayed(p, budget, |h, c, premises, budget| {
                inspect_replayed(p, h, c, premises, budget).unwrap();
                parity(p, h, budget).unwrap();
                assert_eq!(premises.len(), 10);
                assert_eq!(premises[4..7], premises[7..10]);
                let floor = budget.storage();
                let account = budget.work_ledger_identity_v1();
                let denials = (budget.failed_work(), budget.failed_storage());
                for alter_original in [false, true] {
                    for mutation in 0..3 {
                        scoped(budget, |budget| {
                            budget.reserve_storage(SCRATCH)?;
                            let prefix = h.prefix().policy7_relation().policy6_relation();
                            let kernel = KernelId::new("entry");
                            let before = facts(&p.n, &kernel, budget)?;
                            let after = facts(prefix.continuation().output(), &kernel, budget)?;
                            let mut original = occurrences::reads(&before, budget)?;
                            let mut output = occurrences::reads(&after, budget)?;
                            assert_eq!(original.len(), 2);
                            assert_eq!(output.len(), 2);
                            assert_ne!(original[0].location(), original[1].location());
                            assert_ne!(output[0].location(), output[1].location());
                            assert_eq!(original[0].parameter(), 1);
                            for read in original.iter().chain(&output) {
                                occurrences::read_premises(original[0], *read, budget)?;
                            }
                            occurrences::coverage(
                                &before, &after, prefix, &original, &output, budget,
                            )?;
                            let altered = if alter_original {
                                &mut original
                            } else {
                                &mut output
                            };
                            match mutation {
                                0 => altered.swap(0, 1),
                                1 => {
                                    altered.pop();
                                }
                                2 => altered[1] = altered[0],
                                _ => unreachable!(),
                            }
                            // Swaps and duplicates retain identical premise triples;
                            // only occurrence provenance can distinguish these rows.
                            if mutation != 1 {
                                occurrences::coverage_subjects(
                                    &before, &after, &original, &output,
                                )?;
                                occurrences::premises(&before, &original, premises, budget)?;
                                occurrences::premises(&after, &output, premises, budget)?;
                            }
                            let paid = budget.work();
                            let result = occurrences::coverage(
                                &before, &after, prefix, &original, &output, budget,
                            );
                            if mutation == 1 {
                                assert!(matches!(
                                    result,
                                    Err(OutputError::Mismatch("source/output coverage subjects"))
                                ));
                            } else {
                                assert!(matches!(
                                    result,
                                    Err(OutputError::Mismatch("final memory occurrence"))
                                ));
                            }
                            assert!(budget.work() > paid);
                            Ok(())
                        })
                        .unwrap();
                        assert_eq!(budget.storage(), floor);
                        assert!(budget.work_ledger_identity_v1() == account);
                        assert_eq!((budget.failed_work(), budget.failed_storage()), denials);
                    }
                }
            })
        });
    }
}

fn measured_cost() -> (usize, usize) {
    with_complete(Profile::Gfx942, true, |p, budget| {
        with_replayed(p, budget, |h, c, premises, budget| {
            let pad = budget.peak_storage() - budget.storage() + 1;
            budget.reserve_storage(pad).unwrap();
            let (work, floor) = (budget.work(), budget.storage());
            inspect_replayed(p, h, c, premises, budget).unwrap();
            let cost = (budget.work() - work, budget.peak_storage() - floor);
            assert_eq!(budget.storage(), floor);
            budget.release_storage(pad).unwrap();
            cost
        })
    })
}

#[test]
fn conditional_replayed_original_account_exact_and_each_one_short() {
    let (work, storage) = measured_cost();
    assert!(work > 0 && storage > 0);
    for short in 0..3 {
        with_complete(Profile::Gfx942, true, |p, budget| {
            with_replayed(p, budget, |h, c, premises, budget| {
                let remaining_work = work - usize::from(short == 1);
                let remaining_storage = storage - usize::from(short == 2);
                let pad = STORAGE - budget.storage() - remaining_storage;
                budget
                    .charge_work(WORK - budget.work() - remaining_work)
                    .unwrap();
                budget.reserve_storage(pad).unwrap();
                let floor = budget.storage();
                let account = budget.work_ledger_identity_v1();
                let result = inspect_replayed(p, h, c, premises, budget);
                assert_eq!(result.is_ok(), short == 0);
                assert_eq!(budget.storage(), floor);
                assert!(budget.work_ledger_identity_v1() == account);
                assert_eq!(budget.failed_work().is_some(), short == 1);
                assert_eq!(budget.failed_storage().is_some(), short == 2);
                if short == 0 {
                    assert_eq!(budget.work(), WORK);
                    assert_eq!(budget.peak_storage(), STORAGE);
                }
                budget.release_storage(pad).unwrap();
            })
        });
    }
}

#[test]
fn conditional_replayed_visible_backing_floor_refuses_underpayment() {
    with_complete(Profile::Gfx942, true, |p, budget| {
        with_replayed(p, budget, |h, c, premises, budget| {
            let required = replayed_backing_floor(h, budget).unwrap();
            assert!(budget.storage() >= required);
            let removed = budget.storage() - required + 1;
            budget.release_storage(removed).unwrap();
            assert!(matches!(
                inspect_replayed(p, h, c, premises, budget),
                Err(Error::Resource(Resource::Accounting))
            ));
            assert_eq!(budget.storage(), required - 1);
            // Restore only deliberate component-test corruption before owner drop.
            budget.reserve_storage(removed).unwrap();
        })
    });
}

#[test]
fn conditional_replayed_prior_denials_and_unwind_preserve_original_floor() {
    with_complete(Profile::Gfx950, true, |p, budget| {
        with_replayed(p, budget, |h, c, premises, budget| {
            let (floor, work) = (budget.storage(), budget.work());
            let account = budget.work_ledger_identity_v1();
            assert!(budget.charge_work(WORK + 1).is_err());
            assert!(budget.reserve_storage(STORAGE + 1).is_err());
            let denial = (budget.failed_work(), budget.failed_storage());
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = scoped(budget, |budget| -> Result<()> {
                    inspect_replayed(p, h, c, premises, budget)?;
                    budget.reserve_storage(23)?;
                    budget.charge_work(5)?;
                    panic!("after checked-relation component agreement");
                });
            }));
            assert!(panic.is_err());
            assert_eq!(budget.storage(), floor);
            assert!(budget.work() > work);
            assert!(budget.work_ledger_identity_v1() == account);
            assert_eq!((budget.failed_work(), budget.failed_storage()), denial);
        })
    });
}
