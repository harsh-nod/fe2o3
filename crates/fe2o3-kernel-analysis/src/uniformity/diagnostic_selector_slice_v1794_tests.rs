use super::*;
use crate::Variation;
use fe2o3_kernel_ir::{
    AddressSpace, BinaryOp, CheckedBinaryOperator, Constant, MemoryAccess, ScalarType,
    SynchronizationScope,
};

fn scalar(id: u32) -> ValueDef {
    ValueDef::new(ValueId(id), Type::Scalar(ScalarType::U64))
}

fn constant(id: u32, value: u64) -> Operation {
    Operation::new(
        vec![scalar(id)],
        OperationKind::Constant(Constant::U64(value)),
    )
}

fn branch(condition: u32, yes: u32, no: u32) -> Terminator {
    Terminator::ConditionalBranch {
        condition: ValueId(condition),
        then_target: BlockId(yes),
        then_arguments: vec![],
        else_target: BlockId(no),
        else_arguments: vec![],
    }
}

fn dump(body: &FunctionBody, limit: usize) -> (String, bool) {
    let mut out = Trace::new(Vec::new());
    out.selector_slice_with_limit(body, None, None, limit);
    out.finish("test", 0);
    (String::from_utf8(out.writer).unwrap(), out.truncated)
}

fn cycle() -> FunctionBody {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![constant(1, 0), constant(2, 1)];
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(1)],
    });
    let mut header = BasicBlock::new(BlockId(1));
    header.parameters = vec![scalar(3)];
    header.operations = vec![Operation::new(
        vec![scalar(4), ValueDef::new(ValueId(5), Type::BOOL)],
        OperationKind::Binary {
            op: BinaryOp::Checked(CheckedBinaryOperator::Add),
            lhs: ValueId(3),
            rhs: ValueId(2),
        },
    )];
    header.terminator = Some(branch(5, 3, 2));
    let mut latch = BasicBlock::new(BlockId(2));
    latch.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(4)],
    });
    let mut exit = BasicBlock::new(BlockId(3));
    exit.terminator = Some(Terminator::Return { values: vec![] });
    FunctionBody {
        parameters: vec![],
        blocks: vec![entry, header, latch, exit],
    }
}

#[test]
fn selector_slice_retains_checked_result_and_all_cycle_incoming_edges() {
    let body = cycle();
    let report = AnalysisReport {
        function: "selector_cycle".into(),
        values: Default::default(),
        block_controls: [(BlockId(1), Variation::SubgroupUniform)].into(),
        diagnostics: vec![Diagnostic::DivergentBarrier {
            block: BlockId(1),
            operation_index: 0,
            execution_scope: SynchronizationScope::Workgroup,
            control: Variation::SubgroupUniform,
        }],
    };
    let before = (body.clone(), report.clone());
    let mut out = Trace::new(Vec::new());
    out.selector_slice(&body, None, Some(&report));
    out.finish("test", 0);
    assert_eq!((body, report), before);
    assert!(!out.truncated);
    assert!(!out.output_error);
    let text = String::from_utf8(out.writer).unwrap();
    assert!(text.contains("SLICE_SITE block=1 operation=0 ordinal=1 id=5"));
    assert!(text.contains("SLICE_SITE block=1 operation=0 ordinal=0 id=4"));
    assert!(text.contains("SLICE_PHI block=1 ordinal=0 id=3"));
    assert!(text.contains("SLICE_INCOMING source=0 target=1 role=branch ordinal=0"));
    assert!(text.contains("SLICE_INCOMING source=2 target=1 role=branch ordinal=0"));
    assert_eq!(
        text.lines()
            .filter(|l| l.contains("SLICE_NODE id=3 "))
            .count(),
        1
    );
    assert!(text.contains("incomplete=0 semantic_proof=0"));
}

#[test]
fn selector_slice_finds_late_definition_beyond_original_body_dump_cap() {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = (0..=MAX_OPERATIONS)
        .map(|n| constant(n as u32, n as u64))
        .collect();
    let id = block.operations.len() as u32;
    block.operations.push(Operation::new(
        vec![ValueDef::new(ValueId(id), Type::BOOL)],
        OperationKind::Constant(Constant::Bool(true)),
    ));
    block.terminator = Some(branch(id, 1, 1));
    let mut exit = BasicBlock::new(BlockId(1));
    exit.terminator = Some(Terminator::Return { values: vec![] });
    let body = FunctionBody {
        parameters: vec![],
        blocks: vec![block, exit],
    };
    let (text, truncated) = dump(&body, NODE_LIMIT);
    assert!(!truncated);
    assert!(text.contains(&format!(
        "SLICE_SITE block=0 operation={id} ordinal=0 id={id}"
    )));
    assert_eq!(
        text.lines().filter(|l| l.contains("SLICE_NODE ")).count(),
        1
    );
    assert!(text.contains("incomplete=0 semantic_proof=0"));
}

#[test]
fn selector_slice_exact_node_limit_missing_and_duplicate_are_explicit() {
    let body = cycle();
    assert!(!dump(&body, 5).1);
    let (short, truncated) = dump(&body, 4);
    assert!(truncated);
    assert!(short.contains("reason=node_limit"));
    assert!(short.contains("incomplete=1 semantic_proof=0"));
    let mut missing = body.clone();
    missing.blocks[0].operations.remove(0);
    let (text, truncated) = dump(&missing, NODE_LIMIT);
    assert!(truncated);
    assert!(text.contains("SLICE_MISSING id=1"));
    let mut duplicate = body;
    duplicate.blocks[0].operations.push(constant(1, 2));
    let (text, truncated) = dump(&duplicate, NODE_LIMIT);
    assert!(truncated);
    assert!(text.contains("SLICE_INVALID duplicate_definition=1"));
    assert!(text.contains("SLICE_AMBIGUOUS id=1"));
}

#[test]
fn selector_slice_missing_phi_argument_or_predecessor_never_looks_complete() {
    let mut body = cycle();
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![],
    });
    let (text, truncated) = dump(&body, NODE_LIMIT);
    assert!(truncated);
    assert!(text.contains("SLICE_INVALID missing_incoming_argument=1"));
    body.blocks[0].terminator = Some(Terminator::Return { values: vec![] });
    body.blocks[2].terminator = Some(Terminator::Return { values: vec![] });
    let (text, truncated) = dump(&body, NODE_LIMIT);
    assert!(truncated);
    assert!(text.contains("SLICE_MISSING incoming_target=1 ordinal=0"));
    assert!(text.contains("matched=0"));
}

#[test]
fn selector_slice_load_is_opaque_and_output_failure_does_not_mutate_input() {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![Operation::new(
        vec![ValueDef::new(ValueId(1), Type::BOOL)],
        OperationKind::Load {
            pointer: ValueId(0),
            access: MemoryAccess::new(AddressSpace::Private, 1),
        },
    )];
    block.terminator = Some(branch(1, 1, 1));
    let mut exit = BasicBlock::new(BlockId(1));
    exit.terminator = Some(Terminator::Return { values: vec![] });
    let body = FunctionBody {
        parameters: vec![ValueId(0)],
        blocks: vec![block, exit],
    };
    let (text, truncated) = dump(&body, NODE_LIMIT);
    assert!(!truncated);
    assert!(text.contains("SLICE_OPAQUE id=1 memory_or_callee_semantics_not_followed=1"));
    assert!(text.contains("opaque=1"));
    assert!(text.contains("SLICE_PARAMETER ordinal=0 id=0"));
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("diagnostic writer"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let before = body.clone();
    let mut out = Trace::new(Broken);
    out.selector_slice(&body, None, None);
    out.finish("test", 0);
    assert!(out.output_error);
    assert_eq!(body, before);
}

#[test]
fn selector_slice_index_operation_and_edge_caps_are_explicit() {
    let mut body = cycle();
    body.blocks[3].operations =
        vec![
            Operation::new(vec![], OperationKind::Constant(Constant::Bool(true)));
            OPERATION_LIMIT
        ];
    assert!(dump(&body, NODE_LIMIT).1);
    let body = cycle();
    let mut out = Trace::new(Vec::new());
    let mut slice = Slice::new(NODE_LIMIT).unwrap();
    for _ in 0..=INDEX_LIMIT {
        slice.insert(ValueId(1), Definition::Parameter(0));
    }
    assert_eq!(slice.index.len(), INDEX_LIMIT);
    assert!(slice.incomplete);
    let mut slice = Slice::new(NODE_LIMIT).unwrap();
    slice.edges = EDGE_LIMIT - 1;
    slice.incoming(&mut out, &body, BlockId(1), 0);
    assert_eq!(slice.edges, EDGE_LIMIT);
    assert!(slice.incomplete);
    assert!(String::from_utf8(out.writer).unwrap().contains("matched=1"));
}
