//! Independent path semantics for the production control reconstruction planner.
use super::*;
use crate::mixed_optimizer_refinement_v26::Budget;
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work, Function as IrFunction, Module,
    Signature, StorageLayoutLimitsV1, SwitchCase, ValueDef,
    VerifiedCanonicalKernelIrModuleV18 as Owner,
};

const LIMIT: usize = 100_000_000;
const VALUES: [ValueId; 3] = [ValueId(11), ValueId(29), ValueId(47)];
const CONDITIONS: [ValueId; 3] = [ValueId(103), ValueId(107), ValueId(109)];
const PHI: ValueId = ValueId(211);
const LAYOUTS: StorageLayoutLimitsV1 = StorageLayoutLimitsV1 {
    rows: 64,
    edges: 256,
    containment_depth: 32,
    object_bytes: 4096,
};

#[derive(Clone, Copy, Debug)]
enum Case {
    Diamond,
    LinearArms,
    ShortCircuit,
    ShortCircuitAnd,
    Nested,
    SharedDag,
    ParallelPayloads,
    EqualPayloads,
    SelfCycle,
    TwoNodeCycle,
    SideReturn,
    SideUnreachable,
    UnsupportedSwitch,
    ExternalArmPredecessor,
    ExternalPhiPredecessor,
}

fn jump(target: BlockId, value: Option<ValueId>) -> Terminator {
    Terminator::Branch {
        target,
        arguments: value.into_iter().collect(),
    }
}

fn choose(
    condition: usize,
    on_true: (BlockId, Option<ValueId>),
    on_false: (BlockId, Option<ValueId>),
) -> Terminator {
    Terminator::ConditionalBranch {
        condition: CONDITIONS[condition],
        then_target: on_true.0,
        then_arguments: on_true.1.into_iter().collect(),
        else_target: on_false.0,
        else_arguments: on_false.1.into_iter().collect(),
    }
}

fn block(id: BlockId, terminator: Terminator) -> BasicBlock {
    let mut result = BasicBlock::new(id);
    result.terminator = Some(terminator);
    result
}

fn fixture(case: Case, permuted: bool) -> Vec<BasicBlock> {
    let ids = if permuted {
        [800, 44, 990, 77, 15, 602, 3, 405, 901]
    } else {
        [91, 7, 303, 28, 602, 13, 400, 85, 1001]
    }
    .map(BlockId);
    let [
        entry,
        left,
        right,
        inner,
        merge,
        extra,
        shared,
        alternate,
        external,
    ] = ids;
    let leaf = |id, value| block(id, jump(merge, Some(VALUES[value])));
    let mut rows = match case {
        Case::Diamond
        | Case::EqualPayloads
        | Case::ExternalArmPredecessor
        | Case::ExternalPhiPredecessor => vec![
            block(entry, choose(0, (left, None), (right, None))),
            leaf(left, 0),
            leaf(
                right,
                if matches!(case, Case::EqualPayloads) {
                    0
                } else {
                    1
                },
            ),
        ],
        Case::LinearArms => vec![
            block(entry, choose(0, (inner, None), (extra, None))),
            block(inner, jump(left, None)),
            block(extra, jump(right, None)),
            leaf(left, 0),
            leaf(right, 1),
        ],
        Case::ShortCircuit => vec![
            block(entry, choose(0, (left, None), (inner, None))),
            block(inner, choose(1, (left, None), (right, None))),
            leaf(left, 0),
            leaf(right, 1),
        ],
        Case::ShortCircuitAnd => vec![
            block(entry, choose(0, (inner, None), (right, None))),
            block(inner, choose(1, (left, None), (right, None))),
            leaf(left, 0),
            leaf(right, 1),
        ],
        Case::Nested => vec![
            block(entry, choose(0, (inner, None), (extra, None))),
            block(inner, choose(1, (left, None), (right, None))),
            leaf(left, 0),
            leaf(right, 1),
            leaf(extra, 2),
        ],
        Case::SharedDag => vec![
            block(entry, choose(0, (inner, None), (alternate, None))),
            block(inner, choose(1, (shared, None), (extra, None))),
            block(alternate, jump(shared, None)),
            block(shared, choose(2, (left, None), (right, None))),
            leaf(left, 0),
            leaf(right, 1),
            leaf(extra, 2),
        ],
        Case::ParallelPayloads => vec![block(
            entry,
            choose(0, (merge, Some(VALUES[0])), (merge, Some(VALUES[1]))),
        )],
        Case::SelfCycle => vec![
            block(entry, choose(0, (inner, None), (right, None))),
            block(inner, choose(1, (inner, None), (left, None))),
            leaf(left, 0),
            leaf(right, 1),
        ],
        Case::TwoNodeCycle => vec![
            block(entry, choose(0, (inner, None), (right, None))),
            block(inner, jump(extra, None)),
            block(extra, choose(1, (inner, None), (left, None))),
            leaf(left, 0),
            leaf(right, 1),
        ],
        Case::SideReturn | Case::SideUnreachable => vec![
            block(entry, choose(0, (inner, None), (right, None))),
            block(inner, choose(1, (left, None), (extra, None))),
            leaf(left, 0),
            leaf(right, 1),
            block(
                extra,
                if matches!(case, Case::SideReturn) {
                    Terminator::Return {
                        values: vec![VALUES[2]],
                    }
                } else {
                    Terminator::Unreachable
                },
            ),
        ],
        Case::UnsupportedSwitch => vec![
            block(
                entry,
                Terminator::Switch {
                    selector: VALUES[2],
                    cases: vec![SwitchCase {
                        value: 0,
                        target: left,
                        arguments: vec![],
                    }],
                    default_target: right,
                    default_arguments: vec![],
                },
            ),
            leaf(left, 0),
            leaf(right, 1),
        ],
    };
    let mut join = block(merge, Terminator::Return { values: vec![PHI] });
    join.parameters
        .push(ValueDef::new(PHI, Type::Scalar(ScalarType::U32)));
    rows.push(join);
    match case {
        Case::ExternalArmPredecessor => rows.push(block(external, jump(left, None))),
        Case::ExternalPhiPredecessor => rows.push(leaf(external, 2)),
        _ => (),
    }
    if permuted {
        rows[1..].reverse();
    }
    rows
}

fn module(blocks: &[BasicBlock]) -> Module {
    let mut result = Module::new("control-reconstruction-path-oracle");
    result.functions.push(IrFunction::internal_helper(
        "entry",
        Signature::new(
            vec![
                Type::Scalar(ScalarType::U32),
                Type::Scalar(ScalarType::U32),
                Type::Scalar(ScalarType::U32),
                Type::Scalar(ScalarType::Bool),
                Type::Scalar(ScalarType::Bool),
                Type::Scalar(ScalarType::Bool),
            ],
            vec![Type::Scalar(ScalarType::U32)],
        ),
        VALUES.into_iter().chain(CONDITIONS).collect(),
        blocks.to_vec(),
    ));
    result
}

fn scalar(value: ValueId) -> u32 {
    let ordinal = VALUES
        .iter()
        .position(|&candidate| candidate == value)
        .expect("original U32 argument");
    [17, 31, 53][ordinal]
}

fn boolean(value: ValueId, assignment: [bool; 3]) -> bool {
    assignment[CONDITIONS
        .iter()
        .position(|&candidate| candidate == value)
        .expect("original Bool argument")]
}

fn original_path(blocks: &[BasicBlock], assignment: [bool; 3]) -> u32 {
    let mut current = blocks[0].id;
    let mut incoming = None;
    for _ in 0..=blocks.len() {
        let row = blocks.iter().find(|row| row.id == current).unwrap();
        if !row.parameters.is_empty() {
            assert_eq!(
                row.parameters,
                [ValueDef::new(PHI, Type::Scalar(ScalarType::U32))]
            );
            assert!(
                matches!(&row.terminator, Some(Terminator::Return { values }) if values == &[PHI])
            );
            return incoming.expect("exact selected phi-edge payload");
        }
        let (target, arguments) = match row.terminator.as_ref().unwrap() {
            Terminator::Branch { target, arguments } => (*target, arguments),
            Terminator::ConditionalBranch {
                condition,
                then_target,
                then_arguments,
                else_target,
                else_arguments,
            } => {
                if boolean(*condition, assignment) {
                    (*then_target, then_arguments)
                } else {
                    (*else_target, else_arguments)
                }
            }
            other => panic!("positive path terminator: {other:?}"),
        };
        assert!(arguments.len() <= 1);
        incoming = arguments.first().copied().map(scalar);
        current = target;
    }
    panic!("positive source path must be acyclic");
}

fn planned_value(
    input: &Inventory<'_>,
    root: &Term,
    controls: &[Select],
    assignment: [bool; 3],
) -> u32 {
    let mut term = root;
    for _ in 0..=controls.len() {
        match term {
            Term::Original(index) => return scalar(input.definitions()[*index].value.unwrap()),
            Term::Control(index) => {
                let row = &controls[*index];
                term = if boolean(
                    input.definitions()[row.condition].value.unwrap(),
                    assignment,
                ) {
                    &row.on_true
                } else {
                    &row.on_false
                };
            }
        }
    }
    panic!("planned expression must be an acyclic control arena");
}

fn check_terms(input: &Inventory<'_>, root: &Term, controls: &[Select]) {
    let check = |term: &Term, bound: usize| match term {
        Term::Original(index) => {
            let row = &input.definitions()[*index];
            assert_eq!(row.ty, &Type::Scalar(ScalarType::U32));
            assert!(VALUES.contains(&row.value.unwrap()));
            assert_ne!(*index, row.value.unwrap().0 as usize);
        }
        Term::Control(index) => assert!(*index < bound, "controls must refer to earlier controls"),
    };
    check(root, controls.len());
    for (index, row) in controls.iter().enumerate() {
        let condition = &input.definitions()[row.condition];
        assert_eq!(condition.ty, &Type::Scalar(ScalarType::Bool));
        assert!(CONDITIONS.contains(&condition.value.unwrap()));
        check(&row.on_true, index);
        check(&row.on_false, index);
    }
}

fn expected(case: Case, bits: [bool; 3]) -> u32 {
    let ordinal = match case {
        Case::Diamond | Case::LinearArms | Case::ParallelPayloads => {
            if bits[0] {
                0
            } else {
                1
            }
        }
        Case::ShortCircuit => {
            if bits[0] || bits[1] {
                0
            } else {
                1
            }
        }
        Case::ShortCircuitAnd => {
            if bits[0] && bits[1] {
                0
            } else {
                1
            }
        }
        Case::Nested => {
            if !bits[0] {
                2
            } else if bits[1] {
                0
            } else {
                1
            }
        }
        Case::SharedDag => {
            if bits[0] && !bits[1] {
                2
            } else if bits[2] {
                0
            } else {
                1
            }
        }
        Case::EqualPayloads => 0,
        _ => panic!("not a positive truth table"),
    };
    scalar(VALUES[ordinal])
}

fn inspect_positive(case: Case, permuted: bool, shared: bool) {
    let blocks = fixture(case, permuted);
    let module = module(&blocks);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let (owner, receipt) =
        Owner::from_module_ref_with_verification_budget_v18(&module, LAYOUTS, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let (input, receipt) = Inventory::derive_v18(&owner, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let original = input
        .definitions()
        .iter()
        .position(|row| row.value == Some(PHI))
        .unwrap();
    let mut out = Writer::new(&mut budget).unwrap();
    let floor = out.budget.storage();
    let plan = derive(&input, original, &mut out).unwrap();
    check_terms(&input, &plan.root, &plan.controls);
    for mask in 0..8 {
        let assignment = std::array::from_fn(|index| mask & (1 << index) != 0);
        let path = original_path(&blocks, assignment);
        assert_eq!(
            path,
            expected(case, assignment),
            "{case:?}/{permuted}/{mask}"
        );
        assert_eq!(
            planned_value(&input, &plan.root, &plan.controls, assignment),
            path,
            "{case:?}/{permuted}/{mask}"
        );
    }
    if shared {
        let nodes: Vec<_> = plan
            .controls
            .iter()
            .enumerate()
            .filter(|(_, row)| input.definitions()[row.condition].value == Some(CONDITIONS[2]))
            .collect();
        assert_eq!(nodes.len(), 1, "shared black node must be memoized once");
        let shared_index = nodes[0].0;
        let references = plan
            .controls
            .iter()
            .flat_map(|row| [&row.on_true, &row.on_false])
            .filter(|term| matches!(term, Term::Control(index) if *index == shared_index))
            .count();
        assert_eq!(
            references, 2,
            "both original paths share the same control node"
        );
    }
    plan.discard(&mut out).unwrap();
    assert_eq!(out.budget.storage(), floor);
    assert!(out.finish().unwrap().is_empty());
}

#[test]
fn control_reconstruction_matches_original_paths_and_truth_tables() {
    for case in [
        Case::Diamond,
        Case::LinearArms,
        Case::ShortCircuit,
        Case::ShortCircuitAnd,
        Case::Nested,
        Case::EqualPayloads,
    ] {
        for permuted in [false, true] {
            inspect_positive(case, permuted, false);
        }
    }
}

#[test]
fn control_reconstruction_memoizes_shared_dag_without_confusing_value_ids() {
    for permuted in [false, true] {
        inspect_positive(Case::SharedDag, permuted, true);
    }
}

#[test]
fn control_reconstruction_preserves_parallel_successor_payloads() {
    for permuted in [false, true] {
        inspect_positive(Case::ParallelPayloads, permuted, false);
    }
}

fn inspect_refusal(case: Case, permuted: bool) {
    let module = module(&fixture(case, permuted));
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let (owner, receipt) =
        Owner::from_module_ref_with_verification_budget_v18(&module, LAYOUTS, &mut budget)
            .expect("negative fixture must pass canonical verification before planner refusal");
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let (input, receipt) = Inventory::derive_v18(&owner, &mut budget)
        .expect("negative fixture must retain its complete inventory before planner refusal");
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let original = input
        .definitions()
        .iter()
        .position(|row| row.value == Some(PHI))
        .unwrap();
    let mut out = Writer::new(&mut budget).unwrap();
    let floor = out.budget.storage();
    let error = match derive(&input, original, &mut out) {
        Ok(plan) => {
            plan.discard(&mut out).unwrap();
            panic!("{case:?}/{permuted} must not produce a partial region expression");
        }
        Err(error) => error,
    };
    let Error::SourceReconstruction { facts, .. } = error else {
        panic!("expected a structural refusal, not a resource or unrelated error: {error:?}");
    };
    assert_eq!(facts.original, original);
    assert_eq!(facts.coordinate, input.definitions()[original].coordinate);
    assert_eq!(facts.phase, "cfg-region");
    assert!(
        out.budget.storage() >= floor,
        "failed scratch must not refund owner storage"
    );
    assert!(out.finish().unwrap().is_empty());
}

#[test]
fn control_reconstruction_rejects_cycles_side_exits_and_unsupported_terminators() {
    for case in [
        Case::SelfCycle,
        Case::TwoNodeCycle,
        Case::SideReturn,
        Case::SideUnreachable,
        Case::UnsupportedSwitch,
    ] {
        for permuted in [false, true] {
            inspect_refusal(case, permuted);
        }
    }
}

#[test]
fn control_reconstruction_rejects_unaccounted_external_predecessors() {
    for case in [Case::ExternalArmPredecessor, Case::ExternalPhiPredecessor] {
        for permuted in [false, true] {
            inspect_refusal(case, permuted);
        }
    }
}
