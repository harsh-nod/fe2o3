//! Independent fixture-only census for the selected-edge cache work change.
//! This does not call transition State, control-index, or budget helpers.
use fe2o3_kernel_ir::{BlockId, FunctionBody, Module, OperationKind, Terminator, ValueId};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug)]
pub(super) enum Family {
    Licm,
    Preheaders,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Census {
    blocks: i64,
    conditionals: i64,
    switches: i64,
    conditional_phi_arguments: i64,
    rounds: i64,
}

impl Census {
    fn state_delta(self) -> i64 {
        let Self {
            blocks: b,
            conditionals: c,
            switches: s,
            conditional_phi_arguments: p,
            rounds: r,
        } = self;
        // New: B initialized slots, R refreshes of B blocks and C+S roots.
        // Old: 2C reachability roots and P phi roots each round, then 2C
        // final-control roots. Every selector in these fixtures has one root.
        b + r * (b + c + s) - r * (2 * c + p) - 2 * c
    }

    fn control_index_delta(self) -> i64 {
        // Index construction also retires C selected-successor queries and
        // 2C final executable-edge queries, each with one selector-root visit.
        self.state_delta() - 3 * self.conditionals
    }

    pub(super) fn complete_replay_delta(self) -> usize {
        // B/C ordinary check + B/C control index + decoded P3 B/C receipt
        // check + P6 O/I ordinary check. All endpoints have the census below.
        usize::try_from(3 * self.state_delta() + self.control_index_delta()).unwrap()
    }

    pub(super) fn map_refused_replay_delta(self) -> usize {
        // Same prefix, but the P6 Map Storage denial precedes the O/I check.
        usize::try_from(2 * self.state_delta() + self.control_index_delta()).unwrap()
    }
}

pub(super) fn expected(family: Family, mutation: bool) -> Census {
    let (blocks, conditionals, switches, conditional_phi_arguments, rounds) =
        match (family, mutation) {
            (Family::Preheaders, false) => (1, 0, 0, 0, 1),
            (Family::Licm, false) => (2, 1, 0, 0, 1),
            (Family::Preheaders, true) => (5, 1, 1, 1, 2),
            (Family::Licm, true) => (6, 2, 1, 4, 3),
        };
    Census {
        blocks,
        conditionals,
        switches,
        conditional_phi_arguments,
        rounds,
    }
}

fn edges(terminator: &Terminator) -> Vec<(BlockId, &[ValueId])> {
    match terminator {
        Terminator::Branch { target, arguments } => vec![(*target, arguments)],
        Terminator::ConditionalBranch {
            then_target,
            then_arguments,
            else_target,
            else_arguments,
            ..
        } => vec![
            (*then_target, then_arguments),
            (*else_target, else_arguments),
        ],
        Terminator::Switch {
            cases,
            default_target,
            default_arguments,
            ..
        } => cases
            .iter()
            .map(|case| (case.target, case.arguments.as_slice()))
            .chain([(*default_target, default_arguments.as_slice())])
            .collect(),
        Terminator::Return { .. } => Vec::new(),
        other => panic!("fixture schedule does not cover {other:?}"),
    }
}

fn representative(parents: &BTreeMap<ValueId, ValueId>, mut value: ValueId) -> ValueId {
    for _ in 0..=parents.len() {
        let next = parents[&value];
        if next == value {
            return value;
        }
        value = next;
    }
    panic!("fixture equivalence cycle")
}

// Independent equality saturation over the actual edge payloads. There is no
// scalar folding in these fixture selectors. Constants are seeded before the
// production rounds; repeated constant expressions cannot extend the phi
// schedule. A loop-header counter has incompatible 0/1/increment inputs.
// Preheaders aliases the latch counter in round 1, then stabilizes in round 2.
// LICM additionally transports the dynamic root through the backedge: round 1
// joins the downstream copies, round 2 grounds the header, round 3 is stable.
fn phi_rounds(body: &FunctionBody) -> usize {
    let mut parents = BTreeMap::new();
    for value in body
        .parameters
        .iter()
        .copied()
        .chain(body.blocks.iter().flat_map(|block| {
            block.parameters.iter().map(|value| value.id).chain(
                block
                    .operations
                    .iter()
                    .flat_map(|op| op.results.iter().map(|value| value.id)),
            )
        }))
    {
        parents.insert(value, value);
    }
    for round in 1..=parents.len() + 1 {
        let mut changed = false;
        for block in body.blocks.iter().skip(1) {
            for (ordinal, parameter) in block.parameters.iter().enumerate() {
                let current = representative(&parents, parameter.id);
                let mut candidates = BTreeSet::new();
                for predecessor in &body.blocks {
                    for (target, arguments) in edges(predecessor.terminator.as_ref().unwrap()) {
                        if target == block.id {
                            assert_eq!(arguments.len(), block.parameters.len());
                            let incoming = representative(&parents, arguments[ordinal]);
                            if incoming != current {
                                candidates.insert(incoming);
                            }
                        }
                    }
                }
                if candidates.len() == 1 {
                    let incoming = *candidates.first().unwrap();
                    parents.insert(current.max(incoming), current.min(incoming));
                    changed = true;
                }
            }
        }
        if !changed {
            return round;
        }
    }
    panic!("fixture phi equations did not converge")
}

fn census(module: &Module) -> Census {
    assert_eq!(module.functions.len(), 1);
    let body = module.functions[0].body.as_ref().unwrap();
    let mut result = Census {
        blocks: body.blocks.len().try_into().unwrap(),
        conditionals: 0,
        switches: 0,
        conditional_phi_arguments: 0,
        rounds: phi_rounds(body).try_into().unwrap(),
    };
    let mut reachable = BTreeSet::from([body.blocks[0].id]);
    loop {
        let before = reachable.len();
        for block in &body.blocks {
            if reachable.contains(&block.id) {
                for (target, _) in edges(block.terminator.as_ref().unwrap()) {
                    reachable.insert(target);
                }
            }
        }
        if reachable.len() == before {
            break;
        }
    }
    assert_eq!(reachable.len(), body.blocks.len());
    for block in &body.blocks {
        let terminator = block.terminator.as_ref().unwrap();
        let selector = match terminator {
            Terminator::ConditionalBranch { condition, .. } => {
                result.conditionals += 1;
                for (target, arguments) in edges(terminator) {
                    let target = body.blocks.iter().find(|block| block.id == target).unwrap();
                    assert_eq!(arguments.len(), target.parameters.len());
                    result.conditional_phi_arguments +=
                        i64::try_from(target.parameters.len()).unwrap();
                }
                Some(*condition)
            }
            Terminator::Switch {
                selector, cases, ..
            } => {
                assert!(!cases.is_empty());
                result.switches += 1;
                Some(*selector)
            }
            _ => None,
        };
        if let Some(selector) = selector {
            // Selectors are original dynamic function parameters or their
            // directly defining nonconstant operation, never phi aliases.
            assert!(
                !body
                    .blocks
                    .iter()
                    .any(|block| block.parameters.iter().any(|p| p.id == selector))
            );
            if !body.parameters.contains(&selector) {
                let operation = body
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .find(|op| op.results.iter().any(|result| result.id == selector))
                    .unwrap();
                assert!(!matches!(operation.kind, OperationKind::Constant(_)));
            }
        }
    }
    result
}

pub(super) fn assert_policy6_endpoints(
    owner: &crate::ProductionCheckedOutputOwnerPolicy6V1,
    family: Family,
    mutation: bool,
) {
    let fifth = owner.checked_output().intermediate_policy5();
    let third = fifth.intermediate_policy4().intermediate_policy3();
    for (name, module) in [
        ("B", owner.bound().module()),
        ("C", third.owner().module()),
        ("O", fifth.owner().module()),
        ("I", owner.output().module()),
    ] {
        assert_eq!(
            census(module),
            expected(family, mutation),
            "{family:?}/{mutation}/{name}"
        );
    }
}

#[test]
fn transition_cache_fixture_work_equations_are_literal_and_phase_sensitive() {
    for (family, mutation, d, index, complete, partial) in [
        (Family::Preheaders, false, 2, 2, 8, 6),
        (Family::Licm, false, 1, -2, 1, 0),
        (Family::Preheaders, true, 11, 8, 41, 30),
        (Family::Licm, true, 5, -1, 14, 9),
    ] {
        let census = expected(family, mutation);
        assert_eq!(census.state_delta(), d);
        assert_eq!(census.control_index_delta(), index);
        assert_eq!(census.complete_replay_delta(), complete);
        assert_eq!(census.map_refused_replay_delta(), partial);
    }
    assert_eq!(249_631 + 3 * 1, 249_634);
    assert_eq!(238_296 + 2 * 1 + 0, 238_298);
    assert_eq!(2_733_569 + 3 * 14, 2_733_611);
    assert_eq!(2_686_896 + 2 * 14 + 9, 2_686_933);
    assert_eq!(65_552 + 8 + 6, 65_566);
    assert_eq!(845_738 + 41 + 30, 845_809);
}

#[test]
fn transition_cache_fixture_rounds_follow_phi_dependency_order_not_block_count() {
    use fe2o3_kernel_ir::{BasicBlock, ScalarType, Type, ValueDef};
    let mut entry = BasicBlock::new(BlockId(0));
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(0)],
    });
    let mut header = BasicBlock::new(BlockId(1));
    header
        .parameters
        .push(ValueDef::new(ValueId(1), Type::Scalar(ScalarType::U32)));
    header.terminator = Some(Terminator::Branch {
        target: BlockId(2),
        arguments: vec![ValueId(1)],
    });
    let mut latch = BasicBlock::new(BlockId(2));
    latch
        .parameters
        .push(ValueDef::new(ValueId(2), Type::Scalar(ScalarType::U32)));
    latch.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(2)],
    });
    // This is only a control for the independent census, not an admitted owner.
    let mut body = FunctionBody {
        parameters: vec![ValueId(0)],
        blocks: vec![entry, header, latch],
    };
    assert_eq!(phi_rounds(&body), 3);
    body.blocks.swap(1, 2);
    assert_eq!(phi_rounds(&body), 2);
}
