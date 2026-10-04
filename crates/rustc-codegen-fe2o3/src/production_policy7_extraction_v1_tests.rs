//! Live-ledger helper tests, not fabricated collector or signed compilation.
use super::*;
use std::{cell::Cell, rc::Rc};

fn accounting(result: Result7<()>) {
    assert!(matches!(
        result,
        Err(ProductionPipelineError::CheckedOutputPolicy7Stage(
            CheckedOutputPolicy7StageErrorV1::Resource(Resource::Accounting)
        ))
    ));
}

#[test]
fn extraction_checkpoint_requires_actual_live_ledger_and_budget_slot() {
    let mut work = Work::new(100);
    let mut other_work = Work::new(100);
    let mut budget = Budget::new(&mut work, 100);
    let mut other = Budget::new(&mut other_work, 100);
    for value in [&mut budget, &mut other] {
        value.charge_work(7).unwrap();
        value.reserve_storage(13).unwrap();
    }
    let checkpoint = LiveExtractionPhaseV1::capture(&budget);
    checkpoint.check(&budget).unwrap();
    accounting(checkpoint.check(&other));
    // Same slot and numeric history cannot replace the actual Work borrow.
    std::mem::swap(&mut budget, &mut other);
    accounting(checkpoint.check(&budget));
    accounting(checkpoint.check(&other));
    std::mem::swap(&mut budget, &mut other);
    checkpoint.check(&budget).unwrap();
    budget.charge_work(1).unwrap();
    budget.reserve_storage(1).unwrap();
    checkpoint.check(&budget).unwrap();
}

#[test]
fn extraction_checkpoint_rejects_one_short_live_storage_without_charging() {
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, 100);
    budget.charge_work(7).unwrap();
    budget.reserve_storage(13).unwrap();
    let checkpoint = LiveExtractionPhaseV1::capture(&budget);
    checkpoint.check(&budget).unwrap();
    budget.release_storage(1).unwrap();
    accounting(checkpoint.check(&budget));
    assert_eq!(budget.storage(), 12);
    assert_eq!(budget.work(), 7);
    budget.reserve_storage(1).unwrap();
    checkpoint.check(&budget).unwrap();
}

struct Dropped(Rc<Cell<bool>>);
impl Drop for Dropped {
    fn drop(&mut self) {
        self.0.set(true);
    }
}

#[test]
fn extraction_scope_keeps_output_receipt_live_then_restores_only_its_delta() {
    for failure in [false, true] {
        let mut work = Work::new(100);
        let mut budget = Budget::new(&mut work, 100);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(13).unwrap();
        let dropped = Rc::new(Cell::new(false));
        let result = scoped(13, &mut budget, |budget| {
            let entry = LiveExtractionPhaseV1::capture(budget);
            budget.charge_work(5).unwrap();
            budget.reserve_storage(11).unwrap();
            entry.check(budget).unwrap();
            let prepared = LiveExtractionPhaseV1::capture(budget);
            let output = Dropped(dropped.clone());
            assert_eq!(budget.storage(), 24);
            assert!(!dropped.get());
            prepared.check(budget).unwrap();
            if failure {
                return Err(execution_error("test output refusal"));
            }
            drop(output);
            assert!(dropped.get());
            assert_eq!(budget.storage(), 24);
            Ok(())
        });
        assert_eq!(result.is_err(), failure);
        assert!(dropped.get());
        assert_eq!(budget.storage(), 13);
        assert_eq!(budget.work(), 12);
        assert_eq!(budget.peak_storage(), 24);
    }
}

#[test]
fn extraction_scope_exact_and_one_short_limits_preserve_inherited_history() {
    for (work_limit, storage_limit, expected) in [(12, 24, true), (11, 24, false), (12, 23, false)]
    {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(13).unwrap();
        let result = scoped(13, &mut budget, |budget| {
            let entry = LiveExtractionPhaseV1::capture(budget);
            budget.charge_work(5).map_err(resource)?;
            budget.reserve_storage(11).map_err(resource)?;
            entry.check(budget)
        });
        assert_eq!(result.is_ok(), expected);
        assert_eq!(budget.storage(), 13);
        assert_eq!(budget.work(), if work_limit == 11 { 7 } else { 12 });
        if expected {
            assert_eq!(budget.peak_storage(), 24);
        }
    }
}

#[test]
fn extraction_scope_foreign_ledger_is_not_refunded_as_the_original_floor() {
    let mut work = Work::new(100);
    let mut other_work = Work::new(100);
    let mut budget = Budget::new(&mut work, 100);
    let mut other = Budget::new(&mut other_work, 100);
    budget.reserve_storage(13).unwrap();
    other.reserve_storage(13).unwrap();
    let result = scoped(13, &mut budget, |budget| {
        let entry = LiveExtractionPhaseV1::capture(budget);
        budget.charge_work(5).unwrap();
        budget.reserve_storage(11).unwrap();
        other.charge_work(5).unwrap();
        other.reserve_storage(11).unwrap();
        std::mem::swap(budget, &mut other);
        entry.check(budget)
    });
    accounting(result);
    assert_eq!(budget.storage(), 24);
    assert_eq!(other.storage(), 24);
    assert_eq!(budget.work(), 5);
    assert_eq!(other.work(), 5);
}

fn metadata_fixture(names: &[&str]) -> (Vec<(String, WorkgroupSize)>, Vec<Kernel>) {
    let size = WorkgroupSize::new(64, 1, 1);
    let kernels = names
        .iter()
        .map(|name| {
            let mut kernel = Kernel::new(
                *name,
                *name,
                fe2o3_kernel_ir::LaunchDomain::D1 {
                    x: fe2o3_kernel_ir::LaunchExtent::Dynamic,
                },
            );
            kernel.workgroup_size = Some(size);
            kernel
        })
        .collect();
    (
        names
            .iter()
            .map(|name| ((*name).to_owned(), size))
            .collect(),
        kernels,
    )
}

fn metadata_refusal(result: Result7<()>) {
    assert!(matches!(
        result,
        Err(ProductionPipelineError::RankedVerification(
            crate::production_ranked_projection_v1::ProductionRankedVerificationErrorV1::RosterMetadata(
                "checked-output extraction source, ranked or workgroup roster"
            )
        ))
    ));
}

#[test]
fn extraction_metadata_exact_one_short_work_is_cumulative_and_allocation_free() {
    // These borrowed rows exercise metering, not source or launch admission.
    let (groups, kernels) = metadata_fixture(&["first", "second"]);
    let scan = 66 + 36 * kernels.len() + 2 * ("first".len() + "second".len());
    for limit in [7 + scan, 7 + scan - 1] {
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, 19);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(19).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = check_extraction_metadata(&groups, &kernels, &[1; 32], &[1; 32], &mut budget);
        if limit == 7 + scan {
            result.unwrap();
            assert_eq!(budget.work(), limit);
        } else {
            assert!(matches!(
                result,
                Err(ProductionPipelineError::CheckedOutputPolicy7Stage(
                    CheckedOutputPolicy7StageErrorV1::Resource(Resource::Work(_))
                ))
            ));
        }
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(budget.storage(), 19);
        assert_eq!(budget.peak_storage(), 19);
    }
}

#[test]
fn extraction_metadata_keeps_exact_roster_order_sizes_and_source_refusals() {
    let (groups, kernels) = metadata_fixture(&["first", "second"]);
    for fault in 0..6 {
        let mut groups = groups.clone();
        let mut kernels = kernels.clone();
        let mut source = [1; 32];
        match fault {
            0 => {
                groups.pop();
            }
            1 => groups.swap(0, 1),
            2 => groups[0].0.push('_'),
            3 => groups[0].1 = WorkgroupSize::new(32, 1, 1),
            4 => kernels[0].workgroup_size = None,
            5 => source[31] ^= 1,
            _ => unreachable!(),
        }
        let mut work = Work::new(10_000);
        let mut budget = Budget::new(&mut work, 19);
        budget.reserve_storage(19).unwrap();
        metadata_refusal(check_extraction_metadata(
            &groups,
            &kernels,
            &[1; 32],
            &source,
            &mut budget,
        ));
        assert_eq!(budget.storage(), 19);
    }
}

#[test]
fn extraction_metadata_charges_before_hostile_name_and_fixed_digest_comparison() {
    let (mut groups, kernels) = metadata_fixture(&["first"]);
    groups[0].0 = "wrong".into();
    let name_cost = 2 + 4 + "wrong".len() + "first".len() + 32;
    for limit in [name_cost - 1, name_cost] {
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, 0);
        let result = check_extraction_metadata(&groups, &kernels, &[1; 32], &[1; 32], &mut budget);
        if limit == name_cost {
            metadata_refusal(result);
        } else {
            assert!(matches!(
                result,
                Err(ProductionPipelineError::CheckedOutputPolicy7Stage(
                    CheckedOutputPolicy7StageErrorV1::Resource(Resource::Work(_))
                ))
            ));
        }
    }
    for limit in [65, 66] {
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, 0);
        let result = check_extraction_metadata(&[], &[], &[1; 32], &[2; 32], &mut budget);
        if limit == 66 {
            metadata_refusal(result);
        } else {
            assert!(matches!(
                result,
                Err(ProductionPipelineError::CheckedOutputPolicy7Stage(
                    CheckedOutputPolicy7StageErrorV1::Resource(Resource::Work(_))
                ))
            ));
        }
    }
}

#[test]
fn extraction_metadata_charges_full_actual_name_lengths_without_copying() {
    for length in [1, 4096] {
        let name = "k".repeat(length);
        let (groups, kernels) = metadata_fixture(&[&name]);
        let expected = 66 + 36 + 2 * length;
        let mut work = Work::new(expected);
        let mut budget = Budget::new(&mut work, 0);
        check_extraction_metadata(&groups, &kernels, &[1; 32], &[1; 32], &mut budget).unwrap();
        assert_eq!(budget.work(), expected);
        assert_eq!(budget.storage(), 0);
        assert_eq!(budget.peak_storage(), 0);
    }
}
