//! The original ordered operation policy, shared by legacy and paid contexts.
use super::*;

pub(super) trait State {
    type Error;
    fn step(&mut self) -> Result<(), Self::Error>;
    fn reachable(&mut self, block: BlockId) -> Result<bool, Self::Error>;
    fn proven_private(&mut self, pointer: ValueId) -> Result<bool, Self::Error>;
    fn ordered_composition(&self) -> bool;
    fn complete_body_v19(&self) -> bool;
    fn call_pure(&mut self, operation: &Operation) -> Result<bool, Self::Error>;
    fn call_reason(
        &mut self,
        location: FunctionOperationLocation,
        callee: &FunctionId,
    ) -> Result<(), Self::Error>;
    fn reason(&mut self, reason: FormalMemoryIncompleteReason) -> Result<(), Self::Error>;
    fn access(
        &mut self,
        location: FunctionOperationLocation,
        operation: &Operation,
        invocations: InvocationRange1d,
        conservative: bool,
    ) -> Result<Result<FormalMemoryAccess, FormalMemoryIncompleteReason>, Self::Error>;
    fn push(&mut self, access: FormalMemoryAccess) -> Result<(), Self::Error>;
    fn closed_assembly(&mut self, operation: &Operation) -> Result<bool, Self::Error>;
    fn local_effects_empty(&mut self, operation: &Operation) -> Result<bool, Self::Error>;
}

pub(super) fn collect<S: State>(
    source: &Function,
    access_invocations: Option<InvocationRange1d>,
    state: &mut S,
) -> Result<(), S::Error> {
    let body = source.body.as_ref().expect("authenticated root definition");
    for block in &body.blocks {
        state.step()?;
        if !state.reachable(block.id)? {
            continue;
        }
        for (operation_index, operation) in block.operations.iter().enumerate() {
            state.step()?;
            let location = FunctionOperationLocation::new(block.id, operation_index);
            let proven_private = match operation.kind {
                OperationKind::Load { pointer, access }
                | OperationKind::Store {
                    pointer, access, ..
                }
                | OperationKind::GuardedLoad {
                    pointer, access, ..
                }
                | OperationKind::GuardedStore {
                    pointer, access, ..
                } if access.address_space == AddressSpace::Generic => {
                    state.proven_private(pointer)?
                }
                _ => false,
            };
            match &operation.kind {
                OperationKind::Call { callee, .. }
                    if !state.ordered_composition() && !state.call_pure(operation)? =>
                {
                    state.call_reason(location, callee)?;
                }
                OperationKind::Call { .. } => {}
                OperationKind::Load { access, .. }
                    if access.address_space == AddressSpace::Private || proven_private => {}
                OperationKind::Load { .. } => {
                    if let Some(invocations) = access_invocations {
                        match state.access(location, operation, invocations, false)? {
                            Ok(access) => state.push(access)?,
                            Err(reason) => state.reason(reason)?,
                        }
                    }
                }
                OperationKind::Store { access, .. }
                    if access.address_space == AddressSpace::Private || proven_private => {}
                OperationKind::Store { .. } | OperationKind::GuardedStore { .. } => {
                    if proven_private {
                        continue;
                    }
                    if let Some(invocations) = access_invocations {
                        match state.access(location, operation, invocations, false)? {
                            Ok(access) => state.push(access)?,
                            Err(reason) => state.reason(reason)?,
                        }
                    }
                }
                OperationKind::Alloca {
                    address_space: AddressSpace::Private,
                    ..
                } => {}
                OperationKind::Matrix(_) if state.local_effects_empty(operation)? => {}
                OperationKind::Gfx950LdsTranspose(_) => {}
                OperationKind::GuardedLoad { access, .. }
                    if access.address_space == AddressSpace::Private || proven_private => {}
                OperationKind::GuardedLoad { .. } => {
                    let mut checked_guard = false;
                    if let Some(invocations) = access_invocations {
                        match state.access(location, operation, invocations, false)? {
                            Ok(access) => {
                                checked_guard =
                                    matches!(access.domain, FormalAccessDomainV1::SliceBounded(_));
                                state.push(access)?;
                            }
                            Err(exact_reason) => {
                                match state.access(location, operation, invocations, true)? {
                                    Ok(access) => state.push(access)?,
                                    Err(_) => state.reason(exact_reason)?,
                                }
                            }
                        }
                    }
                    if !checked_guard {
                        state.reason(
                            FormalMemoryIncompleteReason::GuardedAccessRequiresRankedProof {
                                location,
                            },
                        )?;
                    }
                }
                OperationKind::Atomic(_) => {
                    if let Some(invocations) = access_invocations {
                        match state.access(location, operation, invocations, false)? {
                            Ok(access) => state.push(access)?,
                            Err(reason) => state.reason(reason)?,
                        }
                    }
                }
                OperationKind::InlineAssembly(_) if state.closed_assembly(operation)? => {}
                OperationKind::Gfx942CompleteBodyDeclaration(_)
                | OperationKind::Gfx942CompleteBodyStep(_)
                    if state.complete_body_v19() => {}
                OperationKind::Gfx942OrderedProgram(_) if state.ordered_composition() => {}
                OperationKind::Storage(_)
                | OperationKind::Execution(_)
                | OperationKind::Gfx942OrderedRegion(_)
                | OperationKind::Gfx942OrderedProgram(_)
                | OperationKind::Gfx942CompleteBodyDeclaration(_)
                | OperationKind::Gfx942CompleteBodyStep(_)
                | OperationKind::Gfx942PhysicalEntryDeclaration(_)
                | OperationKind::Gfx942PhysicalEntryStep(_)
                | OperationKind::Gfx942PhysicalGlobalCopyDeclaration(_)
                | OperationKind::Gfx942PhysicalGlobalCopyStep(_)
                | OperationKind::Gfx942PhysicalLdsExchangeDeclaration(_)
                | OperationKind::Gfx942PhysicalLdsExchangeStep(_)
                | OperationKind::VerificationContract(_)
                | OperationKind::VectorLoad(_)
                | OperationKind::VectorStore(_)
                | OperationKind::VectorLayoutConvert(_)
                | OperationKind::Alloca { .. }
                | OperationKind::Barrier(_)
                | OperationKind::Fence(_)
                | OperationKind::Matrix(_)
                | OperationKind::InlineAssembly(_)
                | OperationKind::WorkgroupBarrier(_)
                | OperationKind::WorkgroupMemory(_) => {
                    state.reason(FormalMemoryIncompleteReason::UnsupportedMemoryEffect {
                        location,
                    })?;
                }
                OperationKind::Constant(_)
                | OperationKind::Intrinsic(_)
                | OperationKind::MemoryIntrinsic(_)
                | OperationKind::Wave(_)
                | OperationKind::Unary { .. }
                | OperationKind::Binary { .. }
                | OperationKind::Compare { .. }
                | OperationKind::Cast { .. }
                | OperationKind::Select { .. }
                | OperationKind::SliceLength { .. }
                | OperationKind::SliceData { .. }
                | OperationKind::GetElementPointer { .. } => {
                    if !state.local_effects_empty(operation)? {
                        state.reason(FormalMemoryIncompleteReason::UnsupportedMemoryEffect {
                            location,
                        })?;
                    }
                }
            }
        }
    }
    Ok(())
}
