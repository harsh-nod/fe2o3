use super::*;
use crate::SwitchCase;

fn switch_fixture(to: ScalarType) -> Module {
    let mut module = fixture(ScalarType::U32, AccessMode::ReadOnly);
    let entry = &mut module.functions[0].body.as_mut().unwrap().blocks[0];
    let to = Type::Scalar(to);
    entry.operations.push(op(
        40,
        to.clone(),
        OperationKind::Cast {
            kind: CastKind::ZeroExtend,
            value: ValueId(7),
            to,
        },
    ));
    entry.terminator = Some(Terminator::Switch {
        selector: ValueId(40),
        cases: vec![
            SwitchCase {
                value: 0,
                target: BlockId(30),
                arguments: vec![],
            },
            SwitchCase {
                value: 1,
                target: BlockId(20),
                arguments: vec![],
            },
        ],
        default_target: BlockId(30),
        default_arguments: vec![],
    });
    module
}

#[test]
fn bool_to_integer_switch_retains_the_exact_true_case_occurrence() {
    for to in [
        ScalarType::I8,
        ScalarType::U8,
        ScalarType::I16,
        ScalarType::U16,
        ScalarType::I32,
        ScalarType::U32,
        ScalarType::I64,
        ScalarType::U64,
    ] {
        let module = switch_fixture(to);
        let analysis = analyze(&module);
        let actual = domain(&analysis);
        assert_eq!(actual.slice(), ValueId(0));
        assert_eq!(actual.index(), ValueId(1));
        assert_eq!(actual.guard_index(), ValueId(1));
        assert_eq!(actual.length(), ValueId(6));
        // The evidence names the compared Bool, not the integer switch value.
        assert_eq!(actual.predicate(), ValueId(7));
        assert_eq!(actual.pointer(), ValueId(9));
        assert_eq!(
            actual.path(),
            FormalGuardedPathV1::TrueEdge {
                source: BlockId(10),
                ordinal: 1,
                target: BlockId(20),
            }
        );
        let [read] = analysis.obligations().accesses() else {
            panic!("one actual ordinary read")
        };
        assert_eq!(
            read.location(),
            FunctionOperationLocation::new(BlockId(20), 1)
        );
        assert_eq!(read.byte_offset(), ByteExpression::Unbounded);
    }
}

#[test]
fn false_case_and_duplicate_target_occurrences_cannot_supply_the_true_edge() {
    for shape in [0, 1, 2] {
        let mut module = switch_fixture(ScalarType::U32);
        let Some(Terminator::Switch {
            cases,
            default_target,
            ..
        }) = &mut module.functions[0].body.as_mut().unwrap().blocks[0].terminator
        else {
            unreachable!()
        };
        match shape {
            0 => {
                // This load is reached exclusively by false case occurrence 0.
                cases[0].target = BlockId(20);
                cases[1].target = BlockId(30);
            }
            1 => {
                // Equal destinations do not merge false and true occurrences.
                cases[0].target = BlockId(20);
            }
            2 => {
                // Even the default occurrence prevents a unique incoming edge.
                *default_target = BlockId(20);
            }
            _ => unreachable!(),
        }
        let analysis = analyze(&module);
        assert!(!analysis.is_complete(), "shape {shape}: {analysis:?}");
        assert!(analysis.obligations().accesses().is_empty());
        assert!(analysis.incomplete_reasons().iter().any(|reason| matches!(
            reason,
            FormalMemoryIncompleteReason::UnsupportedIndexExpression { .. }
        )));
    }
}
