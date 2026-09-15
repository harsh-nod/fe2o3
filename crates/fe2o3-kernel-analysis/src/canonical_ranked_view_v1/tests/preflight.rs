use super::*;
use dialect_kernel::SemanticTypedExpressionV1;

#[test]
fn inert_preflight_exposes_actual_store_parameter_and_rhs_without_opening_gate() {
    let canonical = VerifiedCanonicalKernelIrV13::from_module(fill_module("preflight")).unwrap();
    let preflight =
        preflight_canonical_ranked_view_v1(&canonical, 17, &"preflight".into()).unwrap();
    let [write] = preflight.writes() else {
        panic!("one write")
    };
    assert_eq!(write.source_operation(), 9);
    assert_eq!(write.pointer(), ValueId(11));
    assert_eq!(write.index(), ValueId(9));
    assert_eq!(write.predicate(), Some(ValueId(7)));
    assert_eq!(write.rhs(), ValueId(2));
    assert_eq!(preflight.write_parameter(0), Some(ValueId(0)));
    assert_eq!(preflight.write_view_shape(0), Some(&[DYNAMIC_EXTENT][..]));
    assert!(matches!(
        preflight.write_rhs(0),
        Some(SemanticTypedExpressionV1::Symbol { symbol: 2, .. })
    ));
    assert!(preflight.write_rhs(1).is_none());
    assert!(!preflight.grants_graph_verification_authority());
    assert!(!preflight.grants_artifact_or_launch_authority());
    assert!(matches!(
        prepare_canonical_ranked_view_v1(&canonical, 17, &"preflight".into()),
        Err(CanonicalRankedViewErrorV1::MissingWriteContracts { .. })
    ));
    let mut context = setup();
    let epoch = context.ir_mutation_attempt_epoch().unwrap();
    assert!(
        compile_canonical_ranked_view_v1(&mut context, &canonical, 17, &"preflight".into())
            .is_err()
    );
    assert_eq!(context.ir_mutation_attempt_epoch().unwrap(), epoch);
    assert!(context.is_ir_empty());
}

#[test]
fn preflight_rejects_canonical_epoch_and_function_substitution() {
    let canonical = VerifiedCanonicalKernelIrV13::from_module(fill_module("subject")).unwrap();
    let preflight = preflight_canonical_ranked_view_v1(&canonical, 17, &"subject".into()).unwrap();
    preflight
        .require_exact(&canonical, 17, &"subject".into())
        .unwrap();
    assert!(
        preflight
            .require_exact(&canonical, 18, &"subject".into())
            .is_err()
    );
    assert!(
        preflight
            .require_exact(&canonical, 17, &"other".into())
            .is_err()
    );
    let mut changed = fill_module("subject");
    rhs_constant(&mut changed, 7.0_f32.to_bits());
    let changed = VerifiedCanonicalKernelIrV13::from_module(changed).unwrap();
    assert!(
        preflight
            .require_exact(&changed, 17, &"subject".into())
            .is_err()
    );
}

#[test]
fn preflight_projects_constant_bits_from_actual_rhs_not_an_unconsumed_value() {
    let mut module = fill_module("rhs");
    rhs_constant(&mut module, (-0.0_f32).to_bits());
    operations(&mut module).insert(
        0,
        KirOp::effect_free(
            ValueDef::new(ValueId(50), Type::F32),
            OperationKind::Constant(Constant::F32Bits(99.0_f32.to_bits())),
        ),
    );
    let canonical = VerifiedCanonicalKernelIrV13::from_module(module).unwrap();
    let preflight = preflight_canonical_ranked_view_v1(&canonical, 17, &"rhs".into()).unwrap();
    assert!(
        matches!(preflight.write_rhs(0), Some(SemanticTypedExpressionV1::Constant { bits, .. }) if *bits == u64::from((-0.0_f32).to_bits()))
    );
    preflight
        .numerical_contract()
        .validate(preflight.write_rhs(0).unwrap())
        .unwrap();
}

#[test]
fn preflight_bounds_shared_rhs_expansion() {
    let mut module = fill_module("bounded");
    let ops = operations(&mut module);
    let mut value = ValueId(2);
    for n in 0..20 {
        let next = ValueId(30 + n);
        let position = ops.len() - 1;
        ops.insert(
            position,
            KirOp::effect_free(
                ValueDef::new(next, Type::F32),
                OperationKind::Binary {
                    op: BinaryOp::Add,
                    lhs: value,
                    rhs: value,
                },
            ),
        );
        value = next;
    }
    let OperationKind::GuardedStore { value: rhs, .. } = &mut ops.last_mut().unwrap().kind else {
        panic!("store")
    };
    *rhs = value;
    let canonical = VerifiedCanonicalKernelIrV13::from_module(module).unwrap();
    let error = preflight_canonical_ranked_view_v1(&canonical, 17, &"bounded".into()).unwrap_err();
    assert!(error.to_string().contains("expansion limit"));
}
