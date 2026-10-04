use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
    CanonicalKirBlockCoordinateV1 as Block, CanonicalKirFunctionCoordinateV1 as FunctionCoordinate,
    Constant, Function, MemoryAccess, ScalarType, Signature, Terminator, Type, ValueDef,
};
#[path = "canonical_kir_private_cell_pair_fixture_v1_tests.rs"]
mod fixture;
use fixture::*;
#[path = "canonical_kir_private_cell_pair_effects_v1_tests.rs"]
mod effect_tests;
#[path = "canonical_kir_private_cell_pair_resources_v1_tests.rs"]
mod resource_tests;

#[test]
fn all_fixed_widths_scalar_and_counted_pairs_remove_memory_keep_real_results() {
    for scalar in TYPES {
        for count in [None, Some(1), Some(32)] {
            let input = fixture(scalar, count);
            let (output, rows) = promoted(&input);
            let a = admit(&input);
            let b = admit(&output);
            assert_ne!(
                a.canonical().canonical_bytes(),
                b.canonical().canonical_bytes()
            );
            assert_eq!(
                output.functions[0].body.as_ref().unwrap().blocks[0]
                    .operations
                    .len(),
                3
            );
            assert!(
                matches!(rows[2].kind, OriginKind::LoadCopy { previous_store, stored_value: ValueId(1), .. } if previous_store == coord(5))
            );
            assert!(run(&a, &b, &[coord(1)], &rows, WORK, STORAGE).0.is_ok());
        }
    }
}

#[test]
fn sparse_cells_use_their_exact_prior_store_and_do_not_scale_with_count() {
    let mut expected = None;
    for count in [2, 1 << 16, 1 << 32, 2] {
        let mut input = fixture(ScalarType::U32, Some(count));
        ops(&mut input)[2] = constant(12, count - 1);
        let (output, rows) = candidate(
            &input,
            &[coord(1), coord(3), coord(4), coord(5)],
            &[(coord(6), coord(1), coord(4), ValueId(0))],
        );
        let (result, work, peak) = run(
            &admit(&input),
            &admit(&output),
            &[coord(1)],
            &rows,
            WORK,
            STORAGE,
        );
        assert!(result.is_ok());
        if let Some(prior) = expected {
            assert_eq!((work, peak), prior);
        }
        expected = Some((work, peak));
    }
}

#[test]
fn copy_chains_keep_loaded_value_definitions_instead_of_substituting_or_dce() {
    let mut input = fixture(ScalarType::U32, None);
    let access = MemoryAccess::new(AddressSpace::Private, 4);
    ops(&mut input).push(Operation::new(
        vec![],
        Kind::Store {
            pointer: ValueId(11),
            value: ValueId(14),
            access,
        },
    ));
    ops(&mut input).push(Operation::effect_free(
        ValueDef::new(ValueId(15), Type::Scalar(ScalarType::U32)),
        Kind::Load {
            pointer: ValueId(11),
            access,
        },
    ));
    input.functions[0].body.as_mut().unwrap().blocks[0].terminator = Some(Terminator::Return {
        values: vec![ValueId(15)],
    });
    let (output, rows) = candidate(
        &input,
        &[coord(1), coord(3), coord(4), coord(5), coord(7)],
        &[
            (coord(6), coord(1), coord(5), ValueId(1)),
            (coord(8), coord(1), coord(7), ValueId(14)),
        ],
    );
    accept(&input, &output, &[coord(1)], &rows);
    assert!(matches!(
        output.functions[0].body.as_ref().unwrap().blocks[0].operations[3].kind,
        Kind::Binary {
            lhs: ValueId(14),
            rhs: ValueId(14),
            ..
        }
    ));
}

#[test]
fn unused_selected_addresses_disappear_but_index_count_constants_remain() {
    let mut input = fixture(ScalarType::U32, Some(32));
    ops(&mut input).truncate(4);
    input.functions[0].body.as_mut().unwrap().blocks[0].terminator = Some(Terminator::Return {
        values: vec![ValueId(0)],
    });
    let (output, rows) = candidate(&input, &[coord(1), coord(3)], &[]);
    accept(&input, &output, &[coord(1)], &rows);
    assert_eq!(rows.len(), 2);
    let (wrong, wrong_rows) = candidate(&input, &[coord(0), coord(1), coord(2), coord(3)], &[]);
    refuse(
        &input,
        &wrong,
        &[coord(1)],
        &wrong_rows,
        "missing output operation",
    );
}

#[test]
fn empty_selection_and_independently_admitted_equal_bytes_keep_supplied_borrows() {
    let input = fixture(ScalarType::U32, None);
    let (output, rows) = candidate(&input, &[], &[]);
    let a = admit(&input);
    let b = admit(&output);
    assert!(!std::ptr::eq(&a, &b));
    assert_eq!(
        a.canonical().canonical_bytes(),
        b.canonical().canonical_bytes()
    );
    assert!(run(&a, &b, &[], &rows, WORK, STORAGE).0.is_ok());
    let (changed, changed_rows) = promoted(&input);
    refuse(
        &input,
        &changed,
        &[],
        &changed_rows,
        "retained operation payload",
    );
}

#[test]
fn function_qualified_selection_is_a_complete_ordered_subset_not_global_value_ids() {
    let mut input = fixture(ScalarType::U32, None);
    let mut second = input.functions[0].clone();
    second.id = "second".into();
    input.functions.push(second);
    let (output, rows) = promoted(&input);
    accept(&input, &output, &[coord(1)], &rows);
    let second_allocation = location(1, 0, 1);
    let (both, both_rows) = candidate(
        &input,
        &[
            coord(1),
            coord(3),
            coord(4),
            coord(5),
            second_allocation,
            location(1, 0, 3),
            location(1, 0, 4),
            location(1, 0, 5),
        ],
        &[
            (coord(6), coord(1), coord(5), ValueId(1)),
            (
                location(1, 0, 6),
                second_allocation,
                location(1, 0, 5),
                ValueId(1),
            ),
        ],
    );
    accept(&input, &both, &[coord(1), second_allocation], &both_rows);
    refuse(
        &input,
        &both,
        &[second_allocation, coord(1)],
        &both_rows,
        "ordered unique selected allocations",
    );
    refuse(
        &input,
        &output,
        &[second_allocation],
        &rows,
        "retained operation payload",
    );
}

#[test]
fn actual_backwards_layout_allocation_dominates_one_access_block() {
    let mut input = fixture(ScalarType::U32, None);
    let all = std::mem::take(ops(&mut input));
    let body = input.functions[0].body.as_mut().unwrap();
    let mut use_block = BasicBlock::new(BlockId(17));
    use_block.operations = all[4..].to_vec();
    use_block.terminator = Some(Terminator::Return {
        values: vec![ValueId(14)],
    });
    let mut producer = BasicBlock::new(BlockId(23));
    producer.operations = all[..4].to_vec();
    producer.terminator = Some(Terminator::Branch {
        target: BlockId(17),
        arguments: vec![],
    });
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(23),
        arguments: vec![],
    });
    body.blocks.push(use_block);
    body.blocks.push(producer);
    let allocation = location(0, 2, 1);
    let (output, rows) = candidate(
        &input,
        &[
            allocation,
            location(0, 2, 3),
            location(0, 1, 0),
            location(0, 1, 1),
        ],
        &[(location(0, 1, 2), allocation, location(0, 1, 1), ValueId(1))],
    );
    accept(&input, &output, &[allocation], &rows);
}

#[test]
fn each_origin_axis_and_tag_is_checked_with_complete_roster() {
    let input = fixture(ScalarType::U32, None);
    let (output, rows) = promoted(&input);
    for mode in 0..8 {
        let mut bad = rows.clone();
        match mode {
            0 => bad[2].input = coord(5),
            1 => bad[2].output = coord(1),
            2 => bad[2].kind = OriginKind::Retained,
            3 => bad[0].kind = rows[2].kind,
            4 => {
                bad[2].kind = OriginKind::LoadCopy {
                    allocation: coord(3),
                    previous_store: coord(5),
                    stored_value: ValueId(1),
                }
            }
            5 => {
                bad[2].kind = OriginKind::LoadCopy {
                    allocation: coord(1),
                    previous_store: coord(4),
                    stored_value: ValueId(1),
                }
            }
            6 => {
                bad[2].kind = OriginKind::LoadCopy {
                    allocation: coord(1),
                    previous_store: coord(5),
                    stored_value: ValueId(0),
                }
            }
            _ => bad.swap(0, 1),
        }
        refuse(
            &input,
            &output,
            &[coord(1)],
            &bad,
            "exact complete output origin",
        );
    }
    let mut extra = rows.clone();
    extra.push(rows[0]);
    for bad in [&rows[..2], extra.as_slice()] {
        refuse(
            &input,
            &output,
            &[coord(1)],
            bad,
            "bounded complete claim rosters",
        );
    }
    refuse(
        &input,
        &output,
        &[coord(1), coord(1)],
        &rows,
        "ordered unique selected allocations",
    );
    refuse(
        &input,
        &output,
        &[coord(0)],
        &rows,
        "eligible selected allocation",
    );
    refuse(
        &input,
        &output,
        &[location(7, 0, 1)],
        &rows,
        "actual operation coordinate",
    );
}

#[test]
fn actual_recipe_rejects_alternative_identity_and_wrong_stored_value() {
    let input = fixture(ScalarType::U32, None);
    let (output, rows) = promoted(&input);
    for (op, lhs, rhs) in [
        (BinaryOp::BitAnd, ValueId(1), ValueId(1)),
        (BinaryOp::BitOr, ValueId(0), ValueId(0)),
        (BinaryOp::BitOr, ValueId(1), ValueId(0)),
    ] {
        let mut bad = output.clone();
        ops(&mut bad)[2].kind = Kind::Binary { op, lhs, rhs };
        refuse(
            &input,
            &bad,
            &[coord(1)],
            &rows,
            "exact retained Load copy recipe",
        );
    }
    let mut stale = input.clone();
    let Kind::Store { value, .. } = &mut ops(&mut stale)[5].kind else {
        unreachable!()
    };
    *value = ValueId(0);
    refuse(
        &stale,
        &output,
        &[coord(1)],
        &rows,
        "exact retained Load copy recipe",
    );
    assert!(
        run(
            &admit(&output),
            &admit(&input),
            &[coord(1)],
            &rows,
            WORK,
            STORAGE
        )
        .0
        .is_err()
    );
}

#[test]
fn copy_result_ids_and_types_are_exact_even_when_unused() {
    let mut input = fixture(ScalarType::U32, None);
    input.functions[0].body.as_mut().unwrap().blocks[0].terminator = Some(Terminator::Return {
        values: vec![ValueId(0)],
    });
    let (output, rows) = promoted(&input);
    accept(&input, &output, &[coord(1)], &rows);
    let mut wrong = output.clone();
    ops(&mut wrong)[2].results[0].id = ValueId(99);
    refuse(
        &input,
        &wrong,
        &[coord(1)],
        &rows,
        "exact retained Load copy recipe",
    );
    // Invalid typed shapes cannot construct an admitted owner. Exercise the
    // same private recipe comparator with inert component values instead.
    let original = &input.functions[0].body.as_ref().unwrap().blocks[0].operations[6];
    for mode in 0..3 {
        let mut wrong = output.functions[0].body.as_ref().unwrap().blocks[0].operations[2].clone();
        match mode {
            0 => wrong.results[0].ty = Type::Scalar(ScalarType::I32),
            1 => wrong.results.clear(),
            _ => wrong.results.push(ValueDef::new(ValueId(98), Type::BOOL)),
        }
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        assert_eq!(
            scoped(&mut budget, |meter| check::copy(
                original,
                &wrong,
                ValueId(13),
                ValueId(14),
                ValueId(1),
                meter
            )),
            Err(Error::Mismatch("exact retained Load copy recipe"))
        );
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn incomplete_removal_and_extra_output_cannot_hide_behind_copy_rows() {
    let input = fixture(ScalarType::U32, None);
    for removed in [
        vec![coord(3), coord(4), coord(5)],
        vec![coord(4), coord(5)],
        vec![coord(3), coord(4)],
    ] {
        let (output, rows) = candidate(
            &input,
            &removed,
            &[(coord(6), coord(1), coord(5), ValueId(1))],
        );
        assert!(
            run(
                &admit(&input),
                &admit(&output),
                &[coord(1)],
                &rows,
                WORK,
                STORAGE
            )
            .0
            .is_err()
        );
    }
    let (mut output, mut rows) = promoted(&input);
    ops(&mut output).push(constant(99, 0));
    rows.push(Origin {
        input: coord(0),
        output: coord(3),
        kind: OriginKind::Retained,
    });
    refuse(
        &input,
        &output,
        &[coord(1)],
        &rows,
        "extra output operation",
    );
}

#[test]
fn source_ids_headers_and_cfg_stay_exact_even_with_valid_owners() {
    let input = fixture(ScalarType::U32, None);
    let (output, rows) = promoted(&input);
    let mut bad = output.clone();
    bad.id = "foreign".into();
    refuse(&input, &bad, &[coord(1)], &rows, "module payload");
    let mut bad = output.clone();
    bad.functions[0].id = "renamed".into();
    refuse(&input, &bad, &[coord(1)], &rows, "function payload");
    let mut bad = output.clone();
    bad.functions[0].body.as_mut().unwrap().blocks[0].terminator = Some(Terminator::Return {
        values: vec![ValueId(0)],
    });
    refuse(&input, &bad, &[coord(1)], &rows, "block payload");
    let mut bad = output.clone();
    bad.functions[0].body.as_mut().unwrap().blocks[0].id = BlockId(800);
    refuse(&input, &bad, &[coord(1)], &rows, "block payload");
    let mut bad = output;
    ops(&mut bad)[0] = constant(10, 7);
    refuse(
        &input,
        &bad,
        &[coord(1)],
        &rows,
        "retained operation payload",
    );
}

#[test]
fn pair_invocation_rebuilds_census_and_refuses_unsupported_selection_not_noop() {
    let mut variants = Vec::new();
    let mut m = fixture(ScalarType::U32, Some(2));
    ops(&mut m)[2] = constant(12, 2);
    variants.push(m);
    let mut m = fixture(ScalarType::U32, Some(2));
    if let Kind::GetElementPointer { offset, .. } = &mut ops(&mut m)[3].kind {
        *offset = ValueId(2);
    }
    variants.push(m);
    let mut m = fixture(ScalarType::U32, Some(2));
    if let Kind::Alloca { count, .. } = &mut ops(&mut m)[1].kind {
        *count = Some(ValueId(2));
    }
    variants.push(m);
    variants.push(fixture(ScalarType::U32, Some(0)));
    for ty in [
        ScalarType::Bool,
        ScalarType::Index,
        ScalarType::F32,
        ScalarType::F64,
    ] {
        variants.push(fixture(ty, None));
    }
    let mut m = fixture(ScalarType::U32, None);
    if let Kind::Load { access, .. } = &mut ops(&mut m)[6].kind {
        access.volatile = true;
    }
    variants.push(m);
    let mut m = fixture(ScalarType::U32, None);
    if let Kind::Load { access, .. } = &mut ops(&mut m)[6].kind {
        access.alignment = 1;
    }
    variants.push(m);
    for input in variants {
        let (output, rows) = candidate(&input, &[], &[]);
        accept(&input, &output, &[], &rows);
        refuse(
            &input,
            &output,
            &[coord(1)],
            &rows,
            "eligible selected allocation",
        );
    }
    let mut input = fixture(ScalarType::U32, None);
    ops(&mut input).drain(4..6);
    let (output, rows) = candidate(&input, &[], &[]);
    refuse(
        &input,
        &output,
        &[coord(1)],
        &rows,
        "eligible selected allocation",
    );
}

#[test]
fn exact_and_one_short_limits_cover_mutation_and_noop_without_ledger_reset() {
    let input = fixture(ScalarType::U32, None);
    for mutation in [false, true] {
        let (output, rows) = if mutation {
            promoted(&input)
        } else {
            candidate(&input, &[], &[])
        };
        let selected = if mutation { vec![coord(1)] } else { vec![] };
        let a = admit(&input);
        let b = admit(&output);
        let (result, work, peak) = run(&a, &b, &selected, &rows, WORK, STORAGE);
        assert!(result.is_ok());
        assert!(run(&a, &b, &selected, &rows, work, peak).0.is_ok());
        for result in [
            run(&a, &b, &selected, &rows, work - 1, peak).0,
            run(&a, &b, &selected, &rows, work, peak - 1).0,
        ] {
            assert!(matches!(
                result,
                Err(Error::Resource(_))
                    | Err(Error::Inventory(InventoryError::Resource(_)))
                    | Err(Error::Census(CensusError::Resource(_)))
                    | Err(Error::Census(CensusError::ControlFlow(_)))
            ));
        }
        let (_, again, again_peak) = run(&a, &b, &selected, &rows, WORK, STORAGE);
        assert_eq!((work, peak), (again, again_peak));
    }
}

#[test]
fn retained_witness_receipt_and_existing_work_survive_another_real_check() {
    let input = fixture(ScalarType::U32, None);
    let (output, rows) = promoted(&input);
    let a = admit(&input);
    let b = admit(&output);
    let selected = [coord(1)];
    let (_, required, _) = run(&a, &b, &selected, &rows, WORK, STORAGE);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    budget.charge_work(17).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let retained;
    {
        let (first, receipt) = check_canonical_kir_private_cell_promotion_v1(
            &a,
            &b,
            &selected,
            &rows,
            Limits::default(),
            &mut budget,
        )
        .unwrap();
        retained = receipt.retained_storage();
        budget.reserve_storage(retained).unwrap();
        {
            let (second, other) = check_canonical_kir_private_cell_promotion_v1(
                &a,
                &b,
                &selected,
                &rows,
                Limits::default(),
                &mut budget,
            )
            .unwrap();
            assert!(std::ptr::eq(first.input(), second.input()));
            assert_eq!(other.retained_storage(), retained);
        }
        assert_eq!(budget.storage(), FLOOR + retained);
        assert_eq!(first.origins(), rows.as_slice());
    }
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(budget.work(), 17 + 2 * required);
    assert!(budget.work_ledger_identity_v1() == ledger);
}
