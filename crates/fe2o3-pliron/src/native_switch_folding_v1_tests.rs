use super::*;

fn switch_fixture(selector: Option<Constant>, cases: [Constant; 2], legacy: Option<[u64; 2]>) -> Module {
    let ty = cases[0].ty();
    let mut entry = BasicBlock::new(BlockId(10));
    let selector = if let Some(literal) = selector {
        entry.operations.push(constant(4, literal));
        ValueId(4)
    } else { ValueId(0) };
    let payload = [vec![ValueId(1), ValueId(1)], vec![ValueId(2), ValueId(1)]];
    entry.terminator = Some(if let Some(keys) = legacy {
        Terminator::Switch { selector,
            cases: keys.into_iter().zip(payload).map(|(value, arguments)| SwitchCase {
                value, target: BlockId(70), arguments,
            }).collect(), default_target: BlockId(70), default_arguments: vec![ValueId(3), ValueId(2)] }
    } else {
        Terminator::IntegerSwitch { selector,
            cases: cases.into_iter().zip(payload).map(|(value, arguments)| IntegerSwitchCase {
                value, target: BlockId(70), arguments,
            }).collect(), default_target: BlockId(70), default_arguments: vec![ValueId(3), ValueId(2)] }
    });
    let mut join = returning(70, &[5, 6]);
    join.parameters.extend([value(5), value(6)]);
    function_module(vec![ty, u32_type(), u32_type(), u32_type()], vec![u32_type(), u32_type()],
        (0..4).map(ValueId).collect(), vec![entry, join])
}

fn matrix() -> Vec<([Constant; 2], Constant, [u64; 2])> {
    vec![
        ([Constant::I8(i8::MIN), Constant::I8(7)], Constant::I8(1), [128, 7]),
        ([Constant::I16(i16::MIN), Constant::I16(7)], Constant::I16(1), [32768, 7]),
        ([Constant::I32(i32::MIN), Constant::I32(7)], Constant::I32(1), [2147483648, 7]),
        ([Constant::I64(i64::MIN), Constant::I64(7)], Constant::I64(1), [1_u64 << 63, 7]),
        ([Constant::U8(0), Constant::U8(u8::MAX)], Constant::U8(1), [0, 255]),
        ([Constant::U16(0), Constant::U16(u16::MAX)], Constant::U16(1), [0, 65535]),
        ([Constant::U32(0), Constant::U32(u32::MAX)], Constant::U32(1), [0, 4294967295]),
        ([Constant::U64(0), Constant::U64(u64::MAX)], Constant::U64(1), [0, u64::MAX]),
        ([Constant::Index(0), Constant::Index(u64::MAX)], Constant::Index(1), [0, u64::MAX]),
    ]
}

#[test]
fn actual_switch_fold_checks_case_default_and_duplicate_destination_payloads() {
    for (cases, otherwise, bits) in matrix() {
        for legacy in [None, Some(bits)] {
            for (selector, successor, arguments) in [
                (cases[0].clone(), 0, [1, 1]),
                (cases[1].clone(), 1, [2, 1]),
                (otherwise.clone(), 2, [3, 2]),
            ] {
                with_transition(&switch_fixture(Some(selector), cases.clone(), legacy), |observed, input, output, budget| {
                    let rows = observed.occurrences().candidate();
                    assert_eq!(output.blocks().len(), 1);
                    assert_eq!(rows.segments.len(), 2);
                    assert_eq!(rows.segments[0].connector, Some(Edge { source: b(0), successor }));
                    assert!(rows.edges.is_empty());
                    let uses = output.blocks()[0].terminator_uses.clone();
                    assert_eq!(uses.len(), 2);
                    for (use_row, argument) in output.uses()[uses].iter().zip(arguments) {
                        assert_eq!(output.definitions()[use_row.definition].coordinate,
                            Definition::FunctionArgument { function: FunctionCoordinate(0), argument });
                    }
                    // An occurrence event is only a proposal: substituting a
                    // different same-target edge must fail independent replay.
                    with_row_copy(rows.segments, budget, |changed, budget| {
                        changed[0].connector = Some(Edge { source: b(0), successor: (successor + 1) % 3 });
                        assert!(matches!(rejected(input, output, Candidate { segments: changed, ..rows }, budget), CheckError::Rule(_)));
                    });
                    accepted(input, output, rows, budget);
                });
            }
        }
    }
}

#[test]
fn actual_unknown_switch_keeps_all_edge_occurrences_and_exact_argument_roles() {
    for (cases, _, bits) in matrix() {
        for legacy in [None, Some(bits)] {
            with_transition(&switch_fixture(None, cases.clone(), legacy), |observed, input, output, budget| {
                let rows = observed.occurrences().candidate();
                assert_eq!(output.edges().len(), 3);
                assert_eq!(output.edge_arguments().len(), 6);
                for (ordinal, edge) in rows.edges.iter().enumerate() {
                    assert_eq!(edge.input, Edge { source: b(0), successor: ordinal as u32 });
                }
                with_row_copy(rows.edges, budget, |changed, budget| {
                    let first = changed[0].input;
                    changed[0].input = changed[1].input;
                    changed[1].input = first;
                    assert_eq!(rejected(input, output, Candidate { edges: changed, ..rows }, budget),
                        CheckError::Rule("successor occurrence order"));
                });
                with_row_copy(rows.edge_arguments, budget, |changed, budget| {
                    changed[2].input = changed[0].input;
                    assert_eq!(rejected(input, output, Candidate { edge_arguments: changed, ..rows }, budget),
                        CheckError::Rule("edge argument parameter transport"));
                });
            });
        }
    }
}

#[test]
fn actual_default_only_128_switches_keep_the_only_argument_occurrence() {
    for scalar in [ScalarType::I128, ScalarType::U128] {
        for typed in [false, true] {
            let mut entry = BasicBlock::new(BlockId(10));
            entry.terminator = Some(if typed {
                Terminator::IntegerSwitch { selector: ValueId(0), cases: vec![],
                    default_target: BlockId(70), default_arguments: vec![ValueId(1)] }
            } else {
                Terminator::Switch { selector: ValueId(0), cases: vec![],
                    default_target: BlockId(70), default_arguments: vec![ValueId(1)] }
            });
            let mut join = returning(70, &[2]);
            join.parameters.push(value(2));
            let module = function_module(vec![Type::Scalar(scalar), u32_type()], vec![u32_type()],
                vec![ValueId(0), ValueId(1)], vec![entry, join]);
            with_transition(&module, |observed, _, output, _| {
                assert_eq!(output.blocks().len(), 1);
                assert_eq!(observed.occurrences().candidate().segments[0].connector,
                    Some(Edge { source: b(0), successor: 0 }));
                assert_eq!(output.definitions()[output.uses().last().unwrap().definition].coordinate,
                    Definition::FunctionArgument { function: FunctionCoordinate(0), argument: 1 });
            });
        }
    }
}

#[test]
fn actual_switch_fold_checker_has_exact_and_one_short_resource_boundaries() {
    for selected in [Constant::I32(-7), Constant::I32(0)] {
        let module = switch_fixture(Some(selected), [Constant::I32(-7), Constant::I32(9)], None);
        with_transition(&module, |observed, input, output, _| {
            let rows = observed.occurrences().candidate();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.charge_work(13).unwrap();
            budget.reserve_storage(PREFIX).unwrap();
            let (checked, _) = check(input, output, rows, &mut budget).unwrap();
            drop(checked);
            assert_eq!(budget.storage(), PREFIX);
            let (required, peak) = (budget.work(), budget.peak_storage());
            for (work_limit, storage_limit, succeeds) in [
                (required, peak, true), (required - 1, peak, false), (required, peak - 1, false),
            ] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
                let mut budget = Budget::new(&mut work, storage_limit);
                budget.charge_work(13).unwrap();
                budget.reserve_storage(PREFIX).unwrap();
                let result = check(input, output, rows, &mut budget);
                if succeeds {
                    drop(result.unwrap());
                    assert_eq!(budget.work(), required);
                    assert_eq!(budget.peak_storage(), peak);
                } else {
                    assert!(matches!(result, Err(CheckError::Resource(_))));
                }
                assert_eq!(budget.storage(), PREFIX);
            }
        });
    }
}

#[test]
fn checked_switch_control_exposes_the_selected_occurrence_not_just_its_target() {
    use fe2o3_kernel_analysis::CheckedCanonicalKirControlIndexV1 as Control;
    for (selector, selected) in [(Some(Constant::I32(-7)), Some(0)),
        (Some(Constant::I32(9)), Some(1)), (Some(Constant::I32(0)), Some(2)), (None, None)] {
        with_transition(&switch_fixture(selector, [Constant::I32(-7), Constant::I32(9)], None),
            |observed, input, output, budget| {
                let floor = budget.storage();
                let (checked, receipt) = check(input, output, observed.occurrences().candidate(), budget).unwrap();
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                let (control, retained) = Control::derive(&checked, budget).unwrap();
                budget.reserve_storage(retained.retained_storage()).unwrap();
                assert_eq!(control.block(b(0), budget).unwrap().selected_successor,
                    selected.map(|successor| Edge { source: b(0), successor }));
                for successor in 0..3 {
                    assert_eq!(control.edge(Edge { source: b(0), successor }, budget).unwrap().executable,
                        selected.is_none_or(|selected| selected == successor));
                }
                drop(control);
                budget.release_storage(retained.retained_storage()).unwrap();
                drop(checked);
                budget.release_storage(receipt.retained_storage()).unwrap();
                assert_eq!(budget.storage(), floor);
            });
    }
}

#[test]
fn checked_switch_key_work_scales_with_the_case_roster_not_its_square() {
    let mut previous = None;
    for count in [16_u32, 64, 256, 1024] {
        let mut entry = BasicBlock::new(BlockId(10));
        entry.operations.push(constant(2, Constant::U32(u32::MAX)));
        entry.terminator = Some(Terminator::IntegerSwitch { selector: ValueId(2),
            cases: (0..count).map(|key| IntegerSwitchCase { value: Constant::U32(key),
                target: BlockId(70), arguments: vec![ValueId(0)] }).collect(),
            default_target: BlockId(70), default_arguments: vec![ValueId(1)] });
        let mut join = returning(70, &[3]);
        join.parameters.push(value(3));
        let module = function_module(vec![u32_type(), u32_type()], vec![u32_type()],
            vec![ValueId(0), ValueId(1)], vec![entry, join]);
        with_transition(&module, |observed, input, output, budget| {
            let rows = observed.occurrences().candidate();
            assert_eq!(rows.segments[0].connector, Some(Edge { source: b(0), successor: count }));
            let before = budget.work();
            accepted(input, output, rows, budget);
            let used = budget.work() - before;
            if let Some(previous) = previous {
                assert!(used <= previous * 5, "quadrupling keys must not square checker work: {previous} -> {used}");
            }
            previous = Some(used);
        });
    }
}
