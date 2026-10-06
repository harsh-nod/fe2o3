//! Shared allocation closure and affine pointer traversal, in original visit order.
use super::*;

// Expression traversal can enter one allocation closure, whose three sets
// must remain disjoint from the outer expression visiting set.
#[derive(Clone, Copy)]
pub(in crate::formal_memory_obligations) enum SetRole {
    AllocationVisited,
    AllocationSources,
    AllocationFailure,
    ExpressionVisiting,
}

pub(in crate::formal_memory_obligations) trait State<'source> {
    type Error;
    type Set;
    fn step(&mut self, work: usize) -> Result<(), Self::Error>;
    fn empty<T: Copy>(&mut self) -> Result<Vec<T>, Self::Error>;
    fn push<T: Copy>(&mut self, rows: &mut Vec<T>, value: T) -> Result<(), Self::Error>;
    fn sort<T: Copy>(
        &mut self,
        rows: &mut [T],
        key: impl Fn(&T) -> u32 + Copy,
    ) -> Result<(), Self::Error>;
    fn find<T>(
        &mut self,
        rows: &[T],
        compare: impl Fn(&T) -> std::cmp::Ordering,
    ) -> Result<Option<usize>, Self::Error>;
    fn set(&mut self, role: SetRole) -> Result<Self::Set, Self::Error>;
    fn insert(&mut self, set: &mut Self::Set, value: ValueId) -> Result<bool, Self::Error>;
    fn remove(&mut self, set: &mut Self::Set, value: ValueId) -> Result<(), Self::Error>;
    fn members(&mut self, set: &Self::Set) -> Result<Vec<ValueId>, Self::Error>;
    fn phi_count(&mut self, value: ValueId) -> Result<Option<usize>, Self::Error>;
    fn phi_input(&mut self, value: ValueId, ordinal: usize) -> Result<ValueId, Self::Error>;
    fn origin(&mut self, value: ValueId) -> Result<Option<ValueId>, Self::Error>;
    fn root(
        &mut self,
        value: ValueId,
        slices: bool,
    ) -> Result<Option<FormalAllocationIdentity>, Self::Error>;
    fn operation(
        &mut self,
        value: ValueId,
    ) -> Result<Option<(&'source Operation, FunctionOperationLocation)>, Self::Error>;
    fn valid_cast(&mut self, operation: &Operation, source: ValueId) -> Result<bool, Self::Error>;
    fn load(&mut self, value: ValueId) -> Result<Option<ValueId>, Self::Error>;
    fn width(&mut self, value: ValueId) -> Result<Option<u64>, Self::Error>;
    fn affine(
        &mut self,
        value: ValueId,
    ) -> Result<Result<AffineExpression, IndexExpressionError>, Self::Error>;
    fn allocation(
        &mut self,
        value: ValueId,
    ) -> Result<Option<CachedPointerDerivation<FormalAllocationIdentity>>, Self::Error>;
    fn cache_allocation(
        &mut self,
        value: ValueId,
        result: CachedPointerDerivation<FormalAllocationIdentity>,
        replace: bool,
    ) -> Result<(), Self::Error>;
    fn expression(
        &mut self,
        value: ValueId,
    ) -> Result<Option<CachedPointerDerivation<PointerExpression>>, Self::Error>;
    fn cache_expression(
        &mut self,
        value: ValueId,
        result: CachedPointerDerivation<PointerExpression>,
    ) -> Result<(), Self::Error>;
}

#[derive(Clone, Copy)]
struct Edge {
    source: ValueId,
    dependent: ValueId,
}

fn dependents<'source, S: State<'source>>(
    value: ValueId,
    edges: &[Edge],
    pending: &mut Vec<ValueId>,
    seen: &mut S::Set,
    state: &mut S,
) -> Result<(), S::Error> {
    // Stable sorting preserves the original per-source insertion order. Find
    // the end of the range, then walk its beginning before pushing forwards.
    let Some(last) = state.find(edges, |edge| edge.source.cmp(&value))? else {
        return Ok(());
    };
    let mut first = last;
    while first > 0 {
        state.step(1)?;
        if edges[first - 1].source != value {
            break;
        }
        first -= 1;
    }
    for edge in &edges[first..=last] {
        state.step(1)?;
        if state.insert(seen, edge.dependent)? {
            state.push(pending, edge.dependent)?;
        }
    }
    Ok(())
}

pub(in crate::formal_memory_obligations) fn allocation<'source, S: State<'source>>(
    pointer: ValueId,
    state: &mut S,
) -> Result<CachedPointerDerivation<FormalAllocationIdentity>, S::Error> {
    if let Some(result) = state.allocation(pointer)? {
        return Ok(result);
    }
    let mut pending = state.empty()?;
    state.push(&mut pending, pointer)?;
    let mut visited = state.set(SetRole::AllocationVisited)?;
    let mut sources = state.set(SetRole::AllocationSources)?;
    let mut edges = state.empty()?;
    let mut found = None;
    let mut failure_origin = None;
    let mut failure_covers_visited = false;
    let result = 'derivation: {
        loop {
            state.step(1)?;
            let Some(current) = pending.pop() else { break };
            if !state.insert(&mut visited, current)? {
                continue;
            }
            if let Some(cached) = state.allocation(current)? {
                match cached {
                    Ok(allocation) => {
                        state.insert(&mut sources, current)?;
                        if found.is_some_and(|old| old != allocation) {
                            break 'derivation Err(PointerDerivationFailure::AtAccess(current));
                        }
                        found = Some(allocation);
                    }
                    Err(failure) => {
                        failure_origin = Some(current);
                        break 'derivation Err(failure);
                    }
                }
                continue;
            }
            if let Some(count) = state.phi_count(current)? {
                if count == 0 {
                    failure_origin = Some(current);
                    break 'derivation Err(PointerDerivationFailure::AtAccess(current));
                }
                for ordinal in 0..count {
                    state.step(1)?;
                    let input = state.phi_input(current, ordinal)?;
                    state.push(
                        &mut edges,
                        Edge {
                            source: input,
                            dependent: current,
                        },
                    )?;
                    state.push(&mut pending, input)?;
                }
                continue;
            }
            if let Some(allocation) = state.root(current, true)? {
                state.insert(&mut sources, current)?;
                if found.is_some_and(|old| old != allocation) {
                    break 'derivation Err(PointerDerivationFailure::AtAccess(current));
                }
                found = Some(allocation);
                continue;
            }
            let Some((operation, location)) = state.operation(current)? else {
                failure_origin = Some(current);
                break 'derivation Err(PointerDerivationFailure::AtAccess(current));
            };
            let dependency = match &operation.kind {
                OperationKind::Cast {
                    kind:
                        CastKind::RestrictPointerAccess
                        | CastKind::PointerToGeneric
                        | CastKind::SliceToGeneric,
                    value,
                    ..
                } => {
                    if !state.valid_cast(operation, *value)? {
                        failure_origin = Some(current);
                        break 'derivation Err(PointerDerivationFailure::AtAccess(current));
                    }
                    *value
                }
                OperationKind::SliceData { slice } => *slice,
                OperationKind::GetElementPointer { base, .. } => *base,
                OperationKind::Load { .. } => {
                    let Some(source) = state.load(current)? else {
                        failure_origin = Some(current);
                        break 'derivation Err(PointerDerivationFailure::Unsupported {
                            location,
                            pointer: current,
                        });
                    };
                    source
                }
                _ => {
                    failure_origin = Some(current);
                    break 'derivation Err(PointerDerivationFailure::Unsupported {
                        location,
                        pointer: current,
                    });
                }
            };
            state.push(
                &mut edges,
                Edge {
                    source: dependency,
                    dependent: current,
                },
            )?;
            state.push(&mut pending, dependency)?;
        }
        let Some(allocation) = found else {
            failure_covers_visited = true;
            break 'derivation Err(PointerDerivationFailure::AtAccess(pointer));
        };
        state.sort(&mut edges, |edge| edge.source.0)?;
        let mut resolved = state.members(&sources)?;
        loop {
            state.step(1)?;
            let Some(value) = resolved.pop() else { break };
            dependents(value, &edges, &mut resolved, &mut sources, state)?;
        }
        for value in state.members(&sources)? {
            state.step(1)?;
            state.cache_allocation(value, Ok(allocation), false)?;
        }
        Ok(allocation)
    };
    if let Err(failure) = result {
        if failure_covers_visited {
            for value in state.members(&visited)? {
                state.step(1)?;
                state.cache_allocation(value, Err(failure), false)?;
            }
        } else if let Some(origin) = failure_origin {
            state.sort(&mut edges, |edge| edge.source.0)?;
            let mut pending = state.empty()?;
            let mut seen = state.set(SetRole::AllocationFailure)?;
            state.insert(&mut seen, origin)?;
            state.push(&mut pending, origin)?;
            loop {
                state.step(1)?;
                let Some(value) = pending.pop() else { break };
                state.cache_allocation(value, Err(failure), false)?;
                dependents(value, &edges, &mut pending, &mut seen, state)?;
            }
        }
    }
    state.cache_allocation(pointer, result, true)?;
    Ok(result)
}

#[derive(Clone, Copy)]
pub(in crate::formal_memory_obligations) enum PointerWork {
    Enter(ValueId),
    Alias {
        value: ValueId,
        source: ValueId,
    },
    Gep {
        value: ValueId,
        base: ValueId,
        offset: ValueId,
        location: FunctionOperationLocation,
    },
}

pub(in crate::formal_memory_obligations) fn expression<'source, S: State<'source>>(
    pointer: ValueId,
    state: &mut S,
) -> Result<CachedPointerDerivation<PointerExpression>, S::Error> {
    if let Some(result) = state.expression(pointer)? {
        return Ok(result);
    }
    let mut visiting = state.set(SetRole::ExpressionVisiting)?;
    let mut work = state.empty()?;
    state.push(&mut work, PointerWork::Enter(pointer))?;
    loop {
        state.step(1)?;
        let Some(item) = work.pop() else { break };
        match item {
            PointerWork::Enter(unresolved) => {
                if state.expression(unresolved)?.is_some() {
                    continue;
                }
                let Some(value) = state.origin(unresolved)? else {
                    state.cache_expression(
                        unresolved,
                        Err(PointerDerivationFailure::AtAccess(unresolved)),
                    )?;
                    continue;
                };
                if value != unresolved {
                    state.push(
                        &mut work,
                        PointerWork::Alias {
                            value: unresolved,
                            source: value,
                        },
                    )?;
                    state.push(&mut work, PointerWork::Enter(value))?;
                    continue;
                }
                if !state.insert(&mut visiting, value)? {
                    state
                        .cache_expression(value, Err(PointerDerivationFailure::AtAccess(value)))?;
                    continue;
                }
                if let Some(allocation) = state.root(value, false)? {
                    state.remove(&mut visiting, value)?;
                    state.cache_expression(
                        value,
                        Ok(PointerExpression {
                            allocation,
                            byte_offset: AffineExpression::ZERO,
                        }),
                    )?;
                    continue;
                }
                let Some((operation, location)) = state.operation(value)? else {
                    state.remove(&mut visiting, value)?;
                    state
                        .cache_expression(value, Err(PointerDerivationFailure::AtAccess(value)))?;
                    continue;
                };
                match &operation.kind {
                    OperationKind::Cast {
                        kind:
                            CastKind::RestrictPointerAccess
                            | CastKind::PointerToGeneric
                            | CastKind::SliceToGeneric,
                        value: source,
                        ..
                    } => {
                        if !state.valid_cast(operation, *source)? {
                            state.remove(&mut visiting, value)?;
                            state.cache_expression(
                                value,
                                Err(PointerDerivationFailure::AtAccess(value)),
                            )?;
                            continue;
                        }
                        state.push(
                            &mut work,
                            PointerWork::Alias {
                                value,
                                source: *source,
                            },
                        )?;
                        state.push(&mut work, PointerWork::Enter(*source))?;
                    }
                    OperationKind::SliceData { slice } => {
                        state.remove(&mut visiting, value)?;
                        let result =
                            allocation(*slice, state)?.map(|allocation| PointerExpression {
                                allocation,
                                byte_offset: AffineExpression::ZERO,
                            });
                        state.cache_expression(value, result)?;
                    }
                    OperationKind::GetElementPointer { base, offset } => {
                        state.push(
                            &mut work,
                            PointerWork::Gep {
                                value,
                                base: *base,
                                offset: *offset,
                                location,
                            },
                        )?;
                        state.push(&mut work, PointerWork::Enter(*base))?;
                    }
                    OperationKind::Load { .. } => {
                        if let Some(source) = state.load(value)? {
                            state.push(&mut work, PointerWork::Alias { value, source })?;
                            state.push(&mut work, PointerWork::Enter(source))?;
                        } else {
                            state.remove(&mut visiting, value)?;
                            state.cache_expression(
                                value,
                                Err(PointerDerivationFailure::Unsupported {
                                    location,
                                    pointer: value,
                                }),
                            )?;
                        }
                    }
                    _ => {
                        state.remove(&mut visiting, value)?;
                        state.cache_expression(
                            value,
                            Err(PointerDerivationFailure::Unsupported {
                                location,
                                pointer: value,
                            }),
                        )?;
                    }
                }
            }
            PointerWork::Alias { value, source } => {
                state.remove(&mut visiting, value)?;
                if state.expression(value)?.is_some() {
                    continue;
                }
                let result = state
                    .expression(source)?
                    .unwrap_or(Err(PointerDerivationFailure::AtAccess(source)));
                state.cache_expression(value, result)?;
            }
            PointerWork::Gep {
                value,
                base,
                offset,
                location,
            } => {
                state.remove(&mut visiting, value)?;
                if state.expression(value)?.is_some() {
                    continue;
                }
                let result = 'derive: {
                    let base_expression = match state
                        .expression(base)?
                        .unwrap_or(Err(PointerDerivationFailure::AtAccess(base)))
                    {
                        Ok(result) => result,
                        Err(error) => break 'derive Err(error),
                    };
                    let Some(width) = state.width(base)? else {
                        break 'derive Err(PointerDerivationFailure::Width {
                            location,
                            pointer: base,
                        });
                    };
                    let index = match state.affine(offset)? {
                        Ok(result) => result,
                        Err(IndexExpressionError::Unsupported) => {
                            break 'derive Err(PointerDerivationFailure::Index {
                                location,
                                index: offset,
                                allocation: base_expression.allocation,
                            });
                        }
                        Err(IndexExpressionError::Overflow) => {
                            break 'derive Err(PointerDerivationFailure::Overflow { location });
                        }
                    };
                    state.step(2)?;
                    let Some(byte_delta) = index.checked_multiply_constant(width) else {
                        break 'derive Err(PointerDerivationFailure::Overflow { location });
                    };
                    let Some(byte_offset) = base_expression.byte_offset.checked_add(byte_delta)
                    else {
                        break 'derive Err(PointerDerivationFailure::Overflow { location });
                    };
                    Ok(PointerExpression {
                        allocation: base_expression.allocation,
                        byte_offset,
                    })
                };
                state.cache_expression(value, result)?;
            }
        }
    }
    Ok(state
        .expression(pointer)?
        .unwrap_or(Err(PointerDerivationFailure::AtAccess(pointer))))
}
