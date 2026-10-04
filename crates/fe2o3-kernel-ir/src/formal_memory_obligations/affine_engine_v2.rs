//! The original explicit-stack affine evaluator, with pluggable owned state.
use super::*;
use std::convert::Infallible;

pub(super) type Expression = Result<AffineExpression, IndexExpressionError>;

pub(super) trait State<'source> {
    type Error;
    fn root(&mut self) -> Result<Option<ValueId>, Self::Error>;
    fn origin(&mut self, value: ValueId) -> Result<Option<ValueId>, Self::Error>;
    fn operation(&mut self, value: ValueId) -> Result<Option<&'source Operation>, Self::Error>;
    fn cached(&mut self, value: ValueId) -> Result<Option<Expression>, Self::Error>;
    fn cache(&mut self, value: ValueId, expression: Expression) -> Result<(), Self::Error>;
    fn enter(&mut self, value: ValueId) -> Result<bool, Self::Error>;
    fn leave(&mut self, value: ValueId) -> Result<(), Self::Error>;
    fn push(&mut self, work: AffineWork) -> Result<(), Self::Error>;
    fn pop(&mut self) -> Result<Option<AffineWork>, Self::Error>;
    fn step(&mut self, work: usize) -> Result<(), Self::Error>;
}

fn operand<'source, S: State<'source>>(
    state: &mut S,
    value: ValueId,
) -> Result<Expression, S::Error> {
    let Some(value) = state.origin(value)? else {
        return Ok(Err(IndexExpressionError::Unsupported));
    };
    Ok(state
        .cached(value)?
        .unwrap_or(Err(IndexExpressionError::Unsupported)))
}

fn expression<'source, S: State<'source>>(
    state: &mut S,
    value: ValueId,
) -> Result<Expression, S::Error> {
    state.step(1)?;
    let Some(operation) = state
        .operation(value)?
        .filter(|op| affine_result_is_supported(op))
    else {
        return Ok(Err(IndexExpressionError::Unsupported));
    };
    let result = match &operation.kind {
        OperationKind::Constant(Constant::Index(value))
            if operation.results[0].ty == Type::INDEX =>
        {
            Ok(AffineExpression::constant(*value))
        }
        OperationKind::Constant(Constant::U64(value))
            if operation.results[0].ty == Type::Scalar(ScalarType::U64) =>
        {
            Ok(AffineExpression::constant(*value))
        }
        OperationKind::Cast {
            kind: CastKind::Bitcast,
            value,
            to,
        } if *to == Type::INDEX => {
            let Some(origin) = state.origin(*value)? else {
                return Ok(Err(IndexExpressionError::Unsupported));
            };
            let Some(source) = state.operation(origin)? else {
                return Ok(Err(IndexExpressionError::Unsupported));
            };
            if !matches!(source.results.as_slice(), [result] if result.ty == Type::Scalar(ScalarType::U64))
                || !matches!(source.kind, OperationKind::Constant(Constant::U64(_)))
            {
                return Ok(Err(IndexExpressionError::Unsupported));
            }
            match state
                .cached(origin)?
                .unwrap_or(Err(IndexExpressionError::Unsupported))
            {
                Ok(literal) if literal.invocation_coefficient == 0 => Ok(literal),
                Ok(_) => Err(IndexExpressionError::Unsupported),
                Err(error) => Err(error),
            }
        }
        OperationKind::Intrinsic(intrinsic)
            if intrinsic.kind
                == (IntrinsicKind::InvocationIndex {
                    kind: IndexKind::Global,
                    axis: Axis::X,
                }) =>
        {
            Ok(AffineExpression::INVOCATION)
        }
        OperationKind::Binary { op, lhs, rhs } => {
            let lhs = match operand(state, *lhs)? {
                Ok(value) => value,
                Err(error) => return Ok(Err(error)),
            };
            let rhs = match operand(state, *rhs)? {
                Ok(value) => value,
                Err(error) => return Ok(Err(error)),
            };
            match op {
                BinaryOp::Add => lhs.checked_add(rhs).ok_or(IndexExpressionError::Overflow),
                BinaryOp::Multiply if lhs.invocation_coefficient == 0 => rhs
                    .checked_multiply_constant(lhs.constant)
                    .ok_or(IndexExpressionError::Overflow),
                BinaryOp::Multiply if rhs.invocation_coefficient == 0 => lhs
                    .checked_multiply_constant(rhs.constant)
                    .ok_or(IndexExpressionError::Overflow),
                _ => Err(IndexExpressionError::Unsupported),
            }
        }
        _ => Err(IndexExpressionError::Unsupported),
    };
    Ok(result)
}

struct Legacy<'source, 'map> {
    operations: &'map BTreeMap<ValueId, (&'source Operation, FunctionOperationLocation)>,
    origins: &'map BTreeMap<ValueId, Option<ValueId>>,
    expressions: BTreeMap<ValueId, Expression>,
    visiting: BTreeSet<ValueId>,
    roots: std::vec::IntoIter<ValueId>,
    work: Vec<AffineWork>,
}
impl<'source> State<'source> for Legacy<'source, '_> {
    type Error = Infallible;
    fn root(&mut self) -> Result<Option<ValueId>, Infallible> {
        Ok(self.roots.next())
    }
    fn origin(&mut self, value: ValueId) -> Result<Option<ValueId>, Infallible> {
        Ok(self.origins.get(&value).copied().unwrap_or(Some(value)))
    }
    fn operation(&mut self, value: ValueId) -> Result<Option<&'source Operation>, Infallible> {
        Ok(self.operations.get(&value).map(|(operation, _)| *operation))
    }
    fn cached(&mut self, value: ValueId) -> Result<Option<Expression>, Infallible> {
        Ok(self.expressions.get(&value).copied())
    }
    fn cache(&mut self, value: ValueId, expression: Expression) -> Result<(), Infallible> {
        self.expressions.insert(value, expression);
        Ok(())
    }
    fn enter(&mut self, value: ValueId) -> Result<bool, Infallible> {
        Ok(self.visiting.insert(value))
    }
    fn leave(&mut self, value: ValueId) -> Result<(), Infallible> {
        self.visiting.remove(&value);
        Ok(())
    }
    fn push(&mut self, work: AffineWork) -> Result<(), Infallible> {
        self.work.push(work);
        Ok(())
    }
    fn pop(&mut self) -> Result<Option<AffineWork>, Infallible> {
        Ok(self.work.pop())
    }
    fn step(&mut self, _: usize) -> Result<(), Infallible> {
        Ok(())
    }
}

pub(super) fn legacy<'source>(
    operations: &BTreeMap<ValueId, (&'source Operation, FunctionOperationLocation)>,
    origins: &BTreeMap<ValueId, Option<ValueId>>,
) -> BTreeMap<ValueId, Expression> {
    let roots = operations
        .iter()
        .filter_map(|(value, (operation, _))| {
            affine_result_is_supported(operation).then_some(*value)
        })
        .collect::<Vec<_>>()
        .into_iter();
    let mut state = Legacy {
        operations,
        origins,
        expressions: BTreeMap::new(),
        visiting: BTreeSet::new(),
        roots,
        work: Vec::new(),
    };
    match run(&mut state) {
        Ok(()) => state.expressions,
        Err(never) => match never {},
    }
}

pub(super) fn run<'source, S: State<'source>>(state: &mut S) -> Result<(), S::Error> {
    while let Some(root) = state.root()? {
        if state.cached(root)?.is_some() {
            continue;
        }
        state.push(AffineWork::Enter(root))?;
        while let Some(item) = state.pop()? {
            state.step(1)?;
            match item {
                AffineWork::Enter(value) => {
                    let Some(value) = state.origin(value)? else {
                        continue;
                    };
                    if state.cached(value)?.is_some() {
                        continue;
                    }
                    if !state.enter(value)? {
                        state.cache(value, Err(IndexExpressionError::Unsupported))?;
                        continue;
                    }
                    state.push(AffineWork::Finish(value))?;
                    let Some(operation) = state.operation(value)? else {
                        continue;
                    };
                    if !affine_result_is_supported(operation) {
                        continue;
                    }
                    let dependencies = match &operation.kind {
                        OperationKind::Binary { lhs, rhs, .. } => [Some(*rhs), Some(*lhs)],
                        OperationKind::Cast {
                            kind: CastKind::Bitcast,
                            value,
                            to,
                        } if *to == Type::INDEX => [Some(*value), None],
                        _ => [None, None],
                    };
                    for dependency in dependencies.into_iter().flatten() {
                        state.step(1)?;
                        if let Some(dependency) = state.origin(dependency)? {
                            state.push(AffineWork::Enter(dependency))?;
                        }
                    }
                }
                AffineWork::Finish(value) => {
                    state.leave(value)?;
                    if state.cached(value)?.is_some() {
                        continue;
                    }
                    let expression = expression(state, value)?;
                    state.cache(value, expression)?;
                }
            }
        }
    }
    Ok(())
}
