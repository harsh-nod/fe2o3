use super::*;

#[derive(Clone, Copy)]
struct PointerExpression {
    allocation: FormalAllocationIdentity,
    byte_offset: AffineExpression,
}

#[derive(Clone)]
enum PointerDerivationFailure {
    AtAccess(ValueId),
    Located(FormalMemoryIncompleteReason),
}

impl PointerDerivationFailure {
    fn materialize(
        &self,
        access_location: FunctionOperationLocation,
    ) -> FormalMemoryIncompleteReason {
        match self {
            Self::AtAccess(pointer) => FormalMemoryIncompleteReason::UnsupportedPointerDerivation {
                location: access_location,
                pointer: *pointer,
            },
            Self::Located(reason) => reason.clone(),
        }
    }
}

type CachedPointerDerivation<T> = Result<T, PointerDerivationFailure>;

#[derive(Default)]
struct PointerDerivationCache {
    allocations: BTreeMap<ValueId, CachedPointerDerivation<FormalAllocationIdentity>>,
    expressions: BTreeMap<ValueId, CachedPointerDerivation<PointerExpression>>,
}

fn cache_pointer_allocation_failure(
    origin: ValueId,
    failure: &PointerDerivationFailure,
    reverse_dependencies: &BTreeMap<ValueId, Vec<ValueId>>,
    cache: &mut PointerDerivationCache,
) {
    let mut pending = vec![origin];
    let mut visited = BTreeSet::new();
    while let Some(value) = pending.pop() {
        if !visited.insert(value) {
            continue;
        }
        cache
            .allocations
            .entry(value)
            .or_insert_with(|| Err(failure.clone()));
        pending.extend(
            reverse_dependencies
                .get(&value)
                .into_iter()
                .flatten()
                .copied(),
        );
    }
}

pub(super) struct AccessDerivationContext<'analysis, 'module> {
    definitions: &'analysis Definitions<'module>,
    value_types: &'analysis BTreeMap<ValueId, Type>,
    allocation_by_value: &'analysis BTreeMap<ValueId, FormalAllocationIdentity>,
    private_load_sources: &'analysis BTreeMap<ValueId, ValueId>,
    pointer_derivations: PointerDerivationCache,
}

impl<'analysis, 'module> AccessDerivationContext<'analysis, 'module> {
    pub(super) fn new(
        definitions: &'analysis Definitions<'module>,
        value_types: &'analysis BTreeMap<ValueId, Type>,
        allocation_by_value: &'analysis BTreeMap<ValueId, FormalAllocationIdentity>,
        private_load_sources: &'analysis BTreeMap<ValueId, ValueId>,
    ) -> Self {
        Self {
            definitions,
            value_types,
            allocation_by_value,
            private_load_sources,
            pointer_derivations: PointerDerivationCache::default(),
        }
    }
}

pub(super) fn derive_access(
    location: FunctionOperationLocation,
    pointer: ValueId,
    kind: FormalMemoryAccessKind,
    access: MemoryAccess,
    invocations: InvocationRange1d,
    context: &mut AccessDerivationContext<'_, '_>,
) -> Result<FormalMemoryAccess, FormalMemoryIncompleteReason> {
    let byte_width = context
        .value_types
        .get(&pointer)
        .and_then(pointer_byte_width)
        .ok_or(FormalMemoryIncompleteReason::ElementWidthUnavailable { location, pointer })?;
    let pointer_expression = derive_pointer_expression(
        pointer,
        context.definitions,
        context.value_types,
        context.allocation_by_value,
        context.private_load_sources,
        &mut context.pointer_derivations,
        location,
    )?;
    Ok(FormalMemoryAccess {
        location,
        allocation: pointer_expression.allocation,
        kind,
        address_space: access.address_space,
        byte_offset: pointer_expression.byte_offset.into_byte_expression(),
        byte_width,
        alignment: u64::from(access.alignment),
        invocations,
    })
}

/// Retains the allocation-level read effect when a guarded address cannot be
/// represented by the affine extractor. The owner-held ranked proof remains
/// responsible for the predicate and bounds; this conservative effect prevents
/// that separate proof from erasing alias and race obligations.
pub(super) fn derive_conservative_guarded_access(
    location: FunctionOperationLocation,
    pointer: ValueId,
    access: MemoryAccess,
    invocations: InvocationRange1d,
    context: &mut AccessDerivationContext<'_, '_>,
) -> Result<FormalMemoryAccess, FormalMemoryIncompleteReason> {
    let byte_width = context
        .value_types
        .get(&pointer)
        .and_then(pointer_byte_width)
        .ok_or(FormalMemoryIncompleteReason::ElementWidthUnavailable { location, pointer })?;
    let allocation = derive_pointer_allocation(
        pointer,
        context.definitions,
        context.value_types,
        context.allocation_by_value,
        context.private_load_sources,
        &mut context.pointer_derivations,
        location,
    )?;
    Ok(FormalMemoryAccess {
        location,
        allocation,
        kind: FormalMemoryAccessKind::Read,
        address_space: access.address_space,
        byte_offset: ByteExpression::Unbounded,
        byte_width,
        alignment: u64::from(access.alignment),
        invocations,
    })
}

fn derive_pointer_allocation(
    pointer: ValueId,
    definitions: &Definitions<'_>,
    value_types: &BTreeMap<ValueId, Type>,
    allocation_by_value: &BTreeMap<ValueId, FormalAllocationIdentity>,
    private_load_sources: &BTreeMap<ValueId, ValueId>,
    cache: &mut PointerDerivationCache,
    access_location: FunctionOperationLocation,
) -> Result<FormalAllocationIdentity, FormalMemoryIncompleteReason> {
    derive_pointer_allocation_cached(
        pointer,
        definitions,
        value_types,
        allocation_by_value,
        private_load_sources,
        cache,
    )
    .map_err(|failure| failure.materialize(access_location))
}

fn derive_pointer_allocation_cached(
    pointer: ValueId,
    definitions: &Definitions<'_>,
    value_types: &BTreeMap<ValueId, Type>,
    allocation_by_value: &BTreeMap<ValueId, FormalAllocationIdentity>,
    private_load_sources: &BTreeMap<ValueId, ValueId>,
    cache: &mut PointerDerivationCache,
) -> CachedPointerDerivation<FormalAllocationIdentity> {
    if let Some(result) = cache.allocations.get(&pointer) {
        return result.clone();
    }

    let mut pending = vec![pointer];
    let mut visited = BTreeSet::new();
    let mut allocations = BTreeSet::new();
    let mut allocation_sources = BTreeSet::new();
    let mut reverse_dependencies = BTreeMap::<ValueId, Vec<ValueId>>::new();
    let mut failure_origin = None;
    let mut failure_covers_visited = false;
    let result = 'derivation: {
        while let Some(current) = pending.pop() {
            if !visited.insert(current) {
                continue;
            }
            if let Some(cached) = cache.allocations.get(&current) {
                match cached {
                    Ok(allocation) => {
                        allocations.insert(*allocation);
                        allocation_sources.insert(current);
                        if allocations.len() > 1 {
                            break 'derivation Err(PointerDerivationFailure::AtAccess(current));
                        }
                    }
                    Err(failure) => {
                        failure_origin = Some(current);
                        break 'derivation Err(failure.clone());
                    }
                }
                continue;
            }
            if let Some(inputs) = definitions.block_parameter_inputs.get(&current) {
                if inputs.is_empty() {
                    failure_origin = Some(current);
                    break 'derivation Err(PointerDerivationFailure::AtAccess(current));
                }
                for input in inputs {
                    reverse_dependencies
                        .entry(*input)
                        .or_default()
                        .push(current);
                }
                pending.extend(inputs.iter().copied());
                continue;
            }
            if let Some(allocation) = allocation_by_value.get(&current).copied()
                && matches!(
                    value_types.get(&current),
                    Some(Type::Pointer(_) | Type::Slice(_))
                )
            {
                allocations.insert(allocation);
                allocation_sources.insert(current);
                if allocations.len() > 1 {
                    break 'derivation Err(PointerDerivationFailure::AtAccess(current));
                }
                continue;
            }
            let Some((operation, definition_location)) = definitions.operations.get(&current)
            else {
                failure_origin = Some(current);
                break 'derivation Err(PointerDerivationFailure::AtAccess(current));
            };
            match &operation.kind {
                OperationKind::Cast {
                    kind: CastKind::RestrictPointerAccess,
                    value,
                    ..
                } => {
                    reverse_dependencies
                        .entry(*value)
                        .or_default()
                        .push(current);
                    pending.push(*value);
                }
                OperationKind::SliceData { slice } => {
                    reverse_dependencies
                        .entry(*slice)
                        .or_default()
                        .push(current);
                    pending.push(*slice);
                }
                OperationKind::GetElementPointer { base, .. } => {
                    reverse_dependencies.entry(*base).or_default().push(current);
                    pending.push(*base);
                }
                OperationKind::Load { .. } => {
                    let Some(source) = private_load_sources.get(&current).copied() else {
                        failure_origin = Some(current);
                        break 'derivation Err(PointerDerivationFailure::Located(
                            FormalMemoryIncompleteReason::UnsupportedPointerDerivation {
                                location: *definition_location,
                                pointer: current,
                            },
                        ));
                    };
                    reverse_dependencies
                        .entry(source)
                        .or_default()
                        .push(current);
                    pending.push(source);
                }
                _ => {
                    failure_origin = Some(current);
                    break 'derivation Err(PointerDerivationFailure::Located(
                        FormalMemoryIncompleteReason::UnsupportedPointerDerivation {
                            location: *definition_location,
                            pointer: current,
                        },
                    ));
                }
            }
        }

        let mut allocations = allocations.into_iter();
        let Some(allocation) = allocations.next() else {
            failure_covers_visited = true;
            break 'derivation Err(PointerDerivationFailure::AtAccess(pointer));
        };
        if allocations.next().is_some() {
            break 'derivation Err(PointerDerivationFailure::AtAccess(pointer));
        }
        let mut resolved = allocation_sources.iter().copied().collect::<Vec<_>>();
        let mut has_allocation_path = allocation_sources;
        while let Some(value) = resolved.pop() {
            for dependent in reverse_dependencies.get(&value).into_iter().flatten() {
                if has_allocation_path.insert(*dependent) {
                    resolved.push(*dependent);
                }
            }
        }
        for value in has_allocation_path {
            cache.allocations.entry(value).or_insert(Ok(allocation));
        }
        Ok(allocation)
    };
    if let Err(failure) = &result {
        if failure_covers_visited {
            for value in &visited {
                cache
                    .allocations
                    .entry(*value)
                    .or_insert_with(|| Err(failure.clone()));
            }
        } else if let Some(origin) = failure_origin {
            cache_pointer_allocation_failure(origin, failure, &reverse_dependencies, cache);
        }
    }
    cache.allocations.insert(pointer, result.clone());
    result
}

fn derive_pointer_expression(
    pointer: ValueId,
    definitions: &Definitions<'_>,
    value_types: &BTreeMap<ValueId, Type>,
    allocation_by_value: &BTreeMap<ValueId, FormalAllocationIdentity>,
    private_load_sources: &BTreeMap<ValueId, ValueId>,
    cache: &mut PointerDerivationCache,
    access_location: FunctionOperationLocation,
) -> Result<PointerExpression, FormalMemoryIncompleteReason> {
    derive_pointer_expression_cached(
        pointer,
        definitions,
        value_types,
        allocation_by_value,
        private_load_sources,
        cache,
    )
    .map_err(|failure| failure.materialize(access_location))
}

fn derive_pointer_expression_cached(
    pointer: ValueId,
    definitions: &Definitions<'_>,
    value_types: &BTreeMap<ValueId, Type>,
    allocation_by_value: &BTreeMap<ValueId, FormalAllocationIdentity>,
    private_load_sources: &BTreeMap<ValueId, ValueId>,
    cache: &mut PointerDerivationCache,
) -> CachedPointerDerivation<PointerExpression> {
    if let Some(result) = cache.expressions.get(&pointer) {
        return result.clone();
    }

    #[derive(Clone, Copy)]
    enum PointerWork {
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

    let unsupported = PointerDerivationFailure::AtAccess;
    let mut visiting = BTreeSet::new();
    let mut work = vec![PointerWork::Enter(pointer)];
    while let Some(item) = work.pop() {
        match item {
            PointerWork::Enter(unresolved) => {
                if cache.expressions.contains_key(&unresolved) {
                    continue;
                }
                let Some(value) = definitions.unique_ssa_origin(unresolved) else {
                    cache
                        .expressions
                        .insert(unresolved, Err(unsupported(unresolved)));
                    continue;
                };
                if value != unresolved {
                    work.push(PointerWork::Alias {
                        value: unresolved,
                        source: value,
                    });
                    work.push(PointerWork::Enter(value));
                    continue;
                }
                if !visiting.insert(value) {
                    cache.expressions.insert(value, Err(unsupported(value)));
                    continue;
                }
                if let Some(allocation) = allocation_by_value.get(&value).copied()
                    && matches!(value_types.get(&value), Some(Type::Pointer(_)))
                {
                    visiting.remove(&value);
                    cache.expressions.insert(
                        value,
                        Ok(PointerExpression {
                            allocation,
                            byte_offset: AffineExpression::ZERO,
                        }),
                    );
                    continue;
                }
                let Some((operation, definition_location)) = definitions.operations.get(&value)
                else {
                    visiting.remove(&value);
                    cache.expressions.insert(value, Err(unsupported(value)));
                    continue;
                };
                match &operation.kind {
                    OperationKind::Cast {
                        kind: CastKind::RestrictPointerAccess,
                        value: source,
                        ..
                    } => {
                        work.push(PointerWork::Alias {
                            value,
                            source: *source,
                        });
                        work.push(PointerWork::Enter(*source));
                    }
                    OperationKind::SliceData { slice } => {
                        visiting.remove(&value);
                        let expression = derive_pointer_allocation_cached(
                            *slice,
                            definitions,
                            value_types,
                            allocation_by_value,
                            private_load_sources,
                            cache,
                        )
                        .map(|allocation| PointerExpression {
                            allocation,
                            byte_offset: AffineExpression::ZERO,
                        });
                        cache.expressions.insert(value, expression);
                    }
                    OperationKind::GetElementPointer { base, offset } => {
                        work.push(PointerWork::Gep {
                            value,
                            base: *base,
                            offset: *offset,
                            location: *definition_location,
                        });
                        work.push(PointerWork::Enter(*base));
                    }
                    OperationKind::Load { .. } if private_load_sources.contains_key(&value) => {
                        let Some(source) = private_load_sources.get(&value).copied() else {
                            visiting.remove(&value);
                            cache.expressions.insert(
                                value,
                                Err(PointerDerivationFailure::Located(
                                    FormalMemoryIncompleteReason::UnsupportedPointerDerivation {
                                        location: *definition_location,
                                        pointer: value,
                                    },
                                )),
                            );
                            continue;
                        };
                        work.push(PointerWork::Alias { value, source });
                        work.push(PointerWork::Enter(source));
                    }
                    _ => {
                        visiting.remove(&value);
                        cache.expressions.insert(
                            value,
                            Err(PointerDerivationFailure::Located(
                                FormalMemoryIncompleteReason::UnsupportedPointerDerivation {
                                    location: *definition_location,
                                    pointer: value,
                                },
                            )),
                        );
                    }
                }
            }
            PointerWork::Alias { value, source } => {
                visiting.remove(&value);
                if cache.expressions.contains_key(&value) {
                    continue;
                }
                let expression = cache
                    .expressions
                    .get(&source)
                    .cloned()
                    .unwrap_or_else(|| Err(unsupported(source)));
                cache.expressions.insert(value, expression);
            }
            PointerWork::Gep {
                value,
                base,
                offset,
                location,
            } => {
                visiting.remove(&value);
                if cache.expressions.contains_key(&value) {
                    continue;
                }
                let expression = (|| {
                    let base_expression = cache
                        .expressions
                        .get(&base)
                        .cloned()
                        .unwrap_or_else(|| Err(unsupported(base)))?;
                    let element_width = value_types.get(&base).and_then(pointer_byte_width).ok_or(
                        PointerDerivationFailure::Located(
                            FormalMemoryIncompleteReason::ElementWidthUnavailable {
                                location,
                                pointer: base,
                            },
                        ),
                    )?;
                    let index = derive_affine_index(offset, definitions).map_err(|error| {
                        PointerDerivationFailure::Located(match error {
                            IndexExpressionError::Unsupported => {
                                FormalMemoryIncompleteReason::UnsupportedIndexExpression {
                                    location,
                                    index: offset,
                                    allocation: base_expression.allocation,
                                }
                            }
                            IndexExpressionError::Overflow => {
                                FormalMemoryIncompleteReason::AddressArithmeticOverflow { location }
                            }
                        })
                    })?;
                    let byte_delta = index.checked_multiply_constant(element_width).ok_or(
                        PointerDerivationFailure::Located(
                            FormalMemoryIncompleteReason::AddressArithmeticOverflow { location },
                        ),
                    )?;
                    let byte_offset = base_expression.byte_offset.checked_add(byte_delta).ok_or(
                        PointerDerivationFailure::Located(
                            FormalMemoryIncompleteReason::AddressArithmeticOverflow { location },
                        ),
                    )?;
                    Ok(PointerExpression {
                        allocation: base_expression.allocation,
                        byte_offset,
                    })
                })();
                cache.expressions.insert(value, expression);
            }
        }
    }
    cache
        .expressions
        .get(&pointer)
        .cloned()
        .unwrap_or_else(|| Err(unsupported(pointer)))
}
