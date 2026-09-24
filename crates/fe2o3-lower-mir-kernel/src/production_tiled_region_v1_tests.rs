//! Pure implementation controls only. These cannot create a public borrowed
//! view and are NOT a substitute for the separate genuine Rust-source gate.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::cell::Cell;

const ROOT: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(0);
fn operand(role: SemanticMfmaOperandRoleV1) -> SemanticValueBindingV1 {
    SemanticValueBindingV1::MatrixFragment {
        values: (0..4)
            .map(|i| (ValueId(i + 1), Type::Scalar(ScalarType::Bf16)))
            .collect(),
        contract: SemanticMfmaOperandContractV1 {
            role,
            profile: SemanticMfmaProfileV1::Bf16F32M16N16K16,
            register_distribution: SemanticMfmaRegisterDistributionV1::Tile16x16,
            wave_width: 64,
        },
        storage_layout: SemanticMfmaStorageLayoutV1::RowMajor,
        wave: SemanticCurrentWaveV1::new(64),
    }
}
#[test]
fn paid_capture_handle_does_not_inline_the_frame_into_legacy_none_paths() {
    assert!(size_of::<Option<Capture>>() <= 3 * size_of::<usize>());
    assert!(size_of::<Capture>() < size_of::<Recorder>());
}
#[test]
fn capture_allocation_zero_and_one_below_refuse_before_initializer() {
    let needed = 2 * size_of::<Recorder>() + size_of::<Capture>() + size_of::<Vec<Recorder>>();
    for storage in [0, needed - 1] {
        let mut work = Work::new(1000);
        let mut budget = Budget::new(&mut work, storage);
        let entered = Cell::new(false);
        assert!(
            Capture::allocate(&mut budget, |_| {
                entered.set(true);
                Ok(Recorder::blank(ROOT))
            })
            .is_err()
        );
        assert!(!entered.get());
        assert_eq!(budget.storage(), 0);
    }
}
#[test]
fn capture_exact_allocation_accounts_constructor_coexistence_and_live_payload() {
    let needed = 2 * size_of::<Recorder>() + size_of::<Capture>() + size_of::<Vec<Recorder>>();
    let mut work = Work::new(1000);
    let mut budget = Budget::new(&mut work, needed + 17);
    budget.reserve_storage(17).unwrap();
    let capture = Capture::allocate(&mut budget, |_| Ok(Recorder::blank(ROOT))).unwrap();
    assert_eq!(budget.peak_storage(), needed + 17);
    assert_eq!(
        budget.storage(),
        17 + size_of::<Recorder>() + size_of::<Capture>()
    );
    drop(capture);
    budget
        .release_storage(size_of::<Recorder>() + size_of::<Capture>())
        .unwrap();
    assert_eq!(budget.storage(), 17);
}
#[test]
fn failed_initializer_has_no_unaccounted_payload_and_scope_owns_refund() {
    let mut work = Work::new(1000);
    let mut budget = Budget::new(&mut work, 1 << 20);
    budget.reserve_storage(17).unwrap();
    assert!(
        Capture::allocate(&mut budget, |_| unavailable("injected initializer refusal")).is_err()
    );
    assert_eq!(
        budget.storage(),
        17 + 2 * size_of::<Recorder>() + size_of::<Capture>() + size_of::<Vec<Recorder>>()
    );
    budget.release_storage(budget.storage() - 17).unwrap();
    assert_eq!(budget.storage(), 17);
}
#[test]
fn equal_components_do_not_erase_operand_roles() {
    assert!(emission::components(&operand(SemanticMfmaOperandRoleV1::A), Role::Lhs).is_ok());
    assert!(emission::components(&operand(SemanticMfmaOperandRoleV1::A), Role::Rhs).is_err());
    assert!(emission::components(&operand(SemanticMfmaOperandRoleV1::B), Role::Lhs).is_err());
}
#[test]
fn wrong_layout_width_profile_and_duplicate_components_refuse() {
    for which in 0..5 {
        let mut binding = operand(SemanticMfmaOperandRoleV1::A);
        let SemanticValueBindingV1::MatrixFragment {
            values,
            contract,
            storage_layout,
            wave,
        } = &mut binding
        else {
            unreachable!()
        };
        match which {
            0 => *storage_layout = SemanticMfmaStorageLayoutV1::ColumnMajor,
            1 => wave.width = 32,
            2 => contract.wave_width = 32,
            3 => contract.profile = SemanticMfmaProfileV1::Fp8E4M3F32M16N16K128,
            _ => values[1].0 = values[0].0,
        }
        assert!(emission::components(&binding, Role::Lhs).is_err());
    }
}
#[test]
fn unrelated_scalar_array_cannot_be_a_nominal_producer() {
    let binding = SemanticValueBindingV1::MatrixContext;
    for role in [Role::Lane, Role::Lhs, Role::Rhs, Role::Zero, Role::Result] {
        assert!(emission::components(&binding, role).is_err());
    }
}
#[test]
fn source_profile_rejects_repeated_or_non_wave64_lane_kind() {
    let ty = SemanticTypeIdV1::from_index(0);
    assert_eq!(
        source::role(&SemanticCompilerIntrinsicOperationV1::WaveLaneCurrent {
            lane: ty,
            wave_width: 64
        })
        .unwrap(),
        Some(Role::Lane)
    );
    assert!(
        source::role(&SemanticCompilerIntrinsicOperationV1::WaveLaneCurrent {
            lane: ty,
            wave_width: 32
        })
        .is_err()
    );
    let mut slot = None;
    source::unique(&mut slot, Role::Lane, "duplicate").unwrap();
    assert!(source::unique(&mut slot, Role::Lane, "duplicate").is_err());
}
#[test]
fn recorded_uses_preserve_repetition_and_have_a_finite_cap() {
    let mut recorder = Recorder::blank(ROOT);
    let results = [ValueId(1), ValueId(2), ValueId(3), ValueId(4)];
    for ordinal in 0..USES {
        recorder
            .record_use(results, BlockId(0), Some(ordinal as u32), 0, ValueId(1))
            .unwrap();
    }
    assert_eq!(recorder.use_count, USES);
    assert!(
        recorder
            .record_use(results, BlockId(0), None, 0, ValueId(1))
            .is_err()
    );
    assert_eq!(recorder.use_count, USES);
}
#[test]
fn graph_scan_cap_refuses_without_rewriting_or_truncating_graph() {
    let mut work = Work::new(1000);
    let mut budget = Budget::new(&mut work, 100);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = (0..=OPERATIONS)
        .map(|i| {
            Operation::effect_free(
                ValueDef::new(ValueId(i as u32), Type::Scalar(ScalarType::U32)),
                OperationKind::Constant(Constant::U32(0)),
            )
        })
        .collect();
    let before = block.operations.len();
    assert!(emission::bounded_blocks(std::slice::from_ref(&block), &mut budget).is_err());
    assert_eq!(block.operations.len(), before);
}
#[test]
fn callback_retains_extra_storage_and_cumulative_work() {
    let mut work = Work::new(20);
    let mut budget = Budget::new(&mut work, 100);
    budget.reserve_storage(17).unwrap();
    for _ in 0..2 {
        let out = query::callback(&mut budget, |b| {
            b.charge_work(3)?;
            b.reserve_storage(2)?;
            Ok(9)
        });
        assert!(out.valid);
        assert_eq!(out.result.unwrap(), 9);
    }
    assert_eq!(budget.storage(), 21);
    assert_eq!(budget.work(), 6);
}
#[test]
fn callback_ignored_failed_work_cannot_turn_into_success() {
    let mut work = Work::new(0);
    let mut budget = Budget::new(&mut work, 100);
    let out = query::callback(&mut budget, |b| {
        let _ = b.charge_work(1);
        Ok(9)
    });
    assert!(out.valid);
    assert!(out.result.is_err());
    assert_eq!(budget.work(), 0);
}
#[test]
fn callback_ignored_failed_storage_cannot_turn_into_success() {
    let mut work = Work::new(20);
    let mut budget = Budget::new(&mut work, 17);
    budget.reserve_storage(17).unwrap();
    let out = query::callback(&mut budget, |b| {
        let _ = b.reserve_storage(1);
        Ok(9)
    });
    assert!(out.valid);
    assert!(out.result.is_err());
    assert_eq!(budget.storage(), 17);
}
#[test]
fn callback_undercut_floor_is_not_refunded_or_repaired() {
    let mut work = Work::new(20);
    let mut budget = Budget::new(&mut work, 100);
    budget.reserve_storage(17).unwrap();
    let out = query::callback(&mut budget, |b| {
        b.release_storage(1)?;
        Ok(9)
    });
    assert!(!out.valid);
    assert_eq!(budget.storage(), 16);
}
#[test]
fn callback_replaced_work_ledger_is_not_mutated_back() {
    let mut a = Work::new(20);
    let mut b = Work::new(20);
    let mut budget = Budget::new(&mut a, 100);
    budget.reserve_storage(17).unwrap();
    let foreign = Budget::new(&mut b, 100);
    let out = query::callback(&mut budget, move |budget| {
        *budget = foreign;
        Ok(9)
    });
    assert!(!out.valid);
    assert_eq!(budget.storage(), 0);
}
#[test]
fn callback_panic_is_named_and_keeps_accepted_extra_storage() {
    let mut work = Work::new(20);
    let mut budget = Budget::new(&mut work, 100);
    budget.reserve_storage(17).unwrap();
    let out: query::Outcome<()> = query::callback(&mut budget, |b| {
        b.reserve_storage(3)?;
        panic!("bounded callback control")
    });
    assert!(out.valid);
    assert!(matches!(out.result, Err(Error::CallbackPanicked)));
    assert!(out.panic.is_some());
    assert_eq!(budget.storage(), 20);
}
#[test]
fn callback_error_does_not_reset_work_or_discard_extra_storage() {
    let mut work = Work::new(20);
    let mut budget = Budget::new(&mut work, 100);
    budget.reserve_storage(17).unwrap();
    let out: query::Outcome<()> = query::callback(&mut budget, |b| {
        b.charge_work(3)?;
        b.reserve_storage(2)?;
        unavailable("callback refusal")
    });
    assert!(out.valid);
    assert!(out.result.is_err());
    assert_eq!(budget.work(), 3);
    assert_eq!(budget.storage(), 19);
}

#[test]
fn capture_fixed_initialization_work_zero_and_one_below_never_enter_initializer() {
    for limit in [0, ALIASES + USES + 11] {
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, 1 << 20);
        let entered = Cell::new(false);
        assert!(
            Capture::allocate(&mut budget, |_| {
                entered.set(true);
                Ok(Recorder::blank(ROOT))
            })
            .is_err()
        );
        assert!(!entered.get());
        assert_eq!(budget.work(), 0);
        assert!(budget.failed_work().is_some());
        let owned = budget.storage();
        budget.release_storage(owned).unwrap();
    }
}
#[test]
fn capture_fixed_initialization_exact_work_keeps_original_prefix() {
    let mut work = Work::new(7 + ALIASES + USES + 12);
    let mut budget = Budget::new(&mut work, 1 << 20);
    budget.charge_work(7).unwrap();
    let capture = Capture::allocate(&mut budget, |_| Ok(Recorder::blank(ROOT))).unwrap();
    assert_eq!(budget.work(), 7 + ALIASES + USES + 12);
    assert_eq!(budget.failed_work(), None);
    drop(capture);
    let owned = budget.storage();
    budget.release_storage(owned).unwrap();
}
#[test]
fn borrowed_failed_work_observation_is_sticky_and_never_resets_prefix() {
    let mut work = Work::new(7);
    let mut budget = Budget::new(&mut work, 100);
    budget.charge_work(7).unwrap();
    assert!(budget.charge_work(1).is_err());
    assert_eq!(budget.failed_work(), Some(8));
    budget.charge_work(0).unwrap();
    assert_eq!(budget.work(), 7);
    assert_eq!(budget.failed_work(), Some(8));
}

#[test]
fn source_cycle_root_32_uses_bit31_and_retains_original_floor_and_work() {
    let edges = [(0usize, 31usize), (30, 31)];
    let needed_work = 32 + 32 * 32 + edges.len() + 32;
    let mut work = Work::new(9 + needed_work);
    let mut budget = Budget::new(&mut work, 17 + 128);
    let identity = budget.work_ledger_identity_v1();
    budget.charge_work(9).unwrap();
    budget.reserve_storage(17).unwrap();
    let visits = Cell::new(0);
    source::reject_source_cycle_edges(
        32,
        edges.len(),
        |index| {
            visits.set(visits.get() + 1);
            edges.get(index).copied()
        },
        &mut budget,
    )
    .unwrap();
    assert_eq!(SOURCE_BLOCKS, 32);
    assert_eq!(visits.get(), edges.len());
    assert_eq!(budget.storage(), 17);
    assert_eq!(budget.peak_storage(), 145);
    assert_eq!(budget.work(), 9 + needed_work);
    assert!(budget.work_ledger_identity_v1() == identity);
    assert_eq!((ALIASES, USES, OPERATIONS), (64, 128, 1024));
}

#[test]
fn source_cycle_root_33_refuses_before_scan_or_scratch() {
    let mut work = Work::new(10_000);
    let mut budget = Budget::new(&mut work, 145);
    budget.reserve_storage(17).unwrap();
    let visits = Cell::new(0);
    assert!(matches!(
        source::reject_source_cycle_edges(
            33,
            1,
            |_| {
                visits.set(visits.get() + 1);
                Some((0, 0))
            },
            &mut budget
        ),
        Err(Error::Unavailable("source block cap"))
    ));
    assert_eq!(visits.get(), 0);
    assert_eq!(budget.storage(), 17);
    assert_eq!(budget.peak_storage(), 17);
    assert_eq!(budget.work(), 0);
}

#[test]
fn source_cycle_highest_bit_self_and_cross_cycles_refuse_after_scratch_drop() {
    for edges in [vec![(31usize, 31usize)], vec![(0, 31), (31, 0)]] {
        let needed_work = 32 + 32 * 32 + edges.len() + 32;
        let mut work = Work::new(needed_work);
        let mut budget = Budget::new(&mut work, 145);
        budget.reserve_storage(17).unwrap();
        assert!(matches!(
            source::reject_source_cycle_edges(
                32,
                edges.len(),
                |index| edges.get(index).copied(),
                &mut budget
            ),
            Err(Error::Unavailable(
                "looping source unavailable for first inspection profile"
            ))
        ));
        assert_eq!(budget.storage(), 17);
        assert_eq!(budget.work(), needed_work);
    }
}

#[test]
fn source_cycle_missing_and_out_of_range_rows_refuse_without_losing_floor() {
    for row in [None, Some((32usize, 0usize)), Some((0, 32))] {
        let needed_work = 32 + 32 * 32 + 1 + 32;
        let mut work = Work::new(needed_work);
        let mut budget = Budget::new(&mut work, 145);
        budget.reserve_storage(17).unwrap();
        let result = source::reject_source_cycle_edges(32, 1, |_| row, &mut budget);
        if row.is_none() {
            assert!(matches!(
                result,
                Err(Error::Unavailable("missing source successor row"))
            ));
        } else {
            assert!(matches!(
                result,
                Err(Error::Unavailable("source successor outside fixed roster"))
            ));
        }
        assert_eq!(budget.storage(), 17);
        assert_eq!(budget.work(), needed_work);
    }
}

#[test]
fn source_cycle_work_denial_prevents_all_edge_access_and_scratch_initialization() {
    let needed_work = 32 + 32 * 32 + 1 + 32;
    for limit in [0, needed_work - 1] {
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, 145);
        budget.reserve_storage(17).unwrap();
        let visits = Cell::new(0);
        assert!(matches!(
            source::reject_source_cycle_edges(
                32,
                1,
                |_| {
                    visits.set(visits.get() + 1);
                    Some((0, 31))
                },
                &mut budget
            ),
            Err(Error::Resource(_))
        ));
        assert_eq!(visits.get(), 0);
        assert_eq!(budget.storage(), 17);
        assert_eq!(budget.peak_storage(), 17);
        assert_eq!(budget.work(), 0);
        assert!(budget.failed_work().is_some());
    }
}

#[test]
fn source_cycle_scratch_zero_and_one_short_keep_paid_work_and_no_edge_access() {
    let needed_work = 32 + 32 * 32 + 1 + 32;
    for extra in [0, 127] {
        let mut work = Work::new(needed_work);
        let mut budget = Budget::new(&mut work, 17 + extra);
        budget.reserve_storage(17).unwrap();
        let visits = Cell::new(0);
        assert!(matches!(
            source::reject_source_cycle_edges(
                32,
                1,
                |_| {
                    visits.set(visits.get() + 1);
                    Some((0, 31))
                },
                &mut budget
            ),
            Err(Error::Resource(_))
        ));
        assert_eq!(visits.get(), 0);
        assert_eq!(budget.storage(), 17);
        assert_eq!(budget.peak_storage(), 17);
        assert_eq!(budget.work(), needed_work);
        assert!(budget.failed_storage().is_some());
    }
}

#[test]
fn source_cycle_work_arithmetic_overflow_refuses_before_observation() {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, 128);
    let visits = Cell::new(0);
    assert!(matches!(
        source::reject_source_cycle_edges(
            32,
            usize::MAX,
            |_| {
                visits.set(visits.get() + 1);
                None
            },
            &mut budget
        ),
        Err(Error::Resource(Resource::Arithmetic))
    ));
    assert_eq!(visits.get(), 0);
    assert_eq!(budget.work(), 0);
    assert_eq!(budget.storage(), 0);
}

// Predicate-level controls only: these do not manufacture an occurrence
// owner, frontend seed, emitter binding or public inspection view.
fn alias_test_place(projected: bool) -> SemanticPlaceV1 {
    use fe2o3_mir_model::semantic_mir_v1::{SemanticLocalIdV1, SemanticProjectionV1};
    let ty = SemanticTypeIdV1::from_index(0);
    let projections = if projected {
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), ty).unwrap()]
    } else {
        Vec::new()
    };
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), projections, ty).unwrap()
}
fn alias_test_use(index: u32) -> SsaResolvedEventV1 {
    SsaResolvedEventV1::Use {
        variable: fe2o3_mir_model::SsaVariableIdV1::new(1),
        value: SsaValueV1::Definition(fe2o3_mir_model::SsaDefinitionIdV1::new(index)),
    }
}
#[test]
fn nominal_alias_keeps_whole_copy_move_and_shared_borrow_distinct() {
    for operand in [
        SemanticOperandV1::Copy(alias_test_place(false)),
        SemanticOperandV1::Move(alias_test_place(false)),
    ] {
        assert_eq!(
            source::nominal_alias_operand(&SemanticRvalueKindV1::Use(operand)).unwrap(),
            OperandRole::RvalueOperand(0)
        );
    }
    let borrow = SemanticRvalueKindV1::Borrow {
        kind: SemanticBorrowKindV1::Shared,
        place: alias_test_place(false),
    };
    assert_eq!(
        source::nominal_alias_operand(&borrow).unwrap(),
        OperandRole::RvaluePlace
    );
}
#[test]
fn nominal_alias_rejects_projected_mutable_fake_and_address_formation() {
    for kind in [SemanticBorrowKindV1::Mutable, SemanticBorrowKindV1::Fake] {
        assert!(
            source::nominal_alias_operand(&SemanticRvalueKindV1::Borrow {
                kind,
                place: alias_test_place(false),
            })
            .is_err()
        );
    }
    for value in [
        SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Shared,
            place: alias_test_place(true),
        },
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(alias_test_place(true))),
        SemanticRvalueKindV1::Use(SemanticOperandV1::Move(alias_test_place(true))),
        SemanticRvalueKindV1::AddressOf {
            mutability: SemanticMutabilityV1::Immutable,
            place: alias_test_place(false),
        },
    ] {
        assert!(source::nominal_alias_operand(&value).is_err());
    }
}
#[test]
fn actual_grid_leader_elision_is_not_a_nominal_base_use() {
    source::require_non_elided_borrow(false).unwrap();
    assert!(matches!(
        source::require_non_elided_borrow(true),
        Err(Error::Unavailable(
            "elided borrow cannot establish nominal source SSA transport"
        ))
    ));
}
#[test]
fn shared_borrow_requires_the_actual_lane_or_matrix_context_role() {
    for role in Role::ALL {
        assert_eq!(
            source::require_nominal_borrow_source(role).is_ok(),
            matches!(role, Role::Lane | Role::Context)
        );
    }
}
#[test]
fn promoted_base_use_preserves_exact_ssa_and_original_ledger_floor() {
    let mut work = Work::new(10);
    let mut budget = Budget::new(&mut work, 17);
    budget.charge_work(9).unwrap();
    budget.reserve_storage(17).unwrap();
    let identity = budget.work_ledger_identity_v1();
    let mut selected = None;
    source::record_base_use(
        &mut selected,
        Some(alias_test_use(5)),
        true,
        true,
        &mut budget,
    )
    .unwrap();
    assert_eq!(
        source::require_base_use(selected).unwrap(),
        SsaValueV1::Definition(fe2o3_mir_model::SsaDefinitionIdV1::new(5))
    );
    assert_eq!(
        (budget.storage(), budget.peak_storage(), budget.work()),
        (17, 17, 10)
    );
    assert!(budget.work_ledger_identity_v1() == identity);
    assert_eq!(budget.failed_work(), None);
}
#[test]
fn missing_unresolved_unreachable_or_retained_base_use_cannot_be_synthesized() {
    assert!(source::require_base_use(None).is_err());
    let value = SsaValueV1::Definition(fe2o3_mir_model::SsaDefinitionIdV1::new(5));
    for (resolved, reachable, promoted) in [
        (None, true, true),
        (
            Some(SsaResolvedEventV1::Define {
                variable: fe2o3_mir_model::SsaVariableIdV1::new(1),
                value,
            }),
            true,
            true,
        ),
        (Some(alias_test_use(5)), false, true),
        (Some(alias_test_use(5)), true, false),
    ] {
        let mut work = Work::new(10);
        let mut budget = Budget::new(&mut work, 17);
        budget.charge_work(9).unwrap();
        budget.reserve_storage(17).unwrap();
        let mut selected = None;
        assert!(
            source::record_base_use(&mut selected, resolved, reachable, promoted, &mut budget)
                .is_err()
        );
        assert_eq!(selected, None);
        assert_eq!((budget.storage(), budget.work()), (17, 10));
    }
}
#[test]
fn duplicate_same_or_different_actual_base_uses_refuse_after_consuming_work() {
    for second in [5, 6] {
        let mut work = Work::new(11);
        let mut budget = Budget::new(&mut work, 17);
        budget.charge_work(9).unwrap();
        budget.reserve_storage(17).unwrap();
        let identity = budget.work_ledger_identity_v1();
        let mut selected = None;
        source::record_base_use(
            &mut selected,
            Some(alias_test_use(5)),
            true,
            true,
            &mut budget,
        )
        .unwrap();
        assert!(
            source::record_base_use(
                &mut selected,
                Some(alias_test_use(second)),
                true,
                true,
                &mut budget
            )
            .is_err()
        );
        assert_eq!(
            (budget.storage(), budget.peak_storage(), budget.work()),
            (17, 17, 11)
        );
        assert!(budget.work_ledger_identity_v1() == identity);
    }
}
#[test]
fn denied_base_use_work_never_changes_the_selected_value_or_storage_floor() {
    for prefix in [0, 9] {
        let mut work = Work::new(prefix);
        let mut budget = Budget::new(&mut work, 17);
        budget.charge_work(prefix).unwrap();
        budget.reserve_storage(17).unwrap();
        let identity = budget.work_ledger_identity_v1();
        let mut selected = None;
        assert!(
            source::record_base_use(
                &mut selected,
                Some(alias_test_use(5)),
                true,
                true,
                &mut budget
            )
            .is_err()
        );
        assert_eq!(selected, None);
        assert_eq!(
            (budget.storage(), budget.peak_storage(), budget.work()),
            (17, 17, prefix)
        );
        assert_eq!(budget.failed_work(), Some(prefix + 1));
        assert!(budget.work_ledger_identity_v1() == identity);
    }
}
