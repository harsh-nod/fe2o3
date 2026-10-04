//! Hostile plain-model checks cannot construct a joined production owner.
use super::*;
use fe2o3_kernel_ir::{Operation, Terminator, ValueDef};

#[path = "source_bitselect_prefix_escape_v1_tests.rs"]
mod prefix_escape;

fn graph() -> FunctionBody {
    let mut block = BasicBlock::new(BlockId(9));
    block.operations = [
        (BinaryOp::BitXor, 13, 2, 4),
        (BinaryOp::BitAnd, 14, 13, 8),
        (BinaryOp::BitXor, 15, 4, 14),
    ]
    .map(|(op, result, lhs, rhs)| {
        Operation::effect_free(
            ValueDef::new(ValueId(result), Type::Scalar(ScalarType::U32)),
            OperationKind::Binary {
                op,
                lhs: ValueId(lhs),
                rhs: ValueId(rhs),
            },
        )
    })
    .into();
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(15)],
    });
    FunctionBody {
        parameters: vec![ValueId(2), ValueId(4), ValueId(8)],
        blocks: vec![block],
    }
}

fn check(body: &FunctionBody) -> Result<(), String> {
    require_escape(
        body,
        BlockId(9),
        [0, 1, 2],
        [ValueId(2), ValueId(4), ValueId(8)],
        [ValueId(13), ValueId(14), ValueId(15)],
        &mut ScanMeter::default(),
    )
}

#[test]
fn source_candidate_exact_graph_and_all_operand_escape_routes() {
    check(&graph()).unwrap();
    for value in [13, 14] {
        let mut body = graph();
        body.blocks[0].terminator = Some(Terminator::Return {
            values: vec![ValueId(value)],
        });
        assert_eq!(
            check(&body).unwrap_err(),
            "source-candidate intermediate escapes the selected graph"
        );
        body.blocks[0].terminator = Some(Terminator::Branch {
            target: BlockId(10),
            arguments: vec![ValueId(value)],
        });
        assert_eq!(
            check(&body).unwrap_err(),
            "source-candidate intermediate escapes the selected graph"
        );
        let mut body = graph();
        body.blocks[0].operations.push(Operation::effect_free(
            ValueDef::new(ValueId(20), Type::Scalar(ScalarType::U32)),
            OperationKind::Binary {
                op: BinaryOp::BitXor,
                lhs: ValueId(value),
                rhs: ValueId(2),
            },
        ));
        assert_eq!(
            check(&body).unwrap_err(),
            "source-candidate intermediate escapes the selected graph"
        );
    }
}

#[test]
fn source_candidate_dead_output_out_of_bounds_and_nonentry_refuse() {
    let mut body = graph();
    body.blocks[0].terminator = Some(Terminator::Return { values: vec![] });
    assert_eq!(
        check(&body).unwrap_err(),
        "source-candidate graph is not three live-ins and one live-out"
    );
    let mut body = graph();
    body.blocks.insert(0, BasicBlock::new(BlockId(0)));
    assert_eq!(
        check(&body).unwrap_err(),
        "source-candidate boundary is not three contiguous entry operations"
    );
    assert_eq!(
        require_escape(
            &graph(),
            BlockId(9),
            [1, 2, 3],
            [ValueId(2), ValueId(4), ValueId(8)],
            [ValueId(13), ValueId(14), ValueId(15)],
            &mut ScanMeter::default()
        )
        .unwrap_err(),
        "source-candidate boundary is not three contiguous entry operations"
    );
}

#[test]
fn source_candidate_attribution_is_exact_single_owner_and_checked_arithmetic() {
    let fresh = || Attribution {
        operations: [0, 1, 2],
        counts: [0; 3],
    };
    let mut census = fresh();
    for index in 0..3 {
        census.visit(index as u32, 1, Some(index)).unwrap();
    }
    census.finish().unwrap();
    assert_eq!(
        fresh().finish().unwrap_err(),
        "source-candidate selected attribution missing"
    );
    let mut duplicate = fresh();
    duplicate.visit(0, 1, Some(0)).unwrap();
    assert!(
        duplicate
            .visit(0, 1, Some(0))
            .unwrap_err()
            .contains("ambiguous")
    );
    // Foreign statement, terminator and synthetic spans all use None.
    for source in [None, Some(1)] {
        assert!(fresh().visit(0, 1, source).unwrap_err().contains("foreign"));
    }
    assert!(
        fresh()
            .visit(0, 2, Some(0))
            .unwrap_err()
            .contains("ambiguous")
    );
    assert_eq!(
        fresh().visit(u32::MAX, 1, None).unwrap_err(),
        "source-candidate attribution interval overflow"
    );
}

#[test]
fn source_candidate_target_and_maximum_are_not_inferred_from_required() {
    require_profile(true, Some([64, 1, 1]), Some([64, 1, 1])).unwrap();
    assert_eq!(
        require_profile(false, Some([64, 1, 1]), Some([64, 1, 1])).unwrap_err(),
        "source-candidate requires authenticated gfx942 xnack-off wave64"
    );
    for maximum in [None, Some([128, 1, 1]), Some([64, 2, 1])] {
        assert_eq!(
            require_profile(true, Some([64, 1, 1]), maximum).unwrap_err(),
            "source-candidate requires required and maximum 64x1x1 bounds"
        );
    }
}

#[test]
fn source_candidate_replacement_preserves_prefix_suffix_and_utf8_boundaries() {
    let original = "// λ\nlet result = expression;\n".as_bytes();
    let start = "// λ\nlet result = ".len() as u32;
    let end = start + "expression".len() as u32;
    let result = replace_initializer(
        original,
        start,
        end,
        "new_expression",
        &mut ScanMeter::default(),
    )
    .unwrap();
    assert_eq!(result, "// λ\nlet result = new_expression;\n".as_bytes());
    for (start, end) in [(4, 5), (5, 5), (end, start), (0, u32::MAX)] {
        assert_eq!(
            replace_initializer(original, start, end, "x", &mut ScanMeter::default()).unwrap_err(),
            "source-candidate invalid original initializer range"
        );
    }
    let mut meter = ScanMeter::default();
    meter.scan(1024 * 1024).unwrap();
    assert!(
        replace_initializer(original, start, end, "x", &mut meter)
            .unwrap_err()
            .contains("work limit")
    );
}

#[test]
fn source_candidate_renderer_rejects_aliases_ranges_and_identifier_substitution() {
    assert!(Gfx942OrderedProgramRegistersV1::new(0, 0, [1, 2, 3]).is_err());
    assert!(Gfx942OrderedProgramRegistersV1::new(64, 5, [0, 1, 2]).is_err());
    for inputs in [
        ["a", "a", "mask"],
        ["a", "b", "return"],
        ["a", "b", "mask;evil()"],
    ] {
        assert!(render_bitselect_expression_v1(inputs, registers()).is_err());
    }
}
