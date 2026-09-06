use fe2o3_kernel_ir::*;

const SCALARS: [ScalarType; 16] = [
    ScalarType::Bool,
    ScalarType::I8,
    ScalarType::I16,
    ScalarType::I32,
    ScalarType::I64,
    ScalarType::I128,
    ScalarType::U8,
    ScalarType::U16,
    ScalarType::U32,
    ScalarType::U64,
    ScalarType::U128,
    ScalarType::Index,
    ScalarType::F16,
    ScalarType::Bf16,
    ScalarType::F32,
    ScalarType::F64,
];

const CAST_KINDS: [CastKind; 8] = [
    CastKind::Truncate,
    CastKind::ZeroExtend,
    CastKind::SignExtend,
    CastKind::FloatExtend,
    CastKind::FloatTruncate,
    CastKind::IntegerToFloat,
    CastKind::FloatToInteger,
    CastKind::Bitcast,
];

fn cast_module(kind: CastKind, from: Type, to: Type) -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(1), to.clone()),
        OperationKind::Cast {
            kind,
            value: ValueId(0),
            to,
        },
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });

    let mut module = Module::new("tests::cast_verification");
    module.functions.push(Function::internal_helper(
        "cast",
        Signature::new(vec![from], vec![]),
        vec![ValueId(0)],
        vec![block],
    ));
    module
}

fn expected_scalar_cast(kind: CastKind, from: ScalarType, to: ScalarType) -> bool {
    if matches!(
        (kind, from, to),
        (CastKind::ZeroExtend, ScalarType::U32, ScalarType::Index)
            | (CastKind::Bitcast, ScalarType::U64, ScalarType::Index)
            | (CastKind::Bitcast, ScalarType::Index, ScalarType::U64)
    ) {
        return true;
    }
    if from == ScalarType::Index || to == ScalarType::Index {
        return false;
    }

    let (Some(from_width), Some(to_width)) = (from.bit_width(), to.bit_width()) else {
        return false;
    };
    match kind {
        CastKind::RestrictPointerAccess => false,
        CastKind::Truncate => from.is_integer() && to.is_integer() && from_width > to_width,
        CastKind::ZeroExtend => {
            matches!(
                from,
                ScalarType::Bool
                    | ScalarType::U8
                    | ScalarType::U16
                    | ScalarType::U32
                    | ScalarType::U64
                    | ScalarType::U128
            ) && to.is_integer()
                && from_width < to_width
        }
        CastKind::SignExtend => {
            from.is_signed_integer() && to.is_integer() && from_width < to_width
        }
        CastKind::FloatExtend => from.is_float() && to.is_float() && from_width < to_width,
        CastKind::FloatTruncate => from.is_float() && to.is_float() && from_width > to_width,
        CastKind::IntegerToFloat => from.is_integer() && to.is_float(),
        CastKind::FloatToInteger => from.is_float() && to.is_integer(),
        CastKind::Bitcast => {
            from.is_numeric() && to.is_numeric() && from != to && from_width == to_width
        }
    }
}

#[test]
fn pointer_access_restriction_is_exact_and_one_way() {
    let rw = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Private,
        AccessMode::ReadWrite,
    );
    let ro = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Private,
        AccessMode::ReadOnly,
    );
    verify_module(&cast_module(
        CastKind::RestrictPointerAccess,
        rw.clone(),
        ro.clone(),
    ))
    .expect("read-write to read-only restriction must verify");

    for (from, to) in [
        (ro.clone(), rw.clone()),
        (rw.clone(), rw.clone()),
        (
            rw.clone(),
            Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Private,
                AccessMode::WriteOnly,
            ),
        ),
        (
            rw.clone(),
            Type::pointer(
                Type::Scalar(ScalarType::U64),
                AddressSpace::Private,
                AccessMode::ReadOnly,
            ),
        ),
        (
            rw.clone(),
            Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Global,
                AccessMode::ReadOnly,
            ),
        ),
    ] {
        let errors = verify_module(&cast_module(CastKind::RestrictPointerAccess, from, to))
            .expect_err("pointer widening or shape change must fail closed");
        assert!(errors.contains(DiagnosticCode::InvalidCast));
    }
}

#[test]
fn every_scalar_cast_pair_matches_the_closed_structural_contract() {
    for kind in CAST_KINDS {
        for from in SCALARS {
            for to in SCALARS {
                let module = cast_module(kind, Type::Scalar(from), Type::Scalar(to));
                let actual = verify_module(&module);
                let expected = expected_scalar_cast(kind, from, to);
                assert_eq!(
                    actual.is_ok(),
                    expected,
                    "unexpected verifier result for {kind:?} from {from:?} to {to:?}: {actual:?}"
                );
            }
        }
    }
}

#[test]
fn integer_cast_planner_covers_every_integer_pair_with_verified_steps() {
    for from in SCALARS {
        for to in SCALARS {
            let path = plan_integer_cast_v1(from, to);
            let expected = (from.is_integer() || from == ScalarType::Bool) && to.is_integer();
            assert_eq!(
                path.is_some(),
                expected,
                "planner coverage for {from:?} -> {to:?}"
            );
            let Some(path) = path else {
                continue;
            };
            let mut current = from;
            for (kind, target) in path.into_iter().flatten() {
                assert!(
                    expected_scalar_cast(kind, current, target),
                    "invalid planned step {kind:?}: {current:?} -> {target:?}"
                );
                current = target;
            }
            assert_eq!(current, to, "planner result for {from:?} -> {to:?}");
        }
    }

    assert_eq!(
        plan_integer_cast_v1(ScalarType::Index, ScalarType::U32),
        Some([
            Some((CastKind::Bitcast, ScalarType::U64)),
            Some((CastKind::Truncate, ScalarType::U32)),
        ])
    );
    assert_eq!(
        plan_integer_cast_v1(ScalarType::I32, ScalarType::Index),
        Some([
            Some((CastKind::SignExtend, ScalarType::U64)),
            Some((CastKind::Bitcast, ScalarType::Index)),
        ])
    );
}

#[test]
fn accepts_only_the_documented_index_representation_bridges() {
    for (kind, from, to) in [
        (CastKind::ZeroExtend, ScalarType::U32, ScalarType::Index),
        (CastKind::Bitcast, ScalarType::U64, ScalarType::Index),
        (CastKind::Bitcast, ScalarType::Index, ScalarType::U64),
    ] {
        verify_module(&cast_module(kind, Type::Scalar(from), Type::Scalar(to)))
            .unwrap_or_else(|errors| panic!("documented {from:?} -> {to:?} bridge: {errors}"));
    }

    for (kind, from, to) in [
        (CastKind::ZeroExtend, ScalarType::U8, ScalarType::Index),
        (CastKind::ZeroExtend, ScalarType::U64, ScalarType::Index),
        (CastKind::SignExtend, ScalarType::I32, ScalarType::Index),
        (CastKind::Truncate, ScalarType::Index, ScalarType::U32),
        (CastKind::IntegerToFloat, ScalarType::Index, ScalarType::F64),
        (CastKind::FloatToInteger, ScalarType::F64, ScalarType::Index),
        (CastKind::Bitcast, ScalarType::I64, ScalarType::Index),
        (CastKind::Bitcast, ScalarType::U32, ScalarType::Index),
        (CastKind::Bitcast, ScalarType::Index, ScalarType::F64),
    ] {
        let errors = verify_module(&cast_module(kind, Type::Scalar(from), Type::Scalar(to)))
            .expect_err("undocumented Index conversion must fail closed");
        assert!(errors.contains(DiagnosticCode::InvalidCast));
    }
}

#[test]
fn rejects_wrong_width_signedness_category_bool_and_noop_casts_descriptively() {
    for (kind, from, to) in [
        (CastKind::Truncate, ScalarType::U8, ScalarType::U16),
        (CastKind::Truncate, ScalarType::I32, ScalarType::U32),
        (CastKind::Truncate, ScalarType::Bool, ScalarType::U8),
        (CastKind::ZeroExtend, ScalarType::I8, ScalarType::I16),
        (CastKind::ZeroExtend, ScalarType::U32, ScalarType::U16),
        (CastKind::SignExtend, ScalarType::U8, ScalarType::I16),
        (CastKind::SignExtend, ScalarType::I32, ScalarType::I32),
        (CastKind::FloatExtend, ScalarType::F32, ScalarType::F16),
        (CastKind::FloatExtend, ScalarType::F16, ScalarType::Bf16),
        (CastKind::FloatTruncate, ScalarType::F16, ScalarType::F64),
        (CastKind::IntegerToFloat, ScalarType::Bool, ScalarType::F32),
        (CastKind::IntegerToFloat, ScalarType::F32, ScalarType::F64),
        (CastKind::FloatToInteger, ScalarType::F32, ScalarType::Bool),
        (CastKind::FloatToInteger, ScalarType::U32, ScalarType::I32),
        (CastKind::Bitcast, ScalarType::U16, ScalarType::U32),
        (CastKind::Bitcast, ScalarType::Bool, ScalarType::Bool),
        (CastKind::Bitcast, ScalarType::F32, ScalarType::F32),
    ] {
        let errors = verify_module(&cast_module(kind, Type::Scalar(from), Type::Scalar(to)))
            .expect_err("malformed cast must fail closed");
        let diagnostic = errors
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code == DiagnosticCode::InvalidCast)
            .expect("dedicated invalid-cast diagnostic");
        assert!(diagnostic.message.contains(&format!("{kind:?}")));
        assert!(diagnostic.message.contains(&format!("{from:?}")));
        assert!(diagnostic.message.contains(&format!("{to:?}")));
    }
}

#[test]
fn non_scalar_casts_remain_operand_type_errors() {
    let pointer = Type::pointer(Type::F32, AddressSpace::Global, AccessMode::ReadOnly);
    for (from, to) in [
        (pointer.clone(), Type::Scalar(ScalarType::U64)),
        (Type::Scalar(ScalarType::U64), pointer.clone()),
        (Type::Unit, Type::Scalar(ScalarType::U32)),
    ] {
        let errors = verify_module(&cast_module(CastKind::Bitcast, from, to))
            .expect_err("non-scalar casts must fail closed");
        assert!(errors.contains(DiagnosticCode::InvalidOperandType));
        assert!(!errors.contains(DiagnosticCode::InvalidCast));
    }
}
