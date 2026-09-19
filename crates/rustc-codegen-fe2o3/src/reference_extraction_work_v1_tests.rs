use super::*;

fn constant(bits: u128) -> ReferenceEffectExpressionV1 {
    ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::Scalar {
        scalar: ReferenceScalarTypeV1::U32,
        bits,
    })
}

fn expression(depth: usize) -> ReferenceEffectExpressionV1 {
    let mut result = constant(7);
    for _ in 0..depth {
        result = ReferenceEffectExpressionV1::Unary {
            operation: ReferenceUnaryOpV1::Not,
            operand: Box::new(result),
        };
    }
    result
}

fn atom(depth: usize) -> ReferenceGuardAtomV1 {
    ReferenceGuardAtomV1::SwitchValueSet {
        discriminant: expression(depth),
        values: vec![0, 1, 2].into_boxed_slice(),
        inside_set: true,
    }
}

fn exact_and_one_short<T: std::fmt::Debug>(
    operation: impl Fn(&ReferenceExtractionWorkV1<'_>) -> Result<T, ReferenceBindingErrorV1>,
) -> (T, u64) {
    use fe2o3_mir_model::semantic_mir_v1::HARD_MAX_VALIDATION_WORK_V1;

    let mut measure = SourceClosureWorkV1::default();
    let value = operation(&ReferenceExtractionWorkV1::borrowed(&mut measure)).unwrap();
    let cost = measure.validation_work_for_test();
    assert!(cost > 0 && cost < HARD_MAX_VALIDATION_WORK_V1);

    let mut exact = SourceClosureWorkV1::default();
    exact
        .charge((HARD_MAX_VALIDATION_WORK_V1 - cost) as usize)
        .unwrap();
    operation(&ReferenceExtractionWorkV1::borrowed(&mut exact)).unwrap();
    assert_eq!(
        exact.validation_work_for_test(),
        HARD_MAX_VALIDATION_WORK_V1
    );

    let mut short = SourceClosureWorkV1::default();
    let before = HARD_MAX_VALIDATION_WORK_V1 - cost + 1;
    short.charge(before as usize).unwrap();
    let error = operation(&ReferenceExtractionWorkV1::borrowed(&mut short)).unwrap_err();
    assert!(!error.to_string().is_empty());
    assert!(short.validation_work_for_test() >= before);
    (value, cost)
}

#[test]
fn expression_census_and_clone_debit_use_the_same_inherited_ledger() {
    let expression = expression(8);
    let (_, cost) = exact_and_one_short(|meter| meter.clone_expression(&expression));
    let mut work = SourceClosureWorkV1::default();
    work.charge(23).unwrap();
    ReferenceExtractionWorkV1::borrowed(&mut work)
        .clone_expression(&expression)
        .unwrap();
    assert_eq!(work.validation_work_for_test(), 23 + cost);
}

#[test]
fn expression_counting_itself_fails_before_payload_clone_when_work_is_exhausted() {
    use fe2o3_mir_model::semantic_mir_v1::HARD_MAX_VALIDATION_WORK_V1;

    let expression = expression(8);
    let mut work = SourceClosureWorkV1::default();
    work.charge(HARD_MAX_VALIDATION_WORK_V1 as usize).unwrap();
    assert!(
        ReferenceExtractionWorkV1::borrowed(&mut work)
            .expression(&expression)
            .is_err()
    );
}

#[test]
fn recursive_payload_depth_is_checked_before_recursive_clone() {
    let expression = expression(fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1);
    let mut work = SourceClosureWorkV1::default();
    let error = ReferenceExtractionWorkV1::borrowed(&mut work)
        .clone_expression(&expression)
        .unwrap_err();
    assert!(error.to_string().contains("depth"));
    assert!(work.validation_work_for_test() > 0);
}

#[test]
fn constant_folding_prepays_traversal_on_the_inherited_ledger() {
    let expression = expression(8);
    let rhs = constant(u128::from(u32::MAX));
    let evaluate = |meter: &ReferenceExtractionWorkV1<'_>| {
        Ok((
            reference_fold_constant_v2(meter, &expression)?,
            reference_constant_bits_v2(meter, &expression)?,
            reference_checked_overflow_v2(meter, ReferenceBinaryOpV1::Add, &expression, &rhs)?,
        ))
    };
    let (result, cost) = exact_and_one_short(evaluate);
    assert_eq!(
        result,
        (
            Some(constant(7)),
            Some((ReferenceScalarTypeV1::U32, 7)),
            Some(true)
        )
    );
    // Three traversals of the nine-node lhs and one of the scalar rhs.
    let minimum = 4 * 28 * (std::mem::size_of::<ReferenceEffectExpressionV1>() + 1);
    assert!(cost >= minimum as u64);
    let mut work = SourceClosureWorkV1::default();
    work.charge(29).unwrap();
    assert_eq!(
        evaluate(&ReferenceExtractionWorkV1::borrowed(&mut work)).unwrap(),
        result
    );
    assert_eq!(work.validation_work_for_test(), 29 + cost);
}

#[test]
fn constant_folding_rejects_deep_trees_before_recursive_evaluation() {
    let expression = expression(fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1);
    let rhs = constant(1);
    let mut work = SourceClosureWorkV1::default();
    let meter = ReferenceExtractionWorkV1::borrowed(&mut work);
    assert!(
        reference_fold_constant_v2(&meter, &expression)
            .unwrap_err()
            .to_string()
            .contains("depth")
    );
    assert!(
        reference_constant_bits_v2(&meter, &expression)
            .unwrap_err()
            .to_string()
            .contains("depth")
    );
    assert!(
        reference_checked_overflow_v2(&meter, ReferenceBinaryOpV1::Add, &expression, &rhs)
            .unwrap_err()
            .to_string()
            .contains("depth")
    );
}

#[test]
fn nonconstant_wrappers_keep_budget_errors_distinct_from_none_and_check_rhs_depth() {
    let lhs = constant(1);
    let nonconstant = ReferenceEffectExpressionV1::Binary {
        operation: ReferenceBinaryOpV1::Add,
        lhs: Box::new(constant(7)),
        rhs: Box::new(ReferenceEffectExpressionV1::KernelScalarArgument { argument: 0 }),
        checked: false,
    };
    let fold = |meter: &ReferenceExtractionWorkV1<'_>| {
        assert_eq!(reference_fold_constant_v2(meter, &nonconstant)?, None);
        Ok(())
    };
    let bits = |meter: &ReferenceExtractionWorkV1<'_>| {
        assert_eq!(reference_constant_bits_v2(meter, &nonconstant)?, None);
        Ok(())
    };
    let overflow = |meter: &ReferenceExtractionWorkV1<'_>| {
        assert_eq!(
            reference_checked_overflow_v2(meter, ReferenceBinaryOpV1::Add, &lhs, &nonconstant)?,
            None
        );
        Ok(())
    };
    let (_, fold_cost) = exact_and_one_short(fold);
    let (_, bits_cost) = exact_and_one_short(bits);
    let (_, overflow_cost) = exact_and_one_short(overflow);

    let mut work = SourceClosureWorkV1::default();
    work.charge(31).unwrap();
    {
        let meter = ReferenceExtractionWorkV1::borrowed(&mut work);
        fold(&meter).unwrap();
        bits(&meter).unwrap();
        overflow(&meter).unwrap();
    }
    let before_depth_check = 31 + fold_cost + bits_cost + overflow_cost;
    assert_eq!(work.validation_work_for_test(), before_depth_check);

    let deep_rhs = expression(fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1);
    {
        let meter = ReferenceExtractionWorkV1::borrowed(&mut work);
        // A nonconstant lhs must not short-circuit the RHS resource check either.
        for shallow_lhs in [&lhs, &nonconstant] {
            let error = reference_checked_overflow_v2(
                &meter,
                ReferenceBinaryOpV1::Add,
                shallow_lhs,
                &deep_rhs,
            )
            .unwrap_err();
            assert!(error.to_string().contains("depth"), "{error}");
        }
    }
    assert!(work.validation_work_for_test() > before_depth_check);
}

#[test]
fn predicate_clone_sort_and_dedup_have_exact_and_one_short_shared_boundaries() {
    let predicate = ReferencePathPredicateV1 {
        clauses: vec![
            ReferenceGuardClauseV1 {
                atoms: vec![atom(1), atom(2)].into_boxed_slice(),
            },
            ReferenceGuardClauseV1 {
                atoms: vec![atom(3)].into_boxed_slice(),
            },
        ]
        .into_boxed_slice(),
    };
    let atom = atom(2);
    let (result, _) = exact_and_one_short(|meter| {
        meter.charge(meter.atom(&atom)?)?;
        reference_predicate_and_atom_v1(meter, &predicate, atom.clone())
    });
    assert_eq!(result.clauses.len(), 2);
}

#[test]
fn helper_substitution_prepays_deep_argument_clones() {
    let arguments = [expression(12)];
    let summary = ReferenceEffectExpressionV1::Binary {
        operation: ReferenceBinaryOpV1::Add,
        lhs: Box::new(ReferenceEffectExpressionV1::KernelScalarArgument { argument: 0 }),
        rhs: Box::new(ReferenceEffectExpressionV1::KernelScalarArgument { argument: 0 }),
        checked: false,
    };
    let (result, _) = exact_and_one_short(|meter| {
        substitute_helper_summary_v2(meter, &summary, &arguments, &mut 0, 0)
    });
    assert!(matches!(result, ReferenceEffectExpressionV1::Binary { .. }));
}

#[test]
fn many_helper_argument_clones_prepay_full_operand_rows_with_inherited_boundaries() {
    use std::cell::Cell;
    use std::mem::size_of;

    const ARGUMENTS: usize = 64;
    let value = ReferenceValueV1::SafeHelperCall {
        helper: ReferenceFunctionIdentityV1 {
            def_path_hash: [0; 16],
            function_sha256: [0; 32],
            item_definition_sha256: [0; 32],
            monomorphization_sha256: [0; 32],
            generic_type_arguments_sha256: [0; 32],
            const_generic_arguments_sha256: [0; 32],
            rustc_mir_body_sha256: [0; 32],
        },
        parameters: vec![ReferenceScalarTypeV1::U32; ARGUMENTS].into_boxed_slice(),
        result: ReferenceScalarTypeV1::U32,
        arguments: (0..ARGUMENTS)
            .map(|argument| {
                let place = ReferencePlaceV1 {
                    local: u32::try_from(argument + 1).unwrap(),
                    projection: Box::default(),
                };
                if argument % 2 == 0 {
                    ReferenceOperandV1::Copy(place)
                } else {
                    ReferenceOperandV1::Move(place)
                }
            })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        summary: Box::new(ReferenceEffectExpressionV1::KernelScalarArgument { argument: 0 }),
    };
    // Empty projections isolate the boxed operand rows from nested place payloads.
    let minimum_payload_bytes = size_of::<ReferenceValueV1>()
        + ARGUMENTS * (size_of::<ReferenceScalarTypeV1>() + size_of::<ReferenceOperandV1>())
        + size_of::<ReferenceEffectExpressionV1>();
    let clones = Cell::new(0_usize);
    let clone_value = |meter: &ReferenceExtractionWorkV1<'_>| {
        let units = meter.value(&value)?;
        assert!(
            units >= minimum_payload_bytes,
            "clone debit {units} undercounts {minimum_payload_bytes} payload bytes",
        );
        meter.charge(units)?;
        clones.set(clones.get() + 1);
        Ok(value.clone())
    };
    let (cloned, cost) = exact_and_one_short(clone_value);
    assert_eq!(cloned, value);
    assert_eq!(clones.get(), 2, "one-short work must refuse before cloning");

    let mut inherited = SourceClosureWorkV1::default();
    inherited.charge(23).unwrap();
    let cloned = clone_value(&ReferenceExtractionWorkV1::borrowed(&mut inherited)).unwrap();
    assert_eq!(cloned, value);
    assert_eq!(inherited.validation_work_for_test(), 23 + cost);
    assert_eq!(clones.get(), 3);
}

fn acyclic_effect() -> ReferenceEffectIrV1 {
    ReferenceEffectIrV1 {
        argument_count: 2,
        local_count: 3,
        relations: vec![
            ReferenceArgumentRelationV1::PointCoordinate {
                reference_argument: 0,
                axis: 0,
            },
            ReferenceArgumentRelationV1::DisjointOutputCoordinate {
                argument: 0,
                element: ReferenceScalarTypeV1::U32,
            },
        ]
        .into_boxed_slice(),
        blocks: vec![
            ReferenceBlockV1 {
                block: 0,
                assignments: Box::default(),
                terminator: ReferenceTerminatorV1::Switch {
                    discriminant: ReferenceOperandV1::Copy(ReferencePlaceV1 {
                        local: 1,
                        projection: Box::default(),
                    }),
                    values: vec![(0, 1), (1, 1)].into_boxed_slice(),
                    otherwise: 2,
                },
            },
            ReferenceBlockV1 {
                block: 1,
                assignments: vec![ReferenceAssignmentV1 {
                    statement: 0,
                    destination: ReferencePlaceV1 {
                        local: 2,
                        projection: vec![ReferencePlaceProjectionV1::Dereference]
                            .into_boxed_slice(),
                    },
                    value: ReferenceValueV1::Use(ReferenceOperandV1::Constant(
                        ReferenceConstantV1::Scalar {
                            scalar: ReferenceScalarTypeV1::U32,
                            bits: 7,
                        },
                    )),
                }]
                .into_boxed_slice(),
                terminator: ReferenceTerminatorV1::Return,
            },
            ReferenceBlockV1 {
                block: 2,
                assignments: Box::default(),
                terminator: ReferenceTerminatorV1::Return,
            },
        ]
        .into_boxed_slice(),
        loop_summaries: Box::default(),
        observable_output_effects: Box::default(),
    }
}

#[test]
fn acyclic_effect_derivation_uses_full_shared_query_boundaries() {
    let ir = acyclic_effect();
    let (writes, _) = exact_and_one_short(|meter| ir.observable_output_writes_v1(meter));
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].argument, 0);
    assert_eq!(writes[0].block, 1);
}

#[test]
fn effect_hash_prepayment_preserves_digest_and_accounts_nested_payloads() {
    let mut ir = acyclic_effect();
    ir.observable_output_effects = ir
        .observable_output_writes_v1(&ReferenceExtractionWorkV1::Inspection)
        .unwrap()
        .into_boxed_slice();
    let expected = ir.canonical_sha256_v1();
    let (actual, _) = exact_and_one_short(|meter| {
        meter.ir_hash(&ir)?;
        Ok(ir.canonical_sha256_v1())
    });
    assert_eq!(actual, expected);
}

#[test]
fn successor_iteration_preserves_duplicates_and_default_order_without_allocation() {
    let terminator = ReferenceTerminatorV1::Switch {
        discriminant: ReferenceOperandV1::Constant(ReferenceConstantV1::ZeroSized),
        values: vec![(7, 3), (2, 3)].into_boxed_slice(),
        otherwise: 3,
    };
    assert_eq!(
        reference_successors_v1(&terminator).collect::<Vec<_>>(),
        vec![3, 3, 3]
    );
}

#[test]
fn debit_arithmetic_overflow_fails_without_resetting_prior_work() {
    let mut work = SourceClosureWorkV1::default();
    work.charge(29).unwrap();
    assert!(
        ReferenceExtractionWorkV1::borrowed(&mut work)
            .product(usize::MAX, 2)
            .is_err()
    );
    assert_eq!(work.validation_work_for_test(), 29);
}
