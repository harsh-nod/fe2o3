use super::*;

include!("source_geometry_tests.rs");

fn contract(role: SemanticMfmaOperandRoleV1) -> SemanticMfmaOperandContractV1 {
    SemanticMfmaOperandContractV1 {
        role,
        profile: SemanticMfmaProfileV1::Bf16F32M16N16K16,
        register_distribution: SemanticMfmaRegisterDistributionV1::Tile16x16,
        wave_width: 64,
    }
}

fn symbol(id: u32) -> Expr {
    Expr::Symbol {
        symbol: id,
        scalar: INDEX,
    }
}

fn inputs(stride: u64) -> CpuReadInputsV1 {
    CpuReadInputsV1 {
        lane: symbol(0),
        offset: symbol(1),
        rows: symbol(2),
        columns: symbol(3),
        stride: index(stride),
        first_base: symbol(4),
        second_base: symbol(5),
        allocation_length: symbol(6),
    }
}

fn eval(expression: &Expr, values: &[u64; 7]) -> u64 {
    match expression {
        Expr::Constant { bits, .. } => *bits,
        Expr::Symbol { symbol, .. } => values[*symbol as usize],
        Expr::Binary {
            operation,
            lhs,
            rhs,
            ..
        } => {
            let lhs = eval(lhs, values);
            let rhs = eval(rhs, values);
            match operation {
                Binary::Add => lhs.wrapping_add(rhs),
                Binary::Subtract => lhs.wrapping_sub(rhs),
                Binary::Multiply => lhs.wrapping_mul(rhs),
                Binary::BitAnd => lhs & rhs,
                Binary::ShiftRight => lhs >> rhs,
                _ => panic!("unexpected CPU request operation"),
            }
        }
        Expr::Compare {
            operation,
            lhs,
            rhs,
            ..
        } => {
            let lhs = eval(lhs, values);
            let rhs = eval(rhs, values);
            u64::from(match operation {
                Compare::LessThan => lhs < rhs,
                Compare::LessOrEqual => lhs <= rhs,
                _ => panic!("unexpected CPU request comparison"),
            })
        }
        Expr::Select {
            condition,
            when_true,
            when_false,
            ..
        } => eval(
            if eval(condition, values) != 0 {
                when_true
            } else {
                when_false
            },
            values,
        ),
        Expr::Cast {
            source:
                Scalar::Integer {
                    signed: false,
                    bits,
                },
            target: INDEX,
            operand,
            kind: fe2o3_pliron::ProductionSemanticCastV2::Integer,
        } => {
            eval(operand, values)
                & if *bits == 64 {
                    u64::MAX
                } else {
                    (1_u64 << bits) - 1
                }
        }
        _ => panic!("CPU read requests cannot contain memory leaves or floating semantics"),
    }
}

fn widened_u32(id: u32) -> Expr {
    let source = Scalar::Integer {
        signed: false,
        bits: 32,
    };
    Expr::Cast {
        kind: fe2o3_pliron::ProductionSemanticCastV2::Integer,
        source,
        target: INDEX,
        operand: Box::new(Expr::Symbol {
            symbol: id,
            scalar: source,
        }),
    }
}

#[test]
fn dynamic_rows_and_stride_reuse_exact_source_u32_widening_bounds() {
    for role in [SemanticMfmaOperandRoleV1::A, SemanticMfmaOperandRoleV1::B] {
        let mut source = inputs(16);
        source.rows = widened_u32(2);
        source.stride = widened_u32(3);
        let schedule = source_read_schedule_v1(contract(role), source).unwrap();
        for lane in 0..64 {
            for size in [0, 1, 16, u32::MAX as u64] {
                for offset in [0, 7, u64::MAX] {
                    for base in [0, size.saturating_sub(16)] {
                        let values = [lane, offset, size, size, base, base, u64::MAX];
                        for event in &schedule.events {
                            let actual = (eval(&event.guard, &values) != 0)
                                .then(|| eval(&event.index, &values));
                            assert_eq!(actual, native_source(role, event.component, size, &values));
                        }
                    }
                }
            }
        }
    }
    let mut unbounded_rows = inputs(16);
    unbounded_rows.stride = widened_u32(3);
    assert_eq!(
        source_read_schedule_v1(contract(SemanticMfmaOperandRoleV1::A), unbounded_rows).err(),
        Some(CpuReadRequestErrorV1::UnsupportedDynamicStride)
    );
}

fn native_source(
    role: SemanticMfmaOperandRoleV1,
    component: u8,
    stride: u64,
    values: &[u64; 7],
) -> Option<u64> {
    let [lane, offset, rows, columns, first, second, length] = *values;
    let (row, column) = match role {
        SemanticMfmaOperandRoleV1::A => (
            first.checked_add(lane & 15)?,
            second
                .checked_add((lane >> 4) * 4)?
                .checked_add(component.into())?,
        ),
        SemanticMfmaOperandRoleV1::B => (
            first
                .checked_add((lane >> 4) * 4)?
                .checked_add(component.into())?,
            second.checked_add(lane & 15)?,
        ),
    };
    if row >= rows || column >= columns {
        return None;
    }
    row.checked_mul(stride)
        .and_then(|row_offset| offset.checked_add(row_offset))
        .and_then(|index| index.checked_add(column))
        .filter(|index| *index < length)
}

#[test]
fn all_lanes_roles_tails_and_overflow_match_independent_checked_source() {
    for role in [SemanticMfmaOperandRoleV1::A, SemanticMfmaOperandRoleV1::B] {
        for stride in [0, 1, 16, 31, 1_u64 << 32, u64::MAX] {
            let schedule = source_read_schedule_v1(contract(role), inputs(stride)).unwrap();
            for lane in 0..64 {
                for (offset, rows, columns, first, second, length) in [
                    (0, 16, 16, 0, 0, 256),
                    (0, 15, 13, 0, 0, 225),
                    (0, 0, 16, 0, 0, 256),
                    (0, 16, 0, 0, 0, 256),
                    (0, 16, 16, 0, 0, 0),
                    (3, 32, 32, 16, 16, 1027),
                    (u64::MAX, u64::MAX, u64::MAX, 0, 0, u64::MAX),
                    (0, u64::MAX, u64::MAX, u64::MAX - 1, 0, u64::MAX),
                    (0, u64::MAX, u64::MAX, 0, u64::MAX - 1, u64::MAX),
                    (0, u64::MAX, u64::MAX, u64::MAX / 2, 17, u64::MAX),
                    (0, u64::MAX, u64::MAX, 17, u64::MAX / 2, u64::MAX),
                ] {
                    let values = [lane, offset, rows, columns, first, second, length];
                    assert_eq!(eval(&schedule.lane_precondition, &values), 1);
                    for event in &schedule.events {
                        let requested =
                            (eval(&event.guard, &values) != 0).then(|| eval(&event.index, &values));
                        assert_eq!(
                            requested,
                            native_source(role, event.component, stride, &values),
                            "{role:?} component={} stride={stride} values={values:?}",
                            event.component
                        );
                        assert_eq!(
                            event.fallback,
                            Expr::Constant {
                                scalar: BITS,
                                bits: 0
                            }
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn four_events_remain_distinct_even_when_their_addresses_coincide() {
    let schedule =
        source_read_schedule_v1(contract(SemanticMfmaOperandRoleV1::B), inputs(0)).unwrap();
    let values = [0, 0, 16, 16, 0, 0, 1];
    assert_eq!(
        schedule.events.each_ref().map(|event| event.component),
        [0, 1, 2, 3]
    );
    for event in &schedule.events {
        assert_eq!(eval(&event.guard, &values), 1);
        assert_eq!(eval(&event.index, &values), 0);
    }
    for invalid_lane in [64, 65, u64::MAX] {
        assert_eq!(
            eval(
                &schedule.lane_precondition,
                &[invalid_lane, 0, 16, 16, 0, 0, 256]
            ),
            0
        );
    }
}

#[test]
fn source_memory_leaves_dynamic_stride_and_float_inputs_do_not_acquire_proof() {
    use CpuReadRequestErrorV1 as E;
    for mutation in 0..6 {
        let mut source = inputs(16);
        let mut contract = contract(SemanticMfmaOperandRoleV1::A);
        let expected = match mutation {
            0 => {
                source.stride = symbol(7);
                E::UnsupportedDynamicStride
            }
            1 => {
                source.rows = Expr::Constant {
                    scalar: Scalar::Float { bits: 32 },
                    bits: 0,
                };
                E::InvalidSourceExpression
            }
            2 => {
                source.offset = symbol(fe2o3_pliron::PRODUCTION_SEMANTIC_LOAD_SYMBOL_BASE_V2);
                E::InvalidSourceExpression
            }
            3 => {
                source.offset = Expr::Load(fe2o3_pliron::ProductionSemanticLoadV2 {
                    block: 0,
                    operation: 1,
                    scalar: INDEX,
                    read_mode: fe2o3_pliron::ProductionSemanticReadModeV2::UnorderedNonVolatile,
                    allocation_origin: 1,
                    view: fe2o3_pliron::ProductionRankedValueV1::Argument(0),
                    indices: vec![fe2o3_pliron::ProductionRankedValueV1::Argument(1)]
                        .into_boxed_slice(),
                });
                E::UnboundMemoryInput
            }
            4 => {
                contract.wave_width = 32;
                E::UnsupportedFragment
            }
            _ => {
                source.lane = Expr::Cast {
                    kind: fe2o3_pliron::ProductionSemanticCastV2::FloatToIntegerSaturating,
                    source: Scalar::Float { bits: 32 },
                    target: INDEX,
                    operand: Box::new(Expr::Constant {
                        scalar: Scalar::Float { bits: 32 },
                        bits: 0,
                    }),
                };
                E::InvalidSourceExpression
            }
        };
        assert_eq!(
            source_read_schedule_v1(contract, source).err(),
            Some(expected),
            "mutation {mutation}"
        );
    }
}

#[test]
fn input_expression_budget_is_checked_before_fixed_schedule_expansion() {
    let mut source = inputs(16);
    for _ in 0..40 {
        source.offset = binary(Binary::Add, source.offset, index(1));
    }
    assert_eq!(
        source_read_schedule_v1(contract(SemanticMfmaOperandRoleV1::A), source).err(),
        Some(CpuReadRequestErrorV1::ResourceLimit)
    );
}
