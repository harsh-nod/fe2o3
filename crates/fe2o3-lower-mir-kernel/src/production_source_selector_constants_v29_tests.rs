use super::*;

type Rows = Vec<(Type, Option<Operation>)>;

fn constant(rows: &mut Rows, scalar: ScalarType, constant: Constant) {
    let ty = Type::Scalar(scalar);
    rows.push((
        ty.clone(),
        Some(Operation::effect_free(
            ValueDef::new(ValueId(rows.len() as u32), ty),
            OperationKind::Constant(constant),
        )),
    ));
}

fn cast(rows: &mut Rows, kind: CastKind, scalar: ScalarType) {
    let ty = Type::Scalar(scalar);
    rows.push((
        ty.clone(),
        Some(Operation::effect_free(
            ValueDef::new(ValueId(rows.len() as u32), ty.clone()),
            OperationKind::Cast {
                kind,
                value: ValueId(rows.len() as u32 - 1),
                to: ty,
            },
        )),
    ));
}

fn query(
    rows: &Rows,
    scalar: ScalarType,
    visits: usize,
    work_limit: usize,
) -> (Result<Option<u64>, ProductionSemanticKirErrorV1>, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, 0);
    let result = source_selector_unsigned_constant_v29(
        ValueId(rows.len() as u32 - 1),
        scalar,
        visits,
        &mut budget,
        |value, budget| {
            budget.charge_work(1)?;
            let (ty, operation) = rows
                .get(value.0 as usize)
                .ok_or_else(execution_availability_error_v29)?;
            Ok((ty, operation.as_ref()))
        },
    );
    assert_eq!(budget.storage(), 0);
    assert_eq!(budget.peak_storage(), 0);
    (result, budget.work())
}

#[test]
fn unsigned_selector_constants_follow_exact_widths_and_index_bridges() {
    for (scalar, literal, expected) in [
        (ScalarType::U8, Constant::U8(201), 201),
        (ScalarType::U16, Constant::U16(60001), 60001),
        (
            ScalarType::U32,
            Constant::U32(u32::MAX),
            u64::from(u32::MAX),
        ),
        (ScalarType::U64, Constant::U64(u64::MAX), u64::MAX),
        (ScalarType::Index, Constant::Index(u64::MAX), u64::MAX),
    ] {
        let mut rows = Vec::new();
        constant(&mut rows, scalar, literal);
        assert_eq!(query(&rows, scalar, 1, 13).0.unwrap(), Some(expected));
    }
    let mut rows = Vec::new();
    constant(
        &mut rows,
        ScalarType::U64,
        Constant::U64(0x123456789abcdef0),
    );
    cast(&mut rows, CastKind::Truncate, ScalarType::U8);
    cast(&mut rows, CastKind::ZeroExtend, ScalarType::U32);
    cast(&mut rows, CastKind::ZeroExtend, ScalarType::Index);
    cast(&mut rows, CastKind::Bitcast, ScalarType::U64);
    cast(&mut rows, CastKind::Bitcast, ScalarType::Index);
    assert_eq!(
        query(&rows, ScalarType::Index, rows.len(), 13 * rows.len())
            .0
            .unwrap(),
        Some(0xf0)
    );
    let mut rows = Vec::new();
    constant(&mut rows, ScalarType::U32, Constant::U32(1));
    cast(&mut rows, CastKind::ZeroExtend, ScalarType::U64);
    cast(&mut rows, CastKind::Bitcast, ScalarType::Index);
    assert_eq!(query(&rows, ScalarType::Index, 3, 39).0.unwrap(), Some(1));
}

#[test]
fn unsigned_selector_constants_preserve_unknown_values() {
    for (input, kind) in [
        (ScalarType::I32, CastKind::SignExtend),
        (ScalarType::I64, CastKind::Truncate),
        (ScalarType::I32, CastKind::Bitcast),
        (ScalarType::Bool, CastKind::ZeroExtend),
        (ScalarType::F32, CastKind::FloatToInteger),
    ] {
        let mut rows = vec![(Type::Scalar(input), None)];
        let output = if matches!(kind, CastKind::Truncate | CastKind::Bitcast) {
            ScalarType::U32
        } else {
            ScalarType::U64
        };
        cast(&mut rows, kind, output);
        assert_eq!(query(&rows, output, 2, 26).0.unwrap(), None);
    }
    for scalar in [
        ScalarType::U32,
        ScalarType::I32,
        ScalarType::Bool,
        ScalarType::F32,
    ] {
        let rows = vec![(Type::Scalar(scalar), None)];
        assert_eq!(query(&rows, scalar, 1, 13).0.unwrap(), None);
    }
    let mut rows = Vec::new();
    constant(&mut rows, ScalarType::U32, Constant::U32(1));
    rows[0].1.as_mut().unwrap().kind = OperationKind::Binary {
        op: BinaryOp::Add,
        lhs: ValueId(0),
        rhs: ValueId(0),
    };
    assert_eq!(query(&rows, ScalarType::U32, 1, 13).0.unwrap(), None);
}

#[test]
fn unsigned_selector_constants_reject_forged_definitions_and_casts() {
    for fault in 0..8 {
        let mut rows = Vec::new();
        constant(&mut rows, ScalarType::U32, Constant::U32(1));
        cast(&mut rows, CastKind::ZeroExtend, ScalarType::U64);
        let operation = rows[1].1.as_mut().unwrap();
        match fault {
            0 => operation.results[0].id = ValueId(99),
            1 => operation.results[0].ty = Type::Scalar(ScalarType::U16),
            2 => operation.results.push(operation.results[0].clone()),
            3 => {
                let OperationKind::Cast { to, .. } = &mut operation.kind else {
                    unreachable!()
                };
                *to = Type::Scalar(ScalarType::U16);
            }
            4 => {
                let OperationKind::Cast { kind, .. } = &mut operation.kind else {
                    unreachable!()
                };
                *kind = CastKind::Truncate;
            }
            5 => {
                let OperationKind::Cast { value, .. } = &mut operation.kind else {
                    unreachable!()
                };
                *value = ValueId(99);
            }
            6 => rows[0].0 = Type::Scalar(ScalarType::U64),
            7 => rows[0].1.as_mut().unwrap().results[0].ty = Type::Scalar(ScalarType::U8),
            _ => unreachable!(),
        }
        assert!(
            query(&rows, ScalarType::U64, 2, 26).0.is_err(),
            "fault {fault}"
        );
    }
    // U8 -> Index requires two operations, not one apparent promotion.
    let mut rows = Vec::new();
    constant(&mut rows, ScalarType::U8, Constant::U8(1));
    cast(&mut rows, CastKind::ZeroExtend, ScalarType::Index);
    assert!(query(&rows, ScalarType::Index, 2, 26).0.is_err());
}

#[test]
fn unsigned_selector_constants_keep_multiresult_checked_arithmetic_unknown() {
    for operator in [
        CheckedBinaryOperator::Add,
        CheckedBinaryOperator::Subtract,
        CheckedBinaryOperator::Multiply,
    ] {
        let mut rows = Vec::new();
        constant(&mut rows, ScalarType::U32, Constant::U32(1));
        constant(&mut rows, ScalarType::U32, Constant::U32(2));
        rows.push((
            Type::Scalar(ScalarType::U32),
            Some(Operation::checked_binary(
                ValueDef::new(ValueId(2), Type::Scalar(ScalarType::U32)),
                ValueDef::new(ValueId(3), Type::BOOL),
                operator,
                ValueId(0),
                ValueId(1),
            )),
        ));
        let (result, work) = query(&rows, ScalarType::U32, rows.len(), 13);
        assert_eq!(result.unwrap(), None);
        assert_eq!(
            work, 13,
            "unsupported operations do not traverse their operands"
        );
        assert!(matches!(
            query(&rows, ScalarType::U32, rows.len(), 12).0,
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Work(_)
                )
            )
        ));
    }
}

#[test]
fn unsigned_selector_constants_bound_cycles_and_accept_the_last_visit() {
    let mut rows = Vec::new();
    constant(&mut rows, ScalarType::U64, Constant::U64(42));
    cast(&mut rows, CastKind::Bitcast, ScalarType::Index);
    assert_eq!(query(&rows, ScalarType::Index, 2, 26).0.unwrap(), Some(42));
    assert!(query(&rows, ScalarType::Index, 1, 13).0.is_err());
    rows[0].1.as_mut().unwrap().kind = OperationKind::Cast {
        kind: CastKind::Bitcast,
        value: ValueId(1),
        to: Type::Scalar(ScalarType::U64),
    };
    for visits in [0, 1, 2, 64] {
        let (result, work) = query(&rows, ScalarType::Index, visits, 13 * visits);
        assert!(result.is_err());
        assert_eq!(work, 13 * visits);
    }
}

#[test]
fn unsigned_selector_constants_require_exact_prepaid_work_without_storage() {
    for count in [1, 2, 32, 64, 128] {
        let mut rows = Vec::new();
        constant(&mut rows, ScalarType::U64, Constant::U64(u64::MAX));
        for ordinal in 1..count {
            cast(
                &mut rows,
                CastKind::Bitcast,
                if ordinal % 2 == 1 {
                    ScalarType::Index
                } else {
                    ScalarType::U64
                },
            );
        }
        let scalar = if count % 2 == 0 {
            ScalarType::Index
        } else {
            ScalarType::U64
        };
        let exact = 13 * count;
        let (result, work) = query(&rows, scalar, count, exact);
        assert_eq!(result.unwrap(), Some(u64::MAX));
        assert_eq!(work, exact);
        assert!(matches!(
            query(&rows, scalar, count, exact - 1).0,
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Work(_)
                )
            )
        ));
    }
}
