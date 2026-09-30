use super::*;

pub(super) enum AccessDerivationError<E = GuardedResourceErrorV1> {
    Incomplete(FormalMemoryIncompleteReason),
    Resource(E),
}
impl<E> From<FormalMemoryIncompleteReason> for AccessDerivationError<E> {
    fn from(reason: FormalMemoryIncompleteReason) -> Self {
        Self::Incomplete(reason)
    }
}

#[path = "access_engine_v18.rs"]
pub(super) mod access_engine;
impl From<GuardedResourceErrorV1> for AccessDerivationError {
    fn from(error: GuardedResourceErrorV1) -> Self {
        Self::Resource(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PointerExpression {
    pub(super) allocation: FormalAllocationIdentity,
    pub(super) byte_offset: AffineExpression,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PointerDerivationFailure {
    AtAccess(ValueId),
    Unsupported {
        location: FunctionOperationLocation,
        pointer: ValueId,
    },
    Width {
        location: FunctionOperationLocation,
        pointer: ValueId,
    },
    Index {
        location: FunctionOperationLocation,
        index: ValueId,
        allocation: FormalAllocationIdentity,
    },
    Overflow {
        location: FunctionOperationLocation,
    },
}

impl PointerDerivationFailure {
    pub(super) fn materialize(
        self,
        access_location: FunctionOperationLocation,
    ) -> FormalMemoryIncompleteReason {
        match self {
            Self::AtAccess(pointer) => FormalMemoryIncompleteReason::UnsupportedPointerDerivation {
                location: access_location,
                pointer,
            },
            Self::Unsupported { location, pointer } => {
                FormalMemoryIncompleteReason::UnsupportedPointerDerivation { location, pointer }
            }
            Self::Width { location, pointer } => {
                FormalMemoryIncompleteReason::ElementWidthUnavailable { location, pointer }
            }
            Self::Index {
                location,
                index,
                allocation,
            } => FormalMemoryIncompleteReason::UnsupportedIndexExpression {
                location,
                index,
                allocation,
            },
            Self::Overflow { location } => {
                FormalMemoryIncompleteReason::AddressArithmeticOverflow { location }
            }
        }
    }
}

pub(super) type CachedPointerDerivation<T> = Result<T, PointerDerivationFailure>;

#[derive(Default)]
struct PointerDerivationCache {
    allocations: BTreeMap<ValueId, CachedPointerDerivation<FormalAllocationIdentity>>,
    expressions: BTreeMap<ValueId, CachedPointerDerivation<PointerExpression>>,
}

#[path = "pointer_engine_v18.rs"]
pub(super) mod engine;
#[path = "pointer_legacy_v18.rs"]
mod legacy;

#[cfg(test)]
include!("pointer_observation_v18_tests.rs");

pub(super) struct AccessDerivationContext<'analysis, 'module> {
    definitions: &'analysis Definitions<'module>,
    value_types: &'analysis BTreeMap<ValueId, Type>,
    allocations: &'analysis [FormalAllocationParameter],
    allocation_by_value: &'analysis BTreeMap<ValueId, FormalAllocationIdentity>,
    private_load_sources: &'analysis BTreeMap<ValueId, ValueId>,
    pointer_derivations: PointerDerivationCache,
    pub(super) guarded: Option<GuardedAnalysisV1<'module>>,
}

impl<'analysis, 'module> AccessDerivationContext<'analysis, 'module> {
    pub(super) fn new(
        definitions: &'analysis Definitions<'module>,
        value_types: &'analysis BTreeMap<ValueId, Type>,
        allocations: &'analysis [FormalAllocationParameter],
        allocation_by_value: &'analysis BTreeMap<ValueId, FormalAllocationIdentity>,
        private_load_sources: &'analysis BTreeMap<ValueId, ValueId>,
        guarded: Option<GuardedAnalysisV1<'module>>,
    ) -> Self {
        Self {
            definitions,
            value_types,
            allocations,
            allocation_by_value,
            private_load_sources,
            pointer_derivations: PointerDerivationCache::default(),
            guarded,
        }
    }
}

fn effective_access_space(
    pointer: ValueId,
    access: MemoryAccess,
    allocation: FormalAllocationIdentity,
    location: FunctionOperationLocation,
    context: &AccessDerivationContext<'_, '_>,
) -> Result<AddressSpace, FormalMemoryIncompleteReason> {
    let unsupported =
        || FormalMemoryIncompleteReason::UnsupportedPointerDerivation { location, pointer };
    let Some(Type::Pointer(ty)) = context.value_types.get(&pointer) else {
        return Err(unsupported());
    };
    if ty.address_space != access.address_space {
        return Err(unsupported());
    }
    let ordinal = context
        .allocations
        .binary_search_by_key(&allocation, |row| row.identity)
        .map_err(|_| unsupported())?;
    let plane = context.allocations[ordinal].address_space;
    // The caller has resolved the complete checked alias graph to this root.
    // Generic syntax alone is never sufficient to choose a concrete plane.
    if plane != access.address_space && access.address_space != AddressSpace::Generic {
        return Err(unsupported());
    }
    Ok(plane)
}

impl access_engine::State for AccessDerivationContext<'_, '_> {
    type Error = GuardedResourceErrorV1;
    fn step(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
    fn guarded(
        &mut self,
        location: FunctionOperationLocation,
        pointer: ValueId,
        kind: FormalMemoryAccessKind,
        access: MemoryAccess,
        invocations: InvocationRange1d,
        predicate: Option<ValueId>,
        runtime: bool,
    ) -> Result<Option<FormalMemoryAccess>, AccessDerivationError> {
        let Some(guarded) = &mut self.guarded else {
            return Ok(None);
        };
        if runtime {
            Ok(guarded.runtime_slice_read(
                location,
                pointer,
                kind,
                access,
                invocations,
                predicate,
            )?)
        } else {
            guarded.access(location, pointer, kind, access, invocations, predicate)
        }
    }
    fn width(&mut self, pointer: ValueId) -> Result<Option<u64>, Self::Error> {
        Ok(self.value_types.get(&pointer).and_then(pointer_byte_width))
    }
    fn expression(
        &mut self,
        pointer: ValueId,
        location: FunctionOperationLocation,
    ) -> Result<Result<PointerExpression, FormalMemoryIncompleteReason>, Self::Error> {
        Ok(derive_pointer_expression(
            pointer,
            self.definitions,
            self.value_types,
            self.allocation_by_value,
            self.private_load_sources,
            &mut self.pointer_derivations,
            location,
        ))
    }
    fn allocation(
        &mut self,
        pointer: ValueId,
        location: FunctionOperationLocation,
    ) -> Result<Result<FormalAllocationIdentity, FormalMemoryIncompleteReason>, Self::Error> {
        Ok(derive_pointer_allocation(
            pointer,
            self.definitions,
            self.value_types,
            self.allocation_by_value,
            self.private_load_sources,
            &mut self.pointer_derivations,
            location,
        ))
    }
    fn space(
        &mut self,
        pointer: ValueId,
        access: MemoryAccess,
        allocation: FormalAllocationIdentity,
        location: FunctionOperationLocation,
    ) -> Result<Result<AddressSpace, FormalMemoryIncompleteReason>, Self::Error> {
        Ok(effective_access_space(
            pointer, access, allocation, location, self,
        ))
    }
}

pub(super) fn derive_access(
    location: FunctionOperationLocation,
    pointer: ValueId,
    kind: FormalMemoryAccessKind,
    access: MemoryAccess,
    invocations: InvocationRange1d,
    predicate: Option<ValueId>,
    context: &mut AccessDerivationContext<'_, '_>,
) -> Result<FormalMemoryAccess, AccessDerivationError> {
    access_engine::derive(
        location,
        pointer,
        kind,
        access,
        invocations,
        predicate,
        context,
    )
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
        address_space: effective_access_space(pointer, access, allocation, location, context)?,
        byte_offset: ByteExpression::Unbounded,
        byte_width,
        alignment: u64::from(access.alignment),
        invocations,
        domain: FormalAccessDomainV1::LaunchEnvelope,
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
    let result = engine::allocation(
        pointer,
        &mut legacy::Legacy {
            definitions,
            value_types,
            allocation_by_value,
            private_load_sources,
            cache,
        },
    );
    let result = match result {
        Ok(result) => result,
        Err(never) => match never {},
    };
    result.map_err(|failure| failure.materialize(access_location))
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
    let result = engine::expression(
        pointer,
        &mut legacy::Legacy {
            definitions,
            value_types,
            allocation_by_value,
            private_load_sources,
            cache,
        },
    );
    let result = match result {
        Ok(result) => result,
        Err(never) => match never {},
    };
    result.map_err(|failure| failure.materialize(access_location))
}
