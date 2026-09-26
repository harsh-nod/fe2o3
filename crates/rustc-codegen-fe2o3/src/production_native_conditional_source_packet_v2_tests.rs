//! Inert framing/account tests, not a successful source or formula proof fixture.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{cell::Cell, error::Error, rc::Rc};

const FLOOR: usize = 19;
struct Dropped(Rc<Cell<usize>>);
impl Drop for Dropped {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

#[test]
fn conditional_packet_capacity_exact_one_short_and_denials_stay_on_original_account() {
    let run = |budget: &mut Budget<'_>| {
        retain(budget, |budget| {
            let mut values = vector::<u64>(3, budget)?;
            budget.charge_work(5)?;
            values.extend_from_slice(&[1, 2, 3]);
            let retained = size_of::<Vec<u64>>() + values.capacity() * size_of::<u64>();
            Ok((values, retained))
        })
    };
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, 1000);
    budget.reserve_storage(FLOOR).unwrap();
    let values = run(&mut budget).unwrap();
    let peak = budget.peak_storage();
    let cost = budget.work();
    assert_eq!(budget.storage(), peak);
    drop(values);
    budget.release_storage(peak - FLOOR).unwrap();
    for (work_limit, storage_limit, expected) in
        [(cost, peak, 0), (cost - 1, peak, 1), (cost, peak - 1, 2)]
    {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR).unwrap();
        let account = budget.work_ledger_identity_v1();
        let result = run(&mut budget);
        match expected {
            0 => {
                let values = result.unwrap();
                assert_eq!(values, [1, 2, 3]);
                assert_eq!(budget.storage(), peak);
                drop(values);
                budget.release_storage(peak - FLOOR).unwrap();
            }
            1 => {
                assert!(matches!(result, Err(E::Resource(Resource::Work(_)))));
                assert_eq!(budget.failed_work(), Some(cost));
                assert_eq!(budget.storage(), peak);
            }
            _ => {
                assert!(matches!(result, Err(E::Resource(Resource::Storage(_)))));
                assert_eq!(budget.failed_storage(), Some(peak));
                assert_eq!(budget.storage(), FLOOR);
            }
        }
        assert!(budget.work_ledger_identity_v1() == account);
    }
    assert!(budget.charge_work(usize::MAX).is_err());
    assert!(budget.reserve_storage(usize::MAX).is_err());
    let denials = (budget.failed_work(), budget.failed_storage());
    drop(run(&mut budget).unwrap());
    assert_eq!((budget.failed_work(), budget.failed_storage()), denials);
}

#[test]
fn conditional_packet_releases_only_dead_scratch_and_preserves_refusal_unwind() {
    for mode in 0..3 {
        let mut work = Work::new(100);
        let mut budget = Budget::new(&mut work, 100);
        budget.reserve_storage(FLOOR).unwrap();
        let drops = Rc::new(Cell::new(0));
        let result = catch_unwind(AssertUnwindSafe(|| {
            retain(&mut budget, |budget| {
                budget.reserve_storage(31)?;
                budget.charge_work(7)?;
                let scratch = Dropped(drops.clone());
                if mode == 1 {
                    return Err(E::Mismatch("inert component refusal"));
                }
                if mode == 2 {
                    panic!("inert component unwind");
                }
                drop(scratch);
                Ok((23u64, 8))
            })
        }));
        assert_eq!(drops.get(), 1);
        assert_eq!(budget.work(), 7);
        if mode == 0 {
            assert_eq!(result.unwrap().unwrap(), 23);
            assert_eq!(budget.storage(), FLOOR + 8);
        } else {
            assert_eq!(budget.storage(), FLOOR + 31);
            assert!(if mode == 1 {
                result.unwrap().is_err()
            } else {
                result.is_err()
            });
        }
    }
}

#[test]
fn conditional_packet_foreign_account_damaged_floor_and_underpaid_owner_refuse_transfer() {
    for mode in 0..3 {
        let mut original = Work::new(100);
        let mut foreign = Work::new(100);
        let mut budget = Budget::new(&mut original, 100);
        budget.reserve_storage(FLOOR).unwrap();
        let account = budget.work_ledger_identity_v1();
        let drops = Rc::new(Cell::new(0));
        let result = retain(&mut budget, |budget| {
            if mode == 0 {
                *budget = Budget::new(&mut foreign, 100);
                budget.reserve_storage(FLOOR + 7)?;
            } else if mode == 1 {
                budget.release_storage(1)?;
            } else {
                budget.reserve_storage(7)?;
            }
            Ok((Dropped(drops.clone()), 8))
        });
        assert!(matches!(result, Err(E::Resource(Resource::Accounting))));
        assert_eq!(drops.get(), 1);
        assert_eq!(
            budget.storage(),
            if mode == 1 { FLOOR - 1 } else { FLOOR + 7 }
        );
        assert_eq!(budget.work_ledger_identity_v1() == account, mode != 0);
    }
}

#[test]
fn conditional_packet_c1_refusal_remains_a_typed_cause_and_cannot_refund_outer_owner() {
    let mut work = Work::new(10_000);
    let mut budget = Budget::new(&mut work, 10_000);
    budget.reserve_storage(FLOOR).unwrap();
    let error = retain::<()>(&mut budget, |budget| {
        budget.reserve_storage(41)?;
        let error = validate_native_conditional_source_packet_v2(b"not a packet", &[], budget)
            .err()
            .expect("inert invalid frame must refuse before any proof");
        Err(E::Replay(error))
    })
    .unwrap_err();
    assert!(
        error
            .source()
            .unwrap()
            .is::<fe2o3_verifier::NativeConditionalSourceProofErrorV2>()
    );
    assert_eq!(budget.storage(), FLOOR + 41);
}

#[test]
fn conditional_packet_full_binding_order_rejects_duplicates_and_charges_comparisons() {
    let mut work = Work::new(1000);
    let mut budget = Budget::new(&mut work, 1000);
    let bindings = [[3; 32], [1; 32], [2; 32]];
    let order = canonical_order(3, |i| bindings[i], &mut budget).unwrap();
    assert_eq!(order, [1, 2, 0]);
    assert_eq!(budget.work(), 3 + 3 + 3 * 33);
    let before = budget.work();
    assert!(matches!(
        canonical_order(2, |_| [1; 32], &mut budget),
        Err(E::Mismatch("duplicate kernel binding"))
    ));
    assert_eq!(budget.work() - before, 3 + 2 + 33);
    let before = budget.storage();
    assert!(matches!(
        vector::<u64>(usize::MAX, &mut budget),
        Err(E::Resource(Resource::Arithmetic))
    ));
    assert_eq!(budget.storage(), before);
}

#[test]
fn conditional_packet_induction_adopts_existing_report_with_prepaid_bounds() {
    use crate::production_ranked_projection_v1::with_backend_checked_output_policy3_roster_v1;
    use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        with_backend_checked_output_policy3_roster_v1(profile, |_, ranked, _| {
            let report = ranked.roots()[0].verification().semantic_u32_induction();
            let expected = Induction::from_report(report).unwrap();
            let one = size_of::<Induction>()
                + report.certificates().len()
                    * size_of::<fe2o3_mir_model::SemanticU32InductionNoOverflowCertificateEvidenceV1>(
                    );
            for short in [false, true] {
                let mut work = Work::new(4 * one);
                let mut budget = Budget::new(&mut work, FLOOR + 4 * one - usize::from(short));
                budget.reserve_storage(FLOOR).unwrap();
                let result = encode_induction(report, &mut budget);
                if short {
                    assert!(matches!(result, Err(E::Resource(Resource::Storage(_)))));
                    assert_eq!(budget.work(), 0);
                } else {
                    assert_eq!(
                        result.unwrap().canonical_bytes(),
                        expected.canonical_bytes()
                    );
                    assert_eq!(budget.work(), 4 * one);
                }
            }
        });
    }
}

#[test]
fn original_envelope_leaf_preserves_v1_bytes_work_and_exact_capacity_boundaries() {
    use crate::production_ranked_projection_v1::with_backend_checked_output_policy3_roster_v1;
    use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
    use fe2o3_kernel_ir::InertCanonicalKernelIrContractCatalogV1 as Catalog;
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        with_backend_checked_output_policy3_roster_v1(profile, |owner, _, _| {
            let source = owner.source_semantic_kir();
            let native = source.pre_ranked_executable().unwrap();
            let semantic = *source.semantic().semantic().semantic_sha256().as_bytes();
            let mut catalog_work = Work::new(usize::MAX);
            let mut catalog_budget = Budget::new(&mut catalog_work, usize::MAX);
            let (catalog, receipt) =
                Catalog::from_rows_with_budget(semantic, &[], &[], &mut catalog_budget).unwrap();
            catalog_budget
                .reserve_storage(receipt.retained_storage())
                .unwrap();
            let length =
                112 + native.canonical().canonical_bytes().len() + catalog.canonical_bytes().len();
            let expected_work = catalog_budget.work() + length;
            let mut work = Work::new(expected_work);
            let mut budget = Budget::new(&mut work, usize::MAX);
            budget.reserve_storage(FLOOR).unwrap();
            let (bytes, subject) =
                packet::encode_original_native_envelope_v1(semantic, native, &mut budget).unwrap();
            let expected = fe2o3_compiler_lineage::encode_native_neutral_module_v1(
                &subject,
                native.canonical().canonical_bytes(),
                catalog.canonical_bytes(),
            )
            .unwrap();
            assert_eq!(bytes, expected);
            assert_eq!(
                subject.graph_digest(),
                native.canonical().identity().digest()
            );
            assert_eq!(subject.catalog_digest(), catalog.digest());
            assert_eq!(budget.work(), expected_work);
            let peak = budget.peak_storage();
            for (work_limit, storage_limit, ok) in [
                (expected_work, peak, true),
                (expected_work - 1, peak, false),
                (expected_work, peak - 1, false),
            ] {
                let mut work = Work::new(work_limit);
                let mut budget = Budget::new(&mut work, storage_limit);
                budget.reserve_storage(FLOOR).unwrap();
                let result =
                    packet::encode_original_native_envelope_v1(semantic, native, &mut budget);
                assert_eq!(result.is_ok(), ok);
                if ok {
                    assert_eq!(result.unwrap().0, bytes);
                } else {
                    assert!(budget.failed_work().is_some() || budget.failed_storage().is_some());
                }
            }
        });
    }
}
