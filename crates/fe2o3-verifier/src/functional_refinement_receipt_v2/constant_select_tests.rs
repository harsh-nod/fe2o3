use ProductionSemanticExpressionV2 as Expr;
use ProductionSemanticScalarTypeV2 as Scalar;

fn boolean(bits: u64) -> Expr {
    Expr::Constant {
        scalar: Scalar::Bool,
        bits,
    }
}

fn constant(bits: u64) -> Expr {
    Expr::Constant {
        scalar: Scalar::Float { bits: 32 },
        bits,
    }
}

fn select(condition: Expr, when_true: Expr, when_false: Expr) -> Expr {
    Expr::Select {
        scalar: when_true.scalar(),
        condition: Box::new(condition),
        when_true: Box::new(when_true),
        when_false: Box::new(when_false),
    }
}

fn formula(
    actual: Expr,
    expected: Expr,
) -> Result<ProductionRankedKernelV1, fe2o3_pliron::ProductionRankedKernelErrorV1> {
    let ids = [
        ProductionRankedValueIdV1::new(0),
        ProductionRankedValueIdV1::new(1),
    ];
    let mut operations = [actual, expected]
        .into_iter()
        .zip(ids)
        .map(
            |(expression, result)| ProductionRankedOperationV1::SemanticExpression {
                result,
                numerical_contract: ProductionNumericalContractV2::exact_for_expression(
                    &expression,
                ),
                expression,
            },
        )
        .collect::<Vec<_>>();
    operations.push(
        ProductionRankedOperationV1::RequestAuthenticatedReferenceEquivalent {
            actual: ProductionRankedValueV1::Local(ids[0]),
            expected: ProductionRankedValueV1::Local(ids[1]),
            subjects: subjects(),
        },
    );
    ProductionRankedKernelV1::new(
        "constant_select",
        0,
        vec![ProductionRankedBlockV1::new(
            operations,
            ProductionRankedTerminatorV1::Return,
        )],
    )
}

fn pairs() -> [(ProductionRankedValueV1, ProductionRankedValueV1); 1] {
    [(
        ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0)),
        ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(1)),
    )]
}

#[test]
fn ieee_constant_select_preserves_selected_bits_including_exceptional_values() {
    for (width, patterns) in [
        (32, [0, 0x8000_0000, 0x7f80_0000, 0x7fc0_0001]),
        (
            64,
            [
                0,
                0x8000_0000_0000_0000,
                0x7ff0_0000_0000_0000,
                0x7ff8_0000_0000_0001,
            ],
        ),
    ] {
        for bits in patterns {
            let selected = Expr::Constant {
                scalar: Scalar::Float { bits: width },
                bits,
            };
            let other = Expr::Constant {
                scalar: selected.scalar(),
                bits: bits ^ 1,
            };
            for expression in [
                select(boolean(1), selected.clone(), other.clone()),
                select(boolean(0), other.clone(), selected.clone()),
                select(
                    boolean(1),
                    select(boolean(0), other.clone(), selected.clone()),
                    other.clone(),
                ),
            ] {
                let kernel = formula(expression.clone(), selected.clone()).unwrap();
                SemanticFormulaProgramV2::build(&kernel, &pairs()).unwrap();
                assert_eq!(
                    render_ieee_congruence_expression_v2(&expression).unwrap(),
                    render_ieee_congruence_expression_v2(&selected).unwrap()
                );
            }
        }
    }
}

#[test]
fn ieee_dynamic_select_remains_opaque_and_branch_sensitive() {
    let condition = Expr::Symbol {
        scalar: Scalar::Bool,
        symbol: 3,
    };
    let expression = select(condition.clone(), constant(0), constant(1));
    let rendered = render_ieee_congruence_expression_v2(&expression).unwrap();
    let tag = semantic_operation_tag_v2(6, 0, scalar_tag_v2(expression.scalar()), 0);
    assert!(rendered.starts_with(&format!("fe2o3_ieee_operator_congruence_v2({tag},")));
    for changed in [
        constant(0),
        constant(1),
        select(condition, constant(1), constant(0)),
        select(
            Expr::Symbol {
                scalar: Scalar::Bool,
                symbol: 4,
            },
            constant(0),
            constant(1),
        ),
    ] {
        assert_ne!(
            rendered,
            render_ieee_congruence_expression_v2(&changed).unwrap()
        );
    }
}

#[test]
fn constant_select_retains_unselected_symbols_and_original_obligation_binding() {
    let symbol = Expr::Symbol {
        scalar: Scalar::Float { bits: 32 },
        symbol: 7,
    };
    let dead = Expr::Symbol {
        scalar: symbol.scalar(),
        symbol: 8,
    };
    let kernel = formula(select(boolean(1), symbol.clone(), dead), symbol.clone()).unwrap();
    let program = SemanticFormulaProgramV2::build(&kernel, &pairs()).unwrap();
    assert_eq!(program.symbols, BTreeSet::from([7, 8]));
    let replay = program.render_lemma(&pairs(), "replayed_select").unwrap();
    assert!(replay.contains("s8: int"));
    let definitions = replay
        .lines()
        .filter_map(|line| line.split_once(": int = ").map(|(_, rhs)| rhs))
        .collect::<Vec<_>>();
    assert_eq!(definitions.len(), 2);
    assert_eq!(
        definitions[0], definitions[1],
        "known selection and reference render the same value"
    );

    let generated = [0, 1].map(|bits| {
        generate_ranked_functional_refinement_proof_v2(
            &formula(
                select(boolean(1), symbol.clone(), constant(bits)),
                symbol.clone(),
            )
            .unwrap(),
            0,
            2,
            subjects(),
        )
        .unwrap()
    });
    assert_ne!(
        generated[0].0.normalized_obligation_effect_ir_hash(),
        generated[1].0.normalized_obligation_effect_ir_hash()
    );
    assert_eq!(
        generated[0].1.source(),
        generated[1].1.source(),
        "only pure rendering folds the dead branch"
    );
}

#[test]
fn constant_select_still_validates_and_charges_unselected_branches() {
    let expected = constant(0);
    let invalid = Expr::Symbol {
        scalar: expected.scalar(),
        symbol: fe2o3_pliron::PRODUCTION_SEMANTIC_LOAD_SYMBOL_BASE_V2,
    };
    let mut too_deep = expected.clone();
    for _ in 0..=fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 {
        too_deep = Expr::Unary {
            operation: fe2o3_pliron::ProductionSemanticUnaryOpV2::Negate,
            scalar: expected.scalar(),
            operand: Box::new(too_deep),
        };
    }
    let integer = Scalar::Integer {
        signed: false,
        bits: 32,
    };
    let division_by_zero = Expr::Cast {
        kind: fe2o3_pliron::ProductionSemanticCastV2::IntegerToFloat,
        source: integer,
        target: expected.scalar(),
        operand: Box::new(Expr::Binary {
            operation: ProductionSemanticBinaryOpV2::Divide,
            scalar: integer,
            overflow: ProductionOverflowContractV2::Wrapping,
            lhs: Box::new(Expr::Constant {
                scalar: integer,
                bits: 1,
            }),
            rhs: Box::new(Expr::Constant {
                scalar: integer,
                bits: 0,
            }),
        }),
    };
    for actual in [
        select(boolean(2), expected.clone(), expected.clone()),
        select(boolean(1), expected.clone(), boolean(0)),
        select(boolean(1), expected.clone(), invalid),
        select(boolean(1), expected.clone(), too_deep),
    ] {
        assert!(formula(actual, expected.clone()).is_err());
    }
    let error = formula(
        select(boolean(1), expected.clone(), division_by_zero),
        expected.clone(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        fe2o3_pliron::ProductionRankedKernelErrorV1::InvalidSemanticExpression(
            fe2o3_pliron::ProductionSemanticExpressionErrorV2::IncompleteDomain
        )
    ));

    let mut dead = expected.clone();
    for _ in 0..11 {
        dead = Expr::Binary {
            operation: ProductionSemanticBinaryOpV2::Add,
            scalar: expected.scalar(),
            overflow: ProductionOverflowContractV2::Wrapping,
            lhs: Box::new(dead.clone()),
            rhs: Box::new(dead),
        };
    }
    let actual = select(boolean(1), expected, dead);
    let kernel = formula(actual.clone(), actual).unwrap();
    let error = SemanticFormulaProgramV2::build(&kernel, &pairs())
        .err()
        .unwrap();
    assert_eq!(error.kind(), formula_resource_limit().kind());
}

#[test]
fn pinned_verus_proves_constant_selection_and_rejects_wrong_branch_and_signed_zero() {
    let verifier = std::env::var_os("FE2O3_PINNED_RUST_VERIFY").unwrap_or_else(|| {
        "/home/harsh/.cache/fe2o3-verus-0.2026.08.02/verus-x86-linux/verus".into()
    });
    struct Scratch(std::path::PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let path = std::env::temp_dir().join(format!("fe2o3-constant-select-{}", std::process::id()));
    fs::create_dir(&path).unwrap();
    let scratch = Scratch(path);
    let x = Expr::Symbol {
        scalar: Scalar::Float { bits: 32 },
        symbol: 7,
    };
    let y = Expr::Symbol {
        scalar: x.scalar(),
        symbol: 8,
    };
    for (name, actual, expected, success) in [
        (
            "true",
            select(boolean(1), x.clone(), y.clone()),
            x.clone(),
            true,
        ),
        (
            "false",
            select(boolean(0), x.clone(), y.clone()),
            y.clone(),
            true,
        ),
        (
            "nested",
            select(
                boolean(1),
                select(boolean(0), y.clone(), x.clone()),
                y.clone(),
            ),
            x.clone(),
            true,
        ),
        (
            "wrong-branch",
            select(boolean(0), x.clone(), y.clone()),
            x.clone(),
            false,
        ),
        (
            "dynamic",
            select(
                Expr::Symbol {
                    scalar: Scalar::Bool,
                    symbol: 9,
                },
                x.clone(),
                y,
            ),
            x,
            false,
        ),
        (
            "signed-zero",
            select(boolean(1), constant(0x8000_0000), constant(0)),
            constant(0),
            false,
        ),
    ] {
        let (_, source) = generate_ranked_functional_refinement_proof_v2(
            &formula(actual, expected).unwrap(),
            0,
            2,
            subjects(),
        )
        .unwrap();
        assert!(!source.authenticates_verus_execution());
        let path = scratch.0.join(format!("{name}.rs"));
        fs::write(&path, source.source()).unwrap();
        let output = Command::new(&verifier).arg(path).output().unwrap();
        assert_eq!(output.status.success(), success, "{name}: {output:?}");
        if success {
            validate_proved_output(&super::output(0, &output.stdout, &output.stderr)).unwrap();
        } else {
            assert!(
                String::from_utf8_lossy(&output.stderr).contains("assertion failed"),
                "{name}: {output:?}"
            );
        }
    }
}
