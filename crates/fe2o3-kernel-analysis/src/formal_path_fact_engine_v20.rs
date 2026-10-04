//! Shared original unsigned affine/Boolean grammar, independent of custody.
//! Each adapter owns its indexes, storage and work accounting.

use super::*;

pub(super) trait Source<'source> {
    type Error;
    fn step(&mut self) -> std::result::Result<(), Self::Error>;
    // Only the exact single-result operation defining `value` may be returned.
    // Unsupported/parameter/multi-result definitions are absent, not invented.
    fn operation(
        &mut self,
        value: ValueId,
    ) -> std::result::Result<Option<&'source Operation>, Self::Error>;
    fn cached(
        &mut self,
        value: ValueId,
    ) -> std::result::Result<Option<Option<Affine>>, Self::Error>;
    fn cache(
        &mut self,
        value: ValueId,
        expression: Option<Affine>,
    ) -> std::result::Result<(), Self::Error>;
    fn push_fact(
        &mut self,
        facts: &mut Vec<Fact>,
        fact: Fact,
    ) -> std::result::Result<(), Self::Error>;
    fn coordinate_last(&self) -> u64;
    fn index_width(&self) -> FormalIndexWidth;
}

pub(super) fn maximum(ty: &Type, width: FormalIndexWidth) -> Option<u64> {
    match ty {
        Type::Scalar(ScalarType::Index) => match width {
            FormalIndexWidth::Unknown => None,
            FormalIndexWidth::Bits32 => Some(u64::from(u32::MAX)),
            FormalIndexWidth::Bits64 => Some(u64::MAX),
        },
        Type::Scalar(ScalarType::U8) => Some(u64::from(u8::MAX)),
        Type::Scalar(ScalarType::U16) => Some(u64::from(u16::MAX)),
        Type::Scalar(ScalarType::U32) => Some(u64::from(u32::MAX)),
        Type::Scalar(ScalarType::U64) => Some(u64::MAX),
        _ => None,
    }
}

pub(super) fn affine<'source, S: Source<'source>>(
    source: &mut S,
    value: ValueId,
    depth: usize,
) -> std::result::Result<Option<Affine>, S::Error> {
    source.step()?;
    if depth >= 32 {
        return Ok(None);
    }
    if let Some(cached) = source.cached(value)? {
        return Ok(cached);
    }
    let Some(op) = source.operation(value)? else {
        return Ok(None);
    };
    let Some(value_maximum) = maximum(&op.results[0].ty, source.index_width()) else {
        return Ok(None);
    };
    let candidate = match &op.kind {
        OperationKind::Constant(value) => match value {
            Constant::Index(v) | Constant::U64(v) => Some(Affine::constant(i128::from(*v))),
            Constant::U32(v) => Some(Affine::constant(i128::from(*v))),
            Constant::U16(v) => Some(Affine::constant(i128::from(*v))),
            Constant::U8(v) => Some(Affine::constant(i128::from(*v))),
            _ => None,
        },
        OperationKind::Intrinsic(intrinsic)
            if matches!(
                intrinsic.kind,
                IntrinsicKind::InvocationIndex {
                    kind: IndexKind::Global,
                    axis: Axis::X
                }
            ) =>
        {
            Some(Affine {
                constant: 0,
                coefficient: 1,
            })
        }
        OperationKind::Binary { op, lhs, rhs } => {
            let a = affine(source, *lhs, depth + 1)?;
            let b = affine(source, *rhs, depth + 1)?;
            match (a, b) {
                (Some(a), Some(b)) => match op {
                    BinaryOp::Add => a.add(b),
                    BinaryOp::Subtract => a.subtract(b),
                    BinaryOp::Multiply if b.coefficient == 0 => a.scale(b.constant),
                    BinaryOp::Multiply if a.coefficient == 0 => b.scale(a.constant),
                    _ => None,
                },
                _ => None,
            }
        }
        OperationKind::Cast { kind, value, to } => {
            let input = source.operation(*value)?.map(|op| &op.results[0].ty);
            let admitted = match (kind, input, to) {
                (CastKind::ZeroExtend, Some(input), _) => {
                    maximum(input, source.index_width()).is_some()
                }
                (CastKind::Bitcast, Some(Type::Scalar(from)), Type::Scalar(to)) => {
                    source.index_width() == FormalIndexWidth::Bits64
                        && matches!(
                            (from, to),
                            (ScalarType::Index, ScalarType::U64)
                                | (ScalarType::U64, ScalarType::Index)
                        )
                }
                _ => false,
            };
            if admitted {
                affine(source, *value, depth + 1)?
            } else {
                None
            }
        }
        _ => None,
    }
    .filter(|value| value.fits(value_maximum, source.coordinate_last()));
    source.cache(value, candidate)?;
    Ok(candidate)
}

pub(super) fn predicate<'source, S: Source<'source>>(
    source: &mut S,
    value: ValueId,
    truth: bool,
    depth: usize,
    facts: &mut Vec<Fact>,
) -> std::result::Result<(), S::Error> {
    source.step()?;
    if depth >= 32 {
        return Ok(());
    }
    let Some(op) = source.operation(value)? else {
        return Ok(());
    };
    if op.results[0].ty != Type::BOOL {
        return Ok(());
    }
    let fact = match &op.kind {
        OperationKind::Constant(Constant::Bool(actual)) if *actual != truth => Some(Fact {
            expression: Affine::constant(1),
            equality: true,
        }),
        OperationKind::Unary {
            op: UnaryOp::Not,
            operand,
        } => {
            predicate(source, *operand, !truth, depth + 1, facts)?;
            None
        }
        OperationKind::Binary {
            op: BinaryOp::BitAnd,
            lhs,
            rhs,
        } if truth => {
            predicate(source, *lhs, true, depth + 1, facts)?;
            predicate(source, *rhs, true, depth + 1, facts)?;
            None
        }
        OperationKind::Binary {
            op: BinaryOp::BitOr,
            lhs,
            rhs,
        } if !truth => {
            predicate(source, *lhs, false, depth + 1, facts)?;
            predicate(source, *rhs, false, depth + 1, facts)?;
            None
        }
        OperationKind::Compare {
            predicate: comparison,
            lhs,
            rhs,
        } => {
            let left = affine(source, *lhs, 0)?;
            let right = affine(source, *rhs, 0)?;
            match (left, right) {
                (Some(left), Some(right)) => {
                    let comparison = if truth {
                        *comparison
                    } else {
                        negate(*comparison)
                    };
                    let (a, b, strict, equality) = match comparison {
                        ComparePredicate::Equal => (left, right, false, true),
                        ComparePredicate::LessThan => (left, right, true, false),
                        ComparePredicate::LessThanOrEqual => (left, right, false, false),
                        ComparePredicate::GreaterThan => (right, left, true, false),
                        ComparePredicate::GreaterThanOrEqual => (right, left, false, false),
                        ComparePredicate::NotEqual => return Ok(()),
                    };
                    a.subtract(b)
                        .and_then(|v| v.add(Affine::constant(i128::from(strict))))
                        .map(|expression| Fact {
                            expression,
                            equality,
                        })
                }
                _ => None,
            }
        }
        _ => None,
    };
    if let Some(fact) = fact {
        source.push_fact(facts, fact)?;
    }
    Ok(())
}
