//! Inert replay/join components, not genuine source requests or proof receipts.
use super::fixtures::*;
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use std::panic::{AssertUnwindSafe, catch_unwind};

fn replay(
    fixture: &Fixture,
    budget: &mut Budget<'_>,
    check: impl FnMut(u32, &mut Budget<'_>) -> Result<(), Error>,
) -> Result<(), Error> {
    portable::with_replayed_output_writes_v1(fixture.input().replay, budget, |effects, budget| {
        read_premises::check_replay_v1(&fixture.ir, effects, budget, check)
    })
    .map_err(|error| Error::ProofExecution(error.to_string()))?
}

fn exact_input(raw: u32, _: &mut Budget<'_>) -> Result<(), Error> {
    require(raw == 1, "component raw input substitution")
}

#[test]
fn portable_join_replayed_bounds_use_original_account_exact_and_one_short() {
    let fixture = Fixture::new();
    let run = |limit, storage| {
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, storage);
        budget.charge_work(17).unwrap();
        budget.reserve_storage(31).unwrap();
        let account = budget.work_ledger_identity_v1();
        let result = replay(&fixture, &mut budget, exact_input);
        assert!(budget.work_ledger_identity_v1() == account);
        assert_eq!(budget.storage(), 31);
        (
            result,
            budget.work(),
            budget.peak_storage(),
            budget.failed_work(),
            budget.failed_storage(),
        )
    };
    let (result, work, peak, _, _) = run(usize::MAX, usize::MAX);
    result.unwrap();
    run(work, peak).0.unwrap();
    let short_work = run(work - 1, peak);
    assert!(short_work.0.is_err());
    assert_eq!(short_work.3, Some(work));
    let short_storage = run(work, peak - 1);
    assert!(short_storage.0.is_err());
    assert_eq!(short_storage.4, Some(peak));
}

#[test]
fn portable_join_replayed_bounds_refuse_coherent_control_occurrence_and_input_changes() {
    for mutation in 0..7 {
        let mut fixture = Fixture::new();
        match mutation {
            0 => {
                let ReferenceTerminatorV1::Assert { expected, .. } =
                    &mut fixture.ir.blocks[0].terminator
                else {
                    unreachable!()
                };
                *expected = false;
            }
            1 => {
                fixture.ir.blocks[0].assignments[0].value = ReferenceValueV1::InputLength {
                    reference_argument: 2,
                }
            }
            2 => {
                let mut early = fixture.ir.blocks[0].assignments.to_vec();
                let mut output = fixture.ir.blocks[1].assignments[0].clone();
                output.statement = 2;
                early.push(output);
                fixture.ir.blocks[0].assignments = early.into_boxed_slice();
                fixture.ir.blocks[1].assignments = Box::default();
            }
            3 => fixture.ir.blocks[0].terminator = ReferenceTerminatorV1::Goto { target: 1 },
            4 => {
                let mut blocks = fixture.ir.blocks.to_vec();
                blocks.push(ReferenceBlockV1 {
                    block: 2,
                    assignments: vec![ReferenceAssignmentV1 {
                        statement: 0,
                        destination: local(6),
                        value: ReferenceValueV1::Use(ReferenceOperandV1::Constant(
                            ReferenceConstantV1::Scalar {
                                scalar: ReferenceScalarTypeV1::U32,
                                bits: 7,
                            },
                        )),
                    }]
                    .into_boxed_slice(),
                    terminator: ReferenceTerminatorV1::Return,
                });
                fixture.ir.local_count = 7;
                fixture.ir.blocks = blocks.into_boxed_slice();
            }
            5 => {
                fixture.ir.blocks[1].assignments[0].value = ReferenceValueV1::Use(
                    ReferenceOperandV1::Constant(ReferenceConstantV1::Scalar {
                        scalar: ReferenceScalarTypeV1::F32,
                        bits: 0,
                    }),
                )
            }
            6 => {
                let ReferenceTerminatorV1::Assert { bounds_check, .. } =
                    &mut fixture.ir.blocks[0].terminator
                else {
                    unreachable!()
                };
                *bounds_check = None;
            }
            _ => unreachable!(),
        }
        // Recompute the inert digest AND both cached outputs, so stale cache
        // rejection is not standing in for the read/control check.
        fixture.refresh();
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        budget.reserve_storage(31).unwrap();
        assert!(
            replay(&fixture, &mut budget, exact_input).is_err(),
            "mutation {mutation}"
        );
        assert_eq!(budget.storage(), 31);
    }
}

#[test]
fn portable_join_nested_checker_preserves_denials_and_unwind_cleanup() {
    let fixture = Fixture::new();
    for mode in 0..3 {
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, 1_000_000);
        budget.charge_work(17).unwrap();
        budget.reserve_storage(31).unwrap();
        assert!(budget.charge_work(1_000_000).is_err());
        assert!(budget.reserve_storage(1_000_000).is_err());
        let history = (budget.failed_work(), budget.failed_storage());
        let account = budget.work_ledger_identity_v1();
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            replay(&fixture, &mut budget, |raw, budget| {
                exact_input(raw, budget)?;
                match mode {
                    0 => Ok(()),
                    1 => Err(reject("component read consumer refusal")),
                    _ => panic!("component read consumer panic"),
                }
            })
        }));
        match mode {
            0 => outcome.unwrap().unwrap(),
            1 => assert!(matches!(
                outcome.unwrap(),
                Err(Error::UnsupportedReference(
                    "component read consumer refusal"
                ))
            )),
            _ => assert!(outcome.is_err()),
        }
        assert!(budget.work_ledger_identity_v1() == account);
        assert_eq!(budget.storage(), 31);
        assert_eq!((budget.failed_work(), budget.failed_storage()), history);
        assert!(budget.work() > 17);
        assert!(budget.peak_storage() > 31);
    }
}

#[test]
fn portable_join_funded_foreign_account_and_floor_damage_never_receive_scratch_refunds() {
    let fixture = Fixture::new();
    for foreign in [false, true] {
        for unwind in [false, true] {
            let mut original = Owned::new(Work::new(1_000_000), 1_000_000);
            let mut protected = 0;
            let mut remaining = 0;
            original.with_budget(|budget| {
                budget.reserve_storage(31).unwrap();
                let mut first = true;
                let outcome = catch_unwind(AssertUnwindSafe(|| {
                    replay(&fixture, budget, |_, budget| {
                        if first {
                            first = false;
                            protected = budget.storage();
                            if foreign {
                                let mut replacement = Budget::new(
                                    Box::leak(Box::new(Work::new(1_000_000))),
                                    1_000_000,
                                );
                                replacement.reserve_storage(protected).unwrap();
                                *budget = replacement;
                            } else {
                                budget.release_storage(1).unwrap();
                            }
                            if unwind {
                                panic!("component corrupted-account unwind");
                            }
                        }
                        Ok(())
                    })
                }));
                if unwind {
                    assert!(outcome.is_err());
                } else {
                    assert!(outcome.unwrap().is_err());
                }
                remaining = budget.storage();
            });
            assert!(protected > 31);
            assert_eq!(remaining, protected - usize::from(!foreign));
            assert_eq!(original.storage(), protected - usize::from(!foreign));
        }
    }
}

#[test]
fn portable_join_subjects_preserve_existing_selected_identity_fields() {
    let kernel = identity(10);
    let reference = identity(20);
    let expected = reference_subjects_v1(&kernel, &reference).unwrap();
    for mutation in 0..4 {
        let (mut k, mut r) = (kernel, reference);
        match mutation {
            0 => k.function_sha256[0] ^= 1,
            1 => k.rustc_mir_body_sha256[0] ^= 1,
            2 => r.function_sha256[0] ^= 1,
            3 => r.rustc_mir_body_sha256[0] ^= 1,
            _ => unreachable!(),
        }
        assert_ne!(reference_subjects_v1(&k, &r).unwrap(), expected);
    }
    // The remaining full identity fields stay in input/producer custody and
    // the existing source-root check; this function is not full CPU commitment.
}
