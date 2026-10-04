use fe2o3_kernel_ir::*;

fn assert_visitation(terminator: Terminator, expected: &[u32]) {
    let original = terminator.clone();
    let expected = expected.iter().copied().map(ValueId).collect::<Vec<_>>();
    assert_eq!(terminator.operands(), expected);
    assert_eq!(terminator.operand_count(), expected.len());
    let mut visited = Vec::new();
    terminator.visit_operands(|value| visited.push(value));
    assert_eq!(visited, expected);
    visited.clear();
    assert_eq!(
        terminator.try_visit_operands(|value| {
            visited.push(value);
            Ok::<_, (usize, ValueId)>(())
        }),
        Ok(())
    );
    assert_eq!(visited, expected);
    for stop in 0..expected.len() {
        visited.clear();
        let result = terminator.try_visit_operands(|value| {
            let ordinal = visited.len();
            visited.push(value);
            if ordinal == stop {
                Err((ordinal, value))
            } else {
                Ok(())
            }
        });
        assert_eq!(result, Err((stop, expected[stop])));
        assert_eq!(visited, expected[..=stop]);
    }
    assert_eq!(terminator, original);
}

#[test]
fn branches_preserve_condition_and_each_edge_operand_including_duplicates() {
    for arguments in [
        vec![],
        vec![ValueId(u32::MAX), ValueId(0), ValueId(u32::MAX)],
    ] {
        let expected = arguments.iter().map(|value| value.0).collect::<Vec<_>>();
        assert_visitation(
            Terminator::Branch {
                target: BlockId(17),
                arguments,
            },
            &expected,
        );
    }
    for then_arguments in [vec![], vec![ValueId(8), ValueId(8)]] {
        for else_arguments in [vec![], vec![ValueId(0), ValueId(u32::MAX)]] {
            let mut expected = vec![9];
            expected.extend(then_arguments.iter().map(|value| value.0));
            expected.extend(else_arguments.iter().map(|value| value.0));
            assert_visitation(
                Terminator::ConditionalBranch {
                    condition: ValueId(9),
                    then_target: BlockId(17),
                    then_arguments: then_arguments.clone(),
                    else_target: BlockId(17),
                    else_arguments,
                },
                &expected,
            );
        }
    }
}

#[test]
fn switches_keep_source_order_without_visiting_case_constants_or_targets() {
    // Deliberately unsorted and duplicate cases: visitation is not verification.
    let cases = [
        SwitchCase {
            value: 99,
            target: BlockId(99),
            arguments: vec![ValueId(6), ValueId(6)],
        },
        SwitchCase {
            value: 0,
            target: BlockId(0),
            arguments: vec![],
        },
        SwitchCase {
            value: 99,
            target: BlockId(99),
            arguments: vec![ValueId(u32::MAX)],
        },
    ];
    for count in 0..=cases.len() {
        let cases = cases[..count].to_vec();
        for default_arguments in [vec![], vec![ValueId(0), ValueId(6)]] {
            let mut expected = vec![7];
            expected.extend(
                cases
                    .iter()
                    .flat_map(|case| case.arguments.iter().map(|v| v.0)),
            );
            expected.extend(default_arguments.iter().map(|value| value.0));
            assert_visitation(
                Terminator::Switch {
                    selector: ValueId(7),
                    cases: cases.clone(),
                    default_target: BlockId(99),
                    default_arguments: default_arguments.clone(),
                },
                &expected,
            );
            assert_visitation(
                Terminator::IntegerSwitch {
                    selector: ValueId(7),
                    cases: cases
                        .iter()
                        .map(|case| IntegerSwitchCase {
                            value: Constant::U64(u64::MAX - case.value),
                            target: case.target,
                            arguments: case.arguments.clone(),
                        })
                        .collect(),
                    default_target: BlockId(99),
                    default_arguments,
                },
                &expected,
            );
        }
    }
}

#[test]
fn returns_and_unreachable_have_no_implicit_operands() {
    for values in [vec![], vec![ValueId(0), ValueId(u32::MAX), ValueId(0)]] {
        let expected = values.iter().map(|value| value.0).collect::<Vec<_>>();
        assert_visitation(Terminator::Return { values }, &expected);
    }
    assert_visitation(Terminator::Unreachable, &[]);
    assert_eq!(
        Terminator::Unreachable.try_visit_operands(|_| Err("unexpected")),
        Ok(())
    );
}

#[test]
fn wide_switch_rejection_stops_at_the_selector_before_any_edge_callback() {
    let terminator = Terminator::Switch {
        selector: ValueId(0),
        cases: (0..256)
            .map(|value| SwitchCase {
                value,
                target: BlockId(1),
                arguments: vec![ValueId(1); 256],
            })
            .collect(),
        default_target: BlockId(1),
        default_arguments: vec![ValueId(2); 256],
    };
    let mut callbacks = 0;
    assert_eq!(
        terminator.try_visit_operands(|value| {
            callbacks += 1;
            Err(value)
        }),
        Err(ValueId(0))
    );
    assert_eq!(callbacks, 1);
    assert_eq!(terminator.operand_count(), 1 + 256 * 256 + 256);
}

#[test]
fn verifier_preserves_duplicate_undefined_return_diagnostics() {
    let mut block = BasicBlock::new(BlockId(7));
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(0), ValueId(0)],
    });
    let mut module = Module::new("terminator-diagnostics");
    module.functions.push(Function::definition(
        "entry",
        Signature::new(vec![], vec![Type::INDEX, Type::INDEX]),
        vec![],
        vec![block],
    ));
    let expected = Diagnostic {
        location: DiagnosticLocation {
            module: module.id.clone(),
            function: Some(FunctionId::new("entry")),
            kernel: None,
            block: Some(BlockId(7)),
            operation: None,
        },
        code: DiagnosticCode::UndefinedValue,
        message: "SSA value %0 is not defined in this function".to_owned(),
    };
    assert_eq!(
        verify_module(&module).unwrap_err().diagnostics(),
        &[expected.clone(), expected]
    );
}
