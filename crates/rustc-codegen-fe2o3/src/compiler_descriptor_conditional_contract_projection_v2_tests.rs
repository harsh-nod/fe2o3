//! Inert codec fixtures only; live V2 proof/row coupling is checked by the
//! existing protected-source retention and generated-fields observers.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

#[allow(dead_code)]
#[path = "../../fe2o3-kernel-descriptor/tests/support/conditional_invocation_v2.rs"]
mod fixture;

fn ample<R>(run: impl FnOnce(&mut Budget<'_>) -> R) -> R {
    let mut work = Work::new(100_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    run(&mut budget)
}

#[test]
fn v2_keeps_cpu_commitment_rows_and_distinct_wire_identity() {
    let f = fixture::Fixture::new(2, 3);
    ample(|budget| {
        let v1 = with_encoded(&f.input(), budget, |view, _| {
            assert!(
                decode_conditional_invocation_contract_v2(
                    view.canonical_bytes(),
                    &mut fixture::free
                )
                .is_err()
            );
            view.canonical_bytes().to_vec()
        })
        .unwrap();
        with_encoded_v2(&fixture::input(&f, [17; 32]), budget, |view, budget| {
            assert_eq!(view.canonical_bytes(), fixture::wire(&f));
            assert_eq!(view.theorem().cpu_input_commitment, [17; 32]);
            assert_ne!(
                view.theorem().statement_identity,
                f.theorem.statement_identity
            );
            assert_eq!(*view.subjects(), f.subjects);
            assert_eq!(view.output(), f.output);
            assert_eq!(view.canonical_bytes().len(), v1.len() + 32);
            assert!(
                decode_conditional_invocation_contract_v1(
                    view.canonical_bytes(),
                    &mut fixture::free
                )
                .is_err()
            );
            let mut charge = |n| budget.charge_work(n);
            let mut roots = view.typed_roots();
            for expected in &f.roots {
                assert_eq!(roots.next(&mut charge).unwrap(), Some(*expected));
            }
            assert_eq!(roots.next(&mut charge).unwrap(), None);
            let mut arguments = view.arguments();
            for expected in &f.arguments {
                assert_eq!(arguments.next(&mut charge).unwrap(), Some(*expected));
            }
            assert_eq!(arguments.next(&mut charge).unwrap(), None);
            let mut reads = view.reads();
            for expected in &f.reads {
                assert_eq!(reads.next(&mut charge).unwrap(), Some(*expected));
            }
            assert_eq!(reads.next(&mut charge).unwrap(), None);
            let mut premises = view.premises();
            for expected in &f.premises {
                assert_eq!(premises.next(&mut charge).unwrap(), Some(*expected));
            }
            assert_eq!(premises.next(&mut charge).unwrap(), None);
        })
        .unwrap();
        assert_eq!(budget.storage(), 0);
    });
}

#[test]
fn stale_seven_field_statement_and_v1_relabel_never_enter_callback() {
    let f = fixture::Fixture::new(2, 2);
    for case in 0..9 {
        let mut input = fixture::input(&f, [17; 32]);
        match case {
            0 => input.subjects.aggregate_statement_identity[0] ^= 1,
            1 => input.theorem.generated_source_identity[0] ^= 1,
            2 => input.theorem.staging_receipt_identity[0] ^= 1,
            3 => input.theorem.staging_obligation_identity[0] ^= 1,
            4 => input.theorem.staging_signer_identity[0] ^= 1,
            5 => input.theorem.staging_execution_identity[0] ^= 1,
            6 => input.theorem.cpu_input_commitment[0] ^= 1,
            7 => input.theorem.statement_identity[0] ^= 1,
            8 => input.theorem.statement_identity = f.theorem.statement_identity,
            _ => unreachable!(),
        }
        ample(|budget| {
            budget.reserve_storage(19).unwrap();
            let account = budget.work_ledger_identity_v1();
            assert!(matches!(
                with_encoded_v2(&input, budget, |_, _| panic!("stale V2 theorem accepted")),
                Err(Error::Codec(_))
            ));
            assert_eq!(budget.storage(), 19);
            assert!(budget.work_ledger_identity_v1() == account);
        });
    }
}

#[test]
fn exact_v2_work_storage_limits_keep_inherited_floor_and_history() {
    let f = fixture::Fixture::new(2, 2);
    let input = fixture::input(&f, [17; 32]);
    let needed = ample(|b| {
        with_encoded_v2(&input, b, |_, _| ()).unwrap();
        b.work()
    });
    for limit in [
        0,
        1,
        MAX_CONDITIONAL_INVOCATION_BYTES_V2,
        needed - 1,
        needed,
    ] {
        let mut work = Work::new(limit + 7);
        let mut budget = Budget::new(&mut work, ENCODING_STORAGE_V2 + 23);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(23).unwrap();
        let account = budget.work_ledger_identity_v1();
        let mut called = false;
        let result = with_encoded_v2(&input, &mut budget, |_, _| called = true);
        assert_eq!(result.is_ok(), limit == needed);
        assert_eq!(called, limit == needed);
        assert_eq!(budget.storage(), 23);
        assert!(budget.work_ledger_identity_v1() == account);
        assert!(budget.work() <= limit + 7);
        assert_eq!(budget.failed_work().is_some(), limit < needed);
        let denied = budget.failed_work();
        budget.charge_work(0).unwrap();
        assert_eq!(budget.failed_work(), denied);
        if limit == needed {
            assert_eq!(budget.work(), needed + 7);
            assert_eq!(budget.peak_storage(), ENCODING_STORAGE_V2 + 23);
        }
    }
    let mut work = Work::new(needed);
    let mut budget = Budget::new(&mut work, ENCODING_STORAGE_V2 + 22);
    budget.reserve_storage(23).unwrap();
    assert!(matches!(
        with_encoded_v2(&input, &mut budget, |_, _| panic!("unpaid V2 storage")),
        Err(Error::Resource(Resource::Storage(_)))
    ));
    assert_eq!(budget.storage(), 23);
    assert_eq!(budget.work(), 1);
    assert_eq!(budget.failed_storage(), Some(ENCODING_STORAGE_V2 + 23));
}

#[test]
fn v2_callback_error_and_unwind_release_only_codec_scratch() {
    let f = fixture::Fixture::new(2, 2);
    ample(|budget| {
        budget.reserve_storage(11).unwrap();
        let account = budget.work_ledger_identity_v1();
        let result = with_encoded_v2(&fixture::input(&f, [17; 32]), budget, |_, b| {
            assert!(b.work_ledger_identity_v1() == account);
            b.reserve_storage(17).unwrap();
            Err::<(), _>("consumer error")
        })
        .unwrap();
        assert_eq!(result, Err("consumer error"));
        assert_eq!(budget.storage(), 28);
        let work = budget.work();
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let _ = with_encoded_v2(&fixture::input(&f, [17; 32]), budget, |_, b| {
                    b.reserve_storage(3).unwrap();
                    panic!("consumer unwind");
                });
            }))
            .is_err()
        );
        assert_eq!(budget.storage(), 31);
        assert!(budget.work() > work);
        assert!(budget.work_ledger_identity_v1() == account);
    });
}

#[test]
fn v2_foreign_account_and_damaged_floor_cannot_return_success() {
    let f = fixture::Fixture::new(2, 2);
    let mut first_work = Work::new(100_000_000);
    let mut second_work = Work::new(100_000_000);
    let mut first = Budget::new(&mut first_work, 1_000_000);
    let mut second = Budget::new(&mut second_work, 1_000_000);
    first.reserve_storage(11).unwrap();
    second.reserve_storage(77).unwrap();
    let result = with_encoded_v2(&fixture::input(&f, [17; 32]), &mut first, |_, b| {
        std::mem::swap(b, &mut second);
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    assert_eq!(first.storage(), 77);
    assert_eq!(second.storage(), 11 + ENCODING_STORAGE_V2);
    let result = with_encoded_v2(&fixture::input(&f, [17; 32]), &mut first, |_, b| {
        b.release_storage(1).unwrap();
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    assert_eq!(first.storage(), 76 + ENCODING_STORAGE_V2);
}

#[test]
fn v2_generated_fields_scope_returns_copy_unreserved_for_original_owner() {
    let f = fixture::Fixture::new(2, 2);
    ample(|budget| {
        budget.reserve_storage(29).unwrap();
        let account = budget.work_ledger_identity_v1();
        let owned = budget
            .with_prepaid_scope::<_, Error>(29, 1, 1, 13, |b| {
                with_encoded_v2(
                    &fixture::input(&f, [17; 32]),
                    b,
                    |view, b| -> Result<Vec<u8>, Error> {
                        b.reserve_storage(view.canonical_bytes().len())?;
                        b.charge_work(view.canonical_bytes().len())?;
                        Ok(view.canonical_bytes().to_vec())
                    },
                )?
            })
            .unwrap();
        assert_eq!(budget.storage(), 29);
        assert!(budget.work_ledger_identity_v1() == account);
        budget.reserve_storage(owned.capacity()).unwrap();
        assert_eq!(owned, fixture::wire(&f));
        assert_eq!(budget.storage(), 29 + owned.capacity());
    });
}
