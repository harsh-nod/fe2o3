//! Synthetic CFG controls; the retained r5 graph separately contains Switch.
use super::*;

fn switch(typed: bool, cases: usize, target: BlockId, default: BlockId) -> Terminator {
    if typed {
        Terminator::IntegerSwitch {
            selector: ValueId(2),
            cases: (0..cases)
                .map(|value| IntegerSwitchCase {
                    value: Constant::U32(value as u32),
                    target,
                    arguments: vec![],
                })
                .collect(),
            default_target: default,
            default_arguments: vec![],
        }
    } else {
        Terminator::Switch {
            selector: ValueId(2),
            cases: (0..cases)
                .map(|value| SwitchCase {
                    value: value as u64,
                    target,
                    arguments: vec![],
                })
                .collect(),
            default_target: default,
            default_arguments: vec![],
        }
    }
}

fn switched(typed: bool, cases: usize, target: BlockId, default: BlockId) -> Module {
    let mut module = graph();
    module.functions[0].signature.parameters[2] = Type::Scalar(if typed {
        ScalarType::U32
    } else {
        ScalarType::U64
    });
    module.functions[0].body.as_mut().unwrap().blocks[0].terminator =
        Some(switch(typed, cases, target, default));
    module
}

#[test]
fn both_switch_kinds_preserve_case_and_default_only_cycles() {
    for typed in [false, true] {
        for (cases, target, default) in [
            (2, BlockId(0), BlockId(1)),
            (2, BlockId(1), BlockId(0)),
            (0, BlockId(1), BlockId(0)),
        ] {
            let module = switched(typed, cases, target, default);
            VerifiedCanonicalKernelIrV11::from_module(module.clone()).unwrap();
            let selected = topology::select(&module).unwrap();
            assert_eq!(selected.call, [0, 0, 0]);
            assert_eq!(selected.cycle, vec![0]);
        }
    }
}

#[test]
fn switch_cases_and_default_must_target_real_blocks_and_an_actual_cycle() {
    for typed in [false, true] {
        for (target, default, message) in [
            (
                BlockId(99),
                BlockId(1),
                "source topology: missing successor",
            ),
            (
                BlockId(0),
                BlockId(99),
                "source topology: missing successor",
            ),
            (
                BlockId(1),
                BlockId(1),
                "source topology: actual cyclic retained helper call unavailable",
            ),
        ] {
            assert_eq!(
                topology::select(&switched(typed, 2, target, default)).unwrap_err(),
                message
            );
        }
        let mut unreachable = switched(typed, 2, BlockId(0), BlockId(1));
        let mut entry = BasicBlock::new(BlockId(2));
        entry.terminator = Some(Terminator::Branch {
            target: BlockId(1),
            arguments: vec![],
        });
        unreachable.functions[0]
            .body
            .as_mut()
            .unwrap()
            .blocks
            .insert(0, entry);
        assert_eq!(
            topology::select(&unreachable).unwrap_err(),
            "source topology: actual cyclic retained helper call unavailable"
        );
    }
}

#[test]
fn switch_case_and_cumulative_duplicate_edge_caps_are_exact() {
    for typed in [false, true] {
        assert!(topology::select(&switched(typed, 64, BlockId(0), BlockId(1))).is_ok());
        assert_eq!(
            topology::select(&switched(typed, 65, BlockId(0), BlockId(1))).unwrap_err(),
            "source topology: switch case cap"
        );
        let mut module = switched(typed, 63, BlockId(0), BlockId(1));
        let blocks = &mut module.functions[0].body.as_mut().unwrap().blocks;
        for id in 2..=4 {
            let mut block = BasicBlock::new(BlockId(id));
            block.terminator = Some(switch(typed, 63, BlockId(0), BlockId(1)));
            blocks.push(block);
        }
        // Four switches each have 63 cases plus default: 256 real edges.
        assert!(topology::select(&module).is_ok());
        module.functions[0].body.as_mut().unwrap().blocks[0].terminator =
            Some(switch(typed, 64, BlockId(0), BlockId(1)));
        assert_eq!(
            topology::select(&module).unwrap_err(),
            "source topology: CFG edge cap"
        );
    }
}

#[test]
fn absent_terminator_is_not_treated_as_a_return() {
    let mut module = graph();
    module.functions[0].body.as_mut().unwrap().blocks[0].terminator = None;
    assert_eq!(
        topology::select(&module).unwrap_err(),
        "source topology: missing fixture terminator"
    );
}
