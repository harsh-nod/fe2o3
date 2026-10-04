//! Hostile model-only checks of the joined region's escape guard.
//! These fixtures cannot construct or substitute a genuine source/KIR owner.
use super::*;

const BOUNDARY: &str = "source-candidate boundary is not three contiguous entry operations";
const ESCAPE: &str = "source-candidate intermediate escapes the selected graph";
const LIVE: &str = "source-candidate graph is not three live-ins and one live-out";
const UNBOUND: &str = "source-candidate selected graph has an unbound live-in";

fn bitwise(op: BinaryOp, result: u32, left: u32, right: u32) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(result), Type::Scalar(ScalarType::U32)),
        OperationKind::Binary {
            op,
            lhs: ValueId(left),
            rhs: ValueId(right),
        },
    )
}

fn prefixed(count: usize) -> FunctionBody {
    assert!((1..=8).contains(&count));
    let mut body = graph();
    let mut operations = Vec::new();
    for index in 0..count {
        operations.push(bitwise(BinaryOp::BitOr, 100 + index as u32, 2, 8));
    }
    operations.append(&mut body.blocks[0].operations);
    // Both the genuinely independent prefix and selected final value are live.
    operations.push(bitwise(BinaryOp::BitXor, 200, 15, 100));
    body.blocks[0].operations = operations;
    body.blocks[0].terminator = Some(Terminator::Return {
        values: vec![ValueId(200)],
    });
    body
}

fn selected(body: &FunctionBody, operations: [u32; 3]) -> Result<(), String> {
    require_escape(
        body,
        BlockId(9),
        operations,
        [ValueId(2), ValueId(4), ValueId(8)],
        [ValueId(13), ValueId(14), ValueId(15)],
        &mut ScanMeter::default(),
    )
}

#[test]
fn source_candidate_shifted_region_preserves_exact_live_prefix_and_boundary() {
    check(&graph()).unwrap();
    for offset in [1, 3, 8] {
        let body = prefixed(offset);
        let first = offset as u32;
        selected(&body, [first, first + 1, first + 2]).unwrap();
        assert_eq!(body.blocks[0].operations.len(), offset + 4);
    }
}

#[test]
fn source_candidate_shifted_region_checks_gaps_order_overflow_and_extent() {
    let body = prefixed(1);
    for interval in [
        [1, 3, 4],
        [3, 2, 1],
        [1, 1, 2],
        [3, 4, 5],
        [u32::MAX - 1, u32::MAX, 0],
        [u32::MAX; 3],
    ] {
        assert_eq!(selected(&body, interval).unwrap_err(), BOUNDARY);
    }
    assert_eq!(selected(&graph(), [1, 2, 3]).unwrap_err(), BOUNDARY);
    let mut body = prefixed(1);
    body.blocks.insert(0, BasicBlock::new(BlockId(0)));
    assert_eq!(selected(&body, [1, 2, 3]).unwrap_err(), BOUNDARY);
    body.blocks.clear();
    assert_eq!(selected(&body, [1, 2, 3]).unwrap_err(), BOUNDARY);
}

#[test]
fn source_candidate_shifted_region_rejects_prefix_suffix_and_terminator_escapes() {
    for intermediate in [13, 14] {
        let mut body = prefixed(1);
        body.blocks[0].operations[0] = bitwise(BinaryOp::BitOr, 100, intermediate, 8);
        assert_eq!(selected(&body, [1, 2, 3]).unwrap_err(), ESCAPE);
        let mut body = prefixed(1);
        body.blocks[0].operations[4] = bitwise(BinaryOp::BitXor, 200, intermediate, 100);
        assert_eq!(selected(&body, [1, 2, 3]).unwrap_err(), ESCAPE);
        for terminator in [
            Terminator::Return {
                values: vec![ValueId(intermediate)],
            },
            Terminator::Branch {
                target: BlockId(10),
                arguments: vec![ValueId(intermediate)],
            },
        ] {
            let mut body = prefixed(1);
            body.blocks[0].terminator = Some(terminator);
            assert_eq!(selected(&body, [1, 2, 3]).unwrap_err(), ESCAPE);
        }
    }
}

#[test]
fn source_candidate_shifted_region_uses_relative_predecessor_results_only() {
    let mut body = prefixed(3);
    // Absolute ordinal3 must not grant access to all three selected results.
    body.blocks[0].operations[3] = bitwise(BinaryOp::BitXor, 13, 2, 15);
    assert_eq!(selected(&body, [3, 4, 5]).unwrap_err(), UNBOUND);
    let mut body = prefixed(3);
    body.blocks[0].operations[4] = bitwise(BinaryOp::BitAnd, 14, 14, 8);
    assert_eq!(selected(&body, [3, 4, 5]).unwrap_err(), UNBOUND);
    let mut body = prefixed(3);
    // An unrelated prefix result cannot replace an exact formal live-in.
    body.blocks[0].operations[3] = bitwise(BinaryOp::BitXor, 13, 100, 4);
    assert_eq!(selected(&body, [3, 4, 5]).unwrap_err(), UNBOUND);
}

#[test]
fn source_candidate_prefix_cannot_satisfy_selected_live_in_or_live_out() {
    let mut body = prefixed(1);
    // Prefix uses formal8, but the selected graph no longer uses that formal.
    body.blocks[0].operations[2] = bitwise(BinaryOp::BitAnd, 14, 13, 4);
    assert_eq!(selected(&body, [1, 2, 3]).unwrap_err(), LIVE);
    let mut body = prefixed(1);
    body.blocks[0].operations.pop();
    body.blocks[0].terminator = Some(Terminator::Return {
        values: vec![ValueId(100)],
    });
    assert_eq!(selected(&body, [1, 2, 3]).unwrap_err(), LIVE);
    let mut body = prefixed(1);
    body.blocks[0].operations[0] = bitwise(BinaryOp::BitOr, 100, 15, 8);
    assert_eq!(
        selected(&body, [1, 2, 3]).unwrap_err(),
        "source-candidate output used before selected boundary"
    );
}

#[test]
fn source_candidate_shifted_region_guard_uses_existing_cumulative_scan_budget() {
    let mut meter = ScanMeter::default();
    meter.scan(1024 * 1024).unwrap();
    assert_eq!(
        require_escape(
            &prefixed(1),
            BlockId(9),
            [1, 2, 3],
            [ValueId(2), ValueId(4), ValueId(8)],
            [ValueId(13), ValueId(14), ValueId(15)],
            &mut meter
        )
        .unwrap_err(),
        "source-boundary work limit"
    );
}
