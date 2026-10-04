use super::super::VerificationNumericIndexV1;
use super::*;
use crate::{
    BlockId, CanonicalKernelIrWorkBudgetV1, ScalarType, Type, VerificationDefinitionSiteV1,
    VerificationDefinitionV1,
};

fn build_v1(
    keys: &[u32],
    cells: usize,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<VerificationNumericIndexV1<usize>, CanonicalKernelIrVerificationResourceErrorV1> {
    VerificationNumericIndexV1::build(keys.len(), cells, budget, |ordinal| {
        Ok(VerificationNumericIndexRowV1 {
            key: keys[ordinal],
            value: ordinal,
        })
    })
}

#[test]
fn dense_selection_uses_span_not_maximum_and_leaves_tiny_indices_unchanged() {
    for (keys, dense, expected_work, retained, peak) in [
        (vec![], false, 0, 0, 0),
        (vec![u32::MAX], false, 1, 2, 2),
        (vec![0, 3], true, 27, 13, 17),
        (vec![u32::MAX - 3, u32::MAX], true, 27, 13, 17),
        (vec![8, 8], true, 21, 10, 14),
        (vec![0, 4], false, 14, 4, 4),
        (vec![0, u32::MAX], false, 14, 4, 4),
        (vec![u32::MAX, 0], false, 2_088, 4, 8),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(expected_work);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, peak);
        let index = build_v1(&keys, 2, &mut budget).unwrap();
        assert_eq!(index.dense.is_some(), dense, "{keys:?}");
        assert_eq!(
            (budget.work(), budget.storage(), budget.peak_storage()),
            (expected_work, retained, peak),
            "{keys:?}"
        );
        index.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn stable_dense_rows_and_last_matches_agree_with_exhaustive_input_oracle() {
    // 364 sequences at each origin, including holes, duplicates, and both u32 extremes.
    for minimum in [0, u32::MAX - 3] {
        for count in 0..=5_u32 {
            for mut pattern in 0..3_usize.pow(count) {
                let mut keys = Vec::new();
                for _ in 0..count {
                    keys.push(minimum + [0, 1, 3][pattern % 3]);
                    pattern /= 3;
                }
                let mut expected = keys
                    .iter()
                    .copied()
                    .enumerate()
                    .map(|(ordinal, key)| (key, ordinal))
                    .collect::<Vec<_>>();
                expected.sort_by_key(|row| row.0);
                let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
                let mut budget =
                    CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, usize::MAX);
                let index = build_v1(&keys, 2, &mut budget).unwrap();
                let actual = index
                    .rows()
                    .iter()
                    .map(|row| (row.key, row.value))
                    .collect::<Vec<_>>();
                assert_eq!(actual, expected, "{keys:?}");
                for key in [0, minimum, minimum + 1, minimum + 2, minimum + 3, u32::MAX] {
                    let expected = keys.iter().rposition(|candidate| *candidate == key);
                    assert_eq!(
                        index.find(key, &mut budget).unwrap(),
                        expected,
                        "{keys:?} key={key}"
                    );
                }
                index.release(&mut budget).unwrap();
                assert_eq!(budget.storage(), 0);
            }
        }
    }
}

#[test]
fn duplicate_rows_keep_borrowed_type_and_exact_definition_site() {
    let types = [
        Type::Scalar(ScalarType::U32),
        Type::Scalar(ScalarType::I32),
        Type::Scalar(ScalarType::U64),
    ];
    let keys = [7, 6, 7];
    let sites = [
        VerificationDefinitionSiteV1::FunctionParameter,
        VerificationDefinitionSiteV1::BlockParameter(BlockId(2)),
        VerificationDefinitionSiteV1::Operation(BlockId(9), 4),
    ];
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, usize::MAX);
    let index = VerificationNumericIndexV1::build(3, 6, &mut budget, |ordinal| {
        Ok(VerificationNumericIndexRowV1 {
            key: keys[ordinal],
            value: VerificationDefinitionV1 {
                ty: &types[ordinal],
                site: sites[ordinal],
            },
        })
    })
    .unwrap();
    assert_eq!(
        index.rows().iter().map(|row| row.key).collect::<Vec<_>>(),
        [6, 7, 7]
    );
    for (row, input) in index.rows().iter().zip([1, 0, 2]) {
        assert!(std::ptr::eq(row.value.ty, &types[input]));
        assert_eq!(row.value.site, sites[input]);
    }
    let selected = index.find(7, &mut budget).unwrap().unwrap();
    assert!(std::ptr::eq(selected.ty, &types[2]));
    assert_eq!(
        selected.site,
        VerificationDefinitionSiteV1::Operation(BlockId(9), 4)
    );
    index.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn dense_build_exact_work_and_all_admission_prefixes_are_independent() {
    // N4/S3/r2: rows4 + census10 + selector5 + dense(16+6) =41;
    // retained8+3+5=16, peak retained+scratch8=24.
    for (limit, accepted, rejected, callbacks) in [
        (3, 0, Some(4), 0),
        (4, 4, Some(14), 4),
        (13, 4, Some(14), 4),
        (14, 14, Some(19), 4),
        (18, 14, Some(19), 4),
        (19, 19, Some(41), 4),
        (40, 19, Some(41), 4),
        (41, 41, None, 4),
    ] {
        let mut called = Vec::new();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 24);
        let result = VerificationNumericIndexV1::build(4, 2, &mut budget, |ordinal| {
            called.push(ordinal);
            Ok(VerificationNumericIndexRowV1 {
                key: [12, 10, 12, 10][ordinal],
                value: ordinal,
            })
        });
        if let Some(rejected) = rejected {
            assert!(
                matches!(result, Err(CanonicalKernelIrVerificationResourceErrorV1::Work(error))
                if error.actual() == rejected && error.limit() == limit)
            );
        } else {
            let index = result.unwrap();
            assert_eq!((budget.storage(), budget.peak_storage()), (16, 24));
            index.release(&mut budget).unwrap();
        }
        assert_eq!(called, (0..callbacks).collect::<Vec<_>>());
        assert_eq!((budget.work(), budget.storage()), (accepted, 0));
        assert_eq!(work.failed_work(), rejected);
    }
}

#[test]
fn dense_storage_denial_does_not_fall_back_or_publish_an_owner() {
    for (limit, callbacks, peak, rejected, accepted) in [
        (7, 0, 0, Some(8), 4),
        (8, 4, 8, Some(24), 41),
        (23, 4, 8, Some(24), 41),
        (24, 4, 24, None, 41),
    ] {
        let mut called = 0;
        let mut work = CanonicalKernelIrWorkBudgetV1::new(41);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, limit);
        let result = VerificationNumericIndexV1::build(4, 2, &mut budget, |ordinal| {
            called += 1;
            Ok(VerificationNumericIndexRowV1 {
                key: [12, 10, 12, 10][ordinal],
                value: ordinal,
            })
        });
        if let Some(rejected) = rejected {
            assert!(
                matches!(result, Err(CanonicalKernelIrVerificationResourceErrorV1::Storage(error))
                if error.actual() == rejected && error.limit() == limit)
            );
        } else {
            result.unwrap().release(&mut budget).unwrap();
        }
        assert_eq!(called, callbacks);
        assert_eq!(
            (
                budget.work(),
                budget.storage(),
                budget.peak_storage(),
                budget.failed_storage()
            ),
            (accepted, 0, peak, rejected)
        );
    }
}

#[test]
fn dense_queries_charge_exact_hits_holes_and_outside_without_mutation() {
    for (key, expected, query_work) in [
        (10, Some(3), 4),
        (12, Some(2), 4),
        (11, None, 3),
        (9, None, 1),
        (13, None, 1),
        (u32::MAX, None, 1),
    ] {
        for allowance in 0..=query_work {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(41 + allowance);
            let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 24);
            let index = build_v1(&[12, 10, 12, 10], 2, &mut budget).unwrap();
            let before = index
                .rows()
                .iter()
                .map(|row| (row.key, row.value))
                .collect::<Vec<_>>();
            let result = index.find(key, &mut budget);
            if allowance == query_work {
                assert_eq!(result.unwrap(), expected);
                assert_eq!(budget.work(), 41 + query_work);
            } else {
                let (accepted, attempted) = match allowance {
                    0 => (0, 1),
                    1 | 2 => (1, 3),
                    3 => (3, 4),
                    _ => unreachable!(),
                };
                assert!(
                    matches!(result, Err(CanonicalKernelIrVerificationResourceErrorV1::Work(error))
                    if error.actual() == 41 + attempted)
                );
                assert_eq!(budget.work(), 41 + accepted);
            }
            assert_eq!(
                index
                    .rows()
                    .iter()
                    .map(|row| (row.key, row.value))
                    .collect::<Vec<_>>(),
                before
            );
            assert_eq!((budget.storage(), budget.peak_storage()), (16, 24));
            index.release(&mut budget).unwrap();
        }
    }
}

#[test]
fn callback_errors_precede_mode_census_and_preserve_the_callers_storage() {
    for failure in 0..4 {
        let mut called = Vec::new();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(4);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 15);
        budget.reserve_storage(7).unwrap();
        let result = VerificationNumericIndexV1::build(4, 2, &mut budget, |ordinal| {
            called.push(ordinal);
            if ordinal == failure {
                return Err(CanonicalKernelIrVerificationResourceErrorV1::Accounting);
            }
            Ok(VerificationNumericIndexRowV1 {
                key: 7,
                value: ordinal,
            })
        });
        assert!(matches!(
            result,
            Err(CanonicalKernelIrVerificationResourceErrorV1::Accounting)
        ));
        assert_eq!(called, (0..=failure).collect::<Vec<_>>());
        assert_eq!(
            (budget.work(), budget.storage(), budget.peak_storage()),
            (4, 7, 15)
        );
        assert_eq!(work.failed_work(), None);
    }
}

#[test]
fn dense_work_storage_and_allocator_denials_leave_input_rows_unchanged() {
    for (work_limit, storage_limit, injection, expected) in [
        (21, 24, None, "work"),
        (22, 23, None, "storage"),
        (22, 24, Some(AllocationPointV1::Ends), "allocation"),
        (22, 24, Some(AllocationPointV1::Scratch), "allocation"),
    ] {
        let mut rows =
            [12, 10, 12, 10].map(|key| VerificationNumericIndexRowV1 { key, value: key });
        let before = rows.map(|row| (row.key, row.value));
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(8).unwrap();
        let result = build_dense_span_v1(&mut rows, 10, 3, 2, &mut budget, injection);
        let kind = match result {
            Err(CanonicalKernelIrVerificationResourceErrorV1::Work(_)) => "work",
            Err(CanonicalKernelIrVerificationResourceErrorV1::Storage(_)) => "storage",
            Err(CanonicalKernelIrVerificationResourceErrorV1::Allocation) => "allocation",
            _ => panic!("unexpected dense build outcome"),
        };
        assert_eq!(kind, expected);
        assert_eq!(rows.map(|row| (row.key, row.value)), before);
        assert_eq!(budget.storage(), 8);
        assert_eq!(budget.work(), if expected == "work" { 0 } else { 22 });
        assert_eq!(
            budget.peak_storage(),
            if expected == "allocation" { 24 } else { 8 }
        );
    }
}

#[test]
fn dense_arithmetic_overflow_precedes_all_work_allocation_and_mutation() {
    for (span, cells) in [(usize::MAX, 2), (2, usize::MAX)] {
        let mut rows = [1, 0].map(|key| VerificationNumericIndexRowV1 { key, value: key });
        let before = rows.map(|row| (row.key, row.value));
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, usize::MAX);
        let result = build_dense_span_v1(&mut rows, 0, span, cells, &mut budget, None);
        assert!(matches!(
            result,
            Err(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)
        ));
        assert_eq!(rows.map(|row| (row.key, row.value)), before);
        assert_eq!(
            (budget.work(), budget.storage(), budget.peak_storage()),
            (0, 0, 0)
        );
    }
}

#[test]
fn dense_lookup_never_returns_an_endpoint_with_a_different_exact_key() {
    let rows = [
        VerificationNumericIndexRowV1 { key: 10, value: 1 },
        VerificationNumericIndexRowV1 { key: 12, value: 2 },
    ];
    for ends in [vec![2, 2, 2], vec![3, 3, 3], vec![1, 0, 2]] {
        let dense = VerificationNumericDenseSpanV1 { minimum: 10, ends };
        let key = if dense.ends[1] == 0 { 11 } else { 10 };
        let mut work = CanonicalKernelIrWorkBudgetV1::new(4);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
        assert!(matches!(
            dense.find(&rows, key, &mut budget),
            Err(CanonicalKernelIrVerificationResourceErrorV1::Accounting)
        ));
        assert_eq!((budget.work(), budget.storage()), (4, 0));
    }
}

#[test]
fn multiple_dense_owners_release_their_table_metadata_and_rows_once() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(82);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 47);
    budget.reserve_storage(7).unwrap();
    let first = build_v1(&[12, 10, 12, 10], 2, &mut budget).unwrap();
    assert_eq!((budget.storage(), budget.peak_storage()), (23, 31));
    let second = build_v1(&[12, 10, 12, 10], 2, &mut budget).unwrap();
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (82, 39, 47)
    );
    first.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), 23);
    second.release(&mut budget).unwrap();
    assert_eq!((budget.storage(), budget.peak_storage()), (7, 47));
}

#[test]
fn mixed_type_dense_duplicate_keeps_last_definition_diagnostic_order() {
    use crate::{
        BasicBlock, BinaryOp, Constant, Diagnostic, DiagnosticCode, DiagnosticLocation, Function,
        MeteredKernelIrVerificationErrorV1, Module, Operation, OperationKind, Signature,
        Terminator, ValueDef, ValueId, verify_depth_bounded_module_with_budget_v1, verify_module,
    };
    // Definitions [10:U32 parameter, 11:U32 op0, 10:U64 op1] have N3/S2.
    // A first-match table would erase both op0 dominance errors and its type mismatch.
    let mut block = BasicBlock::new(BlockId(7));
    block.operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(11), Type::Scalar(ScalarType::U32)),
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(10),
                rhs: ValueId(10),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(10), Type::Scalar(ScalarType::U64)),
            OperationKind::Constant(Constant::U64(9)),
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let function = Function::definition(
        "duplicate",
        Signature::new(vec![Type::Scalar(ScalarType::U32)], vec![]),
        vec![ValueId(10)],
        vec![block],
    );
    let mut module = Module::new("dense-diagnostics");
    module.functions.push(function);
    let diagnostic = |operation, code, message: &str| Diagnostic {
        location: DiagnosticLocation {
            module: module.id.clone(),
            function: Some(module.functions[0].id.clone()),
            kernel: None,
            block: Some(BlockId(7)),
            operation: Some(operation),
        },
        code,
        message: message.into(),
    };
    let expected = vec![
        diagnostic(
            0,
            DiagnosticCode::NonDominatingUse,
            "definition of %10 does not dominate this use",
        ),
        diagnostic(
            0,
            DiagnosticCode::NonDominatingUse,
            "definition of %10 does not dominate this use",
        ),
        diagnostic(
            0,
            DiagnosticCode::TypeMismatch,
            "result %11 has type Scalar(U32), expected Scalar(U64)",
        ),
        diagnostic(
            1,
            DiagnosticCode::DuplicateValue,
            "SSA value %10 is defined more than once",
        ),
    ];
    assert_eq!(
        verify_module(&module).unwrap_err().into_diagnostics(),
        expected
    );
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, usize::MAX);
    let result = verify_depth_bounded_module_with_budget_v1(&module, None, &mut budget);
    match result {
        Err(MeteredKernelIrVerificationErrorV1::Verification(errors)) => {
            assert_eq!(errors.into_diagnostics(), expected);
        }
        _ => panic!("expected exact dense-duplicate verification diagnostics"),
    }
    assert_eq!(budget.storage(), 0);
}
