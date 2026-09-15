//! Closed arithmetic description only. Callers retain responsibility for source,
//! exclusive-output custody, 64-bit usize identity and eligible guard origins.

use super::{
    ReferenceBinaryOpV1 as B, ReferenceBindingErrorV1 as Error, ReferenceConstantV1 as C,
    ReferenceEffectExpressionV1 as X, ReferenceGuardAtomV1 as A, ReferencePathPredicateV1,
    ReferenceScalarTypeV1 as T, ReferenceSymbolicWorkBudgetV2 as Work,
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct CompactRowsUsize1D {
    axis: u32,
    divisor: u64,
    stride: u64,
}

impl CompactRowsUsize1D {
    /// Recognizes exactly `(Point0 / D) * S + Point0 % D`. A qualifying
    /// switch must come from an eligible original branch, not a normalized
    /// assertion; this expression/predicate API cannot recover erased origins.
    pub(crate) fn from_expression(
        expression: &X,
        guard: &ReferencePathPredicateV1,
        work: &mut Work,
    ) -> Result<Option<Self>, Error> {
        let Some((scaled, remainder)) = binary(expression, B::Add, work)? else {
            return Ok(None);
        };
        let Some((quotient, stride)) = binary(scaled, B::Multiply, work)? else {
            return Ok(None);
        };
        let Some(divisor) = point_divisor(quotient, B::Divide, work)? else {
            return Ok(None);
        };
        let Some(stride) = literal(stride, work)? else {
            return Ok(None);
        };
        let Some(remainder_divisor) = point_divisor(remainder, B::Remainder, work)? else {
            return Ok(None);
        };
        if stride == 0 || stride > divisor || remainder_divisor != divisor {
            return Ok(None);
        }
        work.charge_v2(1)?;
        if guard.clauses.is_empty() {
            return Ok(None);
        }
        if stride != divisor {
            for clause in &guard.clauses {
                work.charge_v2(1)?;
                let mut bounded = false;
                for atom in &clause.atoms {
                    work.charge_v2(1)?;
                    if active_remainder_bound(atom, divisor, stride, work)? {
                        bounded = true;
                        break;
                    }
                }
                if !bounded {
                    return Ok(None);
                }
            }
        }
        Ok(Some(Self {
            axis: 0,
            divisor,
            stride,
        }))
    }

    pub(crate) const fn axis(&self) -> u32 {
        self.axis
    }

    pub(crate) const fn divisor(&self) -> u64 {
        self.divisor
    }

    pub(crate) const fn stride(&self) -> u64 {
        self.stride
    }

    pub(crate) fn expression(&self, work: &mut Work) -> Result<X, Error> {
        // Precharge every materialized node before the first allocation.
        work.charge_v2(9)?;
        let point = || X::PointCoordinate { axis: self.axis };
        let constant = |bits| {
            X::Constant(C::Scalar {
                scalar: T::Usize,
                bits: u128::from(bits),
            })
        };
        let binary = |operation, lhs, rhs| X::Binary {
            operation,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
            checked: false,
        };
        Ok(binary(
            B::Add,
            binary(
                B::Multiply,
                binary(B::Divide, point(), constant(self.divisor)),
                constant(self.stride),
            ),
            binary(B::Remainder, point(), constant(self.divisor)),
        ))
    }
}

fn binary<'a>(
    expression: &'a X,
    expected: B,
    work: &mut Work,
) -> Result<Option<(&'a X, &'a X)>, Error> {
    work.charge_v2(1)?;
    Ok(match expression {
        X::Binary {
            operation,
            lhs,
            rhs,
            checked: false,
        } if *operation == expected => Some((lhs, rhs)),
        _ => None,
    })
}

fn literal(expression: &X, work: &mut Work) -> Result<Option<u64>, Error> {
    work.charge_v2(1)?;
    Ok(match expression {
        X::Constant(C::Scalar {
            scalar: T::Usize,
            bits,
        }) => u64::try_from(*bits).ok(),
        _ => None,
    })
}

fn point_divisor(expression: &X, operation: B, work: &mut Work) -> Result<Option<u64>, Error> {
    let Some((point, divisor)) = binary(expression, operation, work)? else {
        return Ok(None);
    };
    work.charge_v2(1)?;
    if !matches!(point, X::PointCoordinate { axis: 0 }) {
        return Ok(None);
    }
    Ok(literal(divisor, work)?.filter(|divisor| *divisor != 0))
}

fn active_remainder_bound(
    atom: &A,
    divisor: u64,
    stride: u64,
    work: &mut Work,
) -> Result<bool, Error> {
    let A::SwitchValueSet {
        discriminant,
        values,
        inside_set: false,
    } = atom
    else {
        return Ok(false);
    };
    if values.as_ref() != [0] {
        return Ok(false);
    }
    let Some((remainder, threshold)) = binary(discriminant, B::LessThan, work)? else {
        return Ok(false);
    };
    if point_divisor(remainder, B::Remainder, work)? != Some(divisor) {
        return Ok(false);
    }
    Ok(literal(threshold, work)?.is_some_and(|threshold| threshold <= stride))
}

#[cfg(test)]
#[path = "compact_row_coordinate_v1/tests.rs"]
mod tests;
