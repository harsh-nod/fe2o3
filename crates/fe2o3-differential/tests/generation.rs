use fe2o3_differential::{
    BinaryOp, Expr, GenerateConfig, KernelCase, MAX_EXPR_DEPTH, MAX_EXPR_NODES, MAX_INPUTS,
    MAX_WORK_ITEMS, ModelError, Program, compare_outputs, decode_case_v1, encode_case_v1,
    evaluate_case, generate_case,
};

#[test]
fn evaluator_has_explicit_wrapping_and_select_semantics() {
    let expression = Expr::Select {
        condition: Box::new(Expr::Binary {
            op: BinaryOp::Lt,
            left: Box::new(Expr::GlobalId),
            right: Box::new(Expr::Const(2)),
        }),
        then_value: Box::new(Expr::Binary {
            op: BinaryOp::Add,
            left: Box::new(Expr::Load { input: 0 }),
            right: Box::new(Expr::Const(1)),
        }),
        else_value: Box::new(Expr::Const(-7)),
    };
    let program = Program::new(1, 3, expression).unwrap();
    let case = KernelCase::new(9, program, vec![vec![i32::MAX, 4, 8]]).unwrap();
    assert_eq!(evaluate_case(&case), [i32::MIN, 5, -7]);

    let report = compare_outputs(&case, &[i32::MIN, 6]);
    assert!(report.is_mismatch());
    assert_eq!(report.total_mismatches, 2);
    assert_eq!(report.mismatches[0].lane, 1);
    assert_eq!(report.mismatches[1].expected, Some(-7));
    assert_eq!(report.mismatches[1].observed, None);
}

#[test]
fn generator_is_seeded_deterministic_and_not_constant() {
    let config = GenerateConfig::default();
    let first = generate_case(0x1234, config);
    assert_eq!(first, generate_case(0x1234, config));
    assert_ne!(first, generate_case(0x1235, config));
}

#[test]
fn many_seeds_preserve_bounds_codec_and_evaluation() {
    let config = GenerateConfig::new(4, 19, 63, 9).unwrap();
    for seed in 0..1_024 {
        let case = generate_case(seed, config);
        assert!(case.program().expression().node_count() <= 63);
        assert!(case.program().expression().depth() <= 9);
        let bytes = encode_case_v1(&case).unwrap();
        let decoded = decode_case_v1(&bytes).unwrap();
        assert_eq!(decoded, case, "seed {seed}");
        assert_eq!(evaluate_case(&decoded), evaluate_case(&case), "seed {seed}");
    }
}

#[test]
fn generation_configuration_is_bounded() {
    assert!(GenerateConfig::new((MAX_INPUTS + 1) as u8, 1, 1, 1).is_err());
    assert!(GenerateConfig::new(0, 0, 1, 1).is_err());
    assert!(GenerateConfig::new(0, (MAX_WORK_ITEMS + 1) as u16, 1, 1).is_err());
    assert!(GenerateConfig::new(0, 1, 0, 1).is_err());
    assert!(GenerateConfig::new(0, 1, (MAX_EXPR_NODES + 1) as u8, 1).is_err());
    assert!(GenerateConfig::new(0, 1, 1, 0).is_err());
    assert!(GenerateConfig::new(0, 1, 1, (MAX_EXPR_DEPTH + 1) as u8).is_err());
}

#[test]
fn model_rejects_unknown_inputs_and_bad_shapes() {
    assert_eq!(
        Program::new(0, 1, Expr::Load { input: 0 }).unwrap_err(),
        ModelError::UnknownInput {
            input: 0,
            input_count: 0
        }
    );
    let program = Program::new(1, 2, Expr::Load { input: 0 }).unwrap();
    assert_eq!(
        KernelCase::new(0, program.clone(), vec![]).unwrap_err(),
        ModelError::InputCountMismatch {
            declared: 1,
            actual: 0
        }
    );
    assert_eq!(
        KernelCase::new(0, program, vec![vec![1]]).unwrap_err(),
        ModelError::InputLengthMismatch {
            input: 0,
            expected: 2,
            actual: 1
        }
    );
}
