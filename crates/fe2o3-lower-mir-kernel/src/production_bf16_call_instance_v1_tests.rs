//! Inert predicate/resource controls. Genuine source and current materializer
//! refusal are separate frontend tests; these cannot construct source authority.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::cell::Cell;
const INIT: usize = DEPTH + 32 + 7 + 8;

#[test]
fn exact_scratch_and_work_preserve_incoming_floor_and_identity() {
    let mut work = Work::new(INIT + 9);
    let mut budget = Budget::new(&mut work, SCRATCH + 17);
    budget.reserve_storage(17).unwrap();
    budget.charge_work(9).unwrap();
    let identity = budget.work_ledger_identity_v1();
    assert_eq!(scoped(&mut budget, |_| Ok(7)).unwrap(), 7);
    assert_eq!(budget.storage(), 17);
    assert_eq!(budget.peak_storage(), SCRATCH + 17);
    assert_eq!(budget.work(), INIT + 9);
    assert!(budget.work_ledger_identity_v1() == identity);
}
#[test]
fn zero_and_short_storage_refuse_before_body() {
    for cap in [0, SCRATCH - 1] {
        let mut work = Work::new(INIT);
        let mut budget = Budget::new(&mut work, cap);
        let entered = Cell::new(false);
        assert!(
            scoped(&mut budget, |_| {
                entered.set(true);
                Ok(())
            })
            .is_err()
        );
        assert!(!entered.get());
        assert_eq!(budget.storage(), 0);
        assert_eq!(budget.work(), 0);
        assert!(budget.failed_storage().is_some());
    }
}
#[test]
fn zero_and_short_work_refuse_before_body_and_refund_only_scratch() {
    for cap in [0, INIT - 1] {
        let mut work = Work::new(cap);
        let mut budget = Budget::new(&mut work, SCRATCH + 17);
        budget.reserve_storage(17).unwrap();
        let entered = Cell::new(false);
        assert!(
            scoped(&mut budget, |_| {
                entered.set(true);
                Ok(())
            })
            .is_err()
        );
        assert!(!entered.get());
        assert_eq!(budget.storage(), 17);
        assert_eq!(budget.work(), 0);
        assert!(budget.failed_work().is_some());
    }
}
#[test]
fn callback_storage_remains_charged_on_success_error_and_panic() {
    for case in 0..3 {
        let mut work = Work::new(INIT + 3);
        let mut budget = Budget::new(&mut work, SCRATCH + 19);
        budget.reserve_storage(17).unwrap();
        let out = scoped(&mut budget, |b| {
            b.reserve_storage(2)?;
            b.charge_work(3)?;
            match case {
                0 => Ok(()),
                1 => refuse("exact callback refusal"),
                _ => panic!("bounded callback control"),
            }
        });
        match case {
            0 => assert!(out.is_ok()),
            1 => assert!(matches!(
                out,
                Err(CallError::Unavailable("exact callback refusal"))
            )),
            _ => assert!(matches!(out, Err(CallError::CallbackPanicked))),
        }
        assert_eq!(budget.storage(), 19);
        assert_eq!(budget.work(), INIT + 3);
    }
}
#[test]
fn ignored_work_denial_is_sticky_and_prevents_success() {
    let mut work = Work::new(INIT);
    let mut budget = Budget::new(&mut work, SCRATCH);
    assert!(matches!(
        scoped(&mut budget, |b| {
            let _ = b.charge_work(1);
            Ok(())
        }),
        Err(CallError::Resource(Resource::Accounting))
    ));
    assert_eq!(budget.storage(), 0);
    assert_eq!(budget.work(), INIT);
    assert!(budget.failed_work().is_some());
    let entered = Cell::new(false);
    assert!(
        scoped(&mut budget, |_| {
            entered.set(true);
            Ok(())
        })
        .is_err()
    );
    assert!(!entered.get());
}
#[test]
fn ignored_storage_denial_is_sticky_and_prevents_success() {
    let mut work = Work::new(INIT);
    let mut budget = Budget::new(&mut work, SCRATCH);
    assert!(matches!(
        scoped(&mut budget, |b| {
            let _ = b.reserve_storage(1);
            Ok(())
        }),
        Err(CallError::Resource(Resource::Accounting))
    ));
    assert_eq!(budget.storage(), 0);
    assert!(budget.failed_storage().is_some());
}
#[test]
fn undercut_floor_is_refused_without_fabricated_refund() {
    let mut work = Work::new(INIT);
    let mut budget = Budget::new(&mut work, SCRATCH + 17);
    budget.reserve_storage(17).unwrap();
    assert!(matches!(
        scoped(&mut budget, |b| {
            b.release_storage(1)?;
            Ok(())
        }),
        Err(CallError::Resource(Resource::Accounting))
    ));
    assert_eq!(budget.storage(), SCRATCH + 16);
}
#[test]
fn replacement_ledger_is_not_repaired() {
    let mut a = Work::new(INIT);
    let mut b = Work::new(INIT);
    let mut budget = Budget::new(&mut a, SCRATCH);
    let foreign = Budget::new(&mut b, SCRATCH);
    assert!(matches!(
        scoped(&mut budget, move |b| {
            *b = foreign;
            Ok(())
        }),
        Err(CallError::Resource(Resource::Accounting))
    ));
    assert_eq!(budget.storage(), 0);
    assert_eq!(budget.work(), 0);
}
#[test]
fn only_identity_and_swap01_return_component_ancestries_admit() {
    let mut admitted = 0;
    for a in 0..4 {
        for b in 0..4 {
            for c in 0..4 {
                for d in 0..4 {
                    let row = [a, b, c, d];
                    let expected = row == [0, 1, 2, 3] || row == [1, 0, 2, 3];
                    assert_eq!(ssa::permutation(row).is_ok(), expected);
                    admitted += usize::from(expected);
                }
            }
        }
    }
    assert_eq!(admitted, 2);
    assert!(ssa::permutation([0, 1, 2, 4]).is_err());
}
#[test]
fn selected_roles_are_unique_and_seven_is_additive_not_old_p0() {
    assert_eq!(Role::ALL.len(), 6);
    assert_eq!(CallRole::ALL.len(), 7);
    let mut bits = 0u8;
    for role in CallRole::ALL {
        assert_eq!(bits & (1 << role.index()), 0);
        bits |= 1 << role.index();
    }
    assert_eq!(bits, 127);
    let mut slot = None;
    one(&mut slot, CallRole::Context).unwrap();
    assert!(one(&mut slot, CallRole::Context).is_err());
}
#[test]
fn only_exact_wave64_profile_and_explicit_values_terminal_are_selected() {
    let ty = SemanticTypeIdV1::from_index(0);
    for width in [16, 32, 64] {
        let out = source::role(&SemanticCompilerIntrinsicOperationV1::WaveLaneCurrent {
            lane: ty,
            wave_width: width,
        });
        if width == 64 {
            assert_eq!(out.unwrap(), Some(CallRole::Lane));
        } else {
            assert!(out.is_err());
        }
    }
    assert_eq!(
        source::role(
            &SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorIntoValues {
                fragment: ty,
                values: ty,
            }
        )
        .unwrap(),
        Some(CallRole::Values)
    );
    assert_eq!(
        super::super::source::role(
            &SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorIntoValues {
                fragment: ty,
                values: ty,
            }
        )
        .unwrap(),
        None
    );
}
#[test]
fn source_cycle_checks_remain_closed_with_original_ledger() {
    let mut work = Work::new(1000);
    let mut budget = Budget::new(&mut work, 128);
    assert!(source::cycle_control(2, &[(0, 1)], &mut budget).is_ok());
    assert_eq!(budget.storage(), 0);
    assert!(source::cycle_control(2, &[(0, 1), (1, 0)], &mut budget).is_err());
    assert_eq!(budget.storage(), 0);
}

#[test]
fn actual_array_length_and_minimum_length_are_distinct_bounded_facts() {
    for offset in 0..4 {
        for minimum_length in offset + 1..=4 {
            assert_eq!(
                ssa::component_index(SemanticProjectionKindV1::ConstantIndex {
                    offset,
                    minimum_length,
                    from_end: false,
                })
                .unwrap(),
                offset as usize
            );
        }
    }
    for kind in [
        SemanticProjectionKindV1::ConstantIndex {
            offset: 4,
            minimum_length: 5,
            from_end: false,
        },
        SemanticProjectionKindV1::ConstantIndex {
            offset: 0,
            minimum_length: 0,
            from_end: false,
        },
        SemanticProjectionKindV1::ConstantIndex {
            offset: 1,
            minimum_length: 1,
            from_end: false,
        },
        SemanticProjectionKindV1::ConstantIndex {
            offset: 0,
            minimum_length: 4,
            from_end: true,
        },
        SemanticProjectionKindV1::Field(0),
        SemanticProjectionKindV1::Dereference,
    ] {
        assert!(ssa::component_index(kind).is_err());
    }
}

#[test]
fn bounded_headers_admit_exact_combined_32_with_original_prefix() {
    let mut work = Work::new(39);
    let mut budget = Budget::new(&mut work, 17);
    budget.reserve_storage(17).unwrap();
    budget.charge_work(7).unwrap();
    let identity = budget.work_ledger_identity_v1();
    let (mut blocks, mut items) = (1, 0);
    let mut seen = 0;
    source::scan_bounded_lengths(
        31,
        0,
        &mut blocks,
        &mut items,
        |index| {
            assert_eq!(index, seen);
            seen += 1;
            0
        },
        &mut budget,
    )
    .unwrap();
    assert_eq!((blocks, items, seen), (32, 0, 31));
    assert_eq!(budget.work(), 39);
    assert_eq!(budget.storage(), 17);
    assert!(budget.work_ledger_identity_v1() == identity);
}

#[test]
fn bounded_headers_refuse_combined_33_before_any_block_header() {
    let mut work = Work::new(1);
    let mut budget = Budget::new(&mut work, 0);
    let (mut blocks, mut items) = (1, 0);
    let mut seen = 0;
    let result = source::scan_bounded_lengths(
        32,
        0,
        &mut blocks,
        &mut items,
        |_| {
            seen += 1;
            0
        },
        &mut budget,
    );
    assert!(matches!(
        result,
        Err(CallError::Unavailable(
            "finite source block/local/statement cap"
        ))
    ));
    assert_eq!(seen, 0);
    assert_eq!(budget.work(), 1);
    assert_eq!(budget.storage(), 0);
}

#[test]
fn bounded_headers_admit_4096_and_refuse_4097_locals_before_iteration() {
    for (locals, expected) in [(4093, true), (4096, false)] {
        let mut work = Work::new(3);
        let mut budget = Budget::new(&mut work, 0);
        let (mut blocks, mut items) = (0, 1);
        let mut seen = 0;
        let result = source::scan_bounded_lengths(
            2,
            locals,
            &mut blocks,
            &mut items,
            |index| {
                assert_eq!(index, seen);
                seen += 1;
                1
            },
            &mut budget,
        );
        if expected {
            assert!(result.is_ok());
            assert_eq!((items, seen, budget.work()), (4096, 2, 3));
        } else {
            assert!(matches!(
                result,
                Err(CallError::Unavailable(
                    "finite source block/local/statement cap"
                ))
            ));
            assert_eq!((seen, budget.work()), (0, 1));
        }
    }
}

#[test]
fn bounded_headers_stop_immediately_on_first_4097_statement_total() {
    let mut work = Work::new(3);
    let mut budget = Budget::new(&mut work, 0);
    let (mut blocks, mut items) = (0, 1);
    let mut seen = 0;
    let result = source::scan_bounded_lengths(
        2,
        0,
        &mut blocks,
        &mut items,
        |index| {
            assert_eq!(index, 0);
            seen += 1;
            4096
        },
        &mut budget,
    );
    assert!(matches!(
        result,
        Err(CallError::Unavailable(
            "finite source block/local/statement cap"
        ))
    ));
    assert_eq!((seen, items, budget.work()), (1, 4097, 2));
}

#[test]
fn bounded_headers_overflow_refuses_before_any_later_header() {
    // Overflow in cumulative blocks, cumulative locals, or the first actual
    // statement length preserves checked arithmetic and never visits header 1.
    for case in 0..3 {
        let mut work = Work::new(3);
        let mut budget = Budget::new(&mut work, 0);
        let mut blocks = if case == 0 { usize::MAX } else { 0 };
        let mut items = if case == 1 { usize::MAX } else { 0 };
        let mut seen = 0;
        let result = source::scan_bounded_lengths(
            if case == 0 { 1 } else { 2 },
            1,
            &mut blocks,
            &mut items,
            |index| {
                assert_eq!(index, 0);
                seen += 1;
                usize::MAX
            },
            &mut budget,
        );
        assert!(matches!(
            result,
            Err(CallError::Resource(Resource::Arithmetic))
        ));
        assert_eq!(seen, usize::from(case == 2));
        assert_eq!(budget.work(), 1 + usize::from(case == 2));
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn bounded_headers_work_denial_precedes_header_access() {
    for cap in [0, 1] {
        let mut work = Work::new(cap);
        let mut budget = Budget::new(&mut work, 17);
        budget.reserve_storage(17).unwrap();
        let identity = budget.work_ledger_identity_v1();
        let (mut blocks, mut items) = (0, 0);
        let mut seen = 0;
        assert!(
            source::scan_bounded_lengths(
                1,
                0,
                &mut blocks,
                &mut items,
                |_| {
                    seen += 1;
                    0
                },
                &mut budget
            )
            .is_err()
        );
        assert_eq!(seen, 0);
        assert_eq!(budget.work(), cap);
        assert_eq!(budget.storage(), 17);
        assert!(budget.failed_work().is_some());
        assert!(budget.work_ledger_identity_v1() == identity);
    }
}

#[test]
fn bounded_header_refusal_refunds_only_existing_scoped_scratch() {
    let mut work = Work::new(INIT + 1 + 7);
    let mut budget = Budget::new(&mut work, SCRATCH + 17);
    budget.reserve_storage(17).unwrap();
    budget.charge_work(7).unwrap();
    let identity = budget.work_ledger_identity_v1();
    let mut seen = 0;
    let result = scoped(&mut budget, |budget| {
        let (mut blocks, mut items) = (0, 0);
        source::scan_bounded_lengths(
            33,
            0,
            &mut blocks,
            &mut items,
            |_| {
                seen += 1;
                0
            },
            budget,
        )
    });
    assert!(matches!(
        result,
        Err(CallError::Unavailable(
            "finite source block/local/statement cap"
        ))
    ));
    assert_eq!(seen, 0);
    assert_eq!(budget.storage(), 17);
    assert_eq!(budget.work(), INIT + 1 + 7);
    assert!(budget.work_ledger_identity_v1() == identity);
    assert!(budget.failed_work().is_none());
    assert!(budget.failed_storage().is_none());
}
