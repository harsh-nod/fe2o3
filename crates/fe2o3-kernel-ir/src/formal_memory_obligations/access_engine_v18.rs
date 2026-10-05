use super::*;

pub(in crate::formal_memory_obligations) trait State {
    type Error;
    fn step(&mut self) -> Result<(), Self::Error>;
    fn guarded(
        &mut self,
        location: FunctionOperationLocation,
        pointer: ValueId,
        kind: FormalMemoryAccessKind,
        access: MemoryAccess,
        invocations: InvocationRange1d,
        predicate: Option<ValueId>,
        runtime: bool,
    ) -> Result<Option<FormalMemoryAccess>, AccessDerivationError<Self::Error>>;
    fn width(&mut self, pointer: ValueId) -> Result<Option<u64>, Self::Error>;
    fn expression(
        &mut self,
        pointer: ValueId,
        location: FunctionOperationLocation,
    ) -> Result<Result<PointerExpression, FormalMemoryIncompleteReason>, Self::Error>;
    fn allocation(
        &mut self,
        pointer: ValueId,
        location: FunctionOperationLocation,
    ) -> Result<Result<FormalAllocationIdentity, FormalMemoryIncompleteReason>, Self::Error>;
    fn space(
        &mut self,
        pointer: ValueId,
        access: MemoryAccess,
        allocation: FormalAllocationIdentity,
        location: FunctionOperationLocation,
    ) -> Result<Result<AddressSpace, FormalMemoryIncompleteReason>, Self::Error>;
}

pub(in crate::formal_memory_obligations) fn derive<S: State>(
    location: FunctionOperationLocation,
    pointer: ValueId,
    kind: FormalMemoryAccessKind,
    access: MemoryAccess,
    invocations: InvocationRange1d,
    predicate: Option<ValueId>,
    state: &mut S,
) -> Result<FormalMemoryAccess, AccessDerivationError<S::Error>> {
    state.step().map_err(AccessDerivationError::Resource)?;
    if let Some(access) = state.guarded(
        location,
        pointer,
        kind,
        access,
        invocations,
        predicate,
        false,
    )? {
        return Ok(access);
    }
    let byte_width = state
        .width(pointer)
        .map_err(AccessDerivationError::Resource)?
        .ok_or(FormalMemoryIncompleteReason::ElementWidthUnavailable { location, pointer })?;
    let expression = match state
        .expression(pointer, location)
        .map_err(AccessDerivationError::Resource)?
    {
        Ok(expression) => expression,
        Err(reason) => {
            if matches!(
                &reason,
                FormalMemoryIncompleteReason::UnsupportedIndexExpression { .. }
            ) && let Some(access) = state.guarded(
                location,
                pointer,
                kind,
                access,
                invocations,
                predicate,
                true,
            )? {
                return Ok(access);
            }
            return Err(reason.into());
        }
    };
    Ok(FormalMemoryAccess {
        location,
        allocation: expression.allocation,
        kind,
        address_space: state
            .space(pointer, access, expression.allocation, location)
            .map_err(AccessDerivationError::Resource)??,
        byte_offset: expression.byte_offset.into_byte_expression(),
        byte_width,
        alignment: u64::from(access.alignment),
        invocations,
        domain: FormalAccessDomainV1::LaunchEnvelope,
    })
}

pub(in crate::formal_memory_obligations) fn conservative_read<S: State>(
    location: FunctionOperationLocation,
    pointer: ValueId,
    access: MemoryAccess,
    invocations: InvocationRange1d,
    state: &mut S,
) -> Result<FormalMemoryAccess, AccessDerivationError<S::Error>> {
    state.step().map_err(AccessDerivationError::Resource)?;
    let byte_width = state
        .width(pointer)
        .map_err(AccessDerivationError::Resource)?
        .ok_or(FormalMemoryIncompleteReason::ElementWidthUnavailable { location, pointer })?;
    let allocation = state
        .allocation(pointer, location)
        .map_err(AccessDerivationError::Resource)??;
    Ok(FormalMemoryAccess {
        location,
        allocation,
        kind: FormalMemoryAccessKind::Read,
        address_space: state
            .space(pointer, access, allocation, location)
            .map_err(AccessDerivationError::Resource)??,
        byte_offset: ByteExpression::Unbounded,
        byte_width,
        alignment: u64::from(access.alignment),
        invocations,
        domain: FormalAccessDomainV1::LaunchEnvelope,
    })
}
