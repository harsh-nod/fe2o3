//! Synthetic pure controls. Not retained-source or runtime-origin evidence.
use super::*;
use collector::{Capture, Row};
use fe2o3_kernel_ir::*;

#[path = "topology_switch_tests.rs"]
mod topology_switch_tests;

fn one(id: u32, kind: OperationKind) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), Type::Scalar(ScalarType::U32)),
        kind,
    )
}

fn graph() -> Module {
    let u32_ty = Type::Scalar(ScalarType::U32);
    let mut body = BasicBlock::new(BlockId(0));
    body.operations.push(one(
        3,
        OperationKind::Call {
            callee: "mix".into(),
            arguments: vec![ValueId(0), ValueId(1)],
        },
    ));
    body.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(0),
        then_arguments: vec![],
        else_target: BlockId(1),
        else_arguments: vec![],
    });
    let mut end = BasicBlock::new(BlockId(1));
    end.terminator = Some(Terminator::Return { values: vec![] });
    let entry = Function::kernel_entry(
        "entry",
        Signature::new(vec![u32_ty.clone(), u32_ty.clone(), Type::BOOL], vec![]),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![body, end],
    );
    let mut helper = BasicBlock::new(BlockId(0));
    helper.operations = vec![
        one(2, OperationKind::Constant(Constant::U32(0xffff))),
        one(
            3,
            OperationKind::Binary {
                op: BinaryOp::BitXor,
                lhs: ValueId(0),
                rhs: ValueId(1),
            },
        ),
        one(
            4,
            OperationKind::Binary {
                op: BinaryOp::BitAnd,
                lhs: ValueId(3),
                rhs: ValueId(2),
            },
        ),
    ];
    helper.terminator = Some(Terminator::Return {
        values: vec![ValueId(4)],
    });
    let helper = Function::internal_helper(
        "mix",
        Signature::new(vec![u32_ty.clone(), u32_ty.clone()], vec![u32_ty]),
        vec![ValueId(0), ValueId(1)],
        vec![helper],
    );
    let mut module = Module::new("synthetic-topology-control");
    module.functions = vec![entry, helper];
    module.kernels.push(Kernel::new(
        "loop_helper",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}

#[test]
fn structural_positive_requires_an_actual_reachable_cycle_and_retained_pure_call() {
    let module = graph();
    VerifiedCanonicalKernelIrV11::from_module(module.clone()).unwrap();
    let selected = topology::select(&module).unwrap();
    assert_eq!(selected.call, [0, 0, 0]);
    assert_eq!(selected.cycle, vec![0]);
    assert_eq!(selected.helper, 1);
    assert_eq!(selected.helper_sites.len(), 3);
}

#[test]
fn nonpositional_block_ids_remain_distinct_from_authoring_roster_coordinates() {
    let mut module = graph();
    let blocks = &mut module.functions[0].body.as_mut().unwrap().blocks;
    blocks[0].id = BlockId(17);
    blocks[1].id = BlockId(42);
    blocks[0].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(17),
        then_arguments: vec![],
        else_target: BlockId(42),
        else_arguments: vec![],
    });
    module.functions[1].body.as_mut().unwrap().blocks[0].id = BlockId(99);
    VerifiedCanonicalKernelIrV11::from_module(module.clone()).unwrap();
    let selected = topology::select(&module).unwrap();
    assert_eq!(selected.call, [0, 17, 0]);
    assert_eq!(selected.call_authoring_coordinate, [0, 0, 0]);
    assert_eq!(selected.cycle, [17]);
    assert_eq!(selected.helper_sites, [[1, 99, 0], [1, 99, 1], [1, 99, 2]]);
    assert_eq!(
        selected.helper_authoring_coordinates,
        [[1, 0, 0], [1, 0, 1], [1, 0, 2]]
    );
    let projected = selected.json();
    assert_eq!(projected["call"], json!([0, 17, 0]));
    assert_eq!(projected["call_authoring_coordinate"], json!([0, 0, 0]));
}

#[test]
fn structural_negatives_do_not_accept_source_syntax_or_names_in_place_of_topology() {
    let mut no_loop = graph();
    no_loop.functions[0].body.as_mut().unwrap().blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![],
    });
    assert!(topology::select(&no_loop).is_err());
    let mut no_call = graph();
    no_call.functions[0].body.as_mut().unwrap().blocks[0].operations =
        vec![one(3, OperationKind::Constant(Constant::U32(0)))];
    assert!(topology::select(&no_call).is_err());
    let mut wrong_signature = graph();
    wrong_signature.functions[1].signature.results = vec![Type::INDEX];
    assert!(topology::select(&wrong_signature).is_err());
    let mut impure = graph();
    impure.functions[1].body.as_mut().unwrap().blocks[0]
        .operations
        .push(one(
            5,
            OperationKind::Call {
                callee: "mix".into(),
                arguments: vec![ValueId(0), ValueId(1)],
            },
        ));
    assert!(topology::select(&impure).is_err());
}

#[test]
fn structural_graph_limits_and_ambiguous_calls_fail_closed() {
    let mut ambiguous = graph();
    ambiguous.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .push(one(
            4,
            OperationKind::Call {
                callee: "mix".into(),
                arguments: vec![ValueId(0), ValueId(1)],
            },
        ));
    assert!(topology::select(&ambiguous).is_err());
    let mut large = graph();
    large.functions[0]
        .body
        .as_mut()
        .unwrap()
        .blocks
        .resize(65, BasicBlock::new(BlockId(2)));
    assert!(topology::select(&large).is_err());
}

fn rows(trips: u32) -> (Capture, topology::Topology) {
    let topology = topology::select(&graph()).unwrap();
    let mut capture = Capture::new(true).unwrap();
    for lane in 0..4 {
        let mut push = |site, kind, activation, attempt, offset, bits| {
            capture.rows.push(Row {
                ordinal: capture.rows.len() as u64,
                decision: lane as u64,
                lane,
                site,
                kind,
                activation,
                attempt,
                allocation: if kind == 2 { 7 } else { 0 },
                offset,
                bits,
            });
        };
        for trip in 0..trips {
            push(topology.call, 0, 1, u64::from(trip) + 1, 0, 0);
            for (index, site) in topology.helper_sites.iter().enumerate() {
                push(*site, 0, u64::from(trip) + 2, index as u64 + 1, 0, 0);
                push(*site, 1, u64::from(trip) + 2, index as u64 + 1, 0, 0);
            }
            push(topology.call, 1, 1, u64::from(trip) + 1, 0, 0);
        }
        let site = [0, 1, 0];
        push(site, 0, 1, u64::from(trips) + 1, 0, 0);
        push(
            site,
            2,
            1,
            u64::from(trips) + 1,
            4 + 4 * u64::from(lane),
            oracle(trips),
        );
        push(site, 1, 1, u64::from(trips) + 1, 0, 0);
    }
    (capture, topology)
}

#[test]
fn compact_controls_accept_zero_single_and_repeated_attempts() {
    for trips in [0, 1, 3] {
        let (capture, topology) = rows(trips);
        let report = capture.validate(&topology, trips, oracle(trips)).unwrap();
        assert_eq!(report["helper_activations"], json!(trips * 4));
    }
}

#[test]
fn compact_controls_reject_changed_sites_reused_tokens_missing_pairs_and_extra_writes() {
    let mutations: &[fn(&mut Vec<Row>)] = &[
        |rows| rows[1].site[1] = 99,
        |rows| rows[1].activation = 1,
        |rows| rows[2].attempt = 2,
        |rows| rows[1].lane = 4,
        |rows| {
            let last = rows.len() - 1;
            rows.remove(last);
        },
        |rows| {
            let row = rows.iter_mut().find(|r| r.kind == 2).unwrap();
            row.offset = 0;
        },
        |rows| {
            let row = rows.iter_mut().find(|r| r.kind == 2).unwrap();
            row.bits ^= 1;
        },
    ];
    for mutate in mutations {
        let (mut capture, topology) = rows(1);
        mutate(&mut capture.rows);
        assert!(capture.validate(&topology, 1, oracle(1)).is_err());
    }
    let (mut capture, topology) = rows(3);
    let prior = capture.rows[0];
    capture.rows.resize(collector::MAX_ROWS + 1, prior);
    assert!(capture.validate(&topology, 3, oracle(3)).is_err());
}

#[test]
fn compact_legacy_comparison_ignores_only_origin_tokens() {
    let (capture, _) = rows(1);
    let mut legacy = Capture::new(false).unwrap();
    legacy.rows = capture.rows.clone();
    for row in &mut legacy.rows {
        row.activation = 0;
        row.attempt = 0;
    }
    capture.same_legacy(&legacy).unwrap();
    legacy.rows[0].decision += 1;
    assert!(capture.same_legacy(&legacy).is_err());
}

#[test]
fn independent_output_oracle_and_both_canaries_are_checked() {
    assert_eq!(
        [oracle(0), oracle(1), oracle(3)],
        [0xabcd1234, 0x479e, 0x479d]
    );
    for index in 0..6 {
        let mut values = [0xdeadbeef, 0x479e, 0x479e, 0x479e, 0x479e, 0xcafebabe];
        let good = BufferArgumentV1::from_scalars(
            AccessMode::ReadWrite,
            4,
            &values.map(ScalarBitsV1::u32),
            TARGET,
        )
        .unwrap();
        check_buffer(&good, 0x479e).unwrap();
        values[index] ^= 1;
        let bad = BufferArgumentV1::from_scalars(
            AccessMode::ReadWrite,
            4,
            &values.map(ScalarBitsV1::u32),
            TARGET,
        )
        .unwrap();
        assert!(check_buffer(&bad, 0x479e).is_err());
    }
}

#[test]
fn observer_cli_requires_one_absolute_input_and_exact_lowercase_hash() {
    assert!(arguments(vec!["/tmp/input".into(), "ab".repeat(32)]).is_ok());
    for args in [
        vec![],
        vec!["relative".into(), "ab".repeat(32)],
        vec!["/tmp/input".into(), "AB".repeat(32)],
        vec!["/tmp/input".into(), "ab".repeat(33)],
        vec!["/tmp/in\nput".into(), "ab".repeat(32)],
    ] {
        assert!(arguments(args).is_err());
    }
}
