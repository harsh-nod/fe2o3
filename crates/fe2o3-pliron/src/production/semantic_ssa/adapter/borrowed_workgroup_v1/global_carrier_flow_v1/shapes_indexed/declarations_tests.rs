use super::*;

fn declarations(indices: &[u32]) -> Vec<SemanticLocalDeclV1> {
    indices
        .iter()
        .enumerate()
        .map(|(index, &t)| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([index as u8; 32]),
                ty(t),
                match index {
                    0 => SemanticLocalRoleV1::Return,
                    1 => SemanticLocalRoleV1::Argument(0),
                    _ => SemanticLocalRoleV1::Temporary,
                },
                SemanticSourceProvenanceV1::unavailable(),
            )
        })
        .collect()
}

fn poisoned_retries(shapes: &mut Shapes<'_>, locals: &[SemanticLocalDeclV1]) {
    assert!(shapes.memo.failed);
    let words = shapes.memo.words.clone();
    let mut scalar = budget(MAX_FLOW_WORK);
    assert!(matches!(
        shapes.contains(ty(7), &mut scalar),
        Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
    ));
    assert_eq!(scalar.remaining, MAX_FLOW_WORK - 1);
    for retry in [locals, &[]] {
        let mut work = budget(MAX_FLOW_WORK);
        let mut called = false;
        assert!(matches!(
            shapes.for_each_declaration(retry, &mut work, |_, _, _, _| {
                called = true;
                Ok(())
            }),
            Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
        ));
        assert!(!called);
        assert_eq!(work.remaining, MAX_FLOW_WORK - 2);
    }
    assert_eq!(shapes.memo.words, words);
}

#[test]
fn batch_full_ordered_census_matches_scalar_cold_and_recursive_oracles() {
    let types = graph();
    let facts = facts(&types);
    let locals = declarations(&[32, 7, 2, 32, 8, 33, 39, 22, 3]);
    let mut scalar_work = budget(MAX_FLOW_WORK);
    let mut scalar = Shapes::new(&types, &facts, &mut scalar_work).unwrap();
    let mut cold_work = budget(MAX_FLOW_WORK);
    let mut cold = ColdShapes::new(&types, &facts, &mut cold_work).unwrap();
    let mut expected = Vec::new();
    for declaration in &locals {
        scalar_work.charge(1).unwrap();
        let value = scalar.contains(declaration.ty(), &mut scalar_work).unwrap();
        assert_eq!(
            oracle(&types, &facts, declaration.ty(), &mut BTreeSet::new()),
            Ok(value)
        );
        assert_eq!(
            cold.contains(declaration.ty(), &mut cold_work).unwrap(),
            value
        );
        scalar_work.charge(3).unwrap();
        expected.push(value);
    }
    let mut work = budget(MAX_FLOW_WORK);
    let mut batch = Shapes::new(&types, &facts, &mut work).unwrap();
    let mut actual = Vec::new();
    batch
        .for_each_declaration(&locals, &mut work, |index, declaration, selected, work| {
            assert_eq!(index, actual.len());
            assert!(std::ptr::eq(declaration, &locals[index]));
            work.charge(3)?;
            actual.push(selected);
            Ok(())
        })
        .unwrap();
    assert_eq!(actual, expected);
    assert_eq!(batch.memo.words, scalar.memo.words);
    assert_eq!(work.remaining - scalar_work.remaining, locals.len() - 2);
    assert_eq!(batch.pairs, scalar.pairs);
    assert_eq!(batch.barriers, scalar.barriers);
}

#[test]
fn batch_constructor_nested_duplicates_callbacks_exact_total_and_all_short_prefixes() {
    assert_eq!(MAX_FLOW_WORK, 262_144);
    let types = graph();
    let facts = facts(&types);
    let locals = declarations(&[32, 7, 2, 32]);
    let w = cells::<u64>();
    let (nested, descendant) = nested_32_charge_trace();
    // Both memo headers/allocation/init precede the unchanged three facts.
    let mut charges = vec![header_work(), 2 * w, 0, w, w];
    charges.extend([header_work(), 2 * w, 0, w, w, 8, 10, 12]);
    let constructor = charges.iter().sum::<usize>();
    charges.extend([1, 1, 1]); // batch entry, owner, first declaration
    let descendant_boundary = constructor + 3 + descendant - 1;
    charges.extend_from_slice(&nested[1..]); // only the repeated owner check is hoisted
    let before_first_callback = charges.iter().sum::<usize>();
    let mut callback_before = vec![before_first_callback];
    charges.push(3);
    let mut callback_after = vec![charges.iter().sum::<usize>()];
    for _ in 0..3 {
        charges.extend([1, w]); // each original declaration and warm memo read
        callback_before.push(charges.iter().sum::<usize>());
        charges.push(3);
        callback_after.push(charges.iter().sum::<usize>());
    }
    let total = charges.iter().sum::<usize>();
    assert_eq!(
        total,
        constructor + 78 + 43 * w + 32 * cells::<(SemanticTypeIdV1, bool)>()
    );
    let expected = [true, true, false, true];
    let mut saw_descendant_boundary = false;
    let mut saw_final_read_boundary = false;
    for limit in 0..=total {
        let mut work = budget(limit);
        let (remaining, denied) = expected_charge_prefix(limit, &charges);
        let mut shapes = match Shapes::new(&types, &facts, &mut work) {
            Ok(shapes) => shapes,
            Err(error) => {
                assert!(limit < constructor);
                assert_eq!(work.remaining, remaining);
                assert_denied_charge(error, limit, remaining, denied.unwrap());
                continue;
            }
        };
        let mut entered = Vec::new();
        let mut committed = Vec::new();
        let result = shapes.for_each_declaration(
            &locals,
            &mut work,
            |index, declaration, selected, work| {
                assert_eq!(index, entered.len());
                assert!(std::ptr::eq(declaration, &locals[index]));
                assert_eq!(selected, expected[index]);
                entered.push(index);
                work.charge(3)?;
                committed.push(selected);
                Ok(())
            },
        );
        assert_eq!(work.remaining, remaining, "limit={limit}");
        assert_eq!(
            entered.len(),
            callback_before
                .iter()
                .filter(|&&cost| cost <= limit)
                .count(),
            "entered limit={limit}"
        );
        let completed = callback_after.iter().filter(|&&cost| cost <= limit).count();
        assert_eq!(committed, expected[..completed], "committed limit={limit}");
        match (result, denied) {
            (Ok(()), None) => {
                assert_eq!(limit, total);
                assert_eq!(work.remaining, 0);
                assert_eq!(committed, expected);
                assert!(!shapes.memo.failed);
            }
            (Err(error), Some(requested)) => {
                assert_denied_charge(error, limit, remaining, requested);
                if limit == descendant_boundary {
                    saw_descendant_boundary = true;
                    assert!(entered.is_empty());
                    assert_eq!(
                        shapes.memo.get(ty(30), &mut budget(MAX_FLOW_WORK)).unwrap(),
                        Some(Some(true))
                    );
                    assert_eq!(
                        shapes.memo.get(ty(32), &mut budget(MAX_FLOW_WORK)).unwrap(),
                        Some(None)
                    );
                    assert_eq!(
                        shapes.memo.get(ty(31), &mut budget(MAX_FLOW_WORK)).unwrap(),
                        None
                    );
                }
                if limit == before_first_callback - w {
                    saw_final_read_boundary = true;
                    assert!(entered.is_empty());
                    assert_eq!(
                        shapes.memo.get(ty(32), &mut budget(MAX_FLOW_WORK)).unwrap(),
                        Some(Some(true))
                    );
                }
                poisoned_retries(&mut shapes, &locals);
            }
            _ => panic!("batch boundary mismatch at {limit}"),
        }
    }
    assert!(saw_descendant_boundary && saw_final_read_boundary);
}

#[test]
fn batch_warm_zero_one_two_and_small_censuses_have_independent_every_budget_cost() {
    let types = graph();
    let facts = facts(&types);
    let w = cells::<u64>();
    for n in 0..=5 {
        let locals = declarations(&vec![7; n]);
        let mut charges = vec![1, 1];
        for _ in 0..n {
            charges.extend([1, w, 2]);
        }
        let total = 2 + n * (3 + w);
        assert_eq!(charges.iter().sum::<usize>(), total);
        for limit in 0..=total {
            let mut setup = budget(MAX_FLOW_WORK);
            let mut shapes = Shapes::new(&types, &facts, &mut setup).unwrap();
            assert!(shapes.contains(ty(7), &mut setup).unwrap());
            let words = shapes.memo.words.clone();
            let mut work = budget(limit);
            let mut completed = 0;
            let (remaining, denied) = expected_charge_prefix(limit, &charges);
            let result =
                shapes.for_each_declaration(&locals, &mut work, |index, d, value, work| {
                    assert!(std::ptr::eq(d, &locals[index]));
                    assert!(value);
                    work.charge(2)?;
                    completed += 1;
                    Ok(())
                });
            assert_eq!(work.remaining, remaining, "n={n} limit={limit}");
            assert_eq!(shapes.memo.words, words);
            match (result, denied) {
                (Ok(()), None) => {
                    assert_eq!(completed, n);
                    assert_eq!(limit, total);
                }
                (Err(error), Some(requested)) => {
                    assert_denied_charge(error, limit, remaining, requested);
                    poisoned_retries(&mut shapes, &locals);
                }
                _ => panic!("warm boundary mismatch n={n} limit={limit}"),
            }
        }
        let mut setup = budget(MAX_FLOW_WORK);
        let mut scalar = Shapes::new(&types, &facts, &mut setup).unwrap();
        scalar.contains(ty(7), &mut setup).unwrap();
        let mut work = budget(MAX_FLOW_WORK);
        for d in &locals {
            work.charge(1).unwrap();
            assert!(scalar.contains(d.ty(), &mut work).unwrap());
            work.charge(2).unwrap();
        }
        let old = MAX_FLOW_WORK - work.remaining;
        assert_eq!(old, n * (4 + w));
        assert_eq!(old as isize - total as isize, n as isize - 2);
    }
}

#[test]
fn batch_warm_owner_foreign_allocation_and_same_base_short_slice_reject_and_poison() {
    let types = graph();
    let foreign = types.clone();
    let facts = facts(&types);
    let locals = declarations(&[7, 2]);
    assert_eq!(types[..8].as_ptr(), types.as_ptr());
    assert_ne!(types.len(), types[..8].len());
    assert!(ty(7).index() < 8);
    for replacement in [foreign.as_slice(), &types[..8]] {
        let mut setup = budget(MAX_FLOW_WORK);
        let mut shapes = Shapes::new(&types, &facts, &mut setup).unwrap();
        assert!(shapes.contains(ty(7), &mut setup).unwrap());
        let words = shapes.memo.words.clone();
        shapes.types = replacement;
        let mut work = budget(MAX_FLOW_WORK);
        let mut callbacks = 0;
        assert!(matches!(
            shapes.for_each_declaration(&locals, &mut work, |_, _, _, _| {
                callbacks += 1;
                Ok(())
            }),
            Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
        ));
        assert_eq!(callbacks, 0);
        assert_eq!(work.remaining, MAX_FLOW_WORK - 2);
        assert_eq!(shapes.memo.words, words);
        shapes.types = &types;
        poisoned_retries(&mut shapes, &locals);
    }
}

#[test]
fn batch_callback_partial_storage_and_explicit_failure_poison_completed_descendants() {
    let types = graph();
    let facts = facts(&types);
    let locals = declarations(&[32, 39, 7]);
    let (nested, _) = nested_32_charge_trace();
    let before_callback = 2 + 1 + nested[1..].iter().sum::<usize>();
    // First usize Vec: new capacity2 + zero surplus + one initialized entry.
    let partial = before_callback + 3;
    for exhaust in [false, true] {
        let mut shapes = Shapes::new(&types, &facts, &mut budget(MAX_FLOW_WORK)).unwrap();
        let mut work = budget(if exhaust { partial } else { MAX_FLOW_WORK });
        let mut entered = Vec::new();
        let mut first = Vec::<usize>::new();
        let mut second = Vec::<usize>::new();
        let result = shapes.for_each_declaration(&locals, &mut work, |index, _, selected, work| {
            assert!(selected);
            entered.push(index);
            push(&mut first, index, work)?;
            if exhaust {
                push(&mut second, index, work)
            } else {
                Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
            }
        });
        let error = result.unwrap_err();
        if exhaust {
            assert_denied_charge(error, partial, 0, 2);
        } else {
            assert!(matches!(
                error,
                ProductionSemanticSsaErrorV1::ReplayMismatch
            ));
            assert_eq!(MAX_FLOW_WORK - work.remaining, partial);
        }
        assert_eq!(entered, vec![0]);
        assert_eq!(first, vec![0]);
        assert!(second.is_empty());
        assert_eq!(
            shapes.memo.get(ty(30), &mut budget(MAX_FLOW_WORK)).unwrap(),
            Some(Some(true))
        );
        assert_eq!(
            shapes.memo.get(ty(32), &mut budget(MAX_FLOW_WORK)).unwrap(),
            Some(Some(true))
        );
        assert_eq!(
            shapes.memo.get(ty(39), &mut budget(MAX_FLOW_WORK)).unwrap(),
            None
        );
        poisoned_retries(&mut shapes, &locals);
    }
}

#[test]
fn batch_later_invalid_type_or_cycle_keeps_prior_callback_but_never_visits_suffix() {
    let types = graph();
    let facts = facts(&types);
    for invalid in [34, 35, 36, 37, 38, u32::MAX] {
        let locals = declarations(&[7, invalid, 39]);
        let mut shapes = Shapes::new(&types, &facts, &mut budget(MAX_FLOW_WORK)).unwrap();
        let mut work = budget(MAX_FLOW_WORK);
        let mut visited = Vec::new();
        assert!(matches!(
            shapes.for_each_declaration(&locals, &mut work, |index, d, selected, _| {
                assert!(std::ptr::eq(d, &locals[index]));
                visited.push((index, selected));
                Ok(())
            }),
            Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
        ));
        assert_eq!(visited, vec![(0, true)]);
        assert_eq!(
            shapes.memo.get(ty(39), &mut budget(MAX_FLOW_WORK)).unwrap(),
            None
        );
        poisoned_retries(&mut shapes, &locals);
    }
}

#[test]
fn batch_equal_length_foreign_declarations_never_reuse_a_prior_census() {
    let types = graph();
    let facts = facts(&types);
    let first = declarations(&[7, 2, 32, 8]);
    let foreign = declarations(&[2, 7, 8, 32]);
    assert_eq!(first.len(), foreign.len());
    assert_ne!(first.as_ptr(), foreign.as_ptr());
    let mut shapes = Shapes::new(&types, &facts, &mut budget(MAX_FLOW_WORK)).unwrap();
    for locals in [&first, &foreign, &first] {
        let mut visited = Vec::new();
        shapes
            .for_each_declaration(
                locals,
                &mut budget(MAX_FLOW_WORK),
                |index, d, selected, _| {
                    assert_eq!(index, visited.len());
                    assert!(std::ptr::eq(d, &locals[index]));
                    assert_eq!(
                        oracle(&types, &facts, d.ty(), &mut BTreeSet::new()),
                        Ok(selected)
                    );
                    visited.push(d.ty());
                    Ok(())
                },
            )
            .unwrap();
        assert_eq!(
            visited,
            locals
                .iter()
                .map(SemanticLocalDeclV1::ty)
                .collect::<Vec<_>>()
        );
    }
}
